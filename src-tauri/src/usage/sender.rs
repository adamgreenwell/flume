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

use super::{Delivery, Envelope, Recorder};

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
            // reqwest follows up to 10 redirects by default. A captive portal
            // answers the POST with a 302 to its own login page, which returns
            // 200 -- so with redirects on and `is_success()` as the test, a
            // hotel network reports every batch as delivered and the events,
            // already removed from disk, are destroyed. Refusing to follow
            // means the 302 is seen for what it is.
            .redirect(reqwest::redirect::Policy::none())
            // Explicit rather than reqwest's default, which would announce the
            // dependency's version to the collector for no reason.
            .user_agent(concat!("Flume/", env!("CARGO_PKG_VERSION")))
            .build()
            .inspect_err(|err| {
                // Previously `.ok()?`, which meant a configured build with
                // consent granted could fail to construct a client and say
                // nothing at all, anywhere.
                log::warn!("usage reporting could not build an HTTP client: {err}");
            })
            .ok()?;
        Some(Self { client, endpoint })
    }

    /// Sends whatever is queued, restoring it if the send does not land.
    ///
    /// Does nothing when consent is absent or the queue is empty.
    pub async fn flush(&self, recorder: &Recorder) {
        let Some(envelope) = recorder.take_batch() else {
            return;
        };

        // `take_batch` has already deleted the queue file, so from here until
        // the send resolves the only copy of these events is this local. The
        // quit path awaits this future under a 3-second timeout while the
        // request's own is 10, so it can be dropped mid-await -- and a plain
        // `match` would then run neither arm, restore nothing, and destroy the
        // batch. The guard restores on drop however this scope is left.
        let mut pending = Restore::armed(recorder, &envelope);

        // Read before the send, so the warning below can tell entering a
        // refusal from sitting in one.
        let previous = recorder.delivery();
        let outcome = self.post(&envelope).await;
        recorder.set_delivery(outcome);

        match outcome {
            Delivery::Accepted => {
                pending.disarm();
                log::debug!("sent {} usage events", envelope.events.len());
            }
            // Answered, and the answer will not change on its own. Keeping the
            // batch would retry an identical body every hour until it ages
            // out; it is dropped, and said out loud once below.
            refusal if refusal.is_settled_refusal() => {
                pending.disarm();
                log::debug!("usage batch refused: {refusal:?}");
            }
            // Everything else is worth another try: no answer at all (offline,
            // and not distinguishable from anything else), or a 5xx, which the
            // collector returns specifically to mean "try again later".
            other => log::debug!("usage batch not delivered: {other:?}"),
        }

        warn_on_settled_refusal(previous, outcome);
    }

    /// POSTs one envelope and classifies what came back.
    async fn post(&self, envelope: &Envelope) -> Delivery {
        match self.client.post(self.endpoint).json(envelope).send().await {
            // 204 exactly, not `is_success()`. The collector's only success
            // response is `new Response(null, { status: 204 })`, so any other
            // 2xx came from something that is not the collector -- a portal, a
            // proxy, a parked domain -- and treating it as delivery throws the
            // batch away.
            Ok(response) if response.status().as_u16() == 204 => Delivery::Accepted,
            Ok(response) => Delivery::Refused(response.status().as_u16()),
            Err(_) => Delivery::NoResponse,
        }
    }
}

/// Restores a taken batch unless the send is known to have landed.
///
/// Exists because `flush` can be cancelled: the quit-time flush is awaited
/// under a shorter timeout than the request it makes.
struct Restore<'a> {
    /// Where to put the events back.
    recorder: &'a Recorder,
    /// The batch in flight, or `None` once the outcome made restoring wrong.
    envelope: Option<&'a Envelope>,
}

impl<'a> Restore<'a> {
    /// Arms the guard for a batch that is about to be sent.
    fn armed(recorder: &'a Recorder, envelope: &'a Envelope) -> Self {
        Self {
            recorder,
            envelope: Some(envelope),
        }
    }

    /// Stops the guard restoring, for a batch that must not be retried.
    fn disarm(&mut self) {
        self.envelope = None;
    }
}

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if let Some(envelope) = self.envelope.take() {
            self.recorder.restore(envelope);
        }
    }
}

/// Whether this outcome is worth saying out loud, given the one before it.
///
/// Split from the logging so the decision is testable without capturing a
/// global logger, the same way [`should_warn`] is.
#[must_use]
const fn should_announce(previous: Delivery, outcome: Delivery) -> bool {
    outcome.is_settled_refusal()
        && !matches!((previous, outcome),
        (Delivery::Refused(a), Delivery::Refused(b)) if a == b)
}

/// Says once, out loud, that the collector is refusing in a way that will not
/// resolve itself.
///
/// Only for an answered refusal: a closed laptop, a plane, a captive portal
/// and a blocking firewall cannot produce one, so this cannot fire for being
/// offline. Fired only on entering the state, because `spawn` flushes twice in
/// quick succession at launch -- `tokio`'s interval yields its first tick
/// immediately -- and a warning repeated on every tick is one people learn to
/// scroll past.
fn warn_on_settled_refusal(previous: Delivery, outcome: Delivery) {
    if !should_announce(previous, outcome) {
        return;
    }
    if let Delivery::Refused(status) = outcome {
        log::warn!(
            "the usage collector refused a batch with {status} and will refuse the next \
             one identically. Nothing is being recorded about you that is not already \
             queued, but reporting is not reaching the collector. This is a defect in \
             Flume's build or its collector, not something you did."
        );
    }
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
    use crate::usage::{EventKind, Recorder};

    #[test]
    fn a_send_that_never_resolves_puts_the_batch_back() {
        // The data-loss bug this guard exists for. `take_batch` deletes the
        // queue file, and the quit path awaits `flush` under a 3-second
        // timeout while the request's own is 10 -- so the future can be
        // dropped mid-await. A plain `match` runs neither arm, restores
        // nothing, and the batch is gone.
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = Recorder::new(tmp.path().to_path_buf(), Some(true), "1.0.0".to_owned());
        recorder.record(EventKind::Launched);

        let envelope = recorder.take_batch().expect("a batch");
        assert!(recorder.take_batch().is_none(), "the queue should be empty");

        // Dropped without `disarm`, which is what cancellation looks like.
        drop(Restore::armed(&recorder, &envelope));

        let recovered = recorder
            .take_batch()
            .expect("the batch should have come back");
        assert_eq!(recovered.events.len(), 1);
        assert_eq!(recovered.events[0].kind, EventKind::Launched);
    }

    #[test]
    fn a_landed_send_does_not_put_the_batch_back() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let recorder = Recorder::new(tmp.path().to_path_buf(), Some(true), "1.0.0".to_owned());
        recorder.record(EventKind::Launched);
        let envelope = recorder.take_batch().expect("a batch");

        let mut guard = Restore::armed(&recorder, &envelope);
        guard.disarm();
        drop(guard);

        assert!(
            recorder.take_batch().is_none(),
            "a delivered batch must not be queued again"
        );
    }

    #[test]
    fn only_an_answered_refusal_that_will_not_heal_is_loud() {
        // The whole point. A closed laptop, a plane, a captive portal and a
        // blocking firewall all produce NoResponse, which must never be loud;
        // a status code means a server answered, which none of them can do.
        assert!(!Delivery::Untried.is_settled_refusal());
        assert!(!Delivery::Accepted.is_settled_refusal());
        assert!(!Delivery::NoResponse.is_settled_refusal());

        assert!(Delivery::Refused(404).is_settled_refusal());
        assert!(Delivery::Refused(413).is_settled_refusal());

        // 400 is excluded on purpose: the collector returns it for a clock two
        // hours fast and for events past its age window, both of which are
        // ordinary user situations rather than defects.
        assert!(!Delivery::Refused(400).is_settled_refusal());
        // 5xx is what the collector returns to mean "try again later".
        assert!(!Delivery::Refused(503).is_settled_refusal());
    }

    #[test]
    fn a_settled_refusal_is_announced_once_rather_than_every_tick() {
        // `spawn` flushes twice in quick succession at launch, because tokio's
        // interval yields its first tick immediately. A warning on every tick
        // is one people learn to scroll past.
        assert!(should_announce(Delivery::Accepted, Delivery::Refused(404)));
        assert!(should_announce(
            Delivery::NoResponse,
            Delivery::Refused(413)
        ));
        assert!(!should_announce(
            Delivery::Refused(404),
            Delivery::Refused(404)
        ));

        // A different refusal is a different fact and is worth saying.
        assert!(should_announce(
            Delivery::Refused(404),
            Delivery::Refused(413)
        ));

        // Recovery is silent; nothing is wrong any more.
        assert!(!should_announce(Delivery::Refused(404), Delivery::Accepted));
        assert!(!should_announce(Delivery::Accepted, Delivery::NoResponse));
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
