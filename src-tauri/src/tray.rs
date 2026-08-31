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

/// Reveal the window.
const SHOW: &str = "show";
/// Pause every torrent.
const PAUSE_ALL: &str = "pause_all";
/// Resume every torrent.
const RESUME_ALL: &str = "resume_all";
/// Quit the application.
const QUIT: &str = "quit";

/// The tray's own menu item ids.
///
/// Named rather than repeated as literals in two places: the menu is built
/// from these and [`handle_menu_event`] dispatches on them, and a typo in
/// either would produce an item that silently does nothing.
const OWN_ITEMS: [&str; 4] = [SHOW, PAUSE_ALL, RESUME_ALL, QUIT];

/// Builds the tray icon and installs its menu.
///
/// # Errors
///
/// Returns an error if the platform refuses to create a tray icon, which
/// happens on Linux desktops with no system tray available. The caller should
/// log and continue: the app is perfectly usable without one.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW, "Show Flume", true, None::<&str>)?;
    let pause_all = MenuItem::with_id(app, PAUSE_ALL, "Pause all", true, None::<&str>)?;
    let resume_all = MenuItem::with_id(app, RESUME_ALL, "Resume all", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit Flume", true, None::<&str>)?;

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
///
/// # Why the fall-through is not a warning
///
/// This handler receives *every* menu event in the process, not only the
/// tray's own: the application menu built in [`crate::menu`] and the
/// platform's predefined items land here too. Warning about those was wrong
/// twice over — pressing ⌘, logged `unhandled tray menu item: open_settings`
/// about an item that had just worked correctly, and anyone reading the log
/// afterwards would conclude the tray was broken. It is convincing enough that
/// it was filed as a bug on exactly that reading.
///
/// So an id the tray does not own is not its business and is ignored. An id it
/// *does* own reaching the fall-through is a real defect — a menu item added
/// without a match arm — and keeps the warning.
fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        SHOW => deeplink::focus_main_window(app),
        PAUSE_ALL => set_all_paused(app, true),
        RESUME_ALL => set_all_paused(app, false),
        // `exit` rather than closing the window: the run loop's Exit handler
        // awaits engine shutdown, which is what flushes fast-resume state.
        QUIT => app.exit(0),
        other if OWN_ITEMS.contains(&other) => {
            log::warn!("tray menu item {other} was built but has no handler");
        }
        other => log::trace!("tray ignoring {other}, which belongs to another menu"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tray_does_not_claim_ids_another_menu_owns() {
        // The bug this file had: the application menu's ids reached this
        // handler and were reported as broken tray items. They are not the
        // tray's, and adding one here would resurrect that.
        for foreign in [crate::menu::OPEN_SETTINGS, crate::menu::ADD_TORRENT] {
            assert!(
                !OWN_ITEMS.contains(&foreign),
                "{foreign} belongs to the application menu, not the tray"
            );
        }
    }

    #[test]
    fn every_tray_id_is_distinct() {
        // Two items sharing an id would make one of them dispatch as the
        // other, which no amount of reading the match arm would reveal.
        for (i, id) in OWN_ITEMS.iter().enumerate() {
            assert!(
                !OWN_ITEMS[..i].contains(id),
                "{id} appears twice in the tray menu"
            );
        }
    }
}
