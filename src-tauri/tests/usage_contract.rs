//! Pins the usage wire format against the collector's allowlist.
//!
//! `collector/schema.json` is what the Worker validates every incoming batch
//! against, and it is a hand-maintained file in a different language. This test
//! is the thing that stops the two drifting: it serialises one of every event
//! variant with one of every enum value and asserts the fixture describes
//! exactly that set — no more, no less.
//!
//! A new event or a new field therefore fails here until the collector is
//! taught about it, which is the correct order. The alternative — the client
//! sending something the collector silently drops — looks like working code
//! and loses data.

// `expect` is the right tool in tests: a failed expectation is the diagnostic.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use flume_lib::usage::{
    AddSource, CountBucket, DurationBucket, Envelope, Event, EventKind, FailureKind,
    SCHEMA_VERSION, SettingKey,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Every event variant, with every value of every enum it carries.
///
/// Written out rather than derived. A macro that enumerated variants
/// automatically would keep compiling when someone added one, which is exactly
/// the failure this test exists to cause.
fn every_event() -> Vec<EventKind> {
    let mut events = vec![
        EventKind::Launched,
        EventKind::TorrentAdded,
        EventKind::TorrentCompleted,
        EventKind::TorrentRemoved { deleted_data: true },
        EventKind::TorrentRemoved {
            deleted_data: false,
        },
    ];

    for bucket in [
        DurationBucket::UnderFiveMinutes,
        DurationBucket::UnderHalfHour,
        DurationBucket::UnderTwoHours,
        DurationBucket::UnderEightHours,
        DurationBucket::AllDay,
    ] {
        events.push(EventKind::SessionEnded {
            duration_bucket: bucket,
        });
    }

    for bucket in [
        CountBucket::None,
        CountBucket::Few,
        CountBucket::Some,
        CountBucket::Many,
        CountBucket::Lots,
    ] {
        events.push(EventKind::LibraryCount { bucket });
        events.push(EventKind::LibraryImported { added: bucket });
    }

    for source in [AddSource::Magnet, AddSource::File] {
        events.push(EventKind::TorrentPreviewed { source });
    }

    for key in [
        SettingKey::SpeedDownload,
        SettingKey::SpeedUpload,
        SettingKey::FilesDownloadDir,
        SettingKey::NetDht,
        SettingKey::NetListenPort,
        SettingKey::NetUpnp,
        SettingKey::NetProxy,
        SettingKey::NetEgressGuard,
        SettingKey::NetEgressInterface,
        SettingKey::UiTheme,
        SettingKey::UiDensity,
        SettingKey::UiRail,
        SettingKey::PrivacyUsage,
    ] {
        events.push(EventKind::SettingChanged { key });
    }

    for kind in [
        FailureKind::InvalidMagnet,
        FailureKind::Metadata,
        FailureKind::MetadataTimeout,
        FailureKind::NoPendingPreview,
        FailureKind::UnknownTorrent,
        FailureKind::EngineFailed,
        FailureKind::OperationFailed,
        FailureKind::SettingsSaveFailed,
        FailureKind::SettingsInvalid,
        FailureKind::EngineNotReady,
    ] {
        events.push(EventKind::OperationFailed { kind });
    }

    events
}

/// Reads the collector's allowlist.
fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../collector/schema.json");
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("could not read {path}: {e}; the collector must ship with the app")
    });
    serde_json::from_str(&raw).expect("collector/schema.json is not valid JSON")
}

/// Serialises an event kind into its JSON object.
fn as_object(kind: EventKind) -> Map<String, Value> {
    let value = serde_json::to_value(Event { at: 0, kind }).expect("serialise");
    let mut object = value.as_object().expect("an object").clone();
    object.remove("at");
    object
}

/// What the Rust enums actually produce: event name -> field -> values.
fn observed() -> BTreeMap<String, BTreeMap<String, BTreeSet<String>>> {
    let mut observed: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    for kind in every_event() {
        let object = as_object(kind);
        let name = object
            .get("event")
            .and_then(Value::as_str)
            .expect("every event is tagged")
            .to_owned();

        let fields = observed.entry(name).or_default();
        for (key, value) in &object {
            if key == "event" {
                continue;
            }
            fields
                .entry(key.clone())
                .or_default()
                .insert(value.to_string());
        }
    }

    observed
}

#[test]
fn the_collector_knows_every_event_the_client_can_send() {
    let fixture = fixture();
    let declared: BTreeSet<String> = fixture["events"]
        .as_object()
        .expect("events is an object")
        .keys()
        .cloned()
        .collect();
    let actual: BTreeSet<String> = observed().keys().cloned().collect();

    assert_eq!(
        actual, declared,
        "collector/schema.json lists different events from the Rust enum; \
         the collector rejects anything it does not list, so a client-only \
         addition would be silently dropped"
    );
}

#[test]
fn the_collector_knows_every_field_and_value() {
    let fixture = fixture();

    for (event, fields) in observed() {
        let declared = &fixture["events"][&event];
        let declared = declared
            .as_object()
            .unwrap_or_else(|| panic!("collector/schema.json has no entry for {event}"));

        let declared_fields: BTreeSet<String> = declared.keys().cloned().collect();
        let actual_fields: BTreeSet<String> = fields.keys().cloned().collect();
        assert_eq!(
            actual_fields, declared_fields,
            "fields for {event} differ between the Rust enum and the collector"
        );

        for (field, values) in fields {
            let declared_values: BTreeSet<String> = declared[&field]
                .as_array()
                .unwrap_or_else(|| panic!("{event}.{field} is not a list in the collector schema"))
                .iter()
                .map(Value::to_string)
                .collect();

            assert_eq!(
                values, declared_values,
                "values for {event}.{field} differ between the Rust enum and the collector"
            );
        }
    }
}

#[test]
fn every_event_carries_at_most_one_field() {
    // The collector's table is (field, value) rather than a column per event,
    // and its validator reads `Object.keys(fields)[0]`. A two-field event would
    // be silently half-stored, so it is refused here instead.
    for (event, fields) in observed() {
        assert!(
            fields.len() <= 1,
            "{event} carries {} fields; the collector stores one",
            fields.len()
        );
    }
}

#[test]
fn the_envelope_matches_the_collector() {
    let fixture = fixture();

    assert_eq!(
        fixture["schema"].as_u64(),
        Some(u64::from(SCHEMA_VERSION)),
        "schema version differs between the client and the collector"
    );

    let envelope = Envelope {
        schema: SCHEMA_VERSION,
        install_id: "9f1d6f2e-9a4b-4c2d-8f3a-1b2c3d4e5f60".to_owned(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        events: vec![Event {
            at: 0,
            kind: EventKind::Launched,
        }],
    };
    let value = serde_json::to_value(&envelope).expect("serialise");

    let actual: BTreeSet<String> = value
        .as_object()
        .expect("an object")
        .keys()
        .cloned()
        .collect();
    let declared: BTreeSet<String> = fixture["envelope"]["required"]
        .as_array()
        .expect("required is a list")
        .iter()
        .map(|v| v.as_str().expect("a string").to_owned())
        .collect();

    assert_eq!(
        actual, declared,
        "envelope keys differ; the collector requires exactly the keys it lists"
    );
}

#[test]
fn this_platform_is_one_the_collector_accepts() {
    // A target whose `OS` or `ARCH` string is not in the allowlist would have
    // every batch rejected with a 400, from a build that otherwise looks fine.
    let fixture = fixture();

    for (what, actual) in [
        ("os", std::env::consts::OS),
        ("arch", std::env::consts::ARCH),
    ] {
        let allowed: Vec<&str> = fixture["envelope"][what]
            .as_array()
            .expect("a list")
            .iter()
            .map(|v| v.as_str().expect("a string"))
            .collect();

        assert!(
            allowed.contains(&actual),
            "the collector does not accept {what} {actual:?}; it allows {allowed:?}"
        );
    }
}
