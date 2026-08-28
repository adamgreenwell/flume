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
pub fn compute(bitfields: &[Vec<u8>], total_pieces: u32) -> Option<Availability> {
    if bitfields.is_empty() || total_pieces == 0 {
        return None;
    }

    let total = total_pieces as usize;
    let mut copies = vec![0u32; total];
    let mut seeds = 0;

    for bitfield in bitfields {
        let mut held = 0usize;
        for (piece, count) in copies.iter_mut().enumerate() {
            // Msb0: piece n lives in byte n/8, counting from the high bit.
            let byte = match bitfield.get(piece / 8) {
                Some(b) => *b,
                // A short bitfield claims nothing about the pieces it omits.
                None => break,
            };
            if byte & (0x80 >> (piece % 8)) != 0 {
                *count += 1;
                held += 1;
            }
        }
        if held == total {
            seeds += 1;
        }
    }

    let sum: u64 = copies.iter().map(|c| u64::from(*c)).sum();
    Some(Availability {
        rarest: copies.iter().copied().min().unwrap_or(0),
        average: sum as f64 / total as f64,
        seeds,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{Availability, compute};

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
        assert_eq!(compute(&same, 4).unwrap().average, 1.0);
        assert_eq!(compute(&split, 4).unwrap().average, 1.0);

        // But only one of them can finish.
        assert_eq!(compute(&same, 4).unwrap().rarest, 0);
        assert_eq!(compute(&split, 4).unwrap().rarest, 1);
    }

    #[test]
    fn counts_seeds_and_rarest() {
        let peers = vec![
            vec![0b1111_0000], // all 4
            vec![0b1111_0000], // all 4
            vec![0b1000_0000], // just piece 0
        ];
        assert_eq!(
            compute(&peers, 4).unwrap(),
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
        let a = compute(&peers, 4).unwrap();
        assert_eq!(a.rarest, 1);
        assert_eq!(a.average, 1.0);
        assert_eq!(a.seeds, 1);
    }

    #[test]
    fn spans_multiple_bytes() {
        // 10 pieces: peer holds 0 and 9.
        let peers = vec![vec![0b1000_0000, 0b0100_0000]];
        let a = compute(&peers, 10).unwrap();
        assert_eq!(a.rarest, 0);
        assert_eq!(a.average, 0.2);
    }

    #[test]
    fn nothing_to_judge_from_is_not_a_verdict() {
        assert!(compute(&[], 4).is_none());
        assert!(compute(&[vec![0xff]], 0).is_none());
    }
}
