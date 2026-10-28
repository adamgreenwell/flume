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

/// What to do about the `magnet:` handler on Linux at launch.
///
/// Pure, so the rules are tested without a desktop; [`settle_linux_handler`]
/// carries them out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxHandlerPlan {
    /// Write Flume's own per-user entry, as `register_all` does.
    pub register_own: bool,
    /// Strip the per-user entry an earlier version wrote.
    pub remove_own: bool,
    /// Make the package's own entry the default `magnet:` handler.
    pub default_to_packaged: bool,
}

/// Decides how Flume claims `magnet:` on Linux.
///
/// A `.deb` or `.rpm` ships a desktop entry that handles `magnet:` itself, so
/// the per-user one `register_all` writes is a duplicate: the desktop's "Open
/// With" chooser listed Flume twice. Only an AppImage, or a development build,
/// has no packaged entry and needs Flume to write its own.
///
/// The default moves to the packaged entry only when nothing else holds it, or
/// when Flume's own per-user entry did -- never away from a client the person
/// chose. `register_all` used to reclaim the default at every launch.
///
/// `current_default` is what `xdg-mime query default` reports, and `own_entry`
/// the per-user entry's file name. `own_entry_claims` says whether that file
/// still lists `magnet:` -- `unregister` strips the claim and leaves the file,
/// so its mere existence would repeat the removal at every launch.
#[must_use]
pub fn linux_handler_plan(
    packaged: bool,
    current_default: Option<&str>,
    own_entry: &str,
    own_entry_claims: bool,
) -> LinuxHandlerPlan {
    if !packaged {
        return LinuxHandlerPlan {
            register_own: true,
            remove_own: false,
            default_to_packaged: false,
        };
    }
    let unclaimed = current_default.is_none_or(|name| name.trim().is_empty());
    let held_by_own = current_default.is_some_and(|name| name.trim() == own_entry);
    LinuxHandlerPlan {
        register_own: false,
        remove_own: own_entry_claims,
        default_to_packaged: unclaimed || held_by_own,
    }
}

/// Settles the `magnet:` handler on Linux; see [`linux_handler_plan`].
///
/// Failures are logged, not returned: a launch is never worth failing over a
/// handler, and each step is retried on the next launch.
#[cfg(target_os = "linux")]
pub fn settle_linux_handler(app: &AppHandle) {
    use std::process::Command;

    use tauri::utils::{config::BundleType, platform};
    use tauri_plugin_deep_link::DeepLinkExt;

    const MIME: &str = "x-scheme-handler/magnet";

    let packaged = matches!(
        platform::bundle_type(),
        Some(BundleType::Deb | BundleType::Rpm)
    );
    let own_entry = platform::current_exe()
        .ok()
        .and_then(|exe| {
            exe.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .map(|name| format!("{name}-handler.desktop"));
    let own_entry_claims = own_entry.as_ref().is_some_and(|name| {
        app.path().data_dir().is_ok_and(|dir| {
            std::fs::read_to_string(dir.join("applications").join(name))
                .is_ok_and(|entry| entry.contains(MIME))
        })
    });
    let current_default = Command::new("xdg-mime")
        .args(["query", "default", MIME])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned());

    let plan = linux_handler_plan(
        packaged,
        current_default.as_deref(),
        own_entry.as_deref().unwrap_or_default(),
        own_entry_claims,
    );

    if plan.register_own {
        if let Err(err) = app.deep_link().register_all() {
            log::warn!("could not register Flume as the magnet: handler ({err})");
        }
        return;
    }
    if plan.remove_own
        && let Err(err) = app.deep_link().unregister("magnet")
    {
        log::warn!("could not remove Flume's duplicate magnet: handler entry ({err})");
    }
    if plan.default_to_packaged {
        // The packaged entry is named for the product, as the bundler names it.
        let packaged_entry = format!("{}.desktop", app.package_info().name);
        match Command::new("xdg-mime")
            .args(["default", &packaged_entry, MIME])
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(status) => {
                log::warn!("xdg-mime could not make Flume the magnet: handler ({status})")
            }
            Err(err) => {
                log::warn!("could not run xdg-mime to make Flume the magnet: handler ({err})")
            }
        }
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

    const OWN: &str = "flume-app-handler.desktop";

    #[test]
    fn an_appimage_or_dev_build_writes_its_own_entry() {
        let plan = super::linux_handler_plan(false, None, OWN, false);
        assert!(plan.register_own);
        assert!(!plan.remove_own && !plan.default_to_packaged);
    }

    #[test]
    fn a_fresh_package_install_claims_an_unclaimed_default() {
        for default in [None, Some(""), Some("  ")] {
            let plan = super::linux_handler_plan(true, default, OWN, false);
            assert!(!plan.register_own, "the package ships its own entry");
            assert!(!plan.remove_own, "there is no per-user entry to remove");
            assert!(plan.default_to_packaged, "nothing else holds {default:?}");
        }
    }

    // The upgrade case: an earlier version wrote the per-user entry and made it
    // the default, which is what put Flume twice in the "Open With" chooser.
    #[test]
    fn an_upgraded_package_install_moves_the_default_off_the_duplicate() {
        let plan = super::linux_handler_plan(true, Some(OWN), OWN, true);
        assert!(!plan.register_own);
        assert!(plan.remove_own);
        assert!(plan.default_to_packaged);
    }

    #[test]
    fn another_client_chosen_as_the_default_is_left_alone() {
        let plan = super::linux_handler_plan(true, Some("transmission-gtk.desktop"), OWN, true);
        assert!(plan.remove_own, "the duplicate still goes");
        assert!(!plan.default_to_packaged, "the person's choice stands");
    }

    #[test]
    fn a_default_already_on_the_packaged_entry_needs_nothing() {
        let plan = super::linux_handler_plan(true, Some("Flume.desktop"), OWN, false);
        assert_eq!(
            plan,
            super::LinuxHandlerPlan {
                register_own: false,
                remove_own: false,
                default_to_packaged: false,
            }
        );
    }
}
