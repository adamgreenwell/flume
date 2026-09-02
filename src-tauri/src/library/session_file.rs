//! A reader for librqbit's `session.json`.
//!
//! # Why Flume parses someone else's file
//!
//! Reconciliation cannot work from the live session alone. A torrent missing
//! from [`librqbit::Session`] is not necessarily gone — the restore loop logs
//! and continues when an add fails, keeping the row and retrying next launch —
//! so an unmounted drive makes every torrent on it absent this launch and
//! present the next. And while the egress guard holds there is no session at
//! all, so the live reading is *empty rather than stale*.
//!
//! librqbit will not show Flume its persisted rows either. `session_persistence`
//! is a private module, `SerializedTorrent`'s fields are private, and
//! `Session::persistence` has no accessor. So this is a pinned mirror of
//! someone else's wire format, treated exactly as `collector/schema.json` is:
//! the rev is pinned in `Cargo.toml`, so the shape cannot move without a
//! deliberate bump, and [`tests`] fails loudly if it does anyway.
//!
//! Only the fields Flume needs are mirrored. Unknown ones are ignored, so an
//! upstream addition is not a parse failure.

use std::{collections::HashMap, path::Path};

use serde::Deserialize;

/// The filename librqbit uses inside the session directory.
const SESSION_FILE: &str = "session.json";

/// One persisted torrent, as far as Flume cares.
#[derive(Debug, Clone, Deserialize)]
struct Row {
    /// Hex info hash, exactly the string Flume keys everything else on.
    info_hash: String,
}

/// The file's top level.
#[derive(Debug, Default, Deserialize)]
struct Database {
    /// Keyed by session id, which Flume deliberately ignores: ids are recycled
    /// (`next_id` is `max(keys) + 1` over the persisted map), so the only
    /// stable identity here is the info hash.
    #[serde(default)]
    torrents: HashMap<String, Row>,
}

/// Every info hash librqbit has persisted, or `None` if that cannot be read.
///
/// `None` is not the same as an empty set, and the difference is the whole
/// point: an empty set means "librqbit is persisting no torrents", while `None`
/// means "Flume does not know", and reconciliation must not treat the second as
/// the first. A missing file is `Some(empty)` — that is a real first run.
///
/// Never an error type. Nothing a caller can do about an unreadable
/// `session.json` differs from what it does about an absent one, and this is a
/// side errand on the way to starting an engine.
#[must_use]
pub fn persisted_info_hashes(session_dir: &Path) -> Option<Vec<String>> {
    let path = session_dir.join(SESSION_FILE);

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        // A first run has no session file, and that is a genuine empty set.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };

    let db: Database = serde_json::from_str(&raw).ok()?;
    Some(
        db.torrents
            .into_values()
            .map(|row| row.info_hash.to_ascii_lowercase())
            .collect(),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write(dir: &Path, body: &str) {
        std::fs::write(dir.join(SESSION_FILE), body).expect("write");
    }

    /// The shape librqbit actually writes, as of the pinned rev.
    ///
    /// This is the test the module doc promises. If librqbit's format moves
    /// under a rev bump, this fails rather than the reader silently returning
    /// an empty set and reconciliation quietly doing nothing.
    #[test]
    fn reads_the_shape_librqbit_writes() {
        let tmp = tempfile::tempdir().expect("temp dir");
        write(
            tmp.path(),
            r#"{"torrents":{"0":{"info_hash":"B4C9A1F70E2D83A6157C0D4E9B2F8A1D3E6F7C05",
               "trackers":["http://tracker.example/announce"],
               "output_folder":"/tmp/downloads","only_files":null,"is_paused":false}}}"#,
        );

        let hashes = persisted_info_hashes(tmp.path()).expect("readable");

        assert_eq!(hashes, vec!["b4c9a1f70e2d83a6157c0d4e9b2f8a1d3e6f7c05"]);
    }

    #[test]
    fn lower_cases_the_hash_so_it_matches_what_the_engine_reports() {
        // librqbit serialises the hash in upper case here and Flume's
        // `TorrentSummary::info_hash` is lower case. A mismatch would make
        // every record look absent and reconciliation would recreate the lot.
        let tmp = tempfile::tempdir().expect("temp dir");
        write(
            tmp.path(),
            r#"{"torrents":{"7":{"info_hash":"AABBCCDDEEFF00112233445566778899AABBCCDD"}}}"#,
        );

        assert_eq!(
            persisted_info_hashes(tmp.path()).expect("readable"),
            vec!["aabbccddeeff00112233445566778899aabbccdd"]
        );
    }

    #[test]
    fn ignores_fields_it_does_not_know_about() {
        // An upstream addition must not read as a corrupt file.
        let tmp = tempfile::tempdir().expect("temp dir");
        write(
            tmp.path(),
            r#"{"torrents":{"0":{"info_hash":"aa","something_new":42}},"a_new_top_level":true}"#,
        );

        assert_eq!(
            persisted_info_hashes(tmp.path()).expect("readable").len(),
            1
        );
    }

    #[test]
    fn a_missing_file_is_an_empty_set_not_an_unknown() {
        // A first run genuinely has no torrents; that is not the same as being
        // unable to tell.
        let tmp = tempfile::tempdir().expect("temp dir");
        assert_eq!(persisted_info_hashes(tmp.path()), Some(Vec::new()));
    }

    #[test]
    fn an_unparseable_file_is_unknown_rather_than_empty() {
        // The distinction reconciliation depends on. Returning an empty set
        // here would say "librqbit persists nothing", and a caller that
        // believed it would treat every record as an orphan.
        let tmp = tempfile::tempdir().expect("temp dir");
        write(tmp.path(), "{ not json");

        assert_eq!(persisted_info_hashes(tmp.path()), None);
    }

    #[test]
    fn an_empty_torrent_map_is_an_empty_set() {
        let tmp = tempfile::tempdir().expect("temp dir");
        write(tmp.path(), r#"{"torrents":{}}"#);
        assert_eq!(persisted_info_hashes(tmp.path()), Some(Vec::new()));
    }
}
