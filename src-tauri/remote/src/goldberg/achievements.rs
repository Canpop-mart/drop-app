//! Goldberg / GBE achievement-file parsing, migration and reading.
//!
//! Goldberg-family Steam emulators record achievement unlocks as JSON at
//! `<dll_dir>/drop-goldberg/<AppID>/achievements.json`. Two on-disk shapes
//! exist:
//!
//! * **Map** — `{"ACH_NAME": {"earned": true, "earned_time": 123}}`. This is
//!   what the GBE *fork* writes at runtime and the only format GBE can record
//!   into.
//! * **Definitions array** — `[{"name": "ACH_NAME", ...}]`. This is *not* a
//!   GBE runtime format; it means an older Drop server pre-populated the file
//!   with achievement *definitions*. GBE cannot write unlocks into it, so Drop
//!   [`migrate`](migrate_array_to_map_format)s it to map format before launch.

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Max retries when the achievement file is briefly locked by the game.
const FILE_READ_RETRIES: u32 = 3;
/// Base delay between retries (milliseconds); scaled by attempt number.
const FILE_READ_RETRY_DELAY_MS: u64 = 200;

/// A single achievement entry read from any emulator's save files.
/// Field names match the Goldberg JSON format for backward compatibility.
#[derive(Deserialize, Debug, Clone)]
pub struct GoldbergAchievement {
    pub name: String,
    #[serde(default)]
    pub earned: bool,
    #[serde(default)]
    pub earned_time: u64,
}

/// GBE's runtime map-entry value: `{"earned": false, "earned_time": 0}`.
#[derive(Deserialize, Serialize, Debug)]
struct GbeMapEntry {
    #[serde(default)]
    earned: bool,
    #[serde(default)]
    earned_time: u64,
}

/// Outcome of parsing a GBE achievements file.
enum ParseResult {
    /// GBE map format — the correct runtime format.
    Map(Vec<GoldbergAchievement>),
    /// Server-written definitions array — needs migration to map format.
    DefinitionsArray(Vec<GoldbergAchievement>),
    /// Could not parse as either.
    Error(String),
}

/// Parses an `achievements.json`, distinguishing GBE map format from a
/// server-written definitions array.
fn parse_gbe_achievements_typed(contents: &str) -> ParseResult {
    // Map format first — GBE fork runtime saves (the correct format).
    if let Ok(map) = serde_json::from_str::<HashMap<String, GbeMapEntry>>(contents) {
        // JSON objects parse as maps; arrays don't. The `{`-check rejects an
        // empty array that happened to deserialize as an empty map.
        if !map.is_empty() || contents.trim_start().starts_with('{') {
            let achievements = map
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

    // Array format — Goldberg classic / server definitions.
    if let Ok(achievements) = serde_json::from_str::<Vec<GoldbergAchievement>>(contents) {
        return ParseResult::DefinitionsArray(achievements);
    }

    ParseResult::Error("Could not parse as map or array format".to_string())
}

/// Converts a definitions-array `achievements.json` to GBE map format in
/// place. A migration for files written by older server versions. Returns
/// `true` if the file was rewritten.
fn migrate_array_to_map_format(path: &Path, achievements: &[GoldbergAchievement]) -> bool {
    let map: HashMap<String, GbeMapEntry> = achievements
        .iter()
        .map(|ach| {
            (
                ach.name.clone(),
                GbeMapEntry { earned: ach.earned, earned_time: ach.earned_time },
            )
        })
        .collect();

    match serde_json::to_string_pretty(&map) {
        Ok(json) => match std::fs::write(path, &json) {
            Ok(_) => {
                info!(
                    "[ACH-GSE] MIGRATED {} from definitions array -> GBE map format ({} achievements)",
                    path.display(),
                    map.len()
                );
                true
            }
            Err(e) => {
                warn!("[ACH-GSE] Failed to write migrated file {}: {e}", path.display());
                false
            }
        },
        Err(e) => {
            warn!("[ACH-GSE] Failed to serialize map format: {e}");
            false
        }
    }
}

/// Reads all Goldberg achievements from the local GSE save file for `app_id`.
///
/// Retries on read failure (the game may briefly lock the file while writing).
/// If the file is in the legacy definitions-array format it is migrated to GBE
/// map format on the spot so GBE can record future unlocks.
pub fn read_goldberg_unlocks(app_id: &str, dll_dir: Option<&str>) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-GSE]";
    let path = match super::gse_save_path(app_id, dll_dir) {
        Some(p) => p,
        None => {
            warn!("{TAG} Could not determine save path for AppID {app_id}");
            return Vec::new();
        }
    };

    if !path.exists() {
        info!("{TAG} File does not exist: {} (AppID {app_id})", path.display());
        return Vec::new();
    }

    let mut last_err = String::new();
    for attempt in 0..=FILE_READ_RETRIES {
        match std::fs::read_to_string(&path) {
            Ok(contents) => return finish_parse(&path, &contents),
            Err(e) => {
                last_err = e.to_string();
                if attempt < FILE_READ_RETRIES {
                    debug!(
                        "{TAG} Read attempt {}/{FILE_READ_RETRIES} failed for {} ({e}), retrying...",
                        attempt + 1,
                        path.display()
                    );
                    std::thread::sleep(std::time::Duration::from_millis(
                        FILE_READ_RETRY_DELAY_MS * (attempt as u64 + 1),
                    ));
                }
            }
        }
    }

    warn!("{TAG} READ FAILED after {FILE_READ_RETRIES} retries for {}: {last_err}", path.display());
    Vec::new()
}

/// Parses the file contents, migrating array-format files and logging counts.
fn finish_parse(path: &Path, contents: &str) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-GSE]";
    match parse_gbe_achievements_typed(contents) {
        ParseResult::Map(achievements) => {
            log_counts(TAG, path, &achievements, "map format");
            achievements
        }
        ParseResult::DefinitionsArray(achievements) => {
            // GBE can't record into the array format — migrate it now.
            warn!(
                "{TAG} File {} is in definitions ARRAY format (not GBE map format) — migrating!",
                path.display()
            );
            migrate_array_to_map_format(path, &achievements);
            log_counts(TAG, path, &achievements, "migrated from array");
            achievements
        }
        ParseResult::Error(e) => {
            warn!(
                "{TAG} PARSE FAILED for {}: {e} — first 500 chars: {}",
                path.display(),
                &contents[..contents.len().min(500)]
            );
            Vec::new()
        }
    }
}

fn log_counts(tag: &str, path: &Path, achievements: &[GoldbergAchievement], note: &str) {
    let earned = achievements.iter().filter(|a| a.earned).count();
    info!(
        "{tag} Read {} achievements from {} ({earned} earned, {} locked) [{note}]",
        achievements.len(),
        path.display(),
        achievements.len() - earned
    );
}

/// Scans `<dll_dir>/drop-goldberg/*/achievements.json` and migrates any file
/// in the old definitions-array format to GBE map format.
///
/// Runs at game-launch time (before the emulator starts) so GBE finds the
/// correct on-disk format.
pub fn migrate_runtime_achievements_if_needed(dll_dir: &str) {
    let save_root = std::path::PathBuf::from(dll_dir).join(super::DROP_GSE_FOLDER);
    if !save_root.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&save_root) else { return };

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
                    info!("[ACH-GSE] Pre-launch migration needed for {}", ach_path.display());
                    migrate_array_to_map_format(&ach_path, &achievements);
                }
                ParseResult::Map(_) => {
                    debug!("[ACH-GSE] {} already in map format", ach_path.display());
                }
                ParseResult::Error(e) => {
                    warn!("[ACH-GSE] Could not parse {} for migration check: {e}", ach_path.display());
                }
            }
        }
    }
}
