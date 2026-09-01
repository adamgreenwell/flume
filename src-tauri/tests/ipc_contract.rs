//! Guards the serialized shape of every type that crosses the IPC boundary.
//!
//! # What this replaces, and why
//!
//! This previously drove commands through Tauri's mock runtime, standing up a
//! real application and webview to invoke a handler. That could not run on
//! Windows at all — the test binary failed to start with
//! `STATUS_ENTRYPOINT_NOT_FOUND`, because binaries in `target/deps/` do not get
//! the DLL placement a bundled application does (issue #39).
//!
//! Fixing that by making WebView2 reachable would have missed the point. The
//! test's value was never that Tauri can construct a window — it was that the
//! JSON reaching the frontend has the field names `src/lib/ipc/types.ts`
//! declares. Booting a GUI framework to check `serde` output is coupling that
//! should not have been there.
//!
//! Asserting the serialization directly runs everywhere, is far faster, and
//! covers *every* IPC type rather than only `CoreStatus`.
//!
//! # What is no longer covered
//!
//! That `#[tauri::command]` registration and argument extraction work. That is
//! Tauri's own machinery rather than Flume's logic, and `generate_handler!`
//! fails at compile time if a handler's signature is wrong — so the loss is
//! small and the compiler catches most of it.

// `expect` is the right tool in tests: a failed expectation is the diagnostic.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use flume_lib::{
    commands::CommandError,
    egress::{EgressPath, EgressReport, Hop, InterfaceKind, Verdict},
    engine::{
        Bottleneck, CoreStatus, DhtStatus, EngineHealth, LimitFactor, Note, NoteSeverity, PeerInfo,
        PieceMap, SwarmHealth, SwarmStats, TelemetrySnapshot, TorrentDetail, TorrentFile,
        TorrentFileState, TorrentPreview, TorrentSource, TorrentState, TorrentSummary,
    },
    settings::{Settings, Theme},
};
use serde_json::Value;

/// Asserts a value serializes to an object carrying exactly `expected` keys.
///
/// Exact rather than "contains": an extra field is as much a contract change as
/// a missing one, and a silently added field is how a TypeScript mirror drifts
/// out of date without anyone noticing.
fn assert_keys<T: serde::Serialize>(value: &T, expected: &[&str], what: &str) {
    let json = serde_json::to_value(value).expect("should serialize");
    let object = json
        .as_object()
        .unwrap_or_else(|| panic!("{what} should be a JSON object"));

    let mut actual: Vec<&str> = object.keys().map(String::as_str).collect();
    let mut want: Vec<&str> = expected.to_vec();
    actual.sort_unstable();
    want.sort_unstable();

    assert_eq!(
        actual, want,
        "{what}: serialized keys do not match src/lib/ipc/types.ts"
    );
}

/// Serializes a value and returns it as JSON.
fn json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("should serialize")
}

fn sample_dht() -> DhtStatus {
    DhtStatus {
        enabled: true,
        nodes_v4: 42,
        nodes_v6: 7,
        outstanding_requests: 3,
    }
}

fn sample_core() -> CoreStatus {
    CoreStatus {
        client_version: "Flume 0.1.0".into(),
        listen_port: Some(42221),
        announce_port: Some(42221),
        dht: sample_dht(),
        download_dir: "/tmp/downloads".into(),
        uptime_seconds: 12,
        download_bps: 1024,
        upload_bps: 512,
        live_peers: 3,
        health: EngineHealth::Ready,
    }
}

fn sample_summary() -> TorrentSummary {
    TorrentSummary {
        id: 1,
        info_hash: "abc123".into(),
        name: "ubuntu.iso".into(),
        state: TorrentState::Downloading,
        progress_bytes: 500,
        total_bytes: 1000,
        uploaded_bytes: 100,
        download_bps: 2048,
        upload_bps: 256,
        live_peers: 5,
        known_peers: 42,
        health: SwarmHealth::Unknown,
        detail: "2 min 30 s left".into(),
        eta_seconds: Some(30),
        finished: false,
        error: None,
        output_folder: "/tmp/downloads".into(),
    }
}

#[test]
fn core_status_matches_the_typescript_mirror() {
    assert_keys(
        &sample_core(),
        &[
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
        ],
        "CoreStatus",
    );
}

#[test]
fn dht_status_matches_the_typescript_mirror() {
    assert_keys(
        &sample_dht(),
        &["enabled", "nodesV4", "nodesV6", "outstandingRequests"],
        "DhtStatus",
    );
}

#[test]
fn torrent_summary_matches_the_typescript_mirror() {
    assert_keys(
        &sample_summary(),
        &[
            "id",
            "infoHash",
            "name",
            "state",
            "progressBytes",
            "totalBytes",
            "uploadedBytes",
            "downloadBps",
            "uploadBps",
            "livePeers",
            "knownPeers",
            "health",
            "detail",
            "etaSeconds",
            "finished",
            "error",
            "outputFolder",
        ],
        "TorrentSummary",
    );
}

#[test]
fn telemetry_snapshot_matches_the_typescript_mirror() {
    let snapshot = TelemetrySnapshot {
        core: sample_core(),
        torrents: vec![sample_summary()],
    };
    assert_keys(&snapshot, &["core", "torrents"], "TelemetrySnapshot");
}

#[test]
fn settings_match_the_typescript_mirror() {
    assert_keys(
        &Settings::default(),
        &[
            "downloadDir",
            "listenPort",
            "enableDht",
            "enableUpnp",
            "downloadLimitBps",
            "uploadLimitBps",
            "proxyUrl",
            "theme",
            "density",
            "egressGuard",
            "egressInterface",
            "usageReporting",
        ],
        "Settings",
    );
}

#[test]
fn the_egress_path_matches_the_typescript_mirror() {
    assert_keys(
        &EgressPath {
            v4: Some(Hop {
                interface: "utun6".into(),
                kind: InterfaceKind::Tunnel,
            }),
            v6: None,
        },
        &["v4", "v6"],
        "EgressPath",
    );

    let hop = json(&Hop {
        interface: "wg-torguard".into(),
        kind: InterfaceKind::Tunnel,
    });
    assert_eq!(hop["interface"], "wg-torguard");
    assert_eq!(hop["kind"], "tunnel");
}

#[test]
fn the_egress_report_matches_the_typescript_mirror() {
    assert_keys(
        &EgressReport {
            path: EgressPath::default(),
            verdict: Verdict::Unknown,
        },
        &["path", "verdict"],
        "EgressReport",
    );
}

#[test]
fn every_verdict_matches_the_typescript_mirror() {
    // The tag and the field names both, because `rename_all` renames only the
    // variants: a field arriving as `other_family_outside` would read as
    // `undefined` in the mirror and the IPv6 leak warning would never render.
    let cases = [
        (
            json(&Verdict::Tunnelled {
                interface: "utun6".into(),
                other_family_outside: true,
            }),
            "tunnelled",
            vec!["verdict", "interface", "otherFamilyOutside"],
        ),
        (
            json(&Verdict::Pinned {
                interface: "Local Area Connection".into(),
                other_family_outside: false,
            }),
            "pinned",
            vec!["verdict", "interface", "otherFamilyOutside"],
        ),
        (
            json(&Verdict::Direct {
                interface: "en7".into(),
            }),
            "direct",
            vec!["verdict", "interface"],
        ),
        (
            json(&Verdict::WrongTunnel {
                interface: "utun7".into(),
                expected: "utun6".into(),
            }),
            "wrongTunnel",
            vec!["verdict", "interface", "expected"],
        ),
        (json(&Verdict::Unknown), "unknown", vec!["verdict"]),
    ];

    for (value, tag, mut want) in cases {
        assert_eq!(value["verdict"], tag, "verdict tag for {tag}");

        let mut actual: Vec<&str> = value
            .as_object()
            .expect("a verdict is an object")
            .keys()
            .map(String::as_str)
            .collect();
        actual.sort_unstable();
        want.sort_unstable();
        assert_eq!(actual, want, "fields of the {tag} verdict");
    }
}

#[test]
fn torrent_preview_matches_the_typescript_mirror() {
    let preview = TorrentPreview {
        info_hash: "abc".into(),
        name: "ubuntu".into(),
        total_bytes: 100,
        files: vec![TorrentFile {
            index: 0,
            path: "a.iso".into(),
            length: 100,
        }],
        already_added: false,
        save_path: "/tmp/downloads".into(),
        free_bytes: Some(1_420_000_000_000),
        seen_peers: 6,
        already_on_disk: vec![false],
    };
    assert_keys(
        &preview,
        &[
            "infoHash",
            "name",
            "totalBytes",
            "files",
            "alreadyAdded",
            "savePath",
            "freeBytes",
            "seenPeers",
            "alreadyOnDisk",
        ],
        "TorrentPreview",
    );
    assert_keys(
        &preview.files[0],
        &["index", "path", "length"],
        "TorrentFile",
    );
}

#[test]
fn torrent_detail_matches_the_typescript_mirror() {
    let detail = TorrentDetail {
        peers: vec![PeerInfo {
            address: "1.2.3.4:6881".into(),
            client: Some("Transmission".into()),
            transport: Some("tcp".into()),
            state: "live".into(),
            downloaded_bytes: 10,
            uploaded_bytes: 20,
            pieces_contributed: 4,
            errors: 0,
        }],
        trackers: vec!["https://tracker/announce".into()],
        pieces: Some(PieceMap {
            total_pieces: 100,
            pieces_complete: 42,
            pieces_per_bucket: 1,
            buckets: vec![255, 0],
            availability: Some(vec![4, 9]),
        }),
        swarm: SwarmStats {
            live: 3,
            connecting: 1,
            queued: 8,
            seen: 40,
            dead: 2,
            live_tcp: 2,
            live_utp: 1,
            seeds: Some(1),
            availability: Some(2.5),
            rarest: Some(2),
        },
        note: Note {
            severity: NoteSeverity::Ok,
            title: "Pulling from 3 of 40 known peers".into(),
            body: "500 B verified so far, 500 B to go.".into(),
        },
        bottleneck: Some(Bottleneck {
            factors: vec![LimitFactor {
                name: "Peer upload".into(),
                utilisation: Some(100.0),
                value: "6.6 MB/s".into(),
                binding: true,
            }],
            explanation: "The peers are supplying all they have.".into(),
        }),
    };
    assert_keys(
        &detail,
        &["peers", "trackers", "pieces", "swarm", "note", "bottleneck"],
        "TorrentDetail",
    );
    assert_keys(
        &detail.swarm,
        &[
            "live",
            "connecting",
            "queued",
            "seen",
            "dead",
            "seeds",
            "availability",
            "rarest",
            "liveTcp",
            "liveUtp",
        ],
        "SwarmStats",
    );
    assert_keys(
        &detail.peers[0],
        &[
            "address",
            "client",
            "transport",
            "state",
            "downloadedBytes",
            "uploadedBytes",
            "piecesContributed",
            "errors",
        ],
        "PeerInfo",
    );
    assert_keys(
        detail.pieces.as_ref().expect("pieces"),
        &[
            "totalPieces",
            "piecesComplete",
            "piecesPerBucket",
            "buckets",
            "availability",
        ],
        "PieceMap",
    );
}

#[test]
fn torrent_file_state_matches_the_typescript_mirror() {
    assert_keys(
        &TorrentFileState {
            index: 0,
            path: "a.iso".into(),
            length: 100,
            progress_bytes: 50,
            selected: true,
            first_piece: 0,
            last_piece: 4,
            piece_buckets: vec![255, 255, 0, 0],
        },
        &[
            "index",
            "path",
            "length",
            "progressBytes",
            "selected",
            "firstPiece",
            "lastPiece",
            "pieceBuckets",
        ],
        "TorrentFileState",
    );
}

#[test]
fn command_error_matches_the_typescript_mirror() {
    let err = CommandError::not_ready();
    assert_keys(&err, &["kind", "message"], "CommandError");
    assert_eq!(
        json(&err).get("kind").and_then(Value::as_str),
        Some("engineNotReady"),
        "the frontend branches on this identifier, not on message text"
    );
    assert!(
        json(&err)
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|m| !m.is_empty()),
        "a human-readable message must always accompany the kind"
    );
}

#[test]
fn enums_serialize_as_camel_case_strings() {
    // The TypeScript mirrors declare these as string unions. A serde default
    // would emit PascalCase and every comparison in the UI would silently fail.
    for (health, expected) in [
        (EngineHealth::Starting, "starting"),
        (EngineHealth::Connecting, "connecting"),
        (EngineHealth::Ready, "ready"),
        (EngineHealth::Degraded, "degraded"),
    ] {
        assert_eq!(json(&health).as_str(), Some(expected), "EngineHealth");
    }

    for (health, expected) in [
        (SwarmHealth::Seeding, "seeding"),
        (SwarmHealth::None, "none"),
        (SwarmHealth::Idle, "idle"),
        (SwarmHealth::Unknown, "unknown"),
        (SwarmHealth::Healthy, "healthy"),
        (SwarmHealth::Thin, "thin"),
    ] {
        assert_eq!(json(&health).as_str(), Some(expected), "SwarmHealth");
    }

    for (state, expected) in [
        (TorrentState::Checking, "checking"),
        (TorrentState::Downloading, "downloading"),
        (TorrentState::Seeding, "seeding"),
        (TorrentState::Paused, "paused"),
        (TorrentState::Error, "error"),
    ] {
        assert_eq!(json(&state).as_str(), Some(expected), "TorrentState");
    }

    for (theme, expected) in [
        (Theme::System, "system"),
        (Theme::Light, "light"),
        (Theme::Dark, "dark"),
    ] {
        assert_eq!(json(&theme).as_str(), Some(expected), "Theme");
    }
}

#[test]
fn torrent_source_is_an_internally_tagged_union() {
    // types.ts declares this as a discriminated union on `kind`. The tag must
    // be present and camelCase, or the backend rejects everything the add
    // dialog sends.
    let magnet = TorrentSource::Magnet {
        uri: "magnet:?xt=urn:btih:abc".into(),
    };
    assert_keys(&magnet, &["kind", "uri"], "TorrentSource::Magnet");
    assert_eq!(
        json(&magnet).get("kind").and_then(Value::as_str),
        Some("magnet")
    );

    let file = TorrentSource::File {
        path: "/tmp/a.torrent".into(),
    };
    assert_keys(&file, &["kind", "path"], "TorrentSource::File");
    assert_eq!(
        json(&file).get("kind").and_then(Value::as_str),
        Some("file")
    );
}

#[test]
fn optional_fields_serialize_as_null_not_absent() {
    // types.ts declares these as `T | null`. If serde omitted them instead,
    // TypeScript would need `T | undefined` and the mirrors would be wrong.
    let summary = TorrentSummary {
        eta_seconds: None,
        error: None,
        ..sample_summary()
    };
    let value = json(&summary);
    assert!(value.get("etaSeconds").is_some_and(Value::is_null));
    assert!(value.get("error").is_some_and(Value::is_null));

    let core = CoreStatus {
        listen_port: None,
        announce_port: None,
        ..sample_core()
    };
    let value = json(&core);
    assert!(value.get("listenPort").is_some_and(Value::is_null));
    assert!(value.get("announcePort").is_some_and(Value::is_null));
}
