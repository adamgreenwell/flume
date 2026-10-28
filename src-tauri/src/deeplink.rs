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
//!
//! # Why links are held until the window asks for them
//!
//! A link that *launches* Flume arrives before anything can hear it, on every
//! platform (#241). On Windows and Linux it is a process argument, routed in
//! `setup` -- before the webview has loaded, let alone subscribed -- and
//! `tauri-plugin-deep-link` announces its own copy during plugin setup, before
//! Flume's listener exists. On macOS it arrives as `RunEvent::Opened`, often
//! *before* setup: tao passes AppKit's openURLs on the moment it is called,
//! ahead of didFinishLaunching, which is what runs setup. So macOS links are
//! routed from the run callback, the one place that hears every event, and not
//! from a listener registered in setup, which missed them. Tauri does not
//! buffer events, so each emit went nowhere and the link was lost.
//!
//! So nothing is emitted until the window has said it is listening. Links
//! wait in an [`Inbox`]; the frontend subscribes first, then calls
//! `take_pending_links`, which marks the inbox open and hands over what it
//! held. Deciding "queue or emit" and opening the inbox happen under one
//! lock, so a link cannot fall between them.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

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

/// How long an identical link counts as the same click.
///
/// A second launch on Windows and Linux is routed twice: once by Flume's
/// single-instance callback, and once by the deep-link plugin, which that
/// plugin's `deep-link` feature also invokes. Both arrive within milliseconds.
/// Long enough to collapse that, short enough that clicking the same link
/// again on purpose still works.
const DUPLICATE_WINDOW: Duration = Duration::from_secs(2);

/// Most links held before the window opens. A launch carries one; this only
/// bounds a pathological flood.
const MAX_HELD: usize = 16;

/// What to do with an offered link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The window is not listening yet; held until it asks.
    Queue,
    /// The window is listening; emit it now.
    Emit,
    /// The same link a moment ago -- the double route of one click.
    Duplicate,
}

/// Links waiting for the window, and whether it is listening.
///
/// Pure, so the hand-off rules are tested without Tauri; [`Links`] is the
/// managed, locked wrapper the app uses.
#[derive(Debug, Default)]
pub struct Inbox {
    open: bool,
    held: Vec<String>,
    last: Option<(String, Instant)>,
}

impl Inbox {
    /// Decides what happens to `link`, holding it if the window is not
    /// listening yet.
    pub fn offer(&mut self, link: &str, now: Instant) -> Delivery {
        if let Some((previous, at)) = &self.last
            && previous == link
            && now.saturating_duration_since(*at) < DUPLICATE_WINDOW
        {
            return Delivery::Duplicate;
        }
        self.last = Some((link.to_owned(), now));

        if self.open {
            Delivery::Emit
        } else {
            if self.held.len() < MAX_HELD {
                self.held.push(link.to_owned());
            }
            Delivery::Queue
        }
    }

    /// Marks the window as listening and returns what was held, oldest first.
    ///
    /// Idempotent: a second call -- a reload, or React running an effect
    /// twice in development -- returns nothing, because everything held was
    /// handed over the first time and later links are emitted directly.
    pub fn open(&mut self) -> Vec<String> {
        self.open = true;
        std::mem::take(&mut self.held)
    }
}

/// The app's [`Inbox`], managed so every route can reach it.
#[derive(Debug, Default)]
pub struct Links(Mutex<Inbox>);

impl Links {
    /// Opens the inbox and drains it; see [`Inbox::open`].
    pub fn take_pending(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|mut inbox| inbox.open())
            .unwrap_or_default()
    }
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

/// Forwards an actionable argument to the UI, or holds it until the UI is
/// listening.
///
/// Silently ignores anything that is not a magnet link or `.torrent` path;
/// a second launch carries the process arguments, most of which are noise.
pub fn route_argument(app: &AppHandle, argument: &str) {
    if !actionable(argument) {
        return;
    }
    let link = argument.trim();

    // Decided under the lock, emitted after it: an `Emit` means the window
    // had already opened the inbox, so it is subscribed and will hear this.
    let delivery = app
        .try_state::<Links>()
        .and_then(|links| {
            links
                .0
                .lock()
                .ok()
                .map(|mut inbox| inbox.offer(link, Instant::now()))
        })
        // No inbox (unreachable once the app is built) or a poisoned lock:
        // emitting is the old behaviour, and better than dropping the link.
        .unwrap_or(Delivery::Emit);

    match delivery {
        Delivery::Emit => {
            if let Err(err) = app.emit(OPEN_MAGNET_EVENT, link) {
                log::warn!("could not deliver a link to the window: {err}");
            }
        }
        // Held for the window, which asks for it once it is listening.
        Delivery::Queue => log::info!("holding an opened link until the window is ready"),
        Delivery::Duplicate => log::debug!("ignoring the second route of one opened link"),
    }
}

/// Routes the links macOS hands over in `RunEvent::Opened`.
///
/// Called from the run callback, which may be before `setup` has run and so
/// before the window exists. That is fine: focusing finds no window and does
/// nothing, and the link is held for the page like any other early link.
pub fn handle_opened<'a>(app: &AppHandle, urls: impl IntoIterator<Item = &'a str>) {
    focus_main_window(app);
    for url in urls {
        route_argument(app, url);
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
#[allow(clippy::expect_used)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{DUPLICATE_WINDOW, Delivery, Inbox, actionable};

    const LINK: &str = "magnet:?xt=urn:btih:abc123";
    const OTHER: &str = "magnet:?xt=urn:btih:def456";

    /// The bug (#241): a link offered before the window listens is held, not
    /// emitted into nothing, and handed over once when the window asks.
    #[test]
    fn a_link_before_the_window_listens_is_held_and_handed_over_once() {
        let mut inbox = Inbox::default();
        let now = Instant::now();
        assert_eq!(inbox.offer(LINK, now), Delivery::Queue);
        assert_eq!(inbox.open(), vec![LINK.to_owned()]);
        assert!(
            inbox.open().is_empty(),
            "handed over once, not on every call"
        );
    }

    #[test]
    fn after_the_window_listens_links_are_emitted_directly() {
        let mut inbox = Inbox::default();
        assert!(inbox.open().is_empty());
        assert_eq!(inbox.offer(LINK, Instant::now()), Delivery::Emit);
        assert!(inbox.open().is_empty(), "an emitted link is not also held");
    }

    #[test]
    fn held_links_keep_their_order() {
        let mut inbox = Inbox::default();
        let now = Instant::now();
        inbox.offer(LINK, now);
        inbox.offer(OTHER, now);
        assert_eq!(inbox.open(), vec![LINK.to_owned(), OTHER.to_owned()]);
    }

    /// Windows and Linux route one click twice when Flume is running.
    #[test]
    fn the_double_route_of_one_click_is_collapsed() {
        let mut inbox = Inbox::default();
        inbox.open();
        let now = Instant::now();
        assert_eq!(inbox.offer(LINK, now), Delivery::Emit);
        assert_eq!(
            inbox.offer(LINK, now + Duration::from_millis(5)),
            Delivery::Duplicate
        );
    }

    #[test]
    fn the_same_link_clicked_again_later_still_works() {
        let mut inbox = Inbox::default();
        inbox.open();
        let now = Instant::now();
        inbox.offer(LINK, now);
        assert_eq!(inbox.offer(LINK, now + DUPLICATE_WINDOW), Delivery::Emit);
    }

    #[test]
    fn a_different_link_is_never_a_duplicate() {
        let mut inbox = Inbox::default();
        inbox.open();
        let now = Instant::now();
        inbox.offer(LINK, now);
        assert_eq!(inbox.offer(OTHER, now), Delivery::Emit);
    }

    #[test]
    fn a_flood_before_the_window_opens_is_bounded() {
        let mut inbox = Inbox::default();
        let now = Instant::now();
        for i in 0..100 {
            inbox.offer(&format!("magnet:?xt=urn:btih:{i}"), now);
        }
        assert_eq!(inbox.open().len(), super::MAX_HELD);
    }

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

    // The shipped Linux desktop entry has to hand the link over. Without a
    // field code GLib falls back to %f, which a magnet: URI cannot fill, so a
    // click before Flume has ever run -- before `register_all` has written a
    // per-user handler -- launched Flume with no link at all (#241).
    #[test]
    fn the_linux_desktop_entry_passes_the_link_to_flume() {
        let template = include_str!("../linux/main.desktop");
        let exec = template
            .lines()
            .find(|line| line.starts_with("Exec="))
            .expect("the template has an Exec line");
        assert!(exec.ends_with(" %u"), "Exec must carry %u, got {exec:?}");
        assert!(template.contains("MimeType={{mime_type}}"));

        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid config");
        for package in ["deb", "rpm"] {
            assert_eq!(
                config["bundle"]["linux"][package]["desktopTemplate"], "linux/main.desktop",
                "the .{package} must use the template"
            );
        }
    }
}
