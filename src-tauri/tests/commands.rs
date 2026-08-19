//! Integration tests for the Tauri command layer.
//!
//! These drive commands through Tauri's real IPC machinery using its mock
//! runtime, so the `#[tauri::command]` registration, argument extraction,
//! state injection, and `serde` round trip are all exercised — the parts that
//! a plain unit test on the engine cannot reach.
//!
//! What this proves that `tests/engine.rs` does not: that `invoke("…")` from
//! the webview actually reaches our handler and returns the JSON shape the
//! TypeScript mirrors in `src/lib/ipc/types.ts` expect.

// `expect` is the right tool in tests: a failed expectation is the diagnostic.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use flume_lib::{
    commands,
    engine::{Engine, EngineConfig},
    state::AppState,
};
use tauri::{
    WebviewWindow, WebviewWindowBuilder,
    ipc::{CallbackFn, InvokeBody},
    test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, noop_assets},
    webview::InvokeRequest,
};
use tempfile::TempDir;

/// Builds a mock Tauri app with our real state and command handlers attached.
fn mock_app_with_state(state: AppState) -> tauri::App<MockRuntime> {
    mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![commands::get_core_status])
        .build(tauri::generate_context!(
            "tauri.conf.json",
            assets = noop_assets()
        ))
        .expect("failed to build mock app")
}

/// Creates the `main` webview the commands are invoked from.
///
/// The mock runtime does not build windows declared in `tauri.conf.json`, so
/// the test creates one explicitly.
fn main_webview(app: &tauri::App<MockRuntime>) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(app, "main", Default::default())
        .build()
        .expect("failed to build the main webview")
}

/// Issues `get_core_status` over the IPC bridge exactly as the webview would.
fn invoke_get_core_status<W: AsRef<tauri::Webview<MockRuntime>>>(
    webview: &W,
) -> Result<serde_json::Value, serde_json::Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: "get_core_status".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize().expect("response should be JSON"))
}

#[test]
fn returns_engine_not_ready_before_the_engine_starts() {
    let app = mock_app_with_state(AppState::new());
    let webview = main_webview(&app);

    let err = invoke_get_core_status(&webview).expect_err("should reject while starting");

    assert_eq!(
        err.get("kind").and_then(|v| v.as_str()),
        Some("engineNotReady"),
        "frontend branches on this stable identifier, not on message text"
    );
    assert!(
        err.get("message").and_then(|v| v.as_str()).is_some(),
        "a human-readable message must always accompany the kind"
    );
}

#[test]
fn returns_camel_case_status_once_the_engine_is_running() {
    let tmp = TempDir::new().expect("temp dir");
    let config = EngineConfig {
        download_dir: tmp.path().join("downloads"),
        session_dir: tmp.path().join("session"),
        listen_port: 0,
        enable_dht: false,
        enable_upnp: false,
    };

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let state = AppState::new();
    let engine = runtime
        .block_on(Engine::start(config))
        .expect("engine starts");
    runtime.block_on(state.set_engine(engine));

    let app = mock_app_with_state(state);
    let webview = main_webview(&app);

    let status = invoke_get_core_status(&webview).expect("should resolve once running");

    // Guard the IPC contract: these exact camelCase keys are what
    // src/lib/ipc/types.ts declares. A serde rename regression breaks the UI
    // silently, so assert the shape rather than just the values.
    for key in [
        "clientVersion",
        "listenPort",
        "announcePort",
        "dht",
        "downloadDir",
        "uptimeSeconds",
        "downloadBps",
        "uploadBps",
        "livePeers",
        "health",
    ] {
        assert!(status.get(key).is_some(), "missing IPC field `{key}`");
    }

    let dht = status.get("dht").expect("dht object");
    for key in ["enabled", "nodesV4", "nodesV6", "outstandingRequests"] {
        assert!(dht.get(key).is_some(), "missing DhtStatus field `{key}`");
    }

    assert_eq!(dht.get("enabled").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(
        status.get("health").and_then(|v| v.as_str()),
        Some("degraded"),
        "DHT disabled must serialize as the camelCase EngineHealth variant"
    );
    assert!(
        status
            .get("clientVersion")
            .and_then(|v| v.as_str())
            .is_some_and(|v| v.starts_with("Flume ")),
    );
}
