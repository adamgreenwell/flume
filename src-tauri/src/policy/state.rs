//! Bookkeeping that policy rules need to carry between evaluations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Why policy stopped a torrent.
///
/// Recorded so the UI can explain a stopped torrent rather than showing a bare
/// "paused" that looks indistinguishable from a failure or a user action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PauseReason {
    /// Reached its seed ratio limit.
    RatioReached,
    /// Reached its seed time limit.
    SeedTimeReached,
    /// Waiting for a slot under the active-torrent limits.
    Queued,
}

/// State carried between policy evaluations.
///
/// Keyed by **info hash**, never session id. Session ids are only stable
/// within a run, so bookkeeping keyed on them would attach to the wrong
/// torrent after a restart — or after the re-add primitive, which changes a
/// torrent's id deliberately.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyState {
    /// Torrents policy stopped, and why.
    paused: HashMap<String, PauseReason>,
    /// Seconds each torrent has spent actually seeding, accumulated.
    seed_seconds: HashMap<String, u64>,
}

impl PolicyState {
    /// Records that policy stopped a torrent for `reason`.
    pub fn mark_paused(&mut self, info_hash: &str, reason: PauseReason) {
        self.paused.insert(info_hash.to_owned(), reason);
    }

    /// Forgets that policy stopped a torrent.
    pub fn clear_paused(&mut self, info_hash: &str) {
        self.paused.remove(info_hash);
    }

    /// Why policy stopped this torrent, if it did.
    pub fn paused_reason(&self, info_hash: &str) -> Option<PauseReason> {
        self.paused.get(info_hash).copied()
    }

    /// Adds to a torrent's accumulated seeding time.
    pub fn add_seed_time(&mut self, info_hash: &str, seconds: u64) {
        *self.seed_seconds.entry(info_hash.to_owned()).or_insert(0) += seconds;
    }

    /// How long this torrent has spent seeding, in seconds.
    pub fn seed_seconds(&self, info_hash: &str) -> u64 {
        self.seed_seconds.get(info_hash).copied().unwrap_or(0)
    }

    /// Clears a torrent's accumulated seeding time.
    ///
    /// Used when a user chooses to keep seeding past a limit; without this the
    /// limit would fire again on the next tick.
    pub fn reset_seed_time(&mut self, info_hash: &str) {
        self.seed_seconds.remove(info_hash);
    }

    /// Drops bookkeeping for torrents that no longer exist.
    ///
    /// Without this, state grows for the lifetime of the installation, and a
    /// removed-then-re-added torrent would inherit stale seeding time.
    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.paused.retain(|hash, _| keep(hash));
        self.seed_seconds.retain(|hash, _| keep(hash));
    }
}
