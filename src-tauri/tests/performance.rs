//! Performance characteristics of the telemetry path (#17).
//!
//! The non-functional requirement is that the UI stays responsive with many
//! active torrents. Everything the UI renders comes from one `telemetry()` call
//! per second, so that call is the whole budget: if it stays fast and its
//! payload stays small, the interface cannot be starved by torrent count.
//!
//! **These torrents have no peers.** `test_config` disables the DHT and the
//! torrents are built locally, so nothing ever connects. That makes the numbers
//! here a floor rather than a realistic tick: everything gated on a live swarm
//! is skipped, including the availability walk, which is O(peers x pieces) and
//! the most expensive thing in a real tick. `analyse_stays_within_its_share_of_a_telemetry_tick`
//! in `src/engine/availability.rs` covers that separately, with synthetic
//! bitfields, because a swarm of the size that matters cannot be stood up here.
//!
//! Do not read a pass here as "telemetry is fast with 15 busy torrents". It is
//! "telemetry is fast with 15 idle ones", which is a weaker claim.
//!
//! These are gated behind `--ignored` because timing assertions on a shared CI
//! runner are flaky, and a flaky perf test gets ignored rather than fixed. Run
//! locally before a release:
//!
//! ```text
//! cargo test --test performance -- --ignored --nocapture
//! ```

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::{Duration, Instant};

use flume_lib::engine::{Engine, EngineConfig, TorrentSource};
use tempfile::TempDir;

/// How many torrents to load. The roadmap's target is "10+".
const TORRENT_COUNT: usize = 15;

/// Telemetry runs once a second. Anything approaching that is a problem; this
/// leaves two orders of magnitude of headroom.
const TELEMETRY_BUDGET: Duration = Duration::from_millis(10);

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

/// Builds a distinct torrent, so each has its own info hash.
async fn make_torrent(root: &std::path::Path, index: usize) -> String {
    let dir = root.join(format!("t{index}"));
    std::fs::create_dir_all(&dir).expect("dir");
    // Contents differ per torrent, otherwise identical payloads would produce
    // identical info hashes and the session would deduplicate them.
    std::fs::write(dir.join("payload.bin"), vec![index as u8; 32_768]).expect("write");

    let result = librqbit::create_torrent(
        &dir,
        librqbit::CreateTorrentOptions {
            name: Some("perf"),
            piece_length: Some(1024),
            ..Default::default()
        },
        &librqbit::spawn_utils::BlockingSpawner::new(1),
    )
    .await
    .expect("create torrent");

    let path = dir.join("t.torrent");
    std::fs::write(&path, result.as_bytes().expect("encode")).expect("write torrent");
    path.display().to_string()
}

#[tokio::test]
#[ignore = "timing assertions are unreliable on shared CI runners"]
async fn telemetry_stays_fast_with_many_torrents() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp))
        .await
        .expect("engine starts");

    for index in 0..TORRENT_COUNT {
        let path = make_torrent(&tmp.path().join("src"), index).await;
        let preview = engine
            .preview(TorrentSource::File { path })
            .await
            .expect("preview");
        engine
            .confirm_add(&preview.info_hash, None)
            .await
            .expect("add");
    }

    assert_eq!(
        engine.torrent_summaries().len(),
        TORRENT_COUNT,
        "all torrents should be present"
    );

    // Measure the steady-state cost, after any first-call warmup.
    let _ = engine.telemetry();

    let samples = 100;
    let started = Instant::now();
    let mut payload_bytes = 0;
    for _ in 0..samples {
        let snapshot = engine.telemetry();
        payload_bytes = serde_json::to_vec(&snapshot).expect("serialize").len();
    }
    let per_call = started.elapsed() / samples;

    println!("telemetry with {TORRENT_COUNT} torrents: {per_call:?} per call");
    println!("serialized payload: {payload_bytes} bytes");

    assert!(
        per_call < TELEMETRY_BUDGET,
        "telemetry took {per_call:?} per call with {TORRENT_COUNT} torrents, \
         which is too close to the 1s tick budget"
    );

    // The payload is what crosses the IPC boundary every second. It should
    // scale with torrent count only, not with torrent *size* or piece count --
    // that is the whole reason piece data is downsampled on demand rather than
    // streamed.
    assert!(
        payload_bytes < 32_768,
        "telemetry payload was {payload_bytes} bytes for {TORRENT_COUNT} torrents; \
         something torrent-sized is leaking into the per-tick payload"
    );

    engine.shutdown().await;
}

#[tokio::test]
#[ignore = "timing assertions are unreliable on shared CI runners"]
async fn detail_queries_stay_bounded() {
    let tmp = TempDir::new().expect("temp dir");
    let engine = Engine::start(test_config(&tmp))
        .await
        .expect("engine starts");

    let path = make_torrent(&tmp.path().join("src"), 0).await;
    let preview = engine
        .preview(TorrentSource::File { path })
        .await
        .expect("preview");
    let id = engine
        .confirm_add(&preview.info_hash, None)
        .await
        .expect("add");

    let started = Instant::now();
    for _ in 0..50 {
        let _ = engine.torrent_detail(id);
        let _ = engine.torrent_files(id);
    }
    let per_pair = started.elapsed() / 50;

    println!("detail + files query: {per_pair:?} per pair");

    // The detail panel polls at 2 Hz while open; this has ample headroom.
    assert!(
        per_pair < Duration::from_millis(20),
        "detail queries took {per_pair:?}, too slow for a 2 Hz panel"
    );

    engine.shutdown().await;
}
