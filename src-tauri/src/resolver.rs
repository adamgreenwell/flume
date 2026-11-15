//! The loop that keeps looking for pending magnets' file lists (#148).
//!
//! A magnet the user added before its file list arrived lives in the library
//! record, not in librqbit (see [`crate::library::magnet`]). This loop asks
//! the engine for each one on a lengthening schedule, saves the file list
//! when it arrives, and tells the user. It never adds the torrent: a resolved
//! magnet holds until the user chooses files.
//!
//! # Behind the egress guard, like everything else
//!
//! An attempt is a DHT lookup plus a tracker announce, so it is transfer in
//! every sense the guard cares about. It starts only with an engine present,
//! the guard not holding *and* a fresh probe agreeing, and peer discovery
//! ready. While it runs, it is watched once a second and dropped the moment
//! the guard holds, the engine is replaced or the app begins exiting.
//! Dropping the future cancels the lookup: a list-only add registers nothing
//! in the session until it returns. Architecture rule 12 -- a retry that
//! carried on through a hold would be the guard's first leak.
//!
//! This is torrent traffic through the engine, on the user's behalf, not a
//! request Flume makes for itself -- so it is not a third entry in
//! architecture rule 10's list.
//!
//! # Why the loop lives here and not in `engine` or `library`
//!
//! It needs an `AppHandle` for notifications and for [`AppState`], and both
//! of those modules import no Tauri types. The schedule and the storage stay
//! down there, pure and tested; only the acting on them lives here.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::{
    engine::{Engine, EngineError, TorrentSource},
    state::{AppState, AttemptEnd},
};

/// How often the loop looks for a magnet that is due.
const RESOLVER_INTERVAL: Duration = Duration::from_secs(2);

/// How long one background attempt may wait for a file list.
///
/// Longer than the dialog's ten seconds on purpose. The dialog's deadline
/// exists to reach the user's choice quickly; nobody is waiting on this one,
/// and a minute gives a thinly seeded swarm a fair chance to answer.
pub const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(60);

/// How often a running attempt checks it is still allowed to run.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Spawns the resolver loop, which runs until the app exits.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(RESOLVER_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            attempt_one(&app).await;
        }
    });
}

/// Makes one attempt, if a magnet is due and transfer is allowed.
///
/// One at a time, deliberately: the loop awaits each attempt before looking
/// for the next, so a long list of dead magnets costs one lookup at a time
/// rather than a burst.
async fn attempt_one(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(engine) = permitted_engine(&state).await else {
        return;
    };
    if !engine.peer_discovery_ready() {
        // An attempt on an empty routing table would spend its minute on the
        // bootstrap rather than the swarm -- the same reasoning that starts
        // the dialog's clock at readiness.
        return;
    }
    let Some((info_hash, uri)) = state.claim_due_magnet(Instant::now()).await else {
        return;
    };
    // A short prefix only. The log ships in diagnostics bundles, and a magnet
    // link names its torrent -- rule 11.
    let short = &info_hash[..info_hash.len().min(6)];
    log::info!("looking for the file list of pending magnet {short}");

    // A preview of the same torrent the add dialog is holding belongs to the
    // dialog. Its confirm would fail with "no longer pending" if this took it.
    let dialog_holds_it = engine.pending_metadata(&info_hash).await.is_some();

    let attempt = engine.preview_within(TorrentSource::Magnet { uri }, ATTEMPT_TIMEOUT);
    tokio::pin!(attempt);
    let mut watch = tokio::time::interval(WATCH_INTERVAL);
    // The first tick of an interval completes at once; spend it here so the
    // loop below watches from one interval in.
    watch.tick().await;

    let outcome = loop {
        tokio::select! {
            result = &mut attempt => break Some(result),
            _ = watch.tick() => {
                if !still_permitted(&state, &engine).await {
                    break None;
                }
            }
        }
    };

    let now = Instant::now();
    match outcome {
        None => {
            log::info!("stopped looking for pending magnet {short}: transfer is no longer allowed");
            state
                .finish_magnet_attempt(&info_hash, AttemptEnd::Interrupted, now)
                .await;
        }
        Some(Ok(preview)) => {
            let bytes = if dialog_holds_it {
                engine.pending_metadata(&preview.info_hash).await
            } else {
                engine.take_pending_metadata(&preview.info_hash).await
            };
            let saved = match bytes {
                Some(bytes) if preview.info_hash == info_hash => {
                    state
                        .resolve_pending_magnet(&info_hash, preview.name.clone(), &bytes)
                        .await
                }
                _ => false,
            };
            if saved {
                log::info!("pending magnet {short} has its file list");
                notify_ready(app, &preview.name);
            } else {
                state
                    .finish_magnet_attempt(&info_hash, AttemptEnd::Unanswered, now)
                    .await;
            }
        }
        Some(Err(err)) => {
            // The kind, never the message: librqbit's errors embed tracker
            // URLs, which name the torrent as surely as its title does.
            if !matches!(err, EngineError::MetadataTimeout { .. }) {
                log::info!(
                    "pending magnet {short}: attempt failed ({})",
                    crate::commands::CommandError::from(err).kind
                );
            }
            state
                .finish_magnet_attempt(&info_hash, AttemptEnd::Unanswered, now)
                .await;
        }
    }
}

/// The engine, if an attempt may start on it now.
async fn permitted_engine(state: &AppState) -> Option<Engine> {
    if state.exiting() {
        return None;
    }
    let engine = state.engine().await?;
    // The published status *and* a fresh probe, as the usage sender and the
    // update checker do: a drop since the guard's last tick is caught here
    // rather than a second later.
    if state.egress_refused_now().await {
        return None;
    }
    Some(engine)
}

/// Whether an attempt started on `engine` may keep running.
async fn still_permitted(state: &AppState, engine: &Engine) -> bool {
    permitted_engine(state)
        .await
        .is_some_and(|current| current.is_same_session(engine))
}

/// Tells the user a pending magnet is ready for them to choose files.
///
/// Failures are logged rather than propagated: notification permission may
/// be denied, and the row in the list says the same thing regardless.
fn notify_ready(app: &AppHandle, name: &str) {
    if let Err(err) = app
        .notification()
        .builder()
        .title("Ready to choose files")
        .body(name)
        .show()
    {
        log::warn!("could not show the file-list notification: {err}");
    }
}
