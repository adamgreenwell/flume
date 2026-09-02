//! User settings and their persistence.
//!
//! Like [`crate::engine`], this module imports no Tauri types, so it is
//! testable in a plain `cargo test` process.
//!
//! # Why a plain JSON file rather than `tauri-plugin-store`
//!
//! The agreed decision was "a JSON file in the app-data directory, not
//! SQLite", and that is exactly what this is. The plugin was not used because
//! its distinguishing feature is letting the *frontend* read and write the
//! store directly — which Flume specifically does not want: settings changes
//! have to pass through Rust so the engine can apply them (rate limits live,
//! port and DHT via a session restart). Doing it here avoids a dependency, a
//! capability permission, and a second source of truth.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::egress::EgressGuard;
use crate::engine::{DEFAULT_LISTEN_PORT, EngineConfig};

/// Filename inside the app-data directory.
const SETTINGS_FILE: &str = "settings.json";

/// UI colour scheme preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follow the operating system.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// How tall a torrent row is drawn.
///
/// Frontend-only, like [`Theme`], and persisted here for the same reason: a
/// preference the user re-sets on every launch is not a preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    /// 58px rows with a second line of detail.
    #[default]
    Comfortable,
    /// 40px rows; the detail line is hidden.
    Compact,
}

/// How wide the sidebar is drawn.
///
/// Frontend-only, like [`Theme`] and [`Density`], and persisted here for the
/// same reason: a preference the user re-sets on every launch is not a
/// preference.
///
/// Two states and no third. The rail collapses to an icon rail, never to zero
/// width: its network footer carries the egress guard's held state, and a rail
/// that can hide it turns a deliberate hold into unexplained silence. See the
/// Phase 4 field notes in `docs/Roadmap.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RailState {
    /// 248px, with the wordmark, search, view names and the network footer.
    #[default]
    Expanded,
    /// 56px: view icons and one status dot.
    Collapsed,
}

/// Everything the user can configure.
///
/// Mirrored in `src/lib/ipc/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Where downloads are written.
    ///
    /// Changing this requires a session restart and affects new torrents;
    /// existing torrents keep their original location.
    pub download_dir: PathBuf,

    /// TCP port for incoming peer connections. Requires a session restart.
    pub listen_port: u16,

    /// Whether to run the DHT. Required for magnet links. Requires a restart.
    pub enable_dht: bool,

    /// Whether to request a UPnP port mapping. Requires a restart.
    pub enable_upnp: bool,

    /// Global download limit in bytes per second; `None` means unlimited.
    ///
    /// Applied live, without a restart.
    pub download_limit_bps: Option<u32>,

    /// Global upload limit in bytes per second; `None` means unlimited.
    ///
    /// Applied live, without a restart.
    pub upload_limit_bps: Option<u32>,

    /// SOCKS5 proxy for outgoing peer connections; `None` connects directly.
    ///
    /// Format: `socks5://[user:password@]host:port`. Requires a session
    /// restart, since the proxy is fixed when the session is constructed.
    pub proxy_url: Option<String>,

    /// UI colour scheme. Frontend-only; persisted here so it survives restarts.
    pub theme: Theme,

    /// Row height in the library. Frontend-only, persisted for the same reason.
    pub density: Density,

    /// Sidebar width. Frontend-only, persisted for the same reason.
    pub rail: RailState,

    /// Whether to require that traffic leaves through a tunnel, and what to
    /// do when it does not.
    ///
    /// Defaults to [`EgressGuard::Off`]. Not because the check is expensive —
    /// it sends nothing and costs a route lookup — but because a general-
    /// purpose client that greets every new user with a warning about their
    /// VPN has made an assumption about what they are downloading. Flume does
    /// not make that assumption; see the project notes on use case.
    pub egress_guard: EgressGuard,

    /// The one interface the user will accept traffic leaving by, or `None` to
    /// accept any tunnel.
    ///
    /// Pinning is the stricter setting and the more brittle one: macOS hands
    /// out `utun` numbers dynamically, so a VPN client that reconnects can
    /// land on a different interface and trip the guard. That is a real
    /// trade-off the user is making knowingly, which is why the default is
    /// `None` and the settings copy says so.
    pub egress_interface: Option<String>,

    /// Whether anonymous usage counts may be sent.
    ///
    /// Three states, not two. `None` means *not yet asked*, which is what the
    /// first-run consent step keys off; a decline is `Some(false)` and must
    /// not be re-asked. Collapsing these into a `bool` would either nag a user
    /// who already said no, or silently treat "never asked" as consent.
    ///
    /// Only `Some(true)` sends anything. See [`crate::usage`].
    pub usage_reporting: Option<bool>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Overwritten by `with_os_defaults`; this keeps `Default` usable in
            // tests and lets `#[serde(default)]` fill in missing fields.
            download_dir: PathBuf::new(),
            listen_port: DEFAULT_LISTEN_PORT,
            enable_dht: true,
            enable_upnp: true,
            download_limit_bps: None,
            upload_limit_bps: None,
            proxy_url: None,
            theme: Theme::System,
            density: Density::Comfortable,
            rail: RailState::Expanded,
            egress_guard: EgressGuard::Off,
            egress_interface: None,
            // Not asked yet. Never defaults to `Some(true)`: consent that the
            // user did not give is not consent.
            usage_reporting: None,
        }
    }
}

/// Failures while loading or saving settings.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// The settings file could not be written.
    #[error("could not save settings to {path}: {source}")]
    Save {
        /// The file Flume tried to write.
        path: String,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// The settings are not valid.
    #[error("{0}")]
    Invalid(String),
}

impl Settings {
    /// Settings with the user's Downloads folder as the destination.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Invalid`] if the platform exposes no home
    /// directory.
    pub fn with_os_defaults() -> Result<Self, SettingsError> {
        let user = directories::UserDirs::new().ok_or_else(|| {
            SettingsError::Invalid("could not determine the home directory".into())
        })?;
        let download_dir = user
            .download_dir()
            .map(PathBuf::from)
            .unwrap_or_else(|| user.home_dir().join("Downloads"));

        Ok(Self {
            download_dir,
            ..Self::default()
        })
    }

    /// Whether a settings file exists in `dir`.
    ///
    /// Its absence is what "first run" means. Checked rather than inferred
    /// from the loaded values, because defaults and a file that happens to
    /// hold the defaults are indistinguishable once loaded.
    #[must_use]
    pub fn exists(dir: &Path) -> bool {
        dir.join(SETTINGS_FILE).exists()
    }

    /// Loads settings from `dir`, falling back to defaults.
    ///
    /// A missing file is normal on first run. A *corrupt* file also falls back
    /// to defaults rather than refusing to start: a user who cannot launch the
    /// app cannot fix their settings from inside it. The corruption is
    /// returned so the caller can log it.
    ///
    /// # Errors
    ///
    /// Never fails; the second tuple element describes any problem found.
    pub fn load(dir: &Path) -> (Self, Option<String>) {
        let path = dir.join(SETTINGS_FILE);
        let defaults = Self::with_os_defaults().unwrap_or_default();

        /// Defaults, except that the tunnel guard fails *closed*.
        ///
        /// Every other setting can fall back to its default harmlessly: a lost
        /// rate limit is an uncapped download, a lost theme is the system
        /// theme. The guard is the one field whose default is the unsafe
        /// value. A user who set `Hold`, whose settings file then became
        /// unreadable, would silently start transferring outside their tunnel
        /// — the exact outcome they turned it on to prevent, arrived at by a
        /// mechanism they cannot see.
        ///
        /// So a settings file that *exists* and cannot be used means Hold. The
        /// cost is that someone who never enabled the guard and whose file
        /// corrupts finds transfer held; that is visible, recoverable in one
        /// click, and safe, which is the right way round for this trade.
        fn failing_closed(defaults: &Settings) -> Settings {
            Settings {
                egress_guard: EgressGuard::Hold,
                ..defaults.clone()
            }
        }

        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            // Missing file on first run is expected, not a problem — and not a
            // reason to hold, since there is no prior choice to protect.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (defaults, None),
            Err(e) => {
                return (
                    failing_closed(&defaults),
                    Some(format!(
                        "could not read {}: {e}. Transfer is held until you \
                         confirm how the tunnel check should behave.",
                        path.display()
                    )),
                );
            }
        };

        match serde_json::from_str::<Self>(&raw) {
            Ok(settings) => match settings.validate() {
                Ok(()) => (settings, None),
                // Parsing succeeded, so the user's guard choice is known even
                // though something else in the file is unusable. Honour it
                // rather than overriding a decision that was read correctly.
                Err(e) => (
                    Settings {
                        egress_guard: settings.egress_guard,
                        egress_interface: settings.egress_interface,
                        ..defaults
                    },
                    Some(format!("settings were invalid: {e}")),
                ),
            },
            Err(e) => (
                failing_closed(&defaults),
                Some(format!(
                    "settings file was not valid JSON: {e}. Transfer is held \
                     until you confirm how the tunnel check should behave."
                )),
            ),
        }
    }

    /// Writes settings to `dir`, creating it if needed.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Save`] if the directory or file cannot be
    /// written, or [`SettingsError::Invalid`] if the settings fail validation.
    pub fn save(&self, dir: &Path) -> Result<(), SettingsError> {
        self.validate()?;

        std::fs::create_dir_all(dir).map_err(|source| SettingsError::Save {
            path: dir.display().to_string(),
            source,
        })?;

        let path = dir.join(SETTINGS_FILE);
        // Pretty-printed on purpose: this file is meant to be readable and
        // hand-editable when debugging.
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| SettingsError::Invalid(e.to_string()))?;

        std::fs::write(&path, json).map_err(|source| SettingsError::Save {
            path: path.display().to_string(),
            source,
        })
    }

    /// Checks values that would otherwise fail confusingly later.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Invalid`] with a message suitable for display.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.download_dir.as_os_str().is_empty() {
            return Err(SettingsError::Invalid(
                "the download folder cannot be empty".into(),
            ));
        }
        // Port 0 means "any free port" to the OS, which would silently ignore a
        // user who typed it expecting something specific.
        if self.listen_port == 0 {
            return Err(SettingsError::Invalid(
                "the listen port must be between 1 and 65535".into(),
            ));
        }
        for (label, limit) in [
            ("download", self.download_limit_bps),
            ("upload", self.upload_limit_bps),
        ] {
            if limit == Some(0) {
                return Err(SettingsError::Invalid(format!(
                    "a {label} limit of 0 would stop all transfer; leave it empty for unlimited"
                )));
            }
        }
        if let Some(proxy) = self.proxy_url.as_deref() {
            let proxy = proxy.trim();
            if proxy.is_empty() {
                return Err(SettingsError::Invalid(
                    "leave the proxy empty to connect directly, or give a full socks5:// URL"
                        .into(),
                ));
            }
            // Checked here rather than letting the session fail to start.
            // A bad proxy URL otherwise surfaces as a generic engine failure
            // several seconds later, with nothing pointing at the cause.
            if !proxy.starts_with("socks5://") && !proxy.starts_with("socks5h://") {
                return Err(SettingsError::Invalid(format!(
                    "the proxy must be a socks5:// or socks5h:// URL, not {proxy:?}"
                )));
            }
            if proxy
                .trim_start_matches("socks5h://")
                .trim_start_matches("socks5://")
                .is_empty()
            {
                return Err(SettingsError::Invalid(
                    "the proxy URL is missing a host and port".into(),
                ));
            }
        }
        if let Some(interface) = self.egress_interface.as_deref() {
            // Same reasoning as the proxy: `None` means "any tunnel", and
            // silently treating a half-cleared field as `None` would hide a
            // typo behind a guard that then accepts anything.
            if interface.trim().is_empty() {
                return Err(SettingsError::Invalid(
                    "leave the interface empty to accept any tunnel, or name one".into(),
                ));
            }
            // Not checked for existence. An interface that is absent right now
            // is the normal state of a VPN that is not connected yet, and
            // refusing to save the setting until the tunnel is up would make
            // it impossible to configure the guard before travelling.
        }
        Ok(())
    }

    /// Builds the engine configuration these settings imply.
    ///
    /// `session_dir` is not user-facing, so it is supplied by the caller.
    pub fn to_engine_config(&self, session_dir: PathBuf) -> EngineConfig {
        EngineConfig {
            download_dir: self.download_dir.clone(),
            session_dir,
            listen_port: self.listen_port,
            enable_dht: self.enable_dht,
            enable_upnp: self.enable_upnp,
            proxy_url: self.proxy_url.clone(),
        }
    }

    /// Whether moving from `self` to `next` requires restarting the session.
    ///
    /// Rate limits and the theme apply live; everything the session is
    /// constructed with does not.
    pub fn requires_restart(&self, next: &Self) -> bool {
        self.download_dir != next.download_dir
            || self.listen_port != next.listen_port
            || self.enable_dht != next.enable_dht
            || self.enable_upnp != next.enable_upnp
            // The proxy is fixed when the session is constructed.
            || self.proxy_url != next.proxy_url
    }
}

/// The OS-conventional directory for Flume's settings and session state.
///
/// Falls back to the current directory if the platform exposes no data
/// directory, which only happens on a misconfigured system with no `HOME`.
pub fn session_directory() -> PathBuf {
    directories::ProjectDirs::from("io.github", "adamgreenwell", "Flume")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
// `expect` is right in tests: a failed expectation is the diagnostic.
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn valid() -> Settings {
        Settings {
            download_dir: PathBuf::from("/tmp/downloads"),
            ..Settings::default()
        }
    }

    #[test]
    fn round_trips_through_disk() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let mut settings = valid();
        settings.upload_limit_bps = Some(1024);
        settings.theme = Theme::Dark;

        settings.save(tmp.path()).expect("save");
        let (loaded, problem) = Settings::load(tmp.path());

        assert_eq!(loaded, settings);
        assert!(problem.is_none());
    }

    #[test]
    fn missing_file_is_not_a_problem() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let (_, problem) = Settings::load(tmp.path());
        assert!(problem.is_none(), "first run should not report a problem");
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults_instead_of_refusing_to_start() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        std::fs::write(tmp.path().join(SETTINGS_FILE), "{ not json").expect("write");

        let (loaded, problem) = Settings::load(tmp.path());

        assert_eq!(loaded.listen_port, DEFAULT_LISTEN_PORT);
        assert!(problem.is_some(), "corruption should be reported");
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        // Forward and backward compatibility: an older build must not choke on
        // a field a newer one wrote, and vice versa.
        let tmp = tempfile::TempDir::new().expect("tmp");
        std::fs::write(
            tmp.path().join(SETTINGS_FILE),
            r#"{"downloadDir":"/tmp/x","somethingNew":42}"#,
        )
        .expect("write");

        let (loaded, problem) = Settings::load(tmp.path());

        assert_eq!(loaded.download_dir, PathBuf::from("/tmp/x"));
        assert_eq!(loaded.listen_port, DEFAULT_LISTEN_PORT);
        assert!(problem.is_none());
    }

    #[test]
    fn rejects_port_zero() {
        let settings = Settings {
            listen_port: 0,
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn rejects_a_zero_rate_limit() {
        let settings = Settings {
            upload_limit_bps: Some(0),
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn rejects_an_empty_download_dir() {
        let settings = Settings {
            download_dir: PathBuf::new(),
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn accepts_a_socks5_proxy() {
        for url in [
            "socks5://127.0.0.1:1080",
            "socks5h://proxy.example:9050",
            "socks5://user:pass@10.0.0.1:1080",
        ] {
            let settings = Settings {
                proxy_url: Some(url.into()),
                ..valid()
            };
            assert!(settings.validate().is_ok(), "{url} should be accepted");
        }
    }

    #[test]
    fn rejects_a_proxy_that_is_not_socks5() {
        // http:// proxies are not what librqbit routes peer traffic over, and
        // accepting one would fail later as an opaque engine start failure.
        for url in ["http://proxy:8080", "proxy:1080", "1.2.3.4:1080"] {
            let settings = Settings {
                proxy_url: Some(url.into()),
                ..valid()
            };
            assert!(settings.validate().is_err(), "{url} should be rejected");
        }
    }

    #[test]
    fn rejects_an_empty_proxy_string() {
        // None means direct. An empty string is a half-cleared field, and
        // silently treating it as None would hide a typo.
        let settings = Settings {
            proxy_url: Some("   ".into()),
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn rejects_a_proxy_with_no_host() {
        let settings = Settings {
            proxy_url: Some("socks5://".into()),
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn a_corrupt_settings_file_holds_transfer_rather_than_releasing_it() {
        // The guard is the one setting whose default is the unsafe value, so
        // it is the one setting that must not simply fall back to it.
        let tmp = tempfile::TempDir::new().expect("tmp");
        std::fs::write(tmp.path().join(SETTINGS_FILE), "{ not json").expect("write");

        let (loaded, problem) = Settings::load(tmp.path());

        assert_eq!(
            loaded.egress_guard,
            EgressGuard::Hold,
            "a settings file that exists and cannot be read must fail closed"
        );
        let problem = problem.expect("corruption is reported");
        assert!(
            problem.contains("held"),
            "the message has to say transfer is held, or the hold is inexplicable: {problem}"
        );
    }

    #[test]
    fn a_first_run_does_not_hold_transfer() {
        // No file is not a corrupt file. There is no prior choice to protect,
        // and greeting a new user with a held library would be absurd.
        let tmp = tempfile::TempDir::new().expect("tmp");
        let (loaded, problem) = Settings::load(tmp.path());

        assert_eq!(loaded.egress_guard, EgressGuard::Off);
        assert!(problem.is_none());
    }

    #[test]
    fn a_file_that_parses_but_fails_validation_keeps_the_users_guard_choice() {
        // Parsing succeeded, so what the user chose is known. Overriding a
        // decision that was read correctly would be failing closed for its own
        // sake.
        let tmp = tempfile::TempDir::new().expect("tmp");
        std::fs::write(
            tmp.path().join(SETTINGS_FILE),
            r#"{"downloadDir":"/tmp/x","listenPort":0,"egressGuard":"warn","egressInterface":"utun6"}"#,
        )
        .expect("write");

        let (loaded, problem) = Settings::load(tmp.path());

        assert!(problem.is_some(), "port 0 is invalid and must be reported");
        assert_eq!(loaded.egress_guard, EgressGuard::Warn);
        assert_eq!(loaded.egress_interface.as_deref(), Some("utun6"));
        assert_eq!(
            loaded.listen_port, DEFAULT_LISTEN_PORT,
            "the invalid field still falls back"
        );
    }

    #[test]
    fn the_rail_starts_expanded_and_round_trips_collapsed() {
        // A collapsed rail that came back expanded on every launch would be
        // the "preference the user re-sets every time" this field exists to
        // prevent.
        assert_eq!(Settings::default().rail, RailState::Expanded);

        let tmp = tempfile::TempDir::new().expect("tmp");
        let settings = Settings {
            rail: RailState::Collapsed,
            ..valid()
        };
        settings.save(tmp.path()).expect("save");
        let (loaded, problem) = Settings::load(tmp.path());
        assert!(problem.is_none());
        assert_eq!(loaded.rail, RailState::Collapsed);
    }

    #[test]
    fn the_egress_guard_is_off_until_the_user_asks_for_it() {
        // A general-purpose client does not open with a warning about someone
        // else's network.
        assert_eq!(Settings::default().egress_guard, EgressGuard::Off);
        assert_eq!(Settings::default().egress_interface, None);
    }

    #[test]
    fn rejects_a_blank_pinned_interface() {
        let settings = Settings {
            egress_interface: Some("   ".into()),
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn accepts_a_pinned_interface_that_is_not_up_right_now() {
        // Configuring the guard before connecting the VPN has to work, or the
        // setting can only be changed from the one state it is meant to guard.
        let settings = Settings {
            egress_guard: EgressGuard::Hold,
            egress_interface: Some("utun99".into()),
            ..valid()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn the_egress_guard_applies_without_a_session_restart() {
        // The check reads the routing table live. Restarting librqbit to
        // change a policy it knows nothing about would drop every peer
        // connection for no reason.
        let before = valid();
        for after in [
            Settings {
                egress_guard: EgressGuard::Hold,
                ..valid()
            },
            Settings {
                egress_guard: EgressGuard::Warn,
                egress_interface: Some("utun6".into()),
                ..valid()
            },
        ] {
            assert!(
                !before.requires_restart(&after),
                "expected no restart for {after:?}"
            );
        }
    }

    #[test]
    fn changing_the_proxy_requires_a_restart() {
        let before = valid();
        let after = Settings {
            proxy_url: Some("socks5://127.0.0.1:1080".into()),
            ..valid()
        };
        assert!(before.requires_restart(&after));
    }

    #[test]
    fn rate_limits_and_theme_apply_without_a_restart() {
        let before = valid();
        let after = Settings {
            upload_limit_bps: Some(2048),
            download_limit_bps: Some(4096),
            theme: Theme::Light,
            rail: RailState::Collapsed,
            ..valid()
        };
        assert!(!before.requires_restart(&after));
    }

    #[test]
    fn session_level_changes_require_a_restart() {
        let before = valid();
        for after in [
            Settings {
                listen_port: 12345,
                ..valid()
            },
            Settings {
                enable_dht: false,
                ..valid()
            },
            Settings {
                enable_upnp: false,
                ..valid()
            },
            Settings {
                download_dir: PathBuf::from("/tmp/other"),
                ..valid()
            },
        ] {
            assert!(
                before.requires_restart(&after),
                "expected a restart for {after:?}"
            );
        }
    }

    /// The half of "settings persist" that a restart proves.
    ///
    /// `save` then `load` has to return what went in, including the optional
    /// fields — a limit that round-trips as `None` would silently uncap the
    /// user on every relaunch.
    #[test]
    fn settings_survive_a_save_and_load() {
        let tmp = tempfile::tempdir().expect("temp dir");

        let mut original = valid();
        original.download_limit_bps = Some(2_000_000);
        original.upload_limit_bps = None;
        original.listen_port = 51_413;
        original.enable_dht = false;
        original.theme = Theme::Dark;

        original.save(tmp.path()).expect("save");
        let (loaded, error) = Settings::load(tmp.path());

        assert!(error.is_none(), "a settings file we just wrote should load");
        assert_eq!(loaded, original, "every field must survive the round trip");
    }

    /// A missing file is a first run, not a failure.
    ///
    /// Note this is not `Settings::default()`: that leaves `download_dir`
    /// empty, and `load` resolves a real one. A first run has to arrive with
    /// somewhere to put downloads, since an empty path fails `validate`.
    #[test]
    fn absent_settings_load_as_usable_defaults_without_an_error() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let (loaded, error) = Settings::load(tmp.path());

        assert!(
            error.is_none(),
            "a first run has no settings file and that is not an error"
        );
        assert!(
            !loaded.download_dir.as_os_str().is_empty(),
            "a first run must arrive with a download directory already chosen"
        );
        assert!(
            loaded.validate().is_ok(),
            "whatever load returns on a first run has to be usable as-is"
        );
        assert_eq!(loaded.listen_port, Settings::default().listen_port);
        assert_eq!(loaded.theme, Settings::default().theme);
    }
}
