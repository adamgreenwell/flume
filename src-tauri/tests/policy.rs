//! Tests for the policy engine.
//!
//! The module exists so rules can be tested without a session, a network, or a
//! clock. These construct snapshots directly and assert on the actions, which
//! is what makes the timing and interaction cases — the ones that matter —
//! reproducible at all.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use flume_lib::{
    engine::{
        CoreStatus, DhtStatus, EngineHealth, IpFilterStatus, PauseReason, SwarmHealth,
        TelemetrySnapshot, TorrentState, TorrentSummary,
    },
    library::Library,
    policy::{Action, PolicyState, QueueLimits, Rules, TorrentRules, evaluate},
    settings::Settings,
    state::AppState,
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
        ip_filter: IpFilterStatus {
            blocklist: None,
            allowlist: None,
        },
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
        pause_reason: None,
        eta_seconds: None,
        finished: true,
        added_at: None,
        error: None,
        output_folder: "/tmp".into(),
        forced: false,
        queue_position: None,
    }
}

/// A torrent waiting on a queue slot.
fn queued(hash: &str, id: usize) -> TorrentSummary {
    TorrentSummary {
        state: TorrentState::Queued,
        finished: false,
        ..seeding(hash, id, 0, 1_000)
    }
}

fn snapshot(torrents: Vec<TorrentSummary>) -> TelemetrySnapshot {
    TelemetrySnapshot {
        core: core(),
        torrents,
    }
}

/// The queue lines as info hashes, which is what the assertions compare.
fn lines_of(
    snap: &TelemetrySnapshot,
    state: &PolicyState,
    limits: &QueueLimits,
) -> Vec<Vec<String>> {
    flume_lib::policy::queue_lines(snap, state, limits)
        .into_iter()
        .map(|line| line.into_iter().map(|t| t.info_hash.clone()).collect())
        .collect()
}

/// A torrent's stamped place, or `None` for the dash.
fn place(snap: &TelemetrySnapshot, hash: &str) -> Option<u32> {
    snap.torrents
        .iter()
        .find(|t| t.info_hash == hash)
        .and_then(|t| t.queue_position)
}

fn ratio_limit(limit: f64) -> Rules {
    Rules {
        global: TorrentRules {
            seed_ratio_limit: Some(limit),
            ..Default::default()
        },
        overrides: HashMap::new(),
        queue: QueueLimits::default(),
    }
}

fn seed_time_limit(secs: u64) -> Rules {
    Rules {
        global: TorrentRules {
            seed_time_limit_secs: Some(secs),
            ..Default::default()
        },
        overrides: HashMap::new(),
        queue: QueueLimits::default(),
    }
}

fn downloading(hash: &str, id: usize, added_at: Option<u64>) -> TorrentSummary {
    TorrentSummary {
        state: TorrentState::Downloading,
        finished: false,
        progress_bytes: 500,
        total_bytes: 1_000,
        added_at,
        ..seeding(hash, id, 0, 1_000)
    }
}

fn seed(hash: &str, id: usize, added_at: Option<u64>) -> TorrentSummary {
    TorrentSummary {
        added_at,
        ..seeding(hash, id, 0, 1_000)
    }
}

fn queue(downloads: Option<u32>, seeds: Option<u32>, total: Option<u32>) -> Rules {
    Rules {
        queue: QueueLimits {
            max_active_downloads: downloads,
            max_active_seeds: seeds,
            max_active_total: total,
        },
        ..Rules::default()
    }
}

/// Ids of the torrents an outcome pauses, in order.
fn paused(out: &flume_lib::policy::Outcome) -> Vec<usize> {
    out.actions
        .iter()
        .filter_map(|a| match a {
            Action::Pause { id, .. } => Some(*id),
            Action::Resume { .. } => None,
        })
        .collect()
}

/// Ids of the torrents an outcome resumes, in order.
fn resumed(out: &flume_lib::policy::Outcome) -> Vec<usize> {
    out.actions
        .iter()
        .filter_map(|a| match a {
            Action::Resume { id } => Some(*id),
            Action::Pause { .. } => None,
        })
        .collect()
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
    // inherits stale seeding time. Driven by the removal rather than by a
    // later absence, which is what `evaluate` used to read it from.
    let mut state = PolicyState::default();
    state.add_seed_time("gone", 5_000);
    state.mark_paused("gone", PauseReason::RatioReached);
    state.add_seed_time("staying", 60);

    state.forget("gone");

    assert_eq!(state.seed_seconds("gone"), 0);
    assert_eq!(state.paused_reason("gone"), None);
    assert_eq!(state.seed_seconds("staying"), 60, "pruning one prunes one");
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
        queue: QueueLimits::default(),
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
        queue: QueueLimits::default(),
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

// --- Restored seeding time (#55) -------------------------------------------

#[test]
fn a_torrent_already_past_its_limit_on_load_stops_on_the_first_evaluation() {
    // The case #55 names: Flume quits with a torrent at 20 hours, the limit
    // is 10, and the very first tick after launch must stop it rather than
    // start counting again from zero.
    let restored = PolicyState::with_seed_times(HashMap::from([("a".to_owned(), 72_000)]));

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 0, 1_000)]),
        &seed_time_limit(36_000),
        &restored,
        1,
    );

    assert!(
        matches!(
            out.actions.as_slice(),
            [Action::Pause {
                id: 1,
                reason: PauseReason::SeedTimeReached
            }]
        ),
        "expected one seed-time pause, got {:?}",
        out.actions
    );
}

#[test]
fn restored_seeding_time_is_added_to_rather_than_replaced() {
    // A restart must not reset the count, and must not double it either.
    let restored = PolicyState::with_seed_times(HashMap::from([("a".to_owned(), 1_000)]));

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 0, 1_000)]),
        &Rules::default(),
        &restored,
        60,
    );

    assert_eq!(out.state.seed_seconds("a"), 1_060);
}

#[test]
fn a_torrent_under_its_restored_limit_is_left_running() {
    // The other half of the acceptance criterion: not reaching the limit has
    // to be as reliable as reaching it.
    let restored = PolicyState::with_seed_times(HashMap::from([("a".to_owned(), 35_999)]));

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 0, 1_000)]),
        &seed_time_limit(36_000),
        &restored,
        0,
    );

    assert!(
        out.actions.is_empty(),
        "one second short is still short, got {:?}",
        out.actions
    );
}

#[test]
fn restoring_carries_seeding_time_without_carrying_pause_reasons() {
    // Only seeding time is persisted. A restored pause reason could not be
    // checked against anything -- librqbit's paused bit carries no reason --
    // so the first evaluation re-derives it from the rules and the snapshot.
    let restored = PolicyState::with_seed_times(HashMap::from([("a".to_owned(), 500)]));

    assert_eq!(restored.seed_seconds("a"), 500);
    assert_eq!(restored.paused_reason("a"), None);
}

#[test]
fn restored_time_for_a_torrent_that_is_absent_is_kept() {
    // Restored from a library record, and kept on exactly the terms that
    // record is: a torrent missing from the session has not been removed,
    // so the time it earned is still its own. `evaluate` used to drop it on
    // the first tick, which beat the restore it had just been handed.
    let restored = PolicyState::with_seed_times(HashMap::from([
        ("a".to_owned(), 500),
        ("absent".to_owned(), 900),
    ]));

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 0, 1_000)]),
        &Rules::default(),
        &restored,
        0,
    );

    assert_eq!(
        out.state.seed_seconds("a"),
        500,
        "the restored torrent keeps its time"
    );
    assert_eq!(
        out.state.seed_seconds("absent"),
        900,
        "the absent one was restored for a reason"
    );
}

#[test]
fn what_evaluate_returns_is_what_gets_persisted() {
    // The flush writes `PolicyState::seed_times()` straight into the library
    // record, so that map has to be the same figure the rules read.
    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 0, 1_000)]),
        &Rules::default(),
        &PolicyState::default(),
        90,
    );

    assert_eq!(out.state.seed_times().get("a"), Some(&90));
    assert_eq!(out.state.seed_seconds("a"), 90);
}

// --- The wiring, end to end -------------------------------------------------
//
// The tests above prove `evaluate` handles restored time and the library
// tests prove the record round-trips. Neither notices if the two are never
// connected, which is the whole feature.

#[tokio::test]
async fn app_state_restores_seeding_time_from_the_library_on_construction() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let mut library = Library::default();
    library.record_seed_times(&HashMap::from([("a".to_owned(), 4_242)]));
    library.save(tmp.path()).expect("save");

    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);

    assert_eq!(
        state.policy_state().await.seed_seconds("a"),
        4_242,
        "a limit in hours means nothing if the count restarts at launch"
    );
}

#[tokio::test]
async fn a_flush_writes_seeding_time_back_to_the_library() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);
    state
        .set_policy_state(PolicyState::with_seed_times(HashMap::from([(
            "a".to_owned(),
            900,
        )])))
        .await;

    state.persist_seed_times().await;

    let (reloaded, problem) = Library::load(tmp.path());
    assert!(problem.is_none());
    assert_eq!(reloaded.seed_times().get("a"), Some(&900));
}

#[tokio::test]
async fn seeding_time_survives_a_full_quit_and_relaunch() {
    // The acceptance criterion, exercised through the same calls the app
    // makes: accumulate, flush on exit, construct again.
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().to_path_buf();

    let first = AppState::new(Settings::default(), dir.clone(), false);
    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 0, 1_000)]),
        &Rules::default(),
        &first.policy_state().await,
        3_600,
    );
    first.set_policy_state(out.state).await;
    first.persist_seed_times().await;

    let second = AppState::new(Settings::default(), dir, false);

    assert_eq!(second.policy_state().await.seed_seconds("a"), 3_600);
}

#[tokio::test]
async fn a_flush_with_nothing_accumulated_writes_no_file() {
    // Once a minute forever, so the quiet case has to stay quiet -- and a
    // library file that never existed must not be created by a flush.
    let tmp = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);

    state.persist_seed_times().await;

    assert!(
        !tmp.path().join("library.json").exists(),
        "an empty flush should not have written anything"
    );
}

#[test]
fn a_manual_resume_currently_defeats_the_limit_until_the_record_clears() {
    // Characterisation, not endorsement. A user who resumes a torrent policy
    // stopped gets to keep seeding past the limit: `should_stop` still says
    // stop, but the reason already on record makes `evaluate` treat it as
    // handled and issue nothing.
    //
    // `keep_seeding` is now the sanctioned way to say this, and says it
    // properly: it writes an empty override, clears the record, and leaves
    // nothing stale. Both UI paths for a limit-stopped torrent -- the expanded
    // panel and the context menu -- offer that instead of a plain Resume, so
    // reaching this state from the app takes deliberate effort.
    //
    // Still pinned, because `evaluate` must not start re-pausing behind
    // whatever did the resuming. A torrent resumed by some other route is
    // running because something asked it to, and policy silently undoing that
    // one second later is worse than the stale record.
    //
    // The row stays honest either way -- a seeding torrent describes itself as
    // seeding, per `a_pause_reason_only_speaks_for_a_paused_torrent`.
    let resumed = seeding("a", 1, 5_000, 1_000); // ratio 5.0, running again
    let mut state = PolicyState::default();
    state.mark_paused("a", PauseReason::RatioReached);

    let out = evaluate(&snapshot(vec![resumed]), &ratio_limit(2.0), &state, 1);

    assert!(
        out.actions.is_empty(),
        "today policy does not re-stop it, got {:?}",
        out.actions
    );
    assert_eq!(
        out.state.paused_reason("a"),
        Some(PauseReason::RatioReached),
        "and the record stays set, which is the part that needs deciding"
    );
}

// --- Per-torrent overrides and keep-seeding (#55) ---------------------------

#[tokio::test]
async fn rules_are_assembled_from_settings_and_the_library() {
    // The globals are settings a user types; the overrides are library records
    // pruned with their torrent. `policy` sees one `Rules` and cannot tell.
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        seed_ratio_limit: Some(2.0),
        seed_time_limit_secs: Some(3_600),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);
    state
        .set_torrent_rules("a", Some(TorrentRules::default()))
        .await;

    let rules = state.policy_rules().await;

    assert_eq!(rules.global.seed_ratio_limit, Some(2.0));
    assert_eq!(rules.global.seed_time_limit_secs, Some(3_600));
    assert_eq!(
        rules.for_torrent("a"),
        TorrentRules::default(),
        "an override replaces the globals wholesale rather than merging"
    );
    assert_eq!(
        rules.for_torrent("b").seed_ratio_limit,
        Some(2.0),
        "a torrent without an override still gets the globals"
    );
}

#[tokio::test]
async fn an_empty_override_means_seed_forever() {
    // The whole reason overrides replace rather than merge. If `{}` inherited
    // the global limit back, keep-seeding could not be expressed at all.
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        seed_ratio_limit: Some(2.0),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);
    state
        .set_torrent_rules("a", Some(TorrentRules::default()))
        .await;

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 50_000, 1_000)]), // ratio 50
        &state.policy_rules().await,
        &PolicyState::default(),
        1,
    );

    assert!(
        out.actions.is_empty(),
        "a torrent told to seed forever must not be stopped, got {:?}",
        out.actions
    );
}

#[tokio::test]
async fn an_override_survives_a_restart_and_dies_with_its_torrent() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().to_path_buf();

    let first = AppState::new(Settings::default(), dir.clone(), false);
    first
        .set_torrent_rules("a", Some(TorrentRules::default()))
        .await;

    let second = AppState::new(Settings::default(), dir, false);
    assert!(
        second.policy_rules().await.overrides.contains_key("a"),
        "an override the user set must outlive a quit"
    );

    // The reason it lives on the library record: removal prunes it, where
    // nothing prunes a map in settings.json.
    second.forget_torrent("a").await;
    assert!(
        !second.policy_rules().await.overrides.contains_key("a"),
        "removing the torrent must take its override with it"
    );
}

#[tokio::test]
async fn clearing_an_override_returns_a_torrent_to_the_globals() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        seed_ratio_limit: Some(2.0),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);
    state
        .set_torrent_rules("a", Some(TorrentRules::default()))
        .await;

    state.set_torrent_rules("a", None).await;

    assert_eq!(
        state.policy_rules().await.for_torrent("a").seed_ratio_limit,
        Some(2.0)
    );
}

#[tokio::test]
async fn keep_seeding_clears_the_stop_as_well_as_the_limit() {
    // Both halves matter. Without the cleared record the row would go on
    // claiming a limit stopped a torrent that now has no limit to stop it --
    // which is the stale-bookkeeping case slice 2 pinned.
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        seed_ratio_limit: Some(2.0),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);
    let mut stopped = PolicyState::default();
    stopped.mark_paused("a", PauseReason::RatioReached);
    state.set_policy_state(stopped).await;

    state
        .set_torrent_rules("a", Some(TorrentRules::default()))
        .await;
    state.clear_pause_reason("a").await;

    assert_eq!(state.policy_state().await.paused_reason("a"), None);
    assert!(state.pause_reasons().await.is_empty());

    let out = evaluate(
        &snapshot(vec![seeding("a", 1, 50_000, 1_000)]),
        &state.policy_rules().await,
        &state.policy_state().await,
        1,
    );
    assert!(
        out.actions.is_empty(),
        "it must not be stopped straight back again, got {:?}",
        out.actions
    );
}

// --- Queue management (#56) -------------------------------------------------

#[test]
fn no_limit_means_no_queue() {
    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(1)),
            downloading("b", 2, Some(2)),
            downloading("c", 3, Some(3)),
        ]),
        &Rules::default(),
        &PolicyState::default(),
        1,
    );

    assert!(out.actions.is_empty(), "got {:?}", out.actions);
}

#[test]
fn downloads_beyond_the_limit_are_queued_oldest_first() {
    let out = evaluate(
        &snapshot(vec![
            downloading("c", 3, Some(300)),
            downloading("a", 1, Some(100)),
            downloading("b", 2, Some(200)),
        ]),
        &queue(Some(2), None, None),
        &PolicyState::default(),
        1,
    );

    // Snapshot order is deliberately shuffled: the queue must sort by arrival,
    // not by however the engine happened to hand them over.
    assert_eq!(
        paused(&out),
        vec![3],
        "the newest should be the one to wait"
    );
    assert_eq!(
        out.state.paused_reason("c"),
        Some(PauseReason::Queued),
        "and it should say why"
    );
}

#[test]
fn seeds_and_downloads_have_separate_limits() {
    // Two of each, one slot each. Neither kind should eat the other's slot.
    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(100)),
            downloading("b", 2, Some(200)),
            seed("c", 3, Some(300)),
            seed("d", 4, Some(400)),
        ]),
        &queue(Some(1), Some(1), None),
        &PolicyState::default(),
        1,
    );

    assert_eq!(paused(&out), vec![2, 4]);
}

#[test]
fn the_total_limit_caps_both_kinds_together() {
    // Generous per-kind limits, but only two slots overall.
    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(100)),
            seed("b", 2, Some(200)),
            downloading("c", 3, Some(300)),
        ]),
        &queue(Some(9), Some(9), Some(2)),
        &PolicyState::default(),
        1,
    );

    assert_eq!(paused(&out), vec![3]);
}

#[test]
fn a_torrent_the_user_paused_is_never_admitted() {
    // Rule 1. The slot is free and this torrent is the oldest, and it still
    // must not be started -- the user's pause outranks the queue.
    let mut user_paused = downloading("a", 1, Some(100));
    user_paused.state = TorrentState::Paused;

    let out = evaluate(
        &snapshot(vec![user_paused, downloading("b", 2, Some(200))]),
        &queue(Some(5), None, None),
        &PolicyState::default(),
        1,
    );

    assert!(resumed(&out).is_empty(), "got {:?}", out.actions);
}

#[test]
fn the_queue_does_not_start_a_torrent_a_limit_stopped() {
    // Rule 2, and the interaction #56 names explicitly. The seed is over its
    // ratio, there is a free slot, and it must stay stopped.
    let mut rules = queue(Some(5), Some(5), None);
    rules.global.seed_ratio_limit = Some(2.0);

    let mut stopped = seeding("a", 1, 9_000, 1_000); // ratio 9.0
    stopped.state = TorrentState::Paused;
    let mut state = PolicyState::default();
    state.mark_paused("a", PauseReason::RatioReached);

    let out = evaluate(&snapshot(vec![stopped]), &rules, &state, 1);

    assert!(resumed(&out).is_empty(), "got {:?}", out.actions);
    assert_eq!(
        out.state.paused_reason("a"),
        Some(PauseReason::RatioReached),
        "and the reason must not be overwritten with Queued"
    );
}

#[test]
fn a_torrent_stopped_by_a_limit_does_not_hold_a_slot() {
    // The other half of the same interaction: a ratio-stopped torrent is not
    // running, so it must not count against the limit and starve a torrent
    // that could run.
    let mut rules = queue(None, Some(1), None);
    rules.global.seed_ratio_limit = Some(2.0);

    let mut stopped = seeding("a", 1, 9_000, 1_000);
    stopped.state = TorrentState::Paused;
    stopped.added_at = Some(100);
    let mut state = PolicyState::default();
    state.mark_paused("a", PauseReason::RatioReached);

    let out = evaluate(
        &snapshot(vec![stopped, seed("b", 2, Some(200))]),
        &rules,
        &state,
        1,
    );

    assert!(
        paused(&out).is_empty(),
        "b should keep the free slot, got {:?}",
        out.actions
    );
}

#[test]
fn checking_and_errored_torrents_occupy_no_slot_and_are_left_alone() {
    let mut checking = downloading("a", 1, Some(100));
    checking.state = TorrentState::Checking;
    let mut errored = downloading("b", 2, Some(200));
    errored.state = TorrentState::Error;

    let out = evaluate(
        &snapshot(vec![checking, errored, downloading("c", 3, Some(300))]),
        &queue(Some(1), None, None),
        &PolicyState::default(),
        1,
    );

    assert!(
        out.actions.is_empty(),
        "neither should be touched, nor should they crowd out c: {:?}",
        out.actions
    );
}

#[test]
fn a_freed_slot_starts_the_next_torrent_in_line() {
    // b was queued; a has gone away. b should be resumed, not left waiting.
    let mut waiting = downloading("b", 2, Some(200));
    waiting.state = TorrentState::Paused;
    let mut state = PolicyState::default();
    state.mark_paused("b", PauseReason::Queued);

    let out = evaluate(
        &snapshot(vec![waiting]),
        &queue(Some(1), None, None),
        &state,
        1,
    );

    assert_eq!(resumed(&out), vec![2]);
    assert_eq!(
        out.state.paused_reason("b"),
        None,
        "and it is no longer queued"
    );
}

#[test]
fn a_settled_queue_does_not_flap() {
    // Once the queue has settled, every later tick must be silent. Pausing an
    // already-queued torrent every second would flood the log and the engine.
    let mut waiting = downloading("c", 3, Some(300));
    waiting.state = TorrentState::Paused;
    let mut state = PolicyState::default();
    state.mark_paused("c", PauseReason::Queued);

    let snap = snapshot(vec![
        downloading("a", 1, Some(100)),
        downloading("b", 2, Some(200)),
        waiting,
    ]);
    let rules = queue(Some(2), None, None);

    let out = evaluate(&snap, &rules, &state, 1);
    assert!(out.actions.is_empty(), "first tick: {:?}", out.actions);

    let again = evaluate(&snap, &rules, &out.state, 1);
    assert!(again.actions.is_empty(), "second tick: {:?}", again.actions);
}

#[test]
fn torrents_with_no_arrival_time_queue_last() {
    // An unknown arrival time is not evidence of being early. A pre-1.2.0
    // torrent must not jump ahead of one Flume actually recorded.
    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, None),
            downloading("b", 2, Some(999)),
        ]),
        &queue(Some(1), None, None),
        &PolicyState::default(),
        1,
    );

    assert_eq!(paused(&out), vec![1], "the one with no time should wait");
}

#[test]
fn a_torrent_reported_as_queued_stays_queued() {
    // The dangerous case. Once `summarize` reports `Queued`, the per-torrent
    // pass sees a state it did not see before -- and if its `Leave` arm treats
    // that as "running normally" it clears the reason, the queue then finds an
    // unclaimed torrent and resumes the one it just parked. Every tick.
    let mut waiting = downloading("c", 3, Some(300));
    waiting.state = TorrentState::Queued;
    let mut state = PolicyState::default();
    state.mark_paused("c", PauseReason::Queued);

    let snap = snapshot(vec![
        downloading("a", 1, Some(100)),
        downloading("b", 2, Some(200)),
        waiting,
    ]);
    let rules = queue(Some(2), None, None);

    let out = evaluate(&snap, &rules, &state, 1);
    assert!(out.actions.is_empty(), "tick 1: {:?}", out.actions);
    assert_eq!(
        out.state.paused_reason("c"),
        Some(PauseReason::Queued),
        "the reason must survive the per-torrent pass"
    );

    let two = evaluate(&snap, &rules, &out.state, 1);
    assert!(two.actions.is_empty(), "tick 2: {:?}", two.actions);
    let three = evaluate(&snap, &rules, &two.state, 1);
    assert!(three.actions.is_empty(), "tick 3: {:?}", three.actions);
}

#[test]
fn a_queued_torrent_is_resumed_when_a_slot_frees() {
    // Same state, but the torrents ahead of it are gone.
    let mut waiting = downloading("c", 3, Some(300));
    waiting.state = TorrentState::Queued;
    let mut state = PolicyState::default();
    state.mark_paused("c", PauseReason::Queued);

    let out = evaluate(
        &snapshot(vec![waiting]),
        &queue(Some(2), None, None),
        &state,
        1,
    );

    assert_eq!(resumed(&out), vec![3]);
    assert_eq!(out.state.paused_reason("c"), None);
}

#[test]
fn a_queued_torrent_that_passes_its_ratio_is_restated_not_resumed() {
    // A finished torrent waiting on a seed slot, whose ratio limit it has
    // already exceeded. When a slot frees it must not start: the stop rule
    // claims it first and the reason changes from Queued to RatioReached.
    let mut waiting = seed("a", 1, Some(100));
    waiting.state = TorrentState::Queued;
    waiting.uploaded_bytes = 9_000;
    waiting.progress_bytes = 1_000;
    let mut state = PolicyState::default();
    state.mark_paused("a", PauseReason::Queued);

    let mut rules = queue(None, Some(5), None);
    rules.global.seed_ratio_limit = Some(2.0);

    let out = evaluate(&snapshot(vec![waiting]), &rules, &state, 1);

    assert!(resumed(&out).is_empty(), "got {:?}", out.actions);
    assert_eq!(
        out.state.paused_reason("a"),
        Some(PauseReason::RatioReached),
        "the stop rule should take it over from the queue"
    );
}

#[tokio::test]
async fn queue_limits_come_from_settings() {
    // Session-wide, so unlike the seed limits there is no per-torrent half to
    // fold in -- settings is the whole source.
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        max_active_downloads: Some(3),
        max_active_seeds: Some(2),
        max_active_total: Some(4),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);

    let rules = state.policy_rules().await;

    assert_eq!(rules.queue.max_active_downloads, Some(3));
    assert_eq!(rules.queue.max_active_seeds, Some(2));
    assert_eq!(rules.queue.max_active_total, Some(4));
}

#[tokio::test]
async fn a_fresh_install_queues_nothing() {
    // The limits must be absent by default: an upgrade that silently started
    // parking torrents would be indistinguishable from a bug.
    let tmp = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);

    let rules = state.policy_rules().await;
    assert!(rules.queue.is_empty());

    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(100)),
            downloading("b", 2, Some(200)),
            downloading("c", 3, Some(300)),
        ]),
        &rules,
        &PolicyState::default(),
        1,
    );
    assert!(out.actions.is_empty(), "got {:?}", out.actions);
}

#[tokio::test]
async fn a_limit_set_in_settings_actually_queues() {
    // The whole point of this slice: settings reach the decision.
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        max_active_downloads: Some(1),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);

    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(100)),
            downloading("b", 2, Some(200)),
        ]),
        &state.policy_rules().await,
        &PolicyState::default(),
        1,
    );

    assert_eq!(paused(&out), vec![2]);
    assert_eq!(out.state.paused_reason("b"), Some(PauseReason::Queued));
}

// --- Manual override of the queue (#56) -------------------------------------

#[test]
fn a_manual_resume_out_of_the_queue_survives_the_next_tick() {
    // The defect this closes, and the reason the rule is stated in both
    // directions. Before the force existed the sequence was: the user resumes
    // a queued torrent, it comes back `Downloading` with no reason on record,
    // the queue finds it eligible, sorts it past the limit, and parks it again
    // -- all inside one second.
    //
    // `c` is the newest of three with room for two, so arrival order alone
    // would stop it. The force is the only thing keeping it running.
    let mut state = PolicyState::default();
    state.mark_forced("c");

    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(100)),
            downloading("b", 2, Some(200)),
            downloading("c", 3, Some(300)),
        ]),
        &queue(Some(2), None, None),
        &state,
        1,
    );

    assert!(
        paused(&out).is_empty(),
        "the forced torrent was re-parked: {:?}",
        out.actions
    );
}

#[test]
fn a_forced_torrent_does_not_take_a_slot_from_the_queue() {
    // Forcing runs a torrent *on top of* the limits rather than displacing
    // one. If the force consumed its slot, `b` would be parked to pay for a
    // decision the user made about `c` -- an action on one torrent stopping a
    // different one, which is the surprise this shape exists to avoid.
    let mut state = PolicyState::default();
    state.mark_forced("c");

    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(100)),
            downloading("b", 2, Some(200)),
            downloading("c", 3, Some(300)),
        ]),
        &queue(Some(2), None, None),
        &state,
        1,
    );

    assert!(
        paused(&out).is_empty(),
        "a forced torrent consumed a slot: {:?}",
        out.actions
    );

    // And the limit still binds for everything that is not forced: a fourth,
    // unforced torrent is beyond the two slots `a` and `b` hold.
    let out = evaluate(
        &snapshot(vec![
            downloading("a", 1, Some(100)),
            downloading("b", 2, Some(200)),
            downloading("c", 3, Some(300)),
            downloading("d", 4, Some(400)),
        ]),
        &queue(Some(2), None, None),
        &state,
        1,
    );

    assert_eq!(
        paused(&out),
        vec![4],
        "the limit stopped binding for unforced torrents: {:?}",
        out.actions
    );
}

#[tokio::test]
async fn only_a_queued_torrent_is_forced_by_resuming_it() {
    // The distinction the whole feature turns on. Resuming something the queue
    // parked means "not this one, I want it now". Resuming something the
    // *user* paused is just un-pausing, and it rejoins the queue and waits its
    // turn -- otherwise every ordinary Resume would quietly punch a permanent
    // hole in the limit.
    let tmp = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);

    // Parked by the queue.
    state
        .set_policy_state({
            let mut s = PolicyState::default();
            s.mark_paused("queued", PauseReason::Queued);
            s
        })
        .await;

    assert!(
        state.force_out_of_queue("queued").await,
        "a queued torrent should be forced by a resume"
    );
    assert!(state.is_forced("queued").await);
    assert_eq!(
        state.policy_state().await.paused_reason("queued"),
        None,
        "and it should stop claiming to be waiting for a slot"
    );

    // Paused by the user: no record at all, which is what a hand pause leaves.
    assert!(
        !state.force_out_of_queue("by-hand").await,
        "an ordinary resume must not force"
    );
    assert!(!state.is_forced("by-hand").await);
}

#[tokio::test]
async fn a_force_survives_a_full_quit_and_relaunch() {
    // An unpersisted force would be re-parked by the queue on the next launch,
    // which is the same astonishment one quit later. Exercised through the
    // calls the app makes, like the seeding-time round trip above.
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().to_path_buf();

    let first = AppState::new(Settings::default(), dir.clone(), false);
    first.set_forced("a", true).await;

    let second = AppState::new(Settings::default(), dir.clone(), false);
    assert!(
        second.is_forced("a").await,
        "the force was lost across a restart"
    );

    // And pausing gives the torrent back to the queue, permanently.
    second.set_forced("a", false).await;
    let third = AppState::new(Settings::default(), dir, false);
    assert!(
        !third.is_forced("a").await,
        "the release was lost across a restart"
    );
}

#[tokio::test]
async fn forgetting_a_torrent_drops_its_force() {
    // Nothing in the library outlives its torrent; a force is no exception,
    // or a re-add of the same info hash would inherit an exemption the user
    // granted to something they removed.
    let tmp = tempfile::tempdir().expect("temp dir");
    let dir = tmp.path().to_path_buf();

    let state = AppState::new(Settings::default(), dir.clone(), false);
    state.set_forced("a", true).await;
    state.forget_torrent("a").await;

    let next = AppState::new(Settings::default(), dir, false);
    assert!(
        !next.is_forced("a").await,
        "a removed torrent kept its force"
    );
}

/// The guard must back off after a failed start, not retry every second.
///
/// It retries on every tick, which is once a second. That was harmless while
/// a start could only fail locally; a blocklist URL made a failed start carry
/// an HTTP request to a third party, and a list that 404s fails in
/// milliseconds — so the retry became one request per second, indefinitely.
#[tokio::test]
async fn a_failed_start_is_not_retried_immediately() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);
    let now = Instant::now();

    assert!(
        state.start_allowed(now).await,
        "nothing has failed yet, so a start is allowed"
    );

    state.note_start_failed(now).await;
    assert!(!state.start_allowed(now).await);
    assert!(
        !state
            .start_allowed(now + Duration::from_millis(1_500))
            .await,
        "the first wait is two seconds"
    );
    assert!(state.start_allowed(now + Duration::from_secs(2)).await);
}

/// Consecutive failures wait longer, up to a cap, and never stop.
///
/// Giving up would be the other failure: a start fails for reasons that pass
/// — a list host having a bad minute, a port the previous process has not
/// released — and a guard that stopped trying leaves the library down.
#[tokio::test]
async fn the_wait_grows_with_each_failure_and_then_holds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);
    let now = Instant::now();

    for _ in 0..20 {
        state.note_start_failed(now).await;
    }

    // Capped: twenty doublings from two seconds would be a fortnight.
    assert!(!state.start_allowed(now + Duration::from_secs(59)).await);
    assert!(
        state.start_allowed(now + Duration::from_secs(60)).await,
        "the wait is capped at a minute, so the guard keeps trying"
    );
}

/// Correcting the setting that was failing earns an immediate attempt.
#[tokio::test]
async fn changing_a_setting_clears_the_backoff() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);
    let now = Instant::now();

    state.note_start_failed(now).await;
    assert!(!state.start_allowed(now).await);

    state.clear_start_backoff().await;
    assert!(state.start_allowed(now).await);
}

/// An explicit queue order is admitted in, ahead of arrival order.
///
/// The queue admitted oldest-first and had no way to change it, which was
/// #56's last open bullet. A position placed by the user has to beat arrival
/// time or reordering means nothing.
#[test]
fn an_explicit_queue_position_beats_arrival_order() {
    let older = TorrentSummary {
        added_at: Some(1_000),
        ..queued("a".repeat(40).as_str(), 1)
    };
    let newer = TorrentSummary {
        added_at: Some(2_000),
        ..queued("b".repeat(40).as_str(), 2)
    };

    let snapshot = snapshot(vec![older.clone(), newer.clone()]);
    // The stored order lives in the policy bookkeeping, not on the summary.
    // `TorrentSummary::queue_position` is the rank this order produces, so
    // reading it here would be circular.
    let state = PolicyState::default().with_queue_positions(
        [(newer.info_hash.clone(), 0), (older.info_hash.clone(), 1)]
            .into_iter()
            .collect(),
    );

    let order: Vec<&str> = flume_lib::policy::queue_order(&snapshot, &state)
        .into_iter()
        .map(|t| t.info_hash.as_str())
        .collect();

    assert_eq!(
        order,
        vec![newer.info_hash.as_str(), older.info_hash.as_str()],
        "the placed order must win; without it this is arrival order and the \
         newer torrent sorts second"
    );
}

/// Without positions it is still arrival order, oldest first.
#[test]
fn arrival_order_still_decides_when_nobody_has_reordered() {
    let older = TorrentSummary {
        added_at: Some(1_000),
        ..queued("a".repeat(40).as_str(), 1)
    };
    let newer = TorrentSummary {
        added_at: Some(2_000),
        ..queued("b".repeat(40).as_str(), 2)
    };

    let snapshot = snapshot(vec![newer.clone(), older.clone()]);
    let state = PolicyState::default();

    let order: Vec<&str> = flume_lib::policy::queue_order(&snapshot, &state)
        .into_iter()
        .map(|t| t.info_hash.as_str())
        .collect();

    assert_eq!(
        order,
        vec![older.info_hash.as_str(), newer.info_hash.as_str()]
    );
}

/// Every torrent the queue holds gets a place, even before anyone reorders.
///
/// The regression this covers made the feature unusable rather than wrong:
/// the wire carried the *stored* sort key, which is absent until somebody
/// reorders, so every queued torrent reported no place -- and the UI gates
/// its move actions on having one. Nothing could ever make the first
/// reorder.
#[test]
fn a_queued_torrent_has_a_place_before_anybody_has_reordered() {
    let first = TorrentSummary {
        added_at: Some(1_000),
        ..queued("a".repeat(40).as_str(), 1)
    };
    let second = TorrentSummary {
        added_at: Some(2_000),
        ..queued("b".repeat(40).as_str(), 2)
    };
    let mut snap = snapshot(vec![second.clone(), first.clone()]);

    // Nothing stored: the default state is what a fresh or upgraded library
    // produces, which is exactly the case that was broken.
    let state = PolicyState::default();
    let order: Vec<String> = flume_lib::policy::queue_order(&snap, &state)
        .into_iter()
        .map(|t| t.info_hash.clone())
        .collect();
    flume_lib::policy::stamp_queue_places(&mut snap, &order);

    let place = |hash: &str| {
        snap.torrents
            .iter()
            .find(|t| t.info_hash == hash)
            .and_then(|t| t.queue_position)
    };
    assert_eq!(place(&first.info_hash), Some(1), "oldest is first in line");
    assert_eq!(place(&second.info_hash), Some(2));
}

/// A torrent the queue does not hold has no place, rather than place zero.
#[test]
fn a_torrent_outside_the_queue_has_no_place() {
    let waiting = queued("a".repeat(40).as_str(), 1);
    // Checking is never eligible: the queue does not decide about it.
    let checking = TorrentSummary {
        state: TorrentState::Checking,
        ..queued("b".repeat(40).as_str(), 2)
    };
    let mut snap = snapshot(vec![waiting.clone(), checking.clone()]);

    let state = PolicyState::default();
    let order: Vec<String> = flume_lib::policy::queue_order(&snap, &state)
        .into_iter()
        .map(|t| t.info_hash.clone())
        .collect();
    flume_lib::policy::stamp_queue_places(&mut snap, &order);

    let place = |hash: &str| {
        snap.torrents
            .iter()
            .find(|t| t.info_hash == hash)
            .and_then(|t| t.queue_position)
    };
    assert_eq!(place(&waiting.info_hash), Some(1));
    assert_eq!(
        place(&checking.info_hash),
        None,
        "a dash, not a place: the queue has no opinion about this torrent"
    );
}

/// A kind no limit governs is not in any line.
///
/// With only `max_active_downloads` set, seed admission is unconstrained: a
/// seed is not waiting for anything, and a download moving past one changes
/// nothing about its priority against any other download. Giving those seeds
/// places had the UI offer a move action that could not affect what ran next.
#[test]
fn a_kind_with_no_limit_has_no_line_to_be_in() {
    let early_download = TorrentSummary {
        added_at: Some(100),
        ..queued(&"a".repeat(40), 1)
    };
    let late_download = TorrentSummary {
        added_at: Some(400),
        ..queued(&"b".repeat(40), 2)
    };
    let seed = TorrentSummary {
        added_at: Some(200),
        ..seeding(&"c".repeat(40), 3, 0, 1_000)
    };
    let mut snap = snapshot(vec![
        early_download.clone(),
        seed.clone(),
        late_download.clone(),
    ]);

    let state = PolicyState::default();
    let limits = QueueLimits {
        max_active_downloads: Some(1),
        max_active_seeds: None,
        max_active_total: None,
    };

    // Admission still counts every eligible torrent, seeds included: that is
    // what a total limit would need, and it is not this function's call.
    let admission: Vec<String> = flume_lib::policy::queue_order(&snap, &state)
        .into_iter()
        .map(|t| t.info_hash.clone())
        .collect();
    assert!(
        admission.contains(&seed.info_hash),
        "admission order is over every eligible torrent"
    );

    // The lines the user sees are not.
    let lines = lines_of(&snap, &state, &limits);
    assert_eq!(
        lines,
        vec![vec![
            early_download.info_hash.clone(),
            late_download.info_hash.clone(),
        ]],
        "one line, the downloads, in admission order"
    );

    for line in &lines {
        flume_lib::policy::stamp_queue_places(&mut snap, line);
    }
    assert_eq!(place(&snap, &early_download.info_hash), Some(1));
    assert_eq!(place(&snap, &late_download.info_hash), Some(2));
    assert_eq!(
        place(&snap, &seed.info_hash),
        None,
        "a dash, so the UI offers no move action for a seed nothing is queuing"
    );
}

/// Two per-kind limits are two lines, not one.
///
/// `apply_queue` counts downloads and seeds against separate counters, so
/// with both limits set and no total they are not competing. Ranking them
/// together let a download move past a seed, which reported success and
/// changed both their displayed places without changing which download or
/// seed ran next.
#[test]
fn independent_per_kind_limits_keep_their_own_lines() {
    let download = TorrentSummary {
        added_at: Some(100),
        ..queued(&"a".repeat(40), 1)
    };
    let other_download = TorrentSummary {
        added_at: Some(400),
        ..queued(&"d".repeat(40), 4)
    };
    let seed = TorrentSummary {
        added_at: Some(200),
        ..seeding(&"b".repeat(40), 2, 0, 1_000)
    };
    let other_seed = TorrentSummary {
        added_at: Some(300),
        ..seeding(&"c".repeat(40), 3, 0, 1_000)
    };
    let mut snap = snapshot(vec![
        download.clone(),
        seed.clone(),
        other_seed.clone(),
        other_download.clone(),
    ]);

    let state = PolicyState::default();
    let limits = QueueLimits {
        max_active_downloads: Some(1),
        max_active_seeds: Some(1),
        max_active_total: None,
    };

    assert_eq!(
        lines_of(&snap, &state, &limits),
        vec![
            vec![download.info_hash.clone(), other_download.info_hash.clone()],
            vec![seed.info_hash.clone(), other_seed.info_hash.clone()],
        ],
        "downloads and seeds are separate lines when nothing makes them compete"
    );

    for line in &lines_of(&snap, &state, &limits) {
        flume_lib::policy::stamp_queue_places(&mut snap, line);
    }

    // Each line is ranked from one within itself. A download and a seed both
    // reading "1" is the honest answer: each is first of what it is actually
    // waiting behind.
    assert_eq!(place(&snap, &download.info_hash), Some(1));
    assert_eq!(place(&snap, &other_download.info_hash), Some(2));
    assert_eq!(place(&snap, &seed.info_hash), Some(1));
    assert_eq!(place(&snap, &other_seed.info_hash), Some(2));
}

/// A total limit puts both kinds into one line.
///
/// Under it a download starting does keep a seed waiting, so they are
/// competing for the same slots and ordering one against the other means
/// something.
#[test]
fn a_total_limit_governs_both_kinds() {
    let download = TorrentSummary {
        added_at: Some(100),
        ..queued(&"a".repeat(40), 1)
    };
    let seed = TorrentSummary {
        added_at: Some(200),
        ..seeding(&"b".repeat(40), 2, 0, 1_000)
    };
    let snap = snapshot(vec![download.clone(), seed.clone()]);

    let state = PolicyState::default();
    let limits = QueueLimits {
        max_active_downloads: None,
        max_active_seeds: None,
        max_active_total: Some(1),
    };

    assert_eq!(
        lines_of(&snap, &state, &limits),
        vec![vec![download.info_hash.clone(), seed.info_hash.clone()]],
        "one line, because the total limit decides between them"
    );

    // And no limits at all is no lines, which needs no check of its own.
    assert!(
        lines_of(&snap, &state, &QueueLimits::default()).is_empty(),
        "with nothing limited there is no queue to be in"
    );
}

/// A total limit the per-kind caps cannot reach decides nothing.
///
/// `apply_queue` enforces all three, so a total above everything the per-kind
/// caps can reach together never refuses anything: one download and one seed
/// cannot breach a total of ten. The counters stay independent, so the kinds
/// are still two lines — merging them on the mere presence of a total
/// reported successful moves that changed nothing.
#[test]
fn a_total_limit_that_cannot_bind_does_not_merge_the_lines() {
    let download = TorrentSummary {
        added_at: Some(100),
        ..queued(&"a".repeat(40), 1)
    };
    let seed = TorrentSummary {
        added_at: Some(200),
        ..seeding(&"b".repeat(40), 2, 0, 1_000)
    };
    let snap = snapshot(vec![download.clone(), seed.clone()]);
    let state = PolicyState::default();

    let redundant = QueueLimits {
        max_active_downloads: Some(1),
        max_active_seeds: Some(1),
        max_active_total: Some(10),
    };
    assert!(!redundant.total_can_bind());
    assert_eq!(
        lines_of(&snap, &state, &redundant),
        vec![
            vec![download.info_hash.clone()],
            vec![seed.info_hash.clone()],
        ],
        "a total of ten cannot hold back one download and one seed"
    );

    // Exactly reachable is still not binding: one of each is two, and a
    // total of two refuses nothing the caps already allow.
    let exact = QueueLimits {
        max_active_total: Some(2),
        ..redundant
    };
    assert!(!exact.total_can_bind());

    // One lower, and it decides: now only one of the two may run.
    let binding = QueueLimits {
        max_active_total: Some(1),
        ..redundant
    };
    assert!(binding.total_can_bind());
    assert_eq!(
        lines_of(&snap, &state, &binding),
        vec![vec![download.info_hash.clone(), seed.info_hash.clone()]],
        "one line, because the total now refuses what the caps allow"
    );

    // An uncapped kind is unbounded, so the total is the only thing holding
    // it and always binds.
    let one_kind_uncapped = QueueLimits {
        max_active_downloads: Some(1),
        max_active_seeds: None,
        max_active_total: Some(10),
    };
    assert!(one_kind_uncapped.total_can_bind());
    assert_eq!(
        lines_of(&snap, &state, &one_kind_uncapped),
        vec![vec![download.info_hash.clone(), seed.info_hash.clone()]],
        "ten seeds would breach a total of ten, so they are competing"
    );
}

/// A move is within the torrent's own line, and refused across lines.
///
/// `move_in_queue` had no test at all, which is worth recording given it is
/// the whole of the reorder feature: it derives the order, swaps inside it,
/// and writes both the policy state and the library under one lock. With two
/// independently limited kinds, a download at the bottom of its line is
/// followed in *admission* order by a seed — so a move that walked admission
/// order would report success and change nothing about what runs next.
#[tokio::test]
async fn a_move_stays_inside_its_own_line() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        max_active_downloads: Some(1),
        max_active_seeds: Some(1),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);

    let first_download = "a".repeat(40);
    let second_download = "d".repeat(40);
    let seed = "b".repeat(40);

    let snap = snapshot(vec![
        TorrentSummary {
            added_at: Some(100),
            ..queued(&first_download, 1)
        },
        TorrentSummary {
            added_at: Some(200),
            ..seeding(&seed, 2, 0, 1_000)
        },
        TorrentSummary {
            added_at: Some(400),
            ..queued(&second_download, 4)
        },
    ]);

    // Promoting the later download over the earlier one is a real move.
    assert!(
        state
            .move_in_queue(&snap, &second_download, true, state.removal_mark())
            .await,
        "a download can be moved within the line of downloads"
    );
    let policy = state.policy_state().await;
    assert!(
        policy.queue_position(&second_download) < policy.queue_position(&first_download),
        "and the stored places reflect it"
    );

    // The only seed cannot move: there is nothing in its line to move past,
    // even though downloads sit either side of it in admission order.
    assert!(
        !state
            .move_in_queue(&snap, &seed, true, state.removal_mark())
            .await,
        "a lone seed has nobody to overtake"
    );
    assert!(
        !state
            .move_in_queue(&snap, &seed, false, state.removal_mark())
            .await,
        "in either direction"
    );

    // And the earlier download, now last of two, cannot move down past the
    // seed that follows it in admission order.
    assert!(
        !state
            .move_in_queue(&snap, &first_download, false, state.removal_mark())
            .await,
        "the end of a line is the end, whatever admission order says next"
    );
}

/// A kind capped at zero has no turn, so it has no place either.
///
/// `Settings::validate` says nothing about the queue limits, and a settings
/// file that fails validation has its choices honoured rather than clamped —
/// so a cap of zero is reachable by hand. `apply_queue` can then never admit
/// that kind, so ranking it reported moves that changed nothing, and with a
/// total limit set it also dragged the other kind into a shared line it was
/// not sharing.
#[test]
fn a_kind_capped_at_zero_is_in_no_line() {
    let download = TorrentSummary {
        added_at: Some(100),
        ..queued(&"a".repeat(40), 1)
    };
    let other_download = TorrentSummary {
        added_at: Some(150),
        ..queued(&"d".repeat(40), 4)
    };
    let seed = TorrentSummary {
        added_at: Some(200),
        ..seeding(&"b".repeat(40), 2, 0, 1_000)
    };
    let other_seed = TorrentSummary {
        added_at: Some(300),
        ..seeding(&"c".repeat(40), 3, 0, 1_000)
    };
    let mut snap = snapshot(vec![
        download.clone(),
        other_download.clone(),
        seed.clone(),
        other_seed.clone(),
    ]);
    let state = PolicyState::default();

    // No download can run, and the seeds are held by the total alone.
    let limits = QueueLimits {
        max_active_downloads: Some(0),
        max_active_seeds: None,
        max_active_total: Some(1),
    };
    assert!(
        !limits.can_run(false),
        "nothing gets in under a cap of zero"
    );
    assert!(limits.can_run(true));
    assert!(
        !limits.total_can_bind(),
        "a download that can never run takes no total slot, so nothing is \
         arbitrated"
    );

    assert_eq!(
        lines_of(&snap, &state, &limits),
        vec![vec![seed.info_hash.clone(), other_seed.info_hash.clone()]],
        "the seeds queue against each other; the downloads are not in a line"
    );

    for line in &lines_of(&snap, &state, &limits) {
        flume_lib::policy::stamp_queue_places(&mut snap, line);
    }
    assert_eq!(place(&snap, &seed.info_hash), Some(1));
    assert_eq!(place(&snap, &other_seed.info_hash), Some(2));
    assert_eq!(
        place(&snap, &download.info_hash),
        None,
        "a dash: it is not waiting its turn, it has no turn"
    );
    assert_eq!(place(&snap, &other_download.info_hash), None);

    // The same holds with no total at all, which the finding did not name but
    // is the same thing: a cap of zero is a cap of zero.
    let no_total = QueueLimits {
        max_active_downloads: Some(0),
        max_active_seeds: Some(2),
        max_active_total: None,
    };
    assert_eq!(
        lines_of(&snap, &state, &no_total),
        vec![vec![seed.info_hash.clone(), other_seed.info_hash.clone()]],
        "still no line of downloads"
    );

    // And a total of zero stops everything, so there are no lines at all.
    let nothing_runs = QueueLimits {
        max_active_downloads: None,
        max_active_seeds: None,
        max_active_total: Some(0),
    };
    assert!(
        lines_of(&snap, &state, &nothing_runs).is_empty(),
        "nothing can run, so nobody is in a queue"
    );
}

/// A move inside one line says nothing about the other line.
///
/// Written as the line alone, a first reorder of the downloads gave every
/// download a place while the seeds kept none — and `queue_order` sorts an
/// absent place last. So raising the limits later, so that the two kinds
/// share a line, put every download ahead of every seed. The user moved one
/// download past another; they did not say that.
#[tokio::test]
async fn a_move_does_not_reorder_the_line_it_was_not_in() {
    let tmp = tempfile::tempdir().expect("temp dir");
    // Two lines: each kind capped, no total, so nothing makes them compete.
    let settings = Settings {
        max_active_downloads: Some(1),
        max_active_seeds: Some(1),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);

    let early_download = "a".repeat(40);
    let first_seed = "b".repeat(40);
    let second_seed = "c".repeat(40);
    let late_download = "d".repeat(40);

    // Admission order by arrival, since nobody has reordered: download,
    // seed, seed, download.
    let snap = snapshot(vec![
        TorrentSummary {
            added_at: Some(100),
            ..queued(&early_download, 1)
        },
        TorrentSummary {
            added_at: Some(200),
            ..seeding(&first_seed, 2, 0, 1_000)
        },
        TorrentSummary {
            added_at: Some(300),
            ..seeding(&second_seed, 3, 0, 1_000)
        },
        TorrentSummary {
            added_at: Some(400),
            ..queued(&late_download, 4)
        },
    ]);

    assert!(
        state
            .move_in_queue(&snap, &late_download, true, state.removal_mark())
            .await
    );

    // Every eligible torrent now has a place, not just the downloads.
    let policy = state.policy_state().await;
    for hash in [&early_download, &first_seed, &second_seed, &late_download] {
        assert!(
            policy.queue_position(hash).is_some(),
            "{hash} has no place, so a later limits change would sort it last"
        );
    }

    // The test: under a total that binds, the two kinds share one line, and
    // that line must be the original arrival order with only the two
    // downloads swapped.
    let binding = QueueLimits {
        max_active_downloads: None,
        max_active_seeds: None,
        max_active_total: Some(1),
    };
    assert!(binding.total_can_bind());
    assert_eq!(
        lines_of(&snap, &policy, &binding),
        vec![vec![
            late_download.clone(),
            first_seed.clone(),
            second_seed.clone(),
            early_download.clone(),
        ]],
        "the seeds keep their place between the downloads"
    );
}

/// A reorder leaves the place of a torrent it does not mention.
///
/// `queue_order` returns only the torrents eligible to run, so one that is
/// manually paused, stopped by a limit or forced is absent from it while
/// still holding a place the user gave it. `PolicyState::set_queue_order`
/// replaced its whole map from that order, so reordering anything else
/// silently dropped the absent torrent's place and it sorted last on resume
/// — while `Library::set_queue_order` had kept the record, so a restart
/// restored it and the queue appeared to change on its own.
#[test]
fn a_paused_torrent_keeps_its_place_while_the_rest_are_reordered() {
    let first = "b".repeat(40);
    let second = "c".repeat(40);
    let third = "a".repeat(40);

    // The user put `first` at the top and then paused it themselves, which
    // policy does not record -- so it is not "queued by us" and drops out of
    // the eligible set.
    let mut state = PolicyState::default().with_queue_positions(
        [(first.clone(), 0), (second.clone(), 2), (third.clone(), 1)]
            .into_iter()
            .collect(),
    );

    let paused = TorrentSummary {
        state: TorrentState::Paused,
        ..seeding(&first, 1, 0, 1_000)
    };
    let snap = snapshot(vec![
        paused.clone(),
        seeding(&second, 2, 0, 1_000),
        seeding(&third, 3, 0, 1_000),
    ]);

    let eligible: Vec<String> = flume_lib::policy::queue_order(&snap, &state)
        .into_iter()
        .map(|t| t.info_hash.clone())
        .collect();
    assert_eq!(
        eligible,
        vec![third.clone(), second.clone()],
        "the paused torrent is not eligible, so a reorder cannot name it"
    );

    // Promote `second` over `third`, which is a statement about those two.
    state.set_queue_order(&[second.clone(), third.clone()]);

    assert_eq!(
        state.queue_position(&first),
        Some(0),
        "the reorder said nothing about this torrent and must not move it"
    );

    // Resumed, it is back in the running and still ahead of the one it was
    // ahead of. Its 0 now ties with `second`'s, which `queue_order` breaks on
    // arrival time and then info hash -- a total order either way, and the tie
    // leaves it beside where it was rather than at the end.
    let snap = snapshot(vec![
        seeding(&first, 1, 0, 1_000),
        seeding(&second, 2, 0, 1_000),
        seeding(&third, 3, 0, 1_000),
    ]);
    let resumed: Vec<String> = flume_lib::policy::queue_order(&snap, &state)
        .into_iter()
        .map(|t| t.info_hash.clone())
        .collect();
    assert_eq!(
        resumed,
        vec![first.clone(), second.clone(), third.clone()],
        "a place the user gave survived a reorder it was not part of"
    );
}

/// A reorder does not hand out a slot an absent torrent is already holding.
///
/// Keeping the absent torrent's old number is not enough on its own. A newer
/// torrent promoted to the front and then paused would share slot 0 with
/// whichever torrent the reorder put first, and `queue_order` breaks equal
/// places by arrival time — which never favours the newer one. So the user's
/// explicit "this one first" would lose to arrival order, silently, and stay
/// lost.
#[test]
fn a_promoted_torrent_that_is_paused_does_not_lose_its_slot_to_an_older_one() {
    // Promoted to the front by the user, then paused by them, and the newest
    // of the three — so every tie goes against it.
    let promoted = "c".repeat(40);
    let older = "b".repeat(40);
    let middling = "a".repeat(40);

    let mut state = PolicyState::default().with_queue_positions(
        [
            (promoted.clone(), 0),
            (middling.clone(), 1),
            (older.clone(), 2),
        ]
        .into_iter()
        .collect(),
    );

    let arrived = |hash: &str, id: usize, at: u64| TorrentSummary {
        added_at: Some(at),
        ..seeding(hash, id, 0, 1_000)
    };

    let paused = TorrentSummary {
        state: TorrentState::Paused,
        ..arrived(&promoted, 1, 200)
    };
    let snap = snapshot(vec![
        paused,
        arrived(&middling, 2, 100),
        arrived(&older, 3, 50),
    ]);

    let eligible: Vec<String> = flume_lib::policy::queue_order(&snap, &state)
        .into_iter()
        .map(|t| t.info_hash.clone())
        .collect();
    assert_eq!(eligible, vec![middling.clone(), older.clone()]);

    // The user promotes `older` over `middling`, which says nothing about the
    // paused one.
    let assigned = state.set_queue_order(&[older.clone(), middling.clone()]);
    assert_eq!(
        assigned,
        vec![(older.clone(), 1), (middling.clone(), 2)],
        "slot 0 is taken, so the reorder starts at 1 rather than over the top"
    );
    assert_eq!(
        state.queue_position(&promoted),
        Some(0),
        "and the promoted torrent keeps the slot it was given"
    );

    // Resumed, it is still first. Numbering from zero would have put `older`
    // at 0 beside it, and the arrival-time tie-break would have gone to
    // `older`.
    let snap = snapshot(vec![
        arrived(&promoted, 1, 200),
        arrived(&middling, 2, 100),
        arrived(&older, 3, 50),
    ]);
    let resumed: Vec<String> = flume_lib::policy::queue_order(&snap, &state)
        .into_iter()
        .map(|t| t.info_hash.clone())
        .collect();
    assert_eq!(
        resumed,
        vec![promoted.clone(), older.clone(), middling.clone()],
        "an explicit place must not lose to arrival order"
    );
}

/// A removed torrent's queue place is pruned with the rest of its state.
///
/// `forget` drops the bookkeeping for a torrent that was removed. Queue
/// places were not in that list, so re-adding the same info hash inherited
/// the place its predecessor had — ahead of torrents that had been waiting —
/// even though `Library::forget` had dropped the persisted record.
#[test]
fn a_removed_torrent_does_not_keep_its_place_in_the_queue() {
    let gone = "a".repeat(40);
    let staying = "b".repeat(40);

    let mut state = PolicyState::default().with_queue_positions(
        [(gone.clone(), 0), (staying.clone(), 1)]
            .into_iter()
            .collect(),
    );

    state.forget(&gone);

    assert_eq!(
        state.queue_position(&gone),
        None,
        "the place outlived the torrent, so a re-add would inherit it"
    );
    assert_eq!(
        state.queue_position(&staying),
        Some(1),
        "pruning one must not drop the others"
    );
}

/// Absence from a snapshot is not a removal, and must not prune anything.
///
/// `evaluate` used to drop bookkeeping for every info hash the snapshot did
/// not carry. A persisted torrent whose drive is unmounted fails to restore
/// and is missing from every snapshot of the run, so the first tick after
/// launch stripped the queue place, force, seeding time and stop reason of
/// exactly the torrent `library.json` keeps a record for on purpose.
#[test]
fn absence_from_a_snapshot_is_not_a_removal() {
    let missing = "a".repeat(40);
    let present = "b".repeat(40);

    let mut before = PolicyState::with_seed_times(HashMap::from([(missing.clone(), 4_200)]))
        .with_forced([missing.clone()].into_iter().collect())
        .with_queue_positions([(missing.clone(), 0)].into_iter().collect());
    before.mark_paused(&missing, PauseReason::SeedTimeReached);

    let after = evaluate(
        &snapshot(vec![seeding(&present, 1, 0, 1_000)]),
        &Rules::default(),
        &before,
        1,
    )
    .state;

    assert_eq!(
        after.seed_seconds(&missing),
        4_200,
        "seeding time was lost, so a seed-time limit fires late"
    );
    assert!(
        after.is_forced(&missing),
        "the force was lost, so the queue re-parks a torrent the user started"
    );
    assert_eq!(
        after.queue_position(&missing),
        Some(0),
        "the place was lost, so the torrent sorts behind everything it was put ahead of"
    );
    assert_eq!(
        after.paused_reason(&missing),
        Some(PauseReason::SeedTimeReached),
        "the stop reason was lost, so the row says \"paused\" without saying why"
    );
}

/// The window in full: a failed restore, then an in-process engine restart.
///
/// The two halves compound. Nothing reloads the library into `PolicyState`
/// except `AppState::new`, so a tick that pruned the absent torrent left it
/// stripped for the rest of the run -- a drive that came back and an engine
/// restarted in place recovered the torrent and none of what Flume knew
/// about it. Only quitting the application put it back.
#[tokio::test]
async fn a_torrent_that_failed_to_restore_survives_an_engine_restart() {
    let tmp = tempfile::tempdir().expect("temp dir");

    let mut library = Library::default();
    library.note_added("a", Some(1_000));
    library.set_forced("a", true);
    library.set_queue_positions(&[("a".to_owned(), 0)]);
    library.record_seed_times(&HashMap::from([("a".to_owned(), 4_200)]));
    library.save(tmp.path()).expect("save");

    // Local only: no DHT, no UPnP, an ephemeral port, and a download
    // directory inside the temporary tree.
    let settings = Settings {
        download_dir: tmp.path().join("downloads"),
        listen_port: 0,
        enable_dht: false,
        enable_upnp: false,
        ..Settings::default()
    };
    let state = AppState::new(settings.clone(), tmp.path().to_path_buf(), false);

    // The drive is unmounted, so "a" is in the library and in no snapshot.
    state
        .evaluate_policy(
            &snapshot(vec![seeding("b", 1, 0, 1_000)]),
            &Rules::default(),
            1,
            state.removal_mark(),
        )
        .await;

    state
        .restart_engine(&settings)
        .await
        .expect("engine starts");

    let policy = state.policy_state().await;
    assert_eq!(
        policy.seed_seconds("a"),
        4_200,
        "seeding time did not survive the restart"
    );
    assert!(
        policy.is_forced("a"),
        "the force did not survive the restart"
    );
    assert_eq!(
        policy.queue_position("a"),
        Some(0),
        "the queue place did not survive the restart"
    );
}

/// A removal and a re-add inside one tick inherit nothing.
///
/// The mirror of the case above, and the same cause: pruning was driven by a
/// later absence, so an info hash that was removed and added again before
/// the next evaluation was never absent at all. The new torrent took its
/// predecessor's place at the front of the queue, its force, and seeding
/// time it had not earned.
///
/// In process, deliberately. `forgetting_a_torrent_drops_its_force` proves
/// the *record* is gone by building a second `AppState`, which says nothing
/// about the state the next tick actually reads.
#[tokio::test]
async fn a_removal_prunes_the_bookkeeping_the_next_tick_reads() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);

    state.note_torrent_added("a").await;
    state.set_forced("a", true).await;
    let mut carried = PolicyState::with_seed_times(HashMap::from([("a".to_owned(), 7_200)]))
        .with_forced(["a".to_owned()].into_iter().collect())
        .with_queue_positions([("a".to_owned(), 0)].into_iter().collect());
    carried.mark_paused("a", PauseReason::SeedTimeReached);
    state.set_policy_state(carried).await;

    state.forget_torrent("a").await;

    let policy = state.policy_state().await;
    assert!(
        !policy.is_forced("a"),
        "a re-add inherits an exemption granted to a torrent that is gone"
    );
    assert_eq!(
        policy.seed_seconds("a"),
        0,
        "a re-add inherits seeding time it never earned, so a limit fires early"
    );
    assert_eq!(
        policy.queue_position("a"),
        None,
        "a re-add inherits a place ahead of torrents that had been waiting"
    );
    assert_eq!(
        policy.paused_reason("a"),
        None,
        "a re-add arrives already carrying a stop reason"
    );
}

/// A snapshot taken before a removal cannot put the bookkeeping back.
///
/// The telemetry loop captures a snapshot and then evaluates it, so a
/// removal can land in the gap: `forget_torrent` prunes, and the evaluation
/// that follows is still holding a snapshot in which the torrent exists, so
/// it writes seeding time and a stop reason for a torrent that is gone.
///
/// Harmless while `evaluate` pruned on absence, because the next tick's
/// snapshot dropped it again. With that gone the resurrected state is
/// permanent, and the next seed-time flush writes a record for the removed
/// torrent back into `library.json`, where reconciliation never deletes it.
#[tokio::test]
async fn a_snapshot_taken_before_a_removal_cannot_put_its_bookkeeping_back() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        seed_ratio_limit: Some(1.0),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);

    // Stamped and captured first, like the loop does. Ratio 50, so the
    // evaluation below would stop it and record why.
    let mark = state.removal_mark();
    let stale = snapshot(vec![seeding("a", 1, 50_000, 1_000)]);

    state.forget_torrent("a").await;

    let rules = state.policy_rules().await;
    let actions = state.evaluate_policy(&stale, &rules, 60, mark).await;

    assert!(
        actions.is_empty(),
        "a snapshot a removal has overtaken decides nothing, got {actions:?}"
    );

    let policy = state.policy_state().await;
    assert_eq!(
        policy.seed_seconds("a"),
        0,
        "seeding time came back for a removed torrent, and nothing prunes it now"
    );
    assert_eq!(
        policy.paused_reason("a"),
        None,
        "a stop reason came back for a torrent that no longer exists"
    );
}

/// A removal invalidates the snapshots before it and nothing after them.
///
/// The refusal has to end, or one removal would stop policy for the rest of
/// the run. It also has to end *cleanly* for a re-add of the same info hash:
/// the next snapshot describes the new torrent, and what the user has
/// already said about it -- a force, granted seconds after the re-add --
/// is theirs rather than its predecessor's.
#[tokio::test]
async fn a_snapshot_taken_after_a_removal_is_evaluated_normally() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);

    state.forget_torrent("a").await;

    // Re-added under the same hash, and forced past the queue by hand.
    state.note_torrent_added("a").await;
    state.set_forced("a", true).await;

    // Stamped after the removal, so it describes the torrent that exists.
    let mark = state.removal_mark();
    state
        .evaluate_policy(
            &snapshot(vec![seeding("a", 1, 0, 1_000)]),
            &Rules::default(),
            60,
            mark,
        )
        .await;

    assert!(
        state.is_forced("a").await,
        "a force the user granted to the torrent that replaced it was dropped"
    );
    assert_eq!(
        state.policy_state().await.seed_seconds("a"),
        60,
        "the refusal outlived the removal that caused it"
    );
}

/// The same window, through the reorder path.
///
/// `move_in_queue` derives the order from a snapshot the caller took, so a
/// torrent removed since is still in that order and `set_queue_order` would
/// give it a place -- the one write that puts a queue position back after
/// `forget_torrent` dropped it, and nothing sweeps it afterwards. Refused
/// on the same stamp as an evaluation: the line the user was looking at is
/// not the line any more.
#[tokio::test]
async fn a_reorder_from_a_snapshot_older_than_a_removal_is_refused() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        max_active_downloads: Some(1),
        ..Settings::default()
    };
    let state = AppState::new(settings, tmp.path().to_path_buf(), false);

    let gone = "a".repeat(40);
    let staying = "d".repeat(40);
    let mark = state.removal_mark();
    let stale = snapshot(vec![
        TorrentSummary {
            added_at: Some(100),
            ..queued(&gone, 1)
        },
        TorrentSummary {
            added_at: Some(400),
            ..queued(&staying, 2)
        },
    ]);

    state.forget_torrent(&gone).await;

    assert!(
        !state.move_in_queue(&stale, &staying, true, mark).await,
        "a move derived from a queue that has changed must not report success"
    );
    assert_eq!(
        state.policy_state().await.queue_position(&gone),
        None,
        "the removed torrent was given a place again, and a re-add inherits it"
    );
}

/// A removal prunes even when the library has no record to drop.
///
/// `AppState::forget_torrent` returns early when `Library::forget` reports
/// nothing removed, which covers both a torrent with no record and every
/// write while the library is degraded -- and a force reaches the policy
/// state in that degraded mode whether or not the record accepts it. So the
/// bookkeeping is pruned before the record is touched, never behind that
/// early return.
#[tokio::test]
async fn a_removal_prunes_bookkeeping_the_library_knows_nothing_about() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(Settings::default(), tmp.path().to_path_buf(), false);

    state
        .set_policy_state(
            PolicyState::default().with_forced(["a".to_owned()].into_iter().collect()),
        )
        .await;

    state.forget_torrent("a").await;

    assert!(
        !state.is_forced("a").await,
        "the force outlived the torrent because the record did not exist"
    );
}

/// A force survives a library that cannot be written, and is still reported.
///
/// `Library::set_forced` refuses every write when `library.json` could not
/// be read, so the record stays empty — but `mark_forced` still lands in the
/// policy state, which is what admission actually consults. Reporting from
/// the record meant the queue correctly left the torrent alone while the UI
/// said it was not forced, so an overridden limit read as a broken one in
/// exactly the degraded mode the library is built to survive.
#[test]
fn a_force_is_known_to_policy_even_when_the_library_cannot_record_it() {
    let hash = "a".repeat(40);
    let mut state = PolicyState::default();

    state.mark_forced(&hash);

    assert!(
        state.is_forced(&hash),
        "admission reads this, so it has to hold the force"
    );
    assert!(
        state.forced().contains(&hash),
        "and the snapshot reads this, so the two cannot disagree"
    );
}
