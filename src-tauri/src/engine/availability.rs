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
    /// Copies of the least-held piece **this download still needs**.
    ///
    /// Zero means no connected peer holds some piece that is still missing,
    /// so the torrent cannot finish from this swarm however fast the rest
    /// arrives. This is the figure that decides whether a download is viable.
    ///
    /// Taken over the remainder only — selected and not yet verified — never
    /// the whole torrent. A piece already on disk, or one belonging only to a
    /// file the user left out, cannot stop the download finishing, and
    /// counting it read a peer holding everything still wanted as "No seeds"
    /// whenever it lacked one piece the user already had or never asked for:
    /// exactly what a partial seed of the main file looks like.
    ///
    /// When nothing is still needed the minimum is over an empty set, and it
    /// is reported as the peer count — every needed piece is held by every
    /// peer, vacuously — rather than as zero, which would be a false "cannot
    /// finish" in the tick before a download flips to seeding.
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
    /// Whether a zero [`Availability::rarest`] has lasted long enough to be
    /// believed. Always `true` from [`analyse`]; the engine lowers it through
    /// a [`GapClock`]. See [`Self::settled`].
    pub gap_settled: bool,
}

impl Analysis {
    /// The summary to judge the swarm by, or `None` while it cannot be judged.
    ///
    /// `None` when a needed piece is missing from every bitfield but has not
    /// been missing for [`GAP_SETTLE`] yet. Some clients — BitTorrent and
    /// µTorrent among them — announce a seed as *nearly* complete and send the
    /// last pieces as `Have` messages moments later, so for its first few
    /// seconds a real seed is indistinguishable from a peer that lacks those
    /// pieces. Reading that as "cannot finish" flashed **No seeds** beside a
    /// connected seed. The copies stay as reported, so the histogram still
    /// shows exactly what peers have claimed; only the verdict waits.
    pub fn settled(&self) -> Option<Availability> {
        (self.summary.rarest > 0 || self.gap_settled).then_some(self.summary)
    }
}

/// How long a needed piece must be missing from every connected peer before
/// the swarm is judged unable to finish.
///
/// A lazy bitfield fills its gap within a few seconds of connecting; ten
/// leaves room for a slow link without making a real dead end wait long.
pub const GAP_SETTLE: std::time::Duration = std::time::Duration::from_secs(10);

/// Remembers, per torrent, since when its swarm has lacked a needed piece.
///
/// Timed per torrent rather than per peer on purpose. Holding the verdict
/// while *any* recently connected peer might still fill the gap would never
/// release in a busy swarm, where some peer is always new — so a genuinely
/// stranded download would never say so. A gap that persists for
/// [`GAP_SETTLE`] is believed regardless of who joined meanwhile.
#[derive(Debug, Default)]
pub struct GapClock {
    since: std::collections::HashMap<String, std::time::Instant>,
}

impl GapClock {
    /// Records one reading for `key` and says whether a gap is settled.
    ///
    /// Any reading with no gap resets the clock, so a gap that closes and
    /// reopens starts its wait again.
    pub fn observe(&mut self, key: &str, rarest: u32, now: std::time::Instant) -> bool {
        if rarest > 0 {
            self.since.remove(key);
            return true;
        }
        let since = *self.since.entry(key.to_owned()).or_insert(now);
        now.saturating_duration_since(since) >= GAP_SETTLE
    }

    /// Drops `key`'s clock, for a reading with nothing to judge from.
    pub fn forget(&mut self, key: &str) {
        self.since.remove(key);
    }

    /// Keeps only the clocks `keep` accepts.
    ///
    /// Called with the torrents still downloading, so a torrent that pauses
    /// mid-gap and resumes an hour later waits afresh instead of inheriting a
    /// stale start time — and a removed torrent's entry does not linger.
    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.since.retain(|key, _| keep(key));
    }
}

/// Counts how many connected peers hold each piece.
///
/// `bitfields` are the raw bytes each peer sent. Bitfields are big-endian by
/// piece — piece 0 is the high bit of byte 0 — and byte-padded, so the trailing
/// bits past `total_pieces` are spare and ignored. A peer is not obliged to
/// have zeroed them, and librqbit does not enforce that it did.
///
/// `needed` marks the pieces still wanted (see [`still_needed`]) and scopes
/// only [`Availability::rarest`]; the copies, the average and the seed count
/// describe the swarm and stay whole-torrent. A piece past the end of
/// `needed` counts as needed, so a short or empty mask errs towards the
/// stricter verdict rather than inventing availability.
///
/// Returns `None` when there is nothing to judge from: no peers, or a torrent
/// whose piece count is not known yet. A caller must not read that as a
/// healthy swarm, nor as a broken one.
pub fn analyse(bitfields: &[Vec<u8>], total_pieces: u32, needed: &[bool]) -> Option<Analysis> {
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
    let rarest = copies
        .iter()
        .enumerate()
        .filter(|(index, _)| needed.get(*index).copied().unwrap_or(true))
        .map(|(_, c)| *c)
        .min()
        .unwrap_or_else(|| u32::try_from(bitfields.len()).unwrap_or(u32::MAX));
    Some(Analysis {
        summary: Availability {
            rarest,
            average: sum as f64 / total as f64,
            seeds,
        },
        copies,
        gap_settled: true,
    })
}

/// Marks the pieces a download still needs: in a selected file, and not
/// already verified on disk.
///
/// `selected` is the piece range of each selected file, padding files
/// excluded. A piece shared between a selected file and an unselected one is
/// needed, because the selected file cannot be finished without it — the same
/// rule librqbit's own piece picker applies. `have` is our own bitfield as
/// one bool per piece; a piece past its end is treated as not had.
pub fn still_needed(
    total_pieces: u32,
    selected: impl IntoIterator<Item = std::ops::Range<u32>>,
    have: &[bool],
) -> Vec<bool> {
    let total = total_pieces as usize;
    let mut needed = vec![false; total];
    for range in selected {
        let start = (range.start as usize).min(total);
        let end = (range.end as usize).min(total);
        needed[start..end].fill(true);
    }
    for (slot, had) in needed.iter_mut().zip(have) {
        if *had {
            *slot = false;
        }
    }
    needed
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Availability, GAP_SETTLE, GapClock, analyse, still_needed};

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
        assert_eq!(analyse(&same, 4, &[]).unwrap().summary.average, 1.0);
        assert_eq!(analyse(&split, 4, &[]).unwrap().summary.average, 1.0);

        // But only one of them can finish.
        assert_eq!(analyse(&same, 4, &[]).unwrap().summary.rarest, 0);
        assert_eq!(analyse(&split, 4, &[]).unwrap().summary.rarest, 1);
    }

    #[test]
    fn counts_seeds_and_rarest() {
        let peers = vec![
            vec![0b1111_0000], // all 4
            vec![0b1111_0000], // all 4
            vec![0b1000_0000], // just piece 0
        ];
        assert_eq!(
            analyse(&peers, 4, &[]).unwrap().summary,
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
        let a = analyse(&peers, 4, &[]).unwrap().summary;
        assert_eq!(a.rarest, 1);
        assert_eq!(a.average, 1.0);
        assert_eq!(a.seeds, 1);
    }

    #[test]
    fn spans_multiple_bytes() {
        // 10 pieces: peer holds 0 and 9.
        let peers = vec![vec![0b1000_0000, 0b0100_0000]];
        let a = analyse(&peers, 10, &[]).unwrap().summary;
        assert_eq!(a.rarest, 0);
        assert_eq!(a.average, 0.2);
    }

    /// The case that read "No seeds" with a seed connected: the one peer
    /// lacks a piece, but it is a piece this download already has or never
    /// wanted, so nothing it still needs is missing from the swarm.
    #[test]
    fn rarest_counts_only_the_pieces_still_needed() {
        // 4 pieces; the only peer lacks piece 3.
        let peers = vec![vec![0b1110_0000]];

        // Whole torrent wanted: piece 3 is unobtainable.
        assert_eq!(analyse(&peers, 4, &[]).unwrap().summary.rarest, 0);

        // Piece 3 already verified, or in a file left out: nothing missing.
        let needed = [true, true, true, false];
        let a = analyse(&peers, 4, &needed).unwrap().summary;
        assert_eq!(a.rarest, 1);
        // The swarm-wide figures are unchanged by the selection.
        assert_eq!(a.average, 0.75);
        assert_eq!(a.seeds, 0);

        // And a needed piece nobody holds still says so.
        let needed = [false, false, true, true];
        assert_eq!(analyse(&peers, 4, &needed).unwrap().summary.rarest, 0);
    }

    /// Nothing left to fetch is not "cannot finish".
    #[test]
    fn nothing_still_needed_is_not_a_missing_piece() {
        let peers = vec![vec![0b0000_0000], vec![0b1000_0000]];
        let a = analyse(&peers, 4, &[false; 4]).unwrap().summary;
        assert_eq!(a.rarest, 2);
    }

    #[test]
    fn still_needed_is_selected_and_not_had() {
        // 8 pieces. Files cover 0..3 (selected), 3..6 (left out), 5..8
        // (selected) — piece 5 is shared, so it is needed.
        let have = [true, false, false, false, false, false, true, false];
        assert_eq!(
            still_needed(8, [0..3, 5..8], &have),
            [false, true, true, false, false, true, false, true]
        );
        // A range running past the end is clipped, not a panic.
        assert_eq!(still_needed(2, std::iter::once(1..9), &[]), [false, true]);
    }

    /// A lazy bitfield: the seed claims all but a few pieces, then fills them
    /// in. The gap must not be believed until it has lasted.
    #[test]
    fn a_gap_is_believed_only_once_it_has_lasted() {
        let mut clock = GapClock::default();
        let t0 = Instant::now();

        assert!(!clock.observe("a", 0, t0));
        assert!(!clock.observe("a", 0, t0 + GAP_SETTLE - Duration::from_millis(1)));
        assert!(clock.observe("a", 0, t0 + GAP_SETTLE));

        // Closing resets it; reopening waits again from the new start.
        assert!(clock.observe("a", 1, t0 + GAP_SETTLE * 2));
        let t1 = t0 + GAP_SETTLE * 3;
        assert!(!clock.observe("a", 0, t1));
        assert!(clock.observe("a", 0, t1 + GAP_SETTLE));

        // Torrents are timed independently.
        assert!(!clock.observe("b", 0, t1 + GAP_SETTLE));
    }

    #[test]
    fn forgetting_or_dropping_a_torrent_restarts_its_wait() {
        let mut clock = GapClock::default();
        let t0 = Instant::now();
        clock.observe("a", 0, t0);
        clock.observe("b", 0, t0);
        let later = t0 + GAP_SETTLE;

        clock.forget("a");
        assert!(!clock.observe("a", 0, later));

        clock.retain(|key| key != "b");
        assert!(!clock.observe("b", 0, later));
    }

    #[test]
    fn an_unsettled_gap_is_not_a_verdict() {
        let peers = vec![vec![0b1110_0000]];
        let mut a = analyse(&peers, 4, &[]).unwrap();
        assert_eq!(a.settled().map(|s| s.rarest), Some(0));

        a.gap_settled = false;
        assert_eq!(a.settled(), None);
        // The histogram still sees exactly what was claimed.
        assert_eq!(a.copies, [1, 1, 1, 0]);

        // No gap: the flag is irrelevant.
        let full = vec![vec![0b1111_0000]];
        let mut b = analyse(&full, 4, &[]).unwrap();
        b.gap_settled = false;
        assert_eq!(b.settled().map(|s| s.rarest), Some(1));
    }

    #[test]
    fn nothing_to_judge_from_is_not_a_verdict() {
        assert!(analyse(&[], 4, &[]).is_none());
        assert!(analyse(&[vec![0xff]], 0, &[]).is_none());
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
                assert!(analyse(&bitfields, pieces as u32, &[]).is_some());
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
