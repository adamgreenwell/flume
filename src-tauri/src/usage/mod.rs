//! Opt-in, anonymous usage counts.
//!
//! Not to be confused with [`crate::telemetry`], which is the 1 Hz push of
//! torrent status to the webview and never leaves the process. This module is
//! the only thing in Flume that sends anything to a server, and it sends
//! nothing at all unless the user has said yes.
//!
//! Like [`crate::settings`], it imports no Tauri types.
//!
//! # The schema is closed on purpose
//!
//! Every field of every event is an enum or a bool. There is no free-text
//! field anywhere, and that is a hard requirement rather than a style
//! preference: librqbit's error strings embed tracker URLs and filesystem
//! paths, so a `String` reason field would exfiltrate exactly the things
//! Flume promises not to collect. [`FailureKind`] mirrors the `kind`
//! vocabulary `crate::commands::CommandError` already uses — a set that is
//! closed, stable, and the thing the frontend already branches on.
//!
//! Values arriving from the frontend are parsed into these enums and
//! **dropped if unrecognised**, so a future UI change cannot widen what is
//! collected without a matching change here.
//!
//! # What identifies an install
//!
//! A v4 UUID generated on this machine when consent is granted, and deleted
//! when it is withdrawn. Never a machine ID, MAC address or hardware serial:
//! those are three different problems across the three platforms
//! (`/etc/machine-id` is frequently absent in containers and identical across
//! cloned VMs), and they are linkable across applications, which is precisely
//! the property a BitTorrent client must not have.
//!
//! Timestamps are truncated to the hour. Exact ones let batches be correlated
//! into a session timeline for no analytical gain.

pub mod sender;

use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::settings::Settings;

/// Wire format version. Bump when an event's shape changes incompatibly.
pub const SCHEMA_VERSION: u32 = 1;

/// Queue filename, beside `settings.json`.
const QUEUE_FILE: &str = "usage-queue.jsonl";

/// Install identifier filename, beside `settings.json`.
const INSTALL_ID_FILE: &str = "install-id";

/// How old an event may be and still be worth sending.
///
/// Deliberately below the collector's own four-day window
/// (`MAX_AGE_SECONDS` in `collector/src/index.ts`), so the client discards a
/// stale event rather than posting one the collector will reject. Without the
/// margin a queue that outlives a long app closure is refused with a 400
/// forever, which reads as a broken build rather than as a holiday.
pub const MAX_EVENT_AGE: Duration = Duration::from_secs(3 * 24 * 3_600);

/// Most events held before the oldest are dropped.
///
/// A queue that grows without bound on a machine that is offline for a month
/// is a disk-space bug, and old counts are the least valuable ones.
pub const MAX_QUEUED_EVENTS: usize = 5_000;

/// Rounds a timestamp down to the hour.
const HOUR: u64 = 3_600;

/// Sentinel for "the queue length is not known yet".
///
/// The length is tracked in memory so an append does not have to read the
/// whole file back to find out whether it is time to trim — that turns
/// recording into an O(n²) walk over a long backlog. It starts unknown
/// because a queue can be left on disk by a previous run.
const UNKNOWN_LENGTH: usize = usize::MAX;

/// How long a session ran, as a bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DurationBucket {
    /// Under five minutes.
    #[serde(rename = "<5m")]
    UnderFiveMinutes,
    /// Five to thirty minutes.
    #[serde(rename = "5-30m")]
    UnderHalfHour,
    /// Thirty minutes to two hours.
    #[serde(rename = "30m-2h")]
    UnderTwoHours,
    /// Two to eight hours.
    #[serde(rename = "2-8h")]
    UnderEightHours,
    /// Over eight hours.
    #[serde(rename = "8h+")]
    AllDay,
}

impl DurationBucket {
    /// Buckets a session length.
    #[must_use]
    pub fn of(duration: Duration) -> Self {
        match duration.as_secs() {
            0..300 => Self::UnderFiveMinutes,
            300..1_800 => Self::UnderHalfHour,
            1_800..7_200 => Self::UnderTwoHours,
            7_200..28_800 => Self::UnderEightHours,
            _ => Self::AllDay,
        }
    }
}

/// How large a library is, as a bucket.
///
/// Bucketed rather than exact because a bucket is what anyone would graph, and
/// an exact count is far more identifying — "the install with 1 483 torrents"
/// is one person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CountBucket {
    /// No torrents.
    #[serde(rename = "0")]
    None,
    /// One to five.
    #[serde(rename = "1-5")]
    Few,
    /// Six to twenty.
    #[serde(rename = "6-20")]
    Some,
    /// Twenty-one to a hundred.
    #[serde(rename = "21-100")]
    Many,
    /// More than a hundred.
    #[serde(rename = "100+")]
    Lots,
}

impl CountBucket {
    /// Buckets a library size.
    #[must_use]
    pub fn of(count: usize) -> Self {
        match count {
            0 => Self::None,
            1..=5 => Self::Few,
            6..=20 => Self::Some,
            21..=100 => Self::Many,
            _ => Self::Lots,
        }
    }
}

/// Which add route a torrent came in through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddSource {
    /// A magnet link, typed or opened from the OS.
    Magnet,
    /// A `.torrent` file.
    File,
}

/// Which setting was changed.
///
/// The key only — never the value. Knowing that people change the proxy
/// setting is useful; knowing which proxy they use is not Flume's business.
/// The variants mirror `SettingDef.key` in `src/lib/settings/defs.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingKey {
    /// Download rate limit.
    #[serde(rename = "speed.download")]
    SpeedDownload,
    /// Upload rate limit.
    #[serde(rename = "speed.upload")]
    SpeedUpload,
    /// Download directory.
    #[serde(rename = "files.downloadDir")]
    FilesDownloadDir,
    /// DHT on or off.
    #[serde(rename = "net.dht")]
    NetDht,
    /// Listen port.
    #[serde(rename = "net.listenPort")]
    NetListenPort,
    /// UPnP on or off.
    #[serde(rename = "net.upnp")]
    NetUpnp,
    /// Proxy configured or cleared.
    #[serde(rename = "net.proxy")]
    NetProxy,
    /// Colour scheme.
    #[serde(rename = "ui.theme")]
    UiTheme,
    /// Row density.
    #[serde(rename = "ui.density")]
    UiDensity,
    /// Usage reporting itself.
    #[serde(rename = "privacy.usage")]
    PrivacyUsage,
}

impl SettingKey {
    /// Which settings differ between two versions.
    ///
    /// Derived in Rust rather than reported by the UI: `update_settings` is
    /// the one place that sees both the old and the new values, so a change
    /// made anywhere — the settings screen, the first-run flow, a future
    /// keyboard shortcut — is counted the same way and none can be missed.
    ///
    /// The values themselves never leave. Which setting someone reached for is
    /// the useful signal; what they set it to is their business.
    #[must_use]
    pub fn changed(previous: &Settings, next: &Settings) -> Vec<Self> {
        let mut keys = Vec::new();
        let mut note = |changed: bool, key: Self| {
            if changed {
                keys.push(key);
            }
        };

        note(
            previous.download_limit_bps != next.download_limit_bps,
            Self::SpeedDownload,
        );
        note(
            previous.upload_limit_bps != next.upload_limit_bps,
            Self::SpeedUpload,
        );
        note(
            previous.download_dir != next.download_dir,
            Self::FilesDownloadDir,
        );
        note(previous.enable_dht != next.enable_dht, Self::NetDht);
        note(
            previous.listen_port != next.listen_port,
            Self::NetListenPort,
        );
        note(previous.enable_upnp != next.enable_upnp, Self::NetUpnp);
        note(previous.proxy_url != next.proxy_url, Self::NetProxy);
        note(previous.theme != next.theme, Self::UiTheme);
        note(previous.density != next.density, Self::UiDensity);
        note(
            previous.usage_reporting != next.usage_reporting,
            Self::PrivacyUsage,
        );

        keys
    }
}

/// Which class of operation failed.
///
/// Mirrors the `kind` values of `crate::commands::CommandError`. Reusing that
/// vocabulary means error reporting needs no new set of strings and, more
/// importantly, cannot carry a formatted message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    /// The magnet URI would not parse.
    InvalidMagnet,
    /// Metadata could not be resolved.
    Metadata,
    /// Metadata resolution timed out with no peer answering.
    MetadataTimeout,
    /// A preview was confirmed with none pending.
    NoPendingPreview,
    /// The torrent id was not known to the session.
    UnknownTorrent,
    /// The session or its directories failed.
    EngineFailed,
    /// A torrent operation failed.
    OperationFailed,
    /// Settings could not be written.
    SettingsSaveFailed,
    /// Settings failed validation.
    SettingsInvalid,
    /// The engine had not started yet.
    EngineNotReady,
}

impl FailureKind {
    /// Parses a `CommandError` kind, returning `None` if it is not known.
    ///
    /// Unknown values are dropped rather than passed through. That is the
    /// point: the set of things Flume reports cannot widen by accident.
    #[must_use]
    pub fn parse(kind: &str) -> Option<Self> {
        Some(match kind {
            "invalidMagnet" => Self::InvalidMagnet,
            "metadata" => Self::Metadata,
            "metadataTimeout" => Self::MetadataTimeout,
            "noPendingPreview" => Self::NoPendingPreview,
            "unknownTorrent" => Self::UnknownTorrent,
            "engineFailed" => Self::EngineFailed,
            "operationFailed" => Self::OperationFailed,
            "settingsSaveFailed" => Self::SettingsSaveFailed,
            "settingsInvalid" => Self::SettingsInvalid,
            "engineNotReady" => Self::EngineNotReady,
            _ => return None,
        })
    }
}

/// What happened. Every field is an enum or a bool; none is free text.
///
/// `rename_all_fields` is not optional here. `rename_all` renames the
/// *variants* only, so without it a struct variant's fields go out as
/// `duration_bucket` while the collector's allowlist expects
/// `durationBucket` — and every batch carrying one is rejected with a 400 by a
/// client that otherwise looks fine. `tests/usage_contract.rs` pins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EventKind {
    /// The app started.
    Launched,
    /// The app is shutting down.
    SessionEnded {
        /// How long it ran.
        duration_bucket: DurationBucket,
    },
    /// How many torrents were in the library at launch.
    LibraryCount {
        /// The bucketed count.
        bucket: CountBucket,
    },
    /// A torrent's metadata was resolved and its file list shown.
    ///
    /// Separate from [`Self::TorrentAdded`] because the engine does not retain
    /// the source past the preview — `pending` is keyed by info hash and holds
    /// bytes, nothing more. Recording the source here rather than widening
    /// `confirm_add` to carry it also measures the preview-to-add conversion
    /// rate, which is the number that says whether the select-files-first flow
    /// is working.
    TorrentPreviewed {
        /// Which route it came in through.
        source: AddSource,
    },
    /// A previewed torrent was actually started.
    TorrentAdded,
    /// A torrent finished downloading.
    TorrentCompleted,
    /// A torrent was removed.
    TorrentRemoved {
        /// Whether its files were deleted too.
        deleted_data: bool,
    },
    /// Another client's library was imported.
    ///
    /// Which client it came from would be more useful, but `import_client`
    /// takes a directory rather than a [`ClientKind`] and `ImportOutcome` does
    /// not carry one. Widening a command signature for the benefit of
    /// reporting is the wrong trade; the size of the import comes free.
    LibraryImported {
        /// How many torrents were taken over.
        added: CountBucket,
    },
    /// A setting was changed.
    SettingChanged {
        /// Which setting. Never its value.
        key: SettingKey,
    },
    /// An operation failed.
    OperationFailed {
        /// Which class of failure.
        kind: FailureKind,
    },
}

/// One event, with the hour it happened in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Unix seconds, truncated to the hour.
    pub at: u64,
    /// What happened.
    #[serde(flatten)]
    pub kind: EventKind,
}

impl Event {
    /// Stamps an event with the current hour.
    #[must_use]
    pub fn now(kind: EventKind) -> Self {
        Self {
            at: truncate_to_hour(unix_now()),
            kind,
        }
    }
}

/// Current Unix time in seconds, or 0 if the clock is before the epoch.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Rounds down to the containing hour.
const fn truncate_to_hour(secs: u64) -> u64 {
    secs - (secs % HOUR)
}

/// One batch, as POSTed to the collector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// Wire format version.
    pub schema: u32,
    /// This install's random identifier.
    pub install_id: String,
    /// Flume's version.
    pub app_version: String,
    /// OS family, from `std::env::consts::OS`.
    pub os: String,
    /// Architecture, from `std::env::consts::ARCH`.
    pub arch: String,
    /// The events in this batch.
    pub events: Vec<Event>,
}

/// What happened on the most recent send attempt.
///
/// In memory and per-session on purpose. A durable "it has been broken for N
/// days" clock was designed and rejected: on an always-on seeding box any such
/// clock trips for a household DNS blocklist, a firewall rule someone clicked
/// Deny on months ago, or a VPN kill switch — none of which are defects, and
/// all of which would produce a confident, wrong verdict in a report the user
/// is asked to read and paste. This records what happened, not what it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Delivery {
    /// Nothing has been attempted yet this session.
    #[default]
    Untried,
    /// The collector accepted the batch.
    Accepted,
    /// The request never got an answer.
    ///
    /// One variant rather than several because the causes are genuinely not
    /// distinguishable: `hyper-util` tags every connector failure the same
    /// way, so a closed laptop, a typo in the host, an expired certificate and
    /// a blocking proxy all arrive here identically. Claiming to know which
    /// would be a verdict the data cannot support.
    NoResponse,
    /// The collector answered and refused, with this status.
    Refused(u16),
}

impl Delivery {
    /// Whether a refusal will still be a refusal on the next identical try.
    ///
    /// A 404 means the endpoint is wrong; a 413 means the batch is too large
    /// for the collector's cap. Neither improves by waiting, and both are
    /// answers from a server — which a closed laptop, a plane or a captive
    /// portal can never produce, so this cannot fire for being offline.
    ///
    /// 400 is deliberately excluded. The collector returns it for a clock more
    /// than two hours fast and for events past its age window as well as for a
    /// schema mismatch, and the first two are ordinary user situations.
    #[must_use]
    pub const fn is_settled_refusal(self) -> bool {
        matches!(self, Self::Refused(404 | 413))
    }
}

/// Queues events and decides whether any are collected at all.
///
/// Every method is infallible from the caller's point of view: recording is a
/// side errand on the way to doing something the user asked for, and must
/// never turn a working action into an error. Failures are logged.
pub struct Recorder {
    /// Directory holding the queue and the install id.
    dir: PathBuf,
    /// Whether consent is currently `Some(true)`.
    enabled: AtomicBool,
    /// Serialises appends and rewrites.
    lock: Mutex<()>,
    /// How many events are queued, or [`UNKNOWN_LENGTH`] before the first read.
    queued: AtomicUsize,
    /// The most recent send outcome. Session-scoped; never written to disk.
    delivery: Mutex<Delivery>,
    /// Flume's version, for the envelope.
    app_version: String,
}

impl Recorder {
    /// Builds a recorder for `dir`, with the consent value from settings.
    #[must_use]
    pub fn new(dir: PathBuf, consent: Option<bool>, app_version: String) -> Self {
        Self {
            dir,
            enabled: AtomicBool::new(consent == Some(true)),
            lock: Mutex::new(()),
            queued: AtomicUsize::new(UNKNOWN_LENGTH),
            delivery: Mutex::new(Delivery::Untried),
            app_version,
        }
    }

    /// Whether anything is being collected.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Applies a new consent value.
    ///
    /// Withdrawing deletes the install id *and* the queue: "stop collecting"
    /// that left a stable identifier and an unsent backlog behind would be a
    /// lie.
    ///
    /// Granting deliberately creates nothing. The install id is written
    /// lazily, on the first batch that is actually sent, so consent followed
    /// by a session that records nothing leaves no trace on disk at all.
    pub fn set_consent(&self, consent: Option<bool>) {
        let granted = consent == Some(true);
        self.enabled.store(granted, Ordering::Relaxed);

        if !granted {
            let _guard = self.lock.lock();
            self.queued.store(0, Ordering::Relaxed);
            self.set_delivery(Delivery::Untried);
            for file in [INSTALL_ID_FILE, QUEUE_FILE] {
                let path = self.dir.join(file);
                if let Err(err) = remove_if_present(&path) {
                    log::warn!("could not remove {}: {err}", path.display());
                }
            }
        }
    }

    /// Records one event, or does nothing at all if consent was not given.
    pub fn record(&self, kind: EventKind) {
        if !self.is_enabled() {
            return;
        }
        if let Err(err) = self.append(Event::now(kind)) {
            log::debug!("could not queue a usage event: {err}");
        }
    }

    /// Appends to the queue, trimming it if it has grown past the cap.
    fn append(&self, event: Event) -> std::io::Result<()> {
        let Ok(_guard) = self.lock.lock() else {
            // A poisoned lock means another thread panicked mid-write. The
            // queue may be torn; dropping this event is the right cost.
            return Ok(());
        };

        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(QUEUE_FILE);
        let line = serde_json::to_string(&event)?;

        // Only costs a read when a previous run left a queue behind.
        let mut queued = match self.queued.load(Ordering::Relaxed) {
            UNKNOWN_LENGTH => read_queue(&path)?.len(),
            known => known,
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{line}")?;
        drop(file);
        queued += 1;

        // Trimming rewrites the file, so it has to be rare. Checking a counter
        // rather than re-reading makes an append O(1) amortised; trimming to
        // the cap means the next rewrite is `MAX_QUEUED_EVENTS` appends away.
        if queued > MAX_QUEUED_EVENTS {
            let events = read_queue(&path)?;
            let keep = &events[events.len().saturating_sub(MAX_QUEUED_EVENTS)..];
            write_queue(&path, keep)?;
            queued = keep.len();
        }

        self.queued.store(queued, Ordering::Relaxed);
        Ok(())
    }

    /// Takes everything queued, leaving the queue empty.
    ///
    /// Returns `None` when there is nothing to send, consent is absent, or no
    /// install id could be established.
    pub fn take_batch(&self) -> Option<Envelope> {
        if !self.is_enabled() {
            return None;
        }
        let Ok(_guard) = self.lock.lock() else {
            return None;
        };

        let path = self.dir.join(QUEUE_FILE);
        let events = match read_queue(&path) {
            Ok(events) => events,
            Err(err) => {
                log::debug!("could not read the usage queue: {err}");
                return None;
            }
        };

        // Pruned on the way out, not only on the way back in after a failed
        // send. A queue that outlives a week-long app closure would otherwise
        // be posted verbatim and refused by the collector's own age window --
        // a 400 that repeats every hour and looks exactly like a broken build
        // rather than like a holiday.
        let events = prune_expired(events);
        if events.is_empty() {
            // Everything aged out. The file still has to go, or the same dead
            // events are re-read and re-pruned on every flush forever.
            if let Err(err) = remove_if_present(&path) {
                log::debug!("could not clear a fully-expired usage queue: {err}");
            }
            self.queued.store(0, Ordering::Relaxed);
            return None;
        }

        let install_id = match self.install_id() {
            Ok(id) => id,
            Err(err) => {
                log::warn!("could not read the usage install id: {err}");
                return None;
            }
        };

        if let Err(err) = remove_if_present(&path) {
            // Leaving the file would resend this batch on the next flush. The
            // collector dedupes, but not resending is better than relying on it.
            log::warn!("could not clear the usage queue: {err}");
            return None;
        }
        self.queued.store(0, Ordering::Relaxed);

        Some(Envelope {
            schema: SCHEMA_VERSION,
            install_id,
            app_version: self.app_version.clone(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            events,
        })
    }

    /// Puts a batch back after a failed send.
    ///
    /// Prepended, so the queue stays in order and a retry does not reorder
    /// events relative to ones recorded while the send was in flight.
    pub fn restore(&self, envelope: &Envelope) {
        let Ok(_guard) = self.lock.lock() else {
            return;
        };
        let path = self.dir.join(QUEUE_FILE);
        let mut events = envelope.events.clone();
        match read_queue(&path) {
            Ok(current) => events.extend(current),
            Err(err) => log::debug!("could not read the usage queue while restoring: {err}"),
        }
        if events.len() > MAX_QUEUED_EVENTS {
            let start = events.len() - MAX_QUEUED_EVENTS;
            events.drain(..start);
        }
        match write_queue(&path, &events) {
            Ok(()) => self.queued.store(events.len(), Ordering::Relaxed),
            Err(err) => {
                log::warn!("could not restore the usage queue: {err}");
                self.queued.store(UNKNOWN_LENGTH, Ordering::Relaxed);
            }
        }
    }

    /// Reads the install id, creating one if this is the first time.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error if the file cannot be read or written.
    pub fn install_id(&self) -> std::io::Result<String> {
        let path = self.dir.join(INSTALL_ID_FILE);
        if let Ok(existing) = std::fs::read_to_string(&path) {
            let existing = existing.trim();
            if !existing.is_empty() {
                return Ok(existing.to_owned());
            }
        }
        std::fs::create_dir_all(&self.dir)?;
        let id = uuid::Uuid::new_v4().to_string();
        std::fs::write(&path, &id)?;
        Ok(id)
    }

    /// Records the outcome of a send attempt.
    pub fn set_delivery(&self, outcome: Delivery) {
        if let Ok(mut delivery) = self.delivery.lock() {
            *delivery = outcome;
        }
    }

    /// The outcome of the most recent send attempt this session.
    #[must_use]
    pub fn delivery(&self) -> Delivery {
        self.delivery
            .lock()
            .map_or(Delivery::Untried, |delivery| *delivery)
    }

    /// Whether an install id exists on disk.
    #[must_use]
    pub fn has_install_id(&self) -> bool {
        self.dir.join(INSTALL_ID_FILE).exists()
    }
}

/// Drops events too old to be worth sending.
///
/// Shared by the queue's read path and the sender's restore path so both use
/// one definition of "too old"; a sender that pruned differently from the
/// queue would restore events the queue would immediately discard.
#[must_use]
pub fn prune_expired(mut events: Vec<Event>) -> Vec<Event> {
    let cutoff = unix_now().saturating_sub(MAX_EVENT_AGE.as_secs());
    events.retain(|event| event.at >= cutoff);
    events
}

/// Deletes a file, treating "already gone" as success.
fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Reads every event, skipping lines that will not parse.
///
/// A torn line — from a crash mid-append — costs that event, not the queue.
fn read_queue(path: &Path) -> std::io::Result<Vec<Event>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Event>(&line).ok())
        .collect())
}

/// Rewrites the queue with exactly these events.
fn write_queue(path: &Path, events: &[Event]) -> std::io::Result<()> {
    let mut body = String::new();
    for event in events {
        if let Ok(line) = serde_json::to_string(event) {
            body.push_str(&line);
            body.push('\n');
        }
    }
    std::fs::write(path, body)
}

#[cfg(test)]
// `expect` is right in tests: a failed expectation is the diagnostic.
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn recorder(dir: &Path, consent: Option<bool>) -> Recorder {
        Recorder::new(dir.to_path_buf(), consent, "1.0.0".to_owned())
    }

    #[test]
    fn records_nothing_without_consent() {
        let tmp = tempfile::TempDir::new().expect("tmp");

        for consent in [None, Some(false)] {
            let recorder = recorder(tmp.path(), consent);
            recorder.record(EventKind::Launched);

            assert!(
                !tmp.path().join(QUEUE_FILE).exists(),
                "queued an event with consent {consent:?}"
            );
            assert!(
                !recorder.has_install_id(),
                "created an install id with consent {consent:?}"
            );
            assert!(recorder.take_batch().is_none());
        }
    }

    #[test]
    fn records_and_batches_once_consent_is_given() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), Some(true));

        recorder.record(EventKind::Launched);
        recorder.record(EventKind::TorrentAdded);

        let batch = recorder.take_batch().expect("a batch");
        assert_eq!(batch.schema, SCHEMA_VERSION);
        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].kind, EventKind::Launched);
        assert!(!batch.install_id.is_empty());

        // Taking a batch empties the queue.
        assert!(recorder.take_batch().is_none());
    }

    #[test]
    fn withdrawing_consent_forgets_the_install_and_the_backlog() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), Some(true));
        // Establish both an id and an unsent backlog.
        recorder.install_id().expect("id");
        recorder.record(EventKind::Launched);
        assert!(recorder.has_install_id());

        recorder.set_consent(Some(false));

        // "Stop collecting" that left a queued backlog and a stable id behind
        // would be a lie.
        assert!(!recorder.has_install_id(), "install id survived withdrawal");
        assert!(
            !tmp.path().join(QUEUE_FILE).exists(),
            "queue survived withdrawal"
        );
    }

    #[test]
    fn consent_alone_writes_nothing_to_disk() {
        // The id is written on the first batch that is actually sent, so a
        // user who consents and then records nothing leaves no trace.
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), None);

        recorder.set_consent(Some(true));

        assert!(!recorder.has_install_id());
        assert!(!tmp.path().join(QUEUE_FILE).exists());
    }

    #[test]
    fn granting_consent_later_starts_collecting() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), None);
        recorder.record(EventKind::Launched);
        assert!(recorder.take_batch().is_none());

        recorder.set_consent(Some(true));
        recorder.record(EventKind::TorrentCompleted);

        let batch = recorder.take_batch().expect("a batch");
        assert_eq!(batch.events.len(), 1, "only post-consent events");
        assert_eq!(batch.events[0].kind, EventKind::TorrentCompleted);
    }

    #[test]
    fn the_install_id_is_stable_across_recorders() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let first = recorder(tmp.path(), Some(true)).install_id().expect("id");
        let second = recorder(tmp.path(), Some(true)).install_id().expect("id");
        assert_eq!(first, second);
    }

    #[test]
    fn the_queue_is_capped_and_drops_the_oldest() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), Some(true));

        // One more than the cap; the first one recorded should be gone.
        for _ in 0..MAX_QUEUED_EVENTS {
            recorder.record(EventKind::Launched);
        }
        recorder.record(EventKind::TorrentCompleted);

        let batch = recorder.take_batch().expect("a batch");
        assert_eq!(batch.events.len(), MAX_QUEUED_EVENTS);
        assert_eq!(
            batch.events[MAX_QUEUED_EVENTS - 1].kind,
            EventKind::TorrentCompleted,
            "the newest event should survive"
        );
    }

    #[test]
    fn events_too_old_to_send_never_leave_the_queue() {
        // The holiday case: a queue that outlived a long closure would
        // otherwise be posted stale and refused with a 400 every hour.
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), Some(true));

        let stale = Event {
            at: unix_now() - (MAX_EVENT_AGE.as_secs() + HOUR),
            kind: EventKind::Launched,
        };
        recorder.append(stale).expect("append");
        recorder.record(EventKind::TorrentCompleted);

        let batch = recorder.take_batch().expect("a batch");

        assert_eq!(batch.events.len(), 1, "the stale event should be dropped");
        assert_eq!(batch.events[0].kind, EventKind::TorrentCompleted);
    }

    #[test]
    fn a_queue_of_nothing_but_stale_events_is_cleared() {
        // Otherwise the same dead events are re-read on every flush forever.
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), Some(true));

        recorder
            .append(Event {
                at: unix_now() - (MAX_EVENT_AGE.as_secs() + HOUR),
                kind: EventKind::Launched,
            })
            .expect("append");

        assert!(recorder.take_batch().is_none());
        assert!(
            !tmp.path().join(QUEUE_FILE).exists(),
            "queue should be gone"
        );
    }

    #[test]
    fn a_failed_send_is_restored_in_order() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), Some(true));

        recorder.record(EventKind::Launched);
        let batch = recorder.take_batch().expect("a batch");

        // Recorded while the send was notionally in flight.
        recorder.record(EventKind::TorrentCompleted);
        recorder.restore(&batch);

        let retry = recorder.take_batch().expect("a batch");
        assert_eq!(
            retry.events.iter().map(|e| e.kind).collect::<Vec<_>>(),
            vec![EventKind::Launched, EventKind::TorrentCompleted],
            "the restored batch should come first"
        );
    }

    #[test]
    fn a_torn_line_costs_one_event_not_the_queue() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = recorder(tmp.path(), Some(true));
        recorder.record(EventKind::Launched);

        // Simulates a crash partway through an append.
        let path = tmp.path().join(QUEUE_FILE);
        let mut body = std::fs::read_to_string(&path).expect("read");
        body.push_str("{\"at\":123,\"eve");
        std::fs::write(&path, body).expect("write");

        let batch = recorder.take_batch().expect("a batch");
        assert_eq!(batch.events.len(), 1);
    }

    #[test]
    fn unknown_failure_kinds_are_dropped_rather_than_passed_through() {
        // The whole point of the closed enum: a new `CommandError` kind cannot
        // start being collected without a deliberate change here.
        assert_eq!(
            FailureKind::parse("metadataTimeout"),
            Some(FailureKind::MetadataTimeout)
        );
        assert_eq!(FailureKind::parse("somethingNew"), None);
        assert_eq!(FailureKind::parse(""), None);
    }

    #[test]
    fn an_event_serialises_to_the_documented_shape() {
        let event = Event {
            at: 1_756_598_400,
            kind: EventKind::TorrentPreviewed {
                source: AddSource::Magnet,
            },
        };
        let json = serde_json::to_string(&event).expect("serialise");
        assert_eq!(
            json,
            r#"{"at":1756598400,"event":"torrentPreviewed","source":"magnet"}"#
        );

        // A fieldless variant is still tagged, so the collector can validate
        // every event the same way.
        let bare = Event {
            at: 1_756_598_400,
            kind: EventKind::TorrentAdded,
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("serialise"),
            r#"{"at":1756598400,"event":"torrentAdded"}"#
        );
    }

    #[test]
    fn timestamps_are_truncated_to_the_hour() {
        // Exact timestamps let batches be correlated into a session timeline.
        assert_eq!(truncate_to_hour(1_756_598_400 + 1_759), 1_756_598_400);
        assert_eq!(Event::now(EventKind::Launched).at % HOUR, 0);
    }

    #[test]
    fn changed_settings_are_reported_by_key_only() {
        let previous = Settings::default();
        let next = Settings {
            listen_port: previous.listen_port + 1,
            proxy_url: Some("socks5://10.0.0.9:1080".to_owned()),
            ..previous.clone()
        };

        let keys = SettingKey::changed(&previous, &next);

        assert_eq!(keys, vec![SettingKey::NetListenPort, SettingKey::NetProxy]);
        // The key, never the value. Serialising the whole event must not
        // produce the proxy URL anywhere.
        let json = serde_json::to_string(&Event::now(EventKind::SettingChanged {
            key: SettingKey::NetProxy,
        }))
        .expect("serialise");
        assert!(json.contains("net.proxy"));
        assert!(!json.contains("10.0.0.9"));
    }

    #[test]
    fn identical_settings_report_no_changes() {
        let settings = Settings::default();
        assert!(SettingKey::changed(&settings, &settings).is_empty());
    }

    #[test]
    fn buckets_cover_their_boundaries() {
        assert_eq!(
            DurationBucket::of(Duration::from_secs(299)),
            DurationBucket::UnderFiveMinutes
        );
        assert_eq!(
            DurationBucket::of(Duration::from_secs(300)),
            DurationBucket::UnderHalfHour
        );
        assert_eq!(
            DurationBucket::of(Duration::from_secs(28_800)),
            DurationBucket::AllDay
        );

        assert_eq!(CountBucket::of(0), CountBucket::None);
        assert_eq!(CountBucket::of(5), CountBucket::Few);
        assert_eq!(CountBucket::of(6), CountBucket::Some);
        assert_eq!(CountBucket::of(101), CountBucket::Lots);
    }
}
