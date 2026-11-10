//! Accepts ordered Cursor Bidi append requests and routes them by request_id.
use prost::Message;

use crate::{
    cursor::{
        conversation::TransportCommand,
        protocol::{
            events,
            proto::{agent::v1 as agent, aiserver::v1 as ai},
        },
        transport::{TransportParent, TransportRegistry},
    },
    Error, Result,
};

pub struct DecodedAppend {
    pub request_id: String,
    pub seqno: i64,
    pub message: agent::AgentClientMessage,
}

impl DecodedAppend {
    pub fn model_id(&self) -> Option<&str> {
        let agent::agent_client_message::Message::RunRequest(request) =
            self.message.message.as_ref()?
        else {
            return None;
        };
        request
            .requested_model
            .as_ref()
            .map(|model| model.model_id.as_str())
            .filter(|model| !model.is_empty())
            .or_else(|| {
                request
                    .model_details
                    .as_ref()
                    .map(|model| model.model_id.as_str())
                    .filter(|model| !model.is_empty())
            })
    }

    pub fn conversation_id(&self) -> Option<&str> {
        let agent::agent_client_message::Message::RunRequest(request) =
            self.message.message.as_ref()?
        else {
            return None;
        };
        request.conversation_id.as_deref()
    }

    pub fn is_background_task_completion(&self) -> bool {
        let Some(agent::agent_client_message::Message::RunRequest(request)) =
            self.message.message.as_ref()
        else {
            return false;
        };
        matches!(
            request
                .action
                .as_ref()
                .and_then(|action| action.action.as_ref()),
            Some(agent::conversation_action::Action::BackgroundTaskCompletionAction(_))
        )
    }

    pub fn trace_metadata(&self) -> serde_json::Value {
        let Some(message) = self.message.message.as_ref() else {
            return serde_json::json!({
                "append_seqno": self.seqno,
                "message_type": "empty",
            });
        };
        let agent::agent_client_message::Message::RunRequest(request) = message else {
            return serde_json::json!({
                "append_seqno": self.seqno,
                "message_type": client_message_type(message),
            });
        };
        let (action_type, history_messages, history_images) = request
            .action
            .as_ref()
            .and_then(|action| action.action.as_ref())
            .map(|action| match action {
                agent::conversation_action::Action::UserMessageAction(action) => {
                    let history = action.conversation_history.as_ref();
                    (
                        "user_message",
                        history.map_or(0, |history| history.messages.len()),
                        history.map_or(0, history_image_count),
                    )
                }
                agent::conversation_action::Action::BackgroundTaskCompletionAction(_) => {
                    ("background_task_completion", 0, 0)
                }
                agent::conversation_action::Action::ExecutePlanAction(_) => ("execute_plan", 0, 0),
                agent::conversation_action::Action::SummarizeAction(_) => ("summarize", 0, 0),
                _ => ("other", 0, 0),
            })
            .unwrap_or(("none", 0, 0));
        let state = request.conversation_state.as_ref();
        serde_json::json!({
            "append_seqno": self.seqno,
            "message_type": "run_request",
            "conversation_id": request.conversation_id,
            "model_id": self.model_id(),
            "action_type": action_type,
            "conversation_history_messages": history_messages,
            "conversation_history_images": history_images,
            "root_message_count": state.map_or(0, |state| state.root_prompt_messages_json.len()),
            "turn_count": state.map_or(0, |state| state.turns.len()),
            "prefetched_blob_count": request.pre_fetched_blobs.len(),
        })
    }
}

fn client_message_type(message: &agent::agent_client_message::Message) -> &'static str {
    use agent::agent_client_message::Message;
    match message {
        Message::RunRequest(_) => "run_request",
        Message::ExecClientMessage(_) => "exec_client_message",
        Message::ExecClientControlMessage(_) => "exec_client_control_message",
        Message::KvClientMessage(_) => "kv_client_message",
        Message::ConversationAction(_) => "conversation_action",
        Message::InteractionResponse(_) => "interaction_response",
        Message::ClientHeartbeat(_) => "client_heartbeat",
        Message::PrewarmRequest(_) => "prewarm_request",
    }
}

fn history_image_count(history: &agent::ConversationHistory) -> usize {
    use agent::{
        conversation_history_message::Message,
        conversation_history_tool_result_content::Content as ToolContent,
        conversation_history_user_content::Content as UserContent,
    };
    history
        .messages
        .iter()
        .map(|message| match message.message.as_ref() {
            Some(Message::User(user)) => user
                .content
                .iter()
                .filter(|content| matches!(content.content, Some(UserContent::Image(_))))
                .count(),
            Some(Message::Tool(tool)) => tool
                .content
                .iter()
                .filter(|content| matches!(content.content, Some(ToolContent::Image(_))))
                .count(),
            _ => 0,
        })
        .sum()
}

pub fn decode(request: &ai::BidiAppendRequest) -> Result<DecodedAppend> {
    let request_id = request
        .request_id
        .as_ref()
        .map(|id| id.request_id.as_str())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| Error::Protocol("BidiAppend request_id is required".into()))?;
    // 客户端按 binaryEncoding 开关二选一：`data_binary` 是原始字节，`data` 是同一消息的 hex 文本。
    let hex_payload;
    let payload = match (request.data.is_empty(), request.data_binary.is_empty()) {
        (true, true) => {
            return Err(Error::Protocol(
                "BidiAppend contains no AgentClientMessage".into(),
            ))
        }
        (false, false) => {
            return Err(Error::Protocol(
                "BidiAppend must carry either data or data_binary, not both".into(),
            ))
        }
        (false, true) => {
            hex_payload = hex::decode(&request.data)
                .map_err(|error| Error::Protocol(format!("invalid BidiAppend hex: {error}")))?;
            hex_payload.as_slice()
        }
        (true, false) => request.data_binary.as_slice(),
    };
    Ok(DecodedAppend {
        request_id: request_id.into(),
        seqno: request.append_seqno,
        message: agent::AgentClientMessage::decode(payload).map_err(|error| {
            // 只记录字段号、线型和长度，不记录任何内容，便于定位客户端与本地 proto 的差异。
            tracing::warn!(
                request_id,
                payload_len = payload.len(),
                shape = wire_shape(payload, 3),
                %error,
                "BidiAppend AgentClientMessage failed to decode"
            );
            Error::from(error)
        })?,
    })
}

/// 以 `字段号:线型` 的形式描述 protobuf 的线格式结构，仅用于解码失败时的诊断。
///
/// 长度分隔字段在能完整解析为嵌套消息时展开为 `{...}`，否则只给出长度；条目数有上限。
fn wire_shape(mut buf: &[u8], depth: usize) -> String {
    use prost::encoding::decode_varint;
    const MAX_ITEMS: usize = 48;
    let mut items = Vec::new();
    while !buf.is_empty() {
        if items.len() == MAX_ITEMS {
            items.push("…".to_owned());
            break;
        }
        let Ok(key) = decode_varint(&mut buf) else {
            items.push("<bad key>".to_owned());
            break;
        };
        let (tag, wire_type) = (key >> 3, key & 7);
        match wire_type {
            0 => match decode_varint(&mut buf) {
                Ok(_) => items.push(format!("{tag}:varint")),
                Err(_) => {
                    items.push(format!("{tag}:<bad varint>"));
                    break;
                }
            },
            1 | 5 => {
                let size = if wire_type == 1 { 8 } else { 4 };
                if buf.len() < size {
                    items.push(format!("{tag}:<truncated fixed>"));
                    break;
                }
                buf = &buf[size..];
                items.push(format!("{tag}:fixed{}", size * 8));
            }
            2 => {
                let Ok(length) = decode_varint(&mut buf) else {
                    items.push(format!("{tag}:<bad length>"));
                    break;
                };
                let length = length as usize;
                if length > buf.len() {
                    items.push(format!("{tag}:len {length} > remaining {}", buf.len()));
                    break;
                }
                let (value, rest) = buf.split_at(length);
                buf = rest;
                let nested = (depth > 0 && !value.is_empty() && is_wire_message(value))
                    .then(|| wire_shape(value, depth - 1));
                match nested {
                    Some(nested) => items.push(format!("{tag}:{{{nested}}}")),
                    None => items.push(format!("{tag}:len {length}")),
                }
            }
            other => {
                items.push(format!("{tag}:<wire type {other}>"));
                break;
            }
        }
    }
    items.join(" ")
}

fn is_wire_message(mut buf: &[u8]) -> bool {
    use prost::encoding::decode_varint;
    while !buf.is_empty() {
        let Ok(key) = decode_varint(&mut buf) else {
            return false;
        };
        if key >> 3 == 0 {
            return false;
        }
        match key & 7 {
            0 => {
                if decode_varint(&mut buf).is_err() {
                    return false;
                }
            }
            1 | 5 => {
                let size = if key & 7 == 1 { 8 } else { 4 };
                if buf.len() < size {
                    return false;
                }
                buf = &buf[size..];
            }
            2 => {
                let Ok(length) = decode_varint(&mut buf) else {
                    return false;
                };
                if length as usize > buf.len() {
                    return false;
                }
                buf = &buf[length as usize..];
            }
            _ => return false,
        }
    }
    true
}

pub async fn append(
    registry: &TransportRegistry,
    request: DecodedAppend,
    parent: Option<TransportParent>,
) -> Result<ai::BidiAppendResponse> {
    let replace_closing = request.model_id().is_some();
    let handle = registry
        .get_or_create_for_append(&request.request_id, replace_closing)
        .await?;
    let _admission = handle.admit()?;
    if let Some(conversation_id) = request.conversation_id() {
        handle.set_conversation_id(conversation_id)?;
    }
    if let Some(parent) = parent {
        handle.set_parent(parent)?;
    }
    if matches!(
        request.message.message.as_ref(),
        Some(agent::agent_client_message::Message::ClientHeartbeat(_))
    ) {
        handle.emit(&events::heartbeat())?;
    }
    handle
        .command(TransportCommand::Append {
            seqno: request.seqno,
            message: Box::new(request.message),
        })
        .await?;
    Ok(ai::BidiAppendResponse {})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heartbeat() -> agent::AgentClientMessage {
        agent::AgentClientMessage {
            message: Some(agent::agent_client_message::Message::ClientHeartbeat(
                agent::ClientHeartbeat::default(),
            )),
        }
    }

    fn request(data: String, data_binary: Vec<u8>) -> ai::BidiAppendRequest {
        ai::BidiAppendRequest {
            data,
            request_id: Some(ai::BidiRequestId {
                request_id: "request".into(),
            }),
            append_seqno: 7,
            data_binary,
        }
    }

    #[test]
    fn hex_and_binary_payloads_decode_to_the_same_message() {
        let bytes = heartbeat().encode_to_vec();
        let from_hex = decode(&request(hex::encode(&bytes), Vec::new())).unwrap();
        let from_binary = decode(&request(String::new(), bytes)).unwrap();
        assert_eq!(from_hex.message, from_binary.message);
        assert_eq!(from_binary.seqno, 7);
        assert_eq!(from_binary.request_id, "request");
    }

    #[test]
    fn wire_shape_describes_structure_without_content() {
        // field 1: nested { field 2: "secret" }, field 3: varint 7, field 4: truncated length
        let valid = [0x0a, 0x08, 0x12, 0x06, b's', b'e', b'c', b'r', b'e', b't', 0x18, 0x07];
        let shape = wire_shape(&valid, 3);
        assert_eq!(shape, "1:{2:len 6} 3:varint");
        assert!(!shape.contains("secret"));
        assert_eq!(wire_shape(&[0x22, 0x05, 0x01], 3), "4:len 5 > remaining 1");
        assert_eq!(wire_shape(&[0x0b], 3), "1:<wire type 3>");
    }

    #[test]
    fn empty_or_ambiguous_payloads_are_rejected() {
        let bytes = heartbeat().encode_to_vec();
        assert!(decode(&request(String::new(), Vec::new())).is_err());
        assert!(decode(&request(hex::encode(&bytes), bytes)).is_err());
    }
}
