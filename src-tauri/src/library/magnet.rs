//! Magnets added before their file list arrived (#148).
//!
//! # Why Flume models these and librqbit does not
//!
//! librqbit v9 cannot hold a torrent whose metadata has not arrived:
//! `Session::add_torrent` resolves a magnet *before* the managed torrent
//! exists, whether or not `paused` is set. So "add it and let it resolve in
//! the list" has to be Flume's own state -- a [`PendingMagnet`] on the
//! library record, keyed by the info hash the magnet carries, retried by
//! Flume and turned into a real add only when the user has chosen files.
//!
//! # A resolved magnet holds; it never starts
//!
//! The add flow exists so nobody fetches gigabytes of the wrong thing, and a
//! magnet that resolves while nobody is watching is exactly that case. So the
//! file list is saved here, next to the library, and the torrent stays out of
//! librqbit until the user confirms a selection in the usual dialog. Saved
//! rather than held in the engine, because a guard hold or a settings change
//! discards the engine, and a hard-won file list is the one thing this
//! feature cannot afford to lose to a restart.
//!
//! Like the rest of [`crate::library`], nothing here imports Tauri.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};

/// Folder inside the app-data directory holding resolved file lists.
const MAGNET_DIR: &str = "magnets";

/// A magnet waiting for its file list, or waiting for the user to choose
/// files from it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PendingMagnet {
    /// The link as the user gave it, trackers and all, so a retry asks the
    /// same trackers the user's link named.
    ///
    /// Local only. It names the torrent, so by architecture rule 11 it never
    /// reaches a log line, a usage event or a diagnostics bundle.
    pub uri: String,
    /// The display name the link carried (`dn`), until the real one arrives.
    pub name: Option<String>,
    /// When the file list arrived, in seconds since the epoch.
    ///
    /// `Some` means the list is saved under [`metadata_path`] and the magnet
    /// is waiting on the user, not on the swarm.
    pub resolved_at: Option<u64>,
}

impl PendingMagnet {
    /// Whether the file list has arrived.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        self.resolved_at.is_some()
    }
}

/// Whether a pending magnet is waiting on the swarm or on the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PendingMagnetState {
    /// No peer has sent the file list yet. Flume keeps asking.
    Searching,
    /// The file list arrived and is saved. Nothing downloads until the user
    /// chooses files.
    Ready,
}

/// One pending magnet, as the list shows it. Mirrors the TypeScript
/// `PendingMagnetStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMagnetStatus {
    /// Lower-case hex info hash -- the key every other Flume record uses.
    pub info_hash: String,
    /// The torrent's name once known, else the link's `dn`, else `None`.
    pub name: Option<String>,
    /// When the magnet was first added, in seconds since the epoch.
    pub added_at: Option<u64>,
    /// Waiting on the swarm, or on the user.
    pub state: PendingMagnetState,
    /// Whether an attempt to fetch the file list is running right now.
    pub searching_now: bool,
    /// Attempts that went unanswered since Flume started.
    ///
    /// Per process rather than persisted: a count that survived restarts
    /// would keep growing on a magnet nobody seeds and say nothing a user
    /// could act on.
    pub failed_attempts: u32,
    /// Seconds until the next attempt, when one is scheduled.
    ///
    /// `None` while an attempt runs, once the list has arrived, and while
    /// there is no engine to ask -- a held guard included, where no attempt
    /// may run at all.
    pub next_attempt_in_seconds: Option<u64>,
}

/// Where a resolved magnet's file list is kept.
#[must_use]
pub fn metadata_path(dir: &Path, info_hash: &str) -> PathBuf {
    dir.join(MAGNET_DIR)
        .join(format!("{}.torrent", info_hash.to_ascii_lowercase()))
}

/// Saves a resolved file list, through a temp file and a rename.
///
/// # Errors
///
/// The underlying I/O failure.
pub fn save_metadata(dir: &Path, info_hash: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = metadata_path(dir, info_hash);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("torrent.tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// Reads a saved file list.
///
/// # Errors
///
/// The underlying I/O failure, including `NotFound` for none saved.
pub fn load_metadata(dir: &Path, info_hash: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(metadata_path(dir, info_hash))
}

/// Deletes a saved file list. A list that was never saved is not an error.
pub fn delete_metadata(dir: &Path, info_hash: &str) {
    match std::fs::remove_file(metadata_path(dir, info_hash)) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => log::warn!("could not delete a saved magnet file list: {err}"),
    }
}

/// How long to wait after `failures` unanswered attempts before the next.
///
/// The first retry is quick because the dialog's ten seconds are short, and
/// the wait then lengthens to a quarter of an hour and stays there. It never
/// gives up: a swarm with no seeder this morning can have one this evening,
/// and the user chose to keep this magnet rather than discard it. Each
/// attempt is a DHT lookup and a tracker announce, so a quarter-hour cap
/// keeps a long list of dead magnets from becoming background noise.
#[must_use]
pub fn retry_delay(failures: u32) -> Duration {
    const STEPS: [u64; 5] = [60, 120, 300, 600, 900];
    let index = usize::try_from(failures.saturating_sub(1)).unwrap_or(usize::MAX);
    Duration::from_secs(STEPS.get(index).copied().unwrap_or(900))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn retries_lengthen_and_then_hold_at_a_quarter_hour() {
        assert_eq!(retry_delay(1), Duration::from_secs(60));
        assert_eq!(retry_delay(2), Duration::from_secs(120));
        assert_eq!(retry_delay(3), Duration::from_secs(300));
        assert_eq!(retry_delay(5), Duration::from_secs(900));
        assert_eq!(retry_delay(500), Duration::from_secs(900));
        // Zero failures is not a state the resolver asks about, but it must
        // not panic or wait forever.
        assert_eq!(retry_delay(0), Duration::from_secs(60));
    }

    #[test]
    fn saved_metadata_round_trips_and_deletes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hash = "ABCDEF0123456789ABCDEF0123456789ABCDEF01";

        assert!(load_metadata(tmp.path(), hash).is_err());
        save_metadata(tmp.path(), hash, b"d4:infod4:name1:xee").unwrap();
        assert_eq!(
            load_metadata(tmp.path(), &hash.to_ascii_lowercase()).unwrap(),
            b"d4:infod4:name1:xee"
        );

        delete_metadata(tmp.path(), hash);
        assert!(load_metadata(tmp.path(), hash).is_err());
        // Deleting twice is quiet.
        delete_metadata(tmp.path(), hash);
    }
}
