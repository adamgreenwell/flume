//! Piece availability across a swarm.
//!
//! Answers the question a peer count cannot: *will this finish?* A torrent
//! connected to forty peers who all stopped at 6% will not; one connected to
//! six peers who between them hold every piece will.
//!
//! This needs to know **which** pieces each peer holds, not how many, so it
//! works from the peers' bitfields. librqbit tracks those for piece picking but
//! does not serialise them upstream yet — see `ikatson/rqbit#643` and the
//! `[patch.crates-io]` entry in `Cargo.toml`.

/// What the swarm holds, derived from the connected peers' bitfields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Availability {
    /// Copies of the least-held piece.
    ///
    /// Zero means no connected peer holds some piece, so the torrent cannot
    /// finish from this swarm however fast the rest arrives. This is the
    /// figure that decides whether a download is viable.
    pub rarest: u32,
    /// Mean copies per piece, across the whole torrent.
    ///
    /// The figure other clients label "availability". Useful as a sense of
    /// depth, but it cannot stand in for [`Self::rarest`]: a swarm averaging
    /// 4.0 copies can still be missing a piece entirely.
    pub average: f64,
    /// Connected peers holding every piece.
    pub seeds: u32,
}

/// The full analysis: the summary plus the per-piece copy counts it came from.
///
/// Kept together because both come from one pass over the peers' bitfields,
/// and the caller wants both — the summary for the verdict, the counts for the
/// availability histogram.
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    /// What the swarm holds, in aggregate.
    pub summary: Availability,
    /// Connected peers holding each piece, indexed by piece.
    pub copies: Vec<u32>,
}

/// Counts how many connected peers hold each piece.
///
/// `bitfields` are the raw bytes each peer sent. Bitfields are big-endian by
/// piece — piece 0 is the high bit of byte 0 — and byte-padded, so the trailing
/// bits past `total_pieces` are spare and ignored. A peer is not obliged to
/// have zeroed them, and librqbit does not enforce that it did.
///
/// Returns `None` when there is nothing to judge from: no peers, or a torrent
/// whose piece count is not known yet. A caller must not read that as a
/// healthy swarm, nor as a broken one.
pub fn analyse(bitfields: &[Vec<u8>], total_pieces: u32) -> Option<Analysis> {
    if bitfields.is_empty() || total_pieces == 0 {
        return None;
    }

    let total = total_pieces as usize;
    let mut copies = vec![0u32; total];
    let mut seeds = 0;

    for bitfield in bitfields {
        let mut held = 0usize;
        // Bytes past the last piece are padding, and a short bitfield claims
        // nothing about the pieces it omits — both drop out of the range here
        // rather than being checked per piece.
        let usable = bitfield.len().min(total.div_ceil(8));

        for (index, byte) in bitfield[..usable].iter().enumerate() {
            // Most of a live swarm's bytes are empty on a torrent that is not
            // nearly done, and skipping them whole is the difference between
            // this being cheap and being the most expensive thing in a tick.
            if *byte == 0 {
                continue;
            }
            let base = index * 8;
            // The final byte usually runs past the last real piece.
            let bits = 8.min(total - base);
            for bit in 0..bits {
                // Msb0: piece n lives in byte n/8, counting from the high bit.
                if byte & (0x80 >> bit) != 0 {
                    copies[base + bit] += 1;
                    held += 1;
                }
            }
        }

        if held == total {
            seeds += 1;
        }
    }

    let sum: u64 = copies.iter().map(|c| u64::from(*c)).sum();
    Some(Analysis {
        summary: Availability {
            rarest: copies.iter().copied().min().unwrap_or(0),
            average: sum as f64 / total as f64,
            seeds,
        },
        copies,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{Availability, analyse};

    /// Two peers with the same piece count can be very different swarms.
    ///
    /// This is the case a per-peer count cannot distinguish, and the reason
    /// this module works from bitfields.
    #[test]
    fn overlapping_peers_are_not_the_same_as_complementary_ones() {
        // 4 pieces. Two peers, two pieces each.
        let same = vec![vec![0b1100_0000], vec![0b1100_0000]];
        let split = vec![vec![0b1100_0000], vec![0b0011_0000]];

        // Identical piece counts either way.
        assert_eq!(analyse(&same, 4).unwrap().summary.average, 1.0);
        assert_eq!(analyse(&split, 4).unwrap().summary.average, 1.0);

        // But only one of them can finish.
        assert_eq!(analyse(&same, 4).unwrap().summary.rarest, 0);
        assert_eq!(analyse(&split, 4).unwrap().summary.rarest, 1);
    }

    #[test]
    fn counts_seeds_and_rarest() {
        let peers = vec![
            vec![0b1111_0000], // all 4
            vec![0b1111_0000], // all 4
            vec![0b1000_0000], // just piece 0
        ];
        assert_eq!(
            analyse(&peers, 4).unwrap().summary,
            Availability {
                rarest: 2,
                average: 2.25,
                seeds: 2
            }
        );
    }

    /// Spare trailing bits are not pieces and must not be counted.
    #[test]
    fn ignores_padding_past_the_last_piece() {
        // 4 real pieces in a byte that has every bit set.
        let peers = vec![vec![0b1111_1111]];
        let a = analyse(&peers, 4).unwrap().summary;
        assert_eq!(a.rarest, 1);
        assert_eq!(a.average, 1.0);
        assert_eq!(a.seeds, 1);
    }

    #[test]
    fn spans_multiple_bytes() {
        // 10 pieces: peer holds 0 and 9.
        let peers = vec![vec![0b1000_0000, 0b0100_0000]];
        let a = analyse(&peers, 10).unwrap().summary;
        assert_eq!(a.rarest, 0);
        assert_eq!(a.average, 0.2);
    }

    #[test]
    fn nothing_to_judge_from_is_not_a_verdict() {
        assert!(analyse(&[], 4).is_none());
        assert!(analyse(&[vec![0xff]], 0).is_none());
    }

    /// What `analyse` costs per torrent, at realistic swarm sizes.
    ///
    /// This runs once per *downloading* torrent per telemetry tick, so it is
    /// the term that decides whether the swarm verdict can stay at 1 Hz. It is
    /// O(peers x pieces) and nothing else in the suite covers it: the
    /// `telemetry_stays_fast_with_many_torrents` test in `tests/performance.rs`
    /// builds torrents with no peers at all, so it never reaches this code.
    ///
    /// Ignored because it is a timing measurement, which is unreliable on a
    /// shared runner. Run it before a release:
    ///
    /// ```text
    /// cargo test --release --lib availability -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "timing measurement, unreliable on shared CI runners"]
    fn analyse_stays_within_its_share_of_a_telemetry_tick() {
        use std::time::Instant;

        /// Nanoseconds per peer-piece the walk may take.
        ///
        /// Normalised rather than a flat per-torrent budget because the work
        /// is inherently O(peers x pieces): a 50GB torrent in a big swarm
        /// genuinely costs more than a small one, and a fixed budget would
        /// either fail on legitimate scale or pass on a real regression. What
        /// must not change is the cost *per unit of work*, which the byte-wise
        /// walk holds at well under a nanosecond.
        ///
        /// Bounding the aggregate is a separate problem, solved by not calling
        /// this for every torrent every tick — see `Engine::availability_of`.
        const NS_PER_PEER_PIECE: f64 = 2.0;

        // (label, peers, pieces). 12k pieces is a 50GB torrent at 4MiB
        // pieces, and 80 peers is a healthy public swarm.
        let cases = [
            ("typical", 30, 3_000usize),
            ("busy", 50, 12_000),
            ("worst", 80, 25_000),
        ];

        let mut worst_rate = 0.0f64;
        for (label, peers, pieces) in cases {
            let bytes = pieces / 8 + 1;
            // Varied rather than all-set: a swarm of seeds would count the
            // same either way, but this is closer to a live one.
            let bitfields: Vec<Vec<u8>> = (0..peers)
                .map(|p| (0..bytes).map(|i| ((i + p) % 251) as u8).collect())
                .collect();

            let runs = 20;
            let started = Instant::now();
            for _ in 0..runs {
                assert!(analyse(&bitfields, pieces as u32).is_some());
            }
            let each = started.elapsed() / runs;
            #[allow(clippy::cast_precision_loss)]
            let rate = each.as_nanos() as f64 / (peers as f64 * pieces as f64);
            worst_rate = worst_rate.max(rate);

            println!(
                "{label:>8}: {peers:>3} peers x {pieces:>6} pieces => \
                 {each:>9.2?} per torrent ({rate:.2} ns per peer-piece)"
            );
        }

        assert!(
            worst_rate <= NS_PER_PEER_PIECE,
            "the availability walk costs {worst_rate:.2} ns per peer-piece, over the \
             {NS_PER_PEER_PIECE:.2} ns budget. Something has made it scan bit by bit \
             again rather than skipping empty bytes whole."
        );
    }
}
