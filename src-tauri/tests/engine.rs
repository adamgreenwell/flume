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

use flume_lib::engine::{Engine, EngineConfig, EngineHealth, TorrentSource};
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
        proxy_url: None,
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

// --- Add flow ------------------------------------------------------------

/// Writes a minimal single-file `.torrent` and returns its path.
///
/// Built with librqbit's own encoder rather than a checked-in fixture, so the
/// test stays honest if metadata handling changes. Returns a *path* because
/// that is what the app actually receives from the file picker.
async fn sample_torrent_file(dir: &std::path::Path) -> String {
    std::fs::create_dir_all(dir).expect("dir");
    std::fs::write(dir.join("ubuntu.iso"), vec![7u8; 4096]).expect("write");

    let result = librqbit::create_torrent(
        dir,
        librqbit::CreateTorrentOptions {
            name: Some("flume-test"),
            piece_length: Some(1024),
            ..Default::default()
        },
        &librqbit::spawn_utils::BlockingSpawner::new(1),
    )
    .await
    .expect("create torrent");

    let path = dir.join("sample.torrent");
    std::fs::write(&path, result.as_bytes().expect("encode torrent")).expect("write torrent");
    path.display().to_string()
}

#[tokio::test]
async fn preview_lists_files_without_starting_a_download() {
    let tmp = TempDir::new().expect("temp dir");
    let path = sample_torrent_file(&tmp.path().join("src")).await;

    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");

    let preview = engine
        .preview(TorrentSource::File { path: path.clone() })
        .await
        .expect("preview succeeds");

    assert_eq!(preview.files.len(), 1);
    assert!(preview.files[0].path.contains("ubuntu.iso"));
    assert_eq!(preview.total_bytes, 4096);
    assert!(!preview.already_added);
    assert!(
        engine.torrent_summaries().is_empty(),
        "preview must not add the torrent to the session"
    );

    engine.shutdown().await;
}

#[tokio::test]
async fn confirm_add_starts_the_torrent_and_consumes_the_preview() {
    let tmp = TempDir::new().expect("temp dir");
    let path = sample_torrent_file(&tmp.path().join("src")).await;

    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");
    let preview = engine
        .preview(TorrentSource::File { path })
        .await
        .expect("preview");

    let id = engine
        .confirm_add(&preview.info_hash, None)
        .await
        .expect("add succeeds");

    let summaries = engine.torrent_summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, id);
    assert_eq!(summaries[0].info_hash, preview.info_hash);

    // The stashed metadata is single-use, so an abandoned dialog cannot leak it.
    assert!(matches!(
        engine.confirm_add(&preview.info_hash, None).await,
        Err(flume_lib::engine::EngineError::NoPendingPreview)
    ));

    engine.shutdown().await;
}

#[tokio::test]
async fn discarding_a_preview_releases_it() {
    let tmp = TempDir::new().expect("temp dir");
    let path = sample_torrent_file(&tmp.path().join("src")).await;

    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");
    let preview = engine
        .preview(TorrentSource::File { path })
        .await
        .expect("preview");

    engine.discard_preview(&preview.info_hash).await;

    assert!(matches!(
        engine.confirm_add(&preview.info_hash, None).await,
        Err(flume_lib::engine::EngineError::NoPendingPreview)
    ));

    engine.shutdown().await;
}

#[tokio::test]
async fn invalid_magnet_is_rejected_before_any_network_work() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");

    let err = engine
        .preview(TorrentSource::Magnet {
            uri: "magnet:?xt=urn:btih:not-a-real-hash".into(),
        })
        .await
        .expect_err("should reject");

    assert!(
        matches!(err, flume_lib::engine::EngineError::InvalidMagnet(_)),
        "expected InvalidMagnet, got {err:?}"
    );

    engine.shutdown().await;
}

#[tokio::test]
async fn control_operations_reject_unknown_ids() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");

    for result in [
        engine.pause(999).await,
        engine.resume(999).await,
        engine.remove(999, false).await,
        engine.set_only_files(999, vec![0]).await,
    ] {
        assert!(
            matches!(
                result,
                Err(flume_lib::engine::EngineError::UnknownTorrent(999))
            ),
            "expected UnknownTorrent, got {result:?}"
        );
    }

    engine.shutdown().await;
}

#[tokio::test]
async fn remove_without_delete_leaves_files_on_disk() {
    let tmp = TempDir::new().expect("temp dir");
    let path = sample_torrent_file(&tmp.path().join("src")).await;

    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");
    let preview = engine
        .preview(TorrentSource::File { path })
        .await
        .expect("preview");
    let id = engine
        .confirm_add(&preview.info_hash, None)
        .await
        .expect("add");

    engine.remove(id, false).await.expect("remove");

    assert!(
        engine.torrent_summaries().is_empty(),
        "torrent should be gone from the session"
    );

    engine.shutdown().await;
}

/// Ubuntu 24.04.3 desktop amd64 — a large, extremely well-seeded torrent.
///
/// Chosen because it is legal to distribute, reliably available on the DHT,
/// and representative of Flume's actual use case.
const UBUNTU_MAGNET: &str = "magnet:?xt=urn:btih:d160b8d8ea35a5b4e52837468fc8f03d55cef1f7";

/// Requires internet access: resolves real metadata over the DHT.
///
/// This is the highest-risk path in the add flow — a magnet has no metadata,
/// so the file list has to come from peers found via the DHT. Offline tests
/// cannot cover it.
#[tokio::test]
#[ignore = "requires network access and DHT peers"]
async fn magnet_resolves_real_metadata_over_the_dht() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, true))
        .await
        .expect("engine starts");

    let preview = tokio::time::timeout(
        Duration::from_secs(120),
        engine.preview(TorrentSource::Magnet {
            uri: UBUNTU_MAGNET.to_string(),
        }),
    )
    .await
    .expect("metadata resolution timed out")
    .expect("preview should succeed");

    assert!(
        preview.name.to_lowercase().contains("ubuntu"),
        "unexpected torrent name: {}",
        preview.name
    );
    assert!(!preview.files.is_empty(), "torrent should list files");
    assert!(
        preview.total_bytes > 1_000_000_000,
        "a desktop ISO should be over 1 GB, got {}",
        preview.total_bytes
    );
    assert!(!preview.already_added);

    // Confirming must not re-fetch metadata: the engine kept the resolved
    // bytes, so this returns effectively instantly.
    let id = tokio::time::timeout(
        Duration::from_secs(10),
        engine.confirm_add(&preview.info_hash, Some(vec![0])),
    )
    .await
    .expect("confirm should not re-fetch over the DHT")
    .expect("add should succeed");

    let summaries = engine.torrent_summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, id);

    engine.shutdown().await;
}

#[tokio::test]
async fn torrents_survive_a_restart() {
    let tmp = TempDir::new().expect("temp dir");
    let path = sample_torrent_file(&tmp.path().join("src")).await;
    // A fixed config so both runs share a session directory, which is what
    // makes persistence meaningful.
    let config = test_config(&tmp, false);

    let info_hash = {
        let engine = Engine::start(config.clone()).await.expect("first start");
        let preview = engine
            .preview(TorrentSource::File { path })
            .await
            .expect("preview");
        engine
            .confirm_add(&preview.info_hash, None)
            .await
            .expect("add");

        assert_eq!(engine.torrent_summaries().len(), 1);
        // Shutdown is what flushes session state; killing the process without
        // it is the case that legitimately loses the list.
        engine.shutdown().await;
        preview.info_hash
    };

    let engine = Engine::start(config).await.expect("second start");

    // Restoring reads the persisted list and re-initializes each torrent, so
    // poll briefly rather than assuming it is synchronous.
    let mut summaries = engine.torrent_summaries();
    for _ in 0..20 {
        if !summaries.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        summaries = engine.torrent_summaries();
    }

    assert_eq!(
        summaries.len(),
        1,
        "the torrent should be restored from session state"
    );
    assert_eq!(summaries[0].info_hash, info_hash);

    engine.shutdown().await;
}

#[tokio::test]
async fn file_selection_survives_a_restart() {
    let tmp = TempDir::new().expect("temp dir");
    let path = sample_torrent_file(&tmp.path().join("src")).await;
    let config = test_config(&tmp, false);

    {
        let engine = Engine::start(config.clone()).await.expect("first start");
        let preview = engine
            .preview(TorrentSource::File { path })
            .await
            .expect("preview");
        let id = engine
            .confirm_add(&preview.info_hash, Some(vec![0]))
            .await
            .expect("add");

        let files = engine.torrent_files(id).expect("files");
        assert!(files[0].selected, "the chosen file should be selected");
        engine.shutdown().await;
    }

    let engine = Engine::start(config).await.expect("second start");

    let mut summaries = engine.torrent_summaries();
    for _ in 0..20 {
        if !summaries.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        summaries = engine.torrent_summaries();
    }
    assert_eq!(summaries.len(), 1, "torrent should be restored");

    let files = engine
        .torrent_files(summaries[0].id)
        .expect("files after restart");
    assert!(
        files[0].selected,
        "the file selection must survive a restart, not silently reset to all"
    );

    engine.shutdown().await;
}
