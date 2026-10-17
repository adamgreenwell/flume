//! Decides what should happen to torrents, without doing it.
//!
//! # Why this exists
//!
//! Several things mature clients do — seed ratio limits, queue management,
//! scheduled speed limits — are the same shape: look at current state, apply a
//! rule, act. Flume already receives a telemetry snapshot every second and can
//! pause and resume torrents. What was missing is the layer that turns that
//! into policy.
//!
//! # The shape
//!
//! [`evaluate`] is a pure function: state in, decisions out. It performs no
//! I/O, touches no engine, and does not mutate what it is given. The telemetry
//! loop applies whatever it returns.
//!
//! That separation is the point. Rules become testable by constructing a
//! snapshot and asserting the actions, with no session, no network, and no
//! clock — which matters because the interesting cases are all about timing
//! and interaction, and those are miserable to reproduce against a live
//! engine.
//!
//! # Interaction between rules
//!
//! Rules conflict. A queue limit wants to start a torrent that a ratio limit
//! wants stopped. Two features written independently would resolve that by
//! whichever ran last, which is a bug that appears months later and is nearly
//! impossible to reason about.
//!
//! So precedence is defined here, once, in [`evaluate`]:
//!
//! 1. **Manual intent outranks every rule, in both directions.** A torrent
//!    the user paused is never resumed by policy — without this, a queue slot
//!    opening would silently undo a deliberate pause. And a torrent the user
//!    started out of the queue is never re-parked by it: resuming something
//!    the queue had stopped marks it *forced*, and the queue then leaves it
//!    alone until the user pauses it again.
//!
//!    Only half of that was written at first, and the missing half was a
//!    defect rather than an omission: a manual resume was undone within one
//!    tick, because the torrent came back eligible, sorted past the limit, and
//!    was parked again before the user's finger left the mouse.
//! 2. **Stop rules outrank start rules.** If any rule says a torrent should
//!    stop, it stops, regardless of what a start rule wants.
//! 3. Within stop rules, the first matching reason is reported, so the UI can
//!    explain *why* rather than just showing "paused".

mod rules;
mod state;

use std::collections::HashSet;

use crate::engine::{PauseReason, TelemetrySnapshot, TorrentState, TorrentSummary};

pub use rules::{QueueLimits, QueueLine, Rules, TorrentRules};
pub use state::PolicyState;

/// Something the telemetry loop should do.
///
/// Deliberately small and imperative. Anything richer would tempt the policy
/// module into knowing how actions are carried out, which is exactly the
/// coupling that keeps rules untestable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Stop a torrent, recording why so the UI can explain it.
    Pause {
        /// Session id of the torrent.
        id: usize,
        /// Why policy stopped it.
        reason: PauseReason,
    },
    /// Start a torrent that policy previously stopped.
    Resume {
        /// Session id of the torrent.
        id: usize,
    },
}

/// What [`evaluate`] produced: actions to apply, and the bookkeeping to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Actions the caller should apply, in order.
    pub actions: Vec<Action>,
    /// State to carry into the next evaluation.
    ///
    /// Returned rather than mutated so `evaluate` stays pure and a test can
    /// assert on the transition rather than on a side effect.
    pub state: PolicyState,
}

/// Decides what should change, given the current snapshot and rules.
///
/// `elapsed_secs` is the time since the previous evaluation, used to
/// accumulate seeding time. The caller supplies it rather than the function
/// reading a clock, so tests can advance time by whatever they like.
///
/// # Precedence
///
/// See the module documentation. In short: a manual pause always wins, and a
/// stop always beats a start.
pub fn evaluate(
    snapshot: &TelemetrySnapshot,
    rules: &Rules,
    previous: &PolicyState,
    elapsed_secs: u64,
) -> Outcome {
    let mut state = previous.clone();
    let mut actions = Vec::new();

    // Torrents that vanished should not leave bookkeeping behind for ever.
    let present: HashSet<&str> = snapshot
        .torrents
        .iter()
        .map(|t| t.info_hash.as_str())
        .collect();
    state.retain(|info_hash| present.contains(info_hash));

    for torrent in &snapshot.torrents {
        // Seeding time accrues only while actually seeding. A torrent paused
        // for a week has not been seeding for a week, and a limit expressed in
        // hours would otherwise fire the moment it resumed.
        if torrent.state == TorrentState::Seeding {
            state.add_seed_time(&torrent.info_hash, elapsed_secs);
        }

        let effective = rules.for_torrent(&torrent.info_hash);

        match decide(torrent, &effective, &state) {
            Decision::Stop(reason) => {
                // Already stopped for this reason; nothing to do.
                if state.paused_reason(&torrent.info_hash) == Some(reason) {
                    continue;
                }
                if matches!(torrent.state, TorrentState::Paused | TorrentState::Queued) {
                    // The user stopped it, it is waiting on a slot, or it is
                    // stopped for another reason. Record the reason without
                    // acting, so the UI can explain it, but do not issue a
                    // redundant pause.
                    state.mark_paused(&torrent.info_hash, reason);
                    continue;
                }
                state.mark_paused(&torrent.info_hash, reason);
                actions.push(Action::Pause {
                    id: torrent.id,
                    reason,
                });
            }
            Decision::Start => {
                // Rule 1: a torrent the user paused is never resumed by
                // policy. Only torrents policy itself stopped are eligible.
                if state.paused_reason(&torrent.info_hash).is_some()
                    && torrent.state == TorrentState::Paused
                {
                    state.clear_paused(&torrent.info_hash);
                    actions.push(Action::Resume { id: torrent.id });
                }
            }
            Decision::Leave => {
                // A torrent running normally is no longer policy-paused.
                //
                // `Queued` is excluded deliberately: it *is* policy-paused, and
                // clearing it here would drop the reason every tick, so the
                // queue would resume the torrent it had just parked.
                if !matches!(torrent.state, TorrentState::Paused | TorrentState::Queued) {
                    state.clear_paused(&torrent.info_hash);
                }
            }
        }
    }

    // The queue runs last, over what the stop rules left alone. That ordering
    // *is* rule 2: a torrent stopped at its ratio has already been marked and
    // is skipped here, so the queue cannot start it. Nothing enforces that
    // beyond the order of these two passes, which is why they are one function.
    apply_queue(snapshot, rules, &mut state, &mut actions);

    Outcome { actions, state }
}

/// Writes each torrent's place in the line onto the snapshot.
///
/// The counterpart to [`queue_order`], and the reason the two live together:
/// one computes the order, the other reports it, and a UI that offers to
/// reorder the queue needs the second.
///
/// Every torrent in `order` gets a place, counting from one. Everything else
/// is left `None`, which is what "not in the queue" looks like.
///
/// Reporting a *rank* rather than the stored sort key is the whole point. The
/// stored key is absent until somebody reorders, so sending it meant every
/// queued torrent reported no place -- and a UI that gated its move actions
/// on having one could never make the first reorder.
pub fn stamp_queue_places(snapshot: &mut TelemetrySnapshot, order: &[String]) {
    for (rank, hash) in order.iter().enumerate() {
        if let Some(torrent) = snapshot.torrents.iter_mut().find(|t| &t.info_hash == hash) {
            // From one: the queue counts slots from zero, a user counts
            // places in a line from one.
            torrent.queue_position = u32::try_from(rank + 1).ok();
        }
    }
}

/// The queue in the order it would be admitted from.
///
/// Extracted so the order has one definition. The command that reorders the
/// queue has to agree with the evaluation that admits from it, and two copies
/// of a sort are two copies that can disagree -- which here would mean a user
/// moving a torrent up and a different one starting.
///
/// Eligible means running, or waiting because this queue stopped it. A
/// torrent stopped for any other reason is neither admitted nor counted, and
/// a forced one is out of the queue's hands whatever it is currently doing.
///
/// The user's order where they have given one, arrival order where they have
/// not. `queue_position` is absent until somebody reorders, and the first
/// reorder writes a place for the whole set -- so in practice this is either
/// all arrival order or all explicit, and the mixed case only exists for a
/// torrent that joined since. Those sort last, which is where a new arrival
/// belongs anyway.
///
/// `added_at` is absent for torrents predating the library record, and those
/// sort last rather than first: an unknown arrival time is not evidence of
/// being early.
#[must_use]
pub fn queue_order<'a>(
    snapshot: &'a TelemetrySnapshot,
    state: &PolicyState,
) -> Vec<&'a TorrentSummary> {
    let queued_by_us =
        |state: &PolicyState, hash: &str| state.paused_reason(hash) == Some(PauseReason::Queued);

    let mut eligible: Vec<&TorrentSummary> = snapshot
        .torrents
        .iter()
        .filter(|t| !state.is_forced(&t.info_hash))
        .filter(|t| match t.state {
            TorrentState::Downloading | TorrentState::Seeding => state
                .paused_reason(&t.info_hash)
                .is_none_or(|r| r == PauseReason::Queued),
            // Ours by definition -- the state exists only because this queue
            // put the torrent there.
            TorrentState::Queued => true,
            // Still reachable for one tick after the queue decides, before
            // `summarize` reports `Queued`.
            TorrentState::Paused => queued_by_us(state, &t.info_hash),
            TorrentState::Checking | TorrentState::Error => false,
        })
        .collect();

    eligible.sort_by(|a, b| {
        // The *stored* place, from the policy bookkeeping -- not
        // `TorrentSummary::queue_position`, which is the rank this function's
        // own result is used to stamp. Reading that would be circular, and
        // would have the sort depend on the answer it is computing.
        state
            .queue_position(&a.info_hash)
            .unwrap_or(u32::MAX)
            .cmp(&state.queue_position(&b.info_hash).unwrap_or(u32::MAX))
            .then_with(|| {
                a.added_at
                    .unwrap_or(u64::MAX)
                    .cmp(&b.added_at.unwrap_or(u64::MAX))
            })
            .then_with(|| a.info_hash.cmp(&b.info_hash))
    });
    eligible
}

/// The queue as the user sees it, split into the lines it actually runs.
///
/// Narrower than [`queue_order`], and split, and both differences matter.
/// `queue_order` is *admission* order over every eligible torrent, because
/// admission has to count a seed against the total limit whether or not a
/// seed limit exists. A displayed place and a move action claim something
/// else: that this torrent is waiting its turn among *these* others, and that
/// moving it changes which of them goes next.
///
/// So the torrents are grouped by the limit deciding between them --
/// see [`QueueLimits::line_for`]. Two kinds counted against separate limits
/// are two lines, ranked independently, because a download overtaking a seed
/// changes neither one's priority against anything it is competing with. A
/// kind no limit governs is in no line at all. No limits at all is the
/// degenerate case of that and needs no check of its own: every line comes
/// back empty, so nothing is stamped and nothing can be moved.
///
/// Grouped after the sort, so each line keeps its admission order. At most
/// one shape is ever non-empty -- a total limit puts everything in
/// [`QueueLine::Shared`] -- but that falls out of `line_for` rather than
/// being relied on here.
#[must_use]
pub fn queue_lines<'a>(
    snapshot: &'a TelemetrySnapshot,
    state: &PolicyState,
    limits: &QueueLimits,
) -> Vec<Vec<&'a TorrentSummary>> {
    let mut shared = Vec::new();
    let mut downloads = Vec::new();
    let mut seeds = Vec::new();

    for torrent in queue_order(snapshot, state) {
        match limits.line_for(torrent.finished) {
            Some(QueueLine::Shared) => shared.push(torrent),
            Some(QueueLine::Downloads) => downloads.push(torrent),
            Some(QueueLine::Seeds) => seeds.push(torrent),
            None => {}
        }
    }

    [shared, downloads, seeds]
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect()
}

/// Decides which torrents run and which wait, across the whole session.
///
/// The one rule that cannot be answered per torrent: a slot limit is a
/// statement about the set. This runs after the per-torrent pass so that
/// anything a stop rule claimed is already spoken for.
///
/// # What is eligible
///
/// Only torrents that are running or that the queue itself stopped. In
/// particular:
///
/// * A torrent **the user paused** is never admitted. Rule 1, and the reason
///   the queue cannot simply count running torrents and start the difference.
/// * A torrent **the user forced** is skipped entirely — neither admitted nor
///   counted. It runs *on top of* the limits rather than displacing something
///   already running, which is the reading that cannot surprise anyone: the
///   alternative pauses a torrent the user never touched, in response to an
///   action on a different one.
/// * A torrent **stopped by another rule** is not admitted, and not counted
///   against the limits either — it is not using a slot.
/// * **Checking and errored** torrents are left entirely alone: they occupy no
///   slot, and pausing a torrent mid-verify to free a slot it is not using
///   would be hostile.
///
/// # Order
///
/// Arrival order, oldest first, ties broken by info hash so the result is
/// stable across ticks. Explicit reordering is the rest of #56; until then the
/// order a user would guess is the order they added things in.
fn apply_queue(
    snapshot: &TelemetrySnapshot,
    rules: &Rules,
    state: &mut PolicyState,
    actions: &mut Vec<Action>,
) {
    if rules.queue.is_empty() {
        // No limit means no queue. Worth the early return: without it, every
        // torrent would be "admitted" and any left over from a previous
        // configuration would be resumed on the same tick the user cleared the
        // limits, which is right but noisy to reason about.
        return;
    }

    let eligible = queue_order(snapshot, state);
    let queued_by_us =
        |state: &PolicyState, hash: &str| state.paused_reason(hash) == Some(PauseReason::Queued);

    let mut downloads = 0;
    let mut seeds = 0;

    for torrent in eligible {
        let is_seed = torrent.finished;
        let (used, limit) = if is_seed {
            (seeds, rules.queue.max_active_seeds)
        } else {
            (downloads, rules.queue.max_active_downloads)
        };

        let within_kind = limit.is_none_or(|max| used < max);
        let within_total = rules
            .queue
            .max_active_total
            .is_none_or(|max| downloads + seeds < max);

        if within_kind && within_total {
            if is_seed {
                seeds += 1;
            } else {
                downloads += 1;
            }
            // Only resume what this queue stopped. A torrent already running
            // needs nothing, and one the user paused is not here at all.
            if matches!(torrent.state, TorrentState::Paused | TorrentState::Queued)
                && queued_by_us(state, &torrent.info_hash)
            {
                state.clear_paused(&torrent.info_hash);
                actions.push(Action::Resume { id: torrent.id });
            }
        } else if !matches!(torrent.state, TorrentState::Paused | TorrentState::Queued) {
            state.mark_paused(&torrent.info_hash, PauseReason::Queued);
            actions.push(Action::Pause {
                id: torrent.id,
                reason: PauseReason::Queued,
            });
        }
    }
}

/// What a set of rules wants for one torrent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    /// Should be stopped, for this reason.
    Stop(PauseReason),
    /// Should be running, if policy is what stopped it.
    Start,
    /// No rule has an opinion.
    Leave,
}

/// Applies every rule to one torrent and resolves the result.
///
/// Rule 2 lives here: stop rules are checked first and return immediately, so
/// no start rule can override them.
fn decide(torrent: &TorrentSummary, rules: &TorrentRules, state: &PolicyState) -> Decision {
    if let Some(reason) = rules::should_stop(torrent, rules, state) {
        return Decision::Stop(reason);
    }
    if rules::should_start(torrent, rules, state) {
        return Decision::Start;
    }
    Decision::Leave
}
