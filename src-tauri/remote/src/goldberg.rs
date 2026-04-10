//! Steam emulator detection and local achievement file reader.
//!
//! Drop supports multiple Steam emulators that games may ship with:
//!
//! ## Goldberg / GBE
//! Uses `steam_settings/configs.user.ini` for configuration.
//! Drop writes `local_save_path=./drop-goldberg` to control where saves go.
//! Achievement unlocks are stored as JSON:
//!   `<dll_dir>/drop-goldberg/<AppID>/achievements.json`
//!
//! ## SmartSteamEmu (SSE / RUNE)
//! Uses `steam_emu.ini` next to the DLL for configuration.
//! Saves go to a fixed path (typically `%SystemDrive%\Users\Public\Documents\Steam\RUNE\<AppID>`).
//! Achievement format: `achievements.ini` with per-achievement sections.
//!
//! ## Detection
//! The emulator type is determined by checking for `steam_emu.ini` (SSE) vs
//! `steam_settings/` (Goldberg) next to the Steam API DLL.

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Emulator types ─────────────────────────────────────────────────────────

/// Which Steam emulator a game uses.
#[derive(Debug, Clone)]
pub enum SteamEmulator {
    /// Goldberg / GBE fork — uses `steam_settings/` + `achievements.json`
    Goldberg {
        /// Directory containing the Steam API DLL (where `drop-goldberg/` lives)
        dll_dir: String,
    },
    /// SmartSteamEmu (SSE/RUNE) — uses `steam_emu.ini`
    SmartSteamEmu {
        /// Directory containing the Steam API DLL + `steam_emu.ini`
        dll_dir: String,
        /// Where SSE stores game data (parsed from ini comment/config)
        save_path: PathBuf,
        /// Steam AppID parsed from the ini
        app_id: String,
    },
    /// Steam API DLL found but emulator type couldn't be determined
    Unknown {
        dll_dir: String,
    },
}

/// Result of detecting/configuring the emulator for a game.
#[derive(Debug, Clone)]
pub struct EmulatorInfo {
    pub emulator: SteamEmulator,
}

impl EmulatorInfo {
    /// Returns the DLL directory regardless of emulator type.
    pub fn dll_dir(&self) -> &str {
        match &self.emulator {
            SteamEmulator::Goldberg { dll_dir } => dll_dir,
            SteamEmulator::SmartSteamEmu { dll_dir, .. } => dll_dir,
            SteamEmulator::Unknown { dll_dir } => dll_dir,
        }
    }

    /// Returns the directory where achievement save files should be looked for,
    /// depending on the emulator type.
    pub fn achievement_search_dir(&self) -> Option<String> {
        match &self.emulator {
            SteamEmulator::Goldberg { dll_dir } => Some(dll_dir.clone()),
            SteamEmulator::SmartSteamEmu { save_path, .. } => {
                Some(save_path.to_string_lossy().to_string())
            }
            SteamEmulator::Unknown { .. } => None,
        }
    }
}

// ── Common achievement type ────────────────────────────────────────────────

/// A single achievement entry read from any emulator's save files.
/// Field names match the Goldberg JSON format for backward compat.
#[derive(Deserialize, Debug, Clone)]
pub struct GoldbergAchievement {
    pub name: String,
    #[serde(default)]
    pub earned: bool,
    #[serde(default)]
    pub earned_time: u64,
}

/// GBE's runtime format: a map keyed by achievement API name.
/// e.g. `{"Banish10Items": {"earned": false, "earned_time": 0}}`
#[derive(Deserialize, Serialize, Debug)]
struct GbeMapEntry {
    #[serde(default)]
    earned: bool,
    #[serde(default)]
    earned_time: u64,
}

/// Result of parsing a GBE achievements file.
enum ParseResult {
    /// GBE map format: `{"ACH_NAME": {"earned": false, ...}}`
    Map(Vec<GoldbergAchievement>),
    /// Server definitions array: `[{"name": "ACH_NAME", "displayName": "...", ...}]`
    /// This format is NOT what GBE writes — it means the server pre-populated the
    /// runtime file with definitions. Needs migration to map format.
    DefinitionsArray(Vec<GoldbergAchievement>),
    /// Could not parse
    Error(String),
}

/// Parses a Goldberg/GBE achievements.json, distinguishing between formats:
///   - Map:   `{"X": {"earned": true, "earned_time": 123}}` (GBE runtime saves)
///   - Array: `[{"name": "X", "earned": true, ...}]` (server-side definitions)
fn parse_gbe_achievements_typed(contents: &str) -> ParseResult {
    // Try map format first (GBE fork runtime saves — this is the correct format)
    if let Ok(map) = serde_json::from_str::<HashMap<String, GbeMapEntry>>(contents) {
        // Sanity check: JSON objects parse as maps, but arrays don't
        if !map.is_empty() || contents.trim_start().starts_with('{') {
            let achievements: Vec<GoldbergAchievement> = map
                .into_iter()
                .map(|(name, entry)| GoldbergAchievement {
                    name,
                    earned: entry.earned,
                    earned_time: entry.earned_time,
                })
                .collect();
            return ParseResult::Map(achievements);
        }
    }

    // Try array format (Goldberg classic / server definitions)
    if let Ok(achievements) = serde_json::from_str::<Vec<GoldbergAchievement>>(contents) {
        return ParseResult::DefinitionsArray(achievements);
    }

    ParseResult::Error("Could not parse as map or array format".to_string())
}

/// Backward-compatible wrapper that returns achievements regardless of format.
#[allow(dead_code)]
fn parse_gbe_achievements(contents: &str) -> Result<Vec<GoldbergAchievement>, String> {
    match parse_gbe_achievements_typed(contents) {
        ParseResult::Map(a) | ParseResult::DefinitionsArray(a) => Ok(a),
        ParseResult::Error(e) => Err(e),
    }
}

/// Converts a definitions-array achievements.json file to GBE's map format in place.
/// This is a migration for files that were incorrectly written by older server versions.
/// Returns true if the file was converted.
fn migrate_array_to_map_format(path: &std::path::Path, achievements: &[GoldbergAchievement]) -> bool {
    let mut map: HashMap<String, GbeMapEntry> = HashMap::new();
    for ach in achievements {
        map.insert(ach.name.clone(), GbeMapEntry {
            earned: ach.earned,
            earned_time: ach.earned_time,
        });
    }

    match serde_json::to_string_pretty(&map) {
        Ok(json) => match std::fs::write(path, &json) {
            Ok(_) => {
                info!(
                    "[ACH-GSE] MIGRATED {} from definitions array → GBE map format ({} achievements)",
                    path.display(),
                    map.len()
                );
                true
            }
            Err(e) => {
                warn!(
                    "[ACH-GSE] Failed to write migrated file {}: {}",
                    path.display(), e
                );
                false
            }
        },
        Err(e) => {
            warn!("[ACH-GSE] Failed to serialize map format: {}", e);
            false
        }
    }
}

// ── DLL search ─────────────────────────────────────────────────────────────

/// The folder name Drop tells Goldberg to use via `local_save_path`.
/// Saves end up at `<dll_dir>/drop-goldberg/<AppID>/`.
pub const DROP_GSE_FOLDER: &str = "drop-goldberg";

/// Fallback directory names checked in AppData for emulators not configured
/// by Drop (or for games launched outside Drop).
const APPDATA_FALLBACK_DIRS: &[&str] = &[
    "drop-goldberg",            // legacy Drop location
    "GSE Saves",                // GBE fork default
    "GSE saves",                // case variant
    "Goldberg SteamEmu Saves",  // original Goldberg default
];

/// The DLL file names Goldberg/SSE replace. We search for these to find where
/// the emulator actually lives (it may be in a subdirectory of the install root).
const STEAM_API_DLLS: &[&str] = &[
    "steam_api64.dll",
    "steam_api.dll",
    "libsteam_api.so",
];

/// Recursively searches `root` for a Steam API DLL and returns the directory
/// containing it, or `None` if not found.
fn find_steam_api_dir(root: &std::path::Path) -> Option<PathBuf> {
    // Check root first (fast path)
    for dll in STEAM_API_DLLS {
        if root.join(dll).exists() {
            return Some(root.to_path_buf());
        }
    }
    // Walk subdirectories (breadth-first-ish, max 5 levels)
    find_steam_api_dir_recursive(root, 0, 5)
}

fn find_steam_api_dir_recursive(
    dir: &std::path::Path,
    depth: u32,
    max_depth: u32,
) -> Option<PathBuf> {
    if depth >= max_depth {
        return None;
    }
    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(e) => e.flatten().collect(),
        Err(_) => return None,
    };

    // Check files first
    for entry in &entries {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_ascii_lowercase();
                for dll in STEAM_API_DLLS {
                    if lower == *dll {
                        return Some(dir.to_path_buf());
                    }
                }
            }
        }
    }

    // Recurse into subdirectories
    for entry in &entries {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_steam_api_dir_recursive(&path, depth + 1, max_depth) {
                return Some(found);
            }
        }
    }
    None
}

// ── Emulator detection ─────────────────────────────────────────────────────

/// The SSE config file name, always next to the Steam API DLL.
const SSE_INI_NAME: &str = "steam_emu.ini";

/// Detects which Steam emulator is installed next to the DLL.
///
/// Priority: `steam_settings/` (Goldberg/GBE) wins over `steam_emu.ini` (SSE).
/// After a GBE upgrade, both will exist — `steam_settings/` takes precedence
/// because the DLL has been swapped to GBE.
fn detect_emulator_type(dll_dir: &std::path::Path) -> SteamEmulator {
    let dll_dir_str = dll_dir.to_string_lossy().to_string();

    // Check for Goldberg/GBE first (steam_settings/ directory next to DLL).
    // This takes priority because after an SSE → GBE upgrade, both
    // steam_emu.ini and steam_settings/ exist, but the DLL is now GBE.
    let steam_settings = dll_dir.join("steam_settings");
    if steam_settings.is_dir() {
        info!("[EMU] Detected Goldberg at {}", steam_settings.display());
        return SteamEmulator::Goldberg { dll_dir: dll_dir_str };
    }

    // Check for SmartSteamEmu (steam_emu.ini next to DLL)
    let sse_ini = dll_dir.join(SSE_INI_NAME);
    if sse_ini.exists() {
        info!(
            "[EMU] Detected SmartSteamEmu at {}",
            sse_ini.display()
        );
        if let Some((app_id, save_path)) = parse_sse_ini(&sse_ini) {
            return SteamEmulator::SmartSteamEmu {
                dll_dir: dll_dir_str,
                save_path,
                app_id,
            };
        }
        warn!("[EMU] Found steam_emu.ini but could not parse it");
    }

    // No config found yet — default to Goldberg (we'll create steam_settings/)
    debug!("[EMU] No emulator config found at {}, defaulting to Goldberg", dll_dir.display());
    SteamEmulator::Goldberg { dll_dir: dll_dir_str }
}

// ── SSE ini parsing ────────────────────────────────────────────────────────

/// Parses `steam_emu.ini` to extract the AppID and save path.
/// Returns `(app_id, save_path)` or `None` if parsing fails.
fn parse_sse_ini(ini_path: &std::path::Path) -> Option<(String, PathBuf)> {
    let content = match std::fs::read_to_string(ini_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("[EMU-SSE] Could not read {}: {}", ini_path.display(), e);
            return None;
        }
    };

    let mut app_id: Option<String> = None;
    let mut save_path: Option<PathBuf> = None;

    // Try to extract save path from header comments first
    // e.g. "### Game data is stored at %SystemDrive%\Users\Public\Documents\Steam\RUNE\1794680"
    for line in content.lines() {
        let trimmed = line.trim().trim_start_matches('#').trim();
        if trimmed.starts_with("Game data is stored at ") {
            let raw_path = trimmed
                .trim_start_matches("Game data is stored at ")
                .trim();
            let expanded = expand_env_vars(raw_path);
            save_path = Some(PathBuf::from(expanded));
            debug!("[EMU-SSE] Save path from header: {}", raw_path);
        }
    }

    // Parse [Settings] section for AppId
    let mut in_settings = false;
    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if trimmed == "[Settings]" {
            in_settings = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_settings = false;
            continue;
        }

        if in_settings {
            if let Some(value) = trimmed.strip_prefix("AppId=") {
                app_id = Some(value.trim().to_string());
                debug!("[EMU-SSE] AppId from ini: {}", value.trim());
            }
        }
    }

    let app_id = app_id?;

    // If no save path from header, construct the default RUNE path
    if save_path.is_none() {
        if let Some(sys_drive) = std::env::var("SystemDrive").ok() {
            let default_path = PathBuf::from(&sys_drive)
                .join("Users")
                .join("Public")
                .join("Documents")
                .join("Steam")
                .join("RUNE")
                .join(&app_id);
            debug!(
                "[EMU-SSE] Using default RUNE save path: {}",
                default_path.display()
            );
            save_path = Some(default_path);
        } else {
            // Non-Windows fallback
            save_path = Some(PathBuf::from(format!(
                "C:\\Users\\Public\\Documents\\Steam\\RUNE\\{}",
                app_id
            )));
        }
    }

    Some((app_id, save_path?))
}

/// Expands `%EnvVar%` patterns in a path string.
fn expand_env_vars(input: &str) -> String {
    let mut result = input.to_string();
    // Find all %VAR% patterns
    while let Some(start) = result.find('%') {
        if let Some(end) = result[start + 1..].find('%') {
            let var_name = &result[start + 1..start + 1 + end];
            let replacement = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                replacement,
                &result[start + 2 + end..]
            );
        } else {
            break;
        }
    }
    result
}

// ── Goldberg achievement reading ───────────────────────────────────────────

/// Returns the path to the Goldberg achievement file for a given AppID.
///
/// Check order:
/// 1. DLL directory (`<dll_dir>/drop-goldberg/<AppID>/achievements.json`)
/// 2. AppData fallback paths for common Goldberg forks / legacy Drop
///
/// `dll_dir` is the directory containing the Steam API DLL.
pub fn gse_save_path(app_id: &str, dll_dir: Option<&str>) -> Option<PathBuf> {
    const TAG: &str = "[ACH-GSE]";

    // 1. Install directory path (highest priority)
    if let Some(dir) = dll_dir {
        let game_path = PathBuf::from(dir)
            .join(DROP_GSE_FOLDER)
            .join(app_id)
            .join("achievements.json");
        if game_path.exists() {
            info!("{TAG} Found GSE file in game dir: {}", game_path.display());
            return Some(game_path);
        }
        info!("{TAG} NOT found in game dir: {}", game_path.display());
    }

    // 2. AppData fallback paths
    if let Some(data_dir) = dirs::data_dir() {
        for dir_name in APPDATA_FALLBACK_DIRS {
            let path = data_dir
                .join(dir_name)
                .join(app_id)
                .join("achievements.json");
            if path.exists() {
                info!(
                    "{TAG} Found FALLBACK GSE file at {} (not in game dir)",
                    path.display()
                );
                return Some(path);
            }
        }
    }

    // Nothing found — return the expected path for logging
    if let Some(dir) = dll_dir {
        let expected = PathBuf::from(dir)
            .join(DROP_GSE_FOLDER)
            .join(app_id)
            .join("achievements.json");
        info!(
            "{TAG} No achievements.json found for AppID {app_id}, expected at: {}",
            expected.display()
        );
        Some(expected)
    } else {
        debug!("{TAG} No achievements.json found for AppID {app_id} (no dll_dir provided)");
        dirs::data_dir().map(|d| d.join(DROP_GSE_FOLDER).join(app_id).join("achievements.json"))
    }
}

/// Max retries when the achievement file is temporarily locked by the game.
const FILE_READ_RETRIES: u32 = 3;
/// Delay between retries (milliseconds).
const FILE_READ_RETRY_DELAY_MS: u64 = 200;

/// Reads all Goldberg achievements from the local GSE save file.
/// Retries on read failure (e.g. file locked by the game process).
fn read_goldberg_unlocks_inner(app_id: &str, dll_dir: Option<&str>) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-GSE]";
    let path = match gse_save_path(app_id, dll_dir) {
        Some(p) => p,
        None => {
            warn!("{TAG} Could not determine save path for AppID {app_id}");
            return Vec::new();
        }
    };

    if !path.exists() {
        info!(
            "{TAG} File does not exist: {} (AppID {app_id})",
            path.display()
        );
        return Vec::new();
    }

    // Retry loop: the game may briefly lock the file while writing
    let mut last_err = String::new();
    for attempt in 0..=FILE_READ_RETRIES {
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                return match parse_gbe_achievements_typed(&contents) {
                    ParseResult::Map(achievements) => {
                        let earned_count = achievements.iter().filter(|a| a.earned).count();
                        info!(
                            "{TAG} Read {} achievements from {} ({} earned, {} locked) [map format]",
                            achievements.len(),
                            path.display(),
                            earned_count,
                            achievements.len() - earned_count
                        );
                        achievements
                    }
                    ParseResult::DefinitionsArray(achievements) => {
                        // The file is in the old server-written definitions array format.
                        // GBE can't write to this format — migrate it to map format so
                        // GBE can properly record achievement unlocks.
                        warn!(
                            "{TAG} File {} is in definitions ARRAY format (not GBE map format) — migrating!",
                            path.display()
                        );
                        migrate_array_to_map_format(&path, &achievements);

                        let earned_count = achievements.iter().filter(|a| a.earned).count();
                        info!(
                            "{TAG} Read {} achievements from {} ({} earned, {} locked) [migrated from array]",
                            achievements.len(),
                            path.display(),
                            earned_count,
                            achievements.len() - earned_count
                        );
                        achievements
                    }
                    ParseResult::Error(e) => {
                        warn!(
                            "{TAG} PARSE FAILED for {}: {} — first 500 chars: {}",
                            path.display(),
                            e,
                            &contents[..contents.len().min(500)]
                        );
                        Vec::new()
                    }
                };
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < FILE_READ_RETRIES {
                    debug!(
                        "{TAG} Read attempt {}/{} failed for {} ({}), retrying...",
                        attempt + 1,
                        FILE_READ_RETRIES,
                        path.display(),
                        e
                    );
                    std::thread::sleep(std::time::Duration::from_millis(
                        FILE_READ_RETRY_DELAY_MS * (attempt as u64 + 1),
                    ));
                }
            }
        }
    }

    warn!("{TAG} READ FAILED after {} retries for {}: {}", FILE_READ_RETRIES, path.display(), last_err);
    Vec::new()
}

// ── SSE achievement reading ────────────────────────────────────────────────

/// Reads achievements from SSE's save directory.
///
/// SSE stores achievements in `achievements.ini` with a numbered INI format:
///
/// ```ini
/// [SteamAchievements]
/// Count=2
/// 0=ACH_NAME_1
/// 0_UnlockTime=1234567890
/// 1=ACH_NAME_2
/// 1_UnlockTime=1234567891
/// ```
///
/// When no achievements are earned: `Count=0`.
fn read_sse_unlocks(save_path: &std::path::Path, app_id: &str) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-SSE]";

    if !save_path.exists() {
        debug!(
            "{TAG} SSE save path does not exist: {} (AppID {app_id})",
            save_path.display()
        );
        return Vec::new();
    }

    // Primary: achievements.ini (the standard SSE format)
    let ini_path = save_path.join("achievements.ini");
    if ini_path.exists() {
        debug!("{TAG} Found achievements.ini at {}", ini_path.display());
        return parse_sse_achievements_ini(&ini_path, app_id);
    }

    // Fallback: achievements.json (some SSE builds use Goldberg-compatible JSON)
    let json_path = save_path.join("achievements.json");
    if json_path.exists() {
        debug!("{TAG} Found achievements.json at SSE path");
        if let Ok(contents) = std::fs::read_to_string(&json_path) {
            if let Ok(achievements) = serde_json::from_str::<Vec<GoldbergAchievement>>(&contents) {
                let earned = achievements.iter().filter(|a| a.earned).count();
                info!(
                    "{TAG} Read {} achievements from SSE JSON ({} earned)",
                    achievements.len(),
                    earned
                );
                return achievements;
            }
        }
    }

    // Log what IS in the save directory to help debug
    debug!(
        "{TAG} No known achievement file found at {}",
        save_path.display()
    );
    if let Ok(entries) = std::fs::read_dir(save_path) {
        let files: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        debug!("{TAG} Files in SSE save dir: {:?}", files);
    }

    Vec::new()
}

/// Parses SSE's `achievements.ini` format.
///
/// Format:
/// ```ini
/// [SteamAchievements]
/// Count=N
/// 0=ACHIEVEMENT_API_NAME
/// 0_UnlockTime=1234567890
/// 1=ACHIEVEMENT_API_NAME
/// 1_UnlockTime=1234567891
/// ```
fn parse_sse_achievements_ini(
    ini_path: &std::path::Path,
    app_id: &str,
) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-SSE]";

    let content = match std::fs::read_to_string(ini_path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "{TAG} Could not read {}: {}",
                ini_path.display(),
                e
            );
            return Vec::new();
        }
    };

    let mut in_section = false;
    let mut count: usize = 0;
    // Collect key=value pairs from the [SteamAchievements] section
    let mut entries: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed == "[SteamAchievements]" {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_section = false;
            continue;
        }

        if !in_section {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == "Count" {
                count = value.parse().unwrap_or(0);
            } else {
                entries.insert(key.to_string(), value.to_string());
            }
        }
    }

    if count == 0 {
        debug!("{TAG} SSE achievements.ini: Count=0 for AppID {app_id}");
        return Vec::new();
    }

    // Build achievement list from numbered entries
    let mut achievements = Vec::with_capacity(count);
    for i in 0..count {
        let idx = i.to_string();
        let name = match entries.get(&idx) {
            Some(n) => n.clone(),
            None => {
                debug!("{TAG} Missing entry for index {i} in achievements.ini");
                continue;
            }
        };

        let unlock_time_key = format!("{i}_UnlockTime");
        let earned_time: u64 = entries
            .get(&unlock_time_key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        achievements.push(GoldbergAchievement {
            name,
            earned: true, // SSE only lists earned achievements
            earned_time,
        });
    }

    info!(
        "{TAG} Parsed {} earned achievements from SSE ini for AppID {app_id}",
        achievements.len()
    );
    achievements
}

// ── Unified public API ─────────────────────────────────────────────────────

/// Reads all achievement unlocks for a game, auto-detecting the emulator type.
///
/// If `emulator_info` is provided, uses it to determine where to look.
/// Otherwise falls back to the legacy Goldberg-only path.
pub fn read_unlocks(
    app_id: &str,
    emulator_info: Option<&EmulatorInfo>,
) -> Vec<GoldbergAchievement> {
    match emulator_info {
        Some(info) => match &info.emulator {
            SteamEmulator::Goldberg { dll_dir } => {
                read_goldberg_unlocks_inner(app_id, Some(dll_dir.as_str()))
            }
            SteamEmulator::SmartSteamEmu { save_path, .. } => {
                read_sse_unlocks(save_path, app_id)
            }
            SteamEmulator::Unknown { dll_dir } => {
                // Try Goldberg as best guess
                read_goldberg_unlocks_inner(app_id, Some(dll_dir.as_str()))
            }
        },
        // Legacy path: no emulator info, try Goldberg with no dll_dir
        None => read_goldberg_unlocks_inner(app_id, None),
    }
}

/// Backward-compatible: reads Goldberg unlocks given an optional dll_dir string.
#[allow(dead_code)]
pub fn read_goldberg_unlocks(app_id: &str, dll_dir: Option<&str>) -> Vec<GoldbergAchievement> {
    read_goldberg_unlocks_inner(app_id, dll_dir)
}

/// Returns only earned achievements (with valid timestamp).
pub fn read_earned(
    app_id: &str,
    emulator_info: Option<&EmulatorInfo>,
) -> Vec<GoldbergAchievement> {
    let all = read_unlocks(app_id, emulator_info);
    let earned: Vec<_> = all
        .into_iter()
        .filter(|a| a.earned)
        .collect();
    info!(
        "[ACH] AppID {app_id}: {} earned achievements",
        earned.len()
    );
    earned
}

/// Backward-compatible: returns earned Goldberg achievements.
#[allow(dead_code)]
pub fn read_goldberg_earned(app_id: &str, dll_dir: Option<&str>) -> Vec<GoldbergAchievement> {
    let all = read_goldberg_unlocks_inner(app_id, dll_dir);
    let earned: Vec<_> = all
        .into_iter()
        .filter(|a| a.earned && a.earned_time > 0)
        .collect();
    debug!(
        "[ACH-GSE] AppID {app_id}: {} earned (with valid timestamp)",
        earned.len()
    );
    earned
}

// ── Pre-launch configuration ───────────────────────────────────────────────

/// Section + key written into Goldberg's `steam_settings/configs.user.ini`.
const INI_SECTION: &str = "[user::saves]";
const INI_KEY: &str = "local_save_path";
/// The relative path (from the game DLL/exe) where Goldberg will save data.
const INI_VALUE: &str = "./drop-goldberg";

/// Detects the Steam emulator type and configures it for Drop.
///
/// For Goldberg: writes `local_save_path=./drop-goldberg` and
/// `account_name=<display_name>` to `steam_settings/configs.user.ini`
/// next to the DLL.
///
/// For SSE: reads the existing config (no modifications needed since SSE
/// manages its own save path).
///
/// Returns `EmulatorInfo` describing what was found, or `None` if no
/// Steam API DLL exists in the install directory.
pub fn configure_saves_for_game(install_dir: &str, display_name: Option<&str>) -> Option<EmulatorInfo> {
    let root = PathBuf::from(install_dir);

    // Find where the Steam API DLL lives
    let dll_dir = match find_steam_api_dir(&root) {
        Some(d) => d,
        None => {
            debug!(
                "No steam_api DLL found in {}, skipping emulator config",
                install_dir
            );
            return None;
        }
    };

    let emulator = detect_emulator_type(&dll_dir);

    match &emulator {
        SteamEmulator::Goldberg { dll_dir: dll_dir_str } => {
            configure_goldberg(&dll_dir, display_name);
            // Migrate any runtime achievement files from definitions array → GBE map format
            migrate_runtime_achievements_if_needed(dll_dir_str);
        }
        SteamEmulator::SmartSteamEmu {
            save_path, app_id, ..
        } => {
            info!(
                "[EMU] SSE game detected (AppID {}), saves at: {}",
                app_id,
                save_path.display()
            );
            // SSE manages its own save path — no config changes needed
        }
        SteamEmulator::Unknown { dll_dir: dll_dir_str } => {
            // Try Goldberg setup as a best guess
            configure_goldberg(&dll_dir, display_name);
            migrate_runtime_achievements_if_needed(dll_dir_str);
        }
    }

    Some(EmulatorInfo { emulator })
}

/// Writes the Goldberg save path and account name config into
/// `steam_settings/configs.user.ini`.
fn configure_goldberg(dll_dir: &std::path::Path, display_name: Option<&str>) {
    let steam_settings = dll_dir.join("steam_settings");
    if !steam_settings.is_dir() {
        if let Err(e) = std::fs::create_dir_all(&steam_settings) {
            warn!(
                "Could not create steam_settings at {}: {}",
                steam_settings.display(),
                e
            );
            return;
        }
        info!("Created {} for Goldberg config", steam_settings.display());
    }

    let ini_path = steam_settings.join("configs.user.ini");
    let desired_save_line = format!("{}={}", INI_KEY, INI_VALUE);

    let existing = std::fs::read_to_string(&ini_path).unwrap_or_default();

    let saves_ok = has_correct_setting(&existing);
    let name_ok = display_name
        .map(|n| has_correct_account_name(&existing, n))
        .unwrap_or(true);

    if saves_ok && name_ok {
        debug!(
            "Goldberg config already up to date for {}",
            dll_dir.display()
        );
        return;
    }

    let mut updated = if saves_ok {
        existing.clone()
    } else {
        update_ini_content(&existing, &desired_save_line)
    };

    if let Some(name) = display_name {
        if !name_ok {
            updated = update_ini_account_name(&updated, name);
        }
    }

    match std::fs::write(&ini_path, &updated) {
        Ok(_) => info!(
            "Configured Goldberg for {} (saves → {}, name → {:?})",
            dll_dir.display(),
            DROP_GSE_FOLDER,
            display_name.unwrap_or("<unchanged>")
        ),
        Err(e) => warn!(
            "Could not write configs.user.ini at {}: {}",
            ini_path.display(),
            e
        ),
    }

}

/// Checks for GBE log/crash files to verify the emulator is actually loading.
/// GBE writes logs to `<save_path>/crash_reports/` and sometimes next to the DLL.
/// Call this after the game has been running briefly to diagnose emulator issues.
pub fn check_gbe_activity(dll_dir: &str) {
    let root = PathBuf::from(dll_dir);

    // Check for GBE log files in common locations
    let log_locations = [
        root.join(DROP_GSE_FOLDER).join("crash_reports"),
        root.join(DROP_GSE_FOLDER).join("logs"),
        root.join("crash_reports"),
        root.join("logs"),
    ];

    let mut found_any = false;
    for log_dir in &log_locations {
        if log_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(log_dir) {
                let files: Vec<_> = entries
                    .flatten()
                    .filter(|e| e.path().is_file())
                    .collect();
                if !files.is_empty() {
                    info!(
                        "[GBE-DIAG] Found {} log/crash files in {}",
                        files.len(),
                        log_dir.display()
                    );
                    // Show the most recent file name
                    if let Some(latest) = files.last() {
                        info!(
                            "[GBE-DIAG] Latest file: {}",
                            latest.path().display()
                        );
                    }
                    found_any = true;
                }
            }
        }
    }

    // Check for GBE-specific files that indicate it loaded
    let gbe_markers = [
        root.join(DROP_GSE_FOLDER).join("user_steam_id.txt"),
        root.join(DROP_GSE_FOLDER).join("account_name.txt"),
    ];

    for marker in &gbe_markers {
        if marker.exists() {
            info!("[GBE-DIAG] Found GBE marker file: {}", marker.display());
            found_any = true;
        }
    }

    // Check if any drop-goldberg/<AppID> directories have GBE-created files
    let save_root = root.join(DROP_GSE_FOLDER);
    if save_root.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&save_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(files) = std::fs::read_dir(&path) {
                        let file_names: Vec<String> = files
                            .flatten()
                            .filter_map(|e| {
                                e.file_name().to_str().map(|s| s.to_string())
                            })
                            .collect();
                        // Files like playtime.txt, remote/, stats/ are created by GBE at runtime
                        let gbe_runtime_files: Vec<&String> = file_names.iter()
                            .filter(|f| *f != "achievements.json") // we create this ourselves
                            .collect();
                        if !gbe_runtime_files.is_empty() {
                            found_any = true;
                        }
                        info!(
                            "[GBE-DIAG] Save dir {}: files = {:?}",
                            path.display(),
                            file_names
                        );
                    }
                }
            }
        }
    }

    if found_any {
        info!("[GBE-DIAG] GBE appears active in {}", dll_dir);
    } else {
        warn!(
            "[GBE-DIAG] No GBE activity detected in {} — \
             the emulator may not be loading. Check that the steam_api DLL \
             is actually a Goldberg/GBE build (not the original Steam DLL).",
            dll_dir
        );
    }
}

/// Scans `<dll_dir>/drop-goldberg/*/achievements.json` and migrates any files
/// that are in the old definitions-array format to GBE's map format.
/// This runs at game launch time (before the emulator starts) so GBE finds
/// the correct format on disk.
fn migrate_runtime_achievements_if_needed(dll_dir: &str) {
    let save_root = PathBuf::from(dll_dir).join(DROP_GSE_FOLDER);
    if !save_root.is_dir() {
        return;
    }

    let entries = match std::fs::read_dir(&save_root) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let ach_path = entry.path().join("achievements.json");
        if !ach_path.exists() {
            continue;
        }

        if let Ok(contents) = std::fs::read_to_string(&ach_path) {
            match parse_gbe_achievements_typed(&contents) {
                ParseResult::DefinitionsArray(achievements) => {
                    info!(
                        "[ACH-GSE] Pre-launch migration needed for {}",
                        ach_path.display()
                    );
                    migrate_array_to_map_format(&ach_path, &achievements);
                }
                ParseResult::Map(_) => {
                    debug!(
                        "[ACH-GSE] {} already in map format, no migration needed",
                        ach_path.display()
                    );
                }
                ParseResult::Error(e) => {
                    warn!(
                        "[ACH-GSE] Could not parse {} for migration check: {}",
                        ach_path.display(), e
                    );
                }
            }
        }
    }
}

/// Checks whether the existing INI content already has the correct setting.
fn has_correct_setting(content: &str) -> bool {
    let target = format!("{}={}", INI_KEY, INI_VALUE);
    let target_spaced = format!("{} = {}", INI_KEY, INI_VALUE);

    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(INI_SECTION) {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_section = false;
            continue;
        }
        if in_section {
            let normalized = trimmed.replace(' ', "");
            if normalized == target || trimmed == target_spaced {
                return true;
            }
        }
    }
    false
}

/// GBE section and key for the player's display name.
const INI_GENERAL_SECTION: &str = "[user::general]";
const INI_ACCOUNT_NAME_KEY: &str = "account_name";

/// Checks whether the INI already has the correct account_name.
fn has_correct_account_name(content: &str, expected_name: &str) -> bool {
    let target = format!("{}={}", INI_ACCOUNT_NAME_KEY, expected_name);

    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(INI_GENERAL_SECTION) {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_section = false;
            continue;
        }
        if in_section {
            let normalized = trimmed.replace(' ', "");
            if normalized == target {
                return true;
            }
        }
    }
    false
}

/// Updates or appends the `[user::general]` section with `account_name`.
fn update_ini_account_name(existing: &str, name: &str) -> String {
    let desired_line = format!("{}={}", INI_ACCOUNT_NAME_KEY, name);
    let mut result = String::with_capacity(existing.len() + 80);
    let mut found_section = false;
    let mut replaced_key = false;
    let mut in_section = false;

    for line in existing.lines() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case(INI_GENERAL_SECTION) {
            found_section = true;
            in_section = true;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if trimmed.starts_with('[') {
            if in_section && !replaced_key {
                result.push_str(&desired_line);
                result.push('\n');
                replaced_key = true;
            }
            in_section = false;
        }

        if in_section {
            let key = trimmed.split(['=', ' ']).next().unwrap_or("");
            if key == INI_ACCOUNT_NAME_KEY {
                result.push_str(&desired_line);
                result.push('\n');
                replaced_key = true;
                continue;
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    // If we were still in the section at EOF and didn't replace
    if in_section && !replaced_key {
        result.push_str(&desired_line);
        result.push('\n');
    }

    // If the section didn't exist at all, append it
    if !found_section {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(INI_GENERAL_SECTION);
        result.push('\n');
        result.push_str(&desired_line);
        result.push('\n');
    }

    result
}

/// The old key we used to write. If found inside `[user::saves]`, it should
/// be removed so Goldberg doesn't use the AppData path.
const LEGACY_INI_KEY: &str = "saves_folder_name";

/// Updates (or appends) the `[user::saves]` section with our setting,
/// preserving all other content. Also strips the legacy `saves_folder_name`.
fn update_ini_content(existing: &str, desired_line: &str) -> String {
    if existing.is_empty() {
        return format!("{}\n{}\n", INI_SECTION, desired_line);
    }

    let mut result = String::with_capacity(existing.len() + 80);
    let mut found_section = false;
    let mut replaced_key = false;
    let mut in_section = false;

    for line in existing.lines() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case(INI_SECTION) {
            found_section = true;
            in_section = true;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if trimmed.starts_with('[') {
            if in_section && !replaced_key {
                result.push_str(desired_line);
                result.push('\n');
                replaced_key = true;
            }
            in_section = false;
        }

        if in_section {
            let key = trimmed.split(['=', ' ']).next().unwrap_or("");

            if key == LEGACY_INI_KEY {
                continue;
            }

            if key == INI_KEY {
                result.push_str(desired_line);
                result.push('\n');
                replaced_key = true;
                continue;
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    if in_section && !replaced_key {
        result.push_str(desired_line);
        result.push('\n');
    }

    if !found_section {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(INI_SECTION);
        result.push('\n');
        result.push_str(desired_line);
        result.push('\n');
    }

    result
}


