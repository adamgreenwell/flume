//! Magnets added before their file list arrived (#148).
//!
//! Driven through `AppState` against a real librqbit session, because the
//! claims worth pinning are about the seams: a resolved magnet stays out of
//! librqbit until the user confirms, its file list survives a restart, and a
//! confirmed add leaves an ordinary record behind and nothing else.
//!
//! The network half -- an attempt actually fetching a list over the DHT -- is
//! `magnet_resolves_real_metadata_over_the_dht` in `tests/engine.rs`. Here the
//! file list comes from a torrent built on the spot, standing in for what a
//! peer would have sent.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{path::Path, time::Instant};

use flume_lib::{
    engine::TorrentSource,
    library::magnet::{PendingMagnetState, metadata_path},
    settings::Settings,
    state::{AppState, AttemptEnd, PendingMagnetError},
};
use tempfile::TempDir;

/// A real session that reaches nothing on the network.
fn quiet_settings(dir: &Path) -> Settings {
    Settings {
        download_dir: dir.to_path_buf(),
        listen_port: 0,
        enable_dht: false,
        enable_upnp: false,
        ..Settings::default()
    }
}

/// Builds a one-file torrent and returns its encoded bytes.
async fn sample_torrent(dir: &Path) -> Vec<u8> {
    std::fs::create_dir_all(dir).expect("dir");
    std::fs::write(dir.join("disc.iso"), vec![7u8; 4096]).expect("write");
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
    result.as_bytes().expect("encode torrent").to_vec()
}

/// What `library.json` says about one torrent.
fn record_on_disk(session: &Path, info_hash: &str) -> serde_json::Value {
    let raw = std::fs::read_to_string(session.join("library.json")).expect("library.json");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    json["torrents"][info_hash].clone()
}

struct Fixture {
    tmp: TempDir,
    state: AppState,
    settings: Settings,
}

impl Fixture {
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let settings = quiet_settings(&tmp.path().join("downloads"));
        let state = AppState::new(settings.clone(), tmp.path().join("session"), false);
        Self {
            tmp,
            state,
            settings,
        }
    }

    fn session(&self) -> std::path::PathBuf {
        self.tmp.path().join("session")
    }

    async fn start(&self) {
        self.state
            .restart_engine(&self.settings)
            .await
            .expect("engine starts");
    }

    /// The sample torrent's bytes and info hash, leaving nothing pending.
    async fn torrent(&self) -> (Vec<u8>, String) {
        let bytes = sample_torrent(&self.tmp.path().join("src")).await;
        let engine = self.state.engine().await.expect("engine");
        let preview = engine.preview_saved(bytes.clone()).await.expect("preview");
        engine.discard_preview(&preview.info_hash).await;
        (bytes, preview.info_hash)
    }
}

/// The whole life of a pending magnet, from "add anyway" to a real torrent.
#[tokio::test]
async fn a_pending_magnet_holds_for_selection_and_settles_on_add() {
    let fx = Fixture::new();
    fx.start().await;
    let (bytes, hash) = fx.torrent().await;

    // Upper case on purpose: the record is keyed by the lower-case hash
    // every other part of Flume uses.
    let uri = format!(
        "magnet:?xt=urn:btih:{}&dn=Waiting%20Name",
        hash.to_ascii_uppercase()
    );
    let kept = fx.state.add_pending_magnet(&uri).await.expect("kept");
    assert_eq!(kept, hash);

    let engine = fx.state.engine().await.expect("engine");
    let statuses = fx
        .state
        .pending_magnet_statuses(Some(&engine), Instant::now())
        .await;
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].state, PendingMagnetState::Searching);
    assert_eq!(statuses[0].name.as_deref(), Some("Waiting Name"));
    assert!(statuses[0].added_at.is_some());
    // Persisted before anything else happens: it is the only copy.
    assert_eq!(record_on_disk(&fx.session(), &hash)["magnet"]["uri"], uri);

    // Due at once, and claimed only once.
    let now = Instant::now();
    let (claimed, claimed_uri) = fx.state.claim_due_magnet(now).await.expect("due");
    assert_eq!(
        (claimed.as_str(), claimed_uri.as_str()),
        (hash.as_str(), uri.as_str())
    );
    assert!(
        fx.state.claim_due_magnet(now).await.is_none(),
        "claimed twice"
    );
    let during = fx.state.pending_magnet_statuses(Some(&engine), now).await;
    assert!(during[0].searching_now);

    // Unanswered: counted, and not due again for a minute.
    fx.state
        .finish_magnet_attempt(&hash, AttemptEnd::Unanswered, now)
        .await;
    let after = fx.state.pending_magnet_statuses(Some(&engine), now).await;
    assert_eq!(after[0].failed_attempts, 1);
    assert_eq!(after[0].next_attempt_in_seconds, Some(60));
    assert!(fx.state.claim_due_magnet(now).await.is_none());

    // The file list arrives: saved, offered, and *not* added.
    assert!(
        fx.state
            .resolve_pending_magnet(&hash, "flume-test".into(), &bytes)
            .await
    );
    let ready = fx.state.pending_magnet_statuses(Some(&engine), now).await;
    assert_eq!(ready[0].state, PendingMagnetState::Ready);
    assert_eq!(ready[0].name.as_deref(), Some("flume-test"));
    assert_eq!(ready[0].next_attempt_in_seconds, None);
    assert!(
        !engine.has_torrent(&hash),
        "a resolved magnet must hold for file selection, never start"
    );
    assert!(engine.telemetry().torrents.is_empty());
    assert!(metadata_path(&fx.session(), &hash).exists());
    assert!(
        fx.state.claim_due_magnet(now).await.is_none(),
        "a resolved magnet is not tried again"
    );

    // Opening it is an ordinary preview, confirmed the ordinary way.
    let saved = fx.state.saved_magnet_metadata(&hash).await.expect("saved");
    let preview = engine.preview_saved(saved).await.expect("opens");
    assert_eq!(preview.info_hash, hash);
    assert_eq!(preview.name, "flume-test");
    engine
        .confirm_add(&hash, None, None)
        .await
        .expect("confirm");
    fx.state.settle_pending_magnet(&hash).await;

    assert!(engine.has_torrent(&hash));
    assert!(
        fx.state
            .pending_magnet_statuses(Some(&engine), now)
            .await
            .is_empty()
    );
    assert!(!metadata_path(&fx.session(), &hash).exists());
    let record = record_on_disk(&fx.session(), &hash);
    assert!(
        record["magnet"].is_null(),
        "settled record still pending: {record}"
    );
    assert!(
        record["addedAt"].is_u64(),
        "the arrival time the magnet was added with is kept: {record}"
    );

    fx.state.shutdown().await;
}

/// A pending magnet, and a file list that arrived, both survive a restart.
#[tokio::test]
async fn pending_magnets_survive_a_restart() {
    let fx = Fixture::new();
    fx.start().await;
    let (bytes, hash) = fx.torrent().await;
    let other = "0123456789abcdef0123456789abcdef01234567";

    fx.state
        .add_pending_magnet(&format!("magnet:?xt=urn:btih:{hash}"))
        .await
        .expect("kept");
    fx.state
        .add_pending_magnet(&format!("magnet:?xt=urn:btih:{other}"))
        .await
        .expect("kept");
    assert!(
        fx.state
            .resolve_pending_magnet(&hash, "flume-test".into(), &bytes)
            .await
    );
    fx.state.shutdown().await;

    let reopened = AppState::new(fx.settings.clone(), fx.session(), false);
    let statuses = reopened.pending_magnet_statuses(None, Instant::now()).await;
    let state_of = |h: &str| statuses.iter().find(|s| s.info_hash == h).map(|s| s.state);
    assert_eq!(state_of(&hash), Some(PendingMagnetState::Ready));
    assert_eq!(state_of(other), Some(PendingMagnetState::Searching));
    // No engine, so nothing can be scheduled -- and none is claimed as one.
    assert!(statuses.iter().all(|s| s.next_attempt_in_seconds.is_none()));

    // The one still searching is due at once in the new process.
    let (due, _) = reopened
        .claim_due_magnet(Instant::now())
        .await
        .expect("due");
    assert_eq!(due, other);
}

/// Removing one takes its record and its saved file list with it.
#[tokio::test]
async fn removing_a_pending_magnet_forgets_it_and_its_file_list() {
    let fx = Fixture::new();
    fx.start().await;
    let (bytes, hash) = fx.torrent().await;

    fx.state
        .add_pending_magnet(&format!("magnet:?xt=urn:btih:{hash}"))
        .await
        .expect("kept");
    fx.state
        .resolve_pending_magnet(&hash, "flume-test".into(), &bytes)
        .await;

    assert!(fx.state.remove_pending_magnet(&hash).await);
    assert!(!metadata_path(&fx.session(), &hash).exists());
    assert!(record_on_disk(&fx.session(), &hash).is_null());
    assert!(
        fx.state
            .pending_magnet_statuses(None, Instant::now())
            .await
            .is_empty()
    );
    // A second removal finds nothing to remove.
    assert!(!fx.state.remove_pending_magnet(&hash).await);

    // A file list that arrives after the removal is not brought back.
    assert!(
        !fx.state
            .resolve_pending_magnet(&hash, "flume-test".into(), &bytes)
            .await
    );
    assert!(!metadata_path(&fx.session(), &hash).exists());

    fx.state.shutdown().await;
}

/// A file list that went missing sends the magnet back to searching.
#[tokio::test]
async fn a_missing_file_list_goes_back_to_searching() {
    let fx = Fixture::new();
    fx.start().await;
    let (bytes, hash) = fx.torrent().await;

    fx.state
        .add_pending_magnet(&format!("magnet:?xt=urn:btih:{hash}"))
        .await
        .expect("kept");
    fx.state
        .resolve_pending_magnet(&hash, "flume-test".into(), &bytes)
        .await;
    std::fs::remove_file(metadata_path(&fx.session(), &hash)).unwrap();

    assert!(fx.state.saved_magnet_metadata(&hash).await.is_err());
    let statuses = fx.state.pending_magnet_statuses(None, Instant::now()).await;
    assert_eq!(statuses[0].state, PendingMagnetState::Searching);
    assert!(fx.state.claim_due_magnet(Instant::now()).await.is_some());

    fx.state.shutdown().await;
}

/// The refusals, each of which would otherwise lose something quietly.
#[tokio::test]
async fn magnets_that_cannot_be_kept_are_refused() {
    let fx = Fixture::new();
    fx.start().await;

    // No key to store it under.
    let v2_only = format!("magnet:?xt=urn:btmh:1220{}", "ab".repeat(32));
    assert!(matches!(
        fx.state.add_pending_magnet(&v2_only).await,
        Err(PendingMagnetError::Invalid(_))
    ));
    assert!(matches!(
        fx.state
            .add_pending_magnet("magnet:?xt=urn:btih:nope")
            .await,
        Err(PendingMagnetError::Invalid(_))
    ));

    // Already a real torrent.
    let (bytes, hash) = fx.torrent().await;
    let engine = fx.state.engine().await.expect("engine");
    engine.preview_saved(bytes).await.expect("preview");
    engine.confirm_add(&hash, None, None).await.expect("add");
    assert!(matches!(
        fx.state
            .add_pending_magnet(&format!("magnet:?xt=urn:btih:{hash}"))
            .await,
        Err(PendingMagnetError::AlreadyAdded)
    ));

    fx.state.shutdown().await;
}

/// A library that did not load writes nothing back, so a magnet kept in it
/// would vanish at quit. Refused instead, and said.
#[tokio::test]
async fn a_magnet_is_not_kept_when_the_library_did_not_load() {
    let tmp = TempDir::new().expect("tempdir");
    let session = tmp.path().join("session");
    std::fs::create_dir_all(&session).unwrap();
    std::fs::write(session.join("library.json"), "{ not json").unwrap();

    let state = AppState::new(quiet_settings(&tmp.path().join("d")), session, false);
    let result = state
        .add_pending_magnet("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567")
        .await;
    assert!(matches!(
        result,
        Err(PendingMagnetError::LibraryUnavailable)
    ));
}

/// With the DHT off there is nothing to wait for, so peer discovery is ready
/// at once and a magnet preview does not sit out the readiness wait.
#[tokio::test]
async fn without_a_dht_peer_discovery_is_ready_at_once() {
    let fx = Fixture::new();
    fx.start().await;
    let engine = fx.state.engine().await.expect("engine");
    assert!(engine.peer_discovery_ready());
    assert!(
        engine
            .wait_for_peer_discovery(std::time::Duration::from_secs(30))
            .await
    );
    // Still validates before any waiting.
    assert!(
        engine
            .preview(TorrentSource::Magnet {
                uri: "magnet:?xt=urn:btih:bad".into()
            })
            .await
            .is_err()
    );
    fx.state.shutdown().await;
}
