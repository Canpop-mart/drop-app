//! Steam-emulator detection and **local achievement-file reading**.
//!
//! Drop supports the two Steam emulators a downloaded game may ship with:
//!
//! ## Goldberg / GBE
//! Configured via `steam_settings/configs.user.ini`. Drop writes
//! `local_save_path=./drop-goldberg`, so unlocks land at
//! `<dll_dir>/drop-goldberg/<AppID>/achievements.json` as JSON.
//!
//! ## SmartSteamEmu (SSE / RUNE)
//! Configured via `steam_emu.ini` next to the DLL. Saves go to a fixed path
//! (typically `…\Steam\RUNE\<AppID>`); achievements live in `achievements.ini`.
//!
//! The emulator type is decided by which config sits next to the Steam API DLL
//! — see [`discovery::detect_emulator_type`].
//!
//! # What this module does today vs. the achievement-detection gap
//!
//! **Implemented and live:**
//!
//! * Steam API DLL discovery + emulator-type detection ([`discovery`]).
//! * Goldberg pre-launch config — `local_save_path` + `account_name` written
//!   to `configs.user.ini` ([`config::configure_goldberg`]).
//! * **Reading** achievement *files that already exist on disk* — the GBE
//!   map / definitions-array parser + array→map migration ([`achievements`]),
//!   and the SSE `achievements.ini`/`.json` parser ([`sse`]).
//! * GBE runtime diagnostics ([`config::check_gbe_activity`]).
//!
//! These readers ([`read_unlocks`] / [`read_earned`]) are wired into the
//! achievement poll loop (`remote/src/achievements.rs`): in Goldberg mode it
//! calls [`read_earned`] every 15 s and reports newly-earned achievements to
//! the server.
//!
//! **The gap (deliberately *not* implemented here):** there is no
//! *file-system watcher* / push-based detection. Drop never tells the emulator
//! to emit unlock events, and nothing watches the `achievements.json` for
//! changes — detection is purely the 15 s *poll* re-reading the file. In
//! practice the server-side sync (`session-end` → server re-reads Steam /
//! RetroAchievements) is the authoritative achievement source; this module's
//! file reading is a best-effort *supplement*, not a complete client-side
//! achievement engine. `CLAUDE.md` records this as: *"Goldberg file-based
//! achievement detection is NOT yet implemented — currently relies on
//! server-side sync only."* Closing that gap (a real watcher / unlock hook)
//! is a feature, out of scope for this audit. See `docs/audit/emulation-2026.md`.
//!
//! # Module layout
//!
//! Split by concern from a single 1266-line file; every public item is
//! re-exported here so `remote::goldberg::Foo` paths keep working unchanged.
//!
//! * [`discovery`]    — Steam API DLL search + emulator-type detection.
//! * [`achievements`] — the GBE `achievements.json` parser, array→map
//!   migration, and the retrying file reader.
//! * [`sse`]          — SmartSteamEmu `steam_emu.ini` + achievement parsing.
//! * [`config`]       — Goldberg `configs.user.ini` writing + GBE diagnostics.

pub mod achievements;
pub mod config;
pub mod discovery;
pub mod sse;

use log::{debug, info};
use std::path::PathBuf;

// Re-export the public surface so existing `remote::goldberg::*` call sites
// in the `process`, `games` and achievements code keep compiling unchanged.
pub use achievements::GoldbergAchievement;

/// The folder name Drop tells Goldberg to use via `local_save_path`.
/// Saves end up at `<dll_dir>/drop-goldberg/<AppID>/`.
pub const DROP_GSE_FOLDER: &str = "drop-goldberg";

/// Fallback directory names checked in AppData for emulators not configured by
/// Drop (or games launched outside Drop).
const APPDATA_FALLBACK_DIRS: &[&str] = &[
    "drop-goldberg",           // legacy Drop location
    "GSE Saves",               // GBE fork default
    "GSE saves",               // case variant
    "Goldberg SteamEmu Saves", // original Goldberg default
];

// ── Emulator types ───────────────────────────────────────────────────────

/// Which Steam emulator a game uses.
#[derive(Debug, Clone)]
pub enum SteamEmulator {
    /// Goldberg / GBE fork — `steam_settings/` + `achievements.json`.
    Goldberg {
        /// Directory containing the Steam API DLL (where `drop-goldberg/` lives).
        dll_dir: String,
    },
    /// SmartSteamEmu (SSE/RUNE) — `steam_emu.ini`.
    SmartSteamEmu {
        /// Directory containing the Steam API DLL + `steam_emu.ini`.
        dll_dir: String,
        /// Where SSE stores game data (parsed from the ini).
        save_path: PathBuf,
        /// Steam AppID parsed from the ini.
        app_id: String,
    },
    /// Steam API DLL found but the emulator type couldn't be determined.
    Unknown {
        dll_dir: String,
    },
}

/// Result of detecting / configuring the emulator for a game.
#[derive(Debug, Clone)]
pub struct EmulatorInfo {
    pub emulator: SteamEmulator,
}

impl EmulatorInfo {
    /// The DLL directory, regardless of emulator type.
    pub fn dll_dir(&self) -> &str {
        match &self.emulator {
            SteamEmulator::Goldberg { dll_dir }
            | SteamEmulator::SmartSteamEmu { dll_dir, .. }
            | SteamEmulator::Unknown { dll_dir } => dll_dir,
        }
    }

    /// The directory to search for achievement save files, by emulator type.
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

// ── Save-path resolution ─────────────────────────────────────────────────

/// Returns the path to the Goldberg `achievements.json` for `app_id`.
///
/// Check order: the DLL directory (`<dll_dir>/drop-goldberg/<AppID>/`) first,
/// then the AppData fallback paths for common Goldberg forks / legacy Drop. If
/// nothing exists, returns the *expected* DLL-dir path (useful for logging).
pub fn gse_save_path(app_id: &str, dll_dir: Option<&str>) -> Option<PathBuf> {
    const TAG: &str = "[ACH-GSE]";

    // 1. Install directory — highest priority.
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

    // 2. AppData fallbacks.
    if let Some(data_dir) = dirs::data_dir() {
        for dir_name in APPDATA_FALLBACK_DIRS {
            let path = data_dir.join(dir_name).join(app_id).join("achievements.json");
            if path.exists() {
                info!("{TAG} Found FALLBACK GSE file at {} (not in game dir)", path.display());
                return Some(path);
            }
        }
    }

    // Nothing found — return the expected path for logging.
    if let Some(dir) = dll_dir {
        let expected = PathBuf::from(dir)
            .join(DROP_GSE_FOLDER)
            .join(app_id)
            .join("achievements.json");
        info!("{TAG} No achievements.json for AppID {app_id}, expected at: {}", expected.display());
        Some(expected)
    } else {
        debug!("{TAG} No achievements.json for AppID {app_id} (no dll_dir provided)");
        dirs::data_dir().map(|d| d.join(DROP_GSE_FOLDER).join(app_id).join("achievements.json"))
    }
}

/// Every `achievements.json` that exists for `app_id`, across the DLL-dir
/// `drop-goldberg` folder AND the GBE-fork default save locations (next to the
/// DLL and under %APPDATA%). The reader scans all of these and keeps whichever
/// actually contains unlocks, so Drop's own all-`false` `drop-goldberg` file
/// can't mask real unlocks GBE wrote to its default path.
pub fn gse_candidate_paths(app_id: &str, dll_dir: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // Next to the DLL (where `local_save_path=./drop-goldberg` points, plus the
    // GBE-fork defaults if a fork ignored that redirect).
    if let Some(dir) = dll_dir {
        let base = PathBuf::from(dir);
        for name in APPDATA_FALLBACK_DIRS {
            let p = base.join(name).join(app_id).join("achievements.json");
            if p.exists() {
                out.push(p);
            }
        }
    }
    // The same fork defaults under %APPDATA% (a fork not configured by Drop).
    if let Some(data_dir) = dirs::data_dir() {
        for name in APPDATA_FALLBACK_DIRS {
            let p = data_dir.join(name).join(app_id).join("achievements.json");
            if p.exists() {
                out.push(p);
            }
        }
    }
    out
}

/// Read the game's own Steam AppID from `steam_appid.txt` next to the emulator
/// DLL (or in `steam_settings/`). Used to track Goldberg achievements when the
/// server provided no Goldberg AppID link, and to catch a server AppID that
/// differs from the one the game actually uses on disk.
pub fn read_local_steam_appid(dll_dir: &str) -> Option<String> {
    let base = std::path::Path::new(dll_dir);
    for candidate in [
        base.join("steam_appid.txt"),
        base.join("steam_settings").join("steam_appid.txt"),
    ] {
        if let Ok(s) = std::fs::read_to_string(&candidate) {
            let id = s.trim();
            if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.to_string());
            }
        }
    }
    None
}

// ── Unified achievement-reading API ──────────────────────────────────────

/// Reads all achievement unlocks for a game, auto-selecting the reader by
/// emulator type. With no `emulator_info`, falls back to the Goldberg path.
pub fn read_unlocks(app_id: &str, emulator_info: Option<&EmulatorInfo>) -> Vec<GoldbergAchievement> {
    match emulator_info {
        Some(info) => match &info.emulator {
            SteamEmulator::Goldberg { dll_dir } | SteamEmulator::Unknown { dll_dir } => {
                achievements::read_goldberg_unlocks(app_id, Some(dll_dir.as_str()))
            }
            SteamEmulator::SmartSteamEmu { save_path, .. } => {
                sse::read_sse_unlocks(save_path, app_id)
            }
        },
        None => achievements::read_goldberg_unlocks(app_id, None),
    }
}

/// Returns only the *earned* achievements for a game.
pub fn read_earned(app_id: &str, emulator_info: Option<&EmulatorInfo>) -> Vec<GoldbergAchievement> {
    let earned: Vec<_> = read_unlocks(app_id, emulator_info)
        .into_iter()
        .filter(|a| a.earned)
        .collect();
    info!("[ACH] AppID {app_id}: {} earned achievements", earned.len());
    earned
}

// ── Pre-launch configuration ─────────────────────────────────────────────

/// Detects the Steam emulator for a game install and configures it for Drop.
///
/// * **Goldberg** — writes `local_save_path` + `account_name` and migrates any
///   stale array-format `achievements.json` files to GBE map format.
/// * **SSE** — no changes needed; SSE manages its own save path.
///
/// Returns `EmulatorInfo`, or `None` if no Steam API DLL exists under the
/// install directory.
pub fn configure_saves_for_game(install_dir: &str, display_name: Option<&str>) -> Option<EmulatorInfo> {
    let root = PathBuf::from(install_dir);

    let dll_dir = match discovery::find_steam_api_dir(&root) {
        Some(d) => d,
        None => {
            debug!("[EMU] No steam_api DLL found in {install_dir}, skipping emulator config");
            return None;
        }
    };

    let emulator = discovery::detect_emulator_type(&dll_dir);

    match &emulator {
        // Goldberg and Unknown both get the Goldberg setup (best guess).
        SteamEmulator::Goldberg { dll_dir: dll_dir_str }
        | SteamEmulator::Unknown { dll_dir: dll_dir_str } => {
            config::configure_goldberg(&dll_dir, display_name);
            achievements::migrate_runtime_achievements_if_needed(dll_dir_str);
        }
        SteamEmulator::SmartSteamEmu { save_path, app_id, .. } => {
            info!("[EMU] SSE game detected (AppID {app_id}), saves at: {}", save_path.display());
            // SSE manages its own save path — no config changes needed.
        }
    }

    Some(EmulatorInfo { emulator })
}

/// Checks for GBE log/crash/marker files to verify the emulator loaded.
/// Thin re-export of [`config::check_gbe_activity`] for call-site stability.
/// Returns `true` if GBE looks active (writing logs/markers/runtime files).
pub fn check_gbe_activity(dll_dir: &str) -> bool {
    config::check_gbe_activity(dll_dir)
}
