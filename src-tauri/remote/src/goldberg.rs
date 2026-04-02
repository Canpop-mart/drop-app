//! Goldberg Steam Emulator — local achievement file reader.
//!
//! DRM-free games using the Goldberg emulator store achievement unlock state at:
//!   `%APPDATA%/GSE saves/<AppID>/achievements.json`   (Windows)
//!   `~/.local/share/GSE saves/<AppID>/achievements.json` (Linux)
//!
//! Each entry in the JSON array has at minimum:
//!   - `name`        — Steam-style API name (e.g. "ACH_WIN_ONE_GAME")
//!   - `earned`      — boolean, true once the player unlocks it
//!   - `earned_time` — unix timestamp of when it was earned

use log::{debug, warn};
use serde::Deserialize;
use std::path::PathBuf;

/// A single achievement entry from the GSE saves JSON file.
#[derive(Deserialize, Debug, Clone)]
pub struct GoldbergAchievement {
    pub name: String,
    #[serde(default)]
    pub earned: bool,
    #[serde(default)]
    pub earned_time: u64,
}

/// Returns the path to the GSE saves directory for a given AppID.
/// Returns `None` if the platform-appropriate data directory cannot be resolved.
pub fn gse_save_path(app_id: &str) -> Option<PathBuf> {
    // On Windows: %APPDATA%/GSE saves/<AppID>/achievements.json
    // On Linux:   ~/.local/share/GSE saves/<AppID>/achievements.json
    // On macOS:   ~/Library/Application Support/GSE saves/<AppID>/achievements.json
    let data_dir = if cfg!(target_os = "windows") {
        // %APPDATA% (roaming)
        dirs::data_dir()
    } else {
        // ~/.local/share on Linux, ~/Library/Application Support on macOS
        dirs::data_dir()
    };

    data_dir.map(|d| d.join("GSE saves").join(app_id).join("achievements.json"))
}

/// Reads all Goldberg achievements from the local GSE save file for the given AppID.
/// Returns an empty vec if the file doesn't exist or can't be parsed.
pub fn read_goldberg_unlocks(app_id: &str) -> Vec<GoldbergAchievement> {
    let path = match gse_save_path(app_id) {
        Some(p) => p,
        None => {
            debug!("Could not determine GSE saves path for AppID {}", app_id);
            return Vec::new();
        }
    };

    if !path.exists() {
        debug!("No GSE save file at {}", path.display());
        return Vec::new();
    }

    match std::fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<Vec<GoldbergAchievement>>(&contents) {
            Ok(achievements) => {
                debug!(
                    "Read {} Goldberg achievements from {}",
                    achievements.len(),
                    path.display()
                );
                achievements
            }
            Err(e) => {
                warn!(
                    "Failed to parse Goldberg achievements at {}: {}",
                    path.display(),
                    e
                );
                Vec::new()
            }
        },
        Err(e) => {
            warn!(
                "Failed to read Goldberg save file at {}: {}",
                path.display(),
                e
            );
            Vec::new()
        }
    }
}

/// Returns only the unlocked achievements from the local GSE save file.
pub fn read_goldberg_earned(app_id: &str) -> Vec<GoldbergAchievement> {
    read_goldberg_unlocks(app_id)
        .into_iter()
        .filter(|a| a.earned && a.earned_time > 0)
        .collect()
}
