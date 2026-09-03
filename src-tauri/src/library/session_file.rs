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
//! deliberate bump, and an integration test pins it against a file librqbit
//! really wrote (see below).
//!
//! Only the fields Flume needs are mirrored. Unknown ones are ignored, so an
//! upstream addition is not a parse failure.
//!
//! The tests in this module hand-author their JSON, so they can only fail if
//! *this reader* changes — they cannot notice librqbit's format moving, which
//! is what a pin is for. The real pin is
//! `the_session_file_reader_matches_what_librqbit_writes` in
//! `tests/engine.rs`: it drives an `Engine`, lets librqbit write its own
//! `session.json`, and asserts this reader finds the torrent that is really in
//! it. Verified to fail on a simulated field rename.
//!
//! The info hash is lower-cased on the way out. Not because librqbit varies the
//! case -- it does not, `serialize_info_hash` goes through `Id20::as_string`
//! which is `hex::encode`, the lower-case variant -- but because the failure if
//! it ever did would be silent: every record would look absent, reconciliation
//! would recreate the lot, and every arrival time would read as null with no
//! error anywhere.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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

/// Where librqbit keeps the `.torrent` bytes for one persisted row.
///
/// A mirror of `JsonSessionPersistenceStore::torrent_bytes_filename`, which is
/// `output_folder.join(format!("{info_hash:?}.torrent"))`. Two things about it
/// are load-bearing and neither is obvious:
///
/// - the stem is the *`Debug`* formatting of `Id20`, which writes `{byte:02x?}`
///   per byte — lower-case hex, the same string `persisted_info_hashes`
///   normalises to; and
/// - `output_folder` for the JSON store is the folder holding `session.json`,
///   not the download directory. They are different directories in Flume.
///
/// Pinned against a file librqbit really wrote by
/// `the_missing_sidecar_check_matches_where_librqbit_writes` in
/// `tests/engine.rs`, for the same reason the format itself is pinned.
fn sidecar(session_dir: &Path, info_hash: &str) -> PathBuf {
    session_dir.join(format!("{info_hash}.torrent"))
}

/// Persisted torrents that have no `.torrent` file beside them.
///
/// These are the rows that hang a start. librqbit's `get` warns and returns the
/// row with empty `torrent_bytes` when the sidecar will not open, and
/// `into_add_torrent` branches on that byte length — so an empty one is
/// restored as a *magnet*, and magnet resolution on the restore path has no
/// timeout inside librqbit. For an info hash nobody is seeding it never
/// resolves, and the restore loop cannot exit while a future is pending. See
/// issue #154.
///
/// # Why a `Vec` where [`persisted_info_hashes`] returns an `Option`
///
/// Because the distinction that matters there does not exist here. This is read
/// only to add names to an error message that already stands on its own, and
/// "the file could not be read" and "nothing is missing" lead to the same
/// message: the one without names. Returning an `Option` would invite a caller
/// to tell them apart when there is nothing to tell.
///
/// Sorted, because [`persisted_info_hashes`] reads a `HashMap` and its order
/// varies between runs on identical data. An error that named a different
/// torrent each launch would be worse than one that named none.
#[must_use]
pub fn persisted_without_sidecar(session_dir: &Path) -> Vec<String> {
    let mut missing: Vec<String> = persisted_info_hashes(session_dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|info_hash| !sidecar(session_dir, info_hash).exists())
        .collect();
    missing.sort();
    missing
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
    fn normalises_case_even_though_librqbit_does_not_vary_it() {
        // Both sides are lower-case today: `serialize_info_hash` calls
        // `Id20::as_string`, which is `hex::encode`, which is the lower-case
        // variant -- `hex::encode_upper` exists separately. So this
        // normalisation fixes no observed mismatch.
        //
        // It is kept because the cost of being wrong is asymmetric. If the
        // case ever diverged, every record would look absent, reconciliation
        // would recreate the lot, and every `added_at` would silently read as
        // null -- a failure with no error and no symptom except data quietly
        // going missing. One `to_ascii_lowercase` against that is cheap.
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

    /// Writes a session file listing `hashes`, and a sidecar for each of
    /// `with_sidecar`.
    fn library(dir: &Path, hashes: &[&str], with_sidecar: &[&str]) {
        let rows = hashes
            .iter()
            .enumerate()
            .map(|(id, hash)| format!(r#""{id}":{{"info_hash":"{hash}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        write(dir, &format!(r#"{{"torrents":{{{rows}}}}}"#));

        for hash in with_sidecar {
            std::fs::write(dir.join(format!("{hash}.torrent")), b"d4:infod")
                .expect("write sidecar");
        }
    }

    #[test]
    fn finds_the_rows_whose_sidecar_is_missing() {
        let tmp = tempfile::tempdir().expect("temp dir");
        library(tmp.path(), &["aa", "bb", "cc"], &["bb"]);

        assert_eq!(
            persisted_without_sidecar(tmp.path()),
            vec!["aa".to_owned(), "cc".to_owned()]
        );
    }

    #[test]
    fn a_library_with_every_sidecar_present_has_no_suspects() {
        let tmp = tempfile::tempdir().expect("temp dir");
        library(tmp.path(), &["aa", "bb"], &["aa", "bb"]);

        assert!(persisted_without_sidecar(tmp.path()).is_empty());
    }

    #[test]
    fn suspects_come_back_in_a_stable_order() {
        // `persisted_info_hashes` reads a `HashMap`, whose iteration order is
        // randomised per process. Naming a different torrent on each launch
        // would be worse than naming none, so this asserts the sort rather
        // than trusting the map.
        let tmp = tempfile::tempdir().expect("temp dir");
        library(tmp.path(), &["ff", "aa", "cc", "bb"], &[]);

        assert_eq!(
            persisted_without_sidecar(tmp.path()),
            vec![
                "aa".to_owned(),
                "bb".to_owned(),
                "cc".to_owned(),
                "ff".to_owned()
            ]
        );
    }

    #[test]
    fn an_unreadable_session_file_names_nobody() {
        // Deliberately the same answer as "nothing is missing". The caller adds
        // names to a message that already stands without them, so there is
        // nothing for it to do differently -- and guessing here would name
        // torrents that may be perfectly intact.
        let tmp = tempfile::tempdir().expect("temp dir");
        write(tmp.path(), "{ not json");

        assert!(persisted_without_sidecar(tmp.path()).is_empty());
    }

    #[test]
    fn a_directory_named_like_a_sidecar_still_counts_as_present() {
        // `Path::exists` does not distinguish, and neither does this. librqbit
        // would fail to open it and take the magnet path, so in principle this
        // is a missed suspect -- but a directory called `<hash>.torrent` inside
        // the session folder is not a case worth a `metadata` call to catch,
        // and the message stands without names.
        let tmp = tempfile::tempdir().expect("temp dir");
        library(tmp.path(), &["aa"], &[]);
        std::fs::create_dir(tmp.path().join("aa.torrent")).expect("mkdir");

        assert!(persisted_without_sidecar(tmp.path()).is_empty());
    }
}
