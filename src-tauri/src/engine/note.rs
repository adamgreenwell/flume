//! The sentence that says what a torrent is actually doing.
//!
//! The design's third principle: never a bare adjective. "Stalled" is not a
//! status — "three peers answered; none had anything past 6%" is. Every state
//! carries its cause and the next move.
//!
//! Every branch here says something checkable from data Flume actually has.
//! Where the design's copy needs a fact the engine cannot reach — which piece
//! picker is running, how many peers hold the remainder, when the user paused —
//! the note says what is known and stops, rather than inventing the rest.
//! See `SwarmHealth` and issue #79 for the largest of those gaps.

use serde::{Deserialize, Serialize};

use super::detail::SwarmStats;
use super::torrent::{TorrentState, TorrentSummary};

/// How much attention a note wants.
///
/// Maps onto the status colours, which are reserved for exactly this. `Neutral`
/// is not an absence of severity — it is the deliberate statement that nothing
/// is wrong, which a paused torrent needs to make loudly enough that the user
/// does not think something broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteSeverity {
    /// Working as intended.
    Ok,
    /// Worth knowing; not yet broken.
    Warn,
    /// Stopped, or will not finish without intervention.
    Err,
    /// Deliberately inert. Nothing is wrong.
    Neutral,
}

/// A short headline and the paragraph under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// How much attention this wants.
    pub severity: NoteSeverity,
    /// The headline. A claim about this torrent, never a state word.
    pub title: String,
    /// Two or three sentences: what is happening, and what to do about it.
    pub body: String,
}

/// Formats a byte count the way the frontend does.
///
/// Decimal, three significant figures — see `formatBytes` in
/// `src/lib/format.ts`, which this must agree with. A note that says "4.87 GB"
/// beside a column that says "4.9 GB" reads as two different numbers.
fn bytes(value: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

    if value == 0 {
        return "0 B".to_string();
    }

    #[allow(clippy::cast_precision_loss)]
    let mut scaled = value as f64;
    let mut unit = 0;
    while scaled >= 1000.0 && unit < UNITS.len() - 1 {
        scaled /= 1000.0;
        unit += 1;
    }

    if unit == 0 {
        return format!("{value} B");
    }

    let decimals = if scaled < 10.0 {
        2
    } else if scaled < 100.0 {
        1
    } else {
        0
    };
    format!("{scaled:.decimals$} {}", UNITS[unit])
}

/// Formats a transfer rate the way the frontend does.
///
/// One decimal rather than [`bytes`]'s three significant figures, and MB/s is
/// held down to 0.1 — see `formatSpeed` in `src/lib/format.ts`, which this must
/// agree with. Sizes and rates read differently on purpose, so they cannot
/// share one formatter.
pub(super) fn rate(bytes_per_second: u64) -> String {
    if bytes_per_second == 0 {
        return "0 B/s".to_string();
    }

    #[allow(clippy::cast_precision_loss)]
    let value = bytes_per_second as f64;

    if bytes_per_second >= 100_000 {
        let mb = value / 1_000_000.0;
        let decimals = usize::from(mb < 100.0);
        return format!("{mb:.decimals$} MB/s");
    }

    if bytes_per_second >= 1_000 {
        let kb = value / 1_000.0;
        let decimals = usize::from(kb < 100.0);
        return format!("{kb:.decimals$} KB/s");
    }

    format!("{bytes_per_second} B/s")
}

/// Recognises the failures worth naming specifically.
///
/// Substring matching on an engine message is not something to be proud of,
/// but the alternative is showing every failure as "an error occurred", and
/// running out of disk is both the most common cause and the one with the
/// clearest fix. Anything unrecognised falls through to the raw message, which
/// is still more useful than a paraphrase.
fn explain_failure(message: &str) -> (String, String) {
    let lower = message.to_lowercase();

    if lower.contains("no space") || lower.contains("disk full") {
        return (
            "The disk this is saving to is full".to_string(),
            format!(
                "{message} Free some space, then press Resume — everything \
                 already verified is kept and will not be downloaded again."
            ),
        );
    }

    if lower.contains("permission denied") {
        return (
            "Flume is not allowed to write there".to_string(),
            format!(
                "{message} Check the folder's permissions, or move this \
                 torrent somewhere Flume can write, then press Resume."
            ),
        );
    }

    (
        "Stopped by a failure".to_string(),
        format!(
            "{message} Resuming keeps everything already verified on disk, so \
             nothing downloaded so far is lost."
        ),
    )
}

/// Writes the note for a torrent.
///
/// @param summary - The torrent's current summary.
/// @param swarm - Peer pool counts for the same torrent.
#[must_use]
pub fn describe(summary: &TorrentSummary, swarm: &SwarmStats) -> Note {
    let done = bytes(summary.progress_bytes);
    let remaining = bytes(summary.total_bytes.saturating_sub(summary.progress_bytes));
    let seen = swarm.seen;
    let live = swarm.live;

    match summary.state {
        TorrentState::Error => {
            let (title, body) = match summary.error.as_deref() {
                Some(message) => explain_failure(message),
                None => (
                    "Stopped by a failure".to_string(),
                    "The engine did not say why. Resuming keeps everything \
                     already verified on disk."
                        .to_string(),
                ),
            };
            Note {
                severity: NoteSeverity::Err,
                title,
                body,
            }
        }

        TorrentState::Checking => Note {
            severity: NoteSeverity::Neutral,
            title: "Checking what is already on disk".to_string(),
            body: format!(
                "Flume is re-hashing {} to find out what survived. Anything \
                 that verifies is kept; only pieces that fail are downloaded \
                 again.",
                bytes(summary.total_bytes)
            ),
        },

        TorrentState::Paused => Note {
            severity: NoteSeverity::Neutral,
            title: "Paused, nothing lost".to_string(),
            body: format!(
                "Your {done} is verified on disk. Resuming reconnects to the \
                 swarm and picks up from there — nothing is downloaded twice."
            ),
        },

        TorrentState::Seeding => {
            #[allow(clippy::cast_precision_loss)]
            let ratio = if summary.progress_bytes == 0 {
                0.0
            } else {
                summary.uploaded_bytes as f64 / summary.progress_bytes as f64
            };
            Note {
                severity: NoteSeverity::Ok,
                title: format!("Serving {live} of {seen} known peers"),
                body: format!(
                    "You have uploaded {} against the {done} you downloaded, a \
                     ratio of {ratio:.2}. Flume keeps seeding until you stop it.",
                    bytes(summary.uploaded_bytes)
                ),
            }
        }

        TorrentState::Downloading if live == 0 && seen == 0 => Note {
            severity: NoteSeverity::Warn,
            title: "No peers found yet".to_string(),
            body: "Flume is asking the DHT and this torrent's trackers for \
                   somewhere to start. A magnet link can take a minute the \
                   first time, because the file list has to be fetched from \
                   peers before anything can be downloaded."
                .to_string(),
        },

        TorrentState::Downloading if live == 0 => Note {
            severity: NoteSeverity::Err,
            title: "Nobody reachable has the rest of this".to_string(),
            body: format!(
                "{seen} peers are known for this torrent and none of them is \
                 answering right now. Flume keeps asking the DHT and the \
                 trackers every few minutes; {remaining} is still missing."
            ),
        },

        TorrentState::Downloading if summary.download_bps == 0 => Note {
            severity: NoteSeverity::Warn,
            title: format!("Connected to {live} peers, but nothing is arriving"),
            body: format!(
                "The connections are open and no data is coming down them. \
                 That usually means every connected peer is choking you — \
                 they have nothing you need, or they are busy serving someone \
                 else. {remaining} is still missing."
            ),
        },

        TorrentState::Downloading => {
            let eta = match summary.eta_seconds {
                Some(seconds) => format!(
                    " About {} left at this rate.",
                    super::torrent::format_duration(seconds)
                ),
                None => String::new(),
            };
            Note {
                severity: NoteSeverity::Ok,
                title: format!("Pulling from {live} of {seen} known peers"),
                body: format!(
                    "{done} verified so far, {remaining} to go, arriving at {}.{eta}",
                    rate(summary.download_bps)
                ),
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn summary(state: TorrentState) -> TorrentSummary {
        TorrentSummary {
            id: 0,
            info_hash: "abc".into(),
            name: "debian-13.2.0-amd64-DVD-1.iso".into(),
            state,
            progress_bytes: 19_700_000_000,
            total_bytes: 46_100_000_000,
            uploaded_bytes: 394_000_000,
            download_bps: 6_600_000,
            upload_bps: 900_000,
            live_peers: 41,
            known_peers: 206,
            health: super::super::torrent::SwarmHealth::Unknown,
            detail: String::new(),
            eta_seconds: Some(4020),
            finished: false,
            error: None,
            output_folder: "/tmp".into(),
        }
    }

    fn swarm(live: usize, seen: usize) -> SwarmStats {
        SwarmStats {
            live,
            connecting: 0,
            queued: 0,
            seen,
            dead: 0,
            live_tcp: live,
            live_utp: 0,
            // These notes are written from pool counts and rates; availability
            // is not one of their inputs.
            seeds: None,
            availability: None,
            rarest: None,
        }
    }

    #[test]
    fn rates_agree_with_the_frontend_formatter() {
        // Same assertions as `formatSpeed` in src/lib/format.ts. Rates take one
        // decimal, not three significant figures, and hold MB/s down to 0.1.
        assert_eq!(rate(6_600_000), "6.6 MB/s");
        assert_eq!(rate(900_000), "0.9 MB/s");
        assert_eq!(rate(99_000), "99.0 KB/s");
        assert_eq!(rate(512), "512 B/s");
        assert_eq!(rate(0), "0 B/s");
    }

    #[test]
    fn bytes_agree_with_the_frontend_formatter() {
        // Same assertions as `formatBytes` in src/lib/format.ts. A note saying
        // "4.87 GB" beside a column saying "4.9 GB" reads as two numbers.
        assert_eq!(bytes(46_100_000_000), "46.1 GB");
        assert_eq!(bytes(1_190_000_000), "1.19 GB");
        assert_eq!(bytes(231_000_000_000), "231 GB");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(0), "0 B");
    }

    #[test]
    fn no_title_is_a_bare_state_word() {
        // The row already draws the state. A note headed "Downloading" has
        // spent the loudest line in the panel on nothing.
        let states = [
            TorrentState::Downloading,
            TorrentState::Seeding,
            TorrentState::Paused,
            TorrentState::Checking,
            TorrentState::Error,
        ];

        for state in states {
            let note = describe(&summary(state), &swarm(41, 206));
            for bare in ["Downloading", "Seeding", "Paused", "Checking", "Error"] {
                assert_ne!(note.title, bare, "{state:?} produced a bare title");
            }
            assert!(!note.body.is_empty(), "{state:?} produced no body");
        }
    }

    #[test]
    fn the_three_kinds_of_stall_read_differently() {
        // Each needs a different response from the user, so none of them may
        // collapse into the same sentence.
        let mut stalled = summary(TorrentState::Downloading);
        stalled.download_bps = 0;
        stalled.eta_seconds = None;

        let nothing_known = describe(&stalled, &swarm(0, 0));
        let none_answering = describe(&stalled, &swarm(0, 12));
        let connected_idle = describe(&stalled, &swarm(6, 12));

        assert_eq!(nothing_known.severity, NoteSeverity::Warn);
        assert_eq!(none_answering.severity, NoteSeverity::Err);
        assert_eq!(connected_idle.severity, NoteSeverity::Warn);

        assert_ne!(nothing_known.title, none_answering.title);
        assert_ne!(none_answering.title, connected_idle.title);
        assert!(none_answering.body.contains("12"));
        assert!(connected_idle.title.contains('6'));
    }

    #[test]
    fn a_full_disk_is_named_and_given_a_fix() {
        let mut broken = summary(TorrentState::Error);
        broken.error = Some("Writing piece 489 failed: No space left on device.".into());

        let note = describe(&broken, &swarm(0, 0));

        assert_eq!(note.severity, NoteSeverity::Err);
        assert!(note.title.to_lowercase().contains("full"), "{}", note.title);
        // The remedy, and the reassurance that resuming is safe.
        assert!(note.body.contains("Free some space"));
        assert!(note.body.contains("Resume"));
        // The engine's own words are kept, not paraphrased away.
        assert!(note.body.contains("piece 489"));
    }

    #[test]
    fn an_unrecognised_failure_still_carries_the_engine_message() {
        let mut broken = summary(TorrentState::Error);
        broken.error = Some("something nobody anticipated".into());

        let note = describe(&broken, &swarm(0, 0));

        assert!(note.body.contains("something nobody anticipated"));
        assert_eq!(note.severity, NoteSeverity::Err);
    }

    #[test]
    fn a_failure_with_no_message_does_not_pretend_to_know_why() {
        let note = describe(&summary(TorrentState::Error), &swarm(0, 0));
        assert!(note.body.contains("did not say why"));
    }

    #[test]
    fn pausing_says_plainly_that_nothing_is_lost() {
        // The single most common worry a pause causes.
        let note = describe(&summary(TorrentState::Paused), &swarm(0, 0));

        assert_eq!(note.severity, NoteSeverity::Neutral);
        assert!(note.title.contains("nothing lost"));
        assert!(note.body.contains("19.7 GB"));
    }

    #[test]
    fn seeding_reports_the_ratio_it_actually_achieved() {
        let note = describe(&summary(TorrentState::Seeding), &swarm(9, 61));

        assert_eq!(note.severity, NoteSeverity::Ok);
        assert!(note.title.contains("9 of 61"), "{}", note.title);
        assert!(note.body.contains("0.02"), "{}", note.body);
    }

    #[test]
    fn a_healthy_download_reports_rate_remaining_and_eta() {
        let note = describe(&summary(TorrentState::Downloading), &swarm(41, 206));

        assert_eq!(note.severity, NoteSeverity::Ok);
        assert!(note.body.contains("6.6 MB/s"), "{}", note.body);
        assert!(note.body.contains("26.4 GB"), "{}", note.body);
        assert!(note.body.contains("1 h 07 min"), "{}", note.body);
    }

    #[test]
    fn a_download_with_no_eta_simply_omits_it() {
        let mut unknown = summary(TorrentState::Downloading);
        unknown.eta_seconds = None;

        let note = describe(&unknown, &swarm(41, 206));

        assert!(!note.body.contains("left at this rate"));
        assert!(note.body.contains("6.6 MB/s"));
    }
}
