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
        CoreStatus, DhtStatus, EngineHealth, PauseReason, SwarmHealth, TelemetrySnapshot,
        TorrentState, TorrentSummary,
    },
    library::Library,
    policy::{Action, PolicyState, Rules, TorrentRules, evaluate},
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

fn seed_time_limit(secs: u64) -> Rules {
    Rules {
        global: TorrentRules {
            seed_time_limit_secs: Some(secs),
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
fn restored_time_for_a_torrent_that_is_gone_is_dropped() {
    // `retain` runs against the snapshot, so a torrent removed while Flume
    // was closed does not keep its bookkeeping alive for ever.
    let restored = PolicyState::with_seed_times(HashMap::from([
        ("a".to_owned(), 500),
        ("gone".to_owned(), 900),
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
        "the survivor keeps its time"
    );
    assert_eq!(
        out.state.seed_seconds("gone"),
        0,
        "the absent one is dropped"
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
