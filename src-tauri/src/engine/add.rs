//! Types for the two-step add flow.
//!
//! Flume resolves a torrent's file list *before* downloading anything, so a
//! user can deselect files first. That matters for the ISO use case: distro
//! torrents routinely bundle several images plus checksums when the user wants
//! one image.
//!
//! Mirrored in `src/lib/ipc/types.ts`.

use serde::{Deserialize, Serialize};

/// Where a torrent is being added from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TorrentSource {
    /// A `magnet:` URI. Metadata must be fetched from the DHT.
    Magnet {
        /// The full magnet URI.
        uri: String,
    },
    /// A path to a `.torrent` file on disk.
    ///
    /// Carries the *path*, not the bytes. The webview obtains it from the file
    /// picker or a drag-and-drop, and the engine does the reading — so the
    /// frontend needs no filesystem permission at all, and no file contents
    /// cross the IPC boundary.
    File {
        /// Absolute path to a `.torrent` file.
        path: String,
    },
}

/// One file inside a torrent, as shown in the selection tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFile {
    /// Index within the torrent. This is what selection refers to.
    pub index: usize,
    /// Path relative to the torrent root, using forward slashes.
    pub path: String,
    /// Size in bytes.
    pub length: u64,
}

/// Resolved metadata for a torrent that has not been started yet.
///
/// Returned by the preview step so the UI can render a file tree. The
/// resolved `.torrent` bytes are deliberately **not** included: they are held
/// in the engine and looked up by [`Self::info_hash`] when the user confirms,
/// so a magnet's metadata is fetched from the DHT exactly once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentPreview {
    /// Hex info hash; also the key used to confirm the add.
    pub info_hash: String,
    /// Display name from the metadata.
    pub name: String,
    /// Combined size of every file.
    pub total_bytes: u64,
    /// Every file, in torrent order.
    pub files: Vec<TorrentFile>,
    /// Whether this torrent is already in the session.
    ///
    /// The UI shows "already added" rather than silently doing nothing.
    pub already_added: bool,
    /// Where these files would be written if added now.
    pub save_path: String,
    /// Free space on that volume right now, or `None` if it cannot be read.
    ///
    /// `None` rather than zero: the UI renders it as "unknown", and zero free
    /// bytes is a specific and alarming claim to make by accident.
    pub free_bytes: Option<u64>,
    /// Peers that answered while the metadata was being fetched.
    ///
    /// Not a tracker scrape — there is no seeds/leechers split here, only the
    /// peers librqbit actually heard from. It is a real measurement rather
    /// than an estimate, which is why it is worth showing.
    pub seen_peers: usize,
    /// Per file, whether a file of that name and length is already there.
    ///
    /// Parallel to [`Self::files`]. The add sheet deselects these by default
    /// and says why, so a re-add does not silently fetch what you already
    /// have.
    pub already_on_disk: Vec<bool>,
}
