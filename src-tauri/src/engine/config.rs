//! Configuration for the embedded torrent engine.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Flume's default BitTorrent listen port.
///
/// Chosen to match `rqbit`'s own default so that users who already opened a
/// firewall hole for it do not have to open another one.
pub const DEFAULT_LISTEN_PORT: u16 = 42221;

/// Most peers Flume will hold a connection to, per torrent.
///
/// 128 is not a number chosen here. It is librqbit's own fallback --
/// `peer_limit.unwrap_or(128)` sizes the per-torrent peer semaphore -- so
/// passing it explicitly changes nothing about how Flume has always behaved.
/// That is the point: this setting makes an existing ceiling visible and
/// adjustable rather than introducing one.
///
/// Worth stating plainly because the obvious assumption is the other way
/// round: leaving `SessionOptions.peer_limit` as `None` was never "unlimited
/// peers". It was 128, decided by a dependency, invisible in the UI, and
/// unmentioned in the limiting-factor panel that exists to name constraints.
pub const DEFAULT_PEER_LIMIT: u32 = 128;

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

    /// Most peers to hold a connection to, per torrent.
    ///
    /// Per torrent rather than per session, because that is the only shape
    /// librqbit offers: the semaphore lives on the live torrent state. Ten
    /// torrents at 128 is a ceiling of 1280 sockets, which is the figure that
    /// matters to a modest NAT table, and is why the settings copy says "per
    /// torrent" rather than leaving the reader to assume otherwise.
    pub peer_limit: u32,

    /// A list of IP ranges to refuse peers from, or `None` for no blocklist.
    ///
    /// `http://`, `https://` or `file://`. Fetched by librqbit while the
    /// session is constructed, which has two consequences worth knowing here:
    /// the fetch happens on whatever interface the egress guard has allowed,
    /// and a fetch that fails takes the whole session start down with it.
    pub blocklist_url: Option<String>,

    /// A list of IP ranges to accept peers from *exclusively*, or `None`.
    ///
    /// Far blunter than the blocklist: when set, every address outside it is
    /// refused, so an allowlist that loads nothing refuses everything.
    pub allowlist_url: Option<String>,

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
            peer_limit: DEFAULT_PEER_LIMIT,
            blocklist_url: None,
            allowlist_url: None,
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
