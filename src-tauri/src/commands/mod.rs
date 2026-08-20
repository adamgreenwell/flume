//! Tauri command handlers.
//!
//! Handlers in this module are intentionally thin: they unwrap shared state,
//! call into [`crate::engine`], and map errors into a serializable form. Any
//! logic worth testing belongs in the engine layer, which is testable without
//! Tauri.

use serde::Serialize;
use tauri::State;

use crate::{
    engine::{CoreStatus, EngineError, TelemetrySnapshot, TorrentPreview, TorrentSource},
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

impl From<EngineError> for CommandError {
    /// Maps engine failures onto stable identifiers the frontend can branch on.
    ///
    /// The `kind` is the contract; the message is for humans and may be
    /// reworded freely.
    fn from(err: EngineError) -> Self {
        let kind = match &err {
            EngineError::InvalidMagnet(_) => "invalidMagnet",
            EngineError::Metadata(_) => "metadata",
            EngineError::NoPendingPreview => "noPendingPreview",
            EngineError::UnknownTorrent(_) => "unknownTorrent",
            EngineError::Directory { .. } | EngineError::SessionStart(_) => "engineFailed",
            EngineError::Operation(_) => "operationFailed",
        };
        Self {
            kind,
            message: err.to_string(),
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

/// Resolves a torrent's metadata and file list without downloading anything.
///
/// For a magnet link this fetches metadata over the DHT and can take several
/// seconds. The result feeds the file-selection step; nothing is downloaded
/// until [`confirm_add`] is called.
///
/// # Errors
///
/// `invalidMagnet` for a malformed URI, `metadata` if the torrent cannot be
/// read or resolved, `engineNotReady` while starting.
#[tauri::command]
pub async fn preview_torrent(
    state: State<'_, AppState>,
    source: TorrentSource,
) -> Result<TorrentPreview, CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.preview(source).await?)
}

/// Starts a previewed torrent, downloading only the selected files.
///
/// `onlyFiles` holds indices into the preview's file list; `null` downloads
/// everything.
///
/// # Errors
///
/// `noPendingPreview` if the preview expired or was already consumed.
#[tauri::command]
pub async fn confirm_add(
    state: State<'_, AppState>,
    info_hash: String,
    only_files: Option<Vec<usize>>,
) -> Result<usize, CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.confirm_add(&info_hash, only_files).await?)
}

/// Releases a preview the user cancelled.
///
/// # Errors
///
/// `engineNotReady` while the engine is starting.
#[tauri::command]
pub async fn discard_preview(
    state: State<'_, AppState>,
    info_hash: String,
) -> Result<(), CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    engine.discard_preview(&info_hash).await;
    Ok(())
}

/// Pauses a torrent.
///
/// # Errors
///
/// `unknownTorrent` if no such torrent exists.
#[tauri::command]
pub async fn pause_torrent(state: State<'_, AppState>, id: usize) -> Result<(), CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.pause(id).await?)
}

/// Resumes a paused torrent.
///
/// # Errors
///
/// `unknownTorrent` if no such torrent exists.
#[tauri::command]
pub async fn resume_torrent(state: State<'_, AppState>, id: usize) -> Result<(), CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.resume(id).await?)
}

/// Removes a torrent, optionally deleting its files from disk.
///
/// `deleteFiles` is destructive and irreversible; the UI must confirm it
/// explicitly and must never default it to true.
///
/// # Errors
///
/// `unknownTorrent` if no such torrent exists.
#[tauri::command]
pub async fn remove_torrent(
    state: State<'_, AppState>,
    id: usize,
    delete_files: bool,
) -> Result<(), CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.remove(id, delete_files).await?)
}

/// Changes which files a torrent downloads.
///
/// # Errors
///
/// `unknownTorrent` if no such torrent exists.
#[tauri::command]
pub async fn set_only_files(
    state: State<'_, AppState>,
    id: usize,
    files: Vec<usize>,
) -> Result<(), CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.set_only_files(id, files).await?)
}
