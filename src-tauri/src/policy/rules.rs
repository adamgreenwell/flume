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
    if torrent.state != TorrentState::Paused {
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
