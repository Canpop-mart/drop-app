//! The process manager: launch, monitor, and kill game processes.
//!
//! `process_manager.rs` used to be a single ~2300-line file. It is now split
//! into cohesive modules, each owning one phase of a game's lifecycle:
//!
//!   - [`mod@self`]  — the [`ProcessManager`] struct, handler selection,
//!     kill, the [`ProcessHandler`] trait, startup reconciliation.
//!   - [`launch`] / [`launch_emulator`] — the launch flow (steps 1–8) and
//!     the emulator-command / ENOEXEC-retry helpers.
//!   - [`exit`]  — process-exit handling, playtime stop, heartbeat loop.
//!   - [`kill`]  — process-tree termination (SIGTERM/SIGKILL, `taskkill /T`).
//!   - [`env`]   — launch-environment construction + hardening.
//!   - [`save_sync`] — pre-launch cloud-save synchronisation.
//!
//! ## Lifecycle invariants
//!
//!   - A launched game is in `self.processes` **iff** its transient status
//!     is `Running`. [`launch::ProcessManager::register_running_process`]
//!     sets both; [`exit`] clears both.
//!   - Exit detection is a blocking `wait()` on a dedicated thread per game
//!     — it cannot miss a real exit.
//!   - `self.processes` lives behind the global `PROCESS_MANAGER` mutex, so
//!     all table mutations are serialised; multiple games can run at once.
//!   - A crash drops the in-memory `Running` (the table and the transient
//!     map are process-local), so a game cannot get stuck `Running` across
//!     an app restart. [`ProcessManager::reconcile_running_processes`] is the
//!     belt-and-braces check for any transient `Running` that somehow
//!     survived into a fresh process.

mod env;
mod exit;
mod kill;
mod launch;
mod launch_emulator;
mod save_sync;

use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::Instant,
};

use database::{
    Database, DownloadableMetadata, GameVersion, borrow_db_checked, db::DATA_ROOT_DIR,
    platform::Platform,
};
use log::{debug, info, warn};
use serde::Serialize;
use shared_child::SharedChild;
use tauri::AppHandle;
use tokio::sync::Notify;

use crate::{
    error::ProcessError,
    process_handlers::{
        AsahiMuvmLauncher, MacLauncher, NativeLauncher, UMUCompatLauncher, UMUNativeLauncher,
        WindowsLauncher,
    },
};

/// A game process Drop is currently tracking.
pub struct RunningProcess {
    pub(crate) handle: Arc<SharedChild>,
    /// Monotonic clock at launch — used for session duration. Deliberately
    /// `Instant` rather than `SystemTime` so a clock change during the
    /// session (NTP correction, user editing the system clock) doesn't make
    /// `elapsed()` return `Err` and zero out the session's playtime.
    pub(crate) start: Instant,
    /// Set by [`ProcessManager::kill_game`] so the exit path can tell a
    /// user-requested kill apart from a crash.
    pub(crate) manually_killed: bool,
    pub(crate) playtime_session_id: Arc<std::sync::Mutex<Option<String>>>,
    /// Cancels the periodic playtime heartbeat task. Notified on exit.
    pub(crate) playtime_heartbeat_cancel: Arc<Notify>,
    pub(crate) achievement_poll_cancel: Option<Arc<Notify>>,
    /// Pre-launch save hashes — used to detect which saves changed.
    pub(crate) save_snapshot: Option<SaveSyncSnapshot>,
}

/// Snapshot of save state taken before game launch, used for post-exit sync.
pub struct SaveSyncSnapshot {
    /// RetroArch emulator root (`None` for PC-only games).
    pub emu_root: Option<PathBuf>,
    pub game_id: String,
    /// Game display name (needed for the Ludusavi re-scan on exit).
    pub game_name: Option<String>,
    pub pre_hashes: HashMap<String, String>,
    /// Map of filename → original disk path (for PC saves from Ludusavi).
    pub pc_save_paths: HashMap<String, PathBuf>,
}

/// One launchable configuration of a game, surfaced to the frontend so the
/// user can pick which to run.
#[derive(Serialize)]
pub struct LaunchOption {
    name: String,
}

/// Owns the table of running game processes and the registry of platform
/// launch handlers. A single instance lives behind the global
/// `PROCESS_MANAGER` mutex.
pub struct ProcessManager<'a> {
    current_platform: Platform,
    log_output_dir: PathBuf,
    pub(crate) processes: HashMap<String, RunningProcess>,
    game_launchers: Vec<(
        (Platform, Platform),
        &'a (dyn ProcessHandler + Sync + Send + 'static),
    )>,
    pub(crate) app_handle: AppHandle,
}

impl ProcessManager<'_> {
    pub fn new(app_handle: AppHandle) -> Self {
        ProcessManager {
            #[cfg(target_os = "windows")]
            current_platform: Platform::Windows,
            #[cfg(target_os = "macos")]
            current_platform: Platform::macOS,
            #[cfg(target_os = "linux")]
            current_platform: Platform::Linux,

            processes: HashMap::new(),
            log_output_dir: DATA_ROOT_DIR.join("logs"),
            game_launchers: vec![
                // (current platform, target platform) -> handler
                (
                    (Platform::Windows, Platform::Windows),
                    &WindowsLauncher {} as &(dyn ProcessHandler + Sync + Send + 'static),
                ),
                (
                    (Platform::Linux, Platform::Linux),
                    &NativeLauncher {} as &(dyn ProcessHandler + Sync + Send + 'static),
                ),
                (
                    (Platform::Linux, Platform::Linux),
                    &UMUNativeLauncher {} as &(dyn ProcessHandler + Sync + Send + 'static),
                ),
                (
                    (Platform::macOS, Platform::macOS),
                    &MacLauncher {} as &(dyn ProcessHandler + Sync + Send + 'static),
                ),
                (
                    (Platform::Linux, Platform::Windows),
                    &AsahiMuvmLauncher {} as &(dyn ProcessHandler + Sync + Send + 'static),
                ),
                (
                    (Platform::Linux, Platform::Windows),
                    &UMUCompatLauncher {} as &(dyn ProcessHandler + Sync + Send + 'static),
                ),
            ],
            app_handle,
        }
    }

    /// Kill a running game and its entire process tree.
    ///
    /// Fire-and-forget: signals are dispatched and the call returns
    /// immediately — the per-launch wait thread observes the real exit and
    /// runs [`exit`] cleanup, including the `Running -> Installed`
    /// transition. Blocking here would freeze the UI for the 10+ seconds a
    /// Proton/Wine teardown can take.
    pub fn kill_game(&mut self, game_id: String) -> Result<(), io::Error> {
        match self.processes.get_mut(&game_id) {
            Some(process) => {
                // Mark first so the exit path reports a kill, not a crash.
                process.manually_killed = true;
                kill::kill_process_tree(&process.handle, &game_id);
                Ok(())
            }
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Game ID not running",
            )),
        }
    }

    /// Whether a game process is currently tracked as running.
    pub fn is_game_running(&self, game_id: &str) -> bool {
        self.processes.contains_key(game_id)
    }

    /// Per-launch log directory for a game.
    pub fn get_log_dir(&self, game_id: &str) -> PathBuf {
        self.log_output_dir.join(game_id)
    }

    /// Reconcile the persisted status of any game still marked `Running`.
    ///
    /// The transient-status map is `#[serde(skip)]` and `self.processes` is
    /// process-local, so a normal crash already drops a game's `Running`
    /// state on the next launch. This is the defensive net: at startup,
    /// before the process table can hold anything, *any* transient `Running`
    /// is by definition stale (its process died with the previous app
    /// instance) and is cleared via the central state machine.
    ///
    /// Returns the number of stale `Running` statuses cleared.
    pub fn reconcile_running_processes(db: &mut Database) -> usize {
        use database::ApplicationTransientStatus;
        use games::status::{StatusKind, transition};

        // Collect the game ids whose transient status is `Running`.
        let stale: Vec<DownloadableMetadata> = db
            .applications
            .transient_statuses
            .iter()
            .filter(|(_, status)| matches!(status, ApplicationTransientStatus::Running {}))
            .map(|(meta, _)| meta.clone())
            .collect();

        for meta in &stale {
            warn!(
                "[EXIT] reconcile: game {} marked Running at startup but its process \
                 is dead — clearing to Installed",
                meta.id
            );
            // The persistent status underneath is the real state; log the
            // Running -> Installed correction through the state machine.
            transition(&meta.id, Some(StatusKind::Running), StatusKind::Installed);
            db.applications.transient_statuses.remove(meta);
        }

        if stale.is_empty() {
            info!("[EXIT] reconcile: no stale Running games at startup");
        } else {
            warn!(
                "[EXIT] reconcile: cleared {} stale Running game(s) at startup",
                stale.len()
            );
        }
        stale.len()
    }

    /// Select the launch handler for a `current -> target` platform pair.
    fn fetch_process_handler(
        &self,
        db_lock: &Database,
        target_platform: &Platform,
    ) -> Result<&(dyn ProcessHandler + Send + Sync), ProcessError> {
        info!(
            "[LAUNCH] selecting handler: current={:?}, target={:?}",
            self.current_platform, target_platform
        );
        let handler = self
            .game_launchers
            .iter()
            .find(|e| {
                let (e_current, e_target) = e.0;
                let platform_match =
                    e_current == self.current_platform && e_target == *target_platform;
                if platform_match {
                    let valid = e.1.valid_for_platform(db_lock, target_platform);
                    debug!(
                        "[LAUNCH]   handler ({e_current:?}->{e_target:?}) platform match, valid={valid}"
                    );
                    valid
                } else {
                    false
                }
            })
            .ok_or_else(|| {
                warn!(
                    "[LAUNCH] no valid handler for {:?}->{target_platform:?}",
                    self.current_platform
                );
                ProcessError::InvalidPlatform
            })?;
        info!(
            "[LAUNCH] selected handler for {:?}->{:?}",
            handler.0.0, handler.0.1
        );
        Ok(handler.1)
    }

    /// Whether the current platform can launch a game targeting `platform`.
    pub fn valid_platform(&self, platform: &Platform) -> bool {
        let db_lock = borrow_db_checked();
        self.fetch_process_handler(&db_lock, platform).is_ok()
    }

    /// List the launchable configurations for an installed game.
    pub fn get_launch_options(game_id: String) -> Result<Vec<LaunchOption>, ProcessError> {
        let db_lock = borrow_db_checked();

        let meta = db_lock
            .applications
            .installed_game_version
            .get(&game_id)
            .cloned()
            .ok_or(ProcessError::NotInstalled)?;

        let game_version = db_lock
            .applications
            .game_versions
            .get(&meta.version)
            .ok_or(ProcessError::InvalidVersion)?;

        Ok(game_version
            .launches
            .iter()
            .filter(|v| v.platform == meta.target_platform)
            .map(|v| LaunchOption {
                name: v.name.clone(),
            })
            .collect())
    }
}

/// A platform-specific strategy for turning a game's launch config into a
/// runnable command. Implementors live in [`crate::process_handlers`].
pub trait ProcessHandler: Send + 'static {
    /// Build the final launch command string for this platform/handler.
    fn create_launch_process(
        &self,
        meta: &DownloadableMetadata,
        launch_command: String,
        game_version: &GameVersion,
        current_dir: &str,
        database: &Database,
    ) -> Result<String, ProcessError>;

    /// Whether this handler can service a launch for `target` right now
    /// (e.g. UMU handlers check that umu-launcher is installed).
    fn valid_for_platform(&self, db: &Database, target: &Platform) -> bool;

    /// Apply handler-specific tweaks to the spawnable `Command` (e.g.
    /// `CREATE_NO_WINDOW` on Windows).
    fn modify_command(&self, command: &mut Command);
}
