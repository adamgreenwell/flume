//! Serializable status snapshots handed to the UI.
//!
//! These are Flume's own types, deliberately *not* re-exports of librqbit's
//! internal stats structs. Keeping our own shapes here means a librqbit upgrade
//! cannot silently change the IPC contract the frontend depends on; the
//! compiler forces us to look at the mapping in [`super::Engine`] instead.
//!
//! Every type here has a mirrored TypeScript definition in
//! `src/lib/ipc/types.ts`. Changing one without the other is a defect.

use serde::{Deserialize, Serialize};

/// Health of the DHT subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DhtStatus {
    /// Whether the DHT was enabled in configuration at all.
    pub enabled: bool,

    /// Number of IPv4 nodes in the routing table. Zero while bootstrapping.
    pub nodes_v4: usize,

    /// Number of IPv6 nodes in the routing table.
    pub nodes_v6: usize,

    /// DHT queries currently awaiting a response.
    pub outstanding_requests: usize,
}

impl DhtStatus {
    /// A status representing the DHT being switched off by the user.
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            nodes_v4: 0,
            nodes_v6: 0,
            outstanding_requests: 0,
        }
    }

    /// Total routing-table size across both address families.
    pub const fn total_nodes(&self) -> usize {
        self.nodes_v4 + self.nodes_v6
    }
}

/// Coarse, user-facing readiness of the engine.
///
/// This drives a single status indicator in the UI, so the variants are
/// intentionally few and ordered from worst to best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineHealth {
    /// The session exists but is not yet reachable or peer-discovering.
    Starting,
    /// Bootstrapping: discovery is in progress but not yet useful.
    Connecting,
    /// Peer discovery is working; torrents added now should find peers.
    Ready,
    /// Running, but with a capability disabled or failed (e.g. no DHT).
    Degraded,
}

/// A point-in-time snapshot of engine state, safe to send over IPC.
///
/// Contains only scalars and small strings; no torrent piece data ever crosses
/// this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreStatus {
    /// The librqbit client string, e.g. `"rqbit 9.0.0"`.
    pub client_version: String,

    /// The port actually bound for incoming peer connections, if listening.
    ///
    /// May differ from the configured port when the configured one was taken.
    pub listen_port: Option<u16>,

    /// The port announced to trackers and peers, if any.
    pub announce_port: Option<u16>,

    /// DHT subsystem health.
    pub dht: DhtStatus,

    /// Absolute path downloads are written to, for display.
    pub download_dir: String,

    /// Seconds since the session started.
    pub uptime_seconds: u64,

    /// Current aggregate download rate in bytes per second.
    pub download_bps: u64,

    /// Current aggregate upload rate in bytes per second.
    pub upload_bps: u64,

    /// Peers currently connected across all torrents.
    pub live_peers: u32,

    /// Derived readiness indicator; see [`EngineHealth`].
    pub health: EngineHealth,
}
