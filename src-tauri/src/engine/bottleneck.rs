//! What is limiting this download.
//!
//! The inspector's centrepiece: rank every constraint Flume can actually
//! measure, mark at most one as binding, and say whether changing a setting
//! would help. The design is explicit that a wrong answer here is worse than no
//! panel, so a factor whose ceiling Flume cannot measure is either left out or
//! carries no bar — it is never given a plausible-looking number.
//!
//! ## What is measured, and what is not
//!
//! The design names five factors. Three of them have no data behind them:
//!
//! | Factor            | Status                                                 |
//! | ----------------- | ------------------------------------------------------ |
//! | Peer upload       | Measured, by elimination — see below.                  |
//! | Your download cap | Measured exactly: rate against the configured cap.     |
//! | Piece availability| Measured from peer bitfields (not in the design's list).|
//! | Connection slots  | Absent. `SessionOptions.peer_limit` is `None`, so there |
//! |                   | is no ceiling to be a fraction of.                     |
//! | Disk writes       | Absent. librqbit exposes no write-queue depth.         |
//! | Hash checking     | Absent. No CPU accounting, and sampling it would be a  |
//! |                   | platform-specific guess.                               |
//!
//! **"Peer upload is binding" is a deduction, not a guess.** The rate a swarm
//! will supply is not observable, but its *complement* is: if the configured
//! cap is not saturated and no piece is missing, then nothing on this machine
//! is holding the transfer back, so the peers are. That is why it ranks last —
//! it is the residual, claimed only once the measurable constraints are ruled
//! out.

use serde::{Deserialize, Serialize};

use super::availability::Availability;
use super::note::rate;
use super::torrent::TorrentState;

/// The fraction of a configured cap at which the cap counts as binding.
///
/// Not 100%: a token-bucket limiter sits a little under its nominal rate, and a
/// cap that is 96% saturated is the thing holding the transfer back in every
/// sense the user cares about.
const CAP_SATURATED: f64 = 90.0;

/// One candidate constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitFactor {
    /// Display name, in the user's terms rather than the protocol's.
    pub name: String,
    /// How constrained this torrent is by this factor, 0–100.
    ///
    /// `None` where Flume cannot measure the ceiling — the row then renders
    /// without a bar rather than with an invented one. A full bar always means
    /// "this is at its limit", consistently across factors.
    pub utilisation: Option<f64>,
    /// Preformatted for display: `"6.6 MB/s"`, `"rarest piece on 4 peers"`.
    pub value: String,
    /// Whether this is *the* constraint. At most one factor is ever `true`.
    pub binding: bool,
}

/// The panel's contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bottleneck {
    /// Every measurable factor, most-constrained first.
    pub factors: Vec<LimitFactor>,
    /// Two sentences: what is binding, and whether a setting would help.
    pub explanation: String,
}

/// Ranks the constraints on a torrent.
///
/// Returns `None` when the question does not apply — a paused, checking,
/// errored or seeding torrent is not being limited, it is not trying.
///
/// The order is deliberate. A missing piece outranks everything because it is
/// terminal rather than slow; a saturated cap outranks the swarm because it is
/// the one the user can actually do something about; the swarm is claimed last,
/// by elimination.
pub(super) fn compute(
    state: TorrentState,
    download_bps: u64,
    limit_bps: Option<u32>,
    availability: Option<Availability>,
    live_peers: u32,
) -> Option<Bottleneck> {
    if state != TorrentState::Downloading {
        return None;
    }

    if live_peers == 0 {
        return Some(Bottleneck {
            factors: vec![LimitFactor {
                name: "Peers".to_string(),
                utilisation: Some(100.0),
                value: "none connected".to_string(),
                binding: true,
            }],
            explanation: "Nothing is connected, so nothing is arriving. This \
                 is the swarm rather than a setting — Flume will keep trying, \
                 and a torrent with no reachable peers may simply have none."
                .to_string(),
        });
    }

    let mut factors = Vec::new();

    // Availability first: a piece nobody holds is terminal, not slow.
    let starved = availability.is_some_and(|a| a.rarest == 0);
    if let Some(a) = availability {
        let threshold = f64::from(3.min(live_peers));
        #[allow(clippy::cast_precision_loss)]
        let covered = (f64::from(a.rarest) / threshold * 100.0).min(100.0);
        factors.push(LimitFactor {
            name: "Piece availability".to_string(),
            utilisation: Some(100.0 - covered),
            // Kept short: this sits in a fixed-width column beside a rate,
            // and a value that wraps makes the row ragged. The full phrasing
            // lives in the explanation below the bars.
            value: if a.rarest == 0 {
                "a missing piece".to_string()
            } else {
                format!(
                    "rarest on {} {}",
                    a.rarest,
                    if a.rarest == 1 { "peer" } else { "peers" }
                )
            },
            binding: starved,
        });
    }

    // The cap, if one is set. An unset cap cannot bind.
    let cap_bound = !starved
        && limit_bps.is_some_and(|cap| {
            #[allow(clippy::cast_precision_loss)]
            let used = download_bps as f64 / f64::from(cap) * 100.0;
            used >= CAP_SATURATED
        });
    if let Some(cap) = limit_bps {
        #[allow(clippy::cast_precision_loss)]
        let used = (download_bps as f64 / f64::from(cap) * 100.0).min(100.0);
        factors.push(LimitFactor {
            name: "Your download cap".to_string(),
            utilisation: Some(used),
            value: rate(u64::from(cap)),
            binding: cap_bound,
        });
    }

    // The swarm, by elimination.
    let swarm_bound = !starved && !cap_bound;
    factors.push(LimitFactor {
        name: "Peer upload".to_string(),
        // Only knowable when it is the residual: at that point the swarm is
        // giving everything it will. Otherwise its headroom is unmeasurable.
        utilisation: swarm_bound.then_some(100.0),
        value: rate(download_bps),
        binding: swarm_bound,
    });

    factors.sort_by(|a, b| {
        b.binding.cmp(&a.binding).then_with(|| {
            b.utilisation
                .unwrap_or(-1.0)
                .partial_cmp(&a.utilisation.unwrap_or(-1.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    Some(Bottleneck {
        explanation: explain(starved, cap_bound, limit_bps, download_bps, live_peers),
        factors,
    })
}

/// The two sentences under the bars.
///
/// The second one always answers the same question — would changing a setting
/// help — because that is the only reason a user opens this panel.
fn explain(
    starved: bool,
    cap_bound: bool,
    limit_bps: Option<u32>,
    download_bps: u64,
    live_peers: u32,
) -> String {
    if starved {
        return format!(
            "No piece is held by any of the {live_peers} connected \
             {}, so this cannot finish as it stands. No setting will change \
             that — it needs a peer holding the missing pieces to appear.",
            if live_peers == 1 { "peer" } else { "peers" }
        );
    }

    if cap_bound {
        let cap = limit_bps.map_or_else(|| "the cap".to_string(), |c| rate(u64::from(c)));
        return format!(
            "Your download cap of {cap} is the limit — the swarm is offering \
             at least this much. Raising it in Settings will make this faster, \
             up to whatever the peers can supply."
        );
    }

    let headroom = match limit_bps {
        Some(cap) => format!(" Your cap of {} is not being reached", rate(u64::from(cap))),
        None => " You have no download cap set".to_string(),
    };
    format!(
        "The {live_peers} connected {} are supplying {}, and that is all they \
         are offering.{headroom}, so no setting will make this faster — only \
         more or better-connected peers will.",
        if live_peers == 1 { "peer" } else { "peers" },
        rate(download_bps)
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{Bottleneck, compute};
    use crate::engine::availability::Availability;
    use crate::engine::torrent::TorrentState;

    fn avail(rarest: u32) -> Option<Availability> {
        Some(Availability {
            rarest,
            average: 4.0,
            seeds: 1,
        })
    }

    fn binding(b: &Bottleneck) -> Vec<&str> {
        b.factors
            .iter()
            .filter(|f| f.binding)
            .map(|f| f.name.as_str())
            .collect()
    }

    /// The panel's central promise: never more than one answer.
    #[test]
    fn at_most_one_factor_binds() {
        let cases = [
            (0, None, avail(0), 10),
            (8_000_000, Some(8_000_000), avail(5), 10),
            (1_000_000, Some(8_000_000), avail(5), 10),
            (1_000_000, None, avail(2), 10),
            (500_000, None, None, 3),
        ];
        for (rate, cap, a, peers) in cases {
            let b = compute(TorrentState::Downloading, rate, cap, a, peers).unwrap();
            assert!(
                binding(&b).len() <= 1,
                "more than one binding factor for rate={rate} cap={cap:?}"
            );
        }
    }

    /// A piece nobody holds outranks a saturated cap: one is terminal, the
    /// other is merely slow.
    #[test]
    fn a_missing_piece_outranks_a_saturated_cap() {
        let b = compute(
            TorrentState::Downloading,
            8_000_000,
            Some(8_000_000),
            avail(0),
            10,
        )
        .unwrap();
        assert_eq!(binding(&b), ["Piece availability"]);
        assert!(b.explanation.contains("cannot finish"));
        assert!(b.explanation.contains("No setting will change"));
    }

    #[test]
    fn a_saturated_cap_binds_and_says_raising_it_helps() {
        let b = compute(
            TorrentState::Downloading,
            7_600_000,
            Some(8_000_000),
            avail(5),
            10,
        )
        .unwrap();
        assert_eq!(binding(&b), ["Your download cap"]);
        assert!(b.explanation.contains("Raising it in Settings"));
    }

    /// The deduction: nothing local is limiting, so the swarm is.
    #[test]
    fn an_unsaturated_cap_leaves_the_swarm_binding() {
        let b = compute(
            TorrentState::Downloading,
            1_000_000,
            Some(8_000_000),
            avail(5),
            10,
        )
        .unwrap();
        assert_eq!(binding(&b), ["Peer upload"]);
        assert!(b.explanation.contains("no setting will make this faster"));
    }

    /// With no cap set there is nothing local to blame, so the swarm binds and
    /// no cap row is offered at all.
    #[test]
    fn no_cap_means_no_cap_row() {
        let b = compute(TorrentState::Downloading, 1_000_000, None, avail(5), 10).unwrap();
        assert_eq!(binding(&b), ["Peer upload"]);
        assert!(!b.factors.iter().any(|f| f.name == "Your download cap"));
        assert!(b.explanation.contains("no download cap set"));
    }

    /// Availability is omitted rather than guessed when there is nothing to
    /// judge it from.
    #[test]
    fn unknown_availability_is_left_out_entirely() {
        let b = compute(TorrentState::Downloading, 1_000_000, None, None, 4).unwrap();
        assert!(!b.factors.iter().any(|f| f.name == "Piece availability"));
        assert_eq!(binding(&b), ["Peer upload"]);
    }

    #[test]
    fn no_peers_is_its_own_answer() {
        let b = compute(TorrentState::Downloading, 0, None, None, 0).unwrap();
        assert_eq!(binding(&b), ["Peers"]);
    }

    /// A torrent that is not trying is not being limited.
    #[test]
    fn nothing_is_limiting_a_torrent_that_is_not_downloading() {
        for state in [
            TorrentState::Paused,
            TorrentState::Seeding,
            TorrentState::Checking,
            TorrentState::Error,
        ] {
            assert!(compute(state, 0, None, avail(5), 10).is_none());
        }
    }

    /// A full bar means "at its limit" for every factor, so the binding one is
    /// never drawn shorter than a factor with headroom.
    #[test]
    fn the_binding_factor_sorts_first() {
        let b = compute(
            TorrentState::Downloading,
            1_000_000,
            Some(8_000_000),
            avail(5),
            10,
        )
        .unwrap();
        assert!(b.factors[0].binding);
        assert_eq!(b.factors[0].utilisation, Some(100.0));
    }

    /// Unmeasurable headroom is `None`, never a plausible-looking number.
    #[test]
    fn an_unmeasurable_ceiling_carries_no_bar() {
        let b = compute(
            TorrentState::Downloading,
            7_600_000,
            Some(8_000_000),
            avail(5),
            10,
        )
        .unwrap();
        let swarm = b
            .factors
            .iter()
            .find(|f| f.name == "Peer upload")
            .expect("swarm row");
        assert!(!swarm.binding);
        assert_eq!(swarm.utilisation, None);
    }
}
