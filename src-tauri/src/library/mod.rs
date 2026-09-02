//! What Flume remembers about a torrent that librqbit does not.
//!
//! Like [`crate::engine`] and [`crate::settings`], this module imports no Tauri
//! types and is testable under a plain `cargo test`.
//!
//! # Why it exists
//!
//! librqbit persists exactly five things per torrent — info hash, trackers,
//! output folder, file selection, paused bit. There is no timestamp at any
//! layer, so "recently added" currently sorts by session id, and that is
//! genuinely lossy: `next_id` is `max(keys) + 1` over the *persisted* map, so
//! removing the newest torrent hands its number to the next add, and a torrent
//! removed while the app is closed frees its number too.
//!
//! # Keyed by info hash, never by session id
//!
//! Ids are recycled, and two torrents can even hold the same id at once —
//! `next_id` is read well before it is claimed, so two concurrent adds can both
//! take it. The info hash is the only stable identity, and it is already the
//! string the rest of Flume keys on.
//!
//! # The rule that shapes everything: this never deletes on absence
//!
//! A torrent missing from the live session is not necessarily gone. librqbit's
//! restore loop logs and continues when an add fails, keeping the row and
//! retrying next launch — so an unmounted external drive makes every torrent on
//! it absent this launch and present the next. While the egress guard holds
//! there is no session at all, and the reading is *empty rather than stale*.
//!
//! The asymmetry decides it. Creating a record for a hash that lacks one costs
//! a few hundred bytes, is invisible in a UI that renders the session's
//! torrents, and heals itself — an insert-if-absent on the re-add finds it and
//! keeps the original timestamp. Deleting a record is irreversible. So records
//! are created and updated, and [`Library::forget`] is called only from the one
//! place that *knows* a torrent was removed.
//!
//! # And it is written before the torrent is added
//!
//! librqbit makes `session.json` durable inside `add_torrent`, before the call
//! returns. A record written afterwards loses a kill in that window — and the
//! timestamp lost is always the freshest one, the torrent added seconds ago,
//! which is exactly what a "recently added" sort is for. Record-first inverts
//! the failure into an orphan record, which the rule above makes harmless.

mod session_file;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

pub use session_file::persisted_info_hashes;

/// Filename inside the app-data directory.
const LIBRARY_FILE: &str = "library.json";

/// What Flume remembers about one torrent.
///
/// Every field is optional and additive on purpose: a record created by
/// reconciliation knows only that the torrent exists, and a record written by
/// an older build must survive a newer one reading it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Record {
    /// When Flume first added this torrent, in whole seconds since the epoch.
    ///
    /// `None` for a torrent that predates this feature, and rendered as absent
    /// rather than guessed at. There is nothing to infer from: librqbit's
    /// restore reads a `HashMap` and pushes the adds concurrently, so both the
    /// start and completion order vary between launches of the same binary on
    /// the same data. A backfill would invent a different fictional order every
    /// time.
    pub added_at: Option<u64>,
}

/// Every record, keyed by lower-case hex info hash.
#[derive(Debug, Default)]
pub struct Library {
    records: HashMap<String, Record>,
    /// Whether the file on disk was readable.
    ///
    /// When it was not, the map starts empty, reconciliation is suppressed and
    /// nothing is written back — a user who can still read the file by hand has
    /// lost nothing. See [`Library::load`].
    healthy: bool,
}

/// The shape on disk.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Persisted {
    /// Keyed by info hash.
    torrents: HashMap<String, Record>,
}

/// Failures while saving.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    /// The file could not be written.
    #[error("could not save the library record to {path}: {source}")]
    Save {
        /// The file Flume tried to write.
        path: String,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
}

/// Seconds since the epoch, or `None` if the clock is before it.
#[must_use]
pub fn now_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

impl Library {
    /// Loads the record from `dir`.
    ///
    /// # Corruption is not fatal, and is not fail-closed either
    ///
    /// Neither neighbouring precedent applies. librqbit's is fatal — a
    /// deserialize error of `session.json` becomes an engine start failure, and
    /// because the guard is the only thing that starts an engine, that means
    /// retry-and-fail once a second forever. [`crate::settings::Settings::load`]
    /// is fail-*closed*, forcing the egress guard to `Hold`; that protects
    /// against an unsafe network default, and nothing in a per-torrent record
    /// can hold transfer, so copying it here would be architecture rule 12
    /// applied somewhere it does not belong.
    ///
    /// So: load as empty, report the problem, and mark the library unhealthy —
    /// which suppresses reconciliation and refuses to overwrite the file.
    ///
    /// # Errors
    ///
    /// Never fails; the second tuple element describes any problem found.
    pub fn load(dir: &Path) -> (Self, Option<String>) {
        let path = dir.join(LIBRARY_FILE);

        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            // A first run has no file, which is not a problem and not
            // unhealthy — an empty library is exactly right.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (
                    Self {
                        records: HashMap::new(),
                        healthy: true,
                    },
                    None,
                );
            }
            Err(e) => {
                return (
                    Self::unhealthy(),
                    Some(format!("could not read {}: {e}", path.display())),
                );
            }
        };

        match serde_json::from_str::<Persisted>(&raw) {
            Ok(persisted) => (
                Self {
                    records: persisted.torrents,
                    healthy: true,
                },
                None,
            ),
            Err(e) => (
                Self::unhealthy(),
                Some(format!(
                    "the library record at {} was not valid JSON: {e}. \
                     What Flume remembers about each torrent — when it was \
                     added — is unavailable this session. The file has been \
                     left alone rather than overwritten.",
                    path.display()
                )),
            ),
        }
    }

    /// An empty library that will not write itself back.
    fn unhealthy() -> Self {
        Self {
            records: HashMap::new(),
            healthy: false,
        }
    }

    /// Whether the record loaded cleanly.
    ///
    /// Reconciliation and saving are both suppressed when it did not.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.healthy
    }

    /// How many torrents are on record.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether nothing is on record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The record for one torrent, if there is one.
    #[must_use]
    pub fn get(&self, info_hash: &str) -> Option<&Record> {
        self.records.get(&info_hash.to_ascii_lowercase())
    }

    /// Records a torrent's arrival, keeping any timestamp already held.
    ///
    /// Insert-if-absent, never an upsert. librqbit dedupes on add and returns
    /// `AlreadyManaged`, which [`crate::engine::Engine::confirm_add`] cannot
    /// distinguish from `Added` — so a write that set `added_at = now` on every
    /// successful add would reset the timestamp whenever someone re-adds a
    /// torrent they already have. That is the single most likely way to lose
    /// the one thing this record exists to hold, and `already_added` on the
    /// preview exists precisely because re-adding is a normal thing to do.
    ///
    /// Returns whether anything changed, so the caller can skip a write.
    pub fn note_added(&mut self, info_hash: &str, at: Option<u64>) -> bool {
        let key = info_hash.to_ascii_lowercase();
        match self.records.get(&key) {
            Some(existing) if existing.added_at.is_some() => false,
            Some(_) | None => {
                self.records.entry(key).or_default().added_at = at;
                true
            }
        }
    }

    /// Creates records for torrents that have none, and deletes nothing.
    ///
    /// `present` is every info hash known to exist — which must come from
    /// librqbit's *persisted* rows rather than the live session, because a
    /// torrent that failed to restore is absent from the session and present in
    /// the file, and while the egress guard holds there is no session at all.
    ///
    /// New records get `added_at: None`. A torrent that predates this feature
    /// has no discoverable arrival time and is rendered as absent rather than
    /// invented.
    ///
    /// Suppressed entirely when the record did not load cleanly: reconciling an
    /// empty in-memory map against a full session would look like a library of
    /// brand-new torrents, and writing that back would destroy the real file.
    ///
    /// Returns whether anything changed.
    pub fn reconcile<I, S>(&mut self, present: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if !self.healthy {
            return false;
        }
        let mut changed = false;
        for hash in present {
            let key = hash.as_ref().to_ascii_lowercase();
            if let std::collections::hash_map::Entry::Vacant(seat) = self.records.entry(key) {
                seat.insert(Record::default());
                changed = true;
            }
        }
        changed
    }

    /// Every known arrival time, keyed by info hash.
    ///
    /// Records with no timestamp are omitted rather than carried as `None`:
    /// the caller is building a lookup, and an absent key and a present `None`
    /// mean the same thing to it.
    #[must_use]
    pub fn added_times(&self) -> HashMap<String, u64> {
        self.records
            .iter()
            .filter_map(|(hash, record)| record.added_at.map(|at| (hash.clone(), at)))
            .collect()
    }

    /// Drops a record, for the one caller that knows a torrent was removed.
    ///
    /// The only deletion path. It exists because a *removal* is knowledge,
    /// where an absence is not — and the caller has to capture the info hash
    /// before asking librqbit to delete, since the only id-to-hash mapping
    /// lives in the session entry that the delete destroys.
    ///
    /// Returns whether anything changed.
    pub fn forget(&mut self, info_hash: &str) -> bool {
        self.records
            .remove(&info_hash.to_ascii_lowercase())
            .is_some()
    }

    /// Writes the record to `dir`, atomically.
    ///
    /// Temp file plus rename, which is a convention this repo does not
    /// otherwise have — [`crate::settings::Settings::save`] writes in place.
    /// It matters more here: settings are a handful of values a user can retype,
    /// while this is the only copy of when every torrent arrived, rewritten far
    /// more often.
    ///
    /// A no-op when the record did not load cleanly, so an unreadable file is
    /// never overwritten by an empty one.
    ///
    /// # Errors
    ///
    /// [`LibraryError::Save`] if the directory or file cannot be written.
    pub fn save(&self, dir: &Path) -> Result<(), LibraryError> {
        if !self.healthy {
            return Ok(());
        }

        std::fs::create_dir_all(dir).map_err(|source| LibraryError::Save {
            path: dir.display().to_string(),
            source,
        })?;

        let path = dir.join(LIBRARY_FILE);
        let temp = Self::temp_path(dir);

        let json = serde_json::to_string_pretty(&Persisted {
            torrents: self.records.clone(),
        })
        .map_err(|e| LibraryError::Save {
            path: path.display().to_string(),
            source: std::io::Error::other(e),
        })?;

        std::fs::write(&temp, json).map_err(|source| LibraryError::Save {
            path: temp.display().to_string(),
            source,
        })?;

        // Overwrites on every platform Flume targets: Rust's rename uses
        // MoveFileExW with MOVEFILE_REPLACE_EXISTING on Windows.
        std::fs::rename(&temp, &path).map_err(|source| {
            // A failed rename leaves the temp file behind; clearing it stops
            // the session directory accumulating one per failure.
            let _ = std::fs::remove_file(&temp);
            LibraryError::Save {
                path: path.display().to_string(),
                source,
            }
        })
    }

    /// Where the atomic write stages.
    fn temp_path(dir: &Path) -> PathBuf {
        dir.join(format!("{LIBRARY_FILE}.tmp"))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn loaded(dir: &Path) -> Library {
        Library::load(dir).0
    }

    #[test]
    fn round_trips_through_disk() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = Library::default_healthy();
        library.note_added(A, Some(1_700_000_000));
        library.save(tmp.path()).expect("save");

        let (reloaded, problem) = Library::load(tmp.path());

        assert!(problem.is_none());
        assert_eq!(
            reloaded.get(A).and_then(|r| r.added_at),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn a_first_run_is_empty_and_healthy() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let (library, problem) = Library::load(tmp.path());

        assert!(problem.is_none(), "no file is not a problem");
        assert!(library.is_healthy(), "and it is not corruption either");
        assert!(library.is_empty());
    }

    // --- the rule: never delete on absence --------------------------------

    #[test]
    fn reconcile_creates_records_and_never_removes_them() {
        // The constraint the whole module is shaped around. A torrent that
        // failed to restore -- an unmounted drive, a permissions problem -- is
        // absent this launch and present the next, and deleting its record
        // would be irreversible.
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());
        library.note_added(A, Some(42));

        // B appears, A does not.
        let changed = library.reconcile([B]);

        assert!(changed);
        assert_eq!(
            library.get(A).and_then(|r| r.added_at),
            Some(42),
            "A survives"
        );
        assert!(library.get(B).is_some(), "B is created");
        assert_eq!(
            library.get(B).and_then(|r| r.added_at),
            None,
            "and is not invented"
        );
    }

    #[test]
    fn reconcile_is_idempotent() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());

        assert!(library.reconcile([A, B]));
        assert!(!library.reconcile([A, B]), "a second pass changes nothing");
        assert_eq!(library.len(), 2);
    }

    #[test]
    fn an_empty_session_does_not_empty_the_record() {
        // While the egress guard holds there is no session at all, so the
        // reading is empty rather than stale.
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());
        library.note_added(A, Some(1));

        let changed = library.reconcile(Vec::<String>::new());

        assert!(!changed);
        assert_eq!(library.len(), 1, "an empty reading removes nothing");
    }

    #[test]
    fn forget_is_the_only_way_a_record_leaves() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());
        library.note_added(A, Some(1));

        assert!(library.forget(A));
        assert!(library.get(A).is_none());
        assert!(!library.forget(A), "forgetting twice changes nothing");
    }

    // --- insert-if-absent --------------------------------------------------

    #[test]
    fn a_re_add_does_not_reset_the_timestamp() {
        // `confirm_add` cannot tell `Added` from `AlreadyManaged`, so this is
        // the single most likely way to lose the data the record exists for.
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());

        assert!(library.note_added(A, Some(1_000)));
        assert!(!library.note_added(A, Some(9_999)), "nothing changed");
        assert_eq!(library.get(A).and_then(|r| r.added_at), Some(1_000));
    }

    #[test]
    fn a_reconciled_record_still_accepts_its_first_real_timestamp() {
        // Reconciliation creates `added_at: None`. A later add of that same
        // torrent is the first time an arrival time is actually known, and it
        // must be allowed to fill the seat.
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());
        library.reconcile([A]);

        assert!(library.note_added(A, Some(555)));
        assert_eq!(library.get(A).and_then(|r| r.added_at), Some(555));
    }

    #[test]
    fn hashes_match_whatever_case_they_arrive_in() {
        // librqbit serialises the hash upper-case in session.json while
        // `TorrentSummary::info_hash` is lower-case. A mismatch would make
        // every record look absent.
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());
        library.note_added(&A.to_ascii_uppercase(), Some(7));

        assert_eq!(library.get(A).and_then(|r| r.added_at), Some(7));
        assert!(
            !library.reconcile([A.to_ascii_uppercase()]),
            "not a second record"
        );
    }

    // --- corruption --------------------------------------------------------

    #[test]
    fn a_corrupt_file_loads_empty_reports_and_refuses_to_write_back() {
        // Not fatal like librqbit's session.json, and not fail-closed like
        // settings: a user who can still read the file by hand has lost
        // nothing, and overwriting it would end that.
        let tmp = tempfile::tempdir().expect("temp dir");
        std::fs::write(tmp.path().join(LIBRARY_FILE), "{ not json").expect("write");

        let (mut library, problem) = Library::load(tmp.path());

        assert!(!library.is_healthy());
        let problem = problem.expect("corruption is reported");
        assert!(
            problem.contains("left alone"),
            "the message says so: {problem}"
        );

        assert!(!library.reconcile([A]), "reconciliation is suppressed");
        library.note_added(A, Some(1));
        library
            .save(tmp.path())
            .expect("save is a no-op, not an error");

        assert_eq!(
            std::fs::read_to_string(tmp.path().join(LIBRARY_FILE)).expect("read"),
            "{ not json",
            "the original file is untouched"
        );
    }

    #[test]
    fn unknown_fields_survive_a_round_trip_of_an_older_build() {
        // Forward compatibility: a record written by a newer build must not
        // choke an older one.
        let tmp = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            tmp.path().join(LIBRARY_FILE),
            format!(r#"{{"torrents":{{"{A}":{{"addedAt":9,"label":"films"}}}}}}"#),
        )
        .expect("write");

        let (library, problem) = Library::load(tmp.path());

        assert!(problem.is_none());
        assert_eq!(library.get(A).and_then(|r| r.added_at), Some(9));
    }

    // --- the atomic write --------------------------------------------------

    #[test]
    fn saving_leaves_no_temp_file_behind() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());
        library.note_added(A, Some(1));
        library.save(tmp.path()).expect("save");

        assert!(!Library::temp_path(tmp.path()).exists());
        assert!(tmp.path().join(LIBRARY_FILE).exists());
    }

    #[test]
    fn a_save_over_an_existing_file_replaces_it() {
        // The rename has to overwrite, which is the part that differs between
        // platforms if it is done wrong.
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut library = loaded(tmp.path());
        library.note_added(A, Some(1));
        library.save(tmp.path()).expect("first save");

        library.note_added(B, Some(2));
        library.save(tmp.path()).expect("second save");

        let reloaded = loaded(tmp.path());
        assert_eq!(reloaded.len(), 2);
    }

    impl Library {
        /// An empty, healthy library, for tests that do not touch disk first.
        fn default_healthy() -> Self {
            Self {
                records: HashMap::new(),
                healthy: true,
            }
        }
    }
}
