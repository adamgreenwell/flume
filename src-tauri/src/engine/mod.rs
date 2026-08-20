//! A thin, Tauri-free wrapper around [`librqbit::Session`].
//!
//! # Why this module has no Tauri types
//!
//! Everything here compiles and runs in a plain `cargo test` process. That is a
//! deliberate constraint: the engine is the part of Flume most likely to break
//! on a librqbit upgrade, and it must be testable without spawning a WebView.
//! The Tauri boundary lives one layer up, in [`crate::commands`].
//!
//! # Data flow
//!
//! librqbit writes torrent pieces straight to disk. This module only ever reads
//! back small scalar counters and hands them to the UI as [`CoreStatus`]. Piece
//! data never crosses the IPC boundary.

mod config;
mod status;
mod torrent;

use std::sync::Arc;

use librqbit::{
    DhtSessionConfig, ListenerMode, ListenerOptions, Session, SessionOptions,
    SessionPersistenceConfig, dht::DhtPersistenceConfig,
};

pub use config::{ConfigError, DEFAULT_LISTEN_PORT, EngineConfig};
pub use status::{CoreStatus, DhtStatus, EngineHealth, TelemetrySnapshot};
pub use torrent::{TorrentState, TorrentSummary};

/// Errors that can arise while starting or querying the engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The download or session directory could not be created.
    #[error("could not create directory {path}: {source}")]
    Directory {
        /// The directory Flume tried to create.
        path: String,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// librqbit refused to start a session.
    ///
    /// Most commonly this is a port conflict, or a session directory that
    /// exists but is not writable.
    #[error("failed to start the torrent session: {0}")]
    SessionStart(#[source] anyhow::Error),
}

/// An owned, running torrent session.
///
/// Cloning is cheap: the underlying [`librqbit::Session`] is reference counted.
#[derive(Clone)]
pub struct Engine {
    session: Arc<Session>,
    config: EngineConfig,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Session` is not Debug, so print the parts that matter for logs.
        f.debug_struct("Engine")
            .field("config", &self.config)
            .field("listen_addr", &self.session.listen_addr())
            .finish()
    }
}

impl Engine {
    /// Starts a torrent session using `config`.
    ///
    /// Creates the download and session directories if they do not exist, then
    /// brings up the DHT, the peer listener, and session persistence according
    /// to `config`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Directory`] if a required directory cannot be
    /// created, or [`EngineError::SessionStart`] if librqbit fails to bind its
    /// listener or restore persisted state.
    pub async fn start(config: EngineConfig) -> Result<Self, EngineError> {
        for dir in [&config.download_dir, &config.session_dir] {
            std::fs::create_dir_all(dir).map_err(|source| EngineError::Directory {
                path: dir.display().to_string(),
                source,
            })?;
        }

        let opts = SessionOptions {
            // `None` disables the DHT entirely in librqbit 9; there is no
            // `disable_dht` boolean any more.
            dht: config.enable_dht.then(|| DhtSessionConfig {
                port: None,
                bootstrap_addrs: None,
                // `config_filename: None` would put the routing table in a
                // single global OS location shared by every Flume instance,
                // rather than in this session's directory. That both leaks
                // state out of `session_dir` and makes a second instance try
                // to bind the *same* persisted UDP port -- see issue #19.
                persistence: Some(DhtPersistenceConfig {
                    config_filename: Some(config.session_dir.join("dht.json")),
                    ..Default::default()
                }),
            }),

            // Fast-resume lets a restart skip re-hashing completed pieces.
            fastresume: true,

            // Persist the torrent list so downloads survive a restart.
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(config.session_dir.clone()),
            }),

            // librqbit defaults `listen` to `None`, which means Flume would
            // never accept incoming peers and could not seed. Always listen.
            listen: Some(ListenerOptions {
                mode: ListenerMode::TcpOnly,
                listen_addr: (std::net::Ipv6Addr::UNSPECIFIED, config.listen_port).into(),
                enable_upnp_port_forwarding: config.enable_upnp,
                ..Default::default()
            }),

            client_name_and_version: Some(format!("Flume {}", env!("CARGO_PKG_VERSION"))),

            ..Default::default()
        };

        let session = Session::new_with_opts(config.download_dir.clone(), opts)
            .await
            .map_err(EngineError::SessionStart)?;

        Ok(Self { session, config })
    }

    /// The configuration this engine was started with.
    pub const fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Borrows the underlying librqbit session.
    ///
    /// Used by later phases (adding and controlling torrents); Phase 0 only
    /// needs [`Self::core_status`].
    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }

    /// Reads current DHT routing-table health.
    ///
    /// Returns [`DhtStatus::disabled`] when the DHT was switched off.
    pub fn dht_status(&self) -> DhtStatus {
        match self.session.get_dht() {
            Some(dht) => {
                let stats = dht.stats();
                DhtStatus {
                    enabled: true,
                    nodes_v4: stats.routing_table_size,
                    nodes_v6: stats.routing_table_size_v6,
                    outstanding_requests: stats.outstanding_requests,
                }
            }
            None => DhtStatus::disabled(),
        }
    }

    /// Builds a snapshot of engine state for the UI.
    ///
    /// Cheap enough to call at the UI's ~1 Hz telemetry cadence: every field
    /// reads an atomic counter or a small lock, and nothing here touches disk.
    pub fn core_status(&self) -> CoreStatus {
        let stats = self.session.stats_snapshot();
        let dht = self.dht_status();
        // Read once: two separate calls could disagree, which would let the
        // reported port and the derived health describe different states.
        let listen_addr = self.session.listen_addr();

        CoreStatus {
            client_version: self.session.client_name_and_version().to_string(),
            listen_port: listen_addr.map(|addr| addr.port()),
            announce_port: self.session.announce_port(),
            health: classify_health(&dht, listen_addr.is_some()),
            dht,
            download_dir: self.config.download_dir.display().to_string(),
            uptime_seconds: stats.uptime_seconds,
            download_bps: stats.download_speed.as_bytes(),
            upload_bps: stats.upload_speed.as_bytes(),
            live_peers: stats.peers.live,
        }
    }

    /// Snapshots every torrent currently known to the session.
    ///
    /// Ordered by id so the UI list does not reshuffle between ticks; the
    /// session's internal ordering is not guaranteed.
    pub fn torrent_summaries(&self) -> Vec<TorrentSummary> {
        let mut summaries = self.session.with_torrents(|torrents| {
            torrents
                .map(|(id, handle)| {
                    torrent::summarize(
                        id,
                        // `as_string` is hex encoding. `Id20`'s Debug impl
                        // happens to produce the same thing, but relying on a
                        // Debug format for a wire value is a trap.
                        handle.info_hash().as_string(),
                        handle.name(),
                        handle.output_folder().display().to_string(),
                        &handle.stats(),
                    )
                })
                .collect::<Vec<_>>()
        });
        summaries.sort_by_key(|s| s.id);
        summaries
    }

    /// Builds the full telemetry payload pushed to the UI each tick.
    pub fn telemetry(&self) -> TelemetrySnapshot {
        TelemetrySnapshot {
            core: self.core_status(),
            torrents: self.torrent_summaries(),
        }
    }

    /// Shuts the session down, flushing persistence and fast-resume state.
    ///
    /// Should be awaited before the process exits so that an in-progress
    /// download resumes cleanly rather than re-hashing on next launch.
    pub async fn shutdown(&self) {
        self.session.stop().await;
    }
}

/// Minimum DHT routing-table size before peer discovery is considered useful.
///
/// A freshly bootstrapped node reaches a handful of contacts within a second or
/// two; below this we are still bootstrapping.
const DHT_READY_NODE_THRESHOLD: usize = 8;

/// Derives the coarse [`EngineHealth`] shown in the UI.
///
/// `listening` reports whether the peer listener successfully bound a port.
fn classify_health(dht: &DhtStatus, listening: bool) -> EngineHealth {
    if !listening {
        return EngineHealth::Starting;
    }
    if !dht.enabled {
        return EngineHealth::Degraded;
    }
    if dht.total_nodes() >= DHT_READY_NODE_THRESHOLD {
        EngineHealth::Ready
    } else {
        EngineHealth::Connecting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dht(enabled: bool, nodes: usize) -> DhtStatus {
        DhtStatus {
            enabled,
            nodes_v4: nodes,
            nodes_v6: 0,
            outstanding_requests: 0,
        }
    }

    #[test]
    fn not_listening_is_starting() {
        assert_eq!(
            classify_health(&dht(true, 100), false),
            EngineHealth::Starting
        );
    }

    #[test]
    fn dht_disabled_is_degraded() {
        assert_eq!(
            classify_health(&DhtStatus::disabled(), true),
            EngineHealth::Degraded
        );
    }

    #[test]
    fn empty_routing_table_is_connecting() {
        assert_eq!(
            classify_health(&dht(true, 0), true),
            EngineHealth::Connecting
        );
    }

    #[test]
    fn populated_routing_table_is_ready() {
        assert_eq!(
            classify_health(&dht(true, DHT_READY_NODE_THRESHOLD), true),
            EngineHealth::Ready
        );
    }

    #[test]
    fn total_nodes_sums_both_families() {
        let s = DhtStatus {
            enabled: true,
            nodes_v4: 3,
            nodes_v6: 4,
            outstanding_requests: 0,
        };
        assert_eq!(s.total_nodes(), 7);
    }
}
