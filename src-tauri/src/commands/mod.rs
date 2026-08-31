//! Tauri command handlers.
//!
//! Handlers in this module are intentionally thin: they unwrap shared state,
//! call into [`crate::engine`], and map errors into a serializable form. Any
//! logic worth testing belongs in the engine layer, which is testable without
//! Tauri.

use serde::Serialize;
use tauri::{Manager, State};

use crate::{
    diagnostics::{self, LOG_TAIL_LINES},
    engine::{
        CoreStatus, DetectedClient, Engine, EngineError, ImportOutcome, TelemetrySnapshot,
        TorrentDetail, TorrentFileState, TorrentPreview, TorrentSource,
    },
    settings::{Settings, SettingsError},
    state::AppState,
    usage::{AddSource, CountBucket, EventKind, FailureKind, SettingKey},
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

impl AppState {
    /// Records a failed operation, if the user consented to usage reporting.
    ///
    /// Takes the already-mapped [`CommandError`] rather than the engine error,
    /// so the reported vocabulary is the same `kind` set the frontend branches
    /// on. Anything [`FailureKind::parse`] does not recognise is dropped.
    fn note_failure(&self, err: &CommandError) {
        if let Some(kind) = FailureKind::parse(err.kind) {
            self.note(EventKind::OperationFailed { kind });
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
    let outcome = engine
        .import_from(std::path::Path::new(&torrents_dir), output_folder)
        .await?;
    state.note(EventKind::LibraryImported {
        added: CountBucket::of(outcome.added),
    });
    Ok(outcome)
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

    // Applied before the changes are recorded, so withdrawing consent takes
    // effect *before* the event that would say consent changed. Recording a
    // withdrawal against the install id being withdrawn is not a nuance worth
    // getting wrong.
    if previous.usage_reporting != settings.usage_reporting {
        state.usage().set_consent(settings.usage_reporting);
    }
    for key in SettingKey::changed(&previous, &settings) {
        state.note(EventKind::SettingChanged { key });
    }

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
    let route = match &source {
        TorrentSource::Magnet { .. } => AddSource::Magnet,
        TorrentSource::File { .. } => AddSource::File,
    };

    match engine.preview(source).await.map_err(CommandError::from) {
        Ok(preview) => {
            state.note(EventKind::TorrentPreviewed { source: route });
            Ok(preview)
        }
        Err(err) => {
            state.note_failure(&err);
            Err(err)
        }
    }
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
    match engine
        .confirm_add(&info_hash, only_files)
        .await
        .map_err(CommandError::from)
    {
        Ok(id) => {
            state.note(EventKind::TorrentAdded);
            Ok(id)
        }
        Err(err) => {
            state.note_failure(&err);
            Err(err)
        }
    }
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
    match engine
        .remove(id, delete_files)
        .await
        .map_err(CommandError::from)
    {
        Ok(()) => {
            state.note(EventKind::TorrentRemoved {
                deleted_data: delete_files,
            });
            Ok(())
        }
        Err(err) => {
            state.note_failure(&err);
            Err(err)
        }
    }
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

/// Builds a redacted diagnostics bundle for the user to paste into an issue.
///
/// Nothing is sent anywhere: this returns the text, and the UI shows it before
/// offering to copy it. Everything that identifies the user or what they are
/// downloading is removed first — see [`crate::diagnostics`] for what that
/// covers and, importantly, what it cannot.
///
/// # Errors
///
/// Never fails. A missing log directory, an unreadable log file or an engine
/// that has not started each become a line in the bundle saying so, which is
/// itself diagnostic — an error here would just deny the user the report.
#[tauri::command]
pub async fn get_diagnostics(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    let settings = state.settings().await;
    let engine = state.engine().await;

    // Only the names, and only to redact them. They are never rendered.
    let (core, names) = match &engine {
        Some(engine) => {
            let snapshot = engine.telemetry();
            let names = snapshot
                .torrents
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>();
            (Some(snapshot.core), names)
        }
        None => (None, Vec::new()),
    };

    let home = directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
    let log_tail = app
        .path()
        .app_log_dir()
        .ok()
        .map(|dir| read_log_tail(&dir, LOG_TAIL_LINES))
        .unwrap_or_default();

    let redactor = diagnostics::Redactor::new(home.as_deref(), &settings.download_dir, &names);

    Ok(diagnostics::Report {
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        debug_build: cfg!(debug_assertions),
        settings: &settings,
        core: core.as_ref(),
        torrent_count: names.len(),
        home,
        log_tail: &log_tail,
        redactor: &redactor,
    }
    .render())
}

/// Reads the last `lines` lines of the most recently modified log file.
///
/// The newest file rather than a fixed name: `tauri-plugin-log` rotates, so
/// the interesting one after a long session is not the one it started with.
fn read_log_tail(dir: &std::path::Path, lines: usize) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let newest = entries
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified);

    let Some((_, path)) = newest else {
        return Vec::new();
    };
    let Ok(body) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };

    let all: Vec<&str> = body.lines().collect();
    all[all.len().saturating_sub(lines)..]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}
