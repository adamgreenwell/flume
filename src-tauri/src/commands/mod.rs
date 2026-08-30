//! Tauri command handlers.
//!
//! Handlers in this module are intentionally thin: they unwrap shared state,
//! call into [`crate::engine`], and map errors into a serializable form. Any
//! logic worth testing belongs in the engine layer, which is testable without
//! Tauri.

use serde::Serialize;
use tauri::State;

use crate::{
    engine::{
        CoreStatus, DetectedClient, Engine, EngineError, ImportOutcome, TelemetrySnapshot,
        TorrentDetail, TorrentFileState, TorrentPreview, TorrentSource,
    },
    settings::{Settings, SettingsError},
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
    ///
    /// Public so the IPC contract test can assert its shape without standing
    /// up a Tauri application.
    pub fn not_ready() -> Self {
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
            // Distinct from `metadata`: nothing is wrong with the link, no
            // peer answered. The UI can suggest waiting for the DHT rather
            // than suggesting the input is malformed.
            EngineError::MetadataTimeout { .. } => "metadataTimeout",
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

impl From<SettingsError> for CommandError {
    fn from(err: SettingsError) -> Self {
        let kind = match &err {
            SettingsError::Save { .. } => "settingsSaveFailed",
            SettingsError::Invalid(_) => "settingsInvalid",
        };
        Self {
            kind,
            message: err.to_string(),
        }
    }
}

/// Whether this launch found no settings file.
///
/// The first-run screen shows on the strength of this. It is decided once at
/// startup and does not change while the app runs: the screen writes settings
/// as the user answers it, so a freshly-read value would flip halfway through
/// and take the screen away mid-question.
///
/// # Errors
///
/// Never fails; the `Result` keeps the signature uniform with the others.
#[tauri::command]
pub async fn is_first_run(state: State<'_, AppState>) -> Result<bool, CommandError> {
    Ok(state.is_first_run())
}

/// Lists other BitTorrent clients found on this machine.
///
/// Reads only their torrent stores and download directories. Categories and
/// seeding rules are deliberately not read: Flume has no model for either, so
/// there is nowhere to put them and claiming otherwise would be a lie the
/// first-run screen tells.
///
/// # Errors
///
/// Never fails; a machine with no other clients returns an empty list, as does
/// one whose home directory cannot be determined.
#[tauri::command]
pub async fn detect_clients() -> Result<Vec<DetectedClient>, CommandError> {
    Ok(Engine::detect_clients())
}

/// Takes over every torrent in another client's store.
///
/// Nothing is downloaded again: each torrent is added over its existing files
/// and librqbit verifies them in place, so anything the other client had
/// finished arrives complete.
///
/// # Errors
///
/// Returns [`CommandError`] only if the engine is unavailable. Individual
/// torrents that cannot be read are counted in the outcome rather than
/// aborting the import.
#[tauri::command]
pub async fn import_client(
    state: State<'_, AppState>,
    torrents_dir: String,
    output_folder: Option<String>,
) -> Result<ImportOutcome, CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine
        .import_from(std::path::Path::new(&torrents_dir), output_folder)
        .await?)
}

/// Returns the current user settings.
///
/// # Errors
///
/// Never fails in practice; the `Result` keeps the signature uniform with the
/// other commands.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, CommandError> {
    Ok(state.settings().await)
}

/// Validates, persists, and applies new settings.
///
/// Rate limits are applied to the running session immediately. Changing the
/// listen port, DHT, UPnP, or download directory restarts the session, because
/// those are fixed when it is constructed.
///
/// Settings are persisted *before* being applied, so a restart that fails
/// still leaves the user's choice recorded rather than silently reverting it.
///
/// # Errors
///
/// `settingsInvalid` if validation fails, `settingsSaveFailed` if the file
/// cannot be written, or `engineFailed` if the session will not restart.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, CommandError> {
    settings.validate()?;

    let previous = state.settings().await;
    settings.save(state.session_dir())?;
    state.set_settings(settings.clone()).await;

    if previous.requires_restart(&settings) {
        state.restart_engine(&settings).await?;
    } else if let Some(engine) = state.engine().await {
        engine.apply_limits(settings.download_limit_bps, settings.upload_limit_bps);
    }

    Ok(settings)
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

/// Lists a torrent's files with their progress and current selection.
///
/// # Errors
///
/// `unknownTorrent` if no such torrent exists, or `metadata` if the torrent's
/// file list has not resolved yet.
#[tauri::command]
pub async fn get_torrent_files(
    state: State<'_, AppState>,
    id: usize,
) -> Result<Vec<TorrentFileState>, CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.torrent_files(id)?)
}

/// Returns peers, trackers, and piece completion for one torrent.
///
/// Fetched on demand by the detail view rather than streamed in telemetry:
/// this data is only interesting while someone is looking at it, and pushing
/// it every second would grow the telemetry payload with the torrent count.
///
/// # Errors
///
/// `unknownTorrent` if no such torrent exists.
#[tauri::command]
pub async fn get_torrent_detail(
    state: State<'_, AppState>,
    id: usize,
) -> Result<TorrentDetail, CommandError> {
    let engine = state.engine().await.ok_or_else(CommandError::not_ready)?;
    Ok(engine.torrent_detail(id)?)
}
