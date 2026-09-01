//! The loop that decides whether a torrent engine may exist at all.
//!
//! # Why the engine is stopped rather than the torrents paused
//!
//! The obvious implementation of a kill switch is to pause every torrent when
//! the tunnel drops and resume them when it returns. It does not survive
//! contact with librqbit.
//!
//! `Session::pause` writes `is_paused` into `session.json` synchronously, and
//! librqbit stores exactly one paused bit with no reason attached. So a
//! guard-initiated pause is indistinguishable from a pause the user asked for,
//! on disk and in memory alike — and quitting while held would bring the whole
//! library back paused with the tunnel up, which is precisely the stranding
//! this feature exists to prevent. Recovering from that would mean Flume
//! keeping its own ledger of which torrents it paused, keyed by info hash
//! because ids are reused across restarts, persisted so it survives a crash,
//! and reconciled against the restored session on every launch.
//!
//! Stopping the engine instead deletes that entire problem. The guard never
//! pauses anything, so nothing it does is written to `session.json`, so
//! whatever the user had paused stays paused and whatever they had running
//! comes back running. There is no ledger, nothing to reconcile, and no
//! user-versus-guard distinction to preserve, because the guard never touches
//! the torrents.
//!
//! It is also strictly stronger. Pausing every torrent stops per-torrent peer
//! and tracker traffic and leaves the session's DHT, its TCP listener and its
//! UPnP mapping running — so a "paused" Flume is still announcing itself from
//! the address the guard is trying to protect. No session means no sockets.
//!
//! What it costs: the library goes quiet in the UI while held, and releasing
//! re-announces to trackers and re-bootstraps the DHT. The settle window in
//! [`crate::egress::SETTLE`] exists so that cost is paid once per genuine
//! reconnection rather than once per flap.
//!
//! # Why the loop lives here and not in `egress`
//!
//! [`crate::egress`] imports no Tauri types by the same rule as
//! [`crate::engine`], and this loop needs an `AppHandle` to emit events and to
//! reach [`AppState`]. The decision logic stays down there, pure and tested;
//! only the acting on it lives here.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::{egress::GuardStatus, state::AppState};

/// Event name the frontend subscribes to.
///
/// Changing this breaks the frontend listener; change both together.
pub const GUARD_EVENT: &str = "flume://egress";

/// How often the egress path is re-checked.
///
/// Matched to the telemetry cadence deliberately. The check costs about 62 µs
/// when nothing has moved — the expensive interface walk happens only when a
/// source address actually changes — so a faster cadence would buy sub-second
/// reaction at a cost the settle window makes pointless anyway.
pub const GUARD_INTERVAL: Duration = Duration::from_secs(1);

/// Runs one iteration: probe, publish, and reconcile the engine against it.
///
/// Returns what was published. Separated from [`spawn`] so startup can run the
/// first iteration synchronously and know whether an engine came up before it
/// records anything about the library.
pub async fn tick(app: &AppHandle) -> GuardStatus {
    let state = app.state::<AppState>();
    let status = state.observe_egress(Instant::now()).await;

    if status.held {
        // Idempotent: `shutdown` takes the engine out if there is one and does
        // nothing if there is not, so a held tick costs nothing after the
        // first.
        if state.engine().await.is_some() {
            log::info!(
                "egress guard: holding transfer, stopping the torrent engine ({:?})",
                status.report.verdict
            );
            state.shutdown().await;
        }
    } else if state.engine().await.is_none() {
        let settings = state.settings().await;
        log::info!(
            "egress guard: starting the torrent engine (port={} dht={} upnp={})",
            settings.listen_port,
            settings.enable_dht,
            settings.enable_upnp
        );
        if let Err(err) = state.restart_engine(&settings).await {
            log::error!("torrent engine failed to start: {err}");
        }
    }

    // Emitted every tick rather than on change. The payload is a handful of
    // scalars and two short strings, and the settle countdown has to tick down
    // on screen -- a change-only emitter would show "resumes in 10 s" and then
    // nothing until it resumed.
    if let Err(err) = app.emit(GUARD_EVENT, &status) {
        log::warn!("could not emit the egress status: {err}");
    }

    status
}

/// Spawns the guard loop, which runs until the app exits.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(GUARD_INTERVAL);
        // Skip rather than burst after a suspended machine. A laptop that
        // slept for an hour must not run an hour of catch-up ticks, and the
        // one tick it does run reads the routing table as it is now -- which
        // is the only reading that matters.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            tick(&app).await;
        }
    });
}
