//! Application-wide state shared across Tauri commands.

use std::path::PathBuf;

use tokio::sync::RwLock;

use crate::{
    engine::{Engine, EngineError},
    settings::Settings,
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
}

impl AppState {
    /// Creates state with the given settings and session directory.
    pub fn new(settings: Settings, session_dir: PathBuf, first_run: bool) -> Self {
        Self {
            engine: RwLock::new(None),
            settings: RwLock::new(settings),
            session_dir,
            first_run,
        }
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
