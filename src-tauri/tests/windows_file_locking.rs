//! Reproduction for the reported Windows file-locking problem (#9).
//!
//! # The claim
//!
//! On Windows, another process holding an open handle to a downloaded file can
//! block Flume from seeding it. A community client ("Drift") patched
//! librqbit's storage layer for this. The claim predates librqbit v9 and has
//! **not** been confirmed against it.
//!
//! # Why this file exists
//!
//! Flume is developed on macOS, where POSIX semantics make the scenario
//! impossible to reproduce: an open handle does not prevent another process
//! opening the same file. Rather than write a speculative patch for a bug that
//! cannot be observed, the question is encoded as a test that answers itself
//! the moment it runs on Windows.
//!
//! The whole file is `#[cfg(windows)]`, so it costs nothing elsewhere.
//!
//! # What it actually tests
//!
//! `share_mode(0)` opens a file with **no** sharing — the strictest case, and
//! what an application that has not opted into sharing gets. If librqbit can
//! still initialize and verify a completed torrent whose file is held that
//! way, Flume can seed it and no patch is needed.

#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{os::windows::fs::OpenOptionsExt, time::Duration};

use flume_lib::engine::{Engine, EngineConfig, TorrentSource};
use tempfile::TempDir;

/// Opens a file with no sharing permitted at all.
///
/// This is the worst case: `FILE_SHARE_*` flags all cleared, so any other
/// process attempting to open the file gets a sharing violation.
const NO_SHARING: u32 = 0;

fn test_config(tmp: &TempDir) -> EngineConfig {
    EngineConfig {
        download_dir: tmp.path().join("downloads"),
        session_dir: tmp.path().join("session"),
        listen_port: 0,
        enable_dht: false,
        enable_upnp: false,
        proxy_url: None,
    }
}

/// Builds a torrent whose data already exists in the download directory, so
/// the session has something complete to verify and seed.
async fn seeded_torrent(download_dir: &std::path::Path) -> (String, std::path::PathBuf) {
    std::fs::create_dir_all(download_dir).expect("download dir");
    let payload = download_dir.join("ubuntu.iso");
    std::fs::write(&payload, vec![9u8; 8192]).expect("write payload");

    let result = librqbit::create_torrent(
        download_dir,
        librqbit::CreateTorrentOptions {
            name: Some("flume-lock-test"),
            piece_length: Some(1024),
            ..Default::default()
        },
        &librqbit::spawn_utils::BlockingSpawner::new(1),
    )
    .await
    .expect("create torrent");

    let torrent_path = download_dir.join("sample.torrent");
    std::fs::write(&torrent_path, result.as_bytes().expect("encode")).expect("write torrent");
    (torrent_path.display().to_string(), payload)
}

#[tokio::test]
async fn can_seed_a_file_another_process_holds_open() {
    let tmp = TempDir::new().expect("temp dir");
    let config = test_config(&tmp);
    let (torrent_path, payload) = seeded_torrent(&config.download_dir).await;

    // Hold the completed file with no sharing, standing in for another
    // application that has the download open.
    let _held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(NO_SHARING)
        .open(&payload)
        .expect("should be able to take an exclusive handle");

    let engine = Engine::start(config).await.expect("engine starts");

    let preview = engine
        .preview(TorrentSource::File { path: torrent_path })
        .await
        .expect("preview should succeed even with the file held");

    let id = engine
        .confirm_add(&preview.info_hash, None)
        .await
        .expect("adding should succeed even with the file held");

    // Initialization hashes existing data; that is the step that must open the
    // held file. Poll rather than assuming it completes synchronously.
    let mut summary = None;
    for _ in 0..50 {
        let summaries = engine.torrent_summaries();
        if let Some(s) = summaries.into_iter().find(|s| s.id == id) {
            if s.finished || s.error.is_some() {
                summary = Some(s);
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let summary = summary.expect("torrent should reach a terminal state within 5s");

    assert!(
        summary.error.is_none(),
        "librqbit could not use a file held open by another process, which is \
         the reported Windows behaviour in issue #9. Error: {:?}",
        summary.error
    );
    assert!(
        summary.finished,
        "the torrent should verify as complete from existing data; state was {:?}",
        summary.state
    );

    engine.shutdown().await;
}
