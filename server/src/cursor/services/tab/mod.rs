//! Handles basic local TAB completion independently of Cursor account credentials.
//! Only the explicitly selected Direct mode proxies requests to Cursor.
mod completion;
mod context;
pub mod proto;

use std::{sync::Arc, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::{Extension, State},
    http::{header, HeaderValue, Request, Response},
    routing::post,
    Router,
};
use prost::Message;
use tokio::{sync::Semaphore, time::Instant};

use crate::{
    api::cursor::proxy,
    cursor::{protocol::connect, transport::TransportRegistry},
    plugin::ADAPTER_ID_PREFIX,
    store::{TabMode, TabSettings},
    Error, Result,
};

const MAX_REQUEST_BYTES: usize = 256 * 1024;
const GENERATION_TIMEOUT: Duration = Duration::from_secs(8);
// Keep the normal Cursor origin so the desktop relay sends these requests to the
// existing local backend. Never advertise an arbitrary request-supplied URL.
const TAB_ORIGIN: &str = "https://api2.cursor.sh";

pub const TAB_PATHS: [&str; 16] = [
    "/aiserver.v1.AiService/StreamCpp",
    "/aiserver.v1.AiService/StreamNextCursorPrediction",
    "/aiserver.v1.AiService/GetCppEditClassification",
    "/aiserver.v1.AiService/RefreshTabContext",
    "/aiserver.v1.AiService/CppConfig",
    "/aiserver.v1.AiService/CppEditHistoryStatus",
    "/aiserver.v1.AiService/CppAppend",
    "/aiserver.v1.AiService/CppEditHistoryAppend",
    "/aiserver.v1.AiService/ReportAiCodeChangeMetrics",
    "/aiserver.v1.AiService/WriteGitBranchName",
    "/aiserver.v1.CppService/AvailableModels",
    "/aiserver.v1.CppService/RecordCppFate",
    "/aiserver.v1.FileSyncService/FSSyncFile",
    "/aiserver.v1.FileSyncService/FSIsEnabledForUser",
    "/aiserver.v1.FileSyncService/FSConfig",
    "/aiserver.v1.FileSyncService/FSUploadFile",
];

pub fn is_tab_path(path: &str) -> bool {
    TAB_PATHS.contains(&path)
        || path.starts_with("/aiserver.v1.CppService/")
        || path.starts_with("/aiserver.v1.FileSyncService/")
        || path
            .strip_prefix("/aiserver.v1.AiService/")
            .is_some_and(|method| method.contains("Cpp") || method.contains("Tab"))
}

pub fn router() -> Router<TransportRegistry> {
    let router = TAB_PATHS.into_iter().fold(Router::new(), |router, path| {
        router.route(path, post(handle))
    });
    router
        .route("/aiserver.v1.CppService/{*method}", post(handle))
        .route("/aiserver.v1.FileSyncService/{*method}", post(handle))
        .layer(Extension(Arc::new(Semaphore::new(1))))
}

async fn handle(
    State(registry): State<TransportRegistry>,
    Extension(upstream): Extension<proxy::CursorProxy>,
    Extension(gate): Extension<Arc<Semaphore>>,
    request: Request<Body>,
) -> Result<Response<Body>> {
    let started = Instant::now();
    let is_stream = matches!(
        request.uri().path(),
        "/aiserver.v1.AiService/StreamCpp" | "/aiserver.v1.AiService/StreamNextCursorPrediction"
    );
    let settings = registry.store().tab_settings().await?;
    if settings.mode == TabMode::Direct {
        return proxy::forward(Extension(upstream), request).await;
    }
    let result = local(&registry, &settings, gate, request, started).await;
    match result {
        Err(error) if is_stream => Ok(completion::error_response(&error)),
        result => result,
    }
}

async fn local(
    registry: &TransportRegistry,
    settings: &TabSettings,
    gate: Arc<Semaphore>,
    request: Request<Body>,
    started: Instant,
) -> Result<Response<Body>> {
    let (parts, body) = request.into_parts();
    let timeout = match parts.headers.get("connect-timeout-ms") {
        Some(value) => {
            let millis = value
                .to_str()
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| Error::Protocol("invalid TAB connect-timeout-ms".into()))?;
            Duration::from_millis(millis).min(GENERATION_TIMEOUT)
        }
        None => GENERATION_TIMEOUT,
    };
    let deadline = started + timeout;
    let bytes = tokio::time::timeout_at(deadline, to_bytes(body, MAX_REQUEST_BYTES))
        .await
        .map_err(|_| Error::Provider("TAB request deadline exceeded".into()))?
        .map_err(|_| {
            Error::Protocol(
                "TAB request body exceeds the supported size or could not be read".into(),
            )
        })?;
    let path = parts.uri.path();
    match path {
        "/aiserver.v1.AiService/StreamCpp" => {
            let model_id = settings.model_id.trim();
            if model_id.is_empty() {
                return Err(Error::Config(
                    "select a configured TAB model in the TAB settings".into(),
                ));
            }
            if parts
                .headers
                .get("connect-content-encoding")
                .is_some_and(|value| value != "identity")
            {
                return Err(Error::Protocol(
                    "local TAB supports uncompressed Connect requests only".into(),
                ));
            }
            if let Some(content_type) = parts.headers.get(header::CONTENT_TYPE) {
                let value = content_type.to_str().unwrap_or_default();
                if !value.starts_with("application/proto")
                    && !value.starts_with("application/connect+proto")
                {
                    return Err(Error::Protocol(
                        "local TAB requires protobuf requests".into(),
                    ));
                }
            }
            // The shared unary decoder accepts plain protobuf and one identity envelope.
            // Check a Connect envelope's flags before decoding, rather than treating
            // compressed protobuf bytes as a valid file snapshot.
            if bytes.len() >= 5
                && u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize
                    == bytes.len() - 5
                && bytes[0] != 0
            {
                return Err(Error::Protocol(
                    "local TAB requires an identity Connect message envelope".into(),
                ));
            }
            let request: proto::StreamCppRequest = connect::decode_unary(&bytes)?;
            let file = request.current_file.ok_or_else(|| {
                Error::Protocol("current_file is required for TAB completion".into())
            })?;
            let context_tokens = configured_model(registry, model_id).await?;
            let context = context::CompletionContext::from_file(&file, context_tokens)?;
            let permit = gate.try_acquire_owned()
                .map_err(|_| Error::Provider("local TAB is busy; a newer completion can be requested after the current one ends".into()))?;
            Ok(completion::respond(
                registry.conversations().dependencies().provider.clone(),
                model_id.to_owned(),
                context,
                context_tokens,
                deadline,
                permit,
            ))
        }
        "/aiserver.v1.AiService/StreamNextCursorPrediction" => Ok(completion::empty_stream()),
        "/aiserver.v1.AiService/CppConfig" => {
            let enabled = !settings.model_id.trim().is_empty()
                && configured_model(registry, settings.model_id.trim())
                    .await
                    .is_ok();
            unary(&proto::CppConfigResponse {
                above_radius: Some(0),
                below_radius: Some(0),
                is_on: Some(enabled),
                is_ghost_text: Some(true),
                should_let_user_enable_cpp_even_if_not_pro: Some(true),
                global_debounce_duration_millis: 300,
                client_debounce_duration_millis: 300,
                cpp_url: TAB_ORIGIN.into(),
                geo_cpp_backend_url: TAB_ORIGIN.into(),
                import_prediction_config: Some(proto::ImportPredictionConfig {
                    is_disabled_by_backend: true,
                    should_turn_on_automatically: false,
                    python_enabled: false,
                }),
                include_unchanged_lines: true,
                ..Default::default()
            })
        }
        "/aiserver.v1.CppService/AvailableModels" => {
            let model_id = settings.model_id.trim();
            let enabled =
                !model_id.is_empty() && configured_model(registry, model_id).await.is_ok();
            unary(&proto::AvailableCppModelsResponse {
                models: if enabled {
                    vec![model_id.into()]
                } else {
                    Vec::new()
                },
                default_model: enabled.then(|| model_id.into()),
            })
        }
        "/aiserver.v1.FileSyncService/FSIsEnabledForUser" => {
            unary(&proto::FsIsEnabledForUserResponse { enabled: false })
        }
        "/aiserver.v1.FileSyncService/FSUploadFile" | "/aiserver.v1.FileSyncService/FSSyncFile" => {
            unary(&proto::FileSyncResponse { error: 1 })
        }
        "/aiserver.v1.AiService/CppEditHistoryStatus" => {
            unary(&proto::CppEditHistoryStatusResponse {
                on: false,
                only_if_explicit: true,
            })
        }
        "/aiserver.v1.AiService/CppAppend" => unary(&proto::CppAppendResponse { success: false }),
        "/aiserver.v1.AiService/GetCppEditClassification" => {
            let enabled = !settings.model_id.trim().is_empty()
                && configured_model(registry, settings.model_id.trim())
                    .await
                    .is_ok();
            // Basic insertion is already bounded and validated by completion.rs.
            // Do not globally reject every suggestion in clients that consult this flag.
            unary(&proto::GetCppEditClassificationResponse {
                should_noop: Some(!enabled),
            })
        }
        "/aiserver.v1.AiService/WriteGitBranchName" => Err(Error::Protocol(
            "branch-name generation is not supported by basic local TAB".into(),
        )),
        "/aiserver.v1.AiService/RefreshTabContext"
        | "/aiserver.v1.AiService/CppEditHistoryAppend"
        | "/aiserver.v1.AiService/ReportAiCodeChangeMetrics"
        | "/aiserver.v1.CppService/RecordCppFate"
        | "/aiserver.v1.FileSyncService/FSConfig" => unary(&proto::EmptyResponse {}),
        _ => Err(Error::Protocol("unsupported local TAB method".into())),
    }
}

async fn configured_model(registry: &TransportRegistry, model_id: &str) -> Result<Option<u64>> {
    if model_id.starts_with(ADAPTER_ID_PREFIX) {
        registry
            .plugins()
            .ok_or_else(|| Error::Config("TAB plugin model is unavailable".into()))?
            .model_descriptor(model_id)
            .await?;
        return Ok(None);
    }
    let model = registry
        .store()
        .model(model_id)
        .await?
        .ok_or_else(|| Error::Config("the selected TAB model is not configured".into()))?;
    // Provider routing owns saved reasoning settings; TAB does not rewrite them.
    // Slow models are bounded by the TAB deadline and never expose reasoning as code.
    Ok(model.context_window_tokens)
}

fn unary<M: Message>(message: &M) -> Result<Response<Body>> {
    let mut response = Response::new(Body::from(message.encode_to_vec()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/proto"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
