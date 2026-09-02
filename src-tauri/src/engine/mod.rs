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
mod availability;
mod bottleneck;
mod config;
mod detail;
mod import;
mod note;
mod preflight;
mod status;
mod torrent;

use std::{collections::HashMap, sync::Arc};

use std::time::Duration;

use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, Api, ConnectionOptions, DhtSessionConfig,
    ListenerMode, ListenerOptions, Magnet, ManagedTorrent, PeerStatsFilter, Session,
    SessionOptions, SessionPersistenceConfig, api::TorrentIdOrHash, dht::DhtPersistenceConfig,
};

pub use add::{TorrentFile, TorrentPreview, TorrentSource};
pub use bottleneck::{Bottleneck, LimitFactor};
pub use config::{ConfigError, DEFAULT_LISTEN_PORT, EngineConfig};
pub use detail::{
    MAX_FILE_PIECE_BUCKETS, MAX_PIECE_BUCKETS, PeerInfo, PieceMap, SwarmStats, TorrentDetail,
};
pub use import::{ClientKind, DetectedClient, ImportOutcome};
pub use note::{Note, NoteSeverity};
pub use status::{CoreStatus, DhtStatus, EngineHealth, TelemetrySnapshot};
pub use torrent::{SwarmHealth, TorrentFileState, TorrentState, TorrentSummary};

/// How long `Session::new_with_opts` may take before Flume gives up on it.
///
/// Generous, because a legitimate start is not fast: librqbit rewrites the
/// whole of `session.json` once per restored torrent, verifies fast-resume
/// state, binds the listener and bootstraps the DHT before it returns. A
/// library of a few hundred torrents on a slow disk is allowed to take a
/// while.
///
/// It exists because the alternative is not "slow", it is "never". A row in
/// `session.json` whose `<hash>.torrent` sidecar is missing is restored as a
/// *magnet* — librqbit's `into_add_torrent` branches on the byte length and a
/// missing sidecar reads as empty — and magnet resolution on that path has no
/// timeout of its own. For an info hash nobody is seeding, the restore future
/// never completes, and the restore loop cannot exit while a future is
/// pending. See issue #154.
pub const SESSION_START_TIMEOUT: Duration = Duration::from_secs(120);

/// How long a magnet may take to produce a file list before Flume gives up.
///
/// A well-seeded magnet resolves in seconds — the live-DHT test does it in
/// under ten. This is generous enough for a cold DHT that has not finished
/// bootstrapping, and short enough that a dead magnet fails rather than
/// hanging the add dialog forever.
const MAGNET_METADATA_TIMEOUT: Duration = Duration::from_secs(60);

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

    /// The session did not finish starting within
    /// [`SESSION_START_TIMEOUT`].
    ///
    /// Distinct from [`Self::SessionStart`] because the cause and the remedy
    /// are different: nothing refused, something never answered. The usual
    /// reason is a persisted torrent whose `.torrent` file is missing, which
    /// librqbit restores as a magnet and then waits on forever. See #154.
    #[error(
        "the torrent session did not finish starting within {seconds} seconds. \
         This usually means a torrent in your library is missing its .torrent \
         file and Flume is waiting for peers that will never answer. Nothing \
         has been deleted; the diagnostics report in Settings → Privacy says \
         which session directory to look at."
    )]
    SessionStartTimeout {
        /// How long was waited, in seconds.
        seconds: u64,
    },

    /// The supplied magnet URI could not be parsed.
    #[error("that does not look like a valid magnet link")]
    InvalidMagnet(#[source] anyhow::Error),

    /// The torrent file could not be parsed, or metadata could not be fetched.
    #[error("could not read the torrent: {0}")]
    Metadata(#[source] anyhow::Error),

    /// No peer supplied a magnet's metadata before the deadline.
    ///
    /// A magnet carries an info hash, not a file list. The list has to come
    /// from a peer that already has the torrent, so this means none answered —
    /// not that the link is malformed.
    #[error(
        "no peer sent this torrent's file list within {seconds} seconds. \
         The torrent may have no active seeders, or the DHT may still be \
         warming up — the status dot turns green when it is ready."
    )]
    MetadataTimeout {
        /// How long was waited, in seconds.
        seconds: u64,
    },

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
    /// created, [`EngineError::SessionStart`] if librqbit fails to bind its
    /// listener or restore persisted state, or
    /// [`EngineError::SessionStartTimeout`] if it never finishes.
    pub async fn start(config: EngineConfig) -> Result<Self, EngineError> {
        Self::start_within(config, SESSION_START_TIMEOUT).await
    }

    /// [`Self::start`] with an explicit deadline.
    ///
    /// The seam exists for tests: reproducing the hang in #154 needs a real
    /// session directory and a real DHT, and waiting out the production
    /// deadline to observe it is not a test anyone runs twice.
    ///
    /// # Errors
    ///
    /// As [`Self::start`].
    pub async fn start_within(
        config: EngineConfig,
        deadline: Duration,
    ) -> Result<Self, EngineError> {
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

            // Outgoing peer connections go through the proxy when one is
            // configured. Left as None, librqbit connects directly.
            connect: config
                .proxy_url
                .as_ref()
                .map(|proxy_url| ConnectionOptions {
                    proxy_url: Some(proxy_url.clone()),
                    ..Default::default()
                }),

            client_name_and_version: Some(format!("Flume {}", env!("CARGO_PKG_VERSION"))),

            ..Default::default()
        };

        // Bounded rather than awaited outright. librqbit's restore loop is
        // `while !added_all || !futs.is_empty()`, so a single restore future
        // that never completes holds the whole construction open -- and the
        // magnet path it takes for a sidecar-less row has no timeout inside
        // librqbit at all. Without this, `Engine::start` never returns, which
        // means `AppState::restart_engine` never returns, which means the
        // caller in `crate::guard` never returns and the guard loop is never
        // spawned. See #154.
        let session = tokio::time::timeout(
            deadline,
            Session::new_with_opts(config.download_dir.clone(), opts),
        )
        .await
        .map_err(|_| EngineError::SessionStartTimeout {
            seconds: deadline.as_secs(),
        })?
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
    /// Piece availability for one torrent, or `None` if there is nothing to
    /// judge from.
    ///
    /// Needs every connected peer's bitfield, because *which* pieces a peer
    /// holds is the whole question — a count cannot tell a swarm that holds
    /// every piece from one whose peers all stopped in the same place.
    ///
    /// `api_dump_haves` is used only for its piece count; our own bitfield is
    /// not part of the verdict, since the question is what the *swarm* has.
    fn availability_of(
        &self,
        id: usize,
        handle: &ManagedTorrent,
    ) -> Option<availability::Analysis> {
        // Peers first. `api_dump_haves` clones our own bitfield, which is
        // wasted work on a torrent with nobody connected — and that is the
        // common case in a library that is mostly seeding or idle.
        let bitfields: Vec<Vec<u8>> = handle
            .live()?
            .per_peer_stats_snapshot(PeerStatsFilter {
                include_bitfield: true,
                ..Default::default()
            })
            .peers
            .into_values()
            .filter_map(|peer| peer.have_bitfield)
            .collect();

        if bitfields.is_empty() {
            return None;
        }

        let (_, total_pieces) = Api::new(Arc::clone(&self.session), None)
            .api_dump_haves(TorrentIdOrHash::Id(id))
            .ok()?;

        availability::analyse(&bitfields, total_pieces)
    }

    /// session's internal ordering is not guaranteed.
    pub fn torrent_summaries(&self) -> Vec<TorrentSummary> {
        self.torrent_summaries_with(&std::collections::HashMap::new())
    }

    /// [`Self::torrent_summaries`] with arrival times from the library record.
    ///
    /// Threaded in as a parameter the way `availability` already is, because
    /// the record lives in `AppState`, above the engine — and this module
    /// imports no Tauri types and does not reach upwards.
    pub fn torrent_summaries_with(
        &self,
        added: &std::collections::HashMap<String, u64>,
    ) -> Vec<TorrentSummary> {
        // Handles are collected before anything is computed from them:
        // `availability_of` goes back through the session, and doing that while
        // `with_torrents` holds its lock would re-enter it.
        let handles = self.session.with_torrents(|torrents| {
            torrents
                .map(|(id, handle)| (id, Arc::clone(handle)))
                .collect::<Vec<_>>()
        });

        let mut summaries = handles
            .into_iter()
            .map(|(id, handle)| {
                let stats = handle.stats();

                // Availability is O(peers x pieces) and runs per torrent per
                // tick, so it is only asked for where it changes the answer.
                // `classify_health` reads it for a downloading torrent and
                // ignores it for every other state, which is what makes this
                // a skip rather than a staleness trade.
                let availability = (torrent::classify_state(&stats.state, stats.finished)
                    == torrent::TorrentState::Downloading)
                    .then(|| self.availability_of(id, &handle).map(|a| a.summary))
                    .flatten();

                // `as_string` is hex encoding. `Id20`'s Debug impl happens
                // to produce the same thing, but relying on a Debug format for
                // a wire value is a trap.
                let info_hash = handle.info_hash().as_string();
                let added_at = added.get(&info_hash).copied();

                torrent::summarize(
                    id,
                    info_hash,
                    handle.name(),
                    handle.output_folder().display().to_string(),
                    &stats,
                    availability,
                    added_at,
                )
            })
            .collect::<Vec<_>>();
        summaries.sort_by_key(|s| s.id);
        summaries
    }

    /// Builds the full telemetry payload pushed to the UI each tick.
    pub fn telemetry(&self) -> TelemetrySnapshot {
        self.telemetry_with(&std::collections::HashMap::new())
    }

    /// [`Self::telemetry`] with arrival times from the library record.
    pub fn telemetry_with(
        &self,
        added: &std::collections::HashMap<String, u64>,
    ) -> TelemetrySnapshot {
        TelemetrySnapshot {
            core: self.core_status(),
            torrents: self.torrent_summaries_with(added),
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
        self.preview_within(source, MAGNET_METADATA_TIMEOUT).await
    }

    /// As [`Self::preview`], with the deadline supplied.
    ///
    /// [`Self::preview`] is the entry point; this exists so the timeout path
    /// can be tested in milliseconds rather than in whatever the real deadline
    /// happens to be. Public only because integration tests are a separate
    /// crate — `flume_lib` is this application's internals, not a library
    /// anyone else consumes.
    pub async fn preview_within(
        &self,
        source: TorrentSource,
        timeout: Duration,
    ) -> Result<TorrentPreview, EngineError> {
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

        let listing = self.session.add_torrent(
            add,
            Some(AddTorrentOptions {
                list_only: true,
                ..Default::default()
            }),
        );

        // Only a magnet needs bounding. A `.torrent` already carries its file
        // list, so that path is local work that either succeeds or fails; a
        // magnet has to be answered by a peer that has the torrent, and if
        // none does it waits forever. Without this the add dialog sits on
        // "Fetching the file list from peers" indefinitely, which reads
        // identically at two seconds and at twenty minutes.
        //
        // Dropping the future on timeout cancels the add: nothing is left in
        // the session, because a list-only add is not registered as a torrent
        // until it returns.
        let response =
            match &source {
                TorrentSource::Magnet { .. } => tokio::time::timeout(timeout, listing)
                    .await
                    .map_err(|_| EngineError::MetadataTimeout {
                        seconds: timeout.as_secs(),
                    })?,
                TorrentSource::File { .. } => listing.await,
            }
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

        // librqbit resolves the output folder for a list-only add without
        // creating it, so this is where the files would land.
        let save_path = listed.output_folder.clone();
        let free = preflight::free_bytes(&save_path);
        let on_disk = preflight::already_on_disk(&save_path, &files);

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
            save_path: save_path.display().to_string(),
            free_bytes: free,
            seen_peers: listed.seen_peers.len(),
            already_on_disk: on_disk,
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
    /// Returns the info hash of what was removed.
    ///
    /// Captured from the handle *before* the delete, and returned rather than
    /// discarded, because the only id-to-hash mapping lives in the session
    /// entry that `delete` destroys — after the call there is no way to learn
    /// which torrent went. Combined with id recycling, a caller that looked the
    /// hash up afterwards would eventually write against a torrent that now
    /// holds the same id. See #145.
    pub async fn remove(&self, id: usize, delete_files: bool) -> Result<String, EngineError> {
        // Fail on an unknown id before deleting anything. The handle is also
        // the last chance to learn the info hash.
        let info_hash = self.handle(id)?.info_hash().as_string();
        self.session
            .delete(TorrentIdOrHash::Id(id), delete_files)
            .await
            .map_err(EngineError::Operation)?;
        Ok(info_hash)
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

        // Read the piece bitfield once and slice it per file, rather than
        // asking for it inside the loop. It is the same call the whole-torrent
        // heatmap uses; `None` here just means no fragment strips, which is
        // correct for an initializing or errored torrent.
        let have: Option<Vec<bool>> = Api::new(Arc::clone(&self.session), None)
            .api_dump_haves(TorrentIdOrHash::Id(id))
            .ok()
            .map(|(bitfield, total)| bitfield.iter().by_vals().take(total as usize).collect());

        handle
            .with_metadata(|meta| {
                meta.file_infos
                    .iter()
                    .enumerate()
                    .map(|(index, info)| {
                        let first_piece = info.piece_range.start;
                        let last_piece = info.piece_range.end;
                        TorrentFileState {
                            index,
                            path: info.relative_filename.to_string_lossy().replace('\\', "/"),
                            length: info.len,
                            progress_bytes: stats.file_progress.get(index).copied().unwrap_or(0),
                            selected: selection.as_ref().is_none_or(|only| only.contains(&index)),
                            first_piece,
                            last_piece,
                            piece_buckets: have
                                .as_deref()
                                .map(|h| detail::downsample_file_pieces(h, first_piece, last_piece))
                                .unwrap_or_default(),
                        }
                    })
                    .collect()
            })
            .map_err(EngineError::Metadata)
    }

    /// Collects peers, trackers, and piece completion for the detail view.
    ///
    /// Peers and pieces are only available while a torrent is live or paused;
    /// both degrade to empty rather than erroring, because an initializing
    /// torrent legitimately has neither and the UI should show a quiet empty
    /// state, not a failure.
    ///
    /// # Errors
    ///
    /// [`EngineError::UnknownTorrent`] if no such torrent exists.
    pub fn torrent_detail(&self, id: usize) -> Result<TorrentDetail, EngineError> {
        let handle = self.handle(id)?;

        let peers = handle
            .live()
            .map(|live| {
                live.per_peer_stats_snapshot(Default::default())
                    .peers
                    .into_iter()
                    .map(|(address, stats)| detail::PeerInfo {
                        address,
                        client: stats.client_name,
                        transport: stats.conn_kind.map(|k| format!("{k:?}").to_lowercase()),
                        state: stats.state.to_string(),
                        downloaded_bytes: stats.counters.fetched_bytes,
                        uploaded_bytes: stats.counters.uploaded_bytes,
                        pieces_contributed: stats.counters.downloaded_and_checked_pieces,
                        errors: stats.counters.errors,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut trackers: Vec<String> = handle
            .shared()
            .trackers
            .iter()
            .map(ToString::to_string)
            .collect();
        // A HashSet has no order; sorting keeps the list from reshuffling
        // every time the panel is opened.
        trackers.sort();

        // Computed once: the health verdict and the swarm figures must agree,
        // and it walks every peer's bitfield.
        let avail = self.availability_of(id, &handle);

        // `api_dump_haves` is the only public route to the piece bitfield --
        // `ManagedTorrent::with_chunk_tracker` is crate-private. It errors when
        // the torrent is neither live nor paused, which is not a failure worth
        // surfacing.
        let pieces = Api::new(Arc::clone(&self.session), None)
            .api_dump_haves(TorrentIdOrHash::Id(id))
            .ok()
            .map(|(bitfield, total_pieces)| {
                detail::downsample_pieces(
                    bitfield.iter().by_vals(),
                    total_pieces,
                    avail.as_ref().map(|a| a.copies.as_slice()),
                )
            });

        // Peer-pool counts come from the live state's aggregate stats. They
        // are pool health, not a seeds/leechers split -- see SwarmStats.
        let swarm = handle
            .live()
            .map(|live| {
                let p = live.stats_snapshot().peer_stats;
                detail::SwarmStats {
                    live: p.live as usize,
                    connecting: p.connecting as usize,
                    queued: p.queued as usize,
                    seen: p.seen as usize,
                    dead: p.dead as usize,
                    live_tcp: p.live_tcp as usize,
                    live_utp: p.live_utp as usize,
                    seeds: avail.as_ref().map(|a| a.summary.seeds as usize),
                    availability: avail.as_ref().map(|a| a.summary.average),
                    rarest: avail.as_ref().map(|a| a.summary.rarest),
                }
            })
            .unwrap_or(detail::SwarmStats {
                live: 0,
                connecting: 0,
                queued: 0,
                seen: 0,
                dead: 0,
                live_tcp: 0,
                live_utp: 0,
                // Not live, so there is nothing to judge availability from.
                // None rather than zero: no seeds seen is not the same claim
                // as no seeds exist.
                seeds: None,
                availability: None,
                rarest: None,
            });

        // Built from the same summary the list row shows, so the panel's
        // sentence and the row's line above it cannot disagree about what is
        // happening.

        // The detail panel does not carry the arrival time -- the row above it
        // already shows it, and threading the library down here would make
        // this call the only reason the engine needed it.
        let summary = torrent::summarize(
            id,
            handle.info_hash().as_string(),
            handle.name(),
            handle.output_folder().display().to_string(),
            &handle.stats(),
            avail.as_ref().map(|a| a.summary),
            None,
        );
        let note = note::describe(&summary, &swarm);

        // Ranked from the same summary the row shows, so the panel cannot
        // contradict the sentence above it.
        let bottleneck = bottleneck::compute(
            summary.state,
            summary.download_bps,
            self.current_limits().0,
            avail.as_ref().map(|a| a.summary),
            summary.live_peers,
        );

        Ok(TorrentDetail {
            peers,
            trackers,
            pieces,
            swarm,
            note,
            bottleneck,
        })
    }

    /// Finds other BitTorrent clients installed for the current user.
    ///
    /// Returns an empty list when the platform exposes no home directory,
    /// rather than failing: a first-run screen that errors because it could
    /// not go looking is worse than one that simply offers nothing.
    #[must_use]
    pub fn detect_clients() -> Vec<import::DetectedClient> {
        let Some(user) = directories::UserDirs::new() else {
            return Vec::new();
        };
        import::detect(user.home_dir())
    }

    /// Adds every `.torrent` in `torrents_dir`, saving into `output_folder`.
    ///
    /// Each torrent is added with `overwrite`, which is what makes this a
    /// takeover rather than a re-download: librqbit hashes what is already on
    /// disk at that path and keeps every piece that verifies. A torrent the
    /// other client had finished arrives complete and starts seeding.
    ///
    /// Failures are counted, not propagated. A directory of torrents will
    /// contain the occasional truncated or half-written file, and losing the
    /// other forty-six because of one is not a trade worth making. The counts
    /// are what the UI reports.
    ///
    /// # Errors
    ///
    /// Never returns `Err`; the signature is `Result` for symmetry with the
    /// other commands and to leave room for a future failure mode.
    pub async fn import_from(
        &self,
        torrents_dir: &std::path::Path,
        output_folder: Option<String>,
    ) -> Result<(ImportOutcome, Vec<String>), EngineError> {
        let mut outcome = ImportOutcome::default();
        // The hashes of what was actually added, for the library record. Kept
        // out of `ImportOutcome` because that crosses IPC and the frontend has
        // no use for them -- the counts are the whole of what it renders.
        let mut added = Vec::new();

        for path in import::torrent_files(torrents_dir) {
            let Ok(bytes) = std::fs::read(&path) else {
                outcome.failed += 1;
                continue;
            };

            let response = self
                .session
                .add_torrent(
                    AddTorrent::from_bytes(bytes),
                    Some(AddTorrentOptions {
                        overwrite: true,
                        output_folder: output_folder.clone(),
                        ..Default::default()
                    }),
                )
                .await;

            match response {
                Ok(AddTorrentResponse::Added(_, handle)) => {
                    outcome.added += 1;
                    added.push(handle.info_hash().as_string());
                }
                // Already in the session, e.g. a second run of the import or a
                // torrent the user had added by hand. Not a failure.
                Ok(AddTorrentResponse::AlreadyManaged(..)) => outcome.skipped += 1,
                Ok(AddTorrentResponse::ListOnly(_)) | Err(_) => outcome.failed += 1,
            }
        }

        Ok((outcome, added))
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
