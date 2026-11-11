//! Implements Cursor analytics endpoints and event handling.
use axum::{
    body::{Body, Bytes},
    extract::Extension,
    http::{header, HeaderValue, Request, Response, StatusCode},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::{BufMut, BytesMut};
use prost::Message;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::{api::cursor::proxy, Error, Result};

pub const BOOTSTRAP_STATSIG_PATH: &str = "/aiserver.v1.AnalyticsService/BootstrapStatsig";
const AGENT_RETRIES_GATE: &str = "nal_agent_retries";
// 打开后 Cursor 3.29+ 会把 Agent 回合交给客户端自带的本地循环，并改用 RunInference 协议；
// 本服务只实现 NAL 循环（RunSSE/BidiAppend），必须保持关闭。
const LOCAL_LOOP_GATE: &str = "agent_host_local_loop";
const LOCAL_RULE: &str = "local_enabled";

#[derive(Clone, PartialEq, Message)]
struct BootstrapStatsigResponse {
    #[prost(string, tag = "1")]
    config: String,
    #[prost(uint64, tag = "2")]
    generated_at_ms: u64,
}

pub async fn bootstrap_statsig(
    Extension(upstream): Extension<proxy::CursorProxy>,
    request: Request<Body>,
) -> Result<Response<Body>> {
    match proxy::forward_buffered(&upstream, request).await {
        Ok(response) if response.status.is_success() => match patch_upstream(response) {
            Ok(response) => Ok(response),
            Err(error) => {
                tracing::warn!(%error, "Cursor Statsig bootstrap was invalid; using local bootstrap");
                local_response()
            }
        },
        Ok(response) => {
            tracing::warn!(status = %response.status, "Cursor Statsig bootstrap was rejected; using local bootstrap");
            local_response()
        }
        Err(error) => {
            tracing::warn!(%error, "Cursor Statsig bootstrap was unavailable; using local bootstrap");
            local_response()
        }
    }
}

fn patch_upstream(response: proxy::BufferedResponse) -> Result<Response<Body>> {
    let (framed, payload) = unary_payload(&response.body)?;
    let mut message = BootstrapStatsigResponse::decode(payload)?;
    let mut config = serde_json::from_str::<Value>(&message.config)?;
    apply_gate_overrides(&mut config)?;
    message.config = serde_json::to_string(&config)?;
    Ok(response.with_body(encode_unary(&message, framed)))
}

fn local_response() -> Result<Response<Body>> {
    let generated_at_ms = chrono::Utc::now().timestamp_millis() as u64;
    let mut config = json!({
        "feature_gates": {},
        "dynamic_configs": {},
        "layer_configs": {},
        "user": {
            "userID": "local_ultra",
            "customIDs": { "localUserID": "local_ultra" }
        },
        "has_updates": true,
        "hash_used": "none",
        "sdkParams": {
            "stableID": "local_ultra",
            "disableDiagnosticsLogging": true
        },
        "time": generated_at_ms
    });
    apply_gate_overrides(&mut config)?;
    let message = BootstrapStatsigResponse {
        config: serde_json::to_string(&config)?,
        generated_at_ms,
    };
    let body = message.encode_to_vec();
    let mut response = Response::new(Body::from(body.clone()));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/proto"),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        body.len()
            .to_string()
            .parse()
            .expect("body length is a valid header value"),
    );
    Ok(response)
}

fn apply_gate_overrides(config: &mut Value) -> Result<()> {
    set_gate(config, AGENT_RETRIES_GATE, true)?;
    set_gate(config, LOCAL_LOOP_GATE, false)
}

fn set_gate(config: &mut Value, name: &str, value: bool) -> Result<()> {
    let gate_key = statsig_key(config, name);
    let root = config
        .as_object_mut()
        .ok_or_else(|| Error::Protocol("Statsig bootstrap config must be an object".into()))?;
    let gates = root
        .entry("feature_gates")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| Error::Protocol("Statsig feature_gates must be an object".into()))?;
    gates.insert(gate_key.clone(), gate(&gate_key, value));
    Ok(())
}

fn statsig_key(config: &Value, name: &str) -> String {
    match config.get("hash_used").and_then(Value::as_str) {
        Some("djb2") => djb2(name),
        Some("sha256") => STANDARD.encode(Sha256::digest(name.as_bytes())),
        _ => name.to_owned(),
    }
}

fn djb2(value: &str) -> String {
    value
        .encode_utf16()
        .fold(0_u32, |hash, character| {
            hash.wrapping_mul(31).wrapping_add(u32::from(character))
        })
        .to_string()
}

fn gate(name: &str, value: bool) -> Value {
    json!({
        "name": name,
        "value": value,
        "rule_id": LOCAL_RULE,
        "ruleID": LOCAL_RULE,
        "group_name": LOCAL_RULE,
        "groupName": LOCAL_RULE,
        "secondary_exposures": [],
        "secondaryExposures": [],
        "undelegated_secondary_exposures": [],
        "undelegatedSecondaryExposures": [],
        "is_device_based": false,
        "isDeviceBased": false,
        "id_type": "userID",
        "idType": "userID"
    })
}

fn unary_payload(body: &Bytes) -> Result<(bool, &[u8])> {
    if body.len() < 5 {
        return Ok((false, body));
    }
    let flags = body[0];
    let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
    if length != body.len() - 5 {
        return Ok((false, body));
    }
    if flags != 0 {
        return Err(Error::Protocol(format!(
            "cannot patch compressed or terminal Statsig frame: flags={flags}"
        )));
    }
    Ok((true, &body[5..]))
}

fn encode_unary(message: &impl Message, framed: bool) -> Bytes {
    let payload = message.encode_to_vec();
    if !framed {
        return Bytes::from(payload);
    }
    let mut output = BytesMut::with_capacity(5 + payload.len());
    output.put_u8(0);
    output.put_u32(payload.len() as u32);
    output.extend_from_slice(&payload);
    output.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate_value(config: &Value, name: &str) -> Option<bool> {
        let key = statsig_key(config, name);
        config["feature_gates"][&key]["value"].as_bool()
    }

    #[test]
    fn local_loop_is_forced_off_and_retries_forced_on_for_every_hash_mode() {
        for hash_used in ["none", "djb2", "sha256"] {
            let mut config = json!({"hash_used": hash_used, "feature_gates": {}});
            // 上游把本地循环开关设为 true，必须被覆盖。
            let upstream_key = statsig_key(&config, LOCAL_LOOP_GATE);
            config["feature_gates"][&upstream_key] = gate(&upstream_key, true);

            apply_gate_overrides(&mut config).unwrap();

            assert_eq!(
                gate_value(&config, LOCAL_LOOP_GATE),
                Some(false),
                "{hash_used}"
            );
            assert_eq!(
                gate_value(&config, AGENT_RETRIES_GATE),
                Some(true),
                "{hash_used}"
            );
        }
    }

    #[test]
    fn local_bootstrap_disables_the_local_loop() {
        let response = local_response().unwrap();
        let body = response.into_body();
        let bytes = futures_util::FutureExt::now_or_never(axum::body::to_bytes(body, usize::MAX))
            .expect("body is ready")
            .unwrap();
        let message = BootstrapStatsigResponse::decode(bytes.as_ref()).unwrap();
        let config: Value = serde_json::from_str(&message.config).unwrap();
        assert_eq!(gate_value(&config, LOCAL_LOOP_GATE), Some(false));
        assert_eq!(gate_value(&config, AGENT_RETRIES_GATE), Some(true));
    }

    #[test]
    fn patched_upstream_response_keeps_other_gates_and_framing() {
        let mut config = json!({"hash_used": "djb2", "feature_gates": {"keep": {"value": true}}});
        let key = statsig_key(&config, LOCAL_LOOP_GATE);
        config["feature_gates"][&key] = gate(&key, true);
        let message = BootstrapStatsigResponse {
            config: serde_json::to_string(&config).unwrap(),
            generated_at_ms: 7,
        };
        let framed = encode_unary(&message, true);
        let response = proxy::BufferedResponse {
            status: StatusCode::OK,
            headers: Default::default(),
            body: framed,
        };
        let patched = patch_upstream(response).unwrap();
        let bytes = futures_util::FutureExt::now_or_never(axum::body::to_bytes(
            patched.into_body(),
            usize::MAX,
        ))
        .expect("body is ready")
        .unwrap();
        assert_eq!(bytes[0], 0, "connect envelope is preserved");
        let decoded = BootstrapStatsigResponse::decode(&bytes[5..]).unwrap();
        assert_eq!(decoded.generated_at_ms, 7);
        let config: Value = serde_json::from_str(&decoded.config).unwrap();
        assert_eq!(config["feature_gates"]["keep"]["value"], true);
        assert_eq!(gate_value(&config, LOCAL_LOOP_GATE), Some(false));
    }
}
