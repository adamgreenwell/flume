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
//! 1. **A torrent the user paused is never resumed by policy.** Manual intent
//!    outranks every rule. Without this, a queue slot opening would silently
//!    undo a deliberate pause.
//! 2. **Stop rules outrank start rules.** If any rule says a torrent should
//!    stop, it stops, regardless of what a start rule wants.
//! 3. Within stop rules, the first matching reason is reported, so the UI can
//!    explain *why* rather than just showing "paused".

mod rules;
mod state;

use std::collections::HashSet;

use crate::engine::{TelemetrySnapshot, TorrentState, TorrentSummary};

pub use rules::{Rules, TorrentRules};
pub use state::{PauseReason, PolicyState};

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
                if torrent.state == TorrentState::Paused {
                    // The user stopped it, or it is stopped for another
                    // reason. Record the reason without acting, so the UI can
                    // explain it, but do not issue a redundant pause.
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
                if torrent.state != TorrentState::Paused {
                    state.clear_paused(&torrent.info_hash);
                }
            }
        }
    }

    Outcome { actions, state }
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
