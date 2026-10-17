//! The rules themselves, and the settings that configure them.
//!
//! Each rule is a small function over one torrent. They are kept separate from
//! [`super::evaluate`] so that adding a rule does not mean touching the
//! precedence logic — which is the part that is easy to get subtly wrong.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::PolicyState;
use crate::engine::{PauseReason, TorrentState, TorrentSummary};

/// Rules that can be set globally and overridden per torrent.
///
/// Every field is optional and means "no limit" when absent, so a default
/// `Rules` produces no actions at all.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TorrentRules {
    /// Stop seeding once uploaded ÷ downloaded reaches this.
    pub seed_ratio_limit: Option<f64>,
    /// Stop seeding after this many seconds of actual seeding time.
    pub seed_time_limit_secs: Option<u64>,
}

impl TorrentRules {
    /// Whether any rule is set.
    pub fn is_empty(&self) -> bool {
        self.seed_ratio_limit.is_none() && self.seed_time_limit_secs.is_none()
    }
}

/// One line of torrents waiting on the same limit.
///
/// Which line a torrent is in decides what a place and a move action mean:
/// both are claims about the torrents it is actually competing with. Two
/// kinds counted against separate limits are not competing, so ranking them
/// together would report a move that changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueLine {
    /// Downloads and seeds together, under `max_active_total`.
    Shared,
    /// Downloads alone, under `max_active_downloads`.
    Downloads,
    /// Seeds alone, under `max_active_seeds`.
    Seeds,
}

/// How many torrents may run at once.
///
/// Session-wide rather than per-torrent: a slot limit is a statement about
/// this machine's connection and disk, not about any one torrent. `None` on a
/// field means no limit, so a default `QueueLimits` queues nothing.
///
/// Downloads and seeds are counted separately because they cost different
/// things — a finished torrent spends upload, an unfinished one spends both —
/// and `max_active_total` caps the two together for the case where the real
/// constraint is peer connections rather than either direction.
///
/// **Checking and errored torrents occupy no slot.** Verification is disk work
/// a peer cannot see, and a torrent re-hashing 40 GB would otherwise hold a
/// download slot open for minutes doing nothing. An errored torrent cannot run
/// at all, so counting it would let a failure shrink the queue.
///
/// Mirrored in `src/lib/ipc/types.ts`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QueueLimits {
    /// Most torrents that may be downloading at once.
    pub max_active_downloads: Option<u32>,
    /// Most torrents that may be seeding at once.
    pub max_active_seeds: Option<u32>,
    /// Most torrents that may be running at all, downloads and seeds together.
    pub max_active_total: Option<u32>,
}

impl QueueLimits {
    /// Whether any limit is set.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.max_active_downloads.is_none()
            && self.max_active_seeds.is_none()
            && self.max_active_total.is_none()
    }

    /// The most torrents of this kind that may run, or `None` for no limit.
    ///
    /// Both limits apply, so the binding one is whichever is lower -- which
    /// is also what makes a cap of zero worth naming: nothing of that kind
    /// can ever be admitted.
    #[must_use]
    pub const fn cap_for(&self, is_seed: bool) -> Option<u32> {
        let own = if is_seed {
            self.max_active_seeds
        } else {
            self.max_active_downloads
        };
        match (own, self.max_active_total) {
            (None, total) => total,
            (Some(own), None) => Some(own),
            (Some(own), Some(total)) => Some(if own < total { own } else { total }),
        }
    }

    /// Whether any torrent of this kind can be admitted at all.
    ///
    /// A cap of zero is accepted: `Settings::validate` says nothing about the
    /// queue limits, and a settings file that fails validation has its
    /// choices honoured rather than clamped. So this is reachable by hand,
    /// and the honest answer for such a kind is that it has no queue -- it is
    /// not waiting its turn, it has no turn.
    #[must_use]
    pub const fn can_run(&self, is_seed: bool) -> bool {
        match self.cap_for(is_seed) {
            None => true,
            Some(cap) => cap > 0,
        }
    }

    /// Whether the total limit can ever refuse what the per-kind caps allow.
    ///
    /// `apply_queue` enforces all three, so a total above everything the
    /// per-kind caps can reach together is dead weight: one download and one
    /// seed cannot breach a total of ten, so that total decides nothing and
    /// the two kinds are not competing after all. Treating it as though they
    /// were put them in one line, where moving a download past a seed
    /// reported success and changed nothing about what ran.
    ///
    /// A kind with no cap of its own is unbounded, so the total is the only
    /// thing holding it and always binds.
    ///
    /// And both kinds have to be able to run, or there is nothing for the
    /// total to arbitrate: with downloads capped at zero, no download ever
    /// takes a total slot, so the seeds are queuing against each other alone.
    #[must_use]
    pub const fn total_can_bind(&self) -> bool {
        if !self.can_run(false) || !self.can_run(true) {
            return false;
        }
        match (
            self.max_active_total,
            self.max_active_downloads,
            self.max_active_seeds,
        ) {
            (None, _, _) => false,
            (Some(_), None, _) | (Some(_), _, None) => true,
            // Saturating because the sum is a ceiling, not a count: a pair of
            // caps near `u32::MAX` is already further than any total can
            // reach, and wrapping would say the opposite.
            (Some(total), Some(downloads), Some(seeds)) => total < downloads.saturating_add(seeds),
        }
    }

    /// Which queue line a torrent of this kind waits in, if any.
    ///
    /// Not a yes-or-no question, which is the mistake this replaces. A limit
    /// on downloads says nothing about which seed runs next, so with only
    /// `max_active_downloads` set there is no line of seeds to be in. But
    /// with *both* per-kind limits set and no total, `apply_queue` counts the
    /// two kinds against separate counters -- so they are two lines, not one,
    /// and moving a download past a seed would change both their displayed
    /// places without changing which download or seed runs next.
    ///
    /// A total limit is what makes them compete -- but only if it can
    /// actually bind. Set to more than the two per-kind caps can reach
    /// together, it never refuses anything `apply_queue` would otherwise
    /// have admitted, so the counters stay independent and the kinds are
    /// still two lines. See [`Self::total_can_bind`].
    #[must_use]
    pub const fn line_for(&self, is_seed: bool) -> Option<QueueLine> {
        // Bounded by nothing, so there is no queue for this kind to be in.
        // Asked through `cap_for` rather than of the per-kind field, because
        // a kind with no cap of its own is still held by the total.
        let Some(cap) = self.cap_for(is_seed) else {
            return None;
        };
        // Capped at zero: it has no turn, so a place and a move both claim
        // something false about it.
        if cap == 0 {
            return None;
        }
        if self.total_can_bind() {
            return Some(QueueLine::Shared);
        }
        if is_seed {
            Some(QueueLine::Seeds)
        } else {
            Some(QueueLine::Downloads)
        }
    }
}

/// Global rules, plus per-torrent overrides.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rules {
    /// Applied to any torrent without an override.
    pub global: TorrentRules,
    /// Per-torrent rules, keyed by info hash.
    ///
    /// An override replaces the global rules wholesale rather than merging
    /// field by field. Merging reads as more flexible and is much harder to
    /// reason about: "unlimited for this one torrent" becomes impossible to
    /// express if an unset field inherits the global value.
    pub overrides: HashMap<String, TorrentRules>,
    /// How many torrents may run at once. Not overridable per torrent — a slot
    /// limit is about the machine, and "unlimited for this one" is what the
    /// queue order is for.
    pub queue: QueueLimits,
}

impl Rules {
    /// The rules in force for one torrent.
    pub fn for_torrent(&self, info_hash: &str) -> TorrentRules {
        self.overrides
            .get(info_hash)
            .cloned()
            .unwrap_or_else(|| self.global.clone())
    }
}

/// Uploaded ÷ downloaded, or `None` when nothing has been downloaded.
///
/// Uses `progress_bytes` for the denominator, matching the ratio the UI
/// already shows. For a torrent added on top of existing data that counts
/// bytes never actually transferred — but being consistent with the displayed
/// figure matters more than being theoretically precise, since a limit that
/// fires at a different number than the one on screen is indefensible.
pub fn ratio(torrent: &TorrentSummary) -> Option<f64> {
    if torrent.progress_bytes == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some(torrent.uploaded_bytes as f64 / torrent.progress_bytes as f64)
}

/// Whether any stop rule applies, and the first reason that does.
pub(super) fn should_stop(
    torrent: &TorrentSummary,
    rules: &TorrentRules,
    state: &PolicyState,
) -> Option<PauseReason> {
    // Only seeding torrents are subject to seed limits. A torrent still
    // downloading has not finished, whatever its ratio happens to be.
    if !torrent.finished {
        return None;
    }

    if let Some(limit) = rules.seed_ratio_limit
        && ratio(torrent).is_some_and(|r| r >= limit)
    {
        return Some(PauseReason::RatioReached);
    }

    if let Some(limit) = rules.seed_time_limit_secs
        && state.seed_seconds(&torrent.info_hash) >= limit
    {
        return Some(PauseReason::SeedTimeReached);
    }

    None
}

/// Whether a torrent policy stopped should now be started again.
///
/// Only meaningful for torrents policy itself paused; [`super::evaluate`]
/// enforces that a user's manual pause is never overridden.
pub(super) fn should_start(
    torrent: &TorrentSummary,
    rules: &TorrentRules,
    state: &PolicyState,
) -> bool {
    // Nothing to start if policy did not stop it.
    let Some(reason) = state.paused_reason(&torrent.info_hash) else {
        return false;
    };
    // `Queued` counts as stopped here: a torrent waiting on a slot whose ratio
    // limit was just raised is still not something this function starts -- the
    // queue owns it, and `PauseReason::Queued` falls through to `false` below.
    if !matches!(torrent.state, TorrentState::Paused | TorrentState::Queued) {
        return false;
    }

    match reason {
        // A limit that has been raised or removed should release the torrent.
        PauseReason::RatioReached => rules
            .seed_ratio_limit
            .is_none_or(|limit| ratio(torrent).is_none_or(|r| r < limit)),
        PauseReason::SeedTimeReached => rules
            .seed_time_limit_secs
            .is_none_or(|limit| state.seed_seconds(&torrent.info_hash) < limit),
        // Queue slots are decided across all torrents at once, so a single
        // torrent cannot answer this. Handled when queueing lands.
        PauseReason::Queued => false,
    }
}
