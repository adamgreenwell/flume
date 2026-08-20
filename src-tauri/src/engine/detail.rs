//! Per-torrent detail: peers, trackers, and piece completion.
//!
//! Fetched on demand when the detail view is open, never streamed in
//! telemetry. This data is per-torrent and only interesting while someone is
//! looking at it; pushing it every second would grow the telemetry payload
//! with the torrent count for no benefit.
//!
//! Mirrored in `src/lib/ipc/types.ts`.

use serde::{Deserialize, Serialize};

/// Upper bound on heatmap buckets sent to the UI.
///
/// A torrent can have hundreds of thousands of pieces. Sending one entry each
/// would be a large payload to render a strip a few hundred pixels wide, so
/// the bitfield is downsampled here. 1600 is comfortably more than the
/// horizontal resolution any window will give the widget.
pub const MAX_PIECE_BUCKETS: usize = 1600;

/// One connected peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    /// Remote socket address, as `host:port`.
    pub address: String,
    /// Client software the peer reports, if it identified itself.
    pub client: Option<String>,
    /// Transport in use: `tcp`, `utp`, or `socks`.
    pub transport: Option<String>,
    /// librqbit's state label for this peer.
    pub state: String,
    /// Bytes downloaded from this peer.
    pub downloaded_bytes: u64,
    /// Bytes uploaded to this peer.
    pub uploaded_bytes: u64,
}

/// A downsampled view of which pieces are present.
///
/// Each bucket summarises a contiguous run of pieces as a completion level
/// from 0 (none present) to 255 (all present), which is ample resolution for a
/// heatmap and keeps the payload small and fixed-size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceMap {
    /// Total pieces in the torrent.
    pub total_pieces: u32,
    /// How many pieces each bucket represents.
    pub pieces_per_bucket: u32,
    /// Completion level per bucket, `0..=255`.
    pub buckets: Vec<u8>,
}

/// Everything the detail view shows beyond the file list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentDetail {
    /// Connected peers. Empty when the torrent is not live.
    pub peers: Vec<PeerInfo>,
    /// Tracker announce URLs.
    ///
    /// URLs only: librqbit v9 exposes the configured tracker list but not
    /// per-tracker announce status, so there is no "last announce" or peer
    /// count to show. See the wiki's engine notes.
    pub trackers: Vec<String>,
    /// Piece completion, or `None` when the torrent is not live or paused
    /// (there is no chunk tracker to read while initializing or errored).
    pub pieces: Option<PieceMap>,
}

/// Downsamples a per-piece present/absent iterator into a compact heatmap.
///
/// `total_pieces` is taken separately rather than from the iterator length,
/// because the underlying bitfield is byte-padded and so is usually longer
/// than the real piece count. Counting the padding as missing pieces would
/// draw a permanently incomplete tail on every finished torrent.
pub(super) fn downsample_pieces(have: impl Iterator<Item = bool>, total_pieces: u32) -> PieceMap {
    let total = total_pieces as usize;
    if total == 0 {
        return PieceMap {
            total_pieces: 0,
            pieces_per_bucket: 0,
            buckets: Vec::new(),
        };
    }

    let bucket_count = total.min(MAX_PIECE_BUCKETS);
    // Round up so the last bucket absorbs any remainder rather than the
    // remainder needing an extra, mostly-empty bucket.
    let pieces_per_bucket = total.div_ceil(bucket_count);

    let mut buckets = Vec::with_capacity(bucket_count);
    let mut present_in_bucket = 0usize;
    let mut seen_in_bucket = 0usize;

    for present in have.take(total) {
        if present {
            present_in_bucket += 1;
        }
        seen_in_bucket += 1;

        if seen_in_bucket == pieces_per_bucket {
            buckets.push(level(present_in_bucket, seen_in_bucket));
            present_in_bucket = 0;
            seen_in_bucket = 0;
        }
    }

    // Flush a partial final bucket.
    if seen_in_bucket > 0 {
        buckets.push(level(present_in_bucket, seen_in_bucket));
    }

    PieceMap {
        total_pieces,
        pieces_per_bucket: u32::try_from(pieces_per_bucket).unwrap_or(u32::MAX),
        buckets,
    }
}

/// Scales a present/total ratio to `0..=255`.
fn level(present: usize, total: usize) -> u8 {
    if total == 0 {
        return 0;
    }
    // Integer maths throughout: this runs over every piece of every torrent
    // the user opens, and floats buy nothing at 8-bit output resolution.
    u8::try_from(present * 255 / total).unwrap_or(255)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_torrent_produces_no_buckets() {
        let map = downsample_pieces(std::iter::empty(), 0);
        assert!(map.buckets.is_empty());
        assert_eq!(map.total_pieces, 0);
    }

    #[test]
    fn small_torrents_map_one_piece_per_bucket() {
        let map = downsample_pieces([true, false, true, true].into_iter(), 4);
        assert_eq!(map.pieces_per_bucket, 1);
        assert_eq!(map.buckets, vec![255, 0, 255, 255]);
    }

    #[test]
    fn a_complete_torrent_is_fully_saturated() {
        let map = downsample_pieces(std::iter::repeat_n(true, 5000), 5000);
        assert!(
            map.buckets.iter().all(|&b| b == 255),
            "every bucket should be full"
        );
    }

    #[test]
    fn an_empty_download_is_all_zero() {
        let map = downsample_pieces(std::iter::repeat_n(false, 5000), 5000);
        assert!(map.buckets.iter().all(|&b| b == 0));
    }

    #[test]
    fn large_torrents_are_capped_at_the_bucket_limit() {
        let map = downsample_pieces(std::iter::repeat_n(true, 500_000), 500_000);
        assert!(
            map.buckets.len() <= MAX_PIECE_BUCKETS,
            "expected at most {MAX_PIECE_BUCKETS} buckets, got {}",
            map.buckets.len()
        );
        assert_eq!(map.total_pieces, 500_000);
    }

    #[test]
    fn padding_past_the_piece_count_is_ignored() {
        // Bitfields are byte-padded, so the iterator runs longer than the real
        // piece count. Those trailing bits must not be counted as missing.
        let padded = [true, true, true]
            .into_iter()
            .chain(std::iter::repeat_n(false, 5));
        let map = downsample_pieces(padded, 3);
        assert_eq!(map.buckets, vec![255, 255, 255]);
    }

    #[test]
    fn buckets_only_group_once_pieces_exceed_the_cap() {
        // At or below the cap, each bucket holds exactly one piece, so an
        // alternating pattern stays fully saturated or fully empty.
        let small = downsample_pieces([true, false].into_iter().cycle().take(10), 10);
        assert_eq!(small.pieces_per_bucket, 1);
        assert_eq!(small.buckets, vec![255, 0, 255, 0, 255, 0, 255, 0, 255, 0]);
    }

    #[test]
    fn a_half_full_bucket_reads_as_mid_range() {
        // Twice the cap means two pieces per bucket; alternating gives 50%,
        // which is where averaging actually kicks in.
        let pieces = MAX_PIECE_BUCKETS * 2;
        let map = downsample_pieces(
            [true, false].into_iter().cycle().take(pieces),
            u32::try_from(pieces).unwrap(),
        );

        assert_eq!(map.pieces_per_bucket, 2);
        assert!(
            map.buckets.iter().all(|&b| (120..=135).contains(&b)),
            "expected mid-range levels, got a sample of {:?}",
            &map.buckets[..5.min(map.buckets.len())]
        );
    }

    #[test]
    fn a_partial_final_bucket_is_still_emitted() {
        // 7 pieces with 1600 max buckets → 1 piece per bucket, 7 buckets.
        let map = downsample_pieces(std::iter::repeat_n(true, 7), 7);
        assert_eq!(map.buckets.len(), 7);
    }
}
