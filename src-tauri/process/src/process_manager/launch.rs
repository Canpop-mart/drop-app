//! Game-launch orchestration: the front half of a game's lifecycle.
//!
//! [`ProcessManager::launch_process`] (and its streaming sibling) walk a
//! fixed sequence of numbered steps, each emitting a `launch_trace` event so
//! a black-screen / failed-launch report can be reconstructed from the log
//! alone:
//!
//!   1. resolve game metadata + the persistent install status,
//!   2. pick the launch / setup config for the target platform,
//!   3. select a [`ProcessHandler`] (native, Windows, UMU/Proton, …),
//!   4. build the launch command (direct, or via an emulator),
//!   5. format it through the user's launch template,
//!   6. parse the final command into argv,
//!   7. configure RetroArch + run pre-launch cloud-save sync,
//!   8. spawn the process and register it in the running set.
//!
//! ## Injection safety
//!
//! The launch command originates from a server-provided config and a
//! user-editable template, so it is never handed to a shell as a string.
//! Every stage round-trips through [`ParsedCommand`], which tokenises with
//! `shell_words` and re-quotes on `reconstruct()`; the final spawn uses
//! `Command::new(exe).args(argv)` (argv form — no shell). The one shell that
//! *is* used — the Linux `/bin/bash -c` wrapper for shebang scripts — builds
//! its command string with `shell_words::quote` on every component, so a
//! game path containing `; rm -rf` is passed as one literal argument.
//!
//! Env vars from the launch config are filtered through
//! [`super::env::is_env_key_forbidden`] so a remote config cannot smuggle in
//! `LD_PRELOAD` and friends.

use std::{
    fs::{OpenOptions, create_dir_all},
    path::PathBuf,
    process::Command,
    sync::Arc,
    thread::spawn,
    time::Instant,
};

use database::{
    ApplicationTransientStatus, DownloadType, DownloadableMetadata, GameDownloadStatus,
    borrow_db_checked, borrow_db_mut_checked, models::data::InstalledGameType, platform::Platform,
};
use dynfmt::{Format, SimpleCurlyFormat};
use games::{
    library::push_game_update,
    status::{StatusKind, transition_from_db},
};
use log::{info, warn};
use shared_child::SharedChild;
use tauri::Emitter as _;
use tokio::sync::Notify;

use crate::{
    PROCESS_MANAGER,
    error::ProcessError,
    format::DropFormatArgs,
    parser::{LaunchParameters, ParsedCommand},
    process_manager::{
        ProcessManager, RunningProcess, env, exit, save_sync as save_sync_mod,
    },
};

/// Everything the pre-launch cloud-save sync needs, captured while the
/// PROCESS_MANAGER lock is held so the sync itself can run without it.
struct SaveSyncRequest {
    game_id: String,
    target_platform: Platform,
    /// The emulator's install dir, for an emulator launch. Its presence is
    /// what selects the emulator discovery strategy over Ludusavi.
    effective_cwd: Option<String>,
    emulator_rom_path: Option<String>,
    streaming: bool,
}

/// A launch that is fully resolved and built but not yet spawned.
///
/// Produced under the PROCESS_MANAGER lock by `prepare_launch` and consumed by
/// `finish_launch`, with the cloud-save sync running in between —
/// deliberately outside the lock.
struct PreparedLaunch {
    spawn_plan: SpawnPlan,
    meta: DownloadableMetadata,
    game_id: String,
    log_path: PathBuf,
    error_log_path: PathBuf,
    incognito: bool,
    sync: SaveSyncRequest,
}

/// Holds the `pending_launches` reservation for a launch that is between
/// `prepare_launch` and `finish_launch`, and releases it if that stretch never
/// finishes.
///
/// The stretch is the save sync, and it can unwind. `block_with_timeout` calls
/// `tauri::async_runtime::block_on`, and both `launch_game_streaming` and
/// `start_compat_test` reach `run_launch` from inside `spawn_blocking` — the
/// nested-runtime case that panics. Before the lock split a panic there left
/// the manager clean; without this guard it strands the game id in
/// `pending_launches` for the rest of the app's life, and every later Play
/// press answers `AlreadyRunning` for a game that is not running.
struct LaunchReservation {
    /// `None` once `disarm` has handed ownership of the removal to
    /// `finish_launch`.
    game_id: Option<String>,
}

impl LaunchReservation {
    fn new(game_id: String) -> Self {
        Self {
            game_id: Some(game_id),
        }
    }

    /// Give up the reservation because `finish_launch` is about to remove it
    /// itself. Called with the PROCESS_MANAGER lock already held, which is why
    /// it must consume the guard: a `Drop` running at that point would
    /// deadlock trying to re-take the lock.
    fn disarm(mut self) {
        self.game_id = None;
    }
}

impl Drop for LaunchReservation {
    fn drop(&mut self) {
        let Some(game_id) = self.game_id.take() else {
            return;
        };
        warn!("[LAUNCH] {game_id}: launch unwound before spawn, clearing its reservation");
        PROCESS_MANAGER.lock().pending_launches.remove(&game_id);
    }
}

/// Launch a game end to end: prepare under the lock, sync saves without it,
/// then spawn and register under the lock again.
///
/// The lock split is the point. `launch_process` used to run the whole flow
/// inside one `PROCESS_MANAGER.lock()`, including a save-conflict dialog that
/// blocked on the user for up to five minutes. For that whole window
/// `kill_game`, `is_game_running`, the installed-version list and every other
/// game's save upload were stuck behind a modal nobody had noticed.
pub fn run_launch(
    game_id: String,
    launch_process_index: usize,
    streaming: bool,
    config_override: Option<database::models::data::UserConfiguration>,
    incognito: bool,
    version_id: Option<String>,
) -> Result<(), ProcessError> {
    let (prepared, app_handle) = {
        let mut manager = PROCESS_MANAGER.lock();
        let prepared = manager.prepare_launch(
            game_id,
            launch_process_index,
            streaming,
            config_override,
            incognito,
            version_id,
        )?;
        (prepared, manager.app_handle.clone())
    };

    let reservation = LaunchReservation::new(prepared.game_id.clone());

    let save_snapshot = pre_launch_save_sync(&app_handle, &prepared.sync);

    // Take the lock before disarming so the reservation is never released by a
    // `Drop` that would have to re-enter the lock this thread already holds.
    let mut manager = PROCESS_MANAGER.lock();
    reservation.disarm();
    manager.finish_launch(prepared, save_snapshot)
}

impl ProcessManager<'_> {
    /// Resolve, build and reserve a launch, stopping just short of spawning.
    ///
    /// `incognito` suppresses every server-side side effect of the session:
    /// no `playSession` row, no playtime increment, no achievement-poll
    /// sync, no presence broadcast. The game still launches normally and
    /// the local Running status flips so the UI still tracks the process —
    /// the difference is purely in what reaches the server.
    ///
    /// When `streaming` is true, save-sync conflicts are auto-resolved to
    /// `keep_local` instead of showing a UI dialog (which would appear on the
    /// remote host PC where the user can't interact with it). If
    /// `config_override` is provided it temporarily replaces the game's local
    /// `user_configuration` so the receiver's settings (widescreen, quality, …)
    /// are applied on the host. Streaming never goes incognito — the receiver
    /// expects credit for their play time.
    fn prepare_launch(
        &mut self,
        game_id: String,
        launch_process_index: usize,
        streaming: bool,
        config_override: Option<database::models::data::UserConfiguration>,
        incognito: bool,
        // Which installed version to launch. `None` = the game's current single
        // install (unchanged legacy behaviour); `Some` selects a specific
        // install from the multi-version map (the frontend's per-version launch).
        version_id: Option<String>,
    ) -> Result<PreparedLaunch, ProcessError> {
        if self.processes.contains_key(&game_id) || self.pending_launches.contains(&game_id) {
            return Err(ProcessError::AlreadyRunning);
        }

        // The launch flow only *reads* the database — every status write
        // happens later in its own scoped `borrow_db_mut_checked()`. An
        // immutable borrow here keeps launch from blocking concurrent reads.
        let db_lock = borrow_db_checked();

        // Resolve which install to launch. An explicit version_id (the
        // frontend's per-version launch) selects that install from the
        // multi-version map; without one we launch the game's current single
        // install exactly as before, so existing callers are unchanged.
        let (meta, version_name, install_dir, install_type): (
            DownloadableMetadata,
            &String,
            &String,
            &InstalledGameType,
        ) = if let Some(v) = &version_id {
            let install = db_lock
                .applications
                .get_install(&game_id, v)
                .ok_or(ProcessError::NotInstalled)?;
            if matches!(
                install.install_type,
                InstalledGameType::PartiallyInstalled { .. }
            ) {
                return Err(ProcessError::NotInstalled);
            }
            let meta = DownloadableMetadata::new(
                game_id.clone(),
                install.version_id.clone(),
                install.target_platform.clone(),
                DownloadType::Game,
            );
            (
                meta,
                &install.version_id,
                &install.install_dir,
                &install.install_type,
            )
        } else {
            let meta = db_lock
                .applications
                .installed_game_version
                .get(&game_id)
                .cloned()
                .ok_or(ProcessError::NotInstalled)?;
            let (version_name, install_dir, install_type) =
                match db_lock.applications.game_statuses.get(&game_id) {
                    Some(GameDownloadStatus::Installed {
                        version_id: version_name,
                        install_dir,
                        install_type:
                            install_type @ (InstalledGameType::Installed
                            | InstalledGameType::SetupRequired),
                        ..
                    }) => (version_name, install_dir, install_type),
                    _ => return Err(ProcessError::NotInstalled),
                };
            (meta, version_name, install_dir, install_type)
        };

        let game_version = db_lock
            .applications
            .game_versions
            .get(version_name)
            .ok_or(ProcessError::InvalidVersion)?;

        let game_log_folder = self.get_log_dir(&game_id);
        create_dir_all(&game_log_folder)?;

        let current_time = chrono::offset::Local::now();
        let log_path = game_log_folder
            .join(format!("{}-{}.log", &meta.version, current_time.timestamp()));
        let error_log_path = game_log_folder.join(format!(
            "{}-{}-error.log",
            &meta.version,
            current_time.timestamp()
        ));

        let log_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .read(true)
            .create(true)
            .open(&log_path)?;
        let error_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .read(true)
            .create(true)
            .open(&error_log_path)?;

        let target_platform = meta.target_platform;

        // ── STEP 1: Game metadata ──────────────────────────────────────────
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "1_metadata",
            "game_id": &game_id,
            "target_platform": format!("{:?}", target_platform),
            "version_id": version_name,
            "install_dir": install_dir,
            "install_type": format!("{:?}", install_type),
            "launch_template": &game_version.user_configuration.launch_template,
            "override_proton_path": &game_version.user_configuration.override_proton_path,
        }));
        info!(
            "[LAUNCH] game {game_id} — target_platform={target_platform:?}, \
             version={version_name:?}, install_dir={install_dir:?}, streaming={streaming}"
        );

        // Set to true when the NeedsCompat fallback fires — we correct the
        // stored platform metadata after the database lock is released.
        let mut needs_platform_correction = false;

        // ── STEP 2: Select launch config ───────────────────────────────────
        let (target_command, emulator, disc_paths) = match install_type {
            InstalledGameType::Installed => {
                let (_, launch_config) = game_version
                    .launches
                    .iter()
                    .filter(|v| v.platform == target_platform)
                    .enumerate()
                    .find(|(i, _)| *i == launch_process_index)
                    .ok_or(ProcessError::NotInstalled)?;
                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                    "step": "2_launch_config_selected",
                    "game_id": &game_id,
                    "command": &launch_config.command,
                    "has_emulator": launch_config.emulator.is_some(),
                }));
                (
                    launch_config.command.clone(),
                    launch_config.emulator.as_ref(),
                    launch_config.disc_paths.clone(),
                )
            }
            InstalledGameType::SetupRequired => {
                let setup_config = game_version
                    .setups
                    .iter()
                    .find(|v| v.platform == target_platform)
                    .ok_or(ProcessError::NotInstalled)?;
                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                    "step": "2_setup_config",
                    "game_id": &game_id,
                    "command": &setup_config.command,
                }));
                (setup_config.command.clone(), None, Vec::new())
            }
            _ => unreachable!("game registered as PartiallyInstalled cannot launch"),
        };

        let mut target_command = ParsedCommand::parse(target_command)?;

        // ── STEP 2b: Mod launch override ───────────────────────────────────
        // If an installed mod (e.g. SMAPI) declares a launch override, swap the
        // game's executable for it. The override is relative to install_dir, the
        // same as the base command, so it absolutises identically below. Args
        // are preserved. Emulator launches are left alone (the ROM path must not
        // be replaced). When the mod is uninstalled its ledger is gone, so this
        // returns None and the game launches normally again.
        if emulator.is_none()
            && let Some(override_exe) = games::downloads::mod_data::find_launch_override(
                std::path::Path::new(install_dir),
            )
        {
            info!("[LAUNCH] mod launch override active for {game_id}: {override_exe}");
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "2b_mod_launch_override",
                "game_id": &game_id,
                "override": &override_exe,
            }));
            target_command.command = override_exe;
        }

        // ── STEP 3: Handler selection ──────────────────────────────────────
        // For an emulator launch the handler must target the *emulator's*
        // platform, not the ROM's.
        let handler_target_platform = emulator
            .and_then(|e| db_lock.applications.installed_game_version.get(&e.game_id))
            .map(|m| m.target_platform)
            .unwrap_or(target_platform);
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "3_handler_selection",
            "game_id": &game_id,
            "handler_target_platform": format!("{:?}", handler_target_platform),
            "current_platform": format!("{:?}", self.current_platform),
        }));
        let process_handler = self.fetch_process_handler(&db_lock, &handler_target_platform)?;

        // For emulator launches the working dir must be the emulator's
        // install dir so relative paths in its command resolve.
        let mut effective_cwd: Option<String> = None;
        let mut emulator_rom_path: Option<String> = None;

        // ── STEP 4: Build launch command ───────────────────────────────────
        let target_launch_string = if let Some(emulator) = emulator {
            // `database` does not re-export `LaunchConfigurationEmulator`, so
            // hand the helper a borrowed triple instead of the struct.
            let emulator_ref = crate::process_manager::launch_emulator::EmulatorRef {
                launch_id: &emulator.launch_id,
                game_id: &emulator.game_id,
                version_id: &emulator.version_id,
            };
            self.build_emulator_command(
                &db_lock,
                process_handler,
                &game_id,
                emulator_ref,
                &mut target_command,
                install_dir,
                &disc_paths,
                &mut effective_cwd,
                &mut emulator_rom_path,
            )?
        } else {
            // Resolve the executable against the install dir *before* wrapping
            // it in umu-run. A relative path (e.g. "FarFarWest/FarFarWest.exe")
            // would otherwise resolve against umu-run's own cwd, so Proton boots
            // the prefix, finds no exe, and exits 0 — the game never launches.
            // The NeedsCompat fallback below already does this for the same
            // reason; the direct Windows/Proton path was the one place missing
            // it (umu then derives the game's own working dir from the abs exe).
            target_command.make_absolute(PathBuf::from(install_dir));
            let reconstructed_cmd = target_command.reconstruct();
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "4_direct_launch",
                "game_id": &game_id,
                "reconstructed_command": &reconstructed_cmd,
            }));
            match process_handler.create_launch_process(
                &meta,
                reconstructed_cmd.clone(),
                game_version,
                install_dir,
                &db_lock,
            ) {
                Ok(s) => s,
                Err(ProcessError::NeedsCompat(ref binary)) => {
                    // A native handler found a Windows binary — fall through
                    // to the Windows/Proton handler.
                    warn!("[LAUNCH] NeedsCompat for {binary:?} — falling back to Windows handler");
                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "4_needs_compat_fallback",
                        "game_id": &game_id,
                        "binary": binary,
                    }));
                    let compat = self
                        .fetch_process_handler(&db_lock, &Platform::Windows)
                        .map_err(|_| ProcessError::NoCompat)?;
                    let win_launch_cmd = game_version
                        .launches
                        .iter()
                        .filter(|v| v.platform == Platform::Windows)
                        .nth(launch_process_index)
                        .and_then(|lc| {
                            ParsedCommand::parse(lc.command.clone()).ok().map(|mut p| {
                                p.make_absolute(PathBuf::from(install_dir));
                                p.reconstruct()
                            })
                        })
                        .unwrap_or(reconstructed_cmd);
                    let mut win_meta = meta.clone();
                    win_meta.target_platform = Platform::Windows;
                    let result = compat.create_launch_process(
                        &win_meta,
                        win_launch_cmd,
                        game_version,
                        install_dir,
                        &db_lock,
                    )?;
                    needs_platform_correction = true;
                    result
                }
                Err(e) => {
                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "4_handler_error",
                        "game_id": &game_id,
                        "error": format!("{e}"),
                    }));
                    return Err(e);
                }
            }
        };

        // ── STEP 5: Format through launch template ─────────────────────────
        let working_dir = effective_cwd.as_deref().unwrap_or(install_dir);
        let mut parsed_launch = ParsedCommand::parse(target_launch_string.clone())?;
        let executable_name = parsed_launch.command.clone();
        let game_install_dir_owned = install_dir.to_string();
        parsed_launch.make_absolute(working_dir.into());
        // Launch from the executable's OWN directory for normal games: this
        // matches double-clicking the .exe, and stays correct when an install
        // nests the game one folder deep (otherwise CWD sits a level above the
        // binary and CWD-relative lookups silently fail — Goldberg runtime
        // state, a game loading data past its first menu, etc.). Emulator
        // launches keep their effective_cwd (the emulator install dir).
        let working_dir_owned = if effective_cwd.is_some() {
            working_dir.to_string()
        } else {
            std::path::Path::new(&parsed_launch.command)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| working_dir.to_string())
        };

        let format_args = DropFormatArgs::new(
            target_launch_string,
            install_dir,
            &executable_name,
            parsed_launch.command,
            None,
        );
        // Two passes so a template that itself contains placeholders (e.g.
        // a wrapper that references {abs_exe}) is fully expanded.
        let target_launch_string = SimpleCurlyFormat
            .format(&game_version.user_configuration.launch_template, &format_args)
            .map_err(|e| ProcessError::FormatError(e.to_string()))?
            .to_string();
        let target_launch_string = SimpleCurlyFormat
            .format(&target_launch_string, format_args)
            .map_err(|e| ProcessError::FormatError(e.to_string()))?
            .to_string();
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "5_post_template",
            "game_id": &game_id,
            "final_launch_string": &target_launch_string,
        }));

        // A streaming request can override the user's config so the remote
        // client's settings apply on the host.
        let user_configuration =
            config_override.unwrap_or_else(|| game_version.user_configuration.clone());

        drop(db_lock);

        if needs_platform_correction {
            let mut db_w = borrow_db_mut_checked();
            if let Some(stored) =
                db_w.applications.installed_game_version.get_mut(&game_id)
            {
                stored.target_platform = Platform::Windows;
                info!("[LAUNCH] corrected target_platform for {game_id} to Windows");
            }
        }

        // ── STEP 6: Final command parsing ──────────────────────────────────
        let launch_parameters = LaunchParameters(
            ParsedCommand::parse(target_launch_string.clone())?,
            working_dir_owned.clone().into(),
        );
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "6_final_command",
            "game_id": &game_id,
            "executable": &launch_parameters.0.command,
            "executable_exists": std::path::Path::new(&launch_parameters.0.command).exists(),
            "args": &launch_parameters.0.args,
            "env_vars": &launch_parameters.0.env,
            "working_dir": &working_dir_owned,
        }));

        // Pre-launch guard: if the resolved target is an absolute path that no
        // longer exists, fail with a clear, actionable error instead of a
        // silent/black-screen launch — the usual cause is antivirus removing a
        // game exe or crack DLL. Only fires for absolute, missing paths:
        // Proton/emulator launches resolve `command` to their wrapper (which
        // exists), and relative/PATH-resolved commands are left to spawn.
        {
            let cmd_path = std::path::Path::new(&launch_parameters.0.command);
            if cmd_path.is_absolute() && !cmd_path.exists() {
                info!(
                    "[LAUNCH] target missing (likely quarantined), refusing: {}",
                    launch_parameters.0.command
                );
                return Err(ProcessError::LaunchTargetMissing(
                    launch_parameters.0.command.clone(),
                ));
            }
        }

        info!(
            "[LAUNCH] spawning (cwd {}): {:?}",
            launch_parameters.1.to_string_lossy(),
            launch_parameters.0
        );

        // ── STEP 7: Build the Command + RetroArch + save sync ──────────────
        let spawn_plan = self.build_command(
            launch_parameters,
            process_handler,
            &log_path,
            &error_log_path,
            log_file,
            error_file,
            &game_id,
            &game_install_dir_owned,
            &user_configuration,
            &effective_cwd,
            emulator_rom_path.as_deref(),
        )?;
        // Reserve the game id. The caller drops the PROCESS_MANAGER lock on
        // the next line and runs the save sync without it, so until
        // `finish_launch` inserts into `processes` this set is the only record
        // that a launch is in flight.
        self.pending_launches.insert(game_id.clone());

        Ok(PreparedLaunch {
            spawn_plan,
            sync: SaveSyncRequest {
                game_id: game_id.clone(),
                target_platform: meta.target_platform,
                effective_cwd,
                emulator_rom_path,
                streaming,
            },
            meta,
            game_id,
            log_path,
            error_log_path,
            incognito,
        })
    }

    /// Spawn the prepared command and register the running process.
    ///
    /// The second half of [`run_launch`], run under a freshly taken
    /// PROCESS_MANAGER lock once the cloud-save sync has finished.
    fn finish_launch(
        &mut self,
        prepared: PreparedLaunch,
        save_snapshot: Option<crate::process_manager::SaveSyncSnapshot>,
    ) -> Result<(), ProcessError> {
        let PreparedLaunch {
            spawn_plan,
            meta,
            game_id,
            log_path,
            error_log_path,
            incognito,
            sync: _,
        } = prepared;

        // Drop the reservation now, before any fallible step: a spawn failure
        // must not leave the game stuck reporting "already running".
        self.pending_launches.remove(&game_id);

        let SpawnPlan {
            mut command,
            spawn_executable,
            spawn_args,
            spawn_env,
            is_script,
            working_dir: command_working_dir,
            emulator_info,
            retroarch_ra,
        } = spawn_plan;

        // ── STEP 8: Spawn ──────────────────────────────────────────────────
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "8_spawning",
            "game_id": &game_id,
            "command": &spawn_executable,
            "wrapped_in_bash": is_script,
        }));
        env::log_launch_env_fingerprint(&command, &game_id);

        // Put the child in its own process group so kill can signal the
        // whole tree (bash → umu → proton → wine → game) at once.
        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        let child = match command.spawn() {
            Ok(child) => {
                info!("[LAUNCH] {game_id}: spawned pid {}", child.id());
                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                    "step": "8_spawn_success", "game_id": &game_id, "pid": child.id(),
                }));
                child
            }
            Err(e) => self.spawn_with_enoexec_retry(
                e,
                &game_id,
                &spawn_executable,
                &spawn_args,
                &spawn_env,
                is_script,
                &command_working_dir,
                &log_path,
                &error_log_path,
            )?,
        };

        let launch_process_handle = Arc::new(SharedChild::new(child)?);

        self.register_running_process(
            &meta,
            launch_process_handle,
            emulator_info,
            save_snapshot,
            retroarch_ra,
            incognito,
        );
        Ok(())
    }

    /// Start the playtime session + achievement polling, flip the game to
    /// `Running`, insert it into the process table, and spawn the wait
    /// thread that detects exit. Split out of `launch_process_inner` so the
    /// long spawn flow ends on a single readable call.
    ///
    /// `incognito` suppresses the server-facing parts of the lifecycle:
    /// no playtime session is opened, no heartbeat fires, no achievement
    /// poller runs. The local process table + Running status are unchanged,
    /// so the in-app UI still tracks the launch.
    fn register_running_process(
        &mut self,
        meta: &database::DownloadableMetadata,
        launch_process_handle: Arc<SharedChild>,
        emulator_info: Option<remote::goldberg::EmulatorInfo>,
        save_snapshot: Option<crate::process_manager::SaveSyncSnapshot>,
        retroarch_ra: Option<crate::process_manager::RetroArchRaSession>,
        incognito: bool,
    ) {
        let game_id = meta.id.clone();

        // Start the playtime session asynchronously — never block launch.
        // The id is stored in a shared mutex so the exit path can read it;
        // once established we kick off a heartbeat so the server can bound a
        // session whose stop never arrives (crash, kill -9, power loss).
        //
        // In incognito mode this block is skipped entirely. The slot is
        // still created (but never populated) so the exit path's
        // `wait_for_session_id` will time out into the "no session — skip
        // stop" branch, which is exactly the no-op we want.
        let playtime_session_id = Arc::new(std::sync::Mutex::new(None::<String>));
        let playtime_heartbeat_cancel = Arc::new(Notify::new());
        if incognito {
            info!("[LAUNCH] {game_id}: incognito — no playtime session will be opened");
            let _ = self.app_handle.emit("game_incognito_started", &game_id);
        } else {
            let playtime_game_id = game_id.clone();
            let session_id_slot = playtime_session_id.clone();
            let hb_cancel = playtime_heartbeat_cancel.clone();
            tauri::async_runtime::spawn(async move {
                match remote::playtime::start_playtime(&playtime_game_id).await {
                    Ok(sid) => {
                        info!("[LAUNCH] playtime session started: {sid}");
                        if let Ok(mut slot) = session_id_slot.lock() {
                            *slot = Some(sid.clone());
                        }
                        exit::run_playtime_heartbeat_loop(sid, hb_cancel).await;
                    }
                    Err(e) => warn!(
                        "[LAUNCH] could not start playtime session for {playtime_game_id}: {e}"
                    ),
                }
            });
        }

        // Flip the game to Running through the central state machine, then
        // write the transient status that actually masks the persistent one.
        {
            let mut db_lock = borrow_db_mut_checked();
            transition_from_db(&db_lock, &game_id, StatusKind::Running);
            db_lock
                .applications
                .transient_statuses
                .insert(meta.clone(), ApplicationTransientStatus::Running {});
        }
        push_game_update(
            &self.app_handle,
            &game_id,
            None,
            (None, Some(ApplicationTransientStatus::Running {})),
        );

        // Achievement polling for the session. In incognito we still
        // allocate the cancel notify so the RunningProcess struct stays
        // uniform, but we never spawn the poller — no server-side
        // achievement sync happens for this session.
        let achievement_cancel = Arc::new(Notify::new());
        if !incognito {
            let cancel = achievement_cancel.clone();
            let poll_game_id = game_id.clone();
            let poll_emulator_info = emulator_info;
            let poll_app_handle = self.app_handle.clone();
            tauri::async_runtime::spawn(async move {
                remote::achievements::poll_achievements(
                    poll_game_id,
                    poll_emulator_info,
                    cancel,
                    move |achievement| {
                        info!(
                            "[ACHIEVEMENT] unlocked: {} - {}",
                            achievement.title, achievement.description
                        );
                        let _ = poll_app_handle.emit(
                            "achievement_unlocked",
                            serde_json::json!({
                                "id": achievement.id,
                                "title": achievement.title,
                                "description": achievement.description,
                                "iconUrl": achievement.icon_url,
                            }),
                        );
                    },
                )
                .await;
            });
        } else {
            // Drop the unused `emulator_info` explicitly — keeping it bound
            // would otherwise prompt a clippy warning about a value moved
            // into a branch that's never used.
            let _ = emulator_info;
        }

        let wait_handle = launch_process_handle.clone();
        self.processes.insert(
            game_id.clone(),
            RunningProcess {
                handle: launch_process_handle,
                meta: meta.clone(),
                start: Instant::now(),
                manually_killed: false,
                playtime_session_id,
                playtime_heartbeat_cancel,
                achievement_poll_cancel: Some(achievement_cancel),
                save_snapshot,
                retroarch_ra,
            },
        );

        // The wait thread blocks until the entire process tree exits, then
        // hands off to the exit path. This is the authoritative, reliable
        // exit-detection mechanism: `wait()` cannot miss a real exit.
        spawn(move || {
            let result = wait_handle.wait();
            PROCESS_MANAGER.lock().on_process_finish(game_id, result)
        });
    }

}

/// Run the pre-launch cloud-save sync for whichever discovery strategy
/// applies (emulator vs PC/native). Returns the snapshot for the exit
/// path, or `None` when the game has no syncable saves.
///
/// A free function taking only the `AppHandle`, not a `ProcessManager` method:
/// this is the step that can block on a user-facing conflict dialog, and it
/// must not run with the PROCESS_MANAGER lock held.
fn pre_launch_save_sync(
    app_handle: &tauri::AppHandle,
    req: &SaveSyncRequest,
) -> Option<crate::process_manager::SaveSyncSnapshot> {
    let game_id = req.game_id.as_str();

    // Global cloud-save toggle (settings.cloud_saves_enabled). When the
    // user has cloud sync disabled we skip the entire pre-launch path —
    // no scan, no Ludusavi, no network. The returned `None` also wires
    // through to the exit path so post-exit upload is skipped too.
    if !database::borrow_db_checked().settings.cloud_saves_enabled {
        log::info!(
            "[SAVE-SYNC] cloud_saves_enabled=false — skipping pre-launch sync for {game_id}"
        );
        return None;
    }

    // Every cloud row is keyed by user id on the server, and every local save
    // path is now keyed by it here. With nobody signed in there is no answer to
    // "whose saves are these", and the only safe thing to do is not sync — no
    // fallback path, no shared "unknown" bucket, because either of those is how
    // one account's progress ends up in another account's library. The game
    // still launches; its saves stay on this disk under the signed-out layout.
    let Some(user_id) = remote::save_sync::current_user_id() else {
        log::warn!("[SAVE-SYNC] Nobody is signed in — skipping pre-launch sync for {game_id}");
        return None;
    };

    if let Some(emu_dir) = &req.effective_cwd {
        let _ = app_handle.emit("launch_trace", serde_json::json!({
            "step": "7c_save_sync_start", "game_id": game_id,
        }));
        let snap = save_sync_mod::sync_emulator_saves(
            app_handle,
            &user_id,
            game_id,
            emu_dir,
            req.emulator_rom_path.as_deref(),
            req.streaming,
        );
        let _ = app_handle.emit("launch_trace", serde_json::json!({
            "step": "7c_save_sync_done",
            "game_id": game_id,
            "has_snapshot": snap.is_some(),
        }));
        return snap;
    }

    // PC/native game — discover saves via Ludusavi keyed on the name.
    let game_name = remote::cache::get_cached_object::<games::library::Game>(
        &format!("game/{game_id}"),
    )
    .ok()
    .map(|g| g.m_name)?;

    // Only feed Ludusavi a `--wine-prefix` when we know one applies:
    // Linux host + Windows target. The prefix is created at launch time
    // (see process_handlers.rs), so on first sync it may not yet exist —
    // in that case we omit it and let Ludusavi fall back to defaults.
    let wine_prefix = compute_wine_prefix_for(game_id, &req.target_platform);

    let _ = app_handle.emit("launch_trace", serde_json::json!({
        "step": "7d_pc_save_sync_start",
        "game_id": game_id,
        "game_name": &game_name,
        "wine_prefix": wine_prefix.as_ref().map(|p| p.to_string_lossy().to_string()),
    }));
    let snap = save_sync_mod::sync_pc_saves(
        app_handle,
        &user_id,
        game_id,
        &game_name,
        wine_prefix,
        req.streaming,
    );
    let _ = app_handle.emit("launch_trace", serde_json::json!({
        "step": "7d_pc_save_sync_done",
        "game_id": game_id,
        "has_snapshot": snap.is_some(),
    }));
    snap
}

/// Compute the Wine prefix path to feed to Ludusavi, if applicable.
///
/// Returns `Some(path)` only when ALL of the following hold:
///   * Host OS is Linux.
///   * Target platform is Windows (Drop's UMU/Proton launchers).
///   * The prefix directory actually exists on disk — the prefix is
///     created lazily at launch time by [`crate::process_handlers`], so
///     a first-time sync (e.g. immediately after install) may legitimately
///     have nothing there yet. In that case we return `None` and Ludusavi
///     falls back to its default scan locations.
#[cfg(target_os = "linux")]
fn compute_wine_prefix_for(
    game_id: &str,
    target_platform: &Platform,
) -> Option<PathBuf> {
    if !matches!(target_platform, Platform::Windows) {
        return None;
    }
    let pfx = database::db::DATA_ROOT_DIR.join("pfx").join(game_id);
    if pfx.is_dir() { Some(pfx) } else { None }
}

#[cfg(not(target_os = "linux"))]
fn compute_wine_prefix_for(
    _game_id: &str,
    _target_platform: &Platform,
) -> Option<PathBuf> {
    None
}

/// Everything `build_command` produces — the spawnable `Command` plus the
/// raw pieces needed to retry under a bash wrapper on ENOEXEC.
pub(crate) struct SpawnPlan {
    pub command: Command,
    pub spawn_executable: String,
    pub spawn_args: Vec<String>,
    pub spawn_env: Vec<String>,
    pub is_script: bool,
    pub working_dir: String,
    pub emulator_info: Option<remote::goldberg::EmulatorInfo>,
    /// Present when this is a RetroArch launch with RA credentials injected.
    pub retroarch_ra: Option<crate::process_manager::RetroArchRaSession>,
}

impl ProcessManager<'_> {
    /// Translate the final [`LaunchParameters`] into a ready-to-spawn
    /// [`Command`]: detect shebang scripts (Linux), apply the env (filtered),
    /// scrub AppImage/Gamescope/MangoHud env, run the handler's
    /// `modify_command`, configure Goldberg/RetroArch.
    #[allow(clippy::too_many_arguments)]
    fn build_command(
        &self,
        launch_parameters: LaunchParameters,
        process_handler: &(dyn crate::process_manager::ProcessHandler + Send + Sync),
        log_path: &std::path::Path,
        error_log_path: &std::path::Path,
        log_file: std::fs::File,
        error_file: std::fs::File,
        game_id: &str,
        game_install_dir: &str,
        user_configuration: &database::models::data::UserConfiguration,
        effective_cwd: &Option<String>,
        emulator_rom_path: Option<&str>,
    ) -> Result<SpawnPlan, ProcessError> {
        let _ = (log_path, error_log_path); // reserved for future trace use
        let working_dir_owned = launch_parameters.1.to_string_lossy().to_string();

        // Save the raw command pieces before they're moved — needed for the
        // ENOEXEC bash-wrapper retry.
        let spawn_executable = launch_parameters.0.command.clone();
        let spawn_args = launch_parameters.0.args.clone();
        let spawn_env = launch_parameters.0.env.clone();

        // On Linux, pip/pipx scripts (umu-run) can fail with ENOEXEC when
        // execvp'd directly if their shebang interpreter isn't on the
        // restricted Game Mode PATH. Detect a script by its `#!` magic and
        // wrap it in bash so the shebang resolves.
        let is_script = detect_script(&launch_parameters.0.command);

        let mut command = if is_script {
            info!("[LAUNCH] {game_id}: script executable — wrapping in bash");
            let mut cmd = Command::new("/bin/bash");
            // Build one shell string, every component shell-quoted so a path
            // with spaces or metacharacters survives as a single argument.
            let mut shell_cmd =
                shell_words::quote(&launch_parameters.0.command).to_string();
            for arg in &launch_parameters.0.args {
                shell_cmd.push(' ');
                shell_cmd.push_str(&shell_words::quote(arg));
            }
            cmd.args(["-c", &shell_cmd]);
            cmd
        } else {
            let mut cmd = Command::new(&launch_parameters.0.command);
            cmd.args(&launch_parameters.0.args);
            cmd
        };

        // Apply launch-config env, dropping denylisted keys.
        env::apply_launch_env(&mut command, &launch_parameters.0.env);

        command
            .stderr(error_file)
            .stdout(log_file)
            .current_dir(&launch_parameters.1);
        env::apply_baseline_env_scrub(&mut command);
        env::sanitize_appimage_env(&mut command);

        // Gamescope / Steam Deck display env. `is_appimage` flags a game
        // executable that is itself an AppImage (stale bundled Mesa).
        let is_appimage = spawn_executable.to_lowercase().ends_with(".appimage");
        env::configure_gamescope_env(&mut command, game_id, is_appimage);

        // MangoHud — per-game setting wins over the global Settings value.
        #[cfg(target_os = "linux")]
        {
            let effective_preset = user_configuration
                .mangohud
                .clone()
                .or_else(|| borrow_db_checked().settings.global_mangohud.clone());
            env::configure_mangohud_env(&mut command, effective_preset.as_ref());
        }
        #[cfg(not(target_os = "linux"))]
        let _ = user_configuration;

        // Handler-specific tweaks (e.g. CREATE_NO_WINDOW on Windows).
        process_handler.modify_command(&mut command);

        // Goldberg/SSE Steam-emulator save configuration.
        let display_name = remote::cache::get_cached_object::<::client::user::User>("user")
            .ok()
            .map(|u| u.display_name().to_string());
        let emulator_info = remote::goldberg::configure_saves_for_game(
            game_install_dir,
            display_name.as_deref(),
        );

        // Co-op: seed Goldberg's custom_broadcasts.txt with the active room's
        // peer IPs so LAN discovery works over the ZeroTier overlay (which drops
        // broadcast). Empty list (not in a room) clears any stale file. Bounded
        // + best-effort — slow/absent server must never delay or block a launch.
        if let Some(info) = &emulator_info
            && info.is_goldberg_like()
        {
            let peers = tauri::async_runtime::block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    remote::coop::current_peer_ips(),
                )
                .await
                .unwrap_or_default()
            });
            remote::coop::seed_and_record(
                std::path::Path::new(info.dll_dir()),
                &peers,
            );
        }

        // RetroArch config injection for emulator launches.
        let retroarch_ra = effective_cwd.as_ref().and_then(|emu_dir| {
            self.configure_retroarch(
                &mut command,
                game_id,
                emu_dir,
                user_configuration,
                emulator_rom_path,
                is_script,
            )
        });

        // Switch-emulator config injection. Same slot, same best-effort
        // contract: a Switch emulator is never also RetroArch, so exactly one
        // of these two does any work.
        if let Some(emu_dir) = effective_cwd.as_ref() {
            self.configure_switch_emu(game_id, emu_dir);
        }

        Ok(SpawnPlan {
            command,
            spawn_executable,
            spawn_args,
            spawn_env,
            is_script,
            working_dir: working_dir_owned,
            emulator_info,
            retroarch_ra,
        })
    }

    /// Configure RetroArch for an emulator launch: fetch RA credentials
    /// (tight timeout — nice-to-have), patch `retroarch.cfg`, and inject
    /// `--appendconfig` so the AppImage actually reads our config.
    ///
    /// Returns the RA session details when this really is RetroArch and
    /// credentials were injected, so the exit path can check whether
    /// RetroAchievements rejected them.
    fn configure_retroarch(
        &self,
        command: &mut Command,
        game_id: &str,
        emu_dir: &str,
        user_configuration: &database::models::data::UserConfiguration,
        emulator_rom_path: Option<&str>,
        is_script: bool,
    ) -> Option<crate::process_manager::RetroArchRaSession> {
        // Taken before anything else so it can never be later than the log
        // this launch is about to write. The exit path uses it to ignore logs
        // from earlier sessions.
        let launched_at = std::time::SystemTime::now();
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "7_retroarch_config_start", "game_id": game_id, "emu_dir": emu_dir,
        }));

        // RA auto-login is nice-to-have, not a launch blocker — bound the
        // credential fetch tightly so slow network doesn't delay the game.
        let ra_creds = tauri::async_runtime::block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                remote::retroarch::fetch_ra_credentials(),
            )
            .await
            .unwrap_or_else(|_| {
                info!("[RETROARCH] RA credential fetch timed out after 2s, skipping");
                None
            })
        });
        // Same identity the pre-launch scan used. RetroArch is told where to
        // put saves, so this is the writer half of the per-user path: if it
        // disagreed with the scanner the game would save into a tree cloud
        // sync never looks at.
        let save_user_id = remote::save_sync::current_user_id();
        // Re-resolved every launch for the same reason the Switch-emulator
        // path does it: the user can swap pads between sessions, and a
        // fallback written for the pad they had last week is silently wrong.
        let detected_pad = crate::gamepad::detect_primary_pad_family();
        let retroarch_info = remote::retroarch::configure_retroarch_for_game(
            emu_dir,
            save_user_id.as_deref(),
            game_id,
            ra_creds.as_ref(),
            Some(user_configuration),
            detected_pad,
            emulator_rom_path,
        );

        let cfg_path = std::path::Path::new(emu_dir).join("retroarch.cfg");
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "7_retroarch_config_result",
            "game_id": game_id,
            "cfg_path": cfg_path.display().to_string(),
            "cfg_exists": cfg_path.exists(),
            "retroarch_detected": retroarch_info.is_some(),
            "has_ra_credentials": ra_creds.is_some(),
            "detected_pad": format!("{detected_pad:?}"),
        }));

        // The RetroArch AppImage overrides $HOME, so it reads config from
        // its own .home dir, not the file we wrote. --appendconfig layers
        // our settings on top of its defaults.
        if retroarch_info.is_some() && cfg_path.exists() {
            if is_script {
                warn!("[LAUNCH] RetroArch is script-wrapped — cannot inject --appendconfig");
            } else {
                info!("[LAUNCH] injecting --appendconfig {}", cfg_path.display());
                command.arg("--appendconfig");
                command.arg(cfg_path.as_os_str());
            }
            // Verbose logging so RetroArch dumps video-driver init to stderr
            // — critical for diagnosing "audio but no video" in Gamescope.
            command.arg("--verbose");
        }

        // Only worth watching for an RA rejection when we actually handed
        // RetroArch a token to be rejected.
        match (retroarch_info.is_some(), ra_creds) {
            (true, Some(creds)) => Some(crate::process_manager::RetroArchRaSession {
                emu_root: std::path::PathBuf::from(emu_dir),
                connect_token: creds.connect_token,
                launched_at,
            }),
            _ => None,
        }
    }

    /// Configure a yuzu-family Switch emulator (Eden, yuzu, Citron, Sudachi,
    /// Suyu) for this launch: force portable mode and rewrite the player-1
    /// input bindings for whatever pad is connected right now.
    ///
    /// Re-resolving the pad every launch is the point — SDL renumbers ports as
    /// devices come and go, so a binding written once goes stale silently.
    ///
    /// Best-effort throughout: it returns nothing, swallows every failure into
    /// the launch trace, and can never block or fail a launch. Directories
    /// that hold no Switch emulator (the overwhelmingly common case) fall out
    /// on the first directory read.
    fn configure_switch_emu(&self, game_id: &str, emu_dir: &str) {
        let pad = crate::gamepad::resolve_primary_pad();
        let outcome = remote::switchemu::configure_switch_emu_for_game(emu_dir, pad.as_ref());

        if matches!(
            outcome,
            remote::switchemu::SwitchEmuOutcome::NotSwitchEmulator
        ) {
            return;
        }

        let _ = self.app_handle.emit(
            "launch_trace",
            serde_json::json!({
                "step": "7b_switchemu_config",
                "game_id": game_id,
                "emu_dir": emu_dir,
                "result": outcome,
            }),
        );
    }
}

/// Detect whether `path` is a shebang script by reading its first two bytes.
/// Always `false` outside Linux (only Linux has the umu-run ENOEXEC issue).
fn detect_script(path: &str) -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        false
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::Read as _;
        match std::fs::File::open(path) {
            Ok(mut f) => {
                let mut magic = [0u8; 2];
                match f.read_exact(&mut magic) {
                    Ok(()) => {
                        let is_script = magic == [b'#', b'!'];
                        info!(
                            "[LAUNCH] script detection for {path:?}: \
                             magic=[0x{:02x},0x{:02x}], is_script={is_script}",
                            magic[0], magic[1]
                        );
                        is_script
                    }
                    Err(e) => {
                        warn!("[LAUNCH] script detection: cannot read magic from {path:?}: {e}");
                        false
                    }
                }
            }
            Err(e) => {
                warn!("[LAUNCH] script detection: cannot open {path:?}: {e}");
                false
            }
        }
    }
}
