//! Application-wide state shared across Tauri commands.

use std::{path::PathBuf, sync::Arc, time::Instant};

use tokio::sync::{Mutex, RwLock};

use crate::{
    egress::{EgressGuard, EgressWatcher, Gate, GuardStatus, TransferGate},
    engine::{Engine, EngineError},
    library::{Library, Noted, persisted_info_hashes, write_atomically},
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
    /// The hysteresis between a verdict and acting on it.
    gate: Mutex<TransferGate>,
    /// What Flume remembers about each torrent that librqbit does not.
    ///
    /// Settings-shaped rather than Recorder-shaped: the roadmap puts a label
    /// (#58) and a per-torrent folder (#61) in this record, and silently
    /// losing a label someone just typed is not acceptable the way a missed
    /// usage count is. See [`crate::library`].
    library: RwLock<Library>,

    /// The last status the guard loop published.
    ///
    /// Read by `check_egress` rather than re-probing, so the UI and the engine
    /// loop can never disagree about what the routing table said.
    status: RwLock<GuardStatus>,
}

impl AppState {
    /// Creates state with the given settings and session directory.
    pub fn new(settings: Settings, session_dir: PathBuf, first_run: bool) -> Self {
        let usage = Arc::new(Recorder::new(
            session_dir.clone(),
            settings.usage_reporting,
            env!("CARGO_PKG_VERSION").to_owned(),
        ));

        // Loaded here rather than lazily: `restart_engine` reconciles against
        // it on the very first session construction, which happens before any
        // command runs.
        let (library, problem) = Library::load(&session_dir);
        if let Some(problem) = problem {
            log::warn!("{problem}");
        }

        // Probed once here, synchronously, so no command can ever observe a
        // status that does not exist yet. It costs one uncached probe -- about
        // 3.3 ms -- against a window in which `check_egress` would have to
        // answer "unknown" for reasons that have nothing to do with the
        // network.
        let mut watcher = EgressWatcher::default();
        let report = watcher.report(settings.egress_interface.as_deref());
        let status = GuardStatus {
            guard: settings.egress_guard,
            report,
            // Held until the first real tick says otherwise: a guard that
            // starts by reporting "not held" would be believed for a second.
            held: settings.egress_guard == EgressGuard::Hold,
            resumes_in_seconds: None,
        };
        Self {
            engine: RwLock::new(None),
            settings: RwLock::new(settings),
            session_dir,
            first_run,
            usage,
            started_at: Instant::now(),
            library: RwLock::new(library),
            egress: Mutex::new(watcher),
            gate: Mutex::new(TransferGate::default()),
            status: RwLock::new(status),
        }
    }

    /// The status the guard loop last published.
    ///
    /// Never probes. A command that probed independently would read the
    /// routing table at a different instant from the loop that acts on it, and
    /// the two would disagree on screen during exactly the transitions the
    /// user is watching.
    pub async fn egress_status(&self) -> GuardStatus {
        self.status.read().await.clone()
    }

    /// Probes, folds the verdict through the hysteresis gate, and publishes.
    ///
    /// The only place in Flume that probes. `now` is a parameter so the
    /// hysteresis is driven by the caller's clock rather than a hidden one.
    pub async fn observe_egress(&self, now: Instant) -> GuardStatus {
        // Both fields copied out so the settings lock is released before the
        // egress and gate locks are taken; they are never held together.
        let (guard, pinned) = {
            let settings = self.settings.read().await;
            (settings.egress_guard, settings.egress_interface.clone())
        };

        let report = self.egress.lock().await.report(pinned.as_deref());
        let permitted = report.verdict.allows_transfer();

        let (held, resumes_in_seconds) = if guard.holds_transfer() {
            let mut gate = self.gate.lock().await;
            let decision = gate.observe(permitted, now);
            let remaining = gate.settling_for(now).map(|left| left.as_secs());
            (decision == Gate::Held, remaining)
        } else {
            // Not holding. The gate is released rather than left to drift, so
            // that switching into Hold later starts from a clean state instead
            // of inheriting a settle window nobody was watching.
            self.gate.lock().await.release_now();
            (false, None)
        };

        let status = GuardStatus {
            guard,
            report,
            held,
            resumes_in_seconds,
        };
        *self.status.write().await = status.clone();
        status
    }

    /// Drops the settle window so a user-initiated change takes effect at once.
    ///
    /// Called when the guard mode or the pinned interface changes. Someone who
    /// has just retyped an interface name is watching for the result; making
    /// them wait out a window that started under the previous setting reads as
    /// the change not having worked.
    pub async fn release_egress_settle(&self) {
        self.gate.lock().await.release_now();
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
        // Reconciled *before* the engine is published. Every command gates on
        // `engine()`, so publishing first opens a window in which a
        // `remove_torrent` can delete a record between this pass reading
        // session.json and taking the library lock -- and reconciliation
        // would then put it straight back.
        //
        // Gated on the start having succeeded, and run here rather than at
        // startup because there is no single launch: a session is constructed
        // from the first guard tick, from every guard release, and from any
        // settings change that requires a restart.
        self.reconcile_library().await;

        *self.engine.write().await = Some(engine);
        Ok(())
    }

    /// Creates records for torrents that have none. Deletes nothing.
    ///
    /// Reads librqbit's *persisted* rows rather than the live session. A
    /// torrent that failed to restore -- an unmounted drive, a permissions
    /// problem -- is absent from the session and present in the file, and
    /// reconciling against the live reading would look like it had been
    /// removed. `None` means the file could not be read at all, which is not
    /// the same as no torrents and must not be treated as one.
    async fn reconcile_library(&self) {
        let Some(present) = persisted_info_hashes(&self.session_dir) else {
            log::warn!(
                "could not read librqbit's session file; \
                 leaving the library record alone this session"
            );
            return;
        };

        let json = {
            let mut library = self.library.write().await;
            if !library.reconcile(present) {
                return;
            }
            library.to_json()
        };
        self.persist_library(json);
    }

    /// Writes a serialised record, with no lock held.
    ///
    /// The write is blocking file I/O. Serialising happens under the lock --
    /// it is in-memory and fast -- and the write happens outside it, because
    /// holding a `tokio` write guard across a blocking syscall parks the
    /// runtime worker and blocks `added_times`, which takes a read lock on
    /// every telemetry tick. `Settings` reaches the same arrangement from the
    /// other direction: `update_settings` saves a local value before taking
    /// the lock at all.
    fn persist_library(&self, json: Option<String>) {
        let Some(json) = json else {
            return;
        };
        if let Err(err) = write_atomically(&self.session_dir, &json) {
            log::warn!("could not save the library record: {err}");
        }
    }

    /// Records a torrent's arrival, before it is added.
    ///
    /// Insert-if-absent, so a re-add keeps the original timestamp. Returns
    /// what it did, so a caller that then learns the add failed can withdraw
    /// a record this call invented — see [`Self::withdraw_torrent_added`].
    pub async fn note_torrent_added(&self, info_hash: &str) -> Noted {
        let (noted, json) = {
            let mut library = self.library.write().await;
            let noted = library.note_added(info_hash, crate::library::now_seconds());
            if !noted.changed() {
                return noted;
            }
            (noted, library.to_json())
        };
        self.persist_library(json);
        noted
    }

    /// Undoes a record written for an add that then failed.
    ///
    /// Only withdraws a record the matching [`Self::note_torrent_added`]
    /// *created*. Record-first is justified by the kill window inside
    /// librqbit's `add_torrent`, where the retained timestamp is right to
    /// within seconds — but a returned `Err` is different in kind. It is
    /// positive knowledge that the add did not happen, and leaving the record
    /// means the real add months later silently adopts a timestamp from the
    /// failed attempt, with no path that can ever correct it.
    ///
    /// This does not weaken the never-delete-on-absence rule: an absence is
    /// not knowledge, and this is.
    pub async fn withdraw_torrent_added(&self, info_hash: &str, noted: Noted) {
        if noted != Noted::Created {
            return;
        }
        let json = {
            let mut library = self.library.write().await;
            if !library.forget(info_hash) {
                return;
            }
            library.to_json()
        };
        self.persist_library(json);
    }

    /// Drops a record for a torrent that was definitely removed.
    pub async fn forget_torrent(&self, info_hash: &str) {
        let json = {
            let mut library = self.library.write().await;
            if !library.forget(info_hash) {
                return;
            }
            library.to_json()
        };
        self.persist_library(json);
    }

    /// When each torrent was added, keyed by info hash.
    pub async fn added_times(&self) -> std::collections::HashMap<String, u64> {
        let library = self.library.read().await;
        library.added_times()
    }

    /// Shuts the engine down if one is running, and clears it.
    pub async fn shutdown(&self) {
        if let Some(engine) = self.engine.write().await.take() {
            engine.shutdown().await;
        }
    }
}
