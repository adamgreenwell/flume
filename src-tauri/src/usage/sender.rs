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

/// The raw compiled-in value, exactly as the build environment supplied it.
///
/// Baked in at build time. Prefer [`endpoint`] over reading this directly:
/// `option_env!` distinguishes unset from *empty*, so this is `Some("")` when
/// `FLUME_USAGE_ENDPOINT=` was exported, which is not a usable endpoint and
/// must not be treated as one.
pub const ENDPOINT: Option<&str> = option_env!("FLUME_USAGE_ENDPOINT");

/// The scheme a collector endpoint must use.
///
/// Deliberately identical to the `https://*` gate in
/// `.github/workflows/release.yml`, so a value cannot pass one and fail the
/// other. A predicate stricter than CI's would ship a build that compiles
/// inert after a green release; looser, and CI is the only thing standing
/// between a fork and a Flume that reports over plaintext.
const REQUIRED_SCHEME: &str = "https://";

/// Whether a compiled-in value is something this build could actually POST to.
///
/// Written as a byte walk rather than `starts_with` so it stays a `const fn`:
/// `str::starts_with` is not const, and [`should_warn`] is const through
/// [`is_configured`].
///
/// Requires at least one byte past the scheme, because a bare `https://` has
/// no host and fails every request exactly as `""` does — the failure this
/// whole predicate exists to stop.
const fn is_usable(url: &str) -> bool {
    let bytes = url.as_bytes();
    let scheme = REQUIRED_SCHEME.as_bytes();

    if bytes.len() <= scheme.len() {
        return false;
    }

    let mut i = 0;
    while i < scheme.len() {
        if bytes[i] != scheme[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The endpoint this build can POST to, or `None` if it has none it can use.
///
/// **Everything that asks "can this build report?" must go through here.**
/// [`is_configured`] and [`Sender::new`] used to ask two different questions —
/// `ENDPOINT.is_some()` and `let endpoint = ENDPOINT?` — which happened to
/// agree only while the value was either absent or good. Under `Some("")` they
/// disagreed: the diagnostics report said the endpoint was missing while
/// [`spawn`] built a sender and POSTed to the empty string forever, at debug
/// level. One accessor means they cannot diverge again.
const fn endpoint() -> Option<&'static str> {
    // A let chain rather than nested `if`s: edition 2024, and clippy asks.
    if let Some(url) = ENDPOINT
        && is_usable(url)
    {
        return Some(url);
    }
    None
}

/// Whether this build has a collector endpoint it can actually use.
///
/// Public so the diagnostics report can say so: "usage reporting is on" in a
/// build that cannot send is a sentence that describes nothing happening, and
/// the bundle is where someone would go looking for why.
#[must_use]
pub const fn is_configured() -> bool {
    endpoint().is_some()
}

/// Whether reporting is switched on in a build that cannot send.
///
/// This is the one failure mode the design makes invisible: consent granted,
/// events queueing to disk, a queue file growing, no error anywhere the user
/// can see, and nothing arriving. Left at debug level it is indistinguishable
/// from a healthy install whose collector happens to be empty -- which is
/// exactly how it presented the first two times it happened.
///
/// Called at startup and again whenever consent is granted, because either can
/// be the moment the contradiction appears: a build with no endpoint can be
/// launched with consent already on, or consent can be switched on in one.
#[must_use]
pub const fn should_warn(enabled: bool) -> bool {
    enabled && !is_configured()
}

/// Emits that warning.
///
/// Split from [`should_warn`] so the decision is testable without capturing a
/// global logger. Verifying a "this failure is no longer silent" change by
/// reading the code would repeat the mistake that made it necessary.
pub fn warn_if_unconfigured(enabled: bool) {
    if should_warn(enabled) {
        log::warn!(
            "usage reporting is switched on, but this build has no usable collector \
             endpoint -- FLUME_USAGE_ENDPOINT was unset, empty, or not an https:// \
             URL when it was built. Events will queue on disk and never be sent."
        );
    }
}

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
    endpoint: &'static str,
}

impl Sender {
    /// Builds a sender, or `None` if the HTTP client cannot be constructed.
    #[must_use]
    pub fn new() -> Option<Self> {
        // Through `endpoint()`, never `ENDPOINT`: a sender that exists when
        // `is_configured()` says otherwise is the bug this module had.
        let endpoint = endpoint()?;
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            // Explicit rather than reqwest's default, which would announce the
            // dependency's version to the collector for no reason.
            .user_agent(concat!("Flume/", env!("CARGO_PKG_VERSION")))
            .build()
            .ok()?;
        Some(Self { client, endpoint })
    }

    /// Sends whatever is queued, restoring it if the send fails.
    ///
    /// Does nothing when consent is absent or the queue is empty.
    pub async fn flush(&self, recorder: &Recorder) {
        let Some(envelope) = recorder.take_batch() else {
            return;
        };

        match self.post(self.endpoint, &envelope).await {
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
        // Loud when it matters, quiet when it does not: a build with no
        // endpoint is the normal state for CI, `cargo test`, and any fork, and
        // warning at every launch would train people to ignore it.
        warn_if_unconfigured(recorder.is_enabled());
        if !recorder.is_enabled() {
            log::debug!("usage reporting has no endpoint configured; nothing will be sent");
        }
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
    fn warns_exactly_when_consent_is_on_and_the_build_cannot_send() {
        // Quiet when it does not matter: an endpoint-less build is the normal
        // state for CI, `cargo test` and any fork, and warning at every launch
        // would train people to ignore the one case that matters.
        assert!(!should_warn(false), "never warn without consent");

        if is_configured() {
            assert!(
                !should_warn(true),
                "a configured build has nothing to warn about"
            );
        } else {
            assert!(
                should_warn(true),
                "consent on with no endpoint is the silent failure this exists for"
            );
        }
    }

    #[test]
    fn only_a_real_https_url_counts_as_usable() {
        // Table-driven and over literals, so it holds whatever this build was
        // compiled with -- the previous test asserted
        // `is_configured() == ENDPOINT.is_some()`, which is the exact
        // equivalence that was wrong, and it passed under `Some("")` by
        // agreeing with the bug.
        for good in [
            "https://flume-usage.example.workers.dev/v1/usage",
            "https://x",
            // Trailing whitespace is left alone deliberately: the `url` crate
            // reqwest parses with normalises it away, and rejecting it here
            // when `release.yml`'s glob accepts it would create the reverse
            // drift -- a secret with a stray newline passing CI and shipping
            // a build that compiles inert.
            "https://x.dev/v1/usage\n",
        ] {
            assert!(is_usable(good), "{good:?} should be usable");
        }

        for bad in [
            "",
            "   ",
            "not-a-url",
            "ftp://x",
            "http://collector.example.com/v1/usage",
            // Plaintext to loopback is still plaintext, and no documented
            // workflow needs it: collector/README.md drives `wrangler dev`
            // with curl, never a client build.
            "http://localhost:8787/v1/usage",
            // Scheme but no host: fails every POST exactly as "" does.
            "https://",
            // Case matters, because `release.yml`'s glob is case-sensitive.
            "HTTPS://x.dev",
        ] {
            assert!(!is_usable(bad), "{bad:?} should not be usable");
        }
    }

    #[test]
    fn a_sender_exists_exactly_when_the_build_is_configured() {
        // The invariant that was broken. `Sender::new` gated on `ENDPOINT?`
        // while `is_configured()` gated on `ENDPOINT.is_some()`, so with
        // `FLUME_USAGE_ENDPOINT=""` a sender was built and flushed forever
        // against the empty string while the diagnostics report and the
        // consent-change warning both said there was no endpoint. Both now
        // route through `endpoint()`, and this pins that they agree.
        assert_eq!(Sender::new().is_some(), is_configured());
    }

    #[test]
    fn an_unusable_endpoint_is_not_a_configured_one() {
        // Guards the empty case specifically, on a literal, so it fails on a
        // machine where the variable is unset -- which the old suite could
        // not do.
        assert!(!is_usable(""), "an empty endpoint is not configured");
        assert_eq!(endpoint().is_some(), is_configured());
    }
}
