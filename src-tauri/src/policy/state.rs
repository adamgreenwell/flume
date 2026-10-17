//! Bookkeeping that policy rules need to carry between evaluations.

use std::collections::{BTreeSet, HashMap, HashSet};

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
    /// The user's explicit queue order, as a place per info hash.
    ///
    /// Here as well as in the library record for the same reason `forced` is:
    /// this is what the next tick reads, and the record is what survives a
    /// restart. A reorder writes both.
    queue_positions: HashMap<String, u32>,
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
            queue_positions: HashMap::new(),
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

    /// Restores the explicit queue order from the library record.
    #[must_use]
    pub fn with_queue_positions(mut self, positions: HashMap<String, u32>) -> Self {
        self.queue_positions = positions;
        self
    }

    /// Places the torrents the order names, without taking a slot that a
    /// torrent outside it already holds.
    ///
    /// Returns exactly what it assigned, so the library can be written with
    /// the same numbers. That return value is the point: these places live in
    /// two stores -- here, for the next tick, and in `library.json`, to
    /// survive a restart -- and two independent numberings drift the moment
    /// either one changes. One numbering, computed here, applied to both.
    ///
    /// # Why not simply renumber from zero
    ///
    /// The order handed in is `queue_order`'s result, which holds only the
    /// torrents *eligible* to run. One that is manually paused, stopped by a
    /// limit, or forced is absent from it while still holding a place the user
    /// gave it, so renumbering the eligible ones from zero writes over that
    /// place.
    ///
    /// Keeping the absent torrent's old number is not enough either, and this
    /// is the subtle part: a newer torrent promoted to the front and then
    /// paused would share slot 0 with whichever torrent the reorder put
    /// first, and `queue_order` breaks equal places by arrival time -- which
    /// never favours the newer one. The user's explicit "this first" would
    /// lose to arrival order, permanently and silently.
    ///
    /// So the absent torrents keep their slots and the eligible ones are
    /// distributed around them. No two positioned torrents end up equal, and
    /// a torrent nobody mentioned comes back exactly where it was.
    pub fn set_queue_order(&mut self, order: &[String]) -> Vec<(String, u32)> {
        // Slots held by torrents this order says nothing about.
        let held: BTreeSet<u32> = self
            .queue_positions
            .iter()
            .filter(|(hash, _)| !order.iter().any(|named| named == *hash))
            .map(|(_, at)| *at)
            .collect();

        let mut assigned = Vec::with_capacity(order.len());
        let mut slot: u32 = 0;
        for hash in order {
            // Bounded by `slot < u32::MAX` rather than left to run: without
            // it, a held `u32::MAX` would spin here for ever. Reaching it
            // needs four billion torrents, and the degenerate answer -- the
            // remainder all land on `u32::MAX` -- is one `queue_order`
            // already breaks by arrival time.
            while slot < u32::MAX && held.contains(&slot) {
                slot += 1;
            }
            assigned.push((hash.clone(), slot));
            slot = slot.saturating_add(1);
        }

        for (hash, at) in &assigned {
            self.queue_positions.insert(hash.clone(), *at);
        }
        assigned
    }

    /// Where the user put this torrent, if they have.
    ///
    /// `None` is the overwhelmingly common answer: nobody has reordered, so
    /// the queue falls back to arrival order. It is deliberately *not* the
    /// same thing as the place a torrent currently occupies in the line --
    /// that is a rank over the eligible set, computed per tick, and it is
    /// what `TorrentSummary::queue_position` reports.
    #[must_use]
    pub fn queue_position(&self, info_hash: &str) -> Option<u32> {
        self.queue_positions.get(info_hash).copied()
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

    /// Drops every piece of bookkeeping for a torrent that was removed.
    ///
    /// Without this, state grows for the lifetime of the installation, and a
    /// removed-then-re-added info hash inherits its predecessor's seeding
    /// time, queue place and force -- an exemption the user granted to
    /// something they no longer have.
    ///
    /// Driven by a removal rather than by absence from a snapshot, which is
    /// the same distinction [`crate::library::Library::forget`] already
    /// states for the persisted record: a removal is knowledge, where an
    /// absence is not. A persisted torrent whose drive is unmounted fails to
    /// restore and is missing from every snapshot while its record is kept on
    /// purpose, so pruning on absence stripped exactly the torrent the record
    /// was protecting -- and `AppState::new` is the only thing that reloads
    /// the library into this state, so an in-process engine restart brought
    /// it back with nothing.
    pub fn forget(&mut self, info_hash: &str) {
        self.paused.remove(info_hash);
        self.seed_seconds.remove(info_hash);
        self.forced.remove(info_hash);
        self.queue_positions.remove(info_hash);
    }
}
