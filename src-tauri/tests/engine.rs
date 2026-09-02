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

use std::time::{Duration, Instant};

use flume_lib::engine::{
    Engine, EngineConfig, EngineError, EngineHealth, TorrentSource, TorrentState,
};
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

/// A persisted torrent whose `.torrent` file is gone must not hang the session.
///
/// The bug in #154, reproduced end to end. librqbit reads a missing
/// `<hash>.torrent` sidecar as *empty bytes* rather than as an error, and
/// `into_add_torrent` branches on the byte length -- so the row is restored as
/// a **magnet**. Magnet resolution on that path has no timeout inside
/// librqbit, and the restore loop is `while !added_all || !futs.is_empty()`,
/// so one unresolvable row holds `Session::new_with_opts` open forever. That
/// hangs `Engine::start`, which hangs `AppState::restart_engine`, which hangs
/// the caller in `crate::guard` -- so the guard loop is never spawned and the
/// egress status the UI shows never updates again.
///
/// The info hash below is random, so nobody is seeding it and the DHT will
/// keep offering fresh peers to ask indefinitely. That is what makes this a
/// hang rather than a slow failure, and it is why the test needs real network:
/// with no DHT the peer stream completes and the add fails fast, which is the
/// wrong code path.
#[tokio::test]
#[ignore = "needs working internet: without a live DHT the add fails fast instead of hanging"]
async fn a_torrent_whose_dot_torrent_file_is_missing_cannot_hang_the_session() {
    let tmp = TempDir::new().expect("temp dir");
    let config = test_config(&tmp, true);
    std::fs::create_dir_all(&config.session_dir).expect("session dir");
    std::fs::create_dir_all(&config.download_dir).expect("download dir");

    // One row, and deliberately no `<hash>.torrent` beside it -- which is what
    // a disk-full moment or a kill mid-write leaves behind, since librqbit
    // writes that sidecar best-effort and commits the row regardless.
    let session_json = serde_json::json!({
        "torrents": {
            "0": {
                // Random, so nobody is seeding it.
                "info_hash": "b4c9a1f70e2d83a6157c0d4e9b2f8a1d3e6f7c05",
                "trackers": [],
                "output_folder": config.download_dir.to_str().expect("utf-8 path"),
                "only_files": serde_json::Value::Null,
                "is_paused": false,
            }
        }
    });
    std::fs::write(
        config.session_dir.join("session.json"),
        serde_json::to_string(&session_json).expect("serialize"),
    )
    .expect("write session.json");

    let started = std::time::Instant::now();
    let result = Engine::start_within(config, Duration::from_secs(5)).await;
    let elapsed = started.elapsed();

    assert!(
        matches!(result, Err(EngineError::SessionStartTimeout { .. })),
        "a sidecar-less row should time out rather than hang, got {result:?}"
    );
    // The deadline has to be enforced rather than merely declared: before the
    // fix this call never returned at all.
    assert!(
        elapsed < Duration::from_secs(60),
        "the deadline was not enforced; waited {elapsed:?}"
    );
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
        // `remove` returns the hash it destroyed; normalised so the
        // unknown-id assertion below can treat every call the same.
        engine.remove(999, false).await.map(|_| ()),
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

/// Waits for a path to appear, or gives up.
///
/// librqbit lays a torrent's files out asynchronously after an add, so a bare
/// assertion right afterwards races it.
async fn wait_for(path: &std::path::Path) -> bool {
    for _ in 0..50 {
        if path.exists() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// Adds the sample torrent and returns its id and the file it lays down.
async fn add_sample(tmp: &TempDir, engine: &Engine) -> (usize, std::path::PathBuf) {
    let path = sample_torrent_file(&tmp.path().join("src")).await;
    let preview = engine
        .preview(TorrentSource::File { path })
        .await
        .expect("preview");
    let id = engine
        .confirm_add(&preview.info_hash, None)
        .await
        .expect("add");

    // A single-file torrent: librqbit lays it directly in the download
    // directory rather than under a folder named for the torrent.
    let file = tmp.path().join("downloads").join("ubuntu.iso");
    assert!(
        wait_for(&file).await,
        "librqbit should lay the file out at {file:?} -- without it the \
         delete assertions below would pass vacuously"
    );

    (id, file)
}

#[tokio::test]
async fn remove_without_delete_leaves_files_on_disk() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");
    let (id, file) = add_sample(&tmp, &engine).await;

    let removed = engine.remove(id, false).await.expect("remove");
    // The hash has to come back from the call: the only id-to-hash mapping
    // lives in the session entry that `delete` destroys, so a caller that
    // looked it up afterwards would find nothing -- or, once ids are recycled,
    // someone else's torrent. See #145.
    assert_eq!(removed.len(), 40, "a hex info hash, not empty: {removed:?}");
    assert!(removed.chars().all(|c| c.is_ascii_hexdigit()));

    assert!(
        engine.torrent_summaries().is_empty(),
        "torrent should be gone from the session"
    );
    // The half of this the name promises, which it never actually checked.
    assert!(
        file.exists(),
        "removing without delete must leave the data alone"
    );

    engine.shutdown().await;
}

/// The destructive path, and the one the confirmation dialog exists to guard.
#[tokio::test]
async fn remove_with_delete_takes_the_files_too() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");
    let (id, file) = add_sample(&tmp, &engine).await;

    engine.remove(id, true).await.expect("remove with delete");

    assert!(
        engine.torrent_summaries().is_empty(),
        "torrent should be gone from the session"
    );
    for _ in 0..50 {
        if !file.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !file.exists(),
        "asking to delete the files must actually delete them, or the \
         confirmation dialog is promising something that does not happen"
    );

    engine.shutdown().await;
}

/// Pause and resume are separate commands; a torrent has to survive the round
/// trip and still be there afterwards.
#[tokio::test]
async fn pause_and_resume_round_trip() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");
    let (id, _) = add_sample(&tmp, &engine).await;

    engine.pause(id).await.expect("pause");
    let paused = engine.torrent_summaries();
    assert_eq!(paused.len(), 1, "pausing must not remove the torrent");
    assert_eq!(
        paused[0].state,
        TorrentState::Paused,
        "a paused torrent should report itself paused"
    );

    engine.resume(id).await.expect("resume");
    let resumed = engine.torrent_summaries();
    assert_eq!(resumed.len(), 1, "resuming must not remove the torrent");
    assert_ne!(
        resumed[0].state,
        TorrentState::Paused,
        "a resumed torrent should no longer report itself paused"
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

/// Requires internet access and a SOCKS5 proxy on 127.0.0.1:1080.
///
/// Proves that configuring a proxy actually routes peer traffic through it,
/// rather than merely being accepted by validation and then ignored. Run a
/// SOCKS5 server locally first and watch its log while this runs.
#[tokio::test]
#[ignore = "requires a SOCKS5 proxy on 127.0.0.1:1080 and network access"]
async fn peer_connections_go_through_a_configured_proxy() {
    let tmp = TempDir::new().expect("temp dir");
    let config = EngineConfig {
        proxy_url: Some("socks5://127.0.0.1:1080".into()),
        ..test_config(&tmp, true)
    };

    let engine = Engine::start(config).await.expect("engine starts");

    // Resolving a magnet needs the DHT to find peers and then *connect* to
    // them. Those outgoing connections are what should appear in the proxy log.
    let preview = tokio::time::timeout(
        Duration::from_secs(120),
        engine.preview(TorrentSource::Magnet {
            uri: UBUNTU_MAGNET.to_string(),
        }),
    )
    .await
    .expect("metadata resolution timed out")
    .expect("preview should succeed through the proxy");

    assert!(
        preview.name.to_lowercase().contains("ubuntu"),
        "unexpected torrent name: {}",
        preview.name
    );

    engine.shutdown().await;
}

/// Rate limits apply to the running session, not on next launch.
///
/// The settings screen has no OK or Apply button by design, so a change that
/// only took effect after a restart would be a silent lie — the UI would show
/// the new cap while the session kept using the old one.
#[tokio::test]
async fn rate_limits_apply_to_the_running_session() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");

    assert_eq!(
        engine.current_limits(),
        (None, None),
        "a fresh session starts uncapped"
    );

    engine.apply_limits(Some(2_000_000), Some(500_000));
    assert_eq!(
        engine.current_limits(),
        (Some(2_000_000), Some(500_000)),
        "a limit set while running must be in force immediately"
    );

    // Lifting a cap is the direction that matters most: a stuck limit would
    // throttle the user with the UI claiming otherwise.
    engine.apply_limits(None, None);
    assert_eq!(
        engine.current_limits(),
        (None, None),
        "clearing a limit must lift it, not leave the old one in place"
    );

    engine.shutdown().await;
}

/// A magnet nobody can answer must fail, not hang.
///
/// This is the bug that shipped in 1.0.0: `preview` awaited librqbit's
/// metadata fetch with no deadline, so a magnet whose torrent has no reachable
/// seeder left the add dialog on "Fetching the file list from peers" forever.
/// The message reads identically at two seconds and at twenty minutes.
///
/// Uses `preview_within` with a short deadline rather than the real one, so
/// this costs a second rather than a minute. The DHT is enabled because the
/// point is that even *with* somewhere to look, an unanswerable info hash has
/// to give up.
#[tokio::test]
async fn a_magnet_nobody_answers_times_out_rather_than_hanging() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp, true))
        .await
        .expect("engine starts");

    // A random info hash: syntactically valid, and no swarm exists for it.
    let uri = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567".to_string();

    let started = Instant::now();
    let result = engine
        .preview_within(TorrentSource::Magnet { uri }, Duration::from_millis(750))
        .await;
    let waited = started.elapsed();

    match result {
        Err(EngineError::MetadataTimeout { seconds }) => {
            assert_eq!(seconds, 0, "sub-second deadlines report as 0 whole seconds");
        }
        Err(other) => panic!("expected a metadata timeout, got: {other}"),
        Ok(_) => panic!("a random info hash should not resolve to a file list"),
    }

    assert!(
        waited < Duration::from_secs(10),
        "the deadline should bound the wait; waited {waited:?}"
    );

    engine.shutdown().await;
}

/// The deadline is for magnets only.
///
/// A `.torrent` already carries its file list, so bounding it would only add a
/// way to fail. This passes a deadline far too short for any network fetch and
/// expects the local read to succeed regardless.
#[tokio::test]
async fn a_torrent_file_is_not_subject_to_the_magnet_deadline() {
    let tmp = TempDir::new().expect("temp dir");
    let path = sample_torrent_file(&tmp.path().join("src")).await;
    let engine = Engine::start(test_config(&tmp, false))
        .await
        .expect("engine starts");

    let preview = engine
        .preview_within(TorrentSource::File { path }, Duration::from_nanos(1))
        .await
        .expect("a local .torrent should not be subject to a network deadline");

    assert_eq!(preview.files.len(), 1);

    engine.shutdown().await;
}
