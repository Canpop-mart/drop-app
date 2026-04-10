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
                process.handle.kill()?;
                let exit_status = process.handle.wait()?;
                info!("exit status: {:?}", exit_status);
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
        Ok(self
            .game_launchers
            .iter()
            .find(|e| {
                let (e_current, e_target) = e.0;
                e_current == self.current_platform
                    && e_target == *target_platform
                    && e.1.valid_for_platform(db_lock, target_platform)
            })
            .ok_or(ProcessError::InvalidPlatform)?
            .1)
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

        debug!(
            "Launching process {:?} with version {:?}",
            &game_id,
            db_lock.applications.game_versions.get(version_name)
        );

        let game_version = db_lock
            .applications
            .game_versions
            .get(version_name)
            .ok_or(ProcessError::InvalidVersion)?;

        let game_log_folder = &self.get_log_dir(&game_id);
        create_dir_all(game_log_folder)?;

        let current_time = chrono::offset::Local::now();
        let log_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .read(true)
            .create(true)
            .open(game_log_folder.join(format!(
                "{}-{}.log",
                &meta.version,
                current_time.timestamp()
            )))?;

        let error_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .read(true)
            .create(true)
            .open(game_log_folder.join(format!(
                "{}-{}-error.log",
                &meta.version,
                current_time.timestamp()
            )))?;

        let target_platform = meta.target_platform;

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

                (setup_config.command.clone(), None, Vec::new())
            }
            _ => unreachable!("Game registered as 'Partially Installed'"),
        };

        let mut target_command = ParsedCommand::parse(target_command)?;

        // Select handler based on emulator's platform if present, otherwise game's platform.
        // This ensures Linux AppImages run via NativeLauncher even when the game target is Windows.
        let handler_target_platform = emulator
            .and_then(|e| db_lock.applications.installed_game_version.get(&e.game_id))
            .map(|m| m.target_platform)
            .unwrap_or(target_platform);
        let process_handler = self.fetch_process_handler(&db_lock, &handler_target_platform)?;

        // Track the effective working directory — for emulator launches this
        // must be the *emulator's* install dir so that relative paths in the
        // emulator command (e.g. `cores/snes9x_libretro.dll`) resolve correctly.
        let mut effective_cwd: Option<String> = None;

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

            // Use the emulator's install dir as CWD so relative paths in the
            // emulator command (cores, configs, autoconfig) resolve correctly.
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
            // Move env vars instead of cloning, since target_command.env is not needed after this
            exe_command.env.extend(std::mem::take(&mut target_command.env));
            exe_command.make_absolute(emulator_install_dir.into());

            target_command.make_absolute(PathBuf::from(install_dir.clone()));

            // For multi-disc games, generate a .m3u playlist and use it as {rom}
            // instead of the single ROM path. Clean up any stale .m3u from previous runs.
            let rom_path = if disc_paths.len() > 1 {
                let game_dir = std::path::Path::new(install_dir);
                crate::m3u::cleanup_m3u(game_dir);
                let m3u_path = crate::m3u::generate_m3u(game_dir, &game_id, &disc_paths)?;
                m3u_path.to_string_lossy().to_string()
            } else {
                target_command.command.clone()
            };

            // Substitute {rom} placeholder in emulator args with the actual ROM path
            let mut has_rom_placeholder = false;
            for arg in &mut exe_command.args {
                if arg.contains("{rom}") {
                    *arg = arg.replace("{rom}", &rom_path);
                    has_rom_placeholder = true;
                }
            }
            // Also check the command itself for {rom}
            if exe_command.command.contains("{rom}") {
                exe_command.command = exe_command.command.replace("{rom}", &rom_path);
                has_rom_placeholder = true;
            }
            // If the emulator command has no {rom} placeholder at all,
            // auto-append the ROM path as the last argument so the emulator
            // knows which game to load (e.g. retroarch.exe <rom_path>).
            // For RetroArch, also auto-detect and inject the -L <core> flag.
            if !has_rom_placeholder && !rom_path.is_empty() {
                log::info!("Emulator command has no {{rom}} placeholder — auto-appending ROM path: {}", rom_path);

                // Check if this is RetroArch and no -L flag is present
                let has_core_flag = exe_command.args.iter().any(|a| a == "-L" || a.starts_with("--libretro"));
                if !has_core_flag {
                    if let Some(core_path) = remote::retroarch::resolve_core_for_rom(
                        std::path::Path::new(&emulator_install_dir),
                        &rom_path,
                    ) {
                        log::info!("Auto-detected RetroArch core: {}", core_path.display());
                        exe_command.args.push("-L".to_string());
                        exe_command.args.push(core_path.to_string_lossy().to_string());
                    }
                }

                exe_command.args.push(rom_path);
            }

            process_handler.create_launch_process(
                emulator_metadata,
                exe_command.reconstruct(),
                emulator_game_version,
                emulator_install_dir,
                &db_lock,
            )?
        } else {
            process_handler.create_launch_process(
                &meta,
                target_command.reconstruct(),
                game_version,
                install_dir,
                &db_lock,
            )?
        };

        // For emulator launches, use the emulator's dir as CWD; otherwise use the game's.
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

        // Clone user config before dropping the DB lock (needed for RetroArch setup below)
        let user_configuration = game_version.user_configuration.clone();

        // Drop the DB write lock before spawning — everything below only
        // needs owned data. We re-acquire briefly for the status update.
        drop(db_lock);

        let launch_parameters = LaunchParameters(
            ParsedCommand::parse(target_launch_string)?,
            working_dir_owned.clone().into(),
        );

        info!(
            "launching (in {}): {:?}",
            launch_parameters.1.to_string_lossy(),
            launch_parameters.0
        );

        let mut command = {
            let mut command = Command::new(launch_parameters.0.command);
            command.args(launch_parameters.0.args);
            for parts in launch_parameters
                .0
                .env
                .into_iter()
                .map(|e| e.split("=").map(|v| v.to_string()).collect::<Vec<String>>())
            {
                if let Some(key) = parts.first()
                    && let Some(value) = parts.get(1)
                {
                    command.env(key, value);
                }
            }
            command
        };

        command
            .stderr(error_file)
            .stdout(log_file)
            .env_remove("RUST_LOG")
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

                // Enable Steam Game Mode integration for Proton games
                // This tells the game/Proton that we're in a "Steam-like" session
                command.env("SteamGameId", &game_id);
                command.env("SteamAppId", &game_id);
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
            if let Some(ref preset) = effective_preset {
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
        if let Some(ref emu_dir) = effective_cwd {
            let ra_creds = tauri::async_runtime::block_on(
                remote::retroarch::fetch_ra_credentials(),
            );
            let _retroarch_info = remote::retroarch::configure_retroarch_for_game(
                emu_dir,
                &game_id,
                ra_creds.as_ref(),
                Some(&user_configuration),
            );
        }

        let child = command.spawn()?;

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
