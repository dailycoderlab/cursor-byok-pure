//! Exercises local TAB wire contracts without Cursor, credentials, or model network calls.
#[path = "support/fixtures.rs"]
mod fixtures;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    extract::Extension,
    http::{Request, Response, StatusCode},
    Router,
};
use cursor_server::{
    api::cursor::proxy::CursorProxy,
    cursor::transport::TransportRegistry,
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::connect,
        services::tab::{self, proto},
    },
    model::{ModelConfigInput, ModelInvocation, ModelType, OPENAI_CHAT_ENDPOINT},
    network::NetworkClients,
    provider::{FinishReason, ModelEvent, Provider, ProviderStream},
    store::{Store, TabMode, TabSettings},
    Error,
};
use futures_util::{stream, StreamExt};
use prost::Message;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

#[derive(Clone, Default)]
struct TestProvider {
    requests: Arc<Mutex<Vec<ModelInvocation>>>,
    tokens: Arc<Mutex<Vec<CancellationToken>>>,
    events: Arc<Mutex<Vec<Result<ModelEvent, Error>>>>,
    pending: bool,
}

impl Provider for TestProvider {
    fn stream(
        &self,
        invocation: ModelInvocation,
        cancellation: CancellationToken,
    ) -> ProviderStream {
        self.requests.lock().unwrap().push(invocation);
        self.tokens.lock().unwrap().push(cancellation);
        if self.pending {
            Box::pin(stream::pending())
        } else {
            Box::pin(stream::iter(std::mem::take(
                &mut *self.events.lock().unwrap(),
            )))
        }
    }
}

async fn setup(provider: TestProvider, selected: bool) -> (Router, Store, tempfile::TempDir) {
    let (directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&ModelConfigInput {
            sort_order: 0,
            display_name: "TAB fake".into(),
            group_name: None,
            model_type: ModelType::OpenAi,
            base_url: "https://not-contacted.invalid/v1".into(),
            use_full_url: false,
            api_key: "own-model-key".into(),
            tooltip_data: "offline TAB test model".into(),
            model_id: "fake-completion".into(),
            reasoning_effort: None,
            openai_endpoint: OPENAI_CHAT_ENDPOINT.into(),
            openai_extra_params_enabled: false,
            openai_extra_params: json!({}),
            custom_headers_enabled: false,
            custom_headers: json!({}),
            anthropic_extra_params_enabled: false,
            anthropic_extra_params: json!({}),
            context_window_tokens: Some(2048),
            max_completion_tokens: None,
            anthropic_max_tokens: None,
            anthropic_thinking_effort: None,
            thinking_budget_tokens: None,
        })
        .await
        .unwrap();
    store
        .set_tab_settings(TabSettings {
            mode: TabMode::Local,
            model_id: if selected {
                model.model_hash
            } else {
                String::new()
            },
        })
        .await
        .unwrap();
    let compiler = PromptCompiler::new(PromptAssets::embedded().unwrap());
    let registry = TransportRegistry::new(store.clone(), Arc::new(provider), compiler);
    let proxy = CursorProxy::cursor(NetworkClients::new(store.clone()));
    let router = tab::router().layer(Extension(proxy)).with_state(registry);
    (router, store, directory)
}

fn completion_request(contents: &str, line: i32, column: i32) -> proto::StreamCppRequest {
    proto::StreamCppRequest {
        current_file: Some(proto::CurrentFileInfo {
            relative_workspace_path: "src/example.rs".into(),
            contents: contents.into(),
            cursor_position: Some(proto::CursorPosition { line, column }),
            language_id: "rust".into(),
            ..Default::default()
        }),
    }
}

async fn send(router: Router, path: &str, bytes: Vec<u8>) -> Response<Body> {
    router
        .oneshot(
            Request::post(path)
                .header("content-type", "application/proto")
                .header("content-length", bytes.len().to_string())
                .header("authorization", "Bearer official-account-secret")
                .header("cookie", "official-cookie-secret")
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn frames(response: Response<Body>) -> Vec<(u8, bytes::Bytes)> {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "application/connect+proto"
    );
    connect::decode_frames(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap()
}

fn text_provider(text: &str) -> TestProvider {
    TestProvider {
        events: Arc::new(Mutex::new(vec![
            Ok(ModelEvent::TextDelta(text.into())),
            Ok(ModelEvent::Done(FinishReason::Stop)),
        ])),
        ..Default::default()
    }
}

#[tokio::test]
async fn local_metadata_and_every_tab_path_never_call_a_provider() {
    let provider = TestProvider::default();
    let (router, _, _directory) = setup(provider.clone(), false).await;
    for path in tab::TAB_PATHS {
        let response = send(router.clone(), path, vec![]).await;
        assert!(
            response.status().is_success() || response.status().is_client_error(),
            "{path}"
        );
        let _ = to_bytes(response.into_body(), 65536).await.unwrap();
    }
    assert!(provider.requests.lock().unwrap().is_empty());
    let response = send(router.clone(), "/aiserver.v1.AiService/CppConfig", vec![]).await;
    let body = to_bytes(response.into_body(), 65536).await.unwrap();
    let config = proto::CppConfigResponse::decode(body).unwrap();
    assert_eq!(config.is_on, Some(false));
    assert!(!config.is_fused_cursor_prediction_model);
    assert!(!config.allows_tab_chunks);
    assert!(!config.cpp_url.contains("leokun"));
    let response = send(
        router,
        "/aiserver.v1.FileSyncService/FSIsEnabledForUser",
        vec![],
    )
    .await;
    let body = to_bytes(response.into_body(), 65536).await.unwrap();
    assert!(
        !proto::FsIsEnabledForUserResponse::decode(body)
            .unwrap()
            .enabled
    );
}

#[tokio::test]
async fn completion_is_a_short_stateless_call_without_official_credentials() {
    let provider = text_provider(" value");
    let (router, store, _directory) = setup(provider.clone(), true).await;
    let response = send(
        router,
        tab::TAB_PATHS[0],
        completion_request("let answer =;", 1, 13).encode_to_vec(),
    )
    .await;
    let frames = frames(response).await;
    assert_eq!(frames.last().unwrap().0, connect::END_STREAM_FLAG);
    assert_eq!(frames.last().unwrap().1.as_ref(), b"{}");
    let edits: Vec<proto::StreamCppResponse> = frames[..frames.len() - 1]
        .iter()
        .map(|(_, bytes)| proto::StreamCppResponse::decode(bytes.clone()).unwrap())
        .collect();
    assert_eq!(
        edits[0]
            .range_to_replace
            .as_ref()
            .unwrap()
            .start_line_number,
        1
    );
    assert_eq!(edits[0].begin_edit, Some(true));
    assert_eq!(edits[1].text, "let answer = value;");
    assert_eq!(edits.last().unwrap().done_stream, Some(true));
    let selected_model = store.tab_settings().await.unwrap().model_id;
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0].request;
    assert_eq!(request.model.model_id, selected_model);
    assert_eq!(request.model.max_output_tokens, Some(256));
    assert!(request.prompt.tools.is_empty());
    assert!(!request.model.reasoning.enabled);
    assert_eq!(request.history.len(), 1);
    let serialized = serde_json::to_string(request).unwrap();
    assert!(!serialized.contains("official-account-secret"));
    assert!(!serialized.contains("official-cookie-secret"));
}

#[tokio::test]
async fn empty_model_and_filesync_without_snapshot_fail_closed() {
    let provider = TestProvider::default();
    let (router, store, _directory) = setup(provider.clone(), false).await;
    let response = send(
        router.clone(),
        tab::TAB_PATHS[0],
        completion_request("", 1, 1).encode_to_vec(),
    )
    .await;
    assert!(
        String::from_utf8(frames(response).await.pop().unwrap().1.to_vec())
            .unwrap()
            .contains("select")
    );
    let model = store.models().await.unwrap().remove(0);
    store
        .set_tab_settings(TabSettings {
            mode: TabMode::Local,
            model_id: model.model_hash,
        })
        .await
        .unwrap();
    let mut request = completion_request("", 1, 1);
    request.current_file.as_mut().unwrap().rely_on_filesync = true;
    let response = send(router, tab::TAB_PATHS[0], request.encode_to_vec()).await;
    assert!(
        String::from_utf8(frames(response).await.pop().unwrap().1.to_vec())
            .unwrap()
            .contains("contents")
    );
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn filesync_does_not_acknowledge_a_snapshot_it_did_not_store() {
    let (router, _, _directory) = setup(TestProvider::default(), true).await;
    for path in [
        "/aiserver.v1.FileSyncService/FSUploadFile",
        "/aiserver.v1.FileSyncService/FSSyncFile",
    ] {
        let response = send(router.clone(), path, vec![]).await;
        let body = to_bytes(response.into_body(), 65536).await.unwrap();
        assert_eq!(proto::FileSyncResponse::decode(body).unwrap().error, 1);
    }
}

#[tokio::test]
async fn next_cursor_prediction_is_a_normal_empty_stream() {
    let (router, _, _directory) = setup(TestProvider::default(), true).await;
    let frames = frames(send(router, tab::TAB_PATHS[1], vec![]).await).await;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, connect::END_STREAM_FLAG);
    assert_eq!(frames[0].1.as_ref(), b"{}");
}

#[tokio::test]
async fn utf16_cursor_and_crlf_are_preserved() {
    let provider = text_provider("next\n  line");
    let (router, _, _directory) = setup(provider, true).await;
    let request = completion_request("first\r\n  🦀;\r\nlast", 2, 5);
    let frames = frames(send(router, tab::TAB_PATHS[0], request.encode_to_vec()).await).await;
    let edit = proto::StreamCppResponse::decode(frames[1].1.clone()).unwrap();
    assert_eq!(edit.text, "  🦀next\r\n  line;");
}

#[tokio::test]
async fn invalid_utf16_surrogate_position_never_calls_the_provider() {
    let provider = TestProvider::default();
    let (router, _, _directory) = setup(provider.clone(), true).await;
    let response = send(
        router,
        tab::TAB_PATHS[0],
        completion_request("🦀", 1, 2).encode_to_vec(),
    )
    .await;
    assert!(
        String::from_utf8(frames(response).await.pop().unwrap().1.to_vec())
            .unwrap()
            .contains("UTF-16")
    );
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn tool_calls_and_provider_errors_end_the_connect_stream_without_edits() {
    for event in [
        Ok(ModelEvent::ToolCallStart {
            index: 0,
            call_id: "tool".into(),
            name: "Read".into(),
        }),
        Err(Error::Provider("fake upstream failure".into())),
    ] {
        let provider = TestProvider {
            events: Arc::new(Mutex::new(vec![event])),
            ..Default::default()
        };
        let (router, _, _directory) = setup(provider, true).await;
        let frames = frames(
            send(
                router,
                tab::TAB_PATHS[0],
                completion_request("", 1, 1).encode_to_vec(),
            )
            .await,
        )
        .await;
        assert_eq!(frames.len(), 1);
        let error: serde_json::Value = serde_json::from_slice(&frames[0].1).unwrap();
        assert_eq!(error["error"]["code"], "unavailable");
    }
}

#[tokio::test]
async fn disconnected_body_cancels_provider_and_releases_the_concurrency_slot() {
    let provider = TestProvider {
        pending: true,
        ..Default::default()
    };
    let (router, _, _directory) = setup(provider.clone(), true).await;
    let response = send(
        router.clone(),
        tab::TAB_PATHS[0],
        completion_request("", 1, 1).encode_to_vec(),
    )
    .await;
    let mut body = response.into_body().into_data_stream();
    assert!(tokio::time::timeout(Duration::from_millis(30), body.next())
        .await
        .is_err());
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    let busy = frames(
        send(
            router.clone(),
            tab::TAB_PATHS[0],
            completion_request("", 1, 1).encode_to_vec(),
        )
        .await,
    )
    .await;
    assert!(String::from_utf8(busy.last().unwrap().1.to_vec())
        .unwrap()
        .contains("busy"));
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    let token = provider.tokens.lock().unwrap()[0].clone();
    drop(body);
    assert!(token.is_cancelled());
    let next = send(
        router,
        tab::TAB_PATHS[0],
        completion_request("", 1, 1).encode_to_vec(),
    )
    .await;
    assert_eq!(next.status(), StatusCode::OK);
}

#[tokio::test]
async fn shorter_connect_timeout_cancels_the_provider() {
    let provider = TestProvider {
        pending: true,
        ..Default::default()
    };
    let (router, _, _directory) = setup(provider.clone(), true).await;
    let response = router
        .oneshot(
            Request::post(tab::TAB_PATHS[0])
                .header("content-type", "application/proto")
                .header("connect-timeout-ms", "30")
                .body(Body::from(completion_request("", 1, 1).encode_to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    let frames = tokio::time::timeout(Duration::from_secs(1), frames(response))
        .await
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&frames.last().unwrap().1).unwrap();
    assert_eq!(error["error"]["code"], "unavailable");
    assert!(provider.tokens.lock().unwrap()[0].is_cancelled());
}

#[tokio::test]
async fn known_and_future_tab_methods_are_classified_without_claiming_agent_routes() {
    for path in [
        "/aiserver.v1.AiService/StreamHoldCpp",
        "/aiserver.v1.AiService/NewTabFeature",
        "/aiserver.v1.CppService/FutureMethod",
        "/aiserver.v1.FileSyncService/FutureMethod",
    ] {
        assert!(tab::is_tab_path(path));
    }
    assert!(!tab::is_tab_path("/agent.v1.AgentService/RunSSE"));
    assert!(!tab::is_tab_path("/aiserver.v1.AiService/AvailableModels"));
    let provider = TestProvider::default();
    let (router, _, _directory) = setup(provider.clone(), true).await;
    for path in [
        "/aiserver.v1.CppService/FutureMethod",
        "/aiserver.v1.FileSyncService/FutureMethod",
    ] {
        let response = send(router.clone(), path, vec![]).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 65536).await.unwrap();
        assert!(String::from_utf8(body.to_vec())
            .unwrap()
            .contains("unsupported local TAB"));
    }
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn input_bounds_and_compressed_connect_envelopes_fail_without_model_calls() {
    let provider = TestProvider::default();
    let (router, _, _directory) = setup(provider.clone(), true).await;
    let cases = [
        completion_request(&"x".repeat(300_000), 1, 1).encode_to_vec(),
        completion_request(&"x".repeat(9000), 1, 1).encode_to_vec(),
        completion_request("line", 2, 1).encode_to_vec(),
        completion_request("line", 1, 99).encode_to_vec(),
        completion_request("line", 0, 1).encode_to_vec(),
        completion_request("line", 1, 0).encode_to_vec(),
        vec![1, 0, 0, 0, 1, 0],
    ];
    for bytes in cases {
        let response = send(router.clone(), tab::TAB_PATHS[0], bytes).await;
        let frames = frames(response).await;
        assert_eq!(frames.len(), 1);
        let error: serde_json::Value = serde_json::from_slice(&frames[0].1).unwrap();
        assert_eq!(error["error"]["code"], "invalid_argument");
    }
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn context_is_cropped_to_the_selected_model_and_partial_snapshot_lines_are_kept() {
    let provider = text_provider("value");
    let (router, _, _directory) = setup(provider.clone(), true).await;
    let contents = format!("{}\n\n{}", "x".repeat(8000), "y".repeat(8000));
    let mut request = completion_request(&contents, 6, 1);
    request
        .current_file
        .as_mut()
        .unwrap()
        .contents_start_at_line = 5;
    let frames = frames(send(router, tab::TAB_PATHS[0], request.encode_to_vec()).await).await;
    let start = proto::StreamCppResponse::decode(frames[0].1.clone()).unwrap();
    assert_eq!(start.range_to_replace.unwrap().start_line_number, 6);
    let invocations = provider.requests.lock().unwrap();
    let request = &invocations[0].request;
    let serialized = serde_json::to_string(&request.history).unwrap();
    assert!(serialized.len() < 2048);
    assert_eq!(request.model.context_window_tokens, Some(2048));
}

#[tokio::test]
async fn output_bounds_truncated_and_unfinished_streams_return_errors_before_any_edit() {
    for events in [
        vec![Ok(ModelEvent::TextDelta("x".repeat(4097)))],
        vec![
            Ok(ModelEvent::TextDelta("partial".into())),
            Ok(ModelEvent::Done(FinishReason::Length)),
        ],
        vec![Ok(ModelEvent::TextDelta("partial".into()))],
        vec![
            Ok(ModelEvent::TextDelta("line\n".repeat(17))),
            Ok(ModelEvent::Done(FinishReason::Stop)),
        ],
        vec![
            Ok(ModelEvent::TextDelta("line\r".repeat(17))),
            Ok(ModelEvent::Done(FinishReason::Stop)),
        ],
    ] {
        let provider = TestProvider {
            events: Arc::new(Mutex::new(events)),
            ..Default::default()
        };
        let (router, _, _directory) = setup(provider, true).await;
        let frames = frames(
            send(
                router,
                tab::TAB_PATHS[0],
                completion_request("", 1, 1).encode_to_vec(),
            )
            .await,
        )
        .await;
        assert_eq!(frames.len(), 1);
        let error: serde_json::Value = serde_json::from_slice(&frames[0].1).unwrap();
        assert_eq!(error["error"]["code"], "unavailable");
    }
}

#[tokio::test]
async fn thinking_is_not_exposed_as_code_and_saved_reasoning_does_not_disable_tab() {
    let provider = TestProvider {
        events: Arc::new(Mutex::new(vec![
            Ok(ModelEvent::ThinkingStart),
            Ok(ModelEvent::ThinkingDelta("private thought".into())),
            Ok(ModelEvent::ThinkingEnd),
            Ok(ModelEvent::TextDelta("value".into())),
            Ok(ModelEvent::Done(FinishReason::Stop)),
        ])),
        ..Default::default()
    };
    let (router, store, _directory) = setup(provider, true).await;
    let model = store.models().await.unwrap().remove(0);
    let mut input: ModelConfigInput =
        serde_json::from_value(serde_json::to_value(&model).unwrap()).unwrap();
    input.reasoning_effort = Some("high".into());
    let model = store.update_model(&model.model_hash, &input).await.unwrap();
    store
        .set_tab_settings(TabSettings {
            mode: TabMode::Local,
            model_id: model.model_hash,
        })
        .await
        .unwrap();
    let response = send(router.clone(), "/aiserver.v1.AiService/CppConfig", vec![]).await;
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    assert_eq!(
        proto::CppConfigResponse::decode(bytes).unwrap().is_on,
        Some(true)
    );
    let frames = frames(
        send(
            router,
            tab::TAB_PATHS[0],
            completion_request("", 1, 1).encode_to_vec(),
        )
        .await,
    )
    .await;
    let edit = proto::StreamCppResponse::decode(frames[1].1.clone()).unwrap();
    assert_eq!(edit.text, "value");
    assert_eq!(
        store.models().await.unwrap()[0].reasoning_effort.as_deref(),
        Some("high")
    );
}

#[tokio::test]
async fn fenced_completion_is_cleaned_without_duplicating_the_existing_suffix() {
    let provider = text_provider("```rust\nvalue;\n```");
    let (router, _, _directory) = setup(provider, true).await;
    let frames = frames(
        send(
            router,
            tab::TAB_PATHS[0],
            completion_request(";", 1, 1).encode_to_vec(),
        )
        .await,
    )
    .await;
    assert_eq!(
        proto::StreamCppResponse::decode(frames[1].1.clone())
            .unwrap()
            .text,
        "value;"
    );
}

#[tokio::test]
async fn configured_metadata_and_unpolled_completion_bodies_do_not_start_generation() {
    let provider = TestProvider::default();
    let (router, _, _directory) = setup(provider.clone(), true).await;
    for path in tab::TAB_PATHS {
        let response = send(router.clone(), path, vec![]).await;
        let _ = to_bytes(response.into_body(), 65536).await.unwrap();
    }
    let response = send(
        router.clone(),
        tab::TAB_PATHS[0],
        completion_request("", 1, 1).encode_to_vec(),
    )
    .await;
    drop(response);
    assert!(provider.requests.lock().unwrap().is_empty());
    let response = send(router, "/aiserver.v1.AiService/CppConfig", vec![]).await;
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    assert_eq!(
        proto::CppConfigResponse::decode(bytes).unwrap().is_on,
        Some(true)
    );
}

#[tokio::test]
async fn identity_connect_request_with_explicit_contents_can_ignore_file_sync() {
    let provider = text_provider("code");
    let (router, _, _directory) = setup(provider.clone(), true).await;
    let mut request = completion_request(";", 1, 1);
    request.current_file.as_mut().unwrap().rely_on_filesync = true;
    let envelope = connect::encode_message(&request).unwrap();
    let response = router
        .oneshot(
            Request::post(tab::TAB_PATHS[0])
                .header("content-type", "application/connect+proto")
                .header("connect-content-encoding", "identity")
                .body(Body::from(envelope))
                .unwrap(),
        )
        .await
        .unwrap();
    let frames = frames(response).await;
    let edit = proto::StreamCppResponse::decode(frames[1].1.clone()).unwrap();
    assert_eq!(edit.text, "code;");
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn empty_generation_finishes_normally_without_an_edit() {
    let provider = text_provider("");
    let (router, _, _directory) = setup(provider, true).await;
    let frames = frames(
        send(
            router,
            tab::TAB_PATHS[0],
            completion_request("", 1, 1).encode_to_vec(),
        )
        .await,
    )
    .await;
    assert_eq!(frames.len(), 2);
    let response = proto::StreamCppResponse::decode(frames[0].1.clone()).unwrap();
    assert_eq!(response.done_stream, Some(true));
    assert_eq!(response.begin_edit, None);
    assert_eq!(response.range_to_replace, None);
    assert_eq!(frames[1].1.as_ref(), b"{}");
}

#[tokio::test]
async fn expired_deadline_does_not_start_a_provider_request() {
    let provider = TestProvider::default();
    let (router, _, _directory) = setup(provider.clone(), true).await;
    let response = router
        .oneshot(
            Request::post(tab::TAB_PATHS[0])
                .header("content-type", "application/proto")
                .header("connect-timeout-ms", "0")
                .body(Body::from(completion_request("", 1, 1).encode_to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    let frames = frames(response).await;
    assert_eq!(frames.len(), 1);
    let error: serde_json::Value = serde_json::from_slice(&frames[0].1).unwrap();
    assert_eq!(error["error"]["code"], "unavailable");
    assert!(provider.requests.lock().unwrap().is_empty());
}
