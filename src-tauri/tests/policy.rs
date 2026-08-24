//! Tests for the policy engine.
//!
//! The module exists so rules can be tested without a session, a network, or a
//! clock. These construct snapshots directly and assert on the actions, which
//! is what makes the timing and interaction cases — the ones that matter —
//! reproducible at all.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::HashMap;

use flume_lib::{
    engine::{
        CoreStatus, DhtStatus, EngineHealth, SwarmHealth, TelemetrySnapshot, TorrentState,
        TorrentSummary,
    },
    policy::{Action, PauseReason, PolicyState, Rules, TorrentRules, evaluate},
};

fn core() -> CoreStatus {
    CoreStatus {
        client_version: "Flume test".into(),
        listen_port: Some(1),
        announce_port: Some(1),
        dht: DhtStatus::disabled(),
        download_dir: "/tmp".into(),
        uptime_seconds: 0,
        download_bps: 0,
        upload_bps: 0,
        live_peers: 0,
        health: EngineHealth::Ready,
    }
}

/// A finished torrent, seeding, with the given uploaded/downloaded bytes.
fn seeding(hash: &str, id: usize, uploaded: u64, downloaded: u64) -> TorrentSummary {
    TorrentSummary {
        id,
        info_hash: hash.into(),
        name: format!("torrent-{id}"),
        state: TorrentState::Seeding,
        progress_bytes: downloaded,
        total_bytes: downloaded,
        uploaded_bytes: uploaded,
        download_bps: 0,
        upload_bps: 1024,
        live_peers: 2,
        known_peers: 2,
        // Neither is exercised here: policy reads transfer counters and state,
        // never the swarm verdict or the rendered detail line.
        health: SwarmHealth::Unknown,
        detail: String::new(),
        eta_seconds: None,
        finished: true,
        added_at: None,
        error: None,
        output_folder: "/tmp".into(),
    }
}

fn snapshot(torrents: Vec<TorrentSummary>) -> TelemetrySnapshot {
    TelemetrySnapshot {
        core: core(),
        torrents,
    }
}

fn ratio_limit(limit: f64) -> Rules {
    Rules {
        global: TorrentRules {
            seed_ratio_limit: Some(limit),
            ..Default::default()
        },
        overrides: HashMap::new(),
    }
}

// --- The foundation contract -----------------------------------------------

#[test]
fn an_empty_ruleset_produces_no_actions() {
    let snap = snapshot(vec![seeding("a", 1, 5_000, 1_000)]);
    let out = evaluate(&snap, &Rules::default(), &PolicyState::default(), 1);
    assert!(
        out.actions.is_empty(),
        "no rules configured must mean no actions, got {:?}",
        out.actions
    );
}

#[test]
fn evaluate_does_not_mutate_what_it_is_given() {
    let snap = snapshot(vec![seeding("a", 1, 5_000, 1_000)]);
    let before = PolicyState::default();
    let _ = evaluate(&snap, &ratio_limit(2.0), &before, 60);
    assert_eq!(
        before.seed_seconds("a"),
        0,
        "the caller's state must be untouched; updates come back in Outcome"
    );
}

#[test]
fn seeding_time_accrues_only_while_seeding() {
    let mut paused = seeding("a", 1, 0, 1_000);
    paused.state = TorrentState::Paused;

    let out = evaluate(
        &snapshot(vec![paused]),
        &Rules::default(),
        &PolicyState::default(),
        3_600,
    );
    assert_eq!(
        out.state.seed_seconds("a"),
        0,
        "a paused torrent has not been seeding"
    );

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 0, 1_000)]),
        &Rules::default(),
        &PolicyState::default(),
        3_600,
    );
    assert_eq!(out.state.seed_seconds("a"), 3_600);
}

#[test]
fn bookkeeping_for_removed_torrents_is_dropped() {
    // Otherwise state grows for ever, and a removed-then-re-added torrent
    // inherits stale seeding time.
    let mut state = PolicyState::default();
    state.add_seed_time("gone", 5_000);
    state.mark_paused("gone", PauseReason::RatioReached);

    let out = evaluate(&snapshot(vec![]), &Rules::default(), &state, 1);

    assert_eq!(out.state.seed_seconds("gone"), 0);
    assert_eq!(out.state.paused_reason("gone"), None);
}

// --- Precedence: the interactions that would otherwise be accidental -------

#[test]
fn a_torrent_the_user_paused_is_never_resumed_by_policy() {
    // The rule that matters most. Without it, any start rule would silently
    // undo a deliberate pause.
    let mut manually_paused = seeding("a", 1, 100, 1_000);
    manually_paused.state = TorrentState::Paused;

    // Policy has no record of pausing it, because the user did.
    let out = evaluate(
        &snapshot(vec![manually_paused]),
        &ratio_limit(10.0),
        &PolicyState::default(),
        1,
    );

    assert!(
        out.actions.is_empty(),
        "policy must not touch a user-paused torrent, got {:?}",
        out.actions
    );
}

#[test]
fn policy_resumes_only_what_policy_stopped() {
    let mut torrent = seeding("a", 1, 100, 1_000); // ratio 0.1
    torrent.state = TorrentState::Paused;

    let mut state = PolicyState::default();
    state.mark_paused("a", PauseReason::RatioReached);

    // The limit is now far above the ratio, so it should be released.
    let out = evaluate(&snapshot(vec![torrent]), &ratio_limit(5.0), &state, 1);

    assert_eq!(out.actions, vec![Action::Resume { id: 1 }]);
    assert_eq!(
        out.state.paused_reason("a"),
        None,
        "resuming must clear the record, or it would resume every tick"
    );
}

#[test]
fn a_stop_is_not_repeated_every_tick() {
    let mut torrent = seeding("a", 1, 5_000, 1_000); // ratio 5.0
    torrent.state = TorrentState::Paused;

    let mut state = PolicyState::default();
    state.mark_paused("a", PauseReason::RatioReached);

    let out = evaluate(&snapshot(vec![torrent]), &ratio_limit(2.0), &state, 1);

    assert!(
        out.actions.is_empty(),
        "already stopped for this reason; re-pausing every second would flood the log"
    );
}

#[test]
fn a_running_torrent_clears_a_stale_pause_record() {
    // If something else resumed it, policy should stop claiming it paused it.
    let mut state = PolicyState::default();
    state.mark_paused("a", PauseReason::RatioReached);

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 100, 1_000)]),
        &Rules::default(),
        &state,
        1,
    );

    assert_eq!(out.state.paused_reason("a"), None);
}

// --- Per-torrent overrides -------------------------------------------------

#[test]
fn an_override_replaces_the_global_rules_wholesale() {
    // Not a field-by-field merge: "unlimited for this one torrent" has to be
    // expressible, and merging makes it impossible.
    let mut overrides = HashMap::new();
    overrides.insert("a".to_string(), TorrentRules::default());

    let rules = Rules {
        global: TorrentRules {
            seed_ratio_limit: Some(1.0),
            ..Default::default()
        },
        overrides,
    };

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 9_000, 1_000)]), // ratio 9.0
        &rules,
        &PolicyState::default(),
        1,
    );

    assert!(
        out.actions.is_empty(),
        "an empty override means no limit, even with a global one set"
    );
}

#[test]
fn torrents_without_an_override_use_the_global_rules() {
    let mut overrides = HashMap::new();
    overrides.insert("a".to_string(), TorrentRules::default());

    let rules = Rules {
        global: TorrentRules {
            seed_ratio_limit: Some(1.0),
            ..Default::default()
        },
        overrides,
    };

    let out = evaluate(
        &snapshot(vec![
            seeding("a", 1, 9_000, 1_000), // overridden: no limit
            seeding("b", 2, 9_000, 1_000), // global limit applies
        ]),
        &rules,
        &PolicyState::default(),
        1,
    );

    assert_eq!(
        out.actions,
        vec![Action::Pause {
            id: 2,
            reason: PauseReason::RatioReached
        }]
    );
}

// --- Edge cases in the ratio itself ----------------------------------------

#[test]
fn an_unfinished_torrent_is_not_subject_to_seed_limits() {
    // A torrent still downloading has not finished, whatever its ratio.
    let mut downloading = seeding("a", 1, 9_000, 1_000);
    downloading.finished = false;
    downloading.state = TorrentState::Downloading;

    let out = evaluate(
        &snapshot(vec![downloading]),
        &ratio_limit(1.0),
        &PolicyState::default(),
        1,
    );
    assert!(out.actions.is_empty());
}

#[test]
fn a_torrent_with_nothing_downloaded_has_no_ratio() {
    // Division by zero would otherwise produce infinity and stop it instantly.
    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 500, 0)]),
        &ratio_limit(1.0),
        &PolicyState::default(),
        1,
    );
    assert!(out.actions.is_empty());
}
