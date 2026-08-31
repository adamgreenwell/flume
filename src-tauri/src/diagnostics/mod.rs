//! A redacted diagnostics bundle the user can paste into a bug report.
//!
//! Like [`crate::settings`] and [`crate::engine`], this module imports no
//! Tauri types: it takes plain values in and returns a `String`, so it is
//! testable under a plain `cargo test`.
//!
//! # Why this exists
//!
//! Flume is developed on one machine and runs on three platforms. When
//! something breaks for someone else, the only channel is an issue written
//! from memory. This turns "it doesn't work" into a paste that says which
//! port bound, how many DHT nodes were found, and what the log said — without
//! sending anything anywhere. The user copies it; nothing phones home.
//!
//! # What redaction can and cannot do
//!
//! Patterns catch what has a shape: URLs, magnet links, addresses, info
//! hashes, home directories. Torrent *names* have no shape, so they are
//! redacted by literal match against the names currently in the library —
//! which means a log line naming a torrent the user has since removed can
//! survive. That is why [`Report::render`] ships only a short tail of the
//! current session's log, and why the UI shows the bundle before copying it
//! rather than after.
//!
//! Over-redaction is the safe failure and is chosen deliberately: a git SHA
//! looks exactly like an info hash, and losing one from a bug report costs
//! nothing next to leaking what someone downloaded.

use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;

use crate::{engine::CoreStatus, settings::Settings, usage::Delivery};

/// How many lines of log tail a bundle carries.
///
/// Enough to cover a session's startup and the failure that prompted the
/// report; short enough that a user can actually read it before pasting, which
/// is the point of showing it to them.
pub const LOG_TAIL_LINES: usize = 200;

/// Compiles a literal pattern from this file.
///
/// A malformed literal regex cannot be recovered from and is a programming
/// error this module's own tests catch on the first run.
#[allow(clippy::expect_used)]
fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("literal regex")
}

/// Pattern-based redactions and their replacements, applied in order.
///
/// Order matters. Magnet links and URLs go first because they *contain* the
/// things the later rules look for — a magnet URI carries both an info hash
/// and, in `dn=`, the torrent's name.
///
/// Lazily built rather than a plain `static`: a compiled `Regex` has interior
/// mutability (it caches match state), which a `static` of borrowed
/// temporaries cannot hold.
static RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        // Carries the info hash in `xt=` and very often the torrent name in `dn=`.
        (compile(r"(?i)magnet:\?[^\s\x22'<>]*"), "<magnet>"),
        // Tracker announces, the proxy, anything else with a scheme.
        (
            compile(r"\b[a-zA-Z][a-zA-Z0-9+.\-]*://[^\s\x22'<>\]]+"),
            "<url>",
        ),
        // Home directories, for paths that are not the two we know literally.
        (compile(r"(?i)[a-z]:\\Users\\[^\\\s\x22'<>]+"), "<home>"),
        (compile(r"/(?:Users|home)/[^/\s\x22'<>:]+"), "<home>"),
        // Info hashes: v1 hex, and the base32 form magnet links use.
        (compile(r"\b[0-9a-fA-F]{40}\b"), "<info-hash>"),
        (compile(r"\b[A-Z2-7]{32}\b"), "<info-hash>"),
        // Peer addresses. IPv6 first: bracketed (how Rust formats a v6 socket
        // address), then the `::` compressed form, then the full eight-group
        // form.
        //
        // The compressed and full patterns are deliberately strict. A looser
        // one matches `12:00:00` in a log timestamp and `librqbit::session` in
        // a module path, which would redact the bundle into uselessness.
        // Bracketed, but only when the contents actually look like IPv6: every
        // character of a `[12:00:00]` log timestamp is hex-or-colon too, so a
        // permissive bracket rule redacts every timestamp in the bundle.
        // Either the `::` marker or a full eight-group address is required.
        (
            compile(r"\[[0-9a-fA-F:.]*::[0-9a-fA-F:.]*\](?::\d{1,5})?"),
            "<ip>",
        ),
        (
            compile(r"\[(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\](?::\d{1,5})?"),
            "<ip>",
        ),
        (
            compile(
                r"\b(?:[0-9a-fA-F]{1,4}:){1,7}:(?:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4}){0,6})?",
            ),
            "<ip>",
        ),
        (
            compile(r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b"),
            "<ip>",
        ),
        (compile(r"\b\d{1,3}(?:\.\d{1,3}){3}(?::\d{1,5})?\b"), "<ip>"),
    ]
});

/// Removes anything that identifies the user or what they are downloading.
///
/// Built once per bundle from the values this install actually holds, so the
/// literal replacements can be labelled usefully — a path becomes
/// `<download-dir>` rather than a generic `<path>`.
pub struct Redactor {
    /// Literal replacements, longest-first so a prefix cannot shadow a longer
    /// match. Applied before the patterns.
    literals: Vec<(String, &'static str)>,
}

impl Redactor {
    /// Builds a redactor for one install.
    ///
    /// `torrent_names` should be every name currently in the library. Names
    /// are matched literally, which is the only way to catch them — see the
    /// module docs for what that misses.
    pub fn new(home: Option<&Path>, download_dir: &Path, torrent_names: &[String]) -> Self {
        let mut literals: Vec<(String, &'static str)> = Vec::new();

        // The download directory before the home directory: it usually lives
        // *inside* home, and replacing home first would leave a bare
        // `<home>/Media/Linux` that still names the folder.
        let download = download_dir.display().to_string();
        if !download.is_empty() {
            literals.push((download, "<download-dir>"));
        }
        if let Some(home) = home {
            let home = home.display().to_string();
            if !home.is_empty() {
                literals.push((home, "<home>"));
            }
        }
        for name in torrent_names {
            // A one- or two-character name matches far too much to be worth
            // replacing, and cannot identify anything on its own.
            if name.chars().count() > 2 {
                literals.push((name.clone(), "<torrent>"));
            }
        }

        // Longest first: with `/Users/adam` and `/Users/adam/Downloads` both
        // present, the shorter one would otherwise win and leave the rest of
        // the path behind.
        literals.sort_by_key(|a| std::cmp::Reverse(a.0.len()));

        Self { literals }
    }

    /// A redactor with no literals, for text with no install context.
    pub fn patterns_only() -> Self {
        Self {
            literals: Vec::new(),
        }
    }

    /// Applies every literal and then every pattern.
    pub fn apply(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (needle, replacement) in &self.literals {
            if out.contains(needle.as_str()) {
                out = out.replace(needle.as_str(), replacement);
            }
        }
        for (pattern, replacement) in RULES.iter() {
            out = pattern.replace_all(&out, *replacement).into_owned();
        }
        out
    }
}

/// Everything a bundle is rendered from.
///
/// Assembled by the command handler, which is the only part that needs Tauri.
pub struct Report<'a> {
    /// Flume's own version, from `CARGO_PKG_VERSION`.
    pub app_version: &'a str,
    /// Target OS, from `std::env::consts::OS`.
    pub os: &'a str,
    /// Target architecture, from `std::env::consts::ARCH`.
    pub arch: &'a str,
    /// Whether this is a debug build.
    pub debug_build: bool,
    /// Which commit this binary was built from, or `unknown` without git.
    ///
    /// The commit, not the moment of compilation, and it says nothing about
    /// whether the tree was clean — see `build.rs` for why both of those are
    /// deliberate.
    ///
    /// The version alone cannot tell two builds apart — every build of Flume
    /// says `1.0.0` — so this is what answers "is the binary you are running
    /// the one you think you built?". An installer that picked up a stale
    /// artifact is invisible without it.
    pub build_id: &'a str,
    /// The user's settings, reported as shape rather than value.
    pub settings: &'a Settings,
    /// Engine status, or `None` if the engine has not started.
    pub core: Option<&'a CoreStatus>,
    /// How many torrents are in the library. The count only, never the list.
    pub torrent_count: usize,
    /// The user's home directory, if one could be determined.
    pub home: Option<PathBuf>,
    /// Whether this build has a collector endpoint it can actually use.
    ///
    /// "Usable", not merely "present": an endpoint that is compiled in but
    /// empty or not `https://` sends nothing, and saying it is absent would be
    /// as wrong as saying it works.
    ///
    /// Passed in rather than read from [`crate::usage`] so this module stays a
    /// pure function of its inputs and its tests need no build configuration.
    pub usage_endpoint_configured: bool,
    /// What happened on the most recent send attempt this session.
    pub usage_delivery: Delivery,
    /// The tail of the current session's log, oldest first.
    pub log_tail: &'a [String],
    /// Redacts the log tail and anything else free-form.
    pub redactor: &'a Redactor,
}

impl Report<'_> {
    /// Renders the bundle as markdown, ready to paste into an issue.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(4096);

        out.push_str("# Flume diagnostics\n\n");
        out.push_str(
            "Paths, addresses, URLs, info hashes and torrent names are removed. \
             Read it before you send it.\n\n",
        );

        out.push_str("## Application\n\n");
        line(&mut out, "Version", self.app_version);
        line(&mut out, "Platform", &format!("{} {}", self.os, self.arch));
        line(
            &mut out,
            "Build",
            &format!(
                "{} {}",
                if self.debug_build { "debug" } else { "release" },
                self.build_id
            ),
        );

        out.push_str("\n## Engine\n\n");
        match self.core {
            None => out.push_str("Not started.\n"),
            Some(core) => {
                line(&mut out, "Client", &core.client_version);
                line(&mut out, "Health", &format!("{:?}", core.health));
                line(&mut out, "Uptime", &format!("{} s", core.uptime_seconds));
                line(
                    &mut out,
                    "Listen port",
                    &match core.listen_port {
                        // The configured port matters here: "asked for 42221,
                        // bound 42222" is a different bug from "did not bind".
                        Some(port) if port == self.settings.listen_port => port.to_string(),
                        Some(port) => {
                            format!("{port} (configured {})", self.settings.listen_port)
                        }
                        None => "not listening".to_owned(),
                    },
                );
                line(
                    &mut out,
                    "Announce port",
                    &core
                        .announce_port
                        .map_or_else(|| "none".to_owned(), |p| p.to_string()),
                );
                line(
                    &mut out,
                    "DHT",
                    &if core.dht.enabled {
                        format!(
                            "{} nodes (v4 {}, v6 {}), {} outstanding",
                            core.dht.total_nodes(),
                            core.dht.nodes_v4,
                            core.dht.nodes_v6,
                            core.dht.outstanding_requests
                        )
                    } else {
                        "disabled".to_owned()
                    },
                );
                line(&mut out, "Live peers", &core.live_peers.to_string());
            }
        }
        line(&mut out, "Torrents", &self.torrent_count.to_string());

        out.push_str("\n## Settings\n\n");
        // Reported as shape, not value. "Exists, is a directory, outside the
        // home directory" is both more useful for diagnosis and less
        // revealing than the path itself.
        line(&mut out, "Download folder", &self.describe_download_dir());
        line(
            &mut out,
            "Download limit",
            &describe_limit(self.settings.download_limit_bps),
        );
        line(
            &mut out,
            "Upload limit",
            &describe_limit(self.settings.upload_limit_bps),
        );
        line(&mut out, "DHT", &on_off(self.settings.enable_dht));
        line(&mut out, "UPnP", &on_off(self.settings.enable_upnp));
        line(&mut out, "Proxy", &describe_proxy(&self.settings.proxy_url));
        line(&mut out, "Theme", &format!("{:?}", self.settings.theme));
        line(&mut out, "Density", &format!("{:?}", self.settings.density));
        line(
            &mut out,
            "Usage reporting",
            &describe_usage(
                self.settings.usage_reporting,
                self.usage_endpoint_configured,
                self.usage_delivery,
            ),
        );

        out.push_str("\n## Recent log\n\n");
        if self.log_tail.is_empty() {
            out.push_str("No log lines were available.\n");
        } else {
            out.push_str("```\n");
            for entry in self.log_tail {
                out.push_str(&self.redactor.apply(entry));
                out.push('\n');
            }
            out.push_str("```\n");
        }

        out
    }

    /// Describes the download folder without naming it.
    fn describe_download_dir(&self) -> String {
        let dir = &self.settings.download_dir;
        if dir.as_os_str().is_empty() {
            return "not set".to_owned();
        }

        let mut facts = Vec::new();
        match std::fs::metadata(dir) {
            Ok(meta) if meta.is_dir() => facts.push("exists"),
            // The distinction matters: a path that exists as a *file* fails
            // very differently from one that is simply absent.
            Ok(_) => facts.push("exists but is not a directory"),
            Err(_) => facts.push("missing or unreadable"),
        }
        facts.push(match &self.home {
            Some(home) if dir.starts_with(home) => "inside the home directory",
            Some(_) => "outside the home directory",
            None => "home directory unknown",
        });
        facts.join(", ")
    }
}

/// Appends one `- **Label:** value` line.
fn line(out: &mut String, label: &str, value: &str) {
    out.push_str("- **");
    out.push_str(label);
    out.push_str(":** ");
    out.push_str(value);
    out.push('\n');
}

/// Renders a rate limit, which is `None` when unlimited.
fn describe_limit(bps: Option<u32>) -> String {
    bps.map_or_else(|| "unlimited".to_owned(), |bps| format!("{bps} B/s"))
}

/// Renders a boolean as a word rather than `true`/`false`.
fn on_off(value: bool) -> String {
    if value { "on" } else { "off" }.to_owned()
}

/// Describes usage reporting, including the case where it cannot work.
///
/// Consent and the endpoint are reported as one sentence because either alone
/// misleads. "On" in a build that cannot send describes an app that queues
/// events to disk and sends none of them, with no error anywhere the user can
/// see -- and this line is the one place a bug report would reveal it.
///
/// The endpoint half is *usability*, not presence. An empty or non-`https://`
/// value is compiled in and still cannot work, so a sentence saying no
/// endpoint was compiled in would send someone off to set a variable they
/// already set.
fn describe_usage(consent: Option<bool>, endpoint_configured: bool, delivery: Delivery) -> String {
    match (consent, endpoint_configured) {
        (None, _) => "not asked".to_owned(),
        (Some(false), _) => "off".to_owned(),
        (Some(true), true) => format!("on; {}", describe_delivery(delivery)),
        (Some(true), false) => {
            "ON, BUT THIS BUILD HAS NO USABLE COLLECTOR ENDPOINT -- events are queued \
             and never sent"
                .to_owned()
        }
    }
}

/// Describes the last send attempt as a fact, with no diagnosis attached.
///
/// Deliberately not a verdict. "Delivery looks broken" would be wrong for a
/// laptop that was closed, a machine behind a household DNS blocklist, a
/// firewall rule someone clicked Deny on, or a VPN kill switch — none of them
/// defects, all of them indistinguishable from a dead endpoint at this layer.
/// So a transport failure says only that nothing answered, and names the
/// possibilities rather than picking one.
///
/// A refusal is different: a status code means a server answered, which none
/// of the above can produce, so it is reported plainly.
fn describe_delivery(delivery: Delivery) -> String {
    match delivery {
        Delivery::Untried => "nothing sent yet this session".to_owned(),
        Delivery::Accepted => "last batch accepted".to_owned(),
        Delivery::NoResponse => {
            "last batch got no answer (offline, DNS, TLS or a proxy — not distinguishable \
             from here)"
                .to_owned()
        }
        Delivery::Refused(status) => format!("last batch refused with {status}"),
    }
}

/// Describes a proxy by scheme only.
///
/// Whether a proxy is configured is the diagnostic fact; its host, port and
/// any embedded credentials are not.
fn describe_proxy(url: &Option<String>) -> String {
    match url.as_deref().map(str::trim) {
        None | Some("") => "none".to_owned(),
        Some(url) => match url.split_once("://") {
            Some((scheme, _)) => format!("configured ({scheme})"),
            None => "configured".to_owned(),
        },
    }
}

#[cfg(test)]
// `expect` is right in tests: a failed expectation is the diagnostic.
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn redactor() -> Redactor {
        Redactor::new(
            Some(Path::new("/Users/adam")),
            Path::new("/Users/adam/Media/Linux"),
            &["Ubuntu 24.04 Desktop amd64".to_owned()],
        )
    }

    #[test]
    fn removes_a_magnet_link_whole() {
        // `dn=` carries the torrent's name, so a partially-redacted magnet is
        // still a leak.
        let out = redactor().apply(
            "adding magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=Some+Private+Thing",
        );
        assert_eq!(out, "adding <magnet>");
    }

    #[test]
    fn removes_tracker_urls() {
        let out = redactor().apply("announce to https://tracker.example.org:6969/announce failed");
        assert_eq!(out, "announce to <url> failed");
    }

    #[test]
    fn removes_peer_addresses() {
        assert_eq!(redactor().apply("peer 192.168.1.44:51413"), "peer <ip>");
        assert_eq!(redactor().apply("peer [2001:db8::1]:6881"), "peer <ip>");
        assert_eq!(redactor().apply("peer [::1]:6881"), "peer <ip>");
        assert_eq!(
            redactor().apply("peer [2001:0db8:0000:0000:0000:ff00:0042:8329]:6881"),
            "peer <ip>"
        );
        assert_eq!(redactor().apply("peer 2001:db8::1 sent"), "peer <ip> sent");
    }

    #[test]
    fn removes_info_hashes_in_both_forms() {
        let hex = redactor().apply("torrent 0123456789ABCDEF0123456789abcdef01234567 stalled");
        assert_eq!(hex, "torrent <info-hash> stalled");

        let base32 = redactor().apply("torrent MFRGGZDFMZTWQ2LKNNWG23TPOBYXE43U added");
        assert_eq!(base32, "torrent <info-hash> added");
    }

    #[test]
    fn removes_paths_and_prefers_the_more_specific_label() {
        let out = redactor().apply("writing to /Users/adam/Media/Linux/disc.iso");
        // Not `<home>/Media/Linux/...`, which would still name the folder.
        assert_eq!(out, "writing to <download-dir>/disc.iso");

        let elsewhere = redactor().apply("config at /Users/adam/.config/flume");
        assert_eq!(elsewhere, "config at <home>/.config/flume");
    }

    #[test]
    fn removes_a_home_directory_it_was_not_told_about() {
        // The bundle is built on one machine but a log can mention another
        // user, e.g. after an import from another client.
        let out = Redactor::patterns_only().apply("found /home/someoneelse/.config/deluge");
        assert_eq!(out, "found <home>/.config/deluge");

        let windows = Redactor::patterns_only().apply(r"found C:\Users\Someone\AppData");
        assert_eq!(windows, r"found <home>\AppData");
    }

    #[test]
    fn removes_torrent_names_it_knows() {
        let out = redactor().apply("verifying Ubuntu 24.04 Desktop amd64: 12%");
        assert_eq!(out, "verifying <torrent>: 12%");
    }

    #[test]
    fn keeps_log_timestamps_and_module_paths() {
        // The strict IPv6 patterns exist for this. A looser one eats both, and
        // a bundle with no timestamps or module paths is not worth pasting.
        let line = "[2026-08-31][12:00:00][librqbit::session][INFO] session started";
        assert_eq!(Redactor::patterns_only().apply(line), line);
    }

    #[test]
    fn keeps_version_numbers() {
        let line = "rqbit 9.0.0 starting";
        assert_eq!(Redactor::patterns_only().apply(line), line);
    }

    fn settings() -> Settings {
        Settings {
            download_dir: PathBuf::from("/Users/adam/Media/Linux"),
            proxy_url: Some("socks5://user:hunter2@10.0.0.9:1080".to_owned()),
            ..Settings::default()
        }
    }

    fn report<'a>(settings: &'a Settings, redactor: &'a Redactor, log: &'a [String]) -> Report<'a> {
        Report {
            app_version: "1.0.0",
            os: "macos",
            arch: "aarch64",
            debug_build: false,
            build_id: "2026-08-31 (abc1234)",
            settings,
            core: None,
            torrent_count: 3,
            home: Some(PathBuf::from("/Users/adam")),
            usage_endpoint_configured: true,
            usage_delivery: Delivery::Untried,
            log_tail: log,
            redactor,
        }
    }

    #[test]
    fn a_rendered_bundle_names_nothing_sensitive() {
        let log = [
            "[INFO] adding magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=Some+Private+Thing".to_owned(),
            "[INFO] peer 192.168.1.44:51413 for Ubuntu 24.04 Desktop amd64".to_owned(),
            "[WARN] tracker https://tracker.example.org/announce timed out".to_owned(),
            "[INFO] writing /Users/adam/Media/Linux/disc.iso".to_owned(),
        ];
        let settings = settings();
        let redactor = redactor();
        let rendered = report(&settings, &redactor, &log).render();

        for secret in [
            "Some+Private+Thing",
            "Ubuntu 24.04 Desktop amd64",
            "0123456789abcdef",
            "192.168.1.44",
            "tracker.example.org",
            "/Users/adam",
            // The proxy carries credentials, and is reported by scheme only.
            "hunter2",
            "10.0.0.9",
        ] {
            assert!(
                !rendered.contains(secret),
                "bundle leaked {secret:?}:\n{rendered}"
            );
        }
    }

    #[test]
    fn a_rendered_bundle_still_says_something_useful() {
        let settings = settings();
        let redactor = redactor();
        let rendered = report(&settings, &redactor, &[]).render();

        assert!(rendered.contains("1.0.0"), "no version:\n{rendered}");
        // The version is 1.0.0 for every build ever made, so the commit is the
        // only thing in the report that can tell two binaries apart.
        assert!(
            rendered.contains("release 2026-08-31 (abc1234)"),
            "no build identity:\n{rendered}"
        );
        assert!(rendered.contains("macos aarch64"), "no platform");
        assert!(rendered.contains("configured (socks5)"), "no proxy scheme");
        assert!(rendered.contains("not asked"), "no consent state");
        // The count, never the list.
        assert!(rendered.contains("**Torrents:** 3"), "no torrent count");
    }

    #[test]
    fn a_bundle_says_when_reporting_is_on_in_a_build_that_cannot_send() {
        // The failure this line exists for. Consent is on, events queue to
        // disk, nothing is sent, and no error surfaces anywhere else -- so the
        // bundle has to be the place it becomes visible.
        let settings = Settings {
            usage_reporting: Some(true),
            ..settings()
        };
        let redactor = redactor();
        let mut report = report(&settings, &redactor, &[]);
        report.usage_endpoint_configured = false;

        let rendered = report.render();
        assert!(
            rendered.contains("NO USABLE COLLECTOR ENDPOINT"),
            "a build that cannot send should say so:\n{rendered}"
        );
    }

    #[test]
    fn usage_reporting_is_described_by_consent_and_endpoint_together() {
        let untried = Delivery::Untried;
        assert_eq!(describe_usage(None, true, untried), "not asked");
        assert_eq!(describe_usage(None, false, untried), "not asked");
        // A decline is a decline; the endpoint is irrelevant and mentioning it
        // would imply the setting is not being honoured.
        assert_eq!(describe_usage(Some(false), false, untried), "off");
        assert!(
            describe_usage(Some(true), false, untried).contains("NO USABLE COLLECTOR ENDPOINT")
        );

        // On, and the delivery half is a fact rather than a diagnosis.
        assert_eq!(
            describe_usage(Some(true), true, Delivery::Accepted),
            "on; last batch accepted"
        );
        assert_eq!(
            describe_usage(Some(true), true, Delivery::Refused(404)),
            "on; last batch refused with 404"
        );

        // The load-bearing one: a closed laptop must not be told anything is
        // wrong, and must not be told which of several causes it was.
        let offline = describe_usage(Some(true), true, Delivery::NoResponse);
        assert!(offline.contains("no answer"), "{offline}");
        assert!(offline.contains("not distinguishable"), "{offline}");
        for verdict in ["broken", "BROKEN", "failed", "error"] {
            assert!(
                !offline.contains(verdict),
                "offline should not read as a defect: {offline}"
            );
        }
    }

    #[test]
    fn a_download_folder_is_described_rather_than_named() {
        let settings = settings();
        let redactor = redactor();
        let rendered = report(&settings, &redactor, &[]).render();

        assert!(rendered.contains("inside the home directory"));
        assert!(!rendered.contains("Media/Linux"));
    }
}
