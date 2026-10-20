//! Knowing that a newer Flume exists, and installing it when the format allows.
//!
//! **The decisions this module encodes** (issue #176, recorded in CLAUDE.md):
//!
//! - **On by default, one switch.** `Settings.check_for_updates` defaults to
//!   `true`, is disclosed on the first-run screen, and is honoured: off means
//!   no scheduled request at all. A manual check from the menu is the user
//!   asking, so it runs whatever the switch says.
//! - **Notify everywhere, replace in place only where the format allows it.**
//!   See [`delivery_for`]. Nothing is ever downloaded or installed without a
//!   click.
//! - **Never while the egress guard holds.** Someone relying on the guard does
//!   not expect Flume to open a connection from the address it is protecting,
//!   torrent traffic or not. A held guard skips scheduled checks and refuses
//!   manual ones (architecture rule 12: fail closed).
//! - **The request carries nothing about the install.** [`ENDPOINT`] has no
//!   template variables, so the version, OS and architecture stay on the
//!   machine and the comparison happens here. What leaves is an HTTPS GET with
//!   the updater plugin's generic user agent; `docs/Privacy.md` says exactly
//!   that.
//!
//! This file holds the rules and the wire types and imports no Tauri types, so
//! it runs under plain `cargo test`. `checker` is the thin part that drives
//! the plugin.

pub mod checker;

use std::{future::Future, ops::RangeInclusive, time::Duration};

use serde::Serialize;

/// Where the release manifest lives.
///
/// GitHub's "latest release" asset URL: it skips drafts and pre-releases, and
/// GitHub keeps it pointing at the newest published release, so nothing has to
/// be redeployed when a release goes out. The repository being public is
/// already load-bearing (SignPath requires it), so this adds no new
/// dependency.
///
/// Baked into every binary, so a later move only reaches installs that updated
/// first. That is why there are no `{{target}}`-style variables in it: once
/// shipped, a URL that reports the install's version and platform on every
/// check cannot be taken back.
pub const ENDPOINT: &str =
    "https://github.com/adamgreenwell/flume/releases/latest/download/latest.json";

/// Where a person goes to download a release by hand.
const RELEASES: &str = "https://github.com/adamgreenwell/flume/releases";

/// How often a running Flume checks.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Seconds after launch before the first scheduled check.
///
/// Jittered like the usage sender's, so launch is not a burst of network
/// activity and installs do not arrive at the same second after a release.
pub const STARTUP_DELAY: RangeInclusive<u64> = 30..=90;

/// How long a check may take before it counts as failed.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether Windows installs update in place.
///
/// Off until the installer is Authenticode-signed. The updater runs the
/// installer it downloads, and an unsigned one raises UAC and SmartScreen on
/// every update -- a prompt that looks like malware arriving, every release.
/// The SignPath Foundation application has been pending since 2026-09-10; when
/// it is granted and the release pipeline signs, flip this and Windows joins
/// the in-place tier. Until then Windows is told and linked, like `.deb`.
pub const WINDOWS_IN_PLACE: bool = false;

/// How the running copy of Flume was installed.
///
/// Flume's own mirror of the bundler's answer, so the rules below can be
/// tested without Tauri. `Unknown` is a development build, or a bundle type
/// this version does not know about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bundle {
    /// macOS `.app`, which is also what a `.dmg` installs.
    App,
    /// Linux AppImage.
    AppImage,
    /// Debian package.
    Deb,
    /// RPM package.
    Rpm,
    /// Windows MSI.
    Msi,
    /// Windows NSIS installer.
    Nsis,
    /// Anything else, including `cargo tauri dev`.
    Unknown,
}

/// How a person gets a newer version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Delivery {
    /// Flume downloads, verifies and installs it, then restarts, on a click.
    InPlace,
    /// Flume says it exists and opens the release page.
    Download,
}

/// Which installs Flume may replace itself in.
///
/// - macOS and AppImage replace in place: the updater swaps the bundle and the
///   result is exactly what a fresh download would have been.
/// - `.deb` and `.rpm` belong to a package manager Flume is not in. Replacing
///   files under it leaves the package database lying about what is installed,
///   so these are told and linked.
/// - Windows waits on signing; see [`WINDOWS_IN_PLACE`].
/// - An unknown bundle is told and linked, because a guess wrong in the other
///   direction installs something over a development checkout.
#[must_use]
pub const fn delivery_for(bundle: Bundle) -> Delivery {
    match bundle {
        Bundle::App | Bundle::AppImage => Delivery::InPlace,
        Bundle::Msi | Bundle::Nsis if WINDOWS_IN_PLACE => Delivery::InPlace,
        Bundle::Msi | Bundle::Nsis | Bundle::Deb | Bundle::Rpm | Bundle::Unknown => {
            Delivery::Download
        }
    }
}

/// What started a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The daily timer.
    Scheduled,
    /// The person chose "Check for Updates…".
    Manual,
}

/// Why a check did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// This build has no updater key, so nothing it found could be verified.
    Unconfigured,
    /// The person turned scheduled checks off.
    Off,
    /// The egress guard is holding.
    Held,
}

/// Whether a check may run now.
///
/// The order is the point. An unconfigured build skips before anything else,
/// because it cannot act on an answer. The guard comes before the switch, so a
/// manual check while held is refused rather than run: the switch is the
/// person's preference about scheduling, and a click overrides that, but the
/// guard is their protection, and a click elsewhere in the app does not.
///
/// # Errors
///
/// Returns the reason when the check must not run.
pub const fn may_check(
    trigger: Trigger,
    configured: bool,
    enabled: bool,
    held: bool,
) -> Result<(), Skip> {
    if !configured {
        return Err(Skip::Unconfigured);
    }
    if held {
        return Err(Skip::Held);
    }
    if !enabled && matches!(trigger, Trigger::Scheduled) {
        return Err(Skip::Off);
    }
    Ok(())
}

/// Runs `operation` until it finishes or `stop` resolves, whichever is first.
///
/// `None` means `stop` won, and `operation` has been dropped -- which for an
/// HTTP request is what ends the transfer. The checker passes "the egress
/// guard is holding" as `stop`, so a download cannot outlive the tunnel it
/// started under. Generic, and here rather than in `checker`, so it can be
/// tested without Tauri.
pub async fn abandon_when<T>(
    operation: impl Future<Output = T>,
    stop: impl Future<Output = ()>,
) -> Option<T> {
    tokio::select! {
        out = operation => Some(out),
        () = stop => None,
    }
}

/// The status to publish when the guard stopped or prevented an operation.
///
/// What Flume knew before still stands if it was an announced update -- the
/// banner, which knows the guard is holding, explains why Install waits.
/// Anything else becomes [`UpdateStatus::Held`].
#[must_use]
pub fn after_hold(before: &UpdateStatus) -> UpdateStatus {
    match before {
        UpdateStatus::Available { .. } => before.clone(),
        _ => UpdateStatus::Held,
    }
}

/// Whether a scheduled check should be tried again as soon as the guard
/// releases, rather than at the next daily tick.
///
/// Only a check the guard prevented. A network failure waits for the next
/// tick like anything else: retrying it in a loop would hammer GitHub from a
/// machine that is simply offline.
#[must_use]
pub const fn retry_when_released(status: &UpdateStatus) -> bool {
    matches!(status, UpdateStatus::Held)
}

/// Whether a build carries an updater public key.
///
/// The plugin refuses to start with no `plugins.updater` section, so every
/// build has one; an empty `pubkey` is how a build that cannot verify updates
/// says so. Forks, CI and development builds are in that state, and the right
/// thing for them is to ask nothing -- an update they cannot verify is not one
/// they can install, and a notice they cannot act on is noise.
#[must_use]
pub fn is_configured(pubkey: &str) -> bool {
    !pubkey.trim().is_empty()
}

/// The page for a release, for the "Download" button.
///
/// Built from a version the plugin has already parsed as semver, so it is
/// digits, dots and at most a pre-release tag -- nothing that could steer the
/// URL off this repository's releases.
#[must_use]
pub fn release_page(version: &str) -> String {
    let well_formed = !version.is_empty()
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'));
    if well_formed {
        format!("{RELEASES}/tag/v{version}")
    } else {
        // A version that is not shaped like one goes to the list instead of
        // into a URL. The plugin's semver parse makes this unreachable today;
        // this keeps it true if that ever changes.
        format!("{RELEASES}/latest")
    }
}

/// Why a check or an install failed.
///
/// A closed list, for the same reason usage failures are: the plugin's errors
/// carry URLs and paths, and the frontend renders this. Each kind has its own
/// sentence in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    /// No answer: offline, DNS, TLS, a proxy, a timeout.
    Network,
    /// An answer that was not a usable release manifest.
    Manifest,
    /// The download did not match its signature. Never installed.
    Signature,
    /// Downloading or installing failed after the check succeeded.
    Install,
}

/// Which step failed.
///
/// The banner needs it: a failed *install* is always shown, because someone
/// clicked Install and silence would look like success, while a failed daily
/// check is not -- an offline laptop would otherwise be told so at every
/// launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureStage {
    /// Asking for the manifest.
    Check,
    /// Downloading or installing, after a click.
    Install,
}

/// What Flume knows about updates right now. Pushed to the frontend on every
/// change and returned by `get_update_status`.
///
/// Mirrored in `src/lib/ipc/types.ts` as a union on `state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateStatus {
    /// This build cannot verify updates, so it never checks.
    Unconfigured,
    /// Nothing checked yet this session.
    Idle,
    /// A check is in flight.
    Checking,
    /// The last check found nothing newer.
    #[serde(rename_all = "camelCase")]
    UpToDate {
        /// When, in Unix seconds.
        checked_at: u64,
    },
    /// A newer version exists.
    #[serde(rename_all = "camelCase")]
    Available {
        /// Its version, without a leading `v`.
        version: String,
        /// Whether a click installs it or opens its page.
        delivery: Delivery,
    },
    /// Downloading and installing, after a click.
    Installing,
    /// The last attempt failed.
    #[serde(rename_all = "camelCase")]
    Failed {
        /// Why.
        reason: FailureKind,
        /// At which step.
        stage: FailureStage,
    },
    /// The guard was holding when a check was due, so none was made.
    ///
    /// Never replaces [`Self::Available`]: a hold says nothing about whether
    /// the announced update still exists, and replacing it would leave the
    /// person no way to install it once the tunnel is back. See
    /// [`after_hold`].
    Held,
}

#[cfg(test)]
// `expect` is right in tests: a failed expectation is the diagnostic.
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn only_formats_that_can_be_replaced_update_in_place() {
        assert_eq!(delivery_for(Bundle::App), Delivery::InPlace);
        assert_eq!(delivery_for(Bundle::AppImage), Delivery::InPlace);
        for bundle in [Bundle::Deb, Bundle::Rpm, Bundle::Unknown] {
            assert_eq!(delivery_for(bundle), Delivery::Download, "{bundle:?}");
        }
    }

    #[test]
    fn windows_follows_the_signing_switch() {
        let expected = if WINDOWS_IN_PLACE {
            Delivery::InPlace
        } else {
            Delivery::Download
        };
        assert_eq!(delivery_for(Bundle::Msi), expected);
        assert_eq!(delivery_for(Bundle::Nsis), expected);
    }

    #[test]
    fn an_unconfigured_build_never_checks() {
        for trigger in [Trigger::Scheduled, Trigger::Manual] {
            for enabled in [true, false] {
                for held in [true, false] {
                    assert_eq!(
                        may_check(trigger, false, enabled, held),
                        Err(Skip::Unconfigured)
                    );
                }
            }
        }
    }

    #[test]
    fn a_held_guard_refuses_even_a_manual_check() {
        assert_eq!(
            may_check(Trigger::Manual, true, true, true),
            Err(Skip::Held)
        );
        assert_eq!(
            may_check(Trigger::Manual, true, false, true),
            Err(Skip::Held)
        );
        assert_eq!(
            may_check(Trigger::Scheduled, true, true, true),
            Err(Skip::Held)
        );
    }

    #[test]
    fn the_switch_stops_scheduled_checks_but_not_a_click() {
        assert_eq!(
            may_check(Trigger::Scheduled, true, false, false),
            Err(Skip::Off)
        );
        assert_eq!(may_check(Trigger::Manual, true, false, false), Ok(()));
        assert_eq!(may_check(Trigger::Scheduled, true, true, false), Ok(()));
    }

    /// Records when it is dropped, standing in for an in-flight request.
    struct Transfer(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Transfer {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn a_hold_abandons_a_transfer_in_flight() {
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let transfer = Transfer(std::sync::Arc::clone(&dropped));
        let download = async move {
            let _held_open = transfer;
            tokio::time::sleep(Duration::from_secs(30)).await;
            "finished"
        };
        let guard_holds = tokio::time::sleep(Duration::from_millis(10));

        let outcome =
            tokio::time::timeout(Duration::from_secs(5), abandon_when(download, guard_holds))
                .await
                .expect("the hold ends it promptly, not when the download would have");

        assert_eq!(outcome, None);
        assert!(
            dropped.load(std::sync::atomic::Ordering::SeqCst),
            "the transfer must be dropped, which is what closes the connection"
        );
    }

    #[tokio::test]
    async fn a_finished_transfer_is_kept() {
        let outcome = abandon_when(async { "finished" }, std::future::pending()).await;
        assert_eq!(outcome, Some("finished"));
    }

    #[test]
    fn a_hold_never_hides_an_announced_update() {
        let available = UpdateStatus::Available {
            version: "1.5.1".to_owned(),
            delivery: Delivery::InPlace,
        };
        assert_eq!(after_hold(&available), available);
        for before in [
            UpdateStatus::Idle,
            UpdateStatus::Checking,
            UpdateStatus::UpToDate { checked_at: 1 },
            UpdateStatus::Failed {
                reason: FailureKind::Network,
                stage: FailureStage::Check,
            },
        ] {
            assert_eq!(after_hold(&before), UpdateStatus::Held, "{before:?}");
        }
    }

    #[test]
    fn only_a_guard_skip_is_retried_on_release() {
        assert!(retry_when_released(&UpdateStatus::Held));
        for status in [
            UpdateStatus::Idle,
            UpdateStatus::UpToDate { checked_at: 1 },
            UpdateStatus::Failed {
                reason: FailureKind::Network,
                stage: FailureStage::Check,
            },
            UpdateStatus::Available {
                version: "1.5.1".to_owned(),
                delivery: Delivery::InPlace,
            },
        ] {
            assert!(!retry_when_released(&status), "{status:?}");
        }
    }

    #[test]
    fn an_empty_key_is_not_a_key() {
        assert!(!is_configured(""));
        assert!(!is_configured("  \n"));
        assert!(is_configured("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWdu"));
    }

    #[test]
    fn the_endpoint_carries_nothing_about_the_install() {
        assert!(ENDPOINT.starts_with("https://"));
        assert!(
            !ENDPOINT.contains("{{"),
            "a template variable sends the install's details on every check"
        );
    }

    /// The plugin reads its endpoint and key from `tauri.conf.json`; this
    /// module reads [`ENDPOINT`]. Two copies, so pinned together -- and the
    /// downgrade protection pinned on, since nothing else would notice it
    /// going missing.
    #[test]
    fn the_bundled_config_agrees_with_this_module() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json"))
            .expect("tauri.conf.json parses");
        let updater = &conf["plugins"]["updater"];
        assert_eq!(updater["endpoints"], serde_json::json!([ENDPOINT]));
        assert_eq!(
            updater["requireSignedVersion"], true,
            "without it a forged manifest can pair a new version number with an old signed build"
        );
        assert!(
            updater["pubkey"].is_string(),
            "the plugin refuses to start without a pubkey key, even an empty one"
        );
        assert_ne!(
            updater["dangerousInsecureTransportProtocol"], true,
            "update checks are HTTPS only"
        );
    }

    #[test]
    fn a_release_page_stays_on_this_repository() {
        assert_eq!(
            release_page("1.5.0"),
            "https://github.com/adamgreenwell/flume/releases/tag/v1.5.0"
        );
        assert_eq!(
            release_page("2.0.0-rc.1"),
            "https://github.com/adamgreenwell/flume/releases/tag/v2.0.0-rc.1"
        );
        for hostile in ["", "1.0/../../../evil", "1.0?x=1", "1.0#f", "1 .0"] {
            assert_eq!(
                release_page(hostile),
                "https://github.com/adamgreenwell/flume/releases/latest",
                "{hostile:?}"
            );
        }
    }
}
