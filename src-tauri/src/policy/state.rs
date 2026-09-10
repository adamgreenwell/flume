//! Bookkeeping that policy rules need to carry between evaluations.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::engine::PauseReason;

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
    /// Torrents the user started out of the queue by hand.
    forced: HashSet<String>,
}

impl PolicyState {
    /// Restores accumulated seeding time from persisted per-torrent records.
    ///
    /// Only the seeding time is restored. Pause reasons deliberately are not:
    /// they describe a decision about a *running* session, and librqbit's
    /// paused bit carries no reason, so a restored reason could not be
    /// checked against anything. The first evaluation after launch re-derives
    /// them from the rules and the snapshot, which is the only source that
    /// can be trusted to still be true.
    #[must_use]
    pub fn with_seed_times(seed_seconds: HashMap<String, u64>) -> Self {
        Self {
            paused: HashMap::new(),
            seed_seconds,
            forced: HashSet::new(),
        }
    }

    /// Restores which torrents the user had forced past the queue.
    ///
    /// Unlike a pause reason, this *is* restored: it records something the
    /// user said rather than something policy decided, and the queue would
    /// otherwise re-park the torrent on the next launch -- the same surprise
    /// the force exists to prevent, one quit later.
    #[must_use]
    pub fn with_forced(mut self, forced: HashSet<String>) -> Self {
        self.forced = forced;
        self
    }

    /// Accumulated seeding time for every torrent that has any.
    ///
    /// Borrowed rather than cloned: the one caller writes it straight into the
    /// library record under a lock it already holds.
    #[must_use]
    pub fn seed_times(&self) -> &HashMap<String, u64> {
        &self.seed_seconds
    }

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

    /// Every torrent policy stopped, and why.
    ///
    /// Handed to the engine so a stopped torrent can say what stopped it. The
    /// engine never asks for this -- it is passed in, like arrival times --
    /// which is what keeps `engine` free of any dependency on `policy`.
    #[must_use]
    pub fn pause_reasons(&self) -> &HashMap<String, PauseReason> {
        &self.paused
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

    /// Records that the user started a torrent out of the queue by hand.
    pub fn mark_forced(&mut self, info_hash: &str) {
        self.forced.insert(info_hash.to_owned());
    }

    /// Returns the torrent to the queue's control.
    pub fn clear_forced(&mut self, info_hash: &str) {
        self.forced.remove(info_hash);
    }

    /// Whether the user forced this torrent past the queue.
    #[must_use]
    pub fn is_forced(&self, info_hash: &str) -> bool {
        self.forced.contains(info_hash)
    }

    /// Every torrent the user forced, for the caller that persists them.
    #[must_use]
    pub fn forced(&self) -> &HashSet<String> {
        &self.forced
    }

    /// Drops bookkeeping for torrents that no longer exist.
    ///
    /// Without this, state grows for the lifetime of the installation, and a
    /// removed-then-re-added torrent would inherit stale seeding time.
    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.paused.retain(|hash, _| keep(hash));
        self.seed_seconds.retain(|hash, _| keep(hash));
        self.forced.retain(|hash| keep(hash));
    }
}
