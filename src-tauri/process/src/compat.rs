// Linux-only file

use std::{
    fs::{DirEntry, read_dir, read_to_string},
    io,
    path::PathBuf,
    sync::LazyLock,
};

use database::{borrow_db_checked, borrow_db_mut_checked};
use log::{info, warn};
use serde::Serialize;

static SEARCH_PATHS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut paths = vec!["/usr/share/steam/compatibilitytools.d/".to_owned()];

    if let Some(home_dir) = std::env::home_dir() {
        paths.push(
            home_dir
                .join(".steam/root/compatibilitytools.d/")
                .to_string_lossy()
                .to_string(),
        );
    }

    paths
});

pub fn read_proton_path(proton_path: PathBuf) -> Result<Option<ProtonPath>, io::Error> {
    let read_dir = read_dir(&proton_path)?
        .flatten()
        .collect::<Vec<DirEntry>>();
    let has_proton_path = read_dir
        .iter()
        .find(|v| v.file_name().to_string_lossy() == "proton")
        .is_some();
    if !has_proton_path {
        return Ok(None);
    };

    let compat_vdf = read_dir
        .iter()
        .find(|v| v.file_name().to_string_lossy() == "compatibilitytool.vdf");

    let compat_vdf = match compat_vdf {
        Some(v) => v,
        None => return Ok(None),
    };

    let compat_vdf = read_to_string(compat_vdf.path())?;
    let compat_vdf = keyvalues_parser::parse(&compat_vdf)
        .inspect_err(|err| warn!("failed to parse vdf: {:?}", err))
        .map_err(|err| io::Error::other(err.to_string()))?;

    // Function was made with a lot of trial and error
    // Not intended to be readable
    let get_display_name = || -> Option<String> {
        let compat_tools = compat_vdf.value.unwrap_obj();
        let compat_tools = compat_tools.values().next()?.iter().next()?;
        let compat_tools = compat_tools.get_obj().unwrap();
        let compat_tools = compat_tools.values().next()?.iter().next()?.get_obj()?;
        let display_name = compat_tools.get("display_name")?.iter().next()?.get_str()?;
        Some(display_name.to_string())
    };

    if let Some(display_name) = get_display_name() {
        return Ok(Some(ProtonPath {
            path: proton_path.to_string_lossy().to_string(),
            name: display_name,
        }));
    }

    Ok(None)
}

pub fn discover_proton_paths() -> Result<Vec<ProtonPath>, io::Error> {
    let mut results = Vec::new();

    for search_path in &*SEARCH_PATHS {
        if let Ok(potential_dirs) = read_dir(search_path) {
            for proton_path in potential_dirs {
                if let Some(proton) = read_proton_path(proton_path?.path())? {
                    results.push(proton);
                }
            }
        }
    }

    Ok(results)
}

#[derive(Serialize)]
pub struct ProtonPath {
    pub path: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct ProtonPaths {
    pub autodiscovered: Vec<ProtonPath>,
    pub custom: Vec<ProtonPath>,
    pub default: Option<String>,
}

#[tauri::command]
pub async fn fetch_proton_paths() -> Result<ProtonPaths, String> {
    let autodiscovered = discover_proton_paths().map_err(|v| v.to_string())?;

    let db_lock = borrow_db_checked();

    let custom = db_lock
        .applications
        .additional_proton_paths
        .iter()
        .flat_map(|v| read_proton_path(PathBuf::from(v)))
        .flatten()
        .collect::<Vec<ProtonPath>>();

    let default = db_lock.applications.default_proton_path.clone();

    Ok(ProtonPaths {
        autodiscovered,
        custom,
        default,
    })
}

#[tauri::command]
pub fn add_proton_layer(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);

    let proton_layer = read_proton_path(path)
        .map_err(|err| err.to_string())?
        .ok_or("Unable to detect Proton at selected path.".to_owned())?;

    let mut db = borrow_db_mut_checked();
    db.applications
        .additional_proton_paths
        .push(proton_layer.path);

    Ok(())
}

#[tauri::command]
pub async fn remove_proton_layer(index: usize) {
    let mut db = borrow_db_mut_checked();
    let deleted = db.applications.additional_proton_paths.try_remove(index);
    if let Some(deleted) = deleted
        && let Some(default_path) = &db.applications.default_proton_path
        && *default_path == deleted {
            db.applications.default_proton_path = None;
        }
}

#[tauri::command]
pub async fn set_default(path: String) -> Result<(), String> {
    let proton_paths = fetch_proton_paths().await?;

    let valid = proton_paths
        .autodiscovered
        .iter()
        .find(|v| v.path == path)
        .or(proton_paths.custom.iter().find(|v| v.path == path))
        .is_some();

    if !valid {
        return Err("Invalid default Proton path.".to_string());
    }

    let mut db_lock = borrow_db_mut_checked();
    db_lock.applications.default_proton_path = Some(path);

    Ok(())
}

/// Attempt to install umu-launcher via pipx (preferred) or pip --user (fallback).
/// Returns a success message with the installed path, or an error string.
#[tauri::command]
pub async fn install_umu() -> Result<String, String> {
    use std::process::{Command, Stdio};

    /// Build a clean command that removes Python env vars set by AppImage/Steam.
    fn clean_command(program: &str) -> Command {
        let mut cmd = Command::new(program);
        cmd.env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }

    // Strategy 1: pipx install (preferred, isolates dependencies)
    info!("[UMU-INSTALL] Trying pipx install umu-launcher...");
    let pipx_result = clean_command("pipx")
        .args(["install", "umu-launcher"])
        .output();

    if let Ok(output) = pipx_result {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        info!("[UMU-INSTALL] pipx stdout: {stdout}");
        info!("[UMU-INSTALL] pipx stderr: {stderr}");

        if output.status.success() || stderr.contains("already") {
            // Refresh the cached executable path
            let path = client::compat::get_umu_executable_fresh();
            if let Some(p) = &path {
                return Ok(format!("umu-launcher installed successfully via pipx at {}", p.display()));
            }
            return Ok("umu-launcher installed via pipx (restart Drop to detect)".to_string());
        }
    }

    // Strategy 2: pip install --user (fallback)
    info!("[UMU-INSTALL] pipx failed, trying pip install --user umu-launcher...");
    for pip_cmd in &["pip3", "pip"] {
        let pip_result = clean_command(pip_cmd)
            .args(["install", "--user", "umu-launcher"])
            .output();

        if let Ok(output) = pip_result {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!("[UMU-INSTALL] {pip_cmd} stdout: {stdout}");
            info!("[UMU-INSTALL] {pip_cmd} stderr: {stderr}");

            if output.status.success() {
                let path = client::compat::get_umu_executable_fresh();
                if let Some(p) = &path {
                    return Ok(format!("umu-launcher installed via {pip_cmd} at {}", p.display()));
                }
                return Ok(format!("umu-launcher installed via {pip_cmd} (restart Drop to detect)"));
            }
        }
    }

    Err("Failed to install umu-launcher. Please install it manually:\n\
         • pipx install umu-launcher\n\
         • Or: pip install --user umu-launcher\n\
         • Or via your distro's package manager (e.g. pacman -S umu-launcher)"
        .to_string())
}

/// Diagnostic report for debugging launch issues on Steam Deck / Linux.
/// Returns a human-readable summary of the entire launch environment.
#[derive(Serialize)]
pub struct LaunchDiagnostics {
    pub umu_installed: bool,
    pub umu_path: Option<String>,
    pub proton_default: Option<String>,
    pub proton_default_valid: bool,
    pub proton_autodiscovered: Vec<String>,
    pub proton_custom: Vec<String>,
    pub gamescope_detected: bool,
    pub session_type: String,
    pub env_display: Option<String>,
    pub env_wayland: Option<String>,
    pub env_gamescope: Option<String>,
    pub env_xdg_runtime: Option<String>,
    pub installed_games: Vec<GameDiagnostic>,
}

#[derive(Serialize)]
pub struct GameDiagnostic {
    pub game_id: String,
    pub target_platform: String,
    pub version: String,
    pub install_dir: Option<String>,
    pub has_emulator: bool,
}

#[tauri::command]
pub async fn diagnose_launch_environment() -> Result<LaunchDiagnostics, String> {
    use client::compat::{COMPAT_INFO, UMU_LAUNCHER_EXECUTABLE};

    let umu_installed = COMPAT_INFO
        .as_ref()
        .map(|c| c.umu_installed)
        .unwrap_or(false);
    let umu_path = UMU_LAUNCHER_EXECUTABLE
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    let db = borrow_db_checked();
    let proton_default = db.applications.default_proton_path.clone();
    let proton_default_valid = proton_default
        .as_ref()
        .and_then(|p| read_proton_path(PathBuf::from(p)).ok())
        .flatten()
        .is_some();

    let proton_autodiscovered = discover_proton_paths()
        .unwrap_or_default()
        .into_iter()
        .map(|p| format!("{} ({})", p.name, p.path))
        .collect();

    let proton_custom = db
        .applications
        .additional_proton_paths
        .iter()
        .flat_map(|v| read_proton_path(PathBuf::from(v)))
        .flatten()
        .map(|p| format!("{} ({})", p.name, p.path))
        .collect();

    let gamescope_detected = std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
        || std::env::var("SteamGamepadUI").is_ok();

    let session_type = if gamescope_detected {
        "Gamescope (Gaming Mode)".to_string()
    } else if std::env::var("WAYLAND_DISPLAY").is_ok() {
        "Wayland (Desktop)".to_string()
    } else if std::env::var("DISPLAY").is_ok() {
        "X11 (Desktop)".to_string()
    } else {
        "Unknown".to_string()
    };

    let installed_games = db
        .applications
        .installed_game_version
        .iter()
        .map(|(game_id, meta)| {
            let install_dir = db
                .applications
                .game_statuses
                .get(game_id)
                .and_then(|s| match s {
                    database::GameDownloadStatus::Installed { install_dir, .. } => {
                        Some(install_dir.clone())
                    }
                    _ => None,
                });
            let has_emulator = db
                .applications
                .game_versions
                .get(&meta.version)
                .and_then(|gv| {
                    gv.launches
                        .iter()
                        .find(|l| l.platform == meta.target_platform)
                })
                .and_then(|l| l.emulator.as_ref())
                .is_some();

            GameDiagnostic {
                game_id: game_id.clone(),
                target_platform: format!("{:?}", meta.target_platform),
                version: meta.version.clone(),
                install_dir,
                has_emulator,
            }
        })
        .collect();

    let diag = LaunchDiagnostics {
        umu_installed,
        umu_path,
        proton_default,
        proton_default_valid,
        proton_autodiscovered,
        proton_custom,
        gamescope_detected,
        session_type,
        env_display: std::env::var("DISPLAY").ok(),
        env_wayland: std::env::var("WAYLAND_DISPLAY").ok(),
        env_gamescope: std::env::var("GAMESCOPE_WAYLAND_DISPLAY").ok(),
        env_xdg_runtime: std::env::var("XDG_RUNTIME_DIR").ok(),
        installed_games,
    };

    info!("[DIAGNOSTICS] UMU: installed={}, path={:?}", diag.umu_installed, diag.umu_path);
    info!("[DIAGNOSTICS] Proton: default={:?}, valid={}", diag.proton_default, diag.proton_default_valid);
    info!("[DIAGNOSTICS] Session: {}, gamescope={}", diag.session_type, diag.gamescope_detected);
    info!("[DIAGNOSTICS] Env: DISPLAY={:?}, WAYLAND={:?}, GAMESCOPE={:?}, XDG_RUNTIME={:?}",
        diag.env_display, diag.env_wayland, diag.env_gamescope, diag.env_xdg_runtime);

    Ok(diag)
}
