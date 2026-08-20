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

mod add;
mod config;
mod status;
mod torrent;

use std::{collections::HashMap, sync::Arc};

use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListenerMode,
    ListenerOptions, Magnet, ManagedTorrent, Session, SessionOptions, SessionPersistenceConfig,
    api::TorrentIdOrHash, dht::DhtPersistenceConfig,
};

pub use add::{TorrentFile, TorrentPreview, TorrentSource};
pub use config::{ConfigError, DEFAULT_LISTEN_PORT, EngineConfig};
pub use status::{CoreStatus, DhtStatus, EngineHealth, TelemetrySnapshot};
pub use torrent::{TorrentFileState, TorrentState, TorrentSummary};

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

    /// The supplied magnet URI could not be parsed.
    #[error("that does not look like a valid magnet link")]
    InvalidMagnet(#[source] anyhow::Error),

    /// The torrent file could not be parsed, or metadata could not be fetched.
    #[error("could not read the torrent: {0}")]
    Metadata(#[source] anyhow::Error),

    /// Confirming an add referenced a preview the engine no longer holds.
    #[error("that torrent is no longer pending; preview it again")]
    NoPendingPreview,

    /// No torrent with the given id exists in the session.
    #[error("no torrent with id {0}")]
    UnknownTorrent(usize),

    /// A control operation (pause, resume, remove) failed.
    #[error("the operation failed: {0}")]
    Operation(#[source] anyhow::Error),
}

/// An owned, running torrent session.
///
/// Cloning is cheap: the underlying [`librqbit::Session`] is reference counted.
#[derive(Clone)]
pub struct Engine {
    session: Arc<Session>,
    config: EngineConfig,
    /// Resolved `.torrent` bytes from previews awaiting confirmation, keyed by
    /// hex info hash.
    ///
    /// This is why confirming an add does not re-fetch a magnet's metadata
    /// from the DHT, and why those bytes never need to travel to the webview
    /// and back.
    pending: Arc<tokio::sync::RwLock<HashMap<String, Vec<u8>>>>,
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

        Ok(Self {
            session,
            config,
            pending: Arc::default(),
        })
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

    /// Resolves a torrent's metadata and file list without downloading it.
    ///
    /// For a magnet link this fetches metadata from the DHT, which can take
    /// several seconds and requires a bootstrapped routing table. The resolved
    /// `.torrent` bytes are retained internally so that [`Self::confirm_add`]
    /// does not have to fetch them a second time.
    ///
    /// # Errors
    ///
    /// [`EngineError::InvalidMagnet`] for an unparseable magnet URI, or
    /// [`EngineError::Metadata`] if metadata cannot be read or fetched.
    pub async fn preview(&self, source: TorrentSource) -> Result<TorrentPreview, EngineError> {
        let add = match &source {
            TorrentSource::Magnet { uri } => {
                // Validate before handing it to the session: an unparseable
                // magnet otherwise surfaces as an opaque metadata failure
                // after a long DHT timeout.
                Magnet::parse(uri).map_err(EngineError::InvalidMagnet)?;
                AddTorrent::from_url(uri.clone())
            }
            TorrentSource::File { path } => {
                // Read here rather than in the webview: the frontend then needs
                // no filesystem permission, and `.torrent` contents never cross
                // the IPC boundary.
                let bytes = std::fs::read(path).map_err(|e| {
                    EngineError::Metadata(anyhow::anyhow!("could not read {path}: {e}"))
                })?;
                AddTorrent::from_bytes(bytes)
            }
        };

        let response = self
            .session
            .add_torrent(
                add,
                Some(AddTorrentOptions {
                    list_only: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(EngineError::Metadata)?;

        let listed = match response {
            AddTorrentResponse::ListOnly(listed) => listed,
            // `list_only` should always yield ListOnly; treat anything else as
            // a contract change rather than silently mis-reporting.
            other => {
                let already = matches!(other, AddTorrentResponse::AlreadyManaged(..));
                return Err(EngineError::Metadata(anyhow::anyhow!(
                    "expected a listing from a list-only add (already managed: {already})"
                )));
            }
        };

        let info_hash = listed.info_hash.as_string();
        let files: Vec<TorrentFile> = listed
            .info
            .iter_file_details()
            .enumerate()
            .map(|(index, details)| TorrentFile {
                index,
                path: details
                    .filename
                    .to_pathbuf()
                    .to_string_lossy()
                    .replace('\\', "/"),
                length: details.len,
            })
            .collect();

        let already_added = self
            .session
            .get(TorrentIdOrHash::Hash(listed.info_hash))
            .is_some();

        self.pending
            .write()
            .await
            .insert(info_hash.clone(), listed.torrent_bytes.to_vec());

        Ok(TorrentPreview {
            name: listed
                .info
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| info_hash.clone()),
            total_bytes: files.iter().map(|f| f.length).sum(),
            files,
            info_hash,
            already_added,
        })
    }

    /// Starts a previewed torrent, downloading only `only_files`.
    ///
    /// `only_files` holds indices from [`TorrentPreview::files`]. Passing
    /// `None` downloads everything.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoPendingPreview`] if [`Self::preview`] was not called
    /// for this hash (or its result was already consumed), or
    /// [`EngineError::Metadata`] if the session rejects the torrent.
    pub async fn confirm_add(
        &self,
        info_hash: &str,
        only_files: Option<Vec<usize>>,
    ) -> Result<usize, EngineError> {
        let bytes = self
            .pending
            .write()
            .await
            .remove(info_hash)
            .ok_or(EngineError::NoPendingPreview)?;

        let response = self
            .session
            .add_torrent(
                AddTorrent::from_bytes(bytes),
                Some(AddTorrentOptions {
                    only_files,
                    // Required for librqbit to resume or seed over files that
                    // already exist on disk; without it a restarted download
                    // refuses to touch its own partial data.
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(EngineError::Metadata)?;

        match response {
            AddTorrentResponse::Added(id, _) | AddTorrentResponse::AlreadyManaged(id, _) => Ok(id),
            AddTorrentResponse::ListOnly(_) => Err(EngineError::Metadata(anyhow::anyhow!(
                "session unexpectedly returned a listing"
            ))),
        }
    }

    /// Discards a pending preview the user cancelled.
    ///
    /// Without this, abandoning an add dialog would leak the resolved metadata
    /// for the life of the process.
    pub async fn discard_preview(&self, info_hash: &str) {
        self.pending.write().await.remove(info_hash);
    }

    /// Looks up a torrent handle by session id.
    fn handle(&self, id: usize) -> Result<Arc<ManagedTorrent>, EngineError> {
        self.session
            .get(TorrentIdOrHash::Id(id))
            .ok_or(EngineError::UnknownTorrent(id))
    }

    /// Pauses a torrent, stopping transfer but keeping it in the session.
    ///
    /// # Errors
    ///
    /// [`EngineError::UnknownTorrent`] if no such torrent exists.
    pub async fn pause(&self, id: usize) -> Result<(), EngineError> {
        let handle = self.handle(id)?;
        self.session
            .pause(&handle)
            .await
            .map_err(EngineError::Operation)
    }

    /// Resumes a paused torrent.
    ///
    /// # Errors
    ///
    /// [`EngineError::UnknownTorrent`] if no such torrent exists.
    pub async fn resume(&self, id: usize) -> Result<(), EngineError> {
        let handle = self.handle(id)?;
        self.session
            .unpause(&handle)
            .await
            .map_err(EngineError::Operation)
    }

    /// Removes a torrent, optionally deleting its downloaded files.
    ///
    /// `delete_files` is destructive and irreversible. The UI must confirm it
    /// explicitly and must never default it to true.
    ///
    /// # Errors
    ///
    /// [`EngineError::UnknownTorrent`] if no such torrent exists.
    pub async fn remove(&self, id: usize, delete_files: bool) -> Result<(), EngineError> {
        // Fail on an unknown id before deleting anything.
        self.handle(id)?;
        self.session
            .delete(TorrentIdOrHash::Id(id), delete_files)
            .await
            .map_err(EngineError::Operation)
    }

    /// Changes which files a torrent downloads.
    ///
    /// # Errors
    ///
    /// [`EngineError::UnknownTorrent`] if no such torrent exists.
    pub async fn set_only_files(&self, id: usize, files: Vec<usize>) -> Result<(), EngineError> {
        let handle = self.handle(id)?;
        self.session
            .update_only_files(&handle, &files.into_iter().collect())
            .await
            .map_err(EngineError::Operation)
    }

    /// Lists a torrent's files with their individual progress and selection.
    ///
    /// # Errors
    ///
    /// [`EngineError::UnknownTorrent`] if no such torrent exists, or
    /// [`EngineError::Metadata`] if its metadata has not resolved yet — which
    /// happens briefly for a magnet added before its file list arrived.
    pub fn torrent_files(&self, id: usize) -> Result<Vec<TorrentFileState>, EngineError> {
        let handle = self.handle(id)?;
        let stats = handle.stats();
        // `only_files` is None when every file is selected.
        let selection = handle.only_files();

        handle
            .with_metadata(|meta| {
                meta.file_infos
                    .iter()
                    .enumerate()
                    .map(|(index, info)| TorrentFileState {
                        index,
                        path: info.relative_filename.to_string_lossy().replace('\\', "/"),
                        length: info.len,
                        // `file_progress` is positional and should match
                        // `file_infos`; fall back to zero rather than panicking
                        // if a future version ever disagrees.
                        progress_bytes: stats.file_progress.get(index).copied().unwrap_or(0),
                        selected: selection.as_ref().is_none_or(|only| only.contains(&index)),
                    })
                    .collect()
            })
            .map_err(EngineError::Metadata)
    }

    /// Applies global transfer limits to the running session.
    ///
    /// Takes effect immediately — librqbit swaps the rate limiter in place, so
    /// changing a limit never needs a restart. `None` means unlimited.
    ///
    /// A limit of `Some(0)` is treated as unlimited rather than as a total
    /// stall, because `NonZeroU32` cannot represent zero; settings validation
    /// rejects zero before it reaches here.
    pub fn apply_limits(&self, download_bps: Option<u32>, upload_bps: Option<u32>) {
        self.session
            .ratelimits
            .set_download_bps(download_bps.and_then(std::num::NonZeroU32::new));
        self.session
            .ratelimits
            .set_upload_bps(upload_bps.and_then(std::num::NonZeroU32::new));
    }

    /// Reads back the limits currently in force, in bytes per second.
    pub fn current_limits(&self) -> (Option<u32>, Option<u32>) {
        let config = self.session.ratelimits.get_config();
        (
            config.download_bps.map(std::num::NonZeroU32::get),
            config.upload_bps.map(std::num::NonZeroU32::get),
        )
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
