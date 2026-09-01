//! Application-wide state shared across Tauri commands.

use std::{path::PathBuf, sync::Arc, time::Instant};

use tokio::sync::{Mutex, RwLock};

use crate::{
    egress::{EgressReport, EgressWatcher},
    engine::{Engine, EngineError},
    settings::Settings,
    usage::{EventKind, Recorder},
};

/// Shared state managed by Tauri and injected into command handlers.
///
/// The engine starts asynchronously after the window opens, so commands must
/// tolerate it not being ready yet. That is why [`Self::engine`] is an
/// `Option`: showing an empty window immediately beats blocking startup on DHT
/// bootstrap.
pub struct AppState {
    engine: RwLock<Option<Engine>>,
    settings: RwLock<Settings>,
    /// Directory holding settings and session state. Not user-configurable.
    session_dir: PathBuf,
    /// Whether no settings file existed when the app started.
    ///
    /// Decided once, at startup, and never revised. The first-run screen
    /// writes settings as the user answers it, so a value re-read later would
    /// flip to false halfway through and take the screen with it.
    first_run: bool,
    /// Opt-in usage counts. Inert unless the user consented.
    usage: Arc<Recorder>,
    /// When this process started, for the session-length bucket.
    started_at: Instant,
    /// Cached egress probing.
    ///
    /// Behind a mutex rather than an `RwLock` because every read mutates the
    /// cache: the watcher only avoids a 3.2 ms interface enumeration by
    /// remembering what it saw last time. See [`EgressWatcher`].
    egress: Mutex<EgressWatcher>,
}

impl AppState {
    /// Creates state with the given settings and session directory.
    pub fn new(settings: Settings, session_dir: PathBuf, first_run: bool) -> Self {
        let usage = Arc::new(Recorder::new(
            session_dir.clone(),
            settings.usage_reporting,
            env!("CARGO_PKG_VERSION").to_owned(),
        ));
        Self {
            engine: RwLock::new(None),
            settings: RwLock::new(settings),
            session_dir,
            first_run,
            usage,
            started_at: Instant::now(),
            egress: Mutex::default(),
        }
    }

    /// Reads the current egress path and judges it against the user's pin.
    ///
    /// Cheap enough to call on every telemetry tick — around 62 µs once warm,
    /// against 3.3 ms for an uncached probe.
    pub async fn egress_report(&self) -> EgressReport {
        // Cloned so the settings lock is released before the egress lock is
        // taken; the two are never held together.
        let pinned = self.settings.read().await.egress_interface.clone();
        self.egress.lock().await.report(pinned.as_deref())
    }

    /// The usage recorder. Records nothing unless the user consented.
    pub fn usage(&self) -> &Arc<Recorder> {
        &self.usage
    }

    /// How long this process has been running.
    pub fn uptime(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Records an event, if the user consented.
    ///
    /// A convenience so command handlers stay one line longer rather than
    /// four; see [`crate::usage`] for why every field is an enum.
    pub fn note(&self, kind: EventKind) {
        self.usage.record(kind);
    }

    /// Whether this launch found no settings file.
    pub fn is_first_run(&self) -> bool {
        self.first_run
    }

    /// The directory holding settings and session state.
    pub fn session_dir(&self) -> &PathBuf {
        &self.session_dir
    }

    /// Installs the running engine, replacing any previous one.
    pub async fn set_engine(&self, engine: Engine) {
        *self.engine.write().await = Some(engine);
    }

    /// Returns a clone of the running engine, or `None` if it has not started.
    ///
    /// Cloning an [`Engine`] is cheap (it is an `Arc` internally) and means
    /// callers do not hold the lock while doing engine work.
    pub async fn engine(&self) -> Option<Engine> {
        self.engine.read().await.clone()
    }

    /// A copy of the current settings.
    pub async fn settings(&self) -> Settings {
        self.settings.read().await.clone()
    }

    /// Replaces the stored settings.
    pub async fn set_settings(&self, settings: Settings) {
        *self.settings.write().await = settings;
    }

    /// Stops the running engine and starts a fresh one from `settings`.
    ///
    /// Used when a setting the session is constructed with changes — the
    /// listen port, DHT, UPnP, or the download directory. The old session is
    /// shut down first so its fast-resume state is flushed and its port is
    /// released before the new one tries to bind it.
    ///
    /// # Errors
    ///
    /// Propagates [`EngineError`] if the new session fails to start. The old
    /// engine is already gone at that point, so the app is left with no engine
    /// rather than a stale one — commands then report `engineNotReady`, which
    /// is accurate.
    pub async fn restart_engine(&self, settings: &Settings) -> Result<(), EngineError> {
        // Take the old engine out before shutting it down, so nothing observes
        // a half-stopped session.
        if let Some(old) = self.engine.write().await.take() {
            old.shutdown().await;
        }

        let engine = Engine::start(settings.to_engine_config(self.session_dir.clone())).await?;
        engine.apply_limits(settings.download_limit_bps, settings.upload_limit_bps);
        *self.engine.write().await = Some(engine);
        Ok(())
    }

    /// Shuts the engine down if one is running, and clears it.
    pub async fn shutdown(&self) {
        if let Some(engine) = self.engine.write().await.take() {
            engine.shutdown().await;
        }
    }
}
