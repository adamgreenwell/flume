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
use crate::engine::{DEFAULT_LISTEN_PORT, DEFAULT_PEER_LIMIT, EngineConfig};

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

/// Which key the torrent list is ordered by.
///
/// Frontend-only, and the variant names are column names deliberately: #147
/// moves sorting from the toolbar into the column headers, and a value keyed
/// to toolbar labels would need migrating when it does.
///
/// `Activity` is the exception and stays anyway. It is the sum of the two
/// rate columns rather than a column of its own, and it is the order someone
/// glancing at a running client wants, so it is the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    /// Download plus upload rate. The default.
    #[default]
    Activity,
    /// Torrent name.
    Name,
    /// Total size of the selected files.
    Size,
    /// Share of the selected bytes verified.
    Progress,
    /// Download rate.
    DownloadBps,
    /// Upload rate.
    UploadBps,
    /// Uploaded over downloaded.
    Ratio,
    /// Estimated time to completion.
    Eta,
    /// When Flume first added it.
    Added,
    /// Lifecycle state.
    State,
}

/// Which way a [`SortBy`] runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    /// Smallest, earliest or A-first.
    Asc,
    /// Largest, latest or Z-first. The default, because every numeric key is
    /// more useful that way round.
    #[default]
    Desc,
}

/// How the torrent list is ordered.
///
/// A pair rather than a single id so a direction survives a relaunch too.
/// Flipping the arrow and finding it flipped back next launch is the kind of
/// half-persistence that reads as a bug.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortState {
    /// The key rows are ordered by.
    pub by: SortBy,
    /// Which way that key runs.
    pub direction: SortDirection,
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

/// The largest peer limit [`Settings::validate`] will accept, per torrent.
///
/// Not a figure librqbit imposes -- it is where a per-torrent number stops
/// being about this torrent and starts being about the process. Twenty
/// torrents at this limit is already ten thousand sockets.
pub const MAX_PEER_LIMIT: u32 = 500;

/// Everything the user can configure.
///
/// Mirrored in `src/lib/ipc/types.ts`.
// `Eq` is deliberately absent: `seed_ratio_limit` is an `f64`, and floats are
// not `Eq` because of NaN. `PartialEq` is all this needs -- it is used for
// change detection, never as a map key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// Most peers to connect to at once, per torrent. Requires a restart.
    ///
    /// Not an `Option`, unlike every other limit here, and the difference is
    /// deliberate: there is no "unlimited" to represent. librqbit caps peers
    /// at 128 when given nothing, so an empty value would mean 128 while
    /// reading as "no limit" -- which is exactly the misreading this field
    /// exists to clear up. A number is always in force; the only question is
    /// which one.
    pub peer_limit: u32,

    /// Global download limit in bytes per second; `None` means unlimited.
    ///
    /// Applied live, without a restart.
    pub download_limit_bps: Option<u32>,

    /// Global upload limit in bytes per second; `None` means unlimited.
    ///
    /// Applied live, without a restart.
    pub upload_limit_bps: Option<u32>,

    /// A list of IP ranges to refuse peers from; `None` is no blocklist.
    ///
    /// `http://`, `https://` or `file://`. Requires a session restart, and
    /// unlike every other restart-triggering setting here it can make the
    /// restart *fail*: librqbit fetches the list while constructing the
    /// session and propagates a failure, so an unreachable host or a 404
    /// means no engine. That is the right trade -- starting unfiltered while
    /// the user believes otherwise is the failure this feature exists to
    /// prevent -- but it is why `update_settings` restores the previous
    /// settings when a restart fails.
    pub blocklist_url: Option<String>,

    /// A list of IP ranges to accept peers from exclusively; `None` accepts
    /// any address not blocked.
    ///
    /// Blunter than the blocklist and worth treating as such: every address
    /// outside the list is refused, so a list that loads nothing refuses
    /// every peer rather than none.
    pub allowlist_url: Option<String>,

    /// SOCKS5 proxy for outgoing peer connections; `None` connects directly.
    ///
    /// Format: `socks5://[user:password@]host:port`. Requires a session
    /// restart, since the proxy is fixed when the session is constructed.
    pub proxy_url: Option<String>,

    /// Stop seeding once uploaded / downloaded reaches this; `None` is no
    /// limit.
    ///
    /// The global default. A torrent can override it, but that override lives
    /// in the library record rather than here -- it is keyed by info hash and
    /// has to be pruned when the torrent goes away, which is what that record
    /// already does for every other per-torrent fact.
    ///
    /// Applied live: the policy engine evaluates on each telemetry tick, so a
    /// change takes effect within a second and needs no session restart.
    pub seed_ratio_limit: Option<f64>,

    /// Stop seeding after this many seconds of actual seeding; `None` is no
    /// limit.
    ///
    /// Counts time spent seeding, not time since the torrent was added -- see
    /// [`crate::library::Record::seed_seconds`], which survives restarts so
    /// this means the same thing across a quit.
    pub seed_time_limit_secs: Option<u64>,

    /// Most torrents that may download at once; `None` is no limit.
    ///
    /// Session-wide rather than per-torrent, and deliberately not overridable:
    /// a slot limit is a statement about this machine, and "let this one
    /// through" is what the queue order is for.
    ///
    /// Applied live, like the seed limits above.
    pub max_active_downloads: Option<u32>,

    /// Most torrents that may seed at once; `None` is no limit.
    ///
    /// Counted separately from downloads because they cost different things:
    /// a finished torrent spends upload, an unfinished one spends both.
    pub max_active_seeds: Option<u32>,

    /// Most torrents that may run at all, downloads and seeds together;
    /// `None` is no limit.
    ///
    /// For the case where the real constraint is peer connections rather than
    /// either direction. Whichever limit binds first wins.
    pub max_active_total: Option<u32>,

    /// UI colour scheme. Frontend-only; persisted here so it survives restarts.
    pub theme: Theme,

    /// Row height in the library. Frontend-only, persisted for the same reason.
    pub density: Density,

    /// How the torrent list is ordered. Frontend-only, persisted likewise.
    pub sort: SortState,

    /// The folder the last add was sent to, or `None` for never-changed.
    ///
    /// The default destination for the next add, because someone who has
    /// just filed one torrent somewhere is usually filing the next one
    /// there too. `None` falls back to [`Self::download_dir`], which is why
    /// this is not initialised to a copy of it -- a copy would freeze the
    /// global setting's value at first launch and stop tracking it.
    ///
    /// Not in the settings screen: it is written by the add dialog, not
    /// typed, and giving the same state two controls means keeping them in
    /// step. Not reported as a changed setting either, for the same reason
    /// -- "the user picked a folder" is an add, not a preference anyone
    /// reached for.
    pub last_add_folder: Option<PathBuf>,

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

    /// Whether Flume checks once a day for a newer version.
    ///
    /// A plain `bool`, defaulting to `true`, where usage reporting is a
    /// three-state opt-in -- deliberately, and the difference is the point.
    /// Usage counts are for the project's benefit; an update check is for the
    /// user's, and an install stranded on an old build is a security problem.
    /// So it is on by default, said plainly on the first-run screen, and this
    /// one switch turns it off. Because the struct is `#[serde(default)]`, an
    /// existing install that upgrades reads it as `true` too, which the
    /// release notes have to say.
    ///
    /// Off stops the daily check only; choosing "Check for Updates…" is the
    /// person asking, and still checks. Nothing is ever installed without a
    /// click either way. See [`crate::updates`].
    pub check_for_updates: bool,
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
            peer_limit: DEFAULT_PEER_LIMIT,
            blocklist_url: None,
            allowlist_url: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            proxy_url: None,
            seed_ratio_limit: None,
            seed_time_limit_secs: None,
            max_active_downloads: None,
            max_active_seeds: None,
            max_active_total: None,
            theme: Theme::System,
            density: Density::Comfortable,
            sort: SortState::default(),
            last_add_folder: None,
            rail: RailState::Expanded,
            egress_guard: EgressGuard::Off,
            egress_interface: None,
            // Not asked yet. Never defaults to `Some(true)`: consent that the
            // user did not give is not consent.
            usage_reporting: None,
            check_for_updates: true,
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
        // Zero would hold every torrent at nought peers, which looks
        // identical to a dead swarm from the UI. Rejected rather than
        // accepted as a way to stop everything -- pausing does that, visibly.
        if self.peer_limit == 0 {
            return Err(SettingsError::Invalid(
                "a peer limit of 0 would stop all transfer; pause the torrents instead".into(),
            ));
        }
        // An upper bound because the real ceiling is not this setting. Each
        // peer is a socket, the limit is per torrent, and a library of twenty
        // torrents multiplies whatever is typed here -- so a large number
        // surfaces as EMFILE from somewhere deep in the engine rather than as
        // the slow download the user was trying to fix. Refusing it names the
        // problem instead.
        if self.peer_limit > MAX_PEER_LIMIT {
            return Err(SettingsError::Invalid(format!(
                "a peer limit above {MAX_PEER_LIMIT} per torrent runs the process out of sockets"
            )));
        }
        // An empty string is not a folder. It would be accepted by the add
        // path and resolve to somewhere unpredictable relative to the
        // process, which is a worse outcome than refusing it.
        if self
            .last_add_folder
            .as_ref()
            .is_some_and(|dir| dir.as_os_str().is_empty())
        {
            return Err(SettingsError::Invalid(
                "the last-used download folder cannot be empty".into(),
            ));
        }
        // Checked here rather than at session start, where the same typo
        // costs an engine. A scheme librqbit cannot read is the one mistake
        // that is both easy to make and indistinguishable, afterwards, from
        // the host being down.
        for (label, url) in [
            ("blocklist", self.blocklist_url.as_deref()),
            ("allowlist", self.allowlist_url.as_deref()),
        ] {
            let Some(url) = url else { continue };
            let url = url.trim();
            if url.is_empty() {
                return Err(SettingsError::Invalid(format!(
                    "leave the {label} empty to use none, or give a full URL"
                )));
            }
            if !["http://", "https://", "file://"]
                .iter()
                .any(|scheme| url.starts_with(scheme))
            {
                return Err(SettingsError::Invalid(format!(
                    "the {label} must be an http://, https:// or file:// URL"
                )));
            }
            // A scheme and nothing else parses as a URL and fetches nothing.
            if ["http://", "https://", "file://"].contains(&url) {
                return Err(SettingsError::Invalid(format!(
                    "the {label} URL is missing everything after the scheme"
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
            peer_limit: self.peer_limit,
            blocklist_url: self.blocklist_url.clone(),
            allowlist_url: self.allowlist_url.clone(),
            proxy_url: self.proxy_url.clone(),
        }
    }

    /// Returns `self` with every session-level field taken from `previous`.
    ///
    /// The undo for a settings change whose restart failed. It restores the
    /// fields that caused the restart and leaves everything else alone,
    /// which matters because `update_settings` is not serialised: the dialog
    /// stays interactive while a restart runs, and a start that has to fetch
    /// a blocklist can take a while. Writing back a whole snapshot taken
    /// before the failed change would discard any *other* change saved in
    /// the meantime -- one whose own command already returned success.
    ///
    /// Must list exactly the fields [`Self::requires_restart`] compares.
    /// `a_restored_settings_object_needs_no_restart` pins that: restoring
    /// has to leave nothing for a restart to do.
    #[must_use]
    pub fn with_session_fields_from(&self, previous: &Self) -> Self {
        Self {
            download_dir: previous.download_dir.clone(),
            listen_port: previous.listen_port,
            enable_dht: previous.enable_dht,
            enable_upnp: previous.enable_upnp,
            peer_limit: previous.peer_limit,
            blocklist_url: previous.blocklist_url.clone(),
            allowlist_url: previous.allowlist_url.clone(),
            proxy_url: previous.proxy_url.clone(),
            ..self.clone()
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
            // The peer semaphore is sized when a torrent goes live, so a
            // changed limit reaches existing torrents only on a restart.
            || self.peer_limit != next.peer_limit
            // Both lists are fetched while the session is constructed, so a
            // new URL means a new session -- and re-fetching an unchanged one
            // would be a needless request on every unrelated settings change.
            || self.blocklist_url != next.blocklist_url
            || self.allowlist_url != next.allowlist_url
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
    fn rejects_a_zero_peer_limit() {
        let settings = Settings {
            peer_limit: 0,
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn rejects_a_peer_limit_that_would_exhaust_sockets() {
        let settings = Settings {
            peer_limit: MAX_PEER_LIMIT + 1,
            ..valid()
        };
        assert!(settings.validate().is_err());

        let at_the_ceiling = Settings {
            peer_limit: MAX_PEER_LIMIT,
            ..valid()
        };
        assert!(at_the_ceiling.validate().is_ok());
    }

    /// The default must stay equal to librqbit's own fallback.
    ///
    /// Not a tautology against the constant: the point is that upgrading
    /// Flume does not silently change how many peers an existing install
    /// connects to. If librqbit ever moves its `unwrap_or(128)`, this test
    /// keeps passing and the doc comment on `DEFAULT_PEER_LIMIT` becomes the
    /// thing that is wrong -- which is the right place for the discrepancy to
    /// be found, since it is a claim about the dependency rather than about
    /// this code.
    #[test]
    fn defaults_to_the_limit_librqbit_already_applied() {
        assert_eq!(Settings::default().peer_limit, 128);
    }

    #[test]
    fn a_changed_peer_limit_requires_a_restart() {
        let before = valid();
        let after = Settings {
            peer_limit: 200,
            ..valid()
        };
        assert!(before.requires_restart(&after));
    }

    #[test]
    fn rejects_a_filter_url_with_a_scheme_librqbit_cannot_read() {
        for url in ["ftp://lists.example/list.p2p", "lists.example/list.p2p", ""] {
            let blocked = Settings {
                blocklist_url: Some(url.to_owned()),
                ..valid()
            };
            assert!(
                blocked.validate().is_err(),
                "blocklist {url:?} should be rejected"
            );

            let allowed = Settings {
                allowlist_url: Some(url.to_owned()),
                ..valid()
            };
            assert!(
                allowed.validate().is_err(),
                "allowlist {url:?} should be rejected"
            );
        }
    }

    /// A scheme with nothing after it parses and fetches nothing.
    ///
    /// Worth its own case: it passes a `starts_with` check, which is what the
    /// obvious validation would be, and then loads zero ranges silently.
    #[test]
    fn rejects_a_filter_url_that_is_only_a_scheme() {
        for url in ["http://", "https://", "file://"] {
            let settings = Settings {
                blocklist_url: Some(url.to_owned()),
                ..valid()
            };
            assert!(settings.validate().is_err(), "{url:?} should be rejected");
        }
    }

    #[test]
    fn accepts_the_three_schemes_librqbit_reads() {
        for url in [
            "https://lists.example/list.p2p.gz",
            "http://10.0.0.2/list.p2p",
            "file:///Users/someone/lists/level1.p2p",
        ] {
            let settings = Settings {
                blocklist_url: Some(url.to_owned()),
                allowlist_url: Some(url.to_owned()),
                ..valid()
            };
            assert!(settings.validate().is_ok(), "{url:?} should be accepted");
        }
    }

    /// Restoring the session fields must leave nothing for a restart to do.
    ///
    /// The drift catcher between [`Settings::with_session_fields_from`] and
    /// [`Settings::requires_restart`], which list the same fields in two
    /// places. Add one to `requires_restart` and forget the other and this
    /// fails, rather than a failed restart quietly keeping the value that
    /// caused it.
    #[test]
    fn a_restored_settings_object_needs_no_restart() {
        let previous = valid();
        let changed = Settings {
            download_dir: PathBuf::from("/tmp/elsewhere"),
            listen_port: previous.listen_port + 1,
            enable_dht: !previous.enable_dht,
            enable_upnp: !previous.enable_upnp,
            peer_limit: previous.peer_limit + 1,
            blocklist_url: Some("https://lists.example/list.p2p".to_owned()),
            allowlist_url: Some("https://lists.example/allow.p2p".to_owned()),
            proxy_url: Some("socks5://127.0.0.1:1080".to_owned()),
            // Not a session field, and the point of the second assertion.
            theme: Theme::Dark,
            ..valid()
        };
        assert!(
            previous.requires_restart(&changed),
            "the fixture is meant to change every session field"
        );

        let restored = changed.with_session_fields_from(&previous);

        assert!(
            !previous.requires_restart(&restored),
            "a field compared by requires_restart is not restored by \
             with_session_fields_from"
        );
        assert_eq!(
            restored.theme,
            Theme::Dark,
            "restoring the session fields discarded a change that never \
             needed a restart"
        );
    }

    #[test]
    fn a_changed_filter_url_requires_a_restart() {
        let before = valid();
        for next in [
            Settings {
                blocklist_url: Some("https://lists.example/list.p2p".to_owned()),
                ..valid()
            },
            Settings {
                allowlist_url: Some("https://lists.example/list.p2p".to_owned()),
                ..valid()
            },
        ] {
            assert!(before.requires_restart(&next));
        }
    }

    #[test]
    fn rejects_an_empty_remembered_folder() {
        let settings = Settings {
            last_add_folder: Some(PathBuf::new()),
            ..valid()
        };
        assert!(settings.validate().is_err());
    }

    /// Remembering a folder must not restart the session.
    ///
    /// It changes nothing the engine was constructed with -- the folder is
    /// chosen per add and passed to `add_torrent` -- so treating it as a
    /// session-level change would rebuild librqbit every time somebody filed
    /// a torrent somewhere new.
    #[test]
    fn remembering_a_folder_does_not_require_a_restart() {
        let before = valid();
        let after = Settings {
            last_add_folder: Some(PathBuf::from("/tmp/elsewhere")),
            ..valid()
        };
        assert!(!before.requires_restart(&after));
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

    /// A change that is not session-level must not look like one.
    ///
    /// Load-bearing beyond deciding whether to rebuild the session.
    /// `update_settings` asks the same question to decide whether a rollback
    /// is still relevant: a theme change saved while a restart was failing
    /// is not a newer answer about the session, and treating it as one would
    /// leave the value that failed persisted and the engine down across
    /// relaunches.
    #[test]
    fn a_change_outside_the_session_does_not_ask_for_a_restart() {
        let before = valid();
        for after in [
            Settings {
                theme: Theme::Dark,
                ..valid()
            },
            Settings {
                density: Density::Compact,
                ..valid()
            },
            Settings {
                download_limit_bps: Some(2_000_000),
                ..valid()
            },
            Settings {
                seed_ratio_limit: Some(2.0),
                ..valid()
            },
        ] {
            assert!(
                !before.requires_restart(&after),
                "expected no restart for {after:?}"
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
