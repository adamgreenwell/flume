//! Pushes batched telemetry to the UI on a fixed cadence.
//!
//! # Why push rather than let the UI poll
//!
//! Polling costs one IPC round trip per tick regardless of whether anything
//! changed, and the natural next step — a request per torrent — scales with
//! the list. Emitting one batched payload per tick keeps IPC volume flat as
//! the torrent count grows.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

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

        loop {
            ticker.tick().await;

            let Some(engine) = app.state::<AppState>().engine().await else {
                continue;
            };

            if let Err(err) = app.emit(TELEMETRY_EVENT, engine.telemetry()) {
                // A failed emit means the webview is gone or shutting down.
                // Log once per occurrence and keep the loop alive; the window
                // may still be re-created.
                log::warn!("failed to emit telemetry: {err}");
            }
        }
    });
}
