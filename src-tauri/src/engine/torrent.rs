//! Per-torrent state, as presented to the UI.
//!
//! Like [`super::status`], these are Flume's own types rather than re-exports
//! of librqbit's stats structs, so an engine upgrade cannot silently reshape
//! the IPC contract. Mirrored in `src/lib/ipc/types.ts`.

use librqbit::{TorrentStats, TorrentStatsState};
use serde::{Deserialize, Serialize};

/// User-facing lifecycle state of a torrent.
///
/// Deliberately coarser and more meaningful than librqbit's internal states:
/// the engine distinguishes "live" from "finished", but a user thinks in terms
/// of *downloading* versus *seeding*, which is the distinction rendered here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TorrentState {
    /// Hashing existing data or fetching metadata; no transfer yet.
    Checking,
    /// Actively downloading.
    Downloading,
    /// Complete and uploading to peers.
    Seeding,
    /// Stopped by the user.
    Paused,
    /// Stopped by a failure; see [`TorrentSummary::error`].
    Error,
}

/// A snapshot of one torrent, safe to send over IPC.
///
/// Contains no piece data — only counters, and the file *lengths* needed to
/// render a list row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentSummary {
    /// Session-local identifier. Stable while the app runs; not across restarts.
    pub id: usize,
    /// Hex-encoded info hash. Stable across restarts, unlike [`Self::id`].
    pub info_hash: String,
    /// Display name, or the info hash if metadata has not resolved yet.
    pub name: String,
    /// Coarse lifecycle state.
    pub state: TorrentState,
    /// Bytes downloaded and verified.
    pub progress_bytes: u64,
    /// Total size of the selected files, in bytes.
    pub total_bytes: u64,
    /// Bytes uploaded to peers this session.
    pub uploaded_bytes: u64,
    /// Current download rate in bytes per second; zero when not live.
    pub download_bps: u64,
    /// Current upload rate in bytes per second; zero when not live.
    pub upload_bps: u64,
    /// Peers currently connected to this torrent.
    pub live_peers: u32,
    /// Estimated seconds to completion, or `None` when it cannot be estimated.
    pub eta_seconds: Option<u64>,
    /// Whether all selected files are complete.
    pub finished: bool,
    /// Failure message when [`Self::state`] is [`TorrentState::Error`].
    pub error: Option<String>,
    /// Absolute directory the files are written to.
    pub output_folder: String,
}

/// One file inside an *added* torrent, with its download progress.
///
/// Distinct from [`super::TorrentFile`], which describes a torrent that has not
/// been added yet and so has no progress or selection state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFileState {
    /// Index within the torrent; what selection refers to.
    pub index: usize,
    /// Path relative to the torrent root.
    pub path: String,
    /// Total size in bytes.
    pub length: u64,
    /// Bytes downloaded and verified for this file.
    pub progress_bytes: u64,
    /// Whether this file is currently selected for download.
    pub selected: bool,
}

impl TorrentFileState {
    /// Fraction of this file that is complete, in `0.0..=1.0`.
    pub fn progress_fraction(&self) -> f64 {
        if self.length == 0 {
            return 1.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let fraction = self.progress_bytes as f64 / self.length as f64;
        fraction.clamp(0.0, 1.0)
    }
}

impl TorrentSummary {
    /// Fraction complete in `0.0..=1.0`.
    ///
    /// Returns `1.0` for a finished torrent even if `total_bytes` is zero,
    /// which happens briefly before metadata resolves.
    pub fn progress_fraction(&self) -> f64 {
        if self.finished {
            return 1.0;
        }
        if self.total_bytes == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let fraction = self.progress_bytes as f64 / self.total_bytes as f64;
        fraction.clamp(0.0, 1.0)
    }
}

/// Maps librqbit's lifecycle state onto Flume's.
///
/// `finished` promotes a live torrent from downloading to seeding, which is
/// the distinction the UI actually cares about.
pub(super) fn classify_state(state: &TorrentStatsState, finished: bool) -> TorrentState {
    match state {
        // A torrent can be "initializing" while paused; the user paused it, so
        // report that rather than implying work is happening.
        TorrentStatsState::Initializing { paused: true } => TorrentState::Paused,
        TorrentStatsState::Initializing { paused: false } => TorrentState::Checking,
        TorrentStatsState::Live if finished => TorrentState::Seeding,
        TorrentStatsState::Live => TorrentState::Downloading,
        TorrentStatsState::Paused => TorrentState::Paused,
        TorrentStatsState::Error => TorrentState::Error,
    }
}

/// Estimates seconds remaining from the current rate.
///
/// librqbit exposes an ETA, but only as a type whose inner `Duration` is
/// private — readable as a preformatted string or via `serde`, neither of
/// which suits a numeric IPC field. Computing it here is simple, testable, and
/// lets the frontend format durations consistently with everything else.
///
/// Returns `None` when there is nothing left to download or no rate to
/// extrapolate from, rather than reporting a misleading infinity.
pub(super) fn eta_seconds(progress_bytes: u64, total_bytes: u64, download_bps: u64) -> Option<u64> {
    let remaining = total_bytes.checked_sub(progress_bytes)?;
    if remaining == 0 || download_bps == 0 {
        return None;
    }
    Some(remaining / download_bps)
}

/// Builds a [`TorrentSummary`] from librqbit's per-torrent stats.
pub(super) fn summarize(
    id: usize,
    info_hash: String,
    name: Option<String>,
    output_folder: String,
    stats: &TorrentStats,
) -> TorrentSummary {
    let (download_bps, upload_bps, live_peers) = match stats.live.as_ref() {
        Some(live) => (
            live.download_speed.as_bytes(),
            live.upload_speed.as_bytes(),
            live.snapshot.peer_stats.live,
        ),
        // A paused or erroring torrent has no live stats; reporting zero is
        // accurate, and avoids the UI showing a stale rate forever.
        None => (0, 0, 0),
    };

    TorrentSummary {
        state: classify_state(&stats.state, stats.finished),
        eta_seconds: eta_seconds(stats.progress_bytes, stats.total_bytes, download_bps),
        // Falling back to the info hash keeps the row identifiable during the
        // window where a magnet link has not yet resolved its metadata.
        name: name.unwrap_or_else(|| info_hash.clone()),
        id,
        info_hash,
        progress_bytes: stats.progress_bytes,
        total_bytes: stats.total_bytes,
        uploaded_bytes: stats.uploaded_bytes,
        download_bps,
        upload_bps,
        live_peers,
        finished: stats.finished,
        error: stats.error.clone(),
        output_folder,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_while_initializing_reports_paused_not_checking() {
        assert_eq!(
            classify_state(&TorrentStatsState::Initializing { paused: true }, false),
            TorrentState::Paused
        );
    }

    #[test]
    fn initializing_reports_checking() {
        assert_eq!(
            classify_state(&TorrentStatsState::Initializing { paused: false }, false),
            TorrentState::Checking
        );
    }

    #[test]
    fn live_and_finished_is_seeding() {
        assert_eq!(
            classify_state(&TorrentStatsState::Live, true),
            TorrentState::Seeding
        );
    }

    #[test]
    fn live_and_unfinished_is_downloading() {
        assert_eq!(
            classify_state(&TorrentStatsState::Live, false),
            TorrentState::Downloading
        );
    }

    #[test]
    fn error_state_maps_through() {
        assert_eq!(
            classify_state(&TorrentStatsState::Error, false),
            TorrentState::Error
        );
    }

    #[test]
    fn eta_divides_remaining_by_rate() {
        assert_eq!(eta_seconds(0, 1000, 100), Some(10));
        assert_eq!(eta_seconds(500, 1000, 100), Some(5));
    }

    #[test]
    fn eta_is_none_with_no_rate() {
        assert_eq!(eta_seconds(0, 1000, 0), None);
    }

    #[test]
    fn eta_is_none_when_nothing_remains() {
        assert_eq!(eta_seconds(1000, 1000, 100), None);
    }

    #[test]
    fn eta_is_none_rather_than_panicking_when_progress_exceeds_total() {
        // Can happen transiently when the file selection shrinks mid-download.
        assert_eq!(eta_seconds(2000, 1000, 100), None);
    }

    fn summary(progress: u64, total: u64, finished: bool) -> TorrentSummary {
        TorrentSummary {
            id: 0,
            info_hash: "abc".into(),
            name: "t".into(),
            state: TorrentState::Downloading,
            progress_bytes: progress,
            total_bytes: total,
            uploaded_bytes: 0,
            download_bps: 0,
            upload_bps: 0,
            live_peers: 0,
            eta_seconds: None,
            finished,
            error: None,
            output_folder: "/tmp".into(),
        }
    }

    #[test]
    fn progress_fraction_is_a_ratio() {
        assert!((summary(500, 1000, false).progress_fraction() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_fraction_handles_unknown_total() {
        assert_eq!(summary(0, 0, false).progress_fraction(), 0.0);
    }

    #[test]
    fn finished_is_always_complete_even_before_metadata() {
        assert_eq!(summary(0, 0, true).progress_fraction(), 1.0);
    }

    #[test]
    fn progress_fraction_never_exceeds_one() {
        assert_eq!(summary(2000, 1000, false).progress_fraction(), 1.0);
    }
}
