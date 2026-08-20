//! System tray icon and its menu.
//!
//! The tray exists so Flume stays reachable while its window is closed —
//! a torrent client spends most of its life in the background, and quitting it
//! to close the window would stop seeding.

use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{deeplink, state::AppState};

/// Builds the tray icon and installs its menu.
///
/// # Errors
///
/// Returns an error if the platform refuses to create a tray icon, which
/// happens on Linux desktops with no system tray available. The caller should
/// log and continue: the app is perfectly usable without one.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Flume", true, None::<&str>)?;
    let pause_all = MenuItem::with_id(app, "pause_all", "Pause all", true, None::<&str>)?;
    let resume_all = MenuItem::with_id(app, "resume_all", "Resume all", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Flume", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &pause_all, &resume_all, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("no default window icon for the tray".into())
        })?)
        .tooltip("Flume")
        .menu(&menu)
        // Left click should reveal the window rather than open the menu, which
        // is what people expect from a tray icon on Windows and Linux. macOS
        // convention is menu-on-any-click, and Tauri handles that difference.
        .show_menu_on_left_click(false)
        .on_menu_event(|app: &AppHandle, event| handle_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                deeplink::focus_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Dispatches a tray menu selection.
fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show" => deeplink::focus_main_window(app),
        "pause_all" => set_all_paused(app, true),
        "resume_all" => set_all_paused(app, false),
        // `exit` rather than closing the window: the run loop's Exit handler
        // awaits engine shutdown, which is what flushes fast-resume state.
        "quit" => app.exit(0),
        other => log::warn!("unhandled tray menu item: {other}"),
    }
}

/// Pauses or resumes every torrent.
///
/// Runs on the async runtime because each operation awaits the engine; doing
/// this inline would block the menu callback and freeze the tray.
fn set_all_paused(app: &AppHandle, paused: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(engine) = app.state::<AppState>().engine().await else {
            return;
        };

        for summary in engine.torrent_summaries() {
            // Skip torrents already in the requested state, so a bulk action
            // does not churn through work that changes nothing.
            let already = matches!(summary.state, crate::engine::TorrentState::Paused);
            if already == paused {
                continue;
            }

            let result = if paused {
                engine.pause(summary.id).await
            } else {
                engine.resume(summary.id).await
            };
            if let Err(err) = result {
                log::warn!(
                    "tray {} failed for {}: {err}",
                    if paused { "pause" } else { "resume" },
                    summary.name
                );
            }
        }
    });
}
