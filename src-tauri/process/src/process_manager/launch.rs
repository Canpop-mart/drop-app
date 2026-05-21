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
    ApplicationTransientStatus, GameDownloadStatus, borrow_db_checked, borrow_db_mut_checked,
    models::data::InstalledGameType, platform::Platform,
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

impl ProcessManager<'_> {
    /// Launch a game process for normal (non-streaming) play.
    pub fn launch_process(
        &mut self,
        game_id: String,
        launch_process_index: usize,
    ) -> Result<(), ProcessError> {
        self.launch_process_inner(game_id, launch_process_index, false, None)
    }

    /// Launch a game process for streaming.
    ///
    /// When `streaming` is true, save-sync conflicts are auto-resolved to
    /// `keep_local` instead of showing a UI dialog (which would appear on
    /// the remote host PC where the user can't interact with it). If
    /// `config_override` is provided it temporarily replaces the game's
    /// local `user_configuration` so the receiver's settings (widescreen,
    /// quality, …) are applied on the host.
    pub fn launch_process_streaming(
        &mut self,
        game_id: String,
        launch_process_index: usize,
        config_override: Option<database::models::data::UserConfiguration>,
    ) -> Result<(), ProcessError> {
        self.launch_process_inner(game_id, launch_process_index, true, config_override)
    }

    fn launch_process_inner(
        &mut self,
        game_id: String,
        launch_process_index: usize,
        streaming: bool,
        config_override: Option<database::models::data::UserConfiguration>,
    ) -> Result<(), ProcessError> {
        if self.processes.contains_key(&game_id) {
            return Err(ProcessError::AlreadyRunning);
        }

        // The launch flow only *reads* the database — every status write
        // happens later in its own scoped `borrow_db_mut_checked()`. An
        // immutable borrow here keeps launch from blocking concurrent reads.
        let db_lock = borrow_db_checked();

        let meta = db_lock
            .applications
            .installed_game_version
            .get(&game_id)
            .cloned()
            .ok_or(ProcessError::NotInstalled)?;

        let game_status = db_lock
            .applications
            .game_statuses
            .get(&game_id)
            .ok_or(ProcessError::NotInstalled)?;

        let (version_name, install_dir) = match game_status {
            GameDownloadStatus::Installed {
                version_id: version_name,
                install_dir,
                install_type: InstalledGameType::Installed | InstalledGameType::SetupRequired,
                ..
            } => (version_name, install_dir),
            _ => return Err(ProcessError::NotInstalled),
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
            "install_type": match game_status {
                GameDownloadStatus::Installed { install_type, .. } => format!("{:?}", install_type),
                other => format!("{:?}", other),
            },
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
        let (target_command, emulator, disc_paths) = match game_status {
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::Installed,
                ..
            } => {
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
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::SetupRequired,
                ..
            } => {
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
        let working_dir_owned = working_dir.to_string();
        let game_install_dir_owned = install_dir.to_string();
        parsed_launch.make_absolute(working_dir.into());

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
        let SpawnPlan {
            mut command,
            spawn_executable,
            spawn_args,
            spawn_env,
            is_script,
            working_dir: command_working_dir,
            emulator_info,
        } = spawn_plan;

        // Pre-launch cloud-save sync (blocking, timeout-bounded). Returns a
        // snapshot the exit path diffs against to know what to upload.
        let save_snapshot = self.run_pre_launch_save_sync(
            &game_id,
            &effective_cwd,
            streaming,
        );

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
        );
        Ok(())
    }

    /// Start the playtime session + achievement polling, flip the game to
    /// `Running`, insert it into the process table, and spawn the wait
    /// thread that detects exit. Split out of `launch_process_inner` so the
    /// long spawn flow ends on a single readable call.
    fn register_running_process(
        &mut self,
        meta: &database::DownloadableMetadata,
        launch_process_handle: Arc<SharedChild>,
        emulator_info: Option<remote::goldberg::EmulatorInfo>,
        save_snapshot: Option<crate::process_manager::SaveSyncSnapshot>,
    ) {
        let game_id = meta.id.clone();

        // Start the playtime session asynchronously — never block launch.
        // The id is stored in a shared mutex so the exit path can read it;
        // once established we kick off a heartbeat so the server can bound a
        // session whose stop never arrives (crash, kill -9, power loss).
        let playtime_session_id = Arc::new(std::sync::Mutex::new(None::<String>));
        let playtime_heartbeat_cancel = Arc::new(Notify::new());
        {
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

        // Achievement polling for the session.
        let achievement_cancel = Arc::new(Notify::new());
        {
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
        }

        let wait_handle = launch_process_handle.clone();
        self.processes.insert(
            game_id.clone(),
            RunningProcess {
                handle: launch_process_handle,
                start: Instant::now(),
                manually_killed: false,
                playtime_session_id,
                playtime_heartbeat_cancel,
                achievement_poll_cancel: Some(achievement_cancel),
                save_snapshot,
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

    /// Run the pre-launch cloud-save sync for whichever discovery strategy
    /// applies (emulator vs PC/native). Returns the snapshot for the exit
    /// path, or `None` when the game has no syncable saves.
    fn run_pre_launch_save_sync(
        &self,
        game_id: &str,
        effective_cwd: &Option<String>,
        streaming: bool,
    ) -> Option<crate::process_manager::SaveSyncSnapshot> {
        if let Some(emu_dir) = effective_cwd {
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "7c_save_sync_start", "game_id": game_id,
            }));
            let snap = save_sync_mod::sync_emulator_saves(
                &self.app_handle,
                game_id,
                emu_dir,
                streaming,
            );
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
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
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "7d_pc_save_sync_start", "game_id": game_id, "game_name": &game_name,
        }));
        let snap =
            save_sync_mod::sync_pc_saves(&self.app_handle, game_id, &game_name, streaming);
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "7d_pc_save_sync_done",
            "game_id": game_id,
            "has_snapshot": snap.is_some(),
        }));
        snap
    }
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

        // RetroArch config injection for emulator launches.
        if let Some(emu_dir) = effective_cwd {
            self.configure_retroarch(
                &mut command,
                game_id,
                emu_dir,
                user_configuration,
                emulator_rom_path,
                is_script,
            );
        }

        Ok(SpawnPlan {
            command,
            spawn_executable,
            spawn_args,
            spawn_env,
            is_script,
            working_dir: working_dir_owned,
            emulator_info,
        })
    }

    /// Configure RetroArch for an emulator launch: fetch RA credentials
    /// (tight timeout — nice-to-have), patch `retroarch.cfg`, and inject
    /// `--appendconfig` so the AppImage actually reads our config.
    fn configure_retroarch(
        &self,
        command: &mut Command,
        game_id: &str,
        emu_dir: &str,
        user_configuration: &database::models::data::UserConfiguration,
        emulator_rom_path: Option<&str>,
        is_script: bool,
    ) {
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
        let retroarch_info = remote::retroarch::configure_retroarch_for_game(
            emu_dir,
            game_id,
            ra_creds.as_ref(),
            Some(user_configuration),
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
