//! Flume — a beautiful, cross-platform BitTorrent client.
//!
//! This crate is the Tauri v2 backend. It embeds [`librqbit`] as an in-process
//! torrent engine and exposes a small JSON command surface to the Next.js
//! frontend running in the WebView.
//!
//! # Layering
//!
//! * [`engine`] — a Tauri-free wrapper around `librqbit::Session`. Unit
//!   testable on its own.
//! * [`state`] — process-wide shared state handed to command handlers.
//! * [`commands`] — `#[tauri::command]` entry points. Thin by design.
//! * [`settings`] — user configuration and its persistence. Also Tauri-free.
//! * [`telemetry`] — pushes batched status to the UI on a fixed cadence.
//!
//! Torrent piece data is written to disk by librqbit and never crosses the IPC
//! boundary; the WebView only ever receives small JSON status payloads.

pub mod commands;
pub mod engine;
pub mod settings;
pub mod state;
pub mod telemetry;

use settings::Settings;
use state::AppState;
use tauri::Manager;

/// Builds and runs the Tauri application.
///
/// Starts the window immediately and brings the torrent engine up in the
/// background, so a slow DHT bootstrap never delays first paint.
///
/// # Panics
///
/// Panics if the Tauri runtime itself fails to start, which is unrecoverable.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
// A failure to build the Tauri context means the app binary itself is
// malformed; there is no recovery path and no UI to report it in.
#[allow(clippy::expect_used)]
pub fn run() {
    let session_dir = settings::session_directory();
    let (settings, problem) = Settings::load(&session_dir);
    if let Some(problem) = &problem {
        // Logged rather than fatal: a user who cannot launch the app cannot
        // fix their settings from inside it.
        eprintln!("flume: {problem}; using defaults");
    }

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new(settings, session_dir))
        .invoke_handler(tauri::generate_handler![
            commands::get_core_status,
            commands::get_telemetry,
            commands::preview_torrent,
            commands::confirm_add,
            commands::discard_preview,
            commands::pause_torrent,
            commands::resume_torrent,
            commands::remove_torrent,
            commands::set_only_files,
            commands::get_settings,
            commands::update_settings
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            telemetry::spawn(handle.clone());
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let settings = state.settings().await;
                log::info!(
                    "starting torrent engine: port={} dht={} upnp={}",
                    settings.listen_port,
                    settings.enable_dht,
                    settings.enable_upnp
                );
                if let Err(err) = state.restart_engine(&settings).await {
                    log::error!("torrent engine failed to start: {err}");
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build the Tauri application")
        .run(|app_handle, event| {
            // Flush fast-resume and session state before the process exits so a
            // restart resumes instead of re-hashing.
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                tauri::async_runtime::block_on(state.shutdown());
            }
        });
}
