use std::{fs::create_dir_all, path::PathBuf, process::Command};

use client::compat::{COMPAT_INFO, UMU_LAUNCHER_EXECUTABLE};
use database::{
    Database, DownloadableMetadata, GameVersion, db::DATA_ROOT_DIR, platform::Platform,
};
use log::{debug, info, warn};

use crate::{error::ProcessError, parser::ParsedCommand, process_manager::ProcessHandler};

pub struct MacLauncher;
impl ProcessHandler for MacLauncher {
    fn create_launch_process(
        &self,
        _meta: &DownloadableMetadata,
        launch_command: String,
        _game_version: &GameVersion,
        _current_dir: &str,
        _database: &Database,
    ) -> Result<String, ProcessError> {
        Ok(launch_command)
    }

    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        true
    }

    fn modify_command(&self, _command: &mut Command) {}
}

#[allow(dead_code)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct WindowsLauncher;
impl ProcessHandler for WindowsLauncher {
    fn create_launch_process(
        &self,
        _meta: &DownloadableMetadata,
        launch_command: String,
        _game_version: &GameVersion,
        current_dir: &str,
        _database: &Database,
    ) -> Result<String, ProcessError> {
        // Make the exe path absolute using the game's install directory.
        // Windows does not search current_dir for executables when given a
        // relative path, so we must resolve it here. reconstruct() then
        // shell-quotes any spaces so the path survives re-parsing downstream.
        let mut parsed = ParsedCommand::parse(launch_command)?;
        parsed.make_absolute(PathBuf::from(current_dir));
        Ok(parsed.reconstruct())
    }

    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        true
    }

    #[allow(unused_variables)]
    fn modify_command(&self, command: &mut Command) {
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;
        #[cfg(target_os = "windows")]
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

pub struct NativeLauncher;
impl ProcessHandler for NativeLauncher {
    fn create_launch_process(
        &self,
        _meta: &DownloadableMetadata,
        launch_command: String,
        _game_version: &GameVersion,
        current_dir: &str,
        _database: &Database,
    ) -> Result<String, ProcessError> {
        info!("[NativeLauncher] create_launch_process: command={:?}, current_dir={:?}", &launch_command, current_dir);
        let mut parsed = ParsedCommand::parse(launch_command)?;
        parsed.make_absolute(PathBuf::from(current_dir));
        info!("[NativeLauncher] absolute command: {:?}", &parsed.command);
        info!("[NativeLauncher] command exists on disk: {}", std::path::Path::new(&parsed.command).exists());

        #[cfg(target_os = "linux")]
        {
            let cmd_lower = parsed.command.to_lowercase();
            let is_win_ext = cmd_lower.ends_with(".exe")
                || cmd_lower.ends_with(".bat")
                || cmd_lower.ends_with(".cmd");
            let is_pe_binary = if !is_win_ext {
                match std::fs::File::open(&parsed.command) {
                    Ok(mut f) => {
                        use std::io::Read;
                        let mut magic = [0u8; 2];
                        match f.read_exact(&mut magic) {
                            Ok(()) => {
                                info!("[NativeLauncher] PE check: magic bytes = [{:#04x}, {:#04x}]", magic[0], magic[1]);
                                magic == [0x4D, 0x5A]
                            }
                            Err(e) => {
                                warn!("[NativeLauncher] could not read magic bytes from {:?}: {}", &parsed.command, e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        warn!("[NativeLauncher] could not open {:?} for PE check: {}", &parsed.command, e);
                        false
                    }
                }
            } else {
                false
            };
            info!("[NativeLauncher] Windows check: is_win_ext={}, is_pe_binary={}", is_win_ext, is_pe_binary);
            if is_win_ext || is_pe_binary {
                info!("[NativeLauncher] → Returning NeedsCompat for {:?}", &parsed.command);
                return Err(ProcessError::NeedsCompat(parsed.command.clone()));
            }
        }

        let result = parsed.reconstruct();
        info!("[NativeLauncher] → Returning OK: {:?}", &result);
        Ok(result)
    }

    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        true
    }

    fn modify_command(&self, _command: &mut Command) {}
}

pub struct UMUNativeLauncher;
impl ProcessHandler for UMUNativeLauncher {
    fn create_launch_process(
        &self,
        meta: &DownloadableMetadata,
        launch_command: String,
        game_version: &GameVersion,
        _current_dir: &str,
        _database: &Database,
    ) -> Result<String, ProcessError> {
        let umu_id_override = game_version
            .launches
            .iter()
            .find(|v| v.platform == meta.target_platform)
            .and_then(|v| v.umu_id_override.as_ref())
            .map_or("", |v| v);

        let game_id = if umu_id_override.is_empty() {
            &game_version.version_id
        } else {
            umu_id_override
        };

        let pfx_dir = DATA_ROOT_DIR.join("pfx");
        let pfx_dir = pfx_dir.join(meta.id.clone());
        create_dir_all(&pfx_dir)?;

        Ok(format!(
            "GAMEID={game_id} UMU_NO_PROTON=1 WINEPREFIX={} {umu:?} {launch}",
            pfx_dir.to_string_lossy(),
            umu = UMU_LAUNCHER_EXECUTABLE
                .as_ref()
                .expect("Failed to get UMU_LAUNCHER_EXECUTABLE as ref"),
            launch = launch_command,
        ))
    }

    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        let Some(compat_info) = &*COMPAT_INFO else {
            return false;
        };
        compat_info.umu_installed
    }

    fn modify_command(&self, _command: &mut Command) {}
}

/// umu-launcher reserved shorthands. When PROTONPATH is set to one of these
/// strings, umu resolves it itself (downloading the latest GE-Proton /
/// UMU-Proton release into ~/.cache/umu-launcher/ on first run). We must
/// NOT try to validate these as filesystem paths — they aren't paths.
fn is_umu_proton_keyword(value: &str) -> bool {
    matches!(
        value,
        "GE-Proton" | "GE-Latest" | "UMU-Proton" | "UMU-Latest" | "Proton-GE" | "GE-Proton-Latest"
    )
}

/// Default Proton when the user has configured nothing. GE-Proton has the
/// DXVK/VKD3D/winetricks gamefixes most Windows titles need; umu will
/// auto-download it on first launch.
const DEFAULT_UMU_PROTON: &str = "GE-Proton";

pub struct UMUCompatLauncher;
impl ProcessHandler for UMUCompatLauncher {
    fn create_launch_process(
        &self,
        meta: &DownloadableMetadata,
        launch_command: String,
        game_version: &GameVersion,
        _current_dir: &str,
        database: &Database,
    ) -> Result<String, ProcessError> {
        let umu_id_override = game_version
            .launches
            .iter()
            .find(|v| v.platform == meta.target_platform)
            .and_then(|v| v.umu_id_override.as_ref())
            .map_or("", |v| v);

        let game_id = if umu_id_override.is_empty() {
            &game_version.version_id
        } else {
            umu_id_override
        };

        let pfx_dir = DATA_ROOT_DIR.join("pfx");
        let pfx_dir = pfx_dir.join(meta.id.clone());
        create_dir_all(&pfx_dir)?;

        // Resolve Proton in priority order: per-game override → global default
        // → GE-Proton (umu auto-downloads on first use).
        let proton_path: String = game_version
            .user_configuration
            .override_proton_path
            .clone()
            .or_else(|| database.applications.default_proton_path.clone())
            .unwrap_or_else(|| {
                info!(
                    "[UMUCompat] No Proton configured — falling back to {DEFAULT_UMU_PROTON}. \
                     umu-launcher will auto-download it on first launch."
                );
                DEFAULT_UMU_PROTON.to_string()
            });

        info!("[UMUCompat] Using Proton: {}", proton_path);

        // Keywords (GE-Proton, UMU-Proton, etc.) are resolved by umu itself;
        // skip filesystem validation for those. Real paths still get checked.
        if !is_umu_proton_keyword(&proton_path) {
            #[cfg(target_os = "linux")]
            let proton_valid = crate::compat::read_proton_path(PathBuf::from(&proton_path))
                .ok()
                .flatten()
                .is_some();
            #[cfg(not(target_os = "linux"))]
            let proton_valid = false;
            if !proton_valid {
                warn!(
                    "[UMUCompat] Proton path {:?} is invalid (missing proton binary or compatibilitytool.vdf)",
                    proton_path
                );
                return Err(ProcessError::NoCompat);
            }
        }
        let proton_env = format!("PROTONPATH={}", proton_path);
        info!("[UMUCompat] Proton valid. Building launch command...");

        let umu_exe = UMU_LAUNCHER_EXECUTABLE
            .as_ref()
            .expect("Failed to get UMU_LAUNCHER_EXECUTABLE as ref");

        let result = format!(
            "GAMEID={game_id} {} WINEPREFIX={} {umu:?} {launch}",
            proton_env,
            pfx_dir.to_string_lossy(),
            umu = umu_exe,
            launch = launch_command,
        );
        info!("[UMUCompat] → Final command: {}", &result);
        Ok(result)
    }

    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        let Some(compat_info) = &*COMPAT_INFO else {
            info!("[UMUCompat] valid_for_platform: COMPAT_INFO is None");
            return false;
        };
        info!("[UMUCompat] valid_for_platform: umu_installed={}", compat_info.umu_installed);
        compat_info.umu_installed
    }

    fn modify_command(&self, _command: &mut Command) {}
}

pub struct AsahiMuvmLauncher;
impl ProcessHandler for AsahiMuvmLauncher {
    fn create_launch_process(
        &self,
        meta: &DownloadableMetadata,
        launch_command: String,
        game_version: &GameVersion,
        current_dir: &str,
        database: &Database,
    ) -> Result<String, ProcessError> {
        let umu_launcher = UMUCompatLauncher {};
        let umu_string = umu_launcher.create_launch_process(
            meta,
            launch_command,
            game_version,
            current_dir,
            database,
        )?;
        let mut args_cmd = umu_string
            .split("umu-run")
            .collect::<Vec<&str>>()
            .into_iter();
        let args = args_cmd
            .next()
            .ok_or(ProcessError::InvalidArguments(umu_string.clone()))?
            .trim();
        let cmd = format!(
            "umu-run{}",
            args_cmd
                .next()
                .ok_or(ProcessError::InvalidArguments(umu_string.clone()))?
        );

        Ok(format!("{args} muvm -- {cmd}"))
    }

    #[allow(unreachable_code)]
    #[allow(unused_variables)]
    fn valid_for_platform(&self, _db: &Database, _target: &Platform) -> bool {
        #[cfg(not(target_os = "linux"))]
        return false;

        #[cfg(not(target_arch = "aarch64"))]
        return false;

        let page_size = page_size::get();
        if page_size != 16384 {
            return false;
        }

        let Some(compat_info) = &*COMPAT_INFO else {
            return false;
        };

        compat_info.umu_installed
    }

    fn modify_command(&self, _command: &mut Command) {}
}
