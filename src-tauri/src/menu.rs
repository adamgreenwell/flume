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
/// Checks for a newer version now. Through the frontend, not straight to
/// Rust, so the answer to a check the person asked for is shown to them --
/// the daily check only speaks up when there is something to install.
pub const CHECK_UPDATES: &str = "check_updates";
/// Opens the lifetime statistics view. In Window, where Transmission keeps it.
pub const OPEN_STATISTICS: &str = "open_statistics";

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
    let check_updates =
        MenuItem::with_id(app, CHECK_UPDATES, "Check for Updates…", true, None::<&str>)?;
    let statistics = MenuItem::with_id(app, OPEN_STATISTICS, "Statistics", true, None::<&str>)?;

    let package = app.package_info();
    let about = about_metadata(
        package.name.clone(),
        package.version.to_string(),
        app.config().bundle.copyright.clone(),
    );

    // On macOS the first submenu is the app menu and takes the app's name
    // whatever it is titled here; elsewhere it reads as a normal File menu.
    let app_menu = Submenu::with_items(
        app,
        "Flume",
        true,
        &[
            &PredefinedMenuItem::about(app, None, Some(about))?,
            &check_updates,
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
            &PredefinedMenuItem::separator(app)?,
            &statistics,
        ],
    )?;

    let menu = Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu)?;

    app.on_menu_event(|app, event| {
        let id = event.id().as_ref();
        if ![OPEN_SETTINGS, ADD_TORRENT, CHECK_UPDATES, OPEN_STATISTICS].contains(&id) {
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

/// What the About dialog says.
///
/// Every field has to be supplied, because on Windows and Linux nothing else
/// fills them. muda builds the Windows dialog as a message box whose text is
/// these fields and nothing more, and its title as `About {name}`; Linux's is
/// a GTK `AboutDialog` fed the same way. This used to pass
/// `AboutMetadata::default()`, and both platforms showed an empty box titled
/// `About`. macOS hid it: the standard About panel falls back to the
/// bundle's Info.plist for anything missing, and development is on macOS.
///
/// macOS reads only `name`, `version` and `copyright` of what is set here,
/// and they are the values its Info.plist already carries. `short_version`
/// is left unset so the build number in parentheses still comes from there.
///
/// Name and version come from the package info, so they are what
/// `tauri.conf.json` says, as the rest of the app reports them. The rest is
/// `Cargo.toml`'s, which is where the licence and repository are declared.
fn about_metadata(
    name: String,
    version: String,
    copyright: Option<String>,
) -> AboutMetadata<'static> {
    AboutMetadata {
        name: Some(name),
        version: Some(version),
        // Cargo joins multiple authors with `:`.
        authors: Some(
            env!("CARGO_PKG_AUTHORS")
                .split(':')
                .map(str::to_owned)
                .collect(),
        ),
        comments: Some(env!("CARGO_PKG_DESCRIPTION").to_owned()),
        copyright,
        license: Some(env!("CARGO_PKG_LICENSE").to_owned()),
        website: Some(env!("CARGO_PKG_REPOSITORY").to_owned()),
        ..AboutMetadata::default()
    }
}

#[cfg(test)]
// `expect` is right in tests: a failed expectation is the diagnostic.
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::about_metadata;

    /// The regression this guards: an About dialog with nothing in it on
    /// Windows and Linux. A field that is `None`, or is `Some("")` because
    /// `Cargo.toml` lost the key `env!` reads, renders as nothing.
    #[test]
    fn every_field_the_windows_and_linux_dialogs_render_is_filled() {
        let about = about_metadata(
            "Flume".to_owned(),
            "1.4.1".to_owned(),
            Some("Copyright © 2026 Adam Greenwell".to_owned()),
        );
        for (field, value) in [
            ("name", &about.name),
            ("version", &about.version),
            ("comments", &about.comments),
            ("copyright", &about.copyright),
            ("license", &about.license),
            ("website", &about.website),
        ] {
            assert!(
                value.as_deref().is_some_and(|v| !v.trim().is_empty()),
                "About's {field} is empty: {value:?}"
            );
        }
        let authors = about.authors.expect("About names its authors");
        assert!(!authors.is_empty() && authors.iter().all(|a| !a.trim().is_empty()));
    }

    /// The copyright line is the one field read from `tauri.conf.json`
    /// rather than compiled in, so the test above, which passes it in,
    /// cannot see it go missing. This reads the file the build reads.
    #[test]
    fn the_bundle_declares_a_copyright_line() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json parses");
        let copyright = conf["bundle"]["copyright"].as_str().unwrap_or_default();
        assert!(
            !copyright.trim().is_empty(),
            "bundle.copyright is empty, so About has no copyright line"
        );
    }
}
