//! Configuration for the embedded torrent engine.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Flume's default BitTorrent listen port.
///
/// Chosen to match `rqbit`'s own default so that users who already opened a
/// firewall hole for it do not have to open another one.
pub const DEFAULT_LISTEN_PORT: u16 = 42221;

/// User-controllable settings for the torrent engine.
///
/// This type is deliberately free of Tauri types so the engine can be
/// constructed and tested in a plain `cargo test` process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfig {
    /// Directory that completed and in-progress downloads are written to.
    pub download_dir: PathBuf,

    /// Directory holding librqbit's own session state (fast-resume data, the
    /// DHT routing table, and the list of torrents to restore on next launch).
    ///
    /// Kept separate from [`Self::download_dir`] so that a user pointing
    /// downloads at an external drive does not lose session state when that
    /// drive is unplugged.
    pub session_dir: PathBuf,

    /// TCP/uTP port to listen on for incoming peer connections.
    pub listen_port: u16,

    /// Whether to run the DHT.
    ///
    /// Required for magnet links to resolve without a tracker; disabling it
    /// restricts Flume to torrents with working trackers.
    pub enable_dht: bool,

    /// Whether to ask the router to forward [`Self::listen_port`] via UPnP.
    pub enable_upnp: bool,

    /// SOCKS5 proxy for outgoing peer connections, or `None` for direct.
    ///
    /// Format: `socks5://[user:password@]host:port`.
    ///
    /// Note what this does and does not cover. librqbit routes outgoing *peer*
    /// connections over the proxy. Incoming connections still arrive directly
    /// on the listen port, and the DHT is UDP, which SOCKS5 TCP does not
    /// carry. A user who assumes this hides all traffic would be wrong, which
    /// is why the UI says so.
    pub proxy_url: Option<String>,
}

impl EngineConfig {
    /// Builds a configuration rooted at the OS-conventional application data
    /// directory, with `download_dir` pointing at the user's Downloads folder.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the platform's home or data directories
    /// cannot be determined, which in practice only happens on a misconfigured
    /// system with no `HOME`.
    pub fn with_os_defaults() -> Result<Self, ConfigError> {
        let project = directories::ProjectDirs::from("io.github", "adamgreenwell", "Flume")
            .ok_or(ConfigError::NoHomeDirectory)?;
        let user = directories::UserDirs::new().ok_or(ConfigError::NoHomeDirectory)?;

        let download_dir = user
            .download_dir()
            .map(PathBuf::from)
            .unwrap_or_else(|| user.home_dir().join("Downloads"));

        Ok(Self {
            download_dir,
            session_dir: project.data_dir().to_path_buf(),
            listen_port: DEFAULT_LISTEN_PORT,
            enable_dht: true,
            enable_upnp: true,
            proxy_url: None,
        })
    }
}

/// Failure to derive a default [`EngineConfig`] from the environment.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The platform did not expose a usable home directory.
    #[error("could not determine the user's home directory")]
    NoHomeDirectory,
}
