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
//!
//! Torrent piece data is written to disk by librqbit and never crosses the IPC
//! boundary; the WebView only ever receives small JSON status payloads.

pub mod commands;
pub mod engine;
pub mod state;

use engine::{Engine, EngineConfig};
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
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![commands::get_core_status])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match EngineConfig::with_os_defaults() {
                    Ok(config) => {
                        log::info!(
                            "starting torrent engine: port={} dht={} upnp={}",
                            config.listen_port,
                            config.enable_dht,
                            config.enable_upnp
                        );
                        match Engine::start(config).await {
                            Ok(engine) => {
                                log::info!("torrent engine started: {engine:?}");
                                handle.state::<AppState>().set_engine(engine).await;
                            }
                            Err(err) => log::error!("torrent engine failed to start: {err}"),
                        }
                    }
                    Err(err) => log::error!("could not derive engine configuration: {err}"),
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
