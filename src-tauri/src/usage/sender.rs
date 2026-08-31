//! Sends queued batches to the collector.
//!
//! # Why this is in Rust and not the webview
//!
//! `src-tauri/tauri.conf.json` sets `connect-src 'self' ipc:
//! http://ipc.localhost`, so the webview cannot reach the network at all.
//! That is a security property worth keeping: widening it to let the frontend
//! POST would hand every piece of UI code an egress path, including anything
//! that renders an attacker-controlled torrent name into the DOM. Sending
//! from here means Flume has exactly one place that talks to the outside
//! world, and it is a file you can read top to bottom.
//!
//! # Cadence
//!
//! Once on launch after a randomised delay, once an hour, and once on quit.
//! Never per event: a request every time something happens would both cost
//! more than the data is worth and give a torrent client a distinctive
//! outbound traffic pattern.

use std::{sync::Arc, time::Duration};

use rand::Rng;

use super::{Envelope, Recorder};

/// Where batches are POSTed.
///
/// Baked in at build time. A build without `FLUME_USAGE_ENDPOINT` set compiles
/// with reporting inert, which is what keeps CI and `cargo test` free of any
/// network configuration — and means a fork builds a Flume that cannot report
/// anything without someone deliberately setting it.
pub const ENDPOINT: Option<&str> = option_env!("FLUME_USAGE_ENDPOINT");

/// How long to wait for the collector before giving up.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Time between scheduled flushes.
const FLUSH_INTERVAL: Duration = Duration::from_secs(3_600);

/// Earliest and latest delay before the first flush after launch.
///
/// Randomised so that a release does not produce a thundering herd, and so
/// that the request is not a reliable marker of when the app was opened.
const STARTUP_DELAY: std::ops::Range<u64> = 30..90;

/// Events older than this are dropped rather than retried forever.
const MAX_EVENT_AGE: Duration = Duration::from_secs(3 * 24 * 3_600);

/// Posts batches to the collector.
pub struct Sender {
    /// Shared HTTP client. Reused so connections are pooled across flushes.
    client: reqwest::Client,
    /// The configured endpoint, or `None` when reporting is inert.
    endpoint: Option<&'static str>,
}

impl Sender {
    /// Builds a sender, or `None` if the HTTP client cannot be constructed.
    #[must_use]
    pub fn new() -> Option<Self> {
        let endpoint = ENDPOINT?;
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            // Explicit rather than reqwest's default, which would announce the
            // dependency's version to the collector for no reason.
            .user_agent(concat!("Flume/", env!("CARGO_PKG_VERSION")))
            .build()
            .ok()?;
        Some(Self {
            client,
            endpoint: Some(endpoint),
        })
    }

    /// Sends whatever is queued, restoring it if the send fails.
    ///
    /// Does nothing when consent is absent or the queue is empty.
    pub async fn flush(&self, recorder: &Recorder) {
        let Some(endpoint) = self.endpoint else {
            return;
        };
        let Some(envelope) = recorder.take_batch() else {
            return;
        };

        match self.post(endpoint, &envelope).await {
            Ok(()) => log::debug!("sent {} usage events", envelope.events.len()),
            Err(err) => {
                log::debug!("could not send usage events: {err}");
                recorder.restore(&prune_expired(envelope));
            }
        }
    }

    /// POSTs one envelope, treating any non-2xx as a failure.
    async fn post(&self, endpoint: &str, envelope: &Envelope) -> Result<(), String> {
        let response = self
            .client
            .post(endpoint)
            .json(envelope)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            // A 4xx means the collector rejected the shape, and retrying an
            // identical body forever would not help. It is still dropped by
            // age rather than immediately, so a deploy that briefly 400s does
            // not lose a backlog.
            Err(format!("collector returned {}", response.status()))
        }
    }
}

/// Drops events too old to be worth retrying.
fn prune_expired(mut envelope: Envelope) -> Envelope {
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .saturating_sub(MAX_EVENT_AGE.as_secs());

    envelope.events.retain(|event| event.at >= cutoff);
    envelope
}

/// Runs the flush loop until the process exits.
///
/// Tauri-free by design, so `lib.rs` owns the decision of when to start it and
/// this module stays testable.
pub fn spawn(recorder: Arc<Recorder>) {
    let Some(sender) = Sender::new() else {
        log::debug!("usage reporting has no endpoint configured; nothing will be sent");
        return;
    };

    tauri::async_runtime::spawn(async move {
        let delay = {
            let mut rng = rand::rng();
            rng.random_range(STARTUP_DELAY)
        };
        tokio::time::sleep(Duration::from_secs(delay)).await;

        let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
        // A machine that was asleep should send one batch on waking, not a
        // burst of catch-up requests.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            sender.flush(&recorder).await;
            ticker.tick().await;
        }
    });
}

#[cfg(test)]
// `expect` is right in tests: a failed expectation is the diagnostic.
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::usage::{Event, EventKind};

    fn envelope(events: Vec<Event>) -> Envelope {
        Envelope {
            schema: crate::usage::SCHEMA_VERSION,
            install_id: "test".to_owned(),
            app_version: "1.0.0".to_owned(),
            os: "macos".to_owned(),
            arch: "aarch64".to_owned(),
            events,
        }
    }

    #[test]
    fn expired_events_are_not_retried_forever() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs();

        let fresh = Event {
            at: now,
            kind: EventKind::Launched,
        };
        let stale = Event {
            at: now - (4 * 24 * 3_600),
            kind: EventKind::TorrentCompleted,
        };

        let pruned = prune_expired(envelope(vec![stale, fresh]));

        assert_eq!(pruned.events.len(), 1);
        assert_eq!(pruned.events[0].kind, EventKind::Launched);
    }

    #[test]
    fn a_build_without_an_endpoint_sends_nothing() {
        // The default for CI, `cargo test`, and any fork that has not set one.
        if ENDPOINT.is_none() {
            assert!(
                Sender::new().is_none(),
                "a sender should not exist without an endpoint"
            );
        }
    }
}
