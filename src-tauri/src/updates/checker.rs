//! Drives the updater plugin: the daily check, a check on request, and an
//! install on a click.
//!
//! Thin on purpose. Whether a check may run, which installs replace in place
//! and what the frontend is told are decided in the parent module, which has
//! no Tauri types and is tested there. This file asks it, calls the plugin,
//! and publishes the answer.
//!
//! All of it runs in Rust (architecture rule 10). The plugin also registers
//! JavaScript commands, but the capability file grants none of them: the
//! webview cannot reach the network through the updater, only ask Rust to.

use std::{future::Future, time::Duration};

use rand::RngExt;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterExt};
use tokio::sync::{Mutex, RwLock};

use super::{
    Bundle, CHECK_INTERVAL, CHECK_TIMEOUT, Delivery, FailureKind, FailureStage, STARTUP_DELAY,
    Skip, Trigger, UpdateStatus, after_hold, delivery_for, is_configured, may_check,
};
use crate::state::AppState;

/// Event carrying an [`UpdateStatus`] to the frontend.
///
/// Must match `UPDATE_EVENT` in `src/hooks/useUpdateStatus.ts`.
pub const UPDATE_EVENT: &str = "flume://update";

/// What Flume knows about updates, held for the life of the app.
pub struct Updates {
    configured: bool,
    status: RwLock<UpdateStatus>,
    /// The update the last check found, kept so a click installs exactly
    /// what was announced rather than whatever a second check returns.
    pending: Mutex<Option<Update>>,
    /// One check or install at a time. A click landing during the daily
    /// check gets the status that check publishes rather than racing it.
    busy: Mutex<()>,
}

impl Updates {
    /// State for a build that does or does not carry an updater key.
    #[must_use]
    pub fn new(configured: bool) -> Self {
        Self {
            configured,
            status: RwLock::new(if configured {
                UpdateStatus::Idle
            } else {
                UpdateStatus::Unconfigured
            }),
            pending: Mutex::new(None),
            busy: Mutex::new(()),
        }
    }

    /// Whether this build can verify updates.
    #[must_use]
    pub const fn configured(&self) -> bool {
        self.configured
    }

    /// The current status.
    pub async fn status(&self) -> UpdateStatus {
        self.status.read().await.clone()
    }
}

/// Whether the running build's config carries an updater public key.
#[must_use]
pub fn build_is_configured<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(is_configured)
}

/// How this copy of Flume was installed, as the bundler recorded it.
fn current_bundle() -> Bundle {
    use tauri::utils::{config::BundleType, platform::bundle_type};
    match bundle_type() {
        Some(BundleType::App | BundleType::Dmg) => Bundle::App,
        Some(BundleType::AppImage) => Bundle::AppImage,
        Some(BundleType::Deb) => Bundle::Deb,
        Some(BundleType::Rpm) => Bundle::Rpm,
        Some(BundleType::Msi) => Bundle::Msi,
        Some(BundleType::Nsis) => Bundle::Nsis,
        _ => Bundle::Unknown,
    }
}

/// The delivery this install gets.
#[must_use]
pub fn current_delivery() -> Delivery {
    delivery_for(current_bundle())
}

/// Maps the plugin's errors onto the closed list the frontend renders.
///
/// The messages are never passed on: they carry URLs and file paths.
fn classify(err: &UpdaterError) -> FailureKind {
    match err {
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) | UpdaterError::Http(_) => {
            FailureKind::Network
        }
        UpdaterError::Minisign(_)
        | UpdaterError::SignedVersionMismatch { .. }
        | UpdaterError::MissingSignedVersion => FailureKind::Signature,
        UpdaterError::ReleaseNotFound
        | UpdaterError::Serialization(_)
        | UpdaterError::Semver(_)
        | UpdaterError::UrlParse(_)
        | UpdaterError::TargetNotFound(_)
        | UpdaterError::TargetsNotFound(_)
        | UpdaterError::InvalidUpdaterFormat => FailureKind::Manifest,
        _ => FailureKind::Install,
    }
}

/// How often an in-flight request re-reads the guard. Its own tick rate: no
/// point looking more often than the status can change.
const GUARD_POLL: Duration = Duration::from_secs(1);

/// How often a check that was skipped for a held guard looks for its release.
const HELD_RETRY_POLL: Duration = Duration::from_secs(15);

/// Resolves once the egress guard is holding.
async fn guard_holds<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    while !state.egress_status().await.held {
        tokio::time::sleep(GUARD_POLL).await;
    }
}

/// Runs a network operation, abandoning it the moment the guard holds.
///
/// Checking the guard once before starting was not enough: a tunnel that
/// drops mid-download would leave the transfer running over the address the
/// guard protects, for as long as the download takes. Dropping the plugin's
/// future drops its request, which ends the transfer. `None` means the guard
/// cut it off.
async fn unless_held<R: Runtime, T>(
    app: &AppHandle<R>,
    operation: impl Future<Output = T>,
) -> Option<T> {
    super::abandon_when(operation, guard_holds(app)).await
}

/// Resolves once the guard has stopped holding -- never sooner than one poll.
///
/// The sleep comes first on purpose. The retry loop calls this whenever a
/// check comes back `Held`, and a `Held` can be stale: a check skipped because
/// another one is running returns the stored status. Checking before sleeping
/// would return at once, re-check, get the same answer and spin. Sleeping
/// first bounds the loop to one attempt per poll whatever the status says.
async fn guard_released<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    loop {
        tokio::time::sleep(HELD_RETRY_POLL).await;
        if !state.egress_status().await.held {
            return;
        }
    }
}

/// Stores a status and tells the frontend.
async fn publish<R: Runtime>(app: &AppHandle<R>, updates: &Updates, status: UpdateStatus) {
    status.clone_into(&mut *updates.status.write().await);
    if let Err(err) = app.emit(UPDATE_EVENT, &status) {
        log::warn!("could not deliver the update status: {err}");
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Checks for a newer version, if the rules allow it now, and publishes the
/// result.
///
/// Returns the status after the attempt. A check that is not allowed to run
/// returns the status unchanged, except under a held guard, which is
/// published as [`UpdateStatus::Held`] so a manual check says why nothing
/// happened.
pub async fn check<R: Runtime>(app: &AppHandle<R>, trigger: Trigger) -> UpdateStatus {
    let updates = app.state::<Updates>();
    let state = app.state::<AppState>();
    let enabled = state.settings().await.check_for_updates;
    let held = state.egress_status().await.held;

    let before = updates.status().await;
    match may_check(trigger, updates.configured, enabled, held) {
        Ok(()) => {}
        Err(Skip::Held) => {
            let status = after_hold(&before);
            publish(app, &updates, status.clone()).await;
            return status;
        }
        Err(Skip::Off | Skip::Unconfigured) => {
            // A `Held` left over from before the switch went off is no longer
            // owed a retry; leaving it would keep the scheduled loop waiting
            // on a check it is not allowed to make.
            let status = updates.status().await;
            if status == UpdateStatus::Held {
                publish(app, &updates, UpdateStatus::Idle).await;
                return UpdateStatus::Idle;
            }
            return status;
        }
    }

    let Ok(_busy) = updates.busy.try_lock() else {
        return updates.status().await;
    };

    publish(app, &updates, UpdateStatus::Checking).await;
    let found = unless_held(app, async {
        match app.updater_builder().timeout(CHECK_TIMEOUT).build() {
            Ok(updater) => updater.check().await,
            Err(err) => Err(err),
        }
    })
    .await;
    let Some(found) = found else {
        let status = after_hold(&before);
        publish(app, &updates, status.clone()).await;
        return status;
    };

    let status = match found {
        Ok(Some(update)) => {
            let version = update.version.clone();
            *updates.pending.lock().await = Some(update);
            UpdateStatus::Available {
                version,
                delivery: current_delivery(),
            }
        }
        Ok(None) => {
            *updates.pending.lock().await = None;
            UpdateStatus::UpToDate {
                checked_at: now_secs(),
            }
        }
        Err(err) => {
            // Local log only. The message can name the endpoint, which is
            // public, but nothing here is reported anywhere.
            log::info!("update check failed: {err}");
            UpdateStatus::Failed {
                reason: classify(&err),
                stage: FailureStage::Check,
            }
        }
    };
    publish(app, &updates, status.clone()).await;
    status
}

/// Downloads, verifies and installs the announced update, then restarts.
///
/// Only for installs that update in place; anything else is refused here
/// even though the frontend never offers it, because the button is not the
/// only thing that can call a command. Refused while the guard holds, for
/// the same reason a check is: it is a connection from the protected address.
///
/// The download is raced against the guard and the install is not. The
/// download is the only network traffic, and cutting it off loses nothing;
/// the install writes the app bundle, and stopping that halfway would leave
/// a broken one. `install` verifies the signature before it writes anything.
///
/// Restarts with `request_restart` rather than `restart`, so the normal exit
/// path runs -- seeding time is persisted and queued usage counts are flushed
/// exactly as on a quit.
pub async fn install<R: Runtime>(app: &AppHandle<R>) -> UpdateStatus {
    let updates = app.state::<Updates>();
    if current_delivery() != Delivery::InPlace {
        return updates.status().await;
    }
    let Ok(_busy) = updates.busy.try_lock() else {
        return updates.status().await;
    };
    let Some(update) = updates.pending.lock().await.clone() else {
        return updates.status().await;
    };
    // What to put back if the guard stops this: the announcement, so the
    // person can install once the tunnel returns. Not `Held`, which would
    // drop the banner and leave the pending update unreachable.
    let announced = UpdateStatus::Available {
        version: update.version.clone(),
        delivery: Delivery::InPlace,
    };
    if app.state::<AppState>().egress_status().await.held {
        publish(app, &updates, announced.clone()).await;
        return announced;
    }

    publish(app, &updates, UpdateStatus::Installing).await;
    let Some(downloaded) = unless_held(app, update.download(|_, _| {}, || {})).await else {
        log::info!("update download abandoned: the egress guard started holding");
        publish(app, &updates, announced.clone()).await;
        return announced;
    };
    match downloaded.and_then(|bytes| update.install(bytes)) {
        Ok(()) => {
            log::info!("installed Flume {}; restarting", update.version);
            app.request_restart();
            UpdateStatus::Installing
        }
        Err(err) => {
            log::warn!("update install failed: {err}");
            let status = UpdateStatus::Failed {
                reason: classify(&err),
                stage: FailureStage::Install,
            };
            publish(app, &updates, status.clone()).await;
            status
        }
    }
}

/// Starts the daily check.
///
/// A build with no updater key never starts the loop at all.
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    if !app.state::<Updates>().configured() {
        log::debug!("this build has no updater key; update checks are off");
        return;
    }

    tauri::async_runtime::spawn(async move {
        let delay = rand::rng().random_range(STARTUP_DELAY);
        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;

        let mut ticker = tokio::time::interval(CHECK_INTERVAL);
        // A machine that slept through a check makes one on waking, not a
        // burst of catch-up requests.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            // A check the guard skipped is owed, not cancelled: the banner
            // promises one once traffic is protected again, and a tunnel that
            // was down at launch or at the daily tick would otherwise cost the
            // whole day -- often the whole session.
            while super::retry_when_released(&check(&app, Trigger::Scheduled).await) {
                guard_released(&app).await;
            }
        }
    });
}
