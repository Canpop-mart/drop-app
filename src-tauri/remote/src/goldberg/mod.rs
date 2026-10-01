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
//! calls [`read_earned`] every 15 s, plus once more when the game exits, and
//! reports newly-earned achievements to the server.
//!
//! **This file reading is the ONLY source of Goldberg/Steam-emulator unlocks.**
//! The server never reads Steam for a player: `session-end` only syncs
//! RetroAchievements. An unlock that never lands in a file this module reads
//! is never recorded.
//!
//! There is no file-system watcher / push-based detection: Drop never tells
//! the emulator to emit unlock events, so an unlock is seen at the next 15 s
//! poll (or the exit check), not the instant it happens.
//!
//! **Where it looks.** Next to the DLL first, then the fork default folders
//! under `%APPDATA%`, then every cracker's fixed location ([`crackers`]). On
//! Linux, a Windows game running under Proton writes those "AppData" and
//! "Documents" locations INSIDE its Wine prefix, not the host's, so the
//! prefix's copies are searched too ([`WindowsRoots::in_prefix`]).
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
pub mod crackers;
pub mod discovery;
pub mod sse;

use log::{debug, info, warn};
use std::path::{Path, PathBuf};

// Re-export the public surface so existing `remote::goldberg::*` call sites
// in the `process`, `games` and achievements code keep compiling unchanged.
pub use achievements::GoldbergAchievement;
pub use config::write_custom_broadcasts;

/// The folder name Drop tells Goldberg to use via `local_save_path`.
/// Saves end up at `<dll_dir>/drop-goldberg/<AppID>/`.
pub const DROP_GSE_FOLDER: &str = "drop-goldberg";

/// Fallback directory names checked in AppData for emulators not configured by
/// Drop (or games launched outside Drop).
const APPDATA_FALLBACK_DIRS: &[&str] = &[
    "drop-goldberg",           // legacy Drop location
    "GSE Saves",               // GBE fork default (Windows is case-insensitive,
                               // so this also matches a lowercased "gse saves")
    "Goldberg SteamEmu Saves", // original Goldberg default
];

// ── Windows user-profile roots (host and Wine prefix) ─────────────────────

/// The Windows folders emulators and crackers save achievements into.
///
/// One set describes the host (what a Windows machine really has), another
/// the inside of a Wine/Proton prefix, which is where a Windows game running
/// under Proton on Linux writes "%APPDATA%" and friends. A `None` field means
/// that folder has no meaning for this set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowsRoots {
    /// `%APPDATA%` (Roaming).
    pub app_data: Option<PathBuf>,
    /// `%LOCALAPPDATA%`.
    pub local_app_data: Option<PathBuf>,
    /// The user's Documents folder.
    pub documents: Option<PathBuf>,
    /// `%PUBLIC%\Documents`.
    pub public_documents: Option<PathBuf>,
    /// `%ProgramData%`.
    pub program_data: Option<PathBuf>,
    /// Where a real Steam client might be installed.
    pub steam_dirs: Vec<PathBuf>,
}

impl WindowsRoots {
    /// The host's own folders. On Windows these are the real profile folders,
    /// with the stock `C:\` defaults when an environment variable is unset.
    /// On Linux the Windows-only ones are `None` unless the variable happens to
    /// be set: a bare `C:\...` literal there is a relative path into the
    /// working directory, not a location.
    pub fn host() -> Self {
        fn env_dir(var: &str) -> Option<PathBuf> {
            match std::env::var(var) {
                Ok(p) if !p.is_empty() => Some(PathBuf::from(p)),
                _ => None,
            }
        }

        let mut steam_dirs: Vec<PathBuf> =
            ["STEAM_PATH", "SteamPath"].iter().filter_map(|v| env_dir(v)).collect();
        let public_documents = env_dir("PUBLIC").map(|p| p.join("Documents"));
        let program_data = env_dir("ProgramData");

        // Stock locations when the variables are unset. Windows only.
        #[cfg(windows)]
        let (public_documents, program_data) = {
            steam_dirs.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
            steam_dirs.push(PathBuf::from("C:\\Program Files\\Steam"));
            (
                public_documents
                    .or_else(|| Some(PathBuf::from("C:\\Users\\Public\\Documents"))),
                program_data.or_else(|| Some(PathBuf::from("C:\\ProgramData"))),
            )
        };
        steam_dirs.dedup();

        Self {
            app_data: dirs::data_dir(),
            local_app_data: dirs::data_local_dir(),
            documents: dirs::document_dir(),
            public_documents,
            program_data,
            steam_dirs,
        }
    }

    /// The same folders inside a Wine/Proton prefix (the directory holding
    /// `drive_c`). `None` when the prefix has no user profile yet, which is
    /// the case until the game has been launched once.
    pub fn in_prefix(prefix: &Path) -> Option<Self> {
        let drive_c = prefix.join("drive_c");
        let user = wine_user_dir(prefix)?;
        Some(Self {
            app_data: Some(user.join("AppData").join("Roaming")),
            local_app_data: Some(user.join("AppData").join("Local")),
            documents: Some(user.join("Documents")),
            public_documents: Some(drive_c.join("users").join("Public").join("Documents")),
            program_data: Some(drive_c.join("ProgramData")),
            steam_dirs: vec![
                drive_c.join("Program Files (x86)").join("Steam"),
                drive_c.join("Program Files").join("Steam"),
            ],
        })
    }
}

/// Every set of Windows roots worth searching: the host first, then the
/// game's Wine prefix when there is one.
pub fn save_roots(wine_prefix: Option<&Path>) -> Vec<WindowsRoots> {
    let mut roots = vec![WindowsRoots::host()];
    if let Some(prefix) = wine_prefix {
        match WindowsRoots::in_prefix(prefix) {
            Some(r) => roots.push(r),
            None => debug!(
                "[ACH-GSE] Wine prefix {} has no user profile yet, not searching it",
                prefix.display()
            ),
        }
    }
    roots
}

/// Where a Wine prefix keeps the Windows user profile. Proton names it
/// `steamuser`; other prefixes use the real login name, so fall back to
/// whatever single non-Public user directory is there. (Same rule as the
/// cloud-save scanner's `wine_user_dir` in `save_sync/scan.rs`.)
fn wine_user_dir(prefix: &Path) -> Option<PathBuf> {
    let users = prefix.join("drive_c").join("users");
    let steamuser = users.join("steamuser");
    if steamuser.is_dir() {
        return Some(steamuser);
    }
    std::fs::read_dir(&users).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() && !name.eq_ignore_ascii_case("Public") {
            Some(path)
        } else {
            None
        }
    })
}

/// The Wine/Proton prefix a game's Windows build runs in on this machine.
///
/// Launch sets `WINEPREFIX=<DATA_ROOT_DIR>/pfx/<game_id>` (see
/// `process_handlers.rs` and `compute_wine_prefix_for` in `launch.rs`).
/// umu-launcher normally puts `drive_c` straight in there (with a `pfx`
/// symlink back to it for Proton's own layout); a prefix created by Proton
/// directly has it under `pfx/`. Whichever holds `drive_c` is returned.
///
/// Always `None` off Linux (Windows games run natively there) and when the
/// game has never been launched under Proton.
pub fn wine_prefix_for_game(game_id: &str) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let base = database::db::DATA_ROOT_DIR.join("pfx").join(game_id);
        resolve_prefix_root(&base)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = game_id;
        None
    }
}

/// `base` if it holds `drive_c`, else `base/pfx` if that does.
#[cfg(any(target_os = "linux", test))]
fn resolve_prefix_root(base: &Path) -> Option<PathBuf> {
    if base.join("drive_c").is_dir() {
        return Some(base.to_path_buf());
    }
    let nested = base.join("pfx");
    if nested.join("drive_c").is_dir() {
        return Some(nested);
    }
    None
}

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

    /// `true` for Goldberg/GBE (and Unknown, which Drop treats as Goldberg).
    /// SSE manages its own networking, so co-op broadcast seeding skips it.
    pub fn is_goldberg_like(&self) -> bool {
        matches!(
            self.emulator,
            SteamEmulator::Goldberg { .. } | SteamEmulator::Unknown { .. }
        )
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
/// DLL, under the host's %APPDATA%, and under the %APPDATA% inside the game's
/// Wine prefix when `wine_prefix` is given). The reader scans all of these and
/// keeps whichever actually contains unlocks, so Drop's own all-`false`
/// `drop-goldberg` file can't mask real unlocks GBE wrote to its default path.
pub fn gse_candidate_paths(
    app_id: &str,
    dll_dir: Option<&str>,
    wine_prefix: Option<&Path>,
) -> Vec<PathBuf> {
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
    // The same fork defaults under %APPDATA% (a fork not configured by Drop),
    // on the host and inside the Wine prefix.
    for roots in save_roots(wine_prefix) {
        let Some(data_dir) = roots.app_data else { continue };
        for name in APPDATA_FALLBACK_DIRS {
            let p = data_dir.join(name).join(app_id).join("achievements.json");
            if p.exists() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Marks every achievement in the local Goldberg-format unlock files for
/// these AppIDs as not earned, so a reset on the server is not undone by the
/// next launch re-reading them and so the player can earn them again.
///
/// Only rewrites GBE/Goldberg JSON files (the ones [`gse_candidate_paths`]
/// finds). Other crackers' formats (CODEX, OnlineFix, ...) are left alone;
/// for those the server's reset marker is what stops a re-report, and only
/// when the file carries unlock times.
///
/// Must only run while the game is NOT running: GBE holds its state in memory
/// and writes it back on exit, which would undo this.
pub fn clear_local_unlocks(
    app_ids: &[String],
    dll_dir: &str,
    wine_prefix: Option<&Path>,
) -> ClearReport {
    let mut report = ClearReport::default();
    for app_id in app_ids {
        for path in gse_candidate_paths(app_id, Some(dll_dir), wine_prefix) {
            let contents = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("[ACH-GSE] Reset: could not read {}: {e}", path.display());
                    report.failed.push((path, e.to_string()));
                    continue;
                }
            };
            let Some(cleared) = achievements::clear_earned_json(&contents) else {
                debug!("[ACH-GSE] Reset: nothing earned in {}", path.display());
                continue;
            };
            match std::fs::write(&path, cleared) {
                Ok(()) => {
                    info!("[ACH-GSE] Reset: cleared unlocks in {}", path.display());
                    report.cleared.push(path);
                }
                Err(e) => {
                    warn!("[ACH-GSE] Reset: could not write {}: {e}", path.display());
                    report.failed.push((path, e.to_string()));
                }
            }
        }
    }
    report
}

/// What [`clear_local_unlocks`] did.
#[derive(Debug, Default)]
pub struct ClearReport {
    /// Files rewritten with every achievement marked not earned.
    pub cleared: Vec<PathBuf>,
    /// Files that exist but could not be read or written, with the error.
    pub failed: Vec<(PathBuf, String)>,
}

/// Every Steam AppID the game's emulator may be saving under: the game's own
/// `steam_appid.txt` plus each AppID folder already present in
/// `<dll_dir>/drop-goldberg/`.
pub fn local_app_ids(dll_dir: &str) -> Vec<String> {
    let mut ids: Vec<String> = read_local_steam_appid(dll_dir).into_iter().collect();
    if let Ok(entries) = std::fs::read_dir(Path::new(dll_dir).join(DROP_GSE_FOLDER)) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir()
                && !name.is_empty()
                && name.chars().all(|c| c.is_ascii_digit())
                && !ids.contains(&name)
            {
                ids.push(name);
            }
        }
    }
    ids
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
///
/// `wine_prefix` is the game's Proton prefix on Linux ([`wine_prefix_for_game`]);
/// the AppData / Documents / ProgramData fallbacks are searched inside it as
/// well as on the host.
pub fn read_unlocks(
    app_id: &str,
    emulator_info: Option<&EmulatorInfo>,
    wine_prefix: Option<&Path>,
) -> Vec<GoldbergAchievement> {
    // 1. Emulator-specific read (Goldberg JSON map/array, or SSE ini).
    let mut unlocks = match emulator_info {
        Some(info) => match &info.emulator {
            SteamEmulator::Goldberg { dll_dir } | SteamEmulator::Unknown { dll_dir } => {
                achievements::read_goldberg_unlocks(app_id, Some(dll_dir.as_str()), wine_prefix)
            }
            SteamEmulator::SmartSteamEmu { save_path, .. } => {
                sse::read_sse_unlocks(save_path, app_id)
            }
        },
        None => achievements::read_goldberg_unlocks(app_id, None, wine_prefix),
    };

    // 2. Multi-cracker + real-Steam-client scan. Many games ship a cracker
    //    other than Goldberg/SSE (CODEX, RUNE, OnlineFix, EMPRESS, RLD!,
    //    CreamAPI, SKIDROW, 3DM, Razor1911) that writes unlocks to its own
    //    fixed location/format the read above never sees — the most common
    //    reason a game shows a silently-stuck 0/N. Merge any earned
    //    achievements found there (and from a real Steam install) into the set.
    let dll_dir = emulator_info.map(|i| i.dll_dir());
    let cracker_earned = crackers::scan_all_crackers(app_id, dll_dir, wine_prefix);
    merge_earned(&mut unlocks, cracker_earned);
    unlocks
}

/// Merge `extra` (all earned) into `base`: mark matching entries earned (with
/// the earlier unlock time) and append achievements `base` didn't already have.
fn merge_earned(base: &mut Vec<GoldbergAchievement>, extra: Vec<GoldbergAchievement>) {
    use std::collections::HashMap;
    let mut index: HashMap<String, usize> =
        base.iter().enumerate().map(|(i, a)| (a.name.clone(), i)).collect();
    for ach in extra {
        if let Some(&i) = index.get(&ach.name) {
            let existing = &mut base[i];
            existing.earned = true;
            if ach.earned_time != 0
                && (existing.earned_time == 0 || ach.earned_time < existing.earned_time)
            {
                existing.earned_time = ach.earned_time;
            }
        } else {
            index.insert(ach.name.clone(), base.len());
            base.push(ach);
        }
    }
}

/// Returns only the *earned* achievements for a game.
pub fn read_earned(
    app_id: &str,
    emulator_info: Option<&EmulatorInfo>,
    wine_prefix: Option<&Path>,
) -> Vec<GoldbergAchievement> {
    let earned: Vec<_> = read_unlocks(app_id, emulator_info, wine_prefix)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "drop-goldberg-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn prefix_root_found_at_top_level_or_under_pfx() {
        let flat = temp_dir("flat");
        std::fs::create_dir_all(flat.join("drive_c")).unwrap();
        assert_eq!(resolve_prefix_root(&flat), Some(flat.clone()));

        let nested = temp_dir("nested");
        std::fs::create_dir_all(nested.join("pfx").join("drive_c")).unwrap();
        assert_eq!(resolve_prefix_root(&nested), Some(nested.join("pfx")));

        let empty = temp_dir("empty");
        assert_eq!(resolve_prefix_root(&empty), None);

        for d in [flat, nested, empty] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn prefix_roots_point_inside_steamuser() {
        let pfx = temp_dir("roots");
        let user = pfx.join("drive_c").join("users").join("steamuser");
        std::fs::create_dir_all(&user).unwrap();
        let roots = WindowsRoots::in_prefix(&pfx).expect("roots");
        assert_eq!(roots.app_data, Some(user.join("AppData").join("Roaming")));
        assert_eq!(
            roots.public_documents,
            Some(pfx.join("drive_c").join("users").join("Public").join("Documents"))
        );
        let _ = std::fs::remove_dir_all(pfx);
    }

    #[test]
    fn candidate_paths_include_prefix_appdata() {
        let pfx = temp_dir("cands");
        let file = pfx
            .join("drive_c/users/steamuser/AppData/Roaming/GSE Saves/480/achievements.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "{}").unwrap();
        let found = gse_candidate_paths("480", None, Some(&pfx));
        assert!(found.contains(&file), "{found:?}");
        let _ = std::fs::remove_dir_all(pfx);
    }
}
