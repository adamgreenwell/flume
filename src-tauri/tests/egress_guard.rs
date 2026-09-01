//! The guard's decision path, end to end through `AppState`.
//!
//! `AppState` imports no Tauri types, so everything up to "should there be an
//! engine" is testable in a plain `cargo test` process. What is *not* covered
//! here is the acting on that decision — starting and stopping the session —
//! which lives in `crate::guard` and needs an `AppHandle`.
//!
//! These assertions are written to hold with or without a working network. A
//! machine with no route produces `Verdict::Unknown`, which does not permit
//! transfer, so every case below is either "the guard is not holding because
//! it was told not to" or "the guard is holding because nothing permitted it".
//! Neither depends on what the routing table actually says.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{path::PathBuf, time::Instant};

use flume_lib::{
    egress::{EgressGuard, SETTLE},
    settings::Settings,
    state::AppState,
};

fn state_with(guard: EgressGuard, pinned: Option<&str>) -> AppState {
    let tmp = tempfile::tempdir().expect("temp dir");
    let settings = Settings {
        download_dir: tmp.path().to_path_buf(),
        egress_guard: guard,
        egress_interface: pinned.map(str::to_owned),
        ..Settings::default()
    };
    AppState::new(settings, PathBuf::from(tmp.path()), false)
}

/// An interface name no machine has, so the verdict cannot permit transfer.
const IMPOSSIBLE: &str = "flume-test-no-such-interface";

#[tokio::test]
async fn the_guard_holds_nothing_when_it_is_off() {
    // Off means Flume does not act on the answer, not that it does not know
    // it: the report is still populated so the UI can show where traffic goes.
    let state = state_with(EgressGuard::Off, Some(IMPOSSIBLE));

    let status = state.observe_egress(Instant::now()).await;

    assert!(!status.held, "Off must never hold transfer");
    assert_eq!(status.guard, EgressGuard::Off);
    assert_eq!(status.resumes_in_seconds, None);
}

#[tokio::test]
async fn warn_reports_without_stopping_anything() {
    // The middle setting exists precisely because wanting to know is not the
    // same as wanting transfer stopped.
    let state = state_with(EgressGuard::Warn, Some(IMPOSSIBLE));

    let status = state.observe_egress(Instant::now()).await;

    assert!(!status.held, "Warn says so and stops nothing");
    assert!(
        !status.report.verdict.allows_transfer(),
        "the pin cannot match, so the verdict must not permit"
    );
}

#[tokio::test]
async fn hold_stops_transfer_when_the_verdict_does_not_permit() {
    let state = state_with(EgressGuard::Hold, Some(IMPOSSIBLE));

    let status = state.observe_egress(Instant::now()).await;

    assert!(status.held);
    assert!(!status.report.verdict.allows_transfer());
}

#[tokio::test]
async fn a_held_guard_publishes_what_a_command_later_reads() {
    // `check_egress` reads the published status rather than probing, so that
    // the UI and the engine loop cannot disagree about what the routing table
    // said. This is that contract.
    let state = state_with(EgressGuard::Hold, Some(IMPOSSIBLE));

    let observed = state.observe_egress(Instant::now()).await;
    let published = state.egress_status().await;

    assert_eq!(observed, published);
}

#[tokio::test]
async fn a_status_exists_before_any_tick_has_run() {
    // There must be no window in which a command has to answer "unknown" for
    // reasons that have nothing to do with the network. `AppState::new`
    // probes once, synchronously.
    let state = state_with(EgressGuard::Hold, Some(IMPOSSIBLE));

    let status = state.egress_status().await;

    assert_eq!(status.guard, EgressGuard::Hold);
    assert!(
        status.held,
        "the pre-tick status must fail closed, or it is believed for a second"
    );
}

#[tokio::test]
async fn a_permitting_verdict_still_waits_out_the_settle_window() {
    // Driven through `AppState` rather than the gate directly, to pin that the
    // hysteresis is actually wired in rather than merely implemented.
    let state = state_with(EgressGuard::Hold, None);
    let base = Instant::now();

    // Whatever this machine's routing table says, start from held.
    state.observe_egress(base).await;

    let permitted = state.egress_status().await.report.verdict.allows_transfer();
    if !permitted {
        // No tunnel on this machine, so there is no release to observe. The
        // hysteresis itself is covered by the unit tests in `egress`.
        return;
    }

    let immediately = state.observe_egress(base).await;
    assert!(
        immediately.held,
        "a verdict that has only just started permitting must not release yet"
    );
    assert_eq!(
        immediately.resumes_in_seconds,
        Some(SETTLE.as_secs()),
        "the countdown has to be reportable, or the wait is inexplicable"
    );

    let later = state.observe_egress(base + SETTLE + SETTLE).await;
    assert!(!later.held, "the window elapsed, so transfer resumes");
    assert_eq!(later.resumes_in_seconds, None);
}

#[tokio::test]
async fn a_user_edit_drops_the_settle_window() {
    let state = state_with(EgressGuard::Hold, None);
    let base = Instant::now();

    state.observe_egress(base).await;
    state.release_egress_settle().await;

    // The gate is open, so the next observation is decided purely by the
    // verdict rather than by a window that started under the old setting.
    let status = state.observe_egress(base).await;
    let permitted = status.report.verdict.allows_transfer();
    assert_eq!(
        status.held, !permitted,
        "after a release, held must track the verdict directly"
    );
}
