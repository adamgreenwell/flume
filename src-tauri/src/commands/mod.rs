//! Tauri command handlers.
//!
//! Handlers in this module are intentionally thin: they unwrap shared state,
//! call into [`crate::engine`], and map errors into a serializable form. Any
//! logic worth testing belongs in the engine layer, which is testable without
//! Tauri.

use serde::Serialize;
use tauri::State;

use crate::{
    engine::{CoreStatus, TelemetrySnapshot},
    state::AppState,
};

/// An error returned across the IPC boundary.
///
/// Tauri requires command errors to be `Serialize`; `anyhow::Error` is not, so
/// engine errors are flattened into a message plus a stable machine-readable
/// `kind` the frontend can branch on without string matching.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// Stable identifier for the error class.
    pub kind: &'static str,
    /// Human-readable description, safe to show in the UI.
    pub message: String,
}

impl CommandError {
    /// The engine has not finished starting yet.
    fn not_ready() -> Self {
        Self {
            kind: "engineNotReady",
            message: "The torrent engine is still starting.".to_owned(),
        }
    }
}

/// Returns a snapshot of engine and DHT status.
///
/// # Errors
///
/// Returns [`CommandError`] with kind `engineNotReady` while the engine is
/// still starting up.
#[tauri::command]
pub async fn get_core_status(state: State<'_, AppState>) -> Result<CoreStatus, CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.core_status())
}

/// Returns the full telemetry snapshot: session status plus every torrent.
///
/// The UI receives this continuously as a pushed event; this command exists so
/// the first paint does not have to wait up to a full tick for one.
///
/// # Errors
///
/// Returns [`CommandError`] with kind `engineNotReady` while the engine is
/// still starting up.
#[tauri::command]
pub async fn get_telemetry(state: State<'_, AppState>) -> Result<TelemetrySnapshot, CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.telemetry())
}
