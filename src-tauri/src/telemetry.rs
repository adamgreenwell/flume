//! Pushes batched telemetry to the UI on a fixed cadence.
//!
//! # Why push rather than let the UI poll
//!
//! Polling costs one IPC round trip per tick regardless of whether anything
//! changed, and the natural next step — a request per torrent — scales with
//! the list. Emitting one batched payload per tick keeps IPC volume flat as
//! the torrent count grows.

use std::{collections::HashSet, time::Duration};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;

/// Event name the frontend subscribes to.
///
/// Changing this breaks `src/hooks/useTelemetry.ts`; change both together.
pub const TELEMETRY_EVENT: &str = "flume://telemetry";

/// How often telemetry is emitted.
///
/// One second is fast enough that transfer rates feel live, and slow enough
/// that the webview is not re-rendering constantly while seeding.
pub const TELEMETRY_INTERVAL: Duration = Duration::from_secs(1);

/// Spawns the telemetry loop, which runs until the app exits.
///
/// Ticks are skipped silently while the engine is still starting: the UI shows
/// its own starting state, and an error event every second would be noise.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(TELEMETRY_INTERVAL);
        // If a tick is missed (a slow snapshot, a suspended machine), skip it
        // rather than firing a burst of catch-up events at the webview.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Info hashes already known to be complete, so a torrent is announced
        // once rather than every second.
        let mut announced: HashSet<String> = HashSet::new();
        // Torrents restored from a previous session are already finished; they
        // must seed this set rather than trigger a burst of notifications for
        // downloads the user completed days ago.
        let mut seeded = false;

        loop {
            ticker.tick().await;

            let state = app.state::<AppState>();
            let Some(engine) = state.engine().await else {
                continue;
            };

            // The library record is read per tick rather than cached: it is a
            // small map, it changes only on an add or a remove, and a stale
            // copy would show a torrent the user just added with no arrival
            // time until the next restart.
            let snapshot = engine.telemetry_with(&state.added_times().await);

            let finished = snapshot
                .torrents
                .iter()
                .filter(|t| t.finished)
                .map(|t| (t.info_hash.clone(), t.name.clone()));

            if seeded {
                for (info_hash, name) in finished {
                    if announced.insert(info_hash.clone()) {
                        notify_complete(&app, &info_hash, &name);
                    }
                }
            } else {
                announced.extend(finished.map(|(info_hash, _)| info_hash));
                seeded = true;
            }

            if let Err(err) = app.emit(TELEMETRY_EVENT, snapshot) {
                // A failed emit means the webview is gone or shutting down.
                // Log once per occurrence and keep the loop alive; the window
                // may still be re-created.
                log::warn!("failed to emit telemetry: {err}");
            }
        }
    });
}

/// Shows a desktop notification for a completed torrent.
///
/// Failures are logged rather than propagated: the user may have denied
/// notification permission, and that must not disturb the telemetry loop.
fn notify_complete(app: &AppHandle, info_hash: &str, name: &str) {
    // The id, never the name, and never the info hash either. This line lands
    // in a log file that `crate::diagnostics` ships in a bundle the UI tells
    // people is safe to paste in public, and redaction cannot recover a name
    // once the torrent is gone: it matches literally against the library, so a
    // completed-and-removed torrent leaves its name in the log for weeks. A
    // short prefix is enough to correlate two lines in one log and identifies
    // nothing on its own.
    log::info!("torrent {} finished", &info_hash[..info_hash.len().min(6)]);
    if let Err(err) = app
        .notification()
        .builder()
        .title("Download complete")
        .body(name)
        .show()
    {
        log::warn!("could not show the completion notification: {err}");
    }
}
