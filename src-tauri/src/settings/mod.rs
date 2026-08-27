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

        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            // Missing file on first run is expected, not a problem.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (defaults, None),
            Err(e) => {
                return (
                    defaults,
                    Some(format!("could not read {}: {e}", path.display())),
                );
            }
        };

        match serde_json::from_str::<Self>(&raw) {
            Ok(settings) => match settings.validate() {
                Ok(()) => (settings, None),
                Err(e) => (defaults, Some(format!("settings were invalid: {e}"))),
            },
            Err(e) => (
                defaults,
                Some(format!("settings file was not valid JSON: {e}")),
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
}
