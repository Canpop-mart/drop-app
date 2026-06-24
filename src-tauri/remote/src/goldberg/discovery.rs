//! Steam API DLL discovery and emulator-type detection.
//!
//! A game ships a Steam-emulator build of `steam_api(64).dll` somewhere under
//! its install root. Drop locates that DLL and then decides which emulator it
//! is by looking at the sibling config files.

use log::{debug, info, warn};
use std::path::{Path, PathBuf};

use super::sse::parse_sse_ini;
use super::SteamEmulator;

/// The Steam API DLL names that Goldberg / SSE replace.
const STEAM_API_DLLS: &[&str] = &["steam_api64.dll", "steam_api.dll", "libsteam_api.so"];

/// The SSE config file name — always sits next to the Steam API DLL.
const SSE_INI_NAME: &str = "steam_emu.ini";

/// Recursively searches `root` for a Steam API DLL and returns the directory
/// containing it, or `None` if not found. Walks up to 8 levels deep — repacks
/// often double-nest the game folder and bury the DLL under
/// `…/<Game>_Data/Plugins/x86_64/`, which is already 5+ levels from the install
/// root.
pub fn find_steam_api_dir(root: &Path) -> Option<PathBuf> {
    // Fast path — DLL directly in the install root.
    for dll in STEAM_API_DLLS {
        if root.join(dll).exists() {
            return Some(root.to_path_buf());
        }
    }
    find_steam_api_dir_recursive(root, 0, 8)
}

fn find_steam_api_dir_recursive(dir: &Path, depth: u32, max_depth: u32) -> Option<PathBuf> {
    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(e) => e.flatten().collect(),
        Err(_) => return None,
    };

    // Check files in THIS directory first — at every depth we reach, including
    // max_depth itself, so a DLL sitting exactly at the limit isn't skipped (the
    // old `depth >= max_depth` guard ran before this check and dropped it).
    for entry in &entries {
        let path = entry.path();
        if path.is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_ascii_lowercase();
                if STEAM_API_DLLS.iter().any(|dll| lower == *dll) {
                    return Some(dir.to_path_buf());
                }
            }
    }

    // Recurse into subdirectories only while there's depth budget left.
    if depth >= max_depth {
        return None;
    }
    for entry in &entries {
        let path = entry.path();
        if path.is_dir()
            && let Some(found) = find_steam_api_dir_recursive(&path, depth + 1, max_depth) {
                return Some(found);
            }
    }
    None
}

/// Detects which Steam emulator is installed next to the DLL.
///
/// `steam_settings/` (Goldberg/GBE) takes priority over `steam_emu.ini` (SSE):
/// after an SSE → GBE upgrade both can exist, but the DLL is now GBE. When no
/// config exists yet, defaults to Goldberg (Drop will create `steam_settings/`).
pub fn detect_emulator_type(dll_dir: &Path) -> SteamEmulator {
    let dll_dir_str = dll_dir.to_string_lossy().to_string();

    let steam_settings = dll_dir.join("steam_settings");
    if steam_settings.is_dir() {
        info!("[EMU] Detected Goldberg at {}", steam_settings.display());
        return SteamEmulator::Goldberg { dll_dir: dll_dir_str };
    }

    let sse_ini = dll_dir.join(SSE_INI_NAME);
    if sse_ini.exists() {
        info!("[EMU] Detected SmartSteamEmu at {}", sse_ini.display());
        if let Some((app_id, save_path)) = parse_sse_ini(&sse_ini) {
            return SteamEmulator::SmartSteamEmu { dll_dir: dll_dir_str, save_path, app_id };
        }
        warn!("[EMU] Found steam_emu.ini but could not parse it");
    }

    debug!("[EMU] No emulator config at {}, defaulting to Goldberg", dll_dir.display());
    SteamEmulator::Goldberg { dll_dir: dll_dir_str }
}
