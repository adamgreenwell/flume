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

/// Health of the peer pool for one torrent.
///
/// These are *pool* counts, not seeds versus leechers. librqbit v9 tracks
/// whether a peer holds the whole torrent (`LivePeerState::has_full_torrent`),
/// but `PeerStates` is private on the live state, so the seed count is derived
/// from the peers' bitfields instead — see [`crate::engine::availability`].
///
/// `Eq` is not derived: [`Self::availability`] is a mean and so a float.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmStats {
    /// Peers with an established connection right now.
    pub live: usize,
    /// Peers currently being connected to.
    pub connecting: usize,
    /// Known peers waiting for a connection slot.
    pub queued: usize,
    /// Distinct peers discovered for this torrent, ever.
    pub seen: usize,
    /// Peers that failed and were dropped.
    pub dead: usize,
    /// Live peers connected over TCP.
    pub live_tcp: usize,
    /// Live peers connected over uTP.
    pub live_utp: usize,
    /// Connected peers holding every piece.
    ///
    /// `None` when there were no bitfields to judge from, which is not the
    /// same as zero seeds and must not be rendered as it.
    pub seeds: Option<usize>,
    /// Mean copies of each piece across the connected peers.
    ///
    /// The figure other clients label "availability". A sense of depth only —
    /// it cannot stand in for [`Self::rarest`], since a swarm averaging four
    /// copies can still be missing a piece outright.
    pub availability: Option<f64>,
    /// Copies of the least-held piece.
    ///
    /// Zero means the torrent cannot finish from this swarm, however deep the
    /// average is.
    pub rarest: Option<u32>,
}

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
    /// Pieces this peer supplied that passed verification.
    ///
    /// The most honest measure of whether a peer is actually helping: bytes
    /// can be sent and then fail their hash check.
    pub pieces_contributed: u32,
    /// Errors encountered on this connection.
    pub errors: u32,
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
    /// How many of them are downloaded and verified.
    ///
    /// Counted from the bitfield rather than inferred from the buckets: a
    /// bucket holds an averaged level, so summing them back up would give an
    /// estimate where an exact number is free.
    pub pieces_complete: u32,
    /// How many pieces each bucket represents.
    pub pieces_per_bucket: u32,
    /// Completion level per bucket, `0..=255`.
    pub buckets: Vec<u8>,
    /// Copies of the *least-held* piece in each bucket, same bucketing as
    /// [`Self::buckets`] so the two strips line up column for column.
    ///
    /// The minimum rather than the mean: this strip exists to show where a
    /// torrent is about to stall, and a region averaging eight copies while
    /// containing one piece nobody holds is exactly the case a mean would
    /// hide.
    ///
    /// `None` when there were no peer bitfields to judge from — not the same
    /// as a region held by nobody, and not rendered as one. Saturates at
    /// `u16::MAX`, which no real swarm approaches.
    pub availability: Option<Vec<u16>>,
}

/// Everything the detail view shows beyond the file list.
///
/// `Eq` is not derived: it carries [`SwarmStats`], which holds a mean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Peer pool health.
    pub swarm: SwarmStats,
    /// What this torrent is actually doing, in words.
    ///
    /// The panel's reason for existing. Carried on the detail payload rather
    /// than the 1 Hz summary because only an expanded row shows it, and a
    /// three-sentence string per torrent per second is a lot of IPC for
    /// something nobody is reading.
    pub note: super::note::Note,
    /// What is limiting this download, or `None` when the question does not
    /// apply — a paused or seeding torrent is not being limited.
    pub bottleneck: Option<crate::engine::bottleneck::Bottleneck>,
}

/// Downsamples a per-piece present/absent iterator into a compact heatmap.
///
/// `total_pieces` is taken separately rather than from the iterator length,
/// because the underlying bitfield is byte-padded and so is usually longer
/// than the real piece count. Counting the padding as missing pieces would
/// draw a permanently incomplete tail on every finished torrent.
pub(super) fn downsample_pieces(
    have: impl Iterator<Item = bool>,
    total_pieces: u32,
    copies: Option<&[u32]>,
) -> PieceMap {
    let total = total_pieces as usize;
    if total == 0 {
        return PieceMap {
            total_pieces: 0,
            pieces_complete: 0,
            pieces_per_bucket: 0,
            buckets: Vec::new(),
            availability: None,
        };
    }

    let bucket_count = total.min(MAX_PIECE_BUCKETS);
    // Round up so the last bucket absorbs any remainder rather than the
    // remainder needing an extra, mostly-empty bucket.
    let pieces_per_bucket = total.div_ceil(bucket_count);

    let mut buckets = Vec::with_capacity(bucket_count);
    // Built in the same loop rather than a second pass, so the two strips
    // cannot fall out of step with each other.
    let mut rarest_buckets = copies.map(|_| Vec::with_capacity(bucket_count));
    let mut present_in_bucket = 0usize;
    let mut seen_in_bucket = 0usize;
    let mut rarest_in_bucket = u32::MAX;
    let mut complete = 0u32;

    for (piece, present) in have.take(total).enumerate() {
        if present {
            present_in_bucket += 1;
            complete += 1;
        }
        if let Some(all) = copies {
            // A short `copies` claims nothing about the pieces it omits, so
            // those leave the running minimum alone.
            if let Some(n) = all.get(piece) {
                rarest_in_bucket = rarest_in_bucket.min(*n);
            }
        }
        seen_in_bucket += 1;

        if seen_in_bucket == pieces_per_bucket {
            buckets.push(level(present_in_bucket, seen_in_bucket));
            if let Some(out) = rarest_buckets.as_mut() {
                out.push(saturate(rarest_in_bucket));
            }
            present_in_bucket = 0;
            seen_in_bucket = 0;
            rarest_in_bucket = u32::MAX;
        }
    }

    // Flush a partial final bucket.
    if seen_in_bucket > 0 {
        buckets.push(level(present_in_bucket, seen_in_bucket));
        if let Some(out) = rarest_buckets.as_mut() {
            out.push(saturate(rarest_in_bucket));
        }
    }

    PieceMap {
        total_pieces,
        pieces_complete: complete,
        pieces_per_bucket: u32::try_from(pieces_per_bucket).unwrap_or(u32::MAX),
        buckets,
        availability: rarest_buckets,
    }
}

/// Narrows a copy count for the wire.
///
/// `u32::MAX` is the "nothing seen" sentinel from the running minimum and
/// means the bucket had no counts at all, which reads as zero copies.
fn saturate(rarest: u32) -> u16 {
    if rarest == u32::MAX {
        return 0;
    }
    u16::try_from(rarest).unwrap_or(u16::MAX)
}

/// Downsamples one file's slice of the piece bitfield.
///
/// Files are shown in a narrow row, so the cap is lower than the whole-torrent
/// heatmap's. A file spanning three pieces gets three buckets; one spanning
/// thousands gets [`MAX_FILE_PIECE_BUCKETS`].
pub(super) fn downsample_file_pieces(have: &[bool], first: u32, last: u32) -> Vec<u8> {
    let (start, end) = (first as usize, last as usize);
    if end <= start || start >= have.len() {
        return Vec::new();
    }
    let slice = &have[start..end.min(have.len())];
    let bucket_count = slice.len().min(MAX_FILE_PIECE_BUCKETS);
    if bucket_count == 0 {
        return Vec::new();
    }
    let per_bucket = slice.len().div_ceil(bucket_count);

    slice
        .chunks(per_bucket)
        .map(|chunk| level(chunk.iter().filter(|p| **p).count(), chunk.len()))
        .collect()
}

/// Upper bound on buckets in a per-file fragment strip.
///
/// Lower than the whole-torrent cap because a file row is a fraction of the
/// window's width; more buckets than pixels buys nothing.
pub const MAX_FILE_PIECE_BUCKETS: usize = 400;

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
        let map = downsample_pieces(std::iter::empty(), 0, None);
        assert!(map.buckets.is_empty());
        assert_eq!(map.total_pieces, 0);
    }

    #[test]
    fn small_torrents_map_one_piece_per_bucket() {
        let map = downsample_pieces([true, false, true, true].into_iter(), 4, None);
        assert_eq!(map.pieces_per_bucket, 1);
        assert_eq!(map.buckets, vec![255, 0, 255, 255]);
    }

    #[test]
    fn a_complete_torrent_is_fully_saturated() {
        let map = downsample_pieces(std::iter::repeat_n(true, 5000), 5000, None);
        assert!(
            map.buckets.iter().all(|&b| b == 255),
            "every bucket should be full"
        );
    }

    #[test]
    fn an_empty_download_is_all_zero() {
        let map = downsample_pieces(std::iter::repeat_n(false, 5000), 5000, None);
        assert!(map.buckets.iter().all(|&b| b == 0));
    }

    #[test]
    fn large_torrents_are_capped_at_the_bucket_limit() {
        let map = downsample_pieces(std::iter::repeat_n(true, 500_000), 500_000, None);
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
        let map = downsample_pieces(padded, 3, None);
        assert_eq!(map.buckets, vec![255, 255, 255]);
    }

    #[test]
    fn buckets_only_group_once_pieces_exceed_the_cap() {
        // At or below the cap, each bucket holds exactly one piece, so an
        // alternating pattern stays fully saturated or fully empty.
        let small = downsample_pieces([true, false].into_iter().cycle().take(10), 10, None);
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
            None,
        );

        assert_eq!(map.pieces_per_bucket, 2);
        assert!(
            map.buckets.iter().all(|&b| (120..=135).contains(&b)),
            "expected mid-range levels, got a sample of {:?}",
            &map.buckets[..5.min(map.buckets.len())]
        );
    }

    #[test]
    fn a_file_maps_to_its_own_piece_range() {
        // Pieces 2..5 present, everything else absent. A file covering 2..5
        // should read as complete even though the torrent is not.
        let have = [false, false, true, true, true, false, false];
        assert_eq!(downsample_file_pieces(&have, 2, 5), vec![255, 255, 255]);
    }

    #[test]
    fn a_file_outside_the_bitfield_yields_nothing() {
        // Rather than panicking on a range past the end, which can happen
        // transiently while metadata and piece state disagree.
        let have = [true, true];
        assert!(downsample_file_pieces(&have, 5, 9).is_empty());
    }

    #[test]
    fn a_file_range_is_clamped_to_the_bitfield() {
        let have = [true, true, true];
        assert_eq!(downsample_file_pieces(&have, 1, 99).len(), 2);
    }

    #[test]
    fn an_empty_file_range_yields_nothing() {
        // A zero-length file occupies no pieces.
        let have = [true, true, true];
        assert!(downsample_file_pieces(&have, 2, 2).is_empty());
    }

    #[test]
    fn a_large_file_is_capped_at_the_file_bucket_limit() {
        let have = vec![true; 10_000];
        let buckets = downsample_file_pieces(&have, 0, 10_000);
        assert!(
            buckets.len() <= MAX_FILE_PIECE_BUCKETS,
            "expected at most {MAX_FILE_PIECE_BUCKETS}, got {}",
            buckets.len()
        );
    }

    #[test]
    fn a_partial_final_bucket_is_still_emitted() {
        // 7 pieces with 1600 max buckets → 1 piece per bucket, 7 buckets.
        let map = downsample_pieces(std::iter::repeat_n(true, 7), 7, None);
        assert_eq!(map.buckets.len(), 7);
    }

    /// The two strips are drawn stacked, so a column in one must describe the
    /// same pieces as the column above it.
    #[test]
    fn availability_buckets_line_up_with_completion_buckets() {
        let pieces = 5000;
        let copies: Vec<u32> = (0..pieces).map(|i| i % 7).collect();
        let map = downsample_pieces(
            std::iter::repeat_n(true, pieces as usize),
            pieces,
            Some(&copies),
        );

        let availability = map.availability.expect("availability");
        assert_eq!(availability.len(), map.buckets.len());
    }

    /// The minimum, not the mean: a region averaging plenty while containing
    /// one piece nobody holds is the case this strip exists to show.
    #[test]
    fn a_bucket_reports_its_rarest_piece() {
        // Twice MAX_PIECE_BUCKETS, so each bucket covers exactly two pieces.
        let pieces = MAX_PIECE_BUCKETS * 2;
        let mut copies = vec![9u32; pieces];
        copies[1] = 0; // second piece of the first bucket

        let map = downsample_pieces(
            std::iter::repeat_n(true, pieces),
            u32::try_from(pieces).unwrap(),
            Some(&copies),
        );

        assert_eq!(map.pieces_per_bucket, 2);
        let availability = map.availability.expect("availability");
        // The bucket takes the rarest of its two pieces, not their mean.
        assert_eq!(availability[0], 0);
        assert_eq!(availability[1], 9);
    }

    /// No bitfields to judge from is not a swarm holding nothing.
    #[test]
    fn absent_copies_leave_availability_absent() {
        let map = downsample_pieces([true; 4].into_iter(), 4, None);
        assert_eq!(map.availability, None);
    }

    /// A short `copies` must not panic. Below MAX_PIECE_BUCKETS each bucket is
    /// a single piece, so the uncovered ones read as zero rather than
    /// borrowing a neighbour's count.
    #[test]
    fn a_short_copies_slice_does_not_panic() {
        let copies = [3, 4];
        let map = downsample_pieces([true; 4].into_iter(), 4, Some(&copies));
        assert_eq!(map.availability, Some(vec![3, 4, 0, 0]));
    }
}
