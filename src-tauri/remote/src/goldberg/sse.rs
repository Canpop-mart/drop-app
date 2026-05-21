//! SmartSteamEmu (SSE / RUNE) config parsing and achievement reading.
//!
//! SSE is configured by a `steam_emu.ini` next to the Steam API DLL and
//! stores game data (saves + achievements) at a fixed path, typically
//! `%SystemDrive%\Users\Public\Documents\Steam\RUNE\<AppID>`. Achievements
//! live in `achievements.ini` with a numbered-key format; some builds use a
//! Goldberg-compatible `achievements.json` instead.

use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::achievements::GoldbergAchievement;

/// Parses `steam_emu.ini` to extract `(app_id, save_path)`, or `None` on
/// failure.
///
/// The save path is taken from the header comment
/// (`### Game data is stored at ...`) when present, otherwise the default
/// RUNE path is constructed from the AppID.
pub fn parse_sse_ini(ini_path: &Path) -> Option<(String, PathBuf)> {
    let content = match std::fs::read_to_string(ini_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("[EMU-SSE] Could not read {}: {e}", ini_path.display());
            return None;
        }
    };

    let mut app_id: Option<String> = None;
    let mut save_path: Option<PathBuf> = None;

    // Save path from the header comment.
    for line in content.lines() {
        let trimmed = line.trim().trim_start_matches('#').trim();
        if let Some(raw_path) = trimmed.strip_prefix("Game data is stored at ") {
            let raw_path = raw_path.trim();
            save_path = Some(PathBuf::from(expand_env_vars(raw_path)));
            debug!("[EMU-SSE] Save path from header: {raw_path}");
        }
    }

    // AppId from the [Settings] section.
    let mut in_settings = false;
    for line in content.lines() {
        let trimmed = line.trim();
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

    // Construct the default RUNE path when the header gave none.
    if save_path.is_none() {
        save_path = Some(default_rune_path(&app_id));
    }

    Some((app_id, save_path?))
}

/// Builds SSE's default RUNE save path for an AppID.
fn default_rune_path(app_id: &str) -> PathBuf {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let path = PathBuf::from(drive)
        .join("Users")
        .join("Public")
        .join("Documents")
        .join("Steam")
        .join("RUNE")
        .join(app_id);
    debug!("[EMU-SSE] Using default RUNE save path: {}", path.display());
    path
}

/// Expands `%EnvVar%` patterns in a path string. Unknown variables expand to
/// the empty string; an unterminated `%` is left as-is.
fn expand_env_vars(input: &str) -> String {
    let mut result = input.to_string();
    while let Some(start) = result.find('%') {
        if let Some(end) = result[start + 1..].find('%') {
            let var_name = &result[start + 1..start + 1 + end];
            let replacement = std::env::var(var_name).unwrap_or_default();
            result = format!("{}{replacement}{}", &result[..start], &result[start + 2 + end..]);
        } else {
            break;
        }
    }
    result
}

/// Reads achievements from SSE's save directory for `app_id`.
///
/// Prefers `achievements.ini` (the standard SSE format), falling back to a
/// Goldberg-compatible `achievements.json` for builds that use it.
pub fn read_sse_unlocks(save_path: &Path, app_id: &str) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-SSE]";

    if !save_path.exists() {
        debug!("{TAG} SSE save path does not exist: {} (AppID {app_id})", save_path.display());
        return Vec::new();
    }

    let ini_path = save_path.join("achievements.ini");
    if ini_path.exists() {
        debug!("{TAG} Found achievements.ini at {}", ini_path.display());
        return parse_sse_achievements_ini(&ini_path, app_id);
    }

    let json_path = save_path.join("achievements.json");
    if json_path.exists() {
        debug!("{TAG} Found achievements.json at SSE path");
        if let Ok(contents) = std::fs::read_to_string(&json_path) {
            if let Ok(achievements) = serde_json::from_str::<Vec<GoldbergAchievement>>(&contents) {
                let earned = achievements.iter().filter(|a| a.earned).count();
                info!("{TAG} Read {} achievements from SSE JSON ({earned} earned)", achievements.len());
                return achievements;
            }
        }
    }

    debug!("{TAG} No known achievement file found at {}", save_path.display());
    if let Ok(entries) = std::fs::read_dir(save_path) {
        let files: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        debug!("{TAG} Files in SSE save dir: {files:?}");
    }
    Vec::new()
}

/// Parses SSE's numbered `achievements.ini` format:
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
/// SSE only ever lists *earned* achievements.
fn parse_sse_achievements_ini(ini_path: &Path, app_id: &str) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-SSE]";

    let content = match std::fs::read_to_string(ini_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("{TAG} Could not read {}: {e}", ini_path.display());
            return Vec::new();
        }
    };

    let mut in_section = false;
    let mut count: usize = 0;
    let mut entries: HashMap<String, String> = HashMap::new();

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
            let (key, value) = (key.trim(), value.trim());
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

    let mut achievements = Vec::with_capacity(count);
    for i in 0..count {
        let name = match entries.get(&i.to_string()) {
            Some(n) => n.clone(),
            None => {
                debug!("{TAG} Missing entry for index {i} in achievements.ini");
                continue;
            }
        };
        let earned_time: u64 = entries
            .get(&format!("{i}_UnlockTime"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        achievements.push(GoldbergAchievement { name, earned: true, earned_time });
    }

    info!("{TAG} Parsed {} earned achievements from SSE ini for AppID {app_id}", achievements.len());
    achievements
}
