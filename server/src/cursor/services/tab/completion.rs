//! Runs one cancellable, tool-free Provider call and returns a bounded Connect edit.
use std::{convert::Infallible, sync::Arc};

use axum::{
    body::Body,
    http::{header, HeaderValue, Response},
};
use bytes::Bytes;
use futures_util::StreamExt;
use tokio::{sync::OwnedSemaphorePermit, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::{
    cursor::protocol::connect::{self, ConnectCode, ConnectStreamError},
    model::{
        ContentPart, ModelInvocation, ModelLatency, ModelRequest, ModelSpec, ProjectedContent,
        ProjectedMessage, PromptSpec, Role,
    },
    provider::{FinishReason, ModelEvent, Provider},
    Error, Result,
};

use super::{
    context::{CompletionContext, INSTRUCTIONS, MAX_OUTPUT_BYTES, OUTPUT_TOKENS},
    proto::StreamCppResponse,
};

pub(super) fn respond(
    provider: Arc<dyn Provider>,
    model_id: String,
    context: CompletionContext,
    context_tokens: Option<u64>,
    deadline: Instant,
    permit: OwnedSemaphorePermit,
) -> Response<Body> {
    let cancellation = CancellationToken::new();
    let cancel_on_drop = cancellation.clone().drop_guard();
    let invocation = invocation(model_id, context.user_content.clone(), context_tokens);
    let stream = async_stream::stream! {
        let _permit = permit;
        let _cancel_on_drop = cancel_on_drop;
        let result = generate(provider, invocation, deadline, cancellation.clone()).await
            .and_then(|text| context.replacement(&text));
        match result {
            Ok(Some(text)) => {
                let range = context.range();
                let messages = [
                    StreamCppResponse {
                        suggestion_start_line: Some(range.start_line_number),
                        range_to_replace: Some(range), begin_edit: Some(true), ..Default::default()
                    },
                    StreamCppResponse { text, ..Default::default() },
                    StreamCppResponse { done_edit: Some(true), done_stream: Some(true), ..Default::default() },
                ];
                for message in messages {
                    match connect::encode_message(&message) {
                        Ok(frame) => yield Ok::<Bytes, Infallible>(frame),
                        Err(error) => {
                            yield Ok(error_frame(&error));
                            return;
                        }
                    }
                }
                yield Ok(connect::encode_end_stream());
            }
            Ok(None) => {
                if let Ok(frame) = connect::encode_message(&StreamCppResponse {
                    done_stream: Some(true), ..Default::default()
                }) { yield Ok(frame); }
                yield Ok(connect::encode_end_stream());
            }
            Err(error) => {
                cancellation.cancel();
                yield Ok(error_frame(&error));
            }
        }
    };
    connect_response(Body::from_stream(stream))
}

fn invocation(model_id: String, content: String, context_tokens: Option<u64>) -> ModelInvocation {
    let call_id = format!("tab-{}", uuid::Uuid::new_v4());
    ModelInvocation {
        call_id: call_id.clone(),
        run_id: call_id.clone(),
        conversation_id: call_id,
        provider_call_index: 0,
        request: ModelRequest {
            prompt: PromptSpec {
                instructions: INSTRUCTIONS.into(),
                tools: Vec::new(),
            },
            model: ModelSpec {
                max_output_tokens: Some(OUTPUT_TOKENS),
                context_window_tokens: context_tokens,
                latency: ModelLatency::Fast,
                ..ModelSpec::new(model_id)
            },
            history: vec![ProjectedMessage {
                message_id: "tab-context".into(),
                role: Role::User,
                content: ProjectedContent::Parts(vec![ContentPart::Text { text: content }]),
            }],
        },
    }
}

async fn generate(
    provider: Arc<dyn Provider>,
    invocation: ModelInvocation,
    deadline: Instant,
    cancellation: CancellationToken,
) -> Result<String> {
    if Instant::now() >= deadline {
        return Err(Error::Provider("TAB completion deadline exceeded".into()));
    }
    let mut stream = provider.stream(invocation, cancellation.clone());
    let result = tokio::time::timeout_at(deadline, async {
        let mut text = String::new();
        loop {
            let event = tokio::select! {
                _ = cancellation.cancelled() => return Err(Error::Cancelled),
                event = stream.next() => event,
            };
            match event {
                Some(Ok(ModelEvent::TextDelta(delta))) => {
                    if text.len().saturating_add(delta.len()) > MAX_OUTPUT_BYTES {
                        return Err(Error::Provider(
                            "TAB completion exceeds the supported output size".into(),
                        ));
                    }
                    text.push_str(&delta);
                }
                Some(Ok(ModelEvent::Done(FinishReason::Stop))) => return Ok(text),
                Some(Ok(ModelEvent::Done(FinishReason::Length))) => {
                    return Err(Error::Provider(
                        "TAB completion was truncated at its token limit".into(),
                    ));
                }
                Some(Ok(
                    ModelEvent::ToolCallStart { .. }
                    | ModelEvent::ToolCallArgumentsDelta { .. }
                    | ModelEvent::ToolCallEnd { .. }
                    | ModelEvent::Done(FinishReason::ToolUse),
                )) => {
                    return Err(Error::Provider(
                        "TAB completion must not invoke tools".into(),
                    ));
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error),
                None => {
                    return Err(Error::Provider(
                        "TAB provider stream ended without Done".into(),
                    ))
                }
            }
        }
    })
    .await;
    match result {
        Ok(result) => result,
        Err(_) => {
            cancellation.cancel();
            Err(Error::Provider("TAB completion deadline exceeded".into()))
        }
    }
}

pub(super) fn empty_stream() -> Response<Body> {
    connect_response(Body::from(connect::encode_end_stream()))
}

pub(super) fn error_response(error: &Error) -> Response<Body> {
    connect_response(Body::from(error_frame(error)))
}

fn error_frame(error: &Error) -> Bytes {
    let code = match error {
        Error::Protocol(_) | Error::Decode(_) | Error::Config(_) => ConnectCode::InvalidArgument,
        Error::Cancelled => ConnectCode::Canceled,
        _ => ConnectCode::Unavailable,
    };
    connect::encode_error_end_stream(&ConnectStreamError {
        code,
        message: error.to_string(),
        details: Vec::new(),
    })
    .expect("a TAB Connect error consists only of JSON strings")
}

fn connect_response(body: Body) -> Response<Body> {
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/connect+proto"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
