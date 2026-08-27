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

/// A verdict on whether a torrent will actually finish.
///
/// Deliberately a different question from [`TorrentState`], which only says
/// whether work is happening. A torrent can be `Downloading` and never finish.
///
/// **`Thin` and `Healthy` are not derivable yet.** The design defines them by
/// piece availability — "every piece exists on three or more peers" versus
/// "most pieces exist on one" — which needs the union of the peers' bitfields.
/// librqbit 9.0.0 exposes our own bitfield (`api_dump_haves`) but no per-peer
/// one, so that number cannot be computed from the public API. Rather than
/// guess a verdict from peer counts and put a confident wrong answer in front
/// of the user, anything that would need availability reports [`Self::Unknown`]
/// and the UI states the peer counts instead. See issue #79.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SwarmHealth {
    /// Complete, and serving peers.
    Seeding,
    /// No reachable peer holds the remainder.
    None,
    /// Paused, checking, queued or errored — not trying, so not at risk.
    Idle,
    /// Connected to peers, but whether they hold the remainder is unknowable.
    Unknown,
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
    /// Peers this torrent has ever seen, connected or not.
    ///
    /// The denominator in the list's "24 / 118". A large gap between this and
    /// [`Self::live_peers`] is the difference between "the swarm is small" and
    /// "the swarm is big but nobody will talk to us", which are different
    /// problems with different fixes.
    pub known_peers: u32,
    /// Whether this torrent will actually finish. See [`SwarmHealth`].
    pub health: SwarmHealth,
    /// The one-line explanation shown under the name.
    ///
    /// Never a bare state word — the row already draws the state as an icon,
    /// so repeating it here would spend the only line that can say something
    /// useful on saying nothing.
    pub detail: String,
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
    /// First piece index covering this file.
    pub first_piece: u32,
    /// Piece index just past the last one covering this file.
    pub last_piece: u32,
    /// Downsampled completion across this file's own piece range, `0..=255`.
    ///
    /// Empty when piece state is unavailable — a torrent that is initializing
    /// or errored has no chunk tracker to read.
    ///
    /// This is what makes "which parts of this file do I actually have"
    /// answerable. Overall progress says 60%; this says *which* 60%, which is
    /// what matters when a download stalls against a partial swarm.
    pub piece_buckets: Vec<u8>,
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

/// Formats a duration the way the design writes them: `1 h 07 min`,
/// `2 min 30 s`, `45 s`.
///
/// **Must stay in step with `formatDuration` in `src/lib/format.ts`.** Two
/// formatters that disagree would put "2 min 30 s left" beside "2m 30s" in the
/// same row. The frontend one exists because most durations are formatted
/// there; this one exists because [`describe`] decides what the sentence says,
/// and that decision belongs in the engine.
pub(crate) fn format_duration(total_seconds: u64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours} h {minutes:02} min")
    } else if minutes > 0 {
        format!("{minutes} min {seconds:02} s")
    } else {
        format!("{seconds} s")
    }
}

/// Decides whether a torrent will actually finish.
///
/// Only returns a verdict it can defend. See [`SwarmHealth`] for why `Thin` and
/// `Healthy` are absent: separating them needs piece availability, which
/// librqbit 9.0.0 does not expose.
pub(super) fn classify_health(state: TorrentState, live_peers: u32) -> SwarmHealth {
    match state {
        TorrentState::Seeding => SwarmHealth::Seeding,
        TorrentState::Paused | TorrentState::Checking | TorrentState::Error => SwarmHealth::Idle,
        // Nobody to ask is the one negative verdict that needs no bitfield.
        TorrentState::Downloading if live_peers == 0 => SwarmHealth::None,
        TorrentState::Downloading => SwarmHealth::Unknown,
    }
}

/// Writes the one-line explanation shown under a torrent's name.
///
/// Never a bare state word. The row draws the state as an icon already, so this
/// line is the only place that can say *why* the state is what it is, and
/// spending it on "Downloading" wastes it.
///
/// Every branch says something checkable from the data at hand. Where the
/// design's copy needs a fact Flume does not track yet — how long ago the user
/// paused, which queue slot is blocking — the sentence states what is known
/// instead of inventing the rest.
pub(super) fn describe(
    state: TorrentState,
    eta_seconds: Option<u64>,
    live_peers: u32,
    known_peers: u32,
    progress_bytes: u64,
    uploaded_bytes: u64,
    error: Option<&str>,
) -> String {
    match state {
        // The raw engine message, not a paraphrase. A remedy sentence needs the
        // structured `Problem` from the API contract, which is later work.
        TorrentState::Error => match error {
            Some(message) => format!("stopped — {message}"),
            None => "stopped by a failure".to_string(),
        },
        TorrentState::Checking => "re-checking data already on disk".to_string(),
        TorrentState::Paused => "paused — everything downloaded is verified on disk".to_string(),
        TorrentState::Seeding => {
            #[allow(clippy::cast_precision_loss)]
            let ratio = if progress_bytes == 0 {
                0.0
            } else {
                uploaded_bytes as f64 / progress_bytes as f64
            };
            format!("seeding to {live_peers} of {known_peers} peers · ratio {ratio:.2}")
        }
        TorrentState::Downloading => match eta_seconds {
            Some(seconds) => format!("{} left", format_duration(seconds)),
            // No ETA means no rate to extrapolate from. Which of the two
            // reasons it is changes what the user should do, so say which.
            None if live_peers == 0 && known_peers == 0 => {
                "no peers found yet — asking the DHT and trackers".to_string()
            }
            None if live_peers == 0 => {
                format!("none of the {known_peers} known peers are answering")
            }
            None => format!("connected to {live_peers} peers, nothing arriving yet"),
        },
    }
}

/// Builds a [`TorrentSummary`] from librqbit's per-torrent stats.
pub(super) fn summarize(
    id: usize,
    info_hash: String,
    name: Option<String>,
    output_folder: String,
    stats: &TorrentStats,
) -> TorrentSummary {
    let (download_bps, upload_bps, live_peers, known_peers) = match stats.live.as_ref() {
        Some(live) => (
            live.download_speed.as_bytes(),
            live.upload_speed.as_bytes(),
            live.snapshot.peer_stats.live,
            // `seen` counts every peer this torrent has ever heard of, which is
            // the denominator the list shows. It only grows.
            live.snapshot.peer_stats.seen,
        ),
        // A paused or erroring torrent has no live stats; reporting zero is
        // accurate, and avoids the UI showing a stale rate forever.
        None => (0, 0, 0, 0),
    };

    let state = classify_state(&stats.state, stats.finished);
    let eta = eta_seconds(stats.progress_bytes, stats.total_bytes, download_bps);

    TorrentSummary {
        state,
        eta_seconds: eta,
        health: classify_health(state, live_peers),
        detail: describe(
            state,
            eta,
            live_peers,
            known_peers,
            stats.progress_bytes,
            stats.uploaded_bytes,
            stats.error.as_deref(),
        ),
        known_peers,
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
    fn durations_match_the_frontend_convention() {
        // These exact strings are what `formatDuration` in
        // `src/lib/format.ts` produces. If one side changes, both change.
        assert_eq!(format_duration(4020), "1 h 07 min");
        assert_eq!(format_duration(150), "2 min 30 s");
        assert_eq!(format_duration(45), "45 s");
        assert_eq!(format_duration(0), "0 s");
    }

    #[test]
    fn a_download_with_no_peers_is_the_one_negative_verdict_we_can_defend() {
        assert_eq!(
            classify_health(TorrentState::Downloading, 0),
            SwarmHealth::None
        );
    }

    #[test]
    fn a_download_with_peers_is_unknown_rather_than_a_guess() {
        // Separating "healthy" from "thin" needs piece availability, which
        // librqbit does not expose. Claiming either would be a confident wrong
        // answer, which the design is explicit is worse than none.
        assert_eq!(
            classify_health(TorrentState::Downloading, 24),
            SwarmHealth::Unknown
        );
    }

    #[test]
    fn nothing_that_is_not_trying_is_reported_as_at_risk() {
        for state in [
            TorrentState::Paused,
            TorrentState::Checking,
            TorrentState::Error,
        ] {
            assert_eq!(classify_health(state, 0), SwarmHealth::Idle);
        }
    }

    #[test]
    fn a_complete_torrent_is_seeding_regardless_of_peer_count() {
        assert_eq!(
            classify_health(TorrentState::Seeding, 0),
            SwarmHealth::Seeding
        );
    }

    #[test]
    fn the_detail_line_never_just_repeats_the_state() {
        // The row draws the state as an icon. A detail line that says
        // "Downloading" has spent the only useful line on nothing.
        let cases = [
            describe(TorrentState::Downloading, Some(150), 12, 44, 0, 0, None),
            describe(TorrentState::Downloading, None, 0, 0, 0, 0, None),
            describe(TorrentState::Downloading, None, 0, 3, 0, 0, None),
            describe(TorrentState::Downloading, None, 6, 11, 0, 0, None),
            describe(TorrentState::Seeding, None, 9, 61, 1_000, 4_820, None),
            describe(TorrentState::Paused, None, 0, 0, 0, 0, None),
            describe(TorrentState::Checking, None, 0, 0, 0, 0, None),
            describe(TorrentState::Error, None, 0, 0, 0, 0, Some("no space left")),
        ];

        for detail in &cases {
            assert!(!detail.is_empty(), "every state says something");
            for bare in ["Downloading", "Seeding", "Paused", "Checking", "Error"] {
                assert_ne!(detail.as_str(), bare, "{detail} is a bare state word");
            }
        }
    }

    #[test]
    fn a_stalled_download_says_which_kind_of_nothing_is_happening() {
        // The three reasons need three different responses from the user, so
        // they must not collapse into one sentence.
        let no_peers_known = describe(TorrentState::Downloading, None, 0, 0, 0, 0, None);
        let none_answering = describe(TorrentState::Downloading, None, 0, 3, 0, 0, None);
        let connected_idle = describe(TorrentState::Downloading, None, 6, 11, 0, 0, None);

        assert!(no_peers_known.contains("DHT"));
        assert!(none_answering.contains('3'));
        assert!(connected_idle.contains('6'));
        assert_ne!(no_peers_known, none_answering);
        assert_ne!(none_answering, connected_idle);
    }

    #[test]
    fn an_error_carries_the_engine_message_rather_than_a_paraphrase() {
        let detail = describe(
            TorrentState::Error,
            None,
            0,
            0,
            0,
            0,
            Some("/Volumes/Scratch has 0 B free"),
        );
        assert!(detail.contains("/Volumes/Scratch has 0 B free"));
    }

    #[test]
    fn seeding_reports_the_ratio_it_actually_achieved() {
        let detail = describe(TorrentState::Seeding, None, 9, 61, 1_000, 4_820, None);
        assert!(detail.contains("4.82"), "{detail}");
        assert!(detail.contains("9 of 61"), "{detail}");
    }

    #[test]
    fn a_ratio_with_nothing_downloaded_does_not_divide_by_zero() {
        let detail = describe(TorrentState::Seeding, None, 0, 0, 0, 500, None);
        assert!(detail.contains("0.00"), "{detail}");
    }

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
            known_peers: 0,
            health: SwarmHealth::Unknown,
            detail: String::new(),
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
