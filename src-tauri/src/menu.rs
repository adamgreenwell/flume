//! The application menu bar.
//!
//! macOS users reach for ⌘, before they look for a settings button, and expect
//! an app menu with About, Hide and Quit where every other app puts them.
//! Windows and Linux users expect a File menu. Building one menu that carries
//! both is cheaper than explaining why Flume is the exception.
//!
//! Items that open UI do not act directly: they emit [`MENU_EVENT`] with the
//! item's id, and the frontend opens the corresponding surface. The alternative
//! — a Rust-side command that reaches into the webview — would put the same
//! decision in two places, and the frontend already owns which dialog is open.

use tauri::{
    AppHandle, Emitter, Manager, Runtime,
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu},
};

/// Event carrying a menu item's id to the frontend.
///
/// Must match `MENU_EVENT` in `src/hooks/useMenuEvents.ts`.
pub const MENU_EVENT: &str = "flume://menu";

/// Menu item ids the frontend acts on.
pub const OPEN_SETTINGS: &str = "open_settings";
/// Opens the add-torrent flow.
pub const ADD_TORRENT: &str = "add_torrent";

/// Builds and installs the application menu.
///
/// # Errors
///
/// Returns any error from constructing the menu or attaching it to the app.
pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let settings = MenuItem::with_id(
        app,
        OPEN_SETTINGS,
        "Settings…",
        true,
        // The platform's own convention, which Tauri maps per OS.
        Some("CmdOrCtrl+,"),
    )?;
    let add = MenuItem::with_id(app, ADD_TORRENT, "Add Torrent…", true, Some("CmdOrCtrl+N"))?;

    // On macOS the first submenu is the app menu and takes the app's name
    // whatever it is titled here; elsewhere it reads as a normal File menu.
    let app_menu = Submenu::with_items(
        app,
        "Flume",
        true,
        &[
            &PredefinedMenuItem::about(app, None, Some(AboutMetadata::default()))?,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &add,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    // Edit exists for the text fields — search, the magnet input, the proxy
    // URL. Without it ⌘C and ⌘V do nothing in a webview on macOS, which reads
    // as the app being broken rather than as a missing menu.
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;

    let menu = Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu)?;

    app.on_menu_event(|app, event| {
        let id = event.id().as_ref();
        if id != OPEN_SETTINGS && id != ADD_TORRENT {
            return;
        }

        // A menu item that opens a window's UI has to bring that window
        // forward first: on macOS the menu bar works while the window is
        // hidden or behind another app, and emitting into a window nobody can
        // see looks like the menu doing nothing.
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }

        if let Err(err) = app.emit(MENU_EVENT, id) {
            log::warn!("could not deliver the {id} menu event: {err}");
        }
    });

    Ok(())
}
