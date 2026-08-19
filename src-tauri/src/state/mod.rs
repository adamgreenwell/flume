//! Application-wide state shared across Tauri commands.

use tokio::sync::RwLock;

use crate::engine::Engine;

/// Shared state managed by Tauri and injected into command handlers.
///
/// The engine starts asynchronously after the window opens, so commands must
/// tolerate it not being ready yet. That is why [`Self::engine`] is an
/// `Option` rather than being constructed up front: showing an empty window
/// immediately beats blocking startup on DHT bootstrap.
#[derive(Default)]
pub struct AppState {
    engine: RwLock<Option<Engine>>,
}

impl AppState {
    /// Creates empty state with no engine yet running.
    pub fn new() -> Self {
        Self::default()
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

    /// Shuts the engine down if one is running, and clears it.
    pub async fn shutdown(&self) {
        if let Some(engine) = self.engine.write().await.take() {
            engine.shutdown().await;
        }
    }
}
