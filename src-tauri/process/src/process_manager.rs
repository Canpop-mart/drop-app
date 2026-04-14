use std::{
    collections::HashMap,
    fs::{OpenOptions, create_dir_all},
    io,
    path::PathBuf,
    process::{Command, ExitStatus},
    sync::Arc,
    thread::spawn,
    time::{Duration, SystemTime},
};

use tokio::sync::Notify;

use database::{
    ApplicationTransientStatus, Database, DownloadableMetadata, GameDownloadStatus, GameVersion,
    borrow_db_checked, borrow_db_mut_checked, db::DATA_ROOT_DIR, models::data::InstalledGameType,
    platform::Platform,
};
use dynfmt::Format;
use dynfmt::SimpleCurlyFormat;
use games::{library::push_game_update, state::GameStatusManager};
use log::{debug, info, warn};
use serde::Serialize;
use shared_child::SharedChild;
use tauri::{AppHandle, Emitter as _};

use crate::{
    PROCESS_MANAGER,
    error::ProcessError,
    format::DropFormatArgs,
    parser::{LaunchParameters, ParsedCommand},
    process_handlers::{
        AsahiMuvmLauncher, MacLauncher, NativeLauncher, UMUCompatLauncher, UMUNativeLauncher,
        WindowsLauncher,
    },
};

pub struct RunningProcess {
    handle: Arc<SharedChild>,
    start: SystemTime,
    manually_killed: bool,
    playtime_session_id: Arc<std::sync::Mutex<Option<String>>>,
    achievement_poll_cancel: Option<Arc<Notify>>,
}

pub struct ProcessManager<'a> {
    current_platform: Platform,
    log_output_dir: PathBuf,
    processes: HashMap<String, RunningProcess>,
    game_launchers: Vec<(
        (Platform, Platform),
        &'a (dyn ProcessHandler + Sync + Send + 'static),
    )>,
    app_handle: AppHandle,
}

#[derive(Serialize)]
pub struct LaunchOption {
    name: String,
}

impl ProcessManager<'_> {
    pub fn new(app_handle: AppHandle) -> Self {
        let log_output_dir = DATA_ROOT_DIR.join("logs");

        ProcessManager {
            #[cfg(target_os = "windows")]
            current_platform: Platform::Windows,

            #[cfg(target_os = "macos")]
            current_platform: Platform::macOS,

            #[cfg(target_os = "linux")]
            current_platform: Platform::Linux,

            processes: HashMap::new(),
            log_output_dir,
            game_launchers: vec![
                // Current platform to target platform
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

    pub fn kill_game(&mut self, game_id: String) -> Result<(), io::Error> {
        match self.processes.get_mut(&game_id) {
            Some(process) => {
                process.manually_killed = true;

                // For Proton/Wine games, the process tree is:
                //   bash → umu-run (python) → proton → wine → game.exe
                // A simple kill() sends SIGKILL to the top process but
                // leaves Wine children orphaned, causing a slow cleanup.
                //
                // Strategy: send SIGTERM to the process group, then
                // schedule SIGKILL in a background thread. Do NOT block
                // the UI waiting for the process to exit — the background
                // wait thread (spawned at launch time) handles cleanup.
                #[cfg(target_os = "linux")]
                {
                    let pid = process.handle.id() as i32;
                    info!("[KILL] Sending SIGTERM to process group (pid {})", pid);

                    unsafe {
                        libc::kill(-pid, libc::SIGTERM);
                    }

                    // Spawn a thread to send SIGKILL after a grace period.
                    // This avoids blocking the UI thread on slow Wine cleanup.
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(500));
                        info!("[KILL] Sending SIGKILL to process group (pid {})", pid);
                        unsafe {
                            libc::kill(-pid, libc::SIGKILL);
                        }
                    });
                }

                #[cfg(not(target_os = "linux"))]
                {
                    let _ = process.handle.kill();
                }

                // Do NOT call process.handle.wait() here — it blocks until
                // the entire process tree exits, which can take 10+ seconds
                // for Proton/Wine. The background wait thread (spawned when
                // the game was launched) will call on_process_finish() and
                // clean up state when the process actually terminates.
                info!("[KILL] Kill signals sent, returning immediately");
                Ok(())
            }
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Game ID not running",
            )),
        }
    }

    pub fn get_log_dir(&self, game_id: &str) -> PathBuf {
        self.log_output_dir.join(game_id)
    }

    fn on_process_finish(
        &mut self,
        game_id: String,
        result: Result<ExitStatus, std::io::Error>,
    ) -> Result<(), ProcessError> {
        if !self.processes.contains_key(&game_id) {
            warn!(
                "process on_finish was called, but game_id is no longer valid. finished with result: {result:?}"
            );
            return Ok(());
        }

        debug!("process for {:?} exited with {:?}", &game_id, result);

        let process = match self.processes.remove(&game_id) {
            Some(process) => process,
            None => {
                info!("Attempted to stop process {game_id} which didn't exist");
                return Ok(());
            }
        };

        // Stop achievement polling
        if let Some(cancel) = &process.achievement_poll_cancel {
            cancel.notify_one();
        }

        // Report playtime stop and trigger server-side achievement sync
        {
            let stop_session_id = process.playtime_session_id.lock().ok().and_then(|s| s.clone());
            let sync_game_id = game_id.clone();
            tauri::async_runtime::spawn(async move {
                // Stop playtime session
                if let Some(session_id) = stop_session_id {
                    if let Err(e) = remote::playtime::stop_playtime(&session_id).await {
                        warn!("Failed to report playtime stop: {e}");
                    }
                }

                // Notify server that game session ended — triggers Steam/RA sync
                if let Err(e) =
                    remote::achievements::notify_session_end(&sync_game_id).await
                {
                    warn!("Failed to notify session end for {}: {e}", sync_game_id);
                }
            });
        }

        let mut db_handle = borrow_db_mut_checked();
        let meta = match db_handle
            .applications
            .installed_game_version
            .get(&game_id)
            .cloned()
        {
            Some(meta) => meta,
            None => {
                warn!("Could not get installed version of {}, skipping cleanup", &game_id);
                return Ok(());
            }
        };
        db_handle.applications.transient_statuses.remove(&meta);

        let current_state = db_handle.applications.game_statuses.get_mut(&game_id);
        if let Some(GameDownloadStatus::Installed { install_type, .. }) = current_state
            && let Ok(exit_code) = result
            && exit_code.success()
        {
            *install_type = InstalledGameType::Installed;
        }

        let elapsed = process.start.elapsed().unwrap_or(Duration::ZERO);
        // If we started and ended really quickly, something might've gone wrong
        // Or if the status isn't 0
        // Or if it's an error
        if !process.manually_killed
            && (elapsed.as_secs() <= 2 || result.map_or(true, |r| !r.success()))
        {
            warn!("drop detected that the game {game_id} may have failed to launch properly");
            let _ = self.app_handle.emit("launch_external_error", &game_id);
        }

        let version_data = db_handle
            .applications
            .game_versions
            .get(&meta.version)
            .cloned();
        if version_data.is_none() {
            warn!(
                "game_versions missing entry for version {} (game {}); pushing status update without version",
                meta.version, game_id
            );
        }

        let status = GameStatusManager::fetch_state(&game_id, &db_handle);

        push_game_update(
            &self.app_handle,
            &game_id,
            version_data,
            status,
        );
        Ok(())
    }

    fn fetch_process_handler(
        &self,
        db_lock: &Database,
        target_platform: &Platform,
    ) -> Result<&(dyn ProcessHandler + Send + Sync), ProcessError> {
        info!(
            "[LAUNCH] Selecting handler: current={:?}, target={:?}",
            self.current_platform, target_platform
        );
        let handler = self
            .game_launchers
            .iter()
            .find(|e| {
                let (e_current, e_target) = e.0;
                let platform_match = e_current == self.current_platform && e_target == *target_platform;
                if platform_match {
                    let valid = e.1.valid_for_platform(db_lock, target_platform);
                    debug!(
                        "[LAUNCH]   Handler ({:?}->{:?}) platform match, valid={}",
                        e_current, e_target, valid
                    );
                    valid
                } else {
                    false
                }
            })
            .ok_or_else(|| {
                warn!(
                    "[LAUNCH] No valid handler found for {:?}->{:?}",
                    self.current_platform, target_platform
                );
                ProcessError::InvalidPlatform
            })?;
        info!("[LAUNCH] Selected handler for {:?}->{:?}", handler.0.0, handler.0.1);
        Ok(handler.1)
    }

    pub fn valid_platform(&self, platform: &Platform) -> bool {
        let db_lock = borrow_db_checked();
        let process_handler = self.fetch_process_handler(&db_lock, platform);
        process_handler.is_ok()
    }

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

        let launch_options = game_version
            .launches
            .iter()
            .filter(|v| v.platform == meta.target_platform)
            .map(|v| LaunchOption {
                name: v.name.clone(),
            })
            .collect::<Vec<LaunchOption>>();

        Ok(launch_options)
    }

    pub fn launch_process(
        &mut self,
        game_id: String,
        launch_process_index: usize,
    ) -> Result<(), ProcessError> {
        // Helper macro to emit debug events to the frontend console
        macro_rules! emit_dbg {
            ($step:expr, $($key:expr => $val:expr),+ $(,)?) => {
                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                    "step": $step,
                    "game_id": &game_id,
                    $($key: $val),+
                }));
                info!(concat!("[LAUNCH:{}] ", $step), &game_id);
            };
        }

        if self.processes.contains_key(&game_id) {
            return Err(ProcessError::AlreadyRunning);
        }

        let mut db_lock = borrow_db_mut_checked();

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

        let game_log_folder = &self.get_log_dir(&game_id);
        create_dir_all(game_log_folder)?;

        let current_time = chrono::offset::Local::now();
        let log_path = game_log_folder.join(format!(
            "{}-{}.log",
            &meta.version,
            current_time.timestamp()
        ));
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
            "install_type": format!("{:?}", match game_status {
                GameDownloadStatus::Installed { install_type, .. } => format!("{:?}", install_type),
                other => format!("{:?}", other),
            }),
            "launch_template": &game_version.user_configuration.launch_template,
            "override_proton_path": &game_version.user_configuration.override_proton_path,
            "all_launches": game_version.launches.iter().map(|l| {
                serde_json::json!({
                    "name": &l.name,
                    "platform": format!("{:?}", l.platform),
                    "command": &l.command,
                    "has_emulator": l.emulator.is_some(),
                    "emulator_game_id": l.emulator.as_ref().map(|e| &e.game_id),
                    "emulator_version_id": l.emulator.as_ref().map(|e| &e.version_id),
                    "emulator_launch_id": l.emulator.as_ref().map(|e| &e.launch_id),
                })
            }).collect::<Vec<_>>(),
        }));
        info!(
            "[LAUNCH] Game {:?} — target_platform={:?}, version={:?}, install_dir={:?}",
            &game_id, target_platform, version_name, install_dir
        );

        // Set to true when NeedsCompat fallback fires — we correct stored
        // platform metadata after the database lock is released.
        let mut needs_platform_correction = false;

        // ── STEP 2: Select launch config ───────────────────────────────────
        let (target_command, emulator, disc_paths) = match game_status {
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::Installed,
                ..
            } => {
                let matching_launches: Vec<_> = game_version
                    .launches
                    .iter()
                    .filter(|v| v.platform == target_platform)
                    .collect();
                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                    "step": "2_launch_config_filter",
                    "game_id": &game_id,
                    "filter_platform": format!("{:?}", target_platform),
                    "matching_count": matching_launches.len(),
                    "matching_names": matching_launches.iter().map(|l| &l.name).collect::<Vec<_>>(),
                    "requested_index": launch_process_index,
                }));

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
                    "emulator_game_id": launch_config.emulator.as_ref().map(|e| &e.game_id),
                    "disc_paths": &launch_config.disc_paths,
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
            _ => unreachable!("Game registered as 'Partially Installed'"),
        };

        let mut target_command = ParsedCommand::parse(target_command)?;

        // ── STEP 3: Handler selection ──────────────────────────────────────
        let handler_target_platform = emulator
            .and_then(|e| db_lock.applications.installed_game_version.get(&e.game_id))
            .map(|m| m.target_platform)
            .unwrap_or(target_platform);

        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "3_handler_selection",
            "game_id": &game_id,
            "game_target_platform": format!("{:?}", target_platform),
            "handler_target_platform": format!("{:?}", handler_target_platform),
            "has_emulator": emulator.is_some(),
            "emulator_overrides_platform": emulator.is_some() && handler_target_platform != target_platform,
            "current_platform": format!("{:?}", self.current_platform),
        }));

        let process_handler = self.fetch_process_handler(&db_lock, &handler_target_platform)?;

        // Track the effective working directory — for emulator launches this
        // must be the *emulator's* install dir so that relative paths in the
        // emulator command (e.g. `cores/snes9x_libretro.dll`) resolve correctly.
        let mut effective_cwd: Option<String> = None;
        // ROM path for the game being launched (used later for RetroArch config).
        // Only set for emulator-based launches.
        let mut emulator_rom_path: Option<String> = None;

        // ── STEP 4: Build launch command ───────────────────────────────────
        let target_launch_string = if let Some(emulator) = emulator {
            let err = ProcessError::RequiredDependency(
                emulator.game_id.clone(),
                emulator.version_id.clone(),
            );

            let emulator_metadata = db_lock
                .applications
                .installed_game_version
                .get(&emulator.game_id)
                .ok_or(err.clone())?;

            let emulator_game_status = db_lock
                .applications
                .game_statuses
                .get(&emulator.game_id)
                .ok_or(err.clone())?;

            let emulator_install_dir = match emulator_game_status {
                GameDownloadStatus::Installed {
                    install_type: InstalledGameType::Installed,
                    install_dir,
                    ..
                } => Ok(install_dir),
                GameDownloadStatus::Installed {
                    install_type: InstalledGameType::SetupRequired,
                    ..
                } => Err(ProcessError::InvalidArguments(
                    "Complete emulator setup before launching games that use it.".to_string(),
                )),
                _ => Err(err.clone()),
            }?;

            effective_cwd = Some(emulator_install_dir.clone());

            let emulator_game_version = db_lock
                .applications
                .game_versions
                .get(&emulator.version_id)
                .ok_or(err.clone())?;

            let emulator_launch_config = emulator_game_version
                .launches
                .iter()
                .find(|v| v.launch_id == emulator.launch_id)
                .ok_or(err)?;

            let mut exe_command = ParsedCommand::parse(emulator_launch_config.command.clone())?;
            exe_command.env.extend(std::mem::take(&mut target_command.env));
            exe_command.make_absolute(emulator_install_dir.into());

            target_command.make_absolute(PathBuf::from(install_dir.clone()));

            let rom_path = if disc_paths.len() > 1 {
                let game_dir = std::path::Path::new(install_dir);
                crate::m3u::cleanup_m3u(game_dir);
                let m3u_path = crate::m3u::generate_m3u(game_dir, &game_id, &disc_paths)?;
                m3u_path.to_string_lossy().to_string()
            } else {
                target_command.command.clone()
            };
            emulator_rom_path = Some(rom_path.clone());

            let mut has_rom_placeholder = false;
            for arg in &mut exe_command.args {
                if arg.contains("{rom}") {
                    *arg = arg.replace("{rom}", &rom_path);
                    has_rom_placeholder = true;
                }
            }
            if exe_command.command.contains("{rom}") {
                exe_command.command = exe_command.command.replace("{rom}", &rom_path);
                has_rom_placeholder = true;
            }

            let mut auto_core_path: Option<String> = None;
            if !has_rom_placeholder && !rom_path.is_empty() {
                let has_core_flag = exe_command.args.iter().any(|a| a == "-L" || a.starts_with("--libretro"));
                if !has_core_flag {
                    if let Some(core_path) = remote::retroarch::resolve_core_for_rom(
                        std::path::Path::new(&emulator_install_dir),
                        &rom_path,
                    ) {
                        auto_core_path = Some(core_path.to_string_lossy().to_string());
                        exe_command.args.push("-L".to_string());
                        exe_command.args.push(core_path.to_string_lossy().to_string());
                    }
                }
                exe_command.args.push(rom_path.clone());
            }

            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "4_emulator_build",
                "game_id": &game_id,
                "emulator_game_id": &emulator.game_id,
                "emulator_install_dir": &emulator_install_dir,
                "emulator_command_raw": &emulator_launch_config.command,
                "rom_path": &rom_path,
                "has_rom_placeholder": has_rom_placeholder,
                "auto_core_path": &auto_core_path,
                "final_exe_command": &exe_command.command,
                "final_exe_args": &exe_command.args,
                "final_exe_env": &exe_command.env,
            }));

            let reconstructed = exe_command.reconstruct();

            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "4_emulator_reconstructed",
                "game_id": &game_id,
                "reconstructed_command": &reconstructed,
            }));

            process_handler.create_launch_process(
                emulator_metadata,
                reconstructed,
                emulator_game_version,
                emulator_install_dir,
                &db_lock,
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
                Ok(s) => {
                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "4_handler_result_ok",
                        "game_id": &game_id,
                        "handler_output": &s,
                    }));
                    s
                },
                Err(ProcessError::NeedsCompat(ref _binary)) => {
                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "4_needs_compat_fallback",
                        "game_id": &game_id,
                        "binary": _binary,
                    }));
                    warn!(
                        "NeedsCompat for {:?} — falling through to Windows handler",
                        _binary
                    );
                    let compat = self.fetch_process_handler(&db_lock, &Platform::Windows)
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
                        win_launch_cmd.clone(),
                        game_version,
                        install_dir,
                        &db_lock,
                    )?;

                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "4_compat_result",
                        "game_id": &game_id,
                        "compat_command": &win_launch_cmd,
                        "compat_output": &result,
                    }));

                    needs_platform_correction = true;
                    result
                }
                Err(e) => {
                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "4_handler_error",
                        "game_id": &game_id,
                        "error": format!("{}", e),
                    }));
                    return Err(e);
                },
            }
        };

        // ── STEP 5: Format through launch template ─────────────────────────
        let working_dir = effective_cwd.as_deref().unwrap_or(install_dir);

        let mut parsed_launch = ParsedCommand::parse(target_launch_string.clone())?;
        let executable_name = parsed_launch.command.clone();
        let working_dir_owned = working_dir.to_string();
        let game_install_dir_owned = install_dir.to_string();
        parsed_launch.make_absolute(working_dir.into());

        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "5_pre_template",
            "game_id": &game_id,
            "handler_output_raw": &target_launch_string,
            "parsed_command": &parsed_launch.command,
            "parsed_args": &parsed_launch.args,
            "parsed_env": &parsed_launch.env,
            "working_dir": &working_dir_owned,
            "effective_cwd": &effective_cwd,
            "launch_template": &game_version.user_configuration.launch_template,
        }));

        let format_args = DropFormatArgs::new(
            target_launch_string,
            install_dir,
            &executable_name,
            parsed_launch.command,
            None,
        );

        let target_launch_string = SimpleCurlyFormat
            .format(
                &game_version.user_configuration.launch_template,
                &format_args,
            )
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

        // Clone user config before dropping the DB lock (needed for RetroArch setup below)
        let user_configuration = game_version.user_configuration.clone();

        drop(db_lock);

        if needs_platform_correction {
            let mut db_w = borrow_db_mut_checked();
            if let Some(stored) = db_w
                .applications
                .installed_game_version
                .get_mut(&game_id)
            {
                stored.target_platform = Platform::Windows;
                info!(
                    "Corrected target_platform for {} to Windows",
                    &game_id
                );
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
            "working_dir_exists": std::path::Path::new(&working_dir_owned).exists(),
            "needs_platform_correction": needs_platform_correction,
        }));

        info!(
            "launching (in {}): {:?}",
            launch_parameters.1.to_string_lossy(),
            launch_parameters.0
        );

        // Save command string and args/env before they're moved, so we can
        // retry with bash wrapping if we hit ENOEXEC.
        let spawn_executable = launch_parameters.0.command.clone();
        let spawn_args = launch_parameters.0.args.clone();
        let spawn_env = launch_parameters.0.env.clone();

        // On Linux, scripts installed via pip/pipx (like umu-run) can fail
        // with ENOEXEC when executed directly via execvp if their shebang
        // references an interpreter not on the restricted Gaming Mode PATH.
        // Detect scripts by reading the first two bytes and wrap them in a
        // shell invocation so bash resolves the shebang correctly.
        #[cfg(target_os = "linux")]
        let is_script = match std::fs::File::open(&launch_parameters.0.command) {
            Ok(mut f) => {
                use std::io::Read as _;
                let mut magic = [0u8; 2];
                match f.read_exact(&mut magic) {
                    Ok(()) => {
                        let result = magic == [b'#', b'!'];
                        info!(
                            "[LAUNCH] Script detection for {:?}: magic=[0x{:02x}, 0x{:02x}], is_script={}",
                            &launch_parameters.0.command, magic[0], magic[1], result
                        );
                        result
                    }
                    Err(e) => {
                        warn!("[LAUNCH] Script detection: failed to read magic bytes from {:?}: {}", &launch_parameters.0.command, e);
                        false
                    }
                }
            }
            Err(e) => {
                warn!("[LAUNCH] Script detection: failed to open {:?}: {}", &launch_parameters.0.command, e);
                false
            }
        };
        #[cfg(not(target_os = "linux"))]
        let is_script = false;

        let mut command = {
            let mut command = if is_script {
                info!(
                    "[LAUNCH] Detected script executable, wrapping in bash: {}",
                    &launch_parameters.0.command
                );
                let mut cmd = Command::new("/bin/bash");
                // Build a single shell command string: /path/to/script arg1 arg2 ...
                let mut shell_cmd = shell_words::quote(&launch_parameters.0.command).to_string();
                for arg in &launch_parameters.0.args {
                    shell_cmd.push(' ');
                    shell_cmd.push_str(&shell_words::quote(arg));
                }
                cmd.args(["-c", &shell_cmd]);
                cmd
            } else {
                let mut cmd = Command::new(launch_parameters.0.command);
                cmd.args(launch_parameters.0.args);
                cmd
            };

            for env_str in launch_parameters.0.env {
                if let Some((key, value)) = env_str.split_once('=') {
                    command.env(key, value);
                }
            }
            command
        };

        command
            .stderr(error_file)
            .stdout(log_file)
            .env_remove("RUST_LOG")
            // Steam/Gamescope sets PYTHONHOME and PYTHONPATH to its own
            // bundled Python runtime. When umu-run (a system Python script)
            // inherits these, it fails with "No module named 'encodings'"
            // because it tries to load Steam's Python stdlib instead of
            // the system one. Clear these so umu-run uses system Python.
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .current_dir(launch_parameters.1);

        // ── Gamescope / Steam Deck env vars ─────────────────────────────
        // When running inside Gamescope (SteamOS Game Mode), pass through
        // display-related env vars so games render correctly in the
        // compositor. Also enable steam-game-mode integration.
        #[cfg(target_os = "linux")]
        {
            use ::client::app_state::SessionType;
            let session = SessionType::detect();
            if session == SessionType::Gamescope {
                // Pass through Gamescope display vars so Proton/Wine can
                // find the correct Wayland/X11 display
                for var in &[
                    "GAMESCOPE_WAYLAND_DISPLAY",
                    "DISPLAY",
                    "WAYLAND_DISPLAY",
                    "XDG_RUNTIME_DIR",
                ] {
                    if let Ok(val) = std::env::var(var) {
                        command.env(var, val);
                    }
                }

                // Remove Steam's LD_PRELOAD — it injects the 32-bit
                // gameoverlayrenderer.so into 64-bit processes, which
                // fails and can interfere with video surface creation.
                command.env_remove("LD_PRELOAD");

                // Enable Steam Game Mode integration for Proton games
                // This tells the game/Proton that we're in a "Steam-like" session
                command.env("SteamGameId", &game_id);
                command.env("SteamAppId", &game_id);

                // Tell Proton/Wine the target resolution. Without this,
                // some games default to tiny resolutions (e.g. 320x200)
                // because Proton doesn't know the display size. The
                // Steam Deck display is 1280x800; desktop Gamescope
                // sessions may differ but 1280x800 is a safe default.
                command.env("STEAM_DISPLAY", ":1");
                // SCREEN_WIDTH/HEIGHT are read by some Wine/Proton builds
                // to set the virtual desktop size when no display info is
                // available. Gamescope composites everything fullscreen so
                // this just ensures the game picks a reasonable resolution.
                if std::env::var("SCREEN_WIDTH").is_err() {
                    command.env("SCREEN_WIDTH", "1280");
                }
                if std::env::var("SCREEN_HEIGHT").is_err() {
                    command.env("SCREEN_HEIGHT", "800");
                }
            }
        }

        // ── MangoHud performance overlay (Linux only) ───────────────────
        #[cfg(target_os = "linux")]
        {
            use database::models::data::MangoHudPreset;
            // Per-game setting takes priority; fall back to global setting from Settings
            let effective_preset = user_configuration.mangohud.clone().or_else(|| {
                borrow_db_checked().settings.global_mangohud.clone()
            });
            if let Some(preset) = &effective_preset {
                match preset {
                    MangoHudPreset::Off => {}
                    MangoHudPreset::Minimal => {
                        command.env("MANGOHUD", "1");
                        command.env("MANGOHUD_CONFIG", "fps,no_display");
                    }
                    MangoHudPreset::Standard => {
                        command.env("MANGOHUD", "1");
                        command.env("MANGOHUD_CONFIG", "fps,frametime,cpu_stats,gpu_stats,ram,vram");
                    }
                    MangoHudPreset::Full => {
                        command.env("MANGOHUD", "1");
                        // Full uses MangoHud's default config (shows everything)
                    }
                }
            }
        }

        process_handler.modify_command(&mut command);

        // Detect Steam emulator type and configure saves. Returns EmulatorInfo
        // describing which emulator (Goldberg vs SSE) and where saves go.
        let display_name = remote::cache::get_cached_object::<::client::user::User>("user")
            .ok()
            .map(|u| u.display_name().to_string());
        let emulator_info = remote::goldberg::configure_saves_for_game(
            &game_install_dir_owned,
            display_name.as_deref(),
        );

        // If the game uses an emulator, configure RetroArch (if applicable).
        // This patches retroarch.cfg with correct absolute paths for cores,
        // saves, system BIOS, and controller autoconfig.
        // Also fetch RA Connect credentials so RetroArch can authenticate
        // with RetroAchievements automatically (no manual login needed).
        // ── STEP 7: RetroArch configuration ─────────────────────────────
        if let Some(emu_dir) = &effective_cwd {
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "7_retroarch_config_start",
                "game_id": &game_id,
                "emu_dir": emu_dir,
            }));

            // Fetch RA credentials with a tight timeout so slow network
            // doesn't delay game launch. RA auto-login is nice-to-have,
            // not a blocker — the user can always log in manually.
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
                &game_id,
                ra_creds.as_ref(),
                Some(&user_configuration),
                emulator_rom_path.as_deref(),
            );

            // Dump the written retroarch.cfg to frontend for debugging
            let cfg_path = std::path::Path::new(emu_dir).join("retroarch.cfg");
            let cfg_content = std::fs::read_to_string(&cfg_path).unwrap_or_else(|e| {
                format!("[ERROR reading {}: {}]", cfg_path.display(), e)
            });
            // Extract key lines for Gamescope/controller/video debugging
            let debug_lines: Vec<&str> = cfg_content
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    t.starts_with("video_fullscreen")
                        || t.starts_with("video_windowed")
                        || t.starts_with("video_driver")
                        || t.starts_with("input_joypad_driver")
                        || t.starts_with("input_autodetect")
                        || t.starts_with("libretro_directory")
                        || t.starts_with("menu_driver")
                        || t.starts_with("savefile_directory")
                        || t.starts_with("joypad_autoconfig_dir")
                })
                .collect();
            // Check for AppImage.home config too
            let appimage_home_cfg = std::fs::read_dir(emu_dir)
                .ok()
                .and_then(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .find(|e| {
                            let n = e.file_name().to_string_lossy().to_lowercase();
                            n.contains("retroarch") && n.ends_with(".appimage")
                        })
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            std::path::Path::new(emu_dir)
                                .join(format!("{}.home", name))
                                .join(".config")
                                .join("retroarch")
                                .join("retroarch.cfg")
                        })
                });
            let appimage_cfg_exists = appimage_home_cfg
                .as_ref()
                .map(|p| p.exists())
                .unwrap_or(false);

            // Also read key lines from the AppImage.home config for debugging
            let appimage_debug_lines: Vec<String> = appimage_home_cfg
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|content| {
                    content
                        .lines()
                        .filter(|l| {
                            let t = l.trim();
                            t.starts_with("video_fullscreen")
                                || t.starts_with("video_windowed")
                                || t.starts_with("video_driver")
                                || t.starts_with("video_context")
                                || t.starts_with("input_joypad_driver")
                                || t.starts_with("input_autodetect")
                                || t.starts_with("libretro_directory")
                                || t.starts_with("menu_driver")
                                || t.starts_with("savefile_directory")
                                || t.starts_with("joypad_autoconfig_dir")
                        })
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();

            let bios_warnings: Vec<String> = retroarch_info
                .as_ref()
                .map(|info| info.bios_warnings.clone())
                .unwrap_or_default();

            let crt_shader_path: Option<String> = retroarch_info
                .as_ref()
                .and_then(|info| info.crt_shader_path.clone());

            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "7_retroarch_config_result",
                "game_id": &game_id,
                "cfg_path": cfg_path.display().to_string(),
                "cfg_exists": cfg_path.exists(),
                "retroarch_detected": retroarch_info.is_some(),
                "has_ra_credentials": ra_creds.is_some(),
                "key_settings": debug_lines,
                "cfg_line_count": cfg_content.lines().count(),
                "appimage_home_cfg": appimage_home_cfg.as_ref().map(|p| p.display().to_string()),
                "appimage_home_cfg_exists": appimage_cfg_exists,
                "appimage_home_key_settings": appimage_debug_lines,
                "bios_warnings": bios_warnings,
                "crt_shader_path": crt_shader_path,
            }));

            // ── Inject --appendconfig so RetroArch actually reads our config ──
            // The RetroArch AppImage overrides $HOME to its own .home directory,
            // so RetroArch reads config from $HOME/.config/retroarch/retroarch.cfg
            // instead of the file Drop writes in the emulator directory.
            // --appendconfig loads our settings ON TOP of the AppImage's defaults.
            if retroarch_info.is_some() && cfg_path.exists() {
                if is_script {
                    warn!("[LAUNCH] RetroArch is script-wrapped — cannot inject --appendconfig");
                } else {
                    info!(
                        "[LAUNCH] Injecting --appendconfig {}",
                        cfg_path.display()
                    );
                    command.arg("--appendconfig");
                    command.arg(cfg_path.as_os_str());
                }
                // Enable verbose logging so RetroArch dumps video driver
                // initialization to stderr — critical for diagnosing
                // "audio but no video" issues in Gamescope.
                command.arg("--verbose");
            }
        } else {
            let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                "step": "7_no_emulator",
                "game_id": &game_id,
                "reason": "effective_cwd is None (no emulator configured)",
            }));
        }

        // ── STEP 8: Spawn process ──────────────────────────────────────────
        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
            "step": "8_spawning",
            "game_id": &game_id,
            "command": &spawn_executable,
            "wrapped_in_bash": is_script,
        }));

        // Create a new process group for the child so we can send signals
        // to the entire process tree (bash → umu-run → proton → wine → game)
        // at once, rather than just the top-level process. This makes
        // kill_game terminate everything cleanly and quickly.
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
                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                    "step": "8_spawn_success",
                    "game_id": &game_id,
                    "pid": child.id(),
                }));
                child
            }
            Err(e) => {
                // ── ENOEXEC fallback ─────────────────────────────────────
                // If the initial spawn failed with ENOEXEC and we didn't
                // already wrap in bash, retry by invoking `/bin/bash -c`
                // with the full command string. This handles cases where
                // umu-run (or other pip-installed scripts) can't be
                // detected as scripts via magic bytes (e.g. symlinks,
                // compiled entry points, permission issues).
                #[cfg(target_os = "linux")]
                {
                    let is_enoexec = e.raw_os_error() == Some(8); // ENOEXEC
                    if is_enoexec && !is_script {
                        warn!(
                            "[LAUNCH] ENOEXEC on {:?} — retrying with bash wrapper",
                            &spawn_executable
                        );
                        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                            "step": "8_enoexec_retry",
                            "game_id": &game_id,
                            "original_error": format!("{}", e),
                            "executable": &spawn_executable,
                        }));

                        let mut shell_cmd = shell_words::quote(&spawn_executable).to_string();
                        for arg in &spawn_args {
                            shell_cmd.push(' ');
                            shell_cmd.push_str(&shell_words::quote(arg));
                        }

                        let mut retry_cmd = Command::new("/bin/bash");
                        retry_cmd.args(["-c", &shell_cmd]);
                        for env_str in &spawn_env {
                            if let Some((key, value)) = env_str.split_once('=') {
                                retry_cmd.env(key, value);
                            }
                        }
                        retry_cmd
                            .stderr(std::process::Stdio::from(
                                std::fs::OpenOptions::new()
                                    .create(true).append(true)
                                    .open(&error_log_path)
                                    .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap())
                            ))
                            .stdout(std::process::Stdio::from(
                                std::fs::OpenOptions::new()
                                    .create(true).append(true)
                                    .open(&log_path)
                                    .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap())
                            ))
                            .env_remove("RUST_LOG")
                            .current_dir(&working_dir_owned);

                        // Re-apply Gamescope env vars for the retry
                        #[cfg(target_os = "linux")]
                        {
                            let in_gamescope_retry = std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
                                || std::env::var("SteamGamepadUI").is_ok();
                            if in_gamescope_retry {
                                retry_cmd.env_remove("LD_PRELOAD");
                                for var in &[
                                    "DISPLAY", "WAYLAND_DISPLAY",
                                    "GAMESCOPE_WAYLAND_DISPLAY",
                                    "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS",
                                ] {
                                    if let Ok(val) = std::env::var(var) {
                                        retry_cmd.env(var, val);
                                    }
                                }
                                retry_cmd.env("SteamGameId", &game_id);
                                retry_cmd.env("SteamAppId", &game_id);
                            }
                        }

                        match retry_cmd.spawn() {
                            Ok(child) => {
                                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                                    "step": "8_enoexec_retry_success",
                                    "game_id": &game_id,
                                    "pid": child.id(),
                                }));
                                child
                            }
                            Err(retry_err) => {
                                let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                                    "step": "8_spawn_FAILED",
                                    "game_id": &game_id,
                                    "error": format!("{}", retry_err),
                                    "error_kind": format!("{:?}", retry_err.kind()),
                                    "executable": &spawn_executable,
                                    "executable_exists": std::path::Path::new(&spawn_executable).exists(),
                                    "was_enoexec_retry": true,
                                }));
                                return Err(retry_err.into());
                            }
                        }
                    } else {
                        let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                            "step": "8_spawn_FAILED",
                            "game_id": &game_id,
                            "error": format!("{}", e),
                            "error_kind": format!("{:?}", e.kind()),
                            "executable": &spawn_executable,
                            "executable_exists": std::path::Path::new(&spawn_executable).exists(),
                        }));
                        return Err(e.into());
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ = self.app_handle.emit("launch_trace", serde_json::json!({
                        "step": "8_spawn_FAILED",
                        "game_id": &game_id,
                        "error": format!("{}", e),
                        "error_kind": format!("{:?}", e.kind()),
                        "executable": &spawn_executable,
                        "executable_exists": std::path::Path::new(&spawn_executable).exists(),
                    }));
                    return Err(e.into());
                }
            }
        };

        let launch_process_handle = Arc::new(SharedChild::new(child)?);

        // Start playtime session asynchronously — never block game launch.
        // The session ID is stored in a shared mutex so it's available when
        // the game exits (the stop code reads from the same mutex).
        let playtime_session_id = Arc::new(std::sync::Mutex::new(None::<String>));
        {
            let playtime_game_id = meta.id.clone();
            let session_id_slot = playtime_session_id.clone();
            tauri::async_runtime::spawn(async move {
                match remote::playtime::start_playtime(&playtime_game_id).await {
                    Ok(sid) => {
                        info!("Playtime session started: {}", sid);
                        if let Ok(mut slot) = session_id_slot.lock() {
                            *slot = Some(sid);
                        }
                    }
                    Err(e) => {
                        warn!("Could not start playtime session for {}: {e}", playtime_game_id);
                    }
                }
            });
        }

        {
            let mut db_lock = borrow_db_mut_checked();
            db_lock
                .applications
                .transient_statuses
                .insert(meta.clone(), ApplicationTransientStatus::Running {});
        }

        push_game_update(
            &self.app_handle,
            &meta.id,
            None,
            (None, Some(ApplicationTransientStatus::Running {})),
        );

        let wait_thread_handle = launch_process_handle.clone();
        let wait_thread_game_id = meta.clone();

        // Start achievement polling for this game
        let achievement_cancel = Arc::new(Notify::new());
        {
            let cancel = achievement_cancel.clone();
            let poll_game_id = meta.id.clone();
            let poll_emulator_info = emulator_info.clone();
            let poll_app_handle = self.app_handle.clone();
            tauri::async_runtime::spawn(async move {
                remote::achievements::poll_achievements(
                    poll_game_id,
                    poll_emulator_info,
                    cancel,
                    move |achievement| {
                        info!(
                            "Achievement unlocked: {} - {}",
                            achievement.title, achievement.description
                        );
                        // Emit event to Vue frontend for toast notification
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

        self.processes.insert(
            meta.id,
            RunningProcess {
                handle: wait_thread_handle,
                start: SystemTime::now(),
                manually_killed: false,
                playtime_session_id,
                achievement_poll_cancel: Some(achievement_cancel),
            },
        );
        spawn(move || {
            let result: Result<ExitStatus, std::io::Error> = launch_process_handle.wait();

            PROCESS_MANAGER
                .lock()
                .on_process_finish(wait_thread_game_id.id, result)
        });
        Ok(())
    }
}

pub trait ProcessHandler: Send + 'static {
    fn create_launch_process(
        &self,
        meta: &DownloadableMetadata,
        launch_command: String,
        game_version: &GameVersion,
        current_dir: &str,
        database: &Database,
    ) -> Result<String, ProcessError>;

    fn valid_for_platform(&self, db: &Database, target: &Platform) -> bool;

    fn modify_command(&self, command: &mut Command);
}
