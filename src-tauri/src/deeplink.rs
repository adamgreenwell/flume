//! OS integration for `magnet:` links and second launches.
//!
//! # Why single-instance matters here
//!
//! Two Flume processes would both try to bind the listen port and write the
//! same session directory. The second would fail with an opaque bind error, or
//! worse, both would write torrent state over each other. So a second launch
//! must hand its arguments to the running instance and exit, not start a rival
//! engine.
//!
//! Clicking a magnet link in a browser *is* a second launch, which is why the
//! two features belong together.

use tauri::{AppHandle, Emitter, Manager};

/// Event carrying a magnet URI to the UI, which opens the add dialog with it.
///
/// Changing this breaks `src/hooks/useMagnetLinks.ts`; change both together.
pub const OPEN_MAGNET_EVENT: &str = "flume://open-magnet";

/// Recognises the URIs Flume can act on.
///
/// Deliberately narrow: only `magnet:` links and `.torrent` paths, so a stray
/// argument or an unrelated URL scheme cannot be routed into the add flow.
fn actionable(argument: &str) -> bool {
    let lowered = argument.trim().to_lowercase();
    lowered.starts_with("magnet:") || lowered.ends_with(".torrent")
}

/// Brings the main window to the front, restoring it if minimised.
///
/// Called when a second launch happens: the user asked for Flume, so showing
/// them the window they already have is the correct response.
pub fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Forwards an actionable argument to the UI.
///
/// Silently ignores anything that is not a magnet link or `.torrent` path;
/// a second launch carries the process arguments, most of which are noise.
pub fn route_argument(app: &AppHandle, argument: &str) {
    if !actionable(argument) {
        return;
    }
    if let Err(err) = app.emit(OPEN_MAGNET_EVENT, argument.trim()) {
        log::warn!("could not deliver {argument} to the window: {err}");
    }
}

/// Handles a second launch: focus the existing window and act on its arguments.
pub fn handle_second_instance(app: &AppHandle, argv: &[String]) {
    focus_main_window(app);
    // Skip argv[0], the executable path.
    for argument in argv.iter().skip(1) {
        route_argument(app, argument);
    }
}

#[cfg(test)]
mod tests {
    use super::actionable;

    #[test]
    fn magnet_links_are_actionable() {
        assert!(actionable("magnet:?xt=urn:btih:abc123"));
        assert!(actionable("MAGNET:?xt=urn:btih:ABC"));
        assert!(actionable("  magnet:?xt=urn:btih:abc  "));
    }

    #[test]
    fn torrent_paths_are_actionable() {
        assert!(actionable("/Users/x/ubuntu.iso.torrent"));
        assert!(actionable("C:\\Users\\x\\Ubuntu.TORRENT"));
    }

    #[test]
    fn ordinary_arguments_are_ignored() {
        // A second launch carries the whole command line; most of it is noise
        // and none of it should reach the add flow.
        for argument in [
            "",
            "--verbose",
            "/Applications/Flume.app/Contents/MacOS/flume",
            "https://example.com/not-a-torrent",
            "ubuntu.iso",
        ] {
            assert!(!actionable(argument), "{argument} should be ignored");
        }
    }

    #[test]
    fn other_url_schemes_are_ignored() {
        // Guards against a malicious or mistaken handler registration routing
        // arbitrary URLs into the torrent add flow.
        assert!(!actionable("file:///etc/passwd"));
        assert!(!actionable("javascript:alert(1)"));
    }
}
