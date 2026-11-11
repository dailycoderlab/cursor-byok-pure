//! Checks the pure-mode settings and proxy boundaries with isolated stores and loopback upstreams.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use cursor_server::{
    api::cursor::{self, proxy::UPSTREAM_URL_HEADER},
    control::{self, ControlService},
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::connect,
        transport::TransportRegistry,
    },
    network::NetworkClients,
    plugin::{PluginRegistry, PluginRuntime},
    store::{CommitSettings, Store, TabMode, TabSettings},
};
use serde_json::{json, Value};
use tower::ServiceExt;

struct Upstream {
    url: String,
    calls: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Upstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn loopback_upstream(marker: &'static str) -> Upstream {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let router = Router::new().fallback(move || {
        let count = count.clone();
        async move {
            count.fetch_add(1, Ordering::SeqCst);
            marker
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    Upstream { url, calls, task }
}

fn cursor_router(store: &Store, provider: fake_provider::FakeProvider) -> Router {
    let compiler = PromptCompiler::new(PromptAssets::embedded().unwrap());
    let registry = TransportRegistry::new(store.clone(), Arc::new(provider), compiler);
    cursor::router(registry, NetworkClients::new(store.clone())).unwrap()
}

fn control_router(store: &Store, provider: fake_provider::FakeProvider) -> Router {
    let runtime = PluginRuntime::managed().unwrap();
    let plugins = PluginRegistry::managed(store.clone(), runtime.clone(), "0.1.0".into()).unwrap();
    let service = ControlService::new(
        store.clone(),
        Arc::new(provider),
        runtime,
        plugins,
        NetworkClients::new(store.clone()),
    )
    .unwrap();
    control::api_router(service)
}

#[tokio::test]
async fn fresh_tab_settings_default_to_official_direct_without_a_selected_model() {
    let (_directory, store) = fixtures::temp_store().await;
    let settings = store.tab_settings().await.unwrap();
    assert_eq!(settings.mode, TabMode::Direct);
    assert!(settings.model_id.is_empty());
    assert_eq!(serde_json::to_value(settings).unwrap()["mode"], "direct");
    for retired in ["public", "custom"] {
        assert!(serde_json::from_value::<TabSettings>(json!({"mode": retired})).is_err());
    }
}

#[tokio::test]
async fn new_tab_namespace_does_not_read_obsolete_settings_or_reset_other_settings() {
    let (directory, store) = fixtures::temp_store().await;
    sqlx::query(
        "INSERT INTO service_settings(setting_key, value_json, updated_at_ms) VALUES (?, ?, 0)",
    )
    .bind("cursor_tab")
    .bind(r#"{"mode":"public","address":"https://retired.invalid"}"#)
    .execute(store.pool())
    .await
    .unwrap();
    let commit = CommitSettings {
        prompt: "keep this prompt".into(),
        ..CommitSettings::default()
    };
    store.set_commit_settings(commit.clone()).await.unwrap();
    assert_eq!(store.tab_settings().await.unwrap(), TabSettings::default());
    let saved = store
        .set_tab_settings(TabSettings {
            mode: TabMode::Local,
            model_id: " explicitly-selected-model ".into(),
        })
        .await
        .unwrap();
    assert_eq!(saved.model_id, "explicitly-selected-model");
    let url = format!("sqlite://{}", directory.path().join("test.db").display());
    let reopened = Store::connect(&url).await.unwrap();
    assert_eq!(reopened.tab_settings().await.unwrap(), saved);
    assert_eq!(reopened.commit_settings().await.unwrap(), commit);
}

#[tokio::test]
async fn advertising_routes_are_absent_and_do_not_create_an_installation_identifier() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let router = control_router(&store, provider.clone());
    for (method, path) in [
        ("GET", "/__byok-api__/api/promotions"),
        ("GET", "/__byok-api__/api/promotions/images/missing.png"),
        ("POST", "/__byok-api__/api/promotions/example/dismissals"),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM service_settings WHERE setting_key = 'installation_id'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    assert!(provider.requests().is_empty());
}

#[tokio::test]
async fn tab_management_exposes_disabled_default_and_rejects_an_unknown_model() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let router = control_router(&store, provider.clone());
    let response = router
        .clone()
        .oneshot(
            Request::get("/__byok-api__/api/settings/tab")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["mode"], "direct");
    assert_eq!(value["model_id"], "");
    for (model_id, should_succeed) in [("missing-model", false), ("", true)] {
        let response = router
            .clone()
            .oneshot(
                Request::put("/__byok-api__/api/settings/tab")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"mode":"local", "model_id":model_id}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().is_success(), should_succeed);
    }
    assert!(provider.requests().is_empty());
}

#[tokio::test]
async fn future_tab_methods_and_wrong_http_methods_cannot_escape_the_local_proxy_boundary() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    store
        .set_tab_settings(TabSettings {
            mode: TabMode::Local,
            ..TabSettings::default()
        })
        .await
        .unwrap();
    let router = cursor_router(&store, provider.clone());
    let upstream = loopback_upstream("must not be contacted").await;
    for (method, path) in [
        ("POST", "/aiserver.v1.AiService/StreamHoldCpp"),
        ("POST", "/aiserver.v1.AiService/FutureTabMethod"),
        ("GET", "/aiserver.v1.AiService/StreamCpp"),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/connect+proto")
                    .header(UPSTREAM_URL_HEADER, &upstream.url)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let frames = connect::decode_frames(&bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, connect::END_STREAM_FLAG);
        let error: Value = serde_json::from_slice(&frames[0].1).unwrap();
        assert_eq!(error["error"]["code"], "unimplemented");
    }
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 0);
    assert!(provider.requests().is_empty());
}

#[tokio::test]
async fn classifier_does_not_suppress_every_configured_local_suggestion() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let router = cursor_router(&store, provider.clone());
    let input = serde_json::from_value(json!({
        "display_name": "Offline TAB fixture",
        "type": "openai",
        "base_url": "https://not-contacted.invalid/v1",
        "api_key": "offline-test-only",
        "tooltip_data": "no model requests in this test",
        "model_id": "completion-fixture",
        "openai_endpoint": cursor_server::model::OPENAI_CHAT_ENDPOINT,
    }))
    .unwrap();
    let model = store.create_model(&input).await.unwrap();
    for (model_id, should_noop) in [(String::new(), true), (model.model_hash, false)] {
        store
            .set_tab_settings(TabSettings {
                mode: TabMode::Local,
                model_id,
            })
            .await
            .unwrap();
        let response = router
            .clone()
            .oneshot(
                Request::post("/aiserver.v1.AiService/GetCppEditClassification")
                    .header(header::CONTENT_TYPE, "application/proto")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let reply: cursor_server::cursor::services::tab::proto::GetCppEditClassificationResponse =
            prost::Message::decode(&bytes[..]).unwrap();
        assert_eq!(reply.should_noop, Some(should_noop));
    }
    assert!(provider.requests().is_empty());
}
