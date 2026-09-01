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
//! * [`deeplink`] — `magnet:` handling and single-instance behaviour.
//! * [`diagnostics`] — a redacted bundle the user can paste into an issue.
//! * [`settings`] — user configuration and its persistence. Also Tauri-free.
//! * [`telemetry`] — pushes batched status to the UI on a fixed cadence.
//! * [`usage`] — opt-in anonymous counts. The only thing that leaves the
//!   machine, and only with consent. Distinct from [`telemetry`], which never
//!   leaves the process.
//! * [`tray`] — system tray icon and quick actions.
//!
//! Torrent piece data is written to disk by librqbit and never crosses the IPC
//! boundary; the WebView only ever receives small JSON status payloads.

pub mod commands;
pub mod deeplink;
pub mod diagnostics;
pub mod egress;
pub mod engine;
mod menu;
pub mod settings;
pub mod state;
pub mod telemetry;
pub mod tray;
pub mod usage;

use std::{sync::Arc, time::Duration};

use settings::Settings;
use state::AppState;
use tauri::Manager;

/// How long the final usage flush may take before the process exits anyway.
///
/// A quit that hangs on a network request is a much worse bug than a lost
/// batch — the next launch sends it regardless.
const SHUTDOWN_FLUSH_TIMEOUT: Duration = Duration::from_secs(3);

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
    let first_run = !Settings::exists(&session_dir);
    let (settings, problem) = Settings::load(&session_dir);
    if let Some(problem) = &problem {
        // Logged rather than fatal: a user who cannot launch the app cannot
        // fix their settings from inside it.
        eprintln!("flume: {problem}; using defaults");
    }

    tauri::Builder::default()
        // Must be registered first: it decides whether this process continues
        // at all, and a second instance should do as little as possible before
        // handing off and exiting.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            deeplink::handle_second_instance(app, &argv);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new(settings, session_dir, first_run))
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
            commands::get_torrent_files,
            commands::get_torrent_detail,
            commands::get_diagnostics,
            commands::get_settings,
            commands::update_settings,
            commands::is_first_run,
            commands::detect_clients,
            commands::import_client
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            telemetry::spawn(handle.clone());

            // Deliver magnet links opened from the OS while Flume is running.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let deep_link_handle = handle.clone();
                app.deep_link().on_open_url(move |event| {
                    deeplink::focus_main_window(&deep_link_handle);
                    for url in event.urls() {
                        deeplink::route_argument(&deep_link_handle, url.as_str());
                    }
                });

                // Runtime registration matters for Linux and for development,
                // where no .desktop entry or bundle exists yet. macOS does not
                // support it at all -- the association comes from the bundled
                // app's Info.plist -- so "unsupported platform" here is
                // expected and not a problem.
                if let Err(err) = app.deep_link().register_all() {
                    log::debug!(
                        "runtime deep-link registration unavailable ({err}); \
                         on macOS and Windows the installed bundle registers the scheme"
                    );
                }
            }

            // Like the tray, the menu is not worth failing a launch over —
            // though unlike the tray its absence is keenly felt on macOS,
            // where it takes ⌘C and ⌘V with it.
            if let Err(err) = menu::install(&handle) {
                log::warn!("could not install the application menu: {err}");
            }

            // A tray is a nicety, not a requirement: some Linux desktops have
            // none, and failing to create one must not stop the app starting.
            if let Err(err) = tray::install(&handle) {
                log::warn!("could not create the tray icon: {err}");
            }

            // A magnet passed on the command line at cold start, e.g. the very
            // first click of a magnet link before Flume was running.
            for argument in std::env::args().skip(1) {
                deeplink::route_argument(&handle, &argument);
            }
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

                // Recorded after the engine is up so the library size is the
                // restored one rather than zero. Both are no-ops unless the
                // user consented.
                state.note(usage::EventKind::Launched);
                let torrents = state
                    .engine()
                    .await
                    .map_or(0, |engine| engine.telemetry().torrents.len());
                state.note(usage::EventKind::LibraryCount {
                    bucket: usage::CountBucket::of(torrents),
                });

                usage::sender::spawn(Arc::clone(state.usage()));
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
                state.note(usage::EventKind::SessionEnded {
                    duration_bucket: usage::DurationBucket::of(state.uptime()),
                });
                tauri::async_runtime::block_on(async {
                    // The last flush gets one shot, and the timeout is
                    // shorter than the request's own on purpose: a quit that
                    // hangs on a network request is a far worse bug than a
                    // late batch. Cancelling here used to *destroy* the batch,
                    // because `take_batch` had already deleted the queue file
                    // and neither arm of the match ran; `Restore` now puts it
                    // back on drop, so the next launch sends it.
                    if let Some(sender) = usage::sender::Sender::new() {
                        let flush = sender.flush(state.usage());
                        let _ = tokio::time::timeout(SHUTDOWN_FLUSH_TIMEOUT, flush).await;
                    }
                    state.shutdown().await;
                });
            }
        });
}
