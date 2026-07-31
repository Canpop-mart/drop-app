//! Emulator-host management surface.
//!
//! The library groups the ROMs an emulator runs into "consoles", but it never
//! surfaces the emulator *host* itself — the RetroArch install that actually
//! backs those ROMs. Without it there's no way to find the install, drop a new
//! core in, or uninstall it. These commands list the installed emulator hosts
//! and open the folders you'd manage them from.
//!
//! An emulator host is just a Drop game whose install dir is an emulator:
//! either it's referenced as the `emulator` of some ROM's launch config, or its
//! files look like a RetroArch install (a `retroarch` executable / a `cores/`
//! directory). Detection is entirely client-side — emulator installs only exist
//! on the client — so there is no server endpoint to mirror.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use database::{GameDownloadStatus, borrow_db_checked};
use games::library::Game;
use log::warn;
use remote::{cache::get_cached_object, retroarch::discovery::is_retroarch};
use serde::Serialize;
use tauri::AppHandle;

/// One installed emulator host, as surfaced on the library's Emulators section
/// and its per-emulator management page.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorHost {
    /// The host game's id — the same id `uninstall_game` / `open_game_install_dir`
    /// take.
    pub id: String,
    /// Display name from the cached library metadata (e.g. "RetroArch").
    pub name: String,
    /// Absolute install directory on disk.
    pub install_dir: String,
    /// Object id of the host's icon, for `useObject()` in the UI (`""` if none).
    pub icon_object_id: String,
    /// True when the host is a RetroArch install. Gates the cores UI, since the
    /// `cores/` concept is RetroArch-specific.
    pub retroarch: bool,
    /// Core library filenames found in `<install>/cores/`. Empty for a
    /// non-RetroArch host, or a RetroArch install with no cores added yet.
    pub cores: Vec<String>,
}

/// List every installed emulator host.
///
/// Two signals identify a host, unioned: (1) it is referenced as the `emulator`
/// of some game's launch config (catches RetroArch, Ryujinx, Yuzu, Cemu, …), and
/// (2) its install dir looks like a RetroArch install (catches a RetroArch that
/// has no ROMs pointed at it yet). Only currently-installed games qualify.
#[tauri::command]
pub fn list_installed_emulators() -> Vec<EmulatorHost> {
    // Phase 1 — under the DB lock, gather every installed game's (id, dir) plus
    // whether it's referenced as an emulator. No disk I/O happens here, so the
    // lock is held only briefly; the filesystem probing runs after it's dropped.
    let installed: Vec<(String, String, bool)> = {
        let db = borrow_db_checked();
        let referenced: HashSet<String> = db
            .applications
            .game_versions
            .values()
            .flat_map(|v| v.launches.iter())
            .filter_map(|l| l.emulator.as_ref().map(|e| e.game_id.clone()))
            .collect();
        db.applications
            .game_statuses
            .iter()
            .filter_map(|(id, status)| match status {
                GameDownloadStatus::Installed { install_dir, .. } => {
                    Some((id.clone(), install_dir.clone(), referenced.contains(id)))
                }
                _ => None,
            })
            .collect()
    };

    // Phase 2 — probe the filesystem + resolve names with the lock released.
    let mut hosts: Vec<EmulatorHost> = Vec::new();
    for (id, install_dir, is_referenced) in installed {
        let dir = Path::new(&install_dir);
        let retroarch = is_retroarch(dir);
        if !is_referenced && !retroarch {
            continue;
        }
        let (name, icon_object_id) = get_cached_object::<Game>(&format!("game/{id}"))
            .map(|g| (g.m_name, g.m_icon_object_id))
            .unwrap_or_else(|_| ("Emulator".to_string(), String::new()));
        hosts.push(EmulatorHost {
            id,
            name,
            cores: list_core_files(dir),
            install_dir,
            icon_object_id,
            retroarch,
        });
    }
    hosts.sort_by_key(|h| h.name.to_lowercase());
    hosts
}

/// Open an installed emulator's `cores/` folder in the OS file manager so the
/// user can drop a libretro core in. Game-id-keyed (resolving the path from
/// `game_statuses`), mirroring `open_game_install_dir`. Best-effort-creates the
/// folder first so the button always lands somewhere useful, even on a
/// RetroArch install that has no `cores/` yet.
#[tauri::command]
pub fn open_emulator_cores_dir(game_id: String, app_handle: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    let install_dir = {
        let db = borrow_db_checked();
        match db.applications.game_statuses.get(&game_id) {
            Some(GameDownloadStatus::Installed { install_dir, .. }) => install_dir.clone(),
            _ => return Err("Emulator is not installed.".to_string()),
        }
    };

    let cores_dir = Path::new(&install_dir).join("cores");
    if let Err(e) = std::fs::create_dir_all(&cores_dir) {
        warn!(
            "[emulators] couldn't create cores dir {}: {e}",
            cores_dir.display()
        );
    }
    // Fall back to the install dir if the cores dir still isn't there, so the
    // button always opens something.
    let target: PathBuf = if cores_dir.is_dir() {
        cores_dir
    } else {
        PathBuf::from(&install_dir)
    };

    app_handle
        .opener()
        .open_path(target.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Failed to open cores folder: {e}"))
}

/// Core library filenames in `<install>/cores/` (`*.dll` / `*.so` / `*.dylib`),
/// sorted. Empty when there is no `cores/` directory.
fn list_core_files(install_dir: &Path) -> Vec<String> {
    let cores_dir = install_dir.join("cores");
    let Ok(entries) = std::fs::read_dir(&cores_dir) else {
        return Vec::new();
    };
    let mut cores: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| {
            let lower = name.to_lowercase();
            lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib")
        })
        .collect();
    cores.sort();
    cores
}
