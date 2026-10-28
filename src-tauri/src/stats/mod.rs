//! Lifetime transfer totals: what Flume has moved since it was installed.
//!
//! Like [`crate::engine`] and [`crate::library`], this module imports no Tauri
//! types and is testable under a plain `cargo test`.
//!
//! # Why a tally of differences, not a saved total plus the live counter
//!
//! The obvious design reads librqbit's session counters and adds them to a
//! total saved at launch. It is wrong twice over.
//!
//! - **The session is not the launch.** The egress guard stops the engine and
//!   builds a new one when it releases, and each new session counts from zero.
//!   "Saved total plus live counter" forgets everything the previous session
//!   moved the first time the tunnel drops.
//! - **Saving mid-session double-counts.** Write `saved + live` to disk, and the
//!   next save adds `live` to a total that already includes it. Totals that
//!   run ahead of reality undermine every number beside them.
//!
//! So each tick adds only what changed since the previous reading, and a
//! reading that went *down* is a new session, whose whole count is new. The
//! totals then only ever grow, persisting at any moment is safe, and an engine
//! restart costs at most the second between the last reading and the stop --
//! an undercount, which is the direction that does not mislead.
//!
//! # Not usage reporting
//!
//! Nothing here leaves the machine. The file sits beside `library.json` and
//! `settings.json`, and no usage event or diagnostics bundle reads it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{engine::TransferCounters, library::write_file_atomically};

/// Filename inside the app-data directory.
const STATS_FILE: &str = "stats.json";

/// Bytes each way and time open, over some span.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    /// Bytes received from peers.
    pub downloaded_bytes: u64,
    /// Bytes sent to peers.
    pub uploaded_bytes: u64,
    /// Seconds Flume was open and the machine awake.
    ///
    /// Counts while the egress guard holds, too: Flume is open then, it is
    /// just not transferring, and a ratio of bytes to time open should show
    /// that rather than hide it.
    pub active_seconds: u64,
}

impl Totals {
    fn add(&mut self, downloaded: u64, uploaded: u64, seconds: u64) {
        self.downloaded_bytes = self.downloaded_bytes.saturating_add(downloaded);
        self.uploaded_bytes = self.uploaded_bytes.saturating_add(uploaded);
        self.active_seconds = self.active_seconds.saturating_add(seconds);
    }
}

/// What the statistics view shows. Mirrored in `src/lib/ipc/types.ts`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    /// Since Flume was launched, across every engine restart in between.
    pub session: Totals,
    /// Since the statistics file was first written.
    pub all_time: Totals,
    /// How many times Flume has been launched, this one included.
    pub launches: u64,
}

/// What is written to disk.
///
/// Every field defaults, so a file written by a later version with a field
/// this one does not know still loads, and one written by an earlier version
/// missing a field reads it as zero.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
///
/// Laid out flat rather than flattening [`Totals`] into it: serde's `flatten`
/// does not carry this struct's `default` into the flattened one, so a file
/// missing any one byte count would fail to load.
struct Persisted {
    downloaded_bytes: u64,
    uploaded_bytes: u64,
    active_seconds: u64,
    launches: u64,
}

/// The running tally. One per process, owned by `AppState`.
#[derive(Debug)]
pub struct Tally {
    all_time: Totals,
    launches: u64,
    session: Totals,
    /// The previous reading, or `None` when no engine was running at the last
    /// tick. Cleared whenever the engine is absent, so the next session's
    /// first reading counts from zero.
    last: Option<TransferCounters>,
    /// Whether anything changed since the last save.
    dirty: bool,
    /// `false` when the file existed and could not be read. A tally that
    /// started from zero over a file it could not parse must not replace that
    /// file with its own smaller numbers.
    writable: bool,
}

impl Tally {
    /// Loads the totals from `dir` and counts this launch.
    ///
    /// Follows [`crate::library::Library::load`] on corruption: start from
    /// zero, report it, and leave the file alone. The totals are worth less
    /// than a library record, but they are just as unrecoverable once
    /// overwritten.
    ///
    /// # Errors
    ///
    /// Never fails; the second tuple element describes any problem found.
    pub fn load(dir: &Path) -> (Self, Option<String>) {
        let path = dir.join(STATS_FILE);
        let (persisted, problem) = match std::fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Persisted>(&raw) {
                Ok(persisted) => (Some(persisted), None),
                Err(e) => (
                    None,
                    Some(format!(
                        "the statistics file at {} was not valid JSON: {e}. \
                         All-time totals start from zero this session and \
                         the file has been left alone.",
                        path.display()
                    )),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                (Some(Persisted::default()), None)
            }
            Err(e) => (
                None,
                Some(format!("could not read {}: {e}", path.display())),
            ),
        };

        let writable = persisted.is_some();
        let persisted = persisted.unwrap_or_default();
        let tally = Self {
            all_time: Totals {
                downloaded_bytes: persisted.downloaded_bytes,
                uploaded_bytes: persisted.uploaded_bytes,
                active_seconds: persisted.active_seconds,
            },
            launches: persisted.launches.saturating_add(1),
            session: Totals::default(),
            last: None,
            // This launch is itself a change worth writing.
            dirty: true,
            writable,
        };
        (tally, problem)
    }

    /// Folds one tick into the totals.
    ///
    /// `reading` is the engine's counters, or `None` when no engine exists;
    /// `seconds` is how long the tick covered. Time is counted per tick rather
    /// than read from a clock, so a machine that sleeps for an hour adds the
    /// one tick it wakes into rather than the hour.
    pub fn tick(&mut self, reading: Option<TransferCounters>, seconds: u64) {
        let (downloaded, uploaded) = match reading {
            Some(now) => delta(self.last, now),
            None => (0, 0),
        };
        self.last = reading;

        self.session.add(downloaded, uploaded, seconds);
        self.all_time.add(downloaded, uploaded, seconds);
        if downloaded > 0 || uploaded > 0 || seconds > 0 {
            self.dirty = true;
        }
    }

    /// The figures the statistics view shows.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        Statistics {
            session: self.session,
            all_time: self.all_time,
            launches: self.launches,
        }
    }

    /// Writes the totals to `dir` if anything changed since the last write.
    ///
    /// A no-op when the file could not be read at launch, so an unreadable
    /// file is never replaced by a smaller one.
    ///
    /// # Errors
    ///
    /// A description of the write that failed. The tally stays dirty, so the
    /// next save tries again.
    pub fn save(&mut self, dir: &Path) -> Result<(), String> {
        if !self.dirty || !self.writable {
            return Ok(());
        }
        let persisted = Persisted {
            downloaded_bytes: self.all_time.downloaded_bytes,
            uploaded_bytes: self.all_time.uploaded_bytes,
            active_seconds: self.all_time.active_seconds,
            launches: self.launches,
        };
        let json = serde_json::to_string_pretty(&persisted).map_err(|e| e.to_string())?;
        write_file_atomically(dir, STATS_FILE, &json)
            .map_err(|(path, e)| format!("could not save statistics to {}: {e}", path.display()))?;
        self.dirty = false;
        Ok(())
    }
}

/// What moved between two readings, each way.
///
/// A reading lower than the last one in any field -- including uptime, which
/// catches a session that restarted and moved more than the old one between
/// two ticks -- is a different session, so all of it is new.
fn delta(last: Option<TransferCounters>, now: TransferCounters) -> (u64, u64) {
    match last {
        Some(last)
            if now.downloaded_bytes >= last.downloaded_bytes
                && now.uploaded_bytes >= last.uploaded_bytes
                && now.uptime_seconds >= last.uptime_seconds =>
        {
            (
                now.downloaded_bytes - last.downloaded_bytes,
                now.uploaded_bytes - last.uploaded_bytes,
            )
        }
        _ => (now.downloaded_bytes, now.uploaded_bytes),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn reading(downloaded: u64, uploaded: u64, uptime: u64) -> Option<TransferCounters> {
        Some(TransferCounters {
            downloaded_bytes: downloaded,
            uploaded_bytes: uploaded,
            uptime_seconds: uptime,
        })
    }

    fn fresh() -> Tally {
        let tmp = tempfile::tempdir().expect("temp dir");
        Tally::load(tmp.path()).0
    }

    #[test]
    fn a_first_launch_starts_at_zero_and_counts_itself() {
        let tally = fresh();
        let stats = tally.statistics();
        assert_eq!(stats.all_time, Totals::default());
        assert_eq!(stats.launches, 1);
    }

    #[test]
    fn each_tick_adds_only_what_changed() {
        let mut tally = fresh();
        tally.tick(reading(100, 10, 1), 1);
        tally.tick(reading(250, 15, 2), 1);
        tally.tick(reading(250, 15, 3), 1);

        let stats = tally.statistics();
        assert_eq!(stats.session.downloaded_bytes, 250);
        assert_eq!(stats.session.uploaded_bytes, 15);
        assert_eq!(stats.session.active_seconds, 3);
        assert_eq!(stats.all_time, stats.session);
    }

    // The case the whole design exists for: the guard stops the engine and a
    // new session starts counting from zero. Neither the bytes the first
    // session moved nor the second one's are lost.
    #[test]
    fn an_engine_restart_keeps_what_the_last_session_moved() {
        let mut tally = fresh();
        tally.tick(reading(1_000, 400, 60), 1);
        tally.tick(None, 1); // held: no engine
        tally.tick(reading(300, 50, 1), 1);

        let stats = tally.statistics();
        assert_eq!(stats.session.downloaded_bytes, 1_300);
        assert_eq!(stats.session.uploaded_bytes, 450);
        assert_eq!(stats.session.active_seconds, 3, "time counts while held");
    }

    // A stop and a start can both land between two ticks, so the telemetry
    // loop never sees the engine absent. The counters going down is what
    // gives it away.
    #[test]
    fn a_restart_between_two_ticks_is_still_a_new_session() {
        let mut tally = fresh();
        tally.tick(reading(1_000, 400, 60), 1);
        tally.tick(reading(300, 50, 1), 1);

        assert_eq!(tally.statistics().session.downloaded_bytes, 1_300);
        assert_eq!(tally.statistics().session.uploaded_bytes, 450);
    }

    // The same, where the new session has already moved *more* than the old
    // one had -- the bytes alone would read as an ordinary increase. Uptime
    // cannot go backwards within one session.
    #[test]
    fn a_new_session_is_recognised_by_its_uptime_alone() {
        let mut tally = fresh();
        tally.tick(reading(100, 10, 60), 1);
        tally.tick(reading(500, 20, 1), 1);

        assert_eq!(tally.statistics().session.downloaded_bytes, 600);
        assert_eq!(tally.statistics().session.uploaded_bytes, 30);
    }

    // Issue #65's acceptance test: totals never decrease, verified by
    // persisting mid-session.
    #[test]
    fn saving_mid_session_does_not_double_count() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let (mut tally, _) = Tally::load(tmp.path());
        tally.tick(reading(1_000, 100, 1), 1);
        tally.save(tmp.path()).expect("save");
        tally.tick(reading(1_500, 150, 2), 1);
        tally.save(tmp.path()).expect("save");

        let (reloaded, problem) = Tally::load(tmp.path());
        assert!(problem.is_none());
        let stats = reloaded.statistics();
        assert_eq!(stats.all_time.downloaded_bytes, 1_500);
        assert_eq!(stats.all_time.uploaded_bytes, 150);
        assert_eq!(stats.all_time.active_seconds, 2);
        assert_eq!(stats.launches, 2);
        assert_eq!(stats.session, Totals::default(), "a launch starts its own");
    }

    #[test]
    fn totals_carry_across_launches() {
        let tmp = tempfile::tempdir().expect("temp dir");
        for _ in 0..3 {
            let (mut tally, _) = Tally::load(tmp.path());
            tally.tick(reading(10, 1, 1), 1);
            tally.save(tmp.path()).expect("save");
        }

        let stats = Tally::load(tmp.path()).0.statistics();
        assert_eq!(stats.all_time.downloaded_bytes, 30);
        assert_eq!(stats.all_time.uploaded_bytes, 3);
        assert_eq!(stats.launches, 4);
    }

    #[test]
    fn an_unreadable_file_is_reported_and_never_overwritten() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let path = tmp.path().join(STATS_FILE);
        std::fs::write(&path, "{ not json").expect("write");

        let (mut tally, problem) = Tally::load(tmp.path());
        assert!(problem.is_some());
        tally.tick(reading(10, 1, 1), 1);
        tally
            .save(tmp.path())
            .expect("a refused save is not an error");

        assert_eq!(std::fs::read_to_string(&path).expect("read"), "{ not json");
    }

    #[test]
    fn a_file_missing_a_field_reads_it_as_zero() {
        let tmp = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            tmp.path().join(STATS_FILE),
            r#"{ "downloadedBytes": 42, "somethingNewer": true }"#,
        )
        .expect("write");

        let (tally, problem) = Tally::load(tmp.path());
        assert!(problem.is_none());
        let stats = tally.statistics();
        assert_eq!(stats.all_time.downloaded_bytes, 42);
        assert_eq!(stats.all_time.uploaded_bytes, 0);
        assert_eq!(stats.launches, 1);
    }

    #[test]
    fn an_unchanged_tally_is_not_rewritten() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let (mut tally, _) = Tally::load(tmp.path());
        tally.save(tmp.path()).expect("save");
        let path = tmp.path().join(STATS_FILE);
        std::fs::remove_file(&path).expect("remove");

        tally.tick(None, 0);
        tally.save(tmp.path()).expect("save");
        assert!(!path.exists(), "nothing changed, so nothing was written");
    }
}
