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
    /// The contents of a `.torrent` file, which already contain metadata.
    ///
    /// This is torrent *metadata* — a few kilobytes of file names and piece
    /// hashes — not piece data. It is the one thing that legitimately crosses
    /// the boundary, because the user picked the file in the webview.
    File {
        /// Raw `.torrent` bytes.
        bytes: Vec<u8>,
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
}
