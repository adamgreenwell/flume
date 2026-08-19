//! Integration tests for the torrent engine wrapper.
//!
//! These exercise a real [`librqbit::Session`] with no Tauri runtime involved,
//! which is the whole point of keeping the engine layer free of Tauri types.
//!
//! Tests that need the public internet are marked `#[ignore]` so that CI stays
//! deterministic. Run them explicitly with:
//!
//! ```text
//! cargo test -- --ignored
//! ```

// In tests, `expect` is the correct tool: a failed expectation *is* the test
// failure, and the message is the diagnostic. The crate-wide `expect_used` lint
// exists to protect production paths, not this file.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use flume_lib::engine::{Engine, EngineConfig, EngineHealth};
use tempfile::TempDir;

/// Builds a config rooted in a temporary directory, so tests never touch the
/// developer's real Downloads folder or session state.
///
/// Port 0 asks the OS for an ephemeral port, which keeps concurrent test runs
/// from fighting over 42221.
fn test_config(tmp: &TempDir, enable_dht: bool) -> EngineConfig {
    EngineConfig {
        download_dir: tmp.path().join("downloads"),
        session_dir: tmp.path().join("session"),
        listen_port: 0,
        enable_dht,
        enable_upnp: false,
    }
}

#[tokio::test]
async fn starts_without_dht_and_reports_status() {
    let tmp = TempDir::new().expect("temp dir");
    let config = test_config(&tmp, false);

    let engine = Engine::start(config.clone())
        .await
        .expect("engine should start");
    let status = engine.core_status();

    assert!(
        status.listen_port.is_some(),
        "engine should bind a listen port so it can accept peers and seed"
    );
    assert_eq!(
        status.download_dir,
        config.download_dir.display().to_string()
    );
    assert!(!status.dht.enabled, "DHT was disabled in config");
    assert_eq!(
        status.health,
        EngineHealth::Degraded,
        "no DHT means magnet links cannot resolve, which is a degraded state"
    );
    assert!(status.client_version.starts_with("Flume "));

    engine.shutdown().await;
}

#[tokio::test]
async fn creates_missing_directories() {
    let tmp = TempDir::new().expect("temp dir");
    let config = test_config(&tmp, false);
    assert!(!config.download_dir.exists());

    let engine = Engine::start(config.clone())
        .await
        .expect("engine should start");

    assert!(
        config.download_dir.is_dir(),
        "download dir should be created"
    );
    assert!(config.session_dir.is_dir(), "session dir should be created");

    engine.shutdown().await;
}

#[tokio::test]
async fn two_engines_can_coexist_on_ephemeral_ports() {
    let a_tmp = TempDir::new().expect("temp dir");
    let b_tmp = TempDir::new().expect("temp dir");

    let a = Engine::start(test_config(&a_tmp, false)).await.expect("a");
    let b = Engine::start(test_config(&b_tmp, false)).await.expect("b");

    assert_ne!(
        a.core_status().listen_port,
        b.core_status().listen_port,
        "ephemeral ports must differ, otherwise the second bind silently reused the first"
    );

    a.shutdown().await;
    b.shutdown().await;
}

/// Requires internet access: the DHT must reach its bootstrap nodes.
#[tokio::test]
#[ignore = "requires network access to DHT bootstrap nodes"]
async fn dht_bootstraps_and_reaches_ready() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, true))
        .await
        .expect("engine should start");

    // Bootstrapping normally takes a couple of seconds; poll rather than
    // sleeping for a fixed worst case.
    let mut health = EngineHealth::Starting;
    for _ in 0..30 {
        health = engine.core_status().health;
        if health == EngineHealth::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let status = engine.core_status();
    assert_eq!(
        health,
        EngineHealth::Ready,
        "DHT did not bootstrap within 30s (routing table had {} nodes)",
        status.dht.total_nodes()
    );
    assert!(status.dht.enabled);
    assert!(status.dht.total_nodes() > 0);

    engine.shutdown().await;
}
