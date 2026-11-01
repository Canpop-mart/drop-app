use std::{
    collections::HashMap,
    fs::create_dir_all,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
};

use database::{
    Settings, borrow_db_checked, borrow_db_mut_checked, db::DATA_ROOT_DIR, debug::SystemData,
};
use download_manager::error::DownloadManagerError;
use games::scan::{ScanReport, scan_install_dir};
use tauri::AppHandle;
use utils::app_emit;
use log::error;
use serde::Serialize;
use serde_json::Value;
use sysinfo::System;

// Will, in future, return disk/remaining size
// Just returns the directories that have been set up
#[tauri::command]
pub fn fetch_download_dir_stats() -> Vec<PathBuf> {
    let lock = borrow_db_checked();
    lock.applications.install_dirs.clone()
}

#[tauri::command]
pub fn delete_download_dir(index: usize) -> Result<(), String> {
    let mut lock = borrow_db_mut_checked();
    if index >= lock.applications.install_dirs.len() {
        return Err(format!("No install directory at index {index}"));
    }
    lock.applications.install_dirs.remove(index);
    Ok(())
}

/// Add an install directory and import the games already in it.
///
/// The folder may hold anything. Games Drop installed there before, from this
/// client or another (a reset, a second client, a folder copied over), carry a
/// `.dropdata` and are imported by the scan; everything else is left alone.
///
/// Async, so it runs off the main thread: the scan reads a whole folder tree,
/// which is slow on an SD card or a network share.
#[tauri::command]
pub async fn add_download_dir(
    new_dir: PathBuf,
    app_handle: AppHandle,
) -> Result<ScanReport, DownloadManagerError<()>> {
    let new_dir_path = Path::new(&new_dir);
    if new_dir_path.exists() && !new_dir_path.is_dir() {
        return Err(Error::new(
            ErrorKind::NotADirectory,
            "Selected path is not a folder",
        )
        .into());
    }

    // Checked before the folder is created, so a refused one isn't left behind
    // (on an SD card, say). The folders are copied out first: the check reads
    // the disk, which must not happen while holding the database lock.
    let existing = borrow_db_checked().applications.install_dirs.clone();
    check_new_install_dir(&existing, &new_dir)?;

    if !new_dir_path.exists() {
        create_dir_all(new_dir_path)?;
    }
    {
        // The duplicate check again under the lock: two quick adds of the same
        // folder both pass the first check while this runs off the main thread.
        let mut lock = borrow_db_mut_checked();
        if lock.applications.install_dirs.contains(&new_dir) {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                "Selected directory already exists in database",
            )
            .into());
        }
        lock.applications.install_dirs.push(new_dir.clone());
    }

    let report = scan_install_dir(&new_dir);
    if report.installed + report.partial > 0 {
        app_emit!(&app_handle, "update_library", ());
        // Imported installs can't launch until their version details are
        // stored. Fetched in the background, so a slow or unreachable server
        // doesn't hold up the folder being added; skipped entirely offline
        // (the next online library fetch does it instead).
        if !app_is_offline(&app_handle) {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                games::library::backfill_game_versions(Some(&app_handle)).await;
            });
        }
    }
    Ok(report)
}

/// Refuse a folder that is already an install folder, or overlaps one.
fn check_new_install_dir(
    existing: &[PathBuf],
    new_dir: &Path,
) -> Result<(), DownloadManagerError<()>> {
    if existing.iter().any(|d| d == new_dir) {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            "Selected directory already exists in database",
        )
        .into());
    }
    // One folder inside another would scan the same games twice and let a
    // download land inside another game's install.
    if let Some(dir) = existing.iter().find(|d| dirs_overlap(d, new_dir)) {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            format!("Selected folder overlaps the install folder {}", dir.display()),
        )
        .into());
    }
    Ok(())
}

/// True when the app shouldn't touch the network: offline mode is on, or the
/// app found the server unreachable.
pub(crate) fn app_is_offline(app_handle: &AppHandle) -> bool {
    use client::{app_state::AppState, app_status::AppStatus};
    use std::sync::nonpoison::Mutex;
    use tauri::Manager;
    borrow_db_checked().settings.force_offline
        || app_handle
            .try_state::<Mutex<AppState>>()
            .is_some_and(|s| s.lock().status == AppStatus::Offline)
}

/// True when one folder is the other or sits inside it. Compared after
/// resolving links and `..`, so two spellings of one folder count as the same.
/// A folder that doesn't exist yet is resolved through its nearest existing
/// parent, so a new `drop-games` inside an install folder is still caught.
/// A folder with no resolvable parent at all (an unplugged SD card) overlaps
/// nothing.
fn dirs_overlap(a: &Path, b: &Path) -> bool {
    let (Some(a), Some(b)) = (resolve_for_compare(a), resolve_for_compare(b)) else {
        return false;
    };
    a.starts_with(&b) || b.starts_with(&a)
}

/// `path` canonicalized, or, for a path that doesn't exist yet, its nearest
/// existing ancestor canonicalized with the missing part appended.
fn resolve_for_compare(path: &Path) -> Option<PathBuf> {
    let mut missing: Vec<&std::ffi::OsStr> = Vec::new();
    let mut current = path;
    loop {
        if let Ok(real) = current.canonicalize() {
            let mut out = real;
            for part in missing.iter().rev() {
                out.push(part);
            }
            return Some(out);
        }
        // `..` or `.` in the part that doesn't exist yet can't be resolved
        // without the folder; treat the path as unresolvable.
        let name = current.file_name()?;
        missing.push(name);
        current = current.parent()?;
    }
}

/// Opens an install directory (by its index in the configured list) in the
/// platform file manager, mirroring `delete_download_dir`'s index-based
/// access so the frontend can reuse the `dirIdx` it already has.
#[tauri::command]
pub fn open_download_dir(index: usize, app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = {
        let lock = borrow_db_checked();
        lock.applications
            .install_dirs
            .get(index)
            .cloned()
            .ok_or_else(|| format!("No install directory at index {index}"))?
    };
    app_handle
        .opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| format!("Failed to open install directory: {e}"))
}

/// Keys the frontend is allowed to patch via `update_settings`. Must stay in
/// sync with `Settings` field names (camelCase serde form).
const ALLOWED_SETTINGS_KEYS: &[&str] = &[
    "autostart",
    "maxDownloadThreads",
    "forceOffline",
    "globalMangohud",
    // sunshineUsername/sunshinePassword are deliberately absent: the client
    // generates that password itself and Sunshine hashes it into its own
    // credentials file. A frontend patch would only ever desync the two.
    "raUsername",
    "raToken",
    "cloudSavesEnabled",
    "deviceName",
    "displayName",
    "streamingQuality",
    "streamingResolution",
    "streamingHdr",
    "streamingAutoResolution",
    "streamingDisplay",
    "streamingAdapter",
    "streamingAudioSink",
    "streamingVirtualSink",
    "streamingEnabled",
];

/// Max length accepted for free-form string settings. Prevents a rogue
/// frontend bug from stuffing multi-MB blobs into the settings blob.
const MAX_SETTING_STRING_LEN: usize = 4096;

#[tauri::command]
pub fn update_settings(new_settings: Value) {
    let mut db_lock = borrow_db_mut_checked();
    let mut current_settings =
        serde_json::to_value(db_lock.settings.clone()).expect("Failed to parse existing settings");
    let values = match new_settings.as_object() {
        Some(values) => values,
        None => {
            error!("Could not parse settings values into object");
            return;
        }
    };
    for (key, value) in values {
        if !ALLOWED_SETTINGS_KEYS.contains(&key.as_str()) {
            error!("Rejecting unknown settings key: {key}");
            continue;
        }
        if let Some(s) = value.as_str()
            && s.len() > MAX_SETTING_STRING_LEN
        {
            error!(
                "Rejecting oversized settings value for {key} ({} bytes)",
                s.len()
            );
            continue;
        }
        if key == "maxDownloadThreads" {
            let threads = value.as_u64().unwrap_or(0);
            if !(1..=64).contains(&threads) {
                error!("Rejecting out-of-range maxDownloadThreads: {threads}");
                continue;
            }
        }
        current_settings[key] = value.clone();
    }
    let new_settings: Settings = match serde_json::from_value(current_settings) {
        Ok(settings) => settings,
        Err(e) => {
            error!("Could not parse settings with error {}", e);
            return;
        }
    };
    db_lock.settings = new_settings;
}
#[tauri::command]
pub fn fetch_settings() -> Settings {
    borrow_db_checked().settings.clone()
}

/// Save the RetroAchievements Connect token the Drop server handed back after
/// linking, as the local copy RetroArch signs in with.
///
/// Linking itself goes through the server (`PUT
/// /api/v1/user/external-accounts/retroachievements` via server://), because
/// the server needs the RA username to record unlocks. The frontend calls
/// this with the `externalId` and `connectToken` from that response. The
/// password never reaches this process.
#[tauri::command]
pub fn ra_store_credentials(username: String, connect_token: String) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() || connect_token.is_empty() {
        return Err("The server did not return RetroAchievements credentials".to_string());
    }
    let mut db = borrow_db_mut_checked();
    db.settings.ra_username = username.to_string();
    db.settings.ra_token = connect_token;
    // This token is brand new, so whatever expired before is history —
    // RetroArch gets credentials injected again from the next launch.
    db.settings.ra_expired_token = String::new();
    Ok(())
}

/// Re-sync the local copy of the RA credentials with the server now, the same
/// way a RetroArch launch does (see `sync_ra_credentials`): the server's
/// account replaces the local one, a definite "not linked" clears it, and an
/// unreachable server leaves it alone. Returns what happened, including
/// `status: "unreachable"` with the error, so the settings screens can say so
/// instead of silently doing nothing.
#[tauri::command]
pub async fn ra_refresh_credentials() -> ::remote::retroarch::ra::RaSyncReport {
    ::remote::retroarch::ra::sync_ra_credentials().await.1
}

/// Clear the local copy of the RA credentials. The settings screens call
/// this after unlinking on the server; on its own it leaves any
/// server-linked account intact, and the next RetroArch launch would fetch
/// and cache that account's token again.
#[tauri::command]
pub fn ra_clear_credentials() {
    let mut db = borrow_db_mut_checked();
    db.settings.ra_username = String::new();
    db.settings.ra_token = String::new();
    // Nothing local left to be expired. If the server-linked account is also
    // stale, the next launch's exit check will say so again.
    db.settings.ra_expired_token = String::new();
}

/// Describes how the app was packaged, affecting update behavior.
// Most variants are constructed only under their platform's `#[cfg]`
// (Flatpak/AppImage/SystemPackage on Linux, MacOsBundle on macOS), so they
// read as "never constructed" on a Windows build. Allow it rather than gate
// every variant individually.
#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageFormat {
    /// Running inside a Flatpak sandbox — updates via `flatpak update`
    Flatpak,
    /// AppImage — can self-update via Tauri updater
    AppImage,
    /// System package (DEB/RPM) — updates via system package manager
    SystemPackage,
    /// Windows installer (NSIS) — self-update via Tauri updater
    WindowsInstaller,
    /// macOS DMG/app bundle — self-update via Tauri updater
    MacOsBundle,
    /// Unknown packaging
    Unknown,
}

#[tauri::command]
pub fn detect_package_format() -> PackageFormat {
    #[cfg(target_os = "windows")]
    { PackageFormat::WindowsInstaller }

    #[cfg(target_os = "macos")]
    { PackageFormat::MacOsBundle }

    #[cfg(target_os = "linux")]
    {
        // Flatpak sets /.flatpak-info and runs inside /app/
        if Path::new("/.flatpak-info").exists() {
            return PackageFormat::Flatpak;
        }

        // AppImage sets the APPIMAGE environment variable
        if std::env::var("APPIMAGE").is_ok() {
            return PackageFormat::AppImage;
        }

        // Check if we were installed by a system package manager
        let exe = std::env::current_exe().unwrap_or_default();
        let exe_str = exe.to_string_lossy();
        if exe_str.starts_with("/usr/") || exe_str.starts_with("/opt/") {
            return PackageFormat::SystemPackage;
        }

        PackageFormat::Unknown
    }
}

/// Detect available SD card / removable storage mount points (Linux only).
/// Returns paths like /run/media/mmcblk0p1 that can be used as install directories.
#[tauri::command]
pub fn detect_removable_storage() -> Vec<PathBuf> {
    // `results` is mutated only in the cfg(linux) block below, so mark it mut
    // unconditionally and allow the unused-mut warning on non-Linux. (Without
    // this, clippy --fix on Windows strips the `mut` and breaks the Linux build.)
    #[allow(unused_mut)]
    let mut results = Vec::new();

    #[cfg(target_os = "linux")]
    {
        // Steam Deck mounts SD cards at /run/media/mmcblk*
        if let Ok(entries) = std::fs::read_dir("/run/media") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check if it's a removable device (SD card, USB drive)
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("mmcblk") || name.starts_with("deck") {
                        results.push(path);
                    }
                }
            }
        }

        // Also check user media mounts (common on desktop Linux)
        if let Ok(user) = std::env::var("USER") {
            let user_media = PathBuf::from(format!("/run/media/{}", user));
            if user_media.is_dir()
                && let Ok(entries) = std::fs::read_dir(&user_media)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        results.push(path);
                    }
                }
            }
        }
    }

    results
}

#[tauri::command]
pub fn fetch_system_data() -> SystemData {
    let db_handle = borrow_db_checked();
    let client_id = db_handle
        .auth
        .as_ref()
        .map(|a| a.client_id.clone())
        .unwrap_or_default();
    SystemData::new(
        client_id,
        db_handle.base_url.clone(),
        DATA_ROOT_DIR.to_string_lossy().to_string(),
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
    )
}

/// Collects comprehensive system diagnostics for bug reports.
/// Includes OS, hardware, memory, disk, and client version info.
#[tauri::command]
pub fn collect_bug_report_diagnostics() -> HashMap<String, String> {
    let mut info = HashMap::new();

    // Client version
    info.insert("clientVersion".into(), env!("CARGO_PKG_VERSION").into());

    // OS info
    info.insert("os".into(), System::name().unwrap_or_else(|| "unknown".into()));
    info.insert("osVersion".into(), System::os_version().unwrap_or_else(|| "unknown".into()));
    info.insert("kernelVersion".into(), System::kernel_version().unwrap_or_else(|| "unknown".into()));
    info.insert("hostname".into(), System::host_name().unwrap_or_else(|| "unknown".into()));
    info.insert("arch".into(), std::env::consts::ARCH.into());

    // CPU & Memory — use new_all() to refresh everything in one shot
    let sys = System::new_all();
    let cpus = sys.cpus();
    if let Some(cpu) = cpus.first() {
        info.insert("cpu".into(), cpu.brand().to_string());
    }
    info.insert("cpuCount".into(), cpus.len().to_string());

    // Memory
    let total_mem_gb = sys.total_memory() as f64 / 1_073_741_824.0;
    let used_mem_gb = sys.used_memory() as f64 / 1_073_741_824.0;
    info.insert("totalMemoryGB".into(), format!("{:.1}", total_mem_gb));
    info.insert("usedMemoryGB".into(), format!("{:.1}", used_mem_gb));

    // Disk space for install dirs
    let db_handle = borrow_db_checked();
    let install_dirs = &db_handle.applications.install_dirs;
    for (i, dir) in install_dirs.iter().enumerate() {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        for disk in disks.list() {
            if dir.starts_with(disk.mount_point()) {
                let free_gb = disk.available_space() as f64 / 1_073_741_824.0;
                info.insert(
                    format!("installDir{}_freeGB", i),
                    format!("{:.1}", free_gb),
                );
                break;
            }
        }
    }

    // Session type
    let session_type = ::client::app_state::SessionType::detect();
    let session_str = match &session_type {
        st if *st == ::client::app_state::SessionType::Desktop => "Desktop",
        st if *st == ::client::app_state::SessionType::Gamescope => "Gamescope",
        st if *st == ::client::app_state::SessionType::SteamDeckDesktop => "SteamDeckDesktop",
        _ => "Unknown",
    };
    info.insert("sessionType".into(), session_str.into());

    // Data directory
    info.insert("dataDir".into(), DATA_ROOT_DIR.to_string_lossy().to_string());

    // Server URL
    info.insert("serverUrl".into(), db_handle.base_url.clone());

    info
}

/// Reads the last N lines of the client log file for bug report attachment.
/// Output is passed through the secret redactor so tokens never leave
/// the local machine in plaintext.
#[tauri::command]
pub fn collect_bug_report_logs(max_lines: Option<usize>) -> Result<String, String> {
    let log_path = DATA_ROOT_DIR.join("drop.log");
    let content = std::fs::read_to_string(&log_path)
        .map_err(|e| format!("Failed to read log file: {}", e))?;

    let max = max_lines.unwrap_or(200);
    let lines: Vec<&str> = content.lines().collect();
    let start = if lines.len() > max { lines.len() - max } else { 0 };
    let tail = lines[start..].join("\n");
    Ok(::utils::redact(&tail))
}

/// Max bytes of `drop.log` we attach as a file to a bug report. The server
/// has its own upload limits; this is belt-and-braces to keep the payload
/// reasonable for reviewers and to avoid slow uploads on Deck LTE tethers.
const MAX_LOGFILE_BYTES: usize = 5 * 1024 * 1024;

/// Full-file variant of [`collect_bug_report_logs`] — returns the entire
/// `drop.log` (tail-truncated to ~5 MB) after secret redaction, as a plain
/// UTF-8 string. The frontend wraps it in a Blob for multipart upload.
///
/// We return a String (not Vec<u8>) because Tauri serializes Vec<u8> as a
/// JSON array of numbers — a 5MB log would produce ~20MB of JSON text on
/// the IPC bridge, which crashed the webview on Steam Deck.
#[tauri::command]
pub fn collect_bug_report_log_file() -> Result<LogFilePayload, String> {
    let log_path = DATA_ROOT_DIR.join("drop.log");
    let raw = std::fs::read_to_string(&log_path)
        .map_err(|e| format!("Failed to read log file: {}", e))?;

    let redacted = ::utils::redact(&raw);
    let truncated = redacted.len() > MAX_LOGFILE_BYTES;
    let content = if truncated {
        let mut start = redacted.len() - MAX_LOGFILE_BYTES;
        // Advance to the next valid UTF-8 char boundary so we don't split
        // a codepoint in half.
        while start < redacted.len() && !redacted.is_char_boundary(start) {
            start += 1;
        }
        redacted[start..].to_string()
    } else {
        redacted
    };

    Ok(LogFilePayload {
        filename: "drop.log".into(),
        content,
        truncated,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilePayload {
    pub filename: String,
    pub content: String,
    pub truncated: bool,
}

/// Opens the folder containing `drop.log` in the platform file manager.
/// Useful on Steam Deck Desktop Mode when a user wants to grab the log
/// manually before filing an issue.
#[tauri::command]
pub fn open_log_folder(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = DATA_ROOT_DIR.to_path_buf();
    app_handle
        .opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| format!("Failed to open log folder: {}", e))
}

/// Opens the SteamOS on-screen keyboard via `steam://open/keyboard`.
/// Only has an effect on systems where Steam is running and registered
/// as the handler for `steam://`; we still return Ok in other cases and
/// let the caller fall back to the custom keyboard.
#[tauri::command]
pub fn open_steam_keyboard(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app_handle
        .opener()
        .open_url("steam://open/keyboard", None::<&str>)
        .map_err(|e| format!("Failed to open Steam keyboard: {}", e))
}

// ── Windows Defender "safe zone" ─────────────────────────────────────────
// Cracked game payloads (OnlineFix/Goldberg DLLs, repacks) and Drop's own
// unsigned binary routinely trip Defender heuristics → quarantine → broken
// installs and launches. We let the user carve out the paths Drop owns.
//
// Reading the current exclusion LIST requires admin, so the unelevated status
// only reports real-time-protection state + the paths we'd manage. Adding the
// exclusions spawns one UAC-elevated PowerShell — the UAC prompt is the
// consent. Everything here is a no-op off Windows.

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefenderStatus {
    /// False off Windows / when Defender cmdlets are unavailable.
    pub supported: bool,
    /// Real-time protection on? None when unknown. Readable unelevated.
    pub realtime_enabled: Option<bool>,
    /// Paths Drop would exclude: games dirs + data dir + install dir. (The
    /// live exclusion list can't be read without admin, so we surface what
    /// we'd add rather than what's already there.)
    pub managed_paths: Vec<String>,
}

/// Games install dirs + Drop's data dir + Drop's own install dir.
fn defender_managed_paths() -> Vec<String> {
    let mut paths: Vec<String> = {
        let lock = borrow_db_checked();
        lock.applications
            .install_dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    };
    paths.push(DATA_ROOT_DIR.to_string_lossy().to_string());
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        paths.push(dir.to_string_lossy().to_string());
    }
    paths.sort();
    paths.dedup();
    paths
}

#[tauri::command]
pub fn get_defender_status() -> DefenderStatus {
    let managed_paths = defender_managed_paths();
    #[cfg(windows)]
    {
        let realtime_enabled = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-MpComputerStatus).RealTimeProtectionEnabled",
            ])
            .output()
            .ok()
            .and_then(|o| {
                match String::from_utf8_lossy(&o.stdout).trim().to_lowercase().as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                }
            });
        DefenderStatus {
            supported: true,
            realtime_enabled,
            managed_paths,
        }
    }
    #[cfg(not(windows))]
    {
        DefenderStatus {
            supported: false,
            realtime_enabled: None,
            managed_paths,
        }
    }
}

#[tauri::command]
pub fn add_defender_exclusions() -> Result<(), String> {
    #[cfg(windows)]
    {
        let paths = defender_managed_paths();
        if paths.is_empty() {
            return Err("No Drop paths to exclude".to_string());
        }
        // PowerShell array of single-quoted paths (escape embedded quotes).
        let ps_array = paths
            .iter()
            .map(|p| format!("'{}'", p.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",");
        let inner = format!("Add-MpPreference -ExclusionPath {ps_array}");
        // Base64(UTF-16LE) the inner command for -EncodedCommand so we never
        // fight nested PowerShell quoting across the elevation hop.
        let encoded = {
            use base64::{engine::general_purpose::STANDARD, Engine};
            let utf16: Vec<u8> =
                inner.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            STANDARD.encode(utf16)
        };
        // Start-Process -Verb RunAs raises the UAC prompt — that IS the
        // consent. -Wait blocks until the elevated process exits; declining
        // UAC makes Start-Process throw, so the outer shell exits non-zero.
        let outer = format!(
            "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden \
             -ArgumentList '-NoProfile','-EncodedCommand','{encoded}'"
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &outer])
            .status()
            .map_err(|e| format!("Failed to start elevated PowerShell: {e}"))?;
        if !status.success() {
            return Err(
                "Couldn't add the Defender exclusions — the elevation prompt may have been declined."
                    .to_string(),
            );
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Windows Defender exclusions only apply on Windows.".to_string())
    }
}

#[cfg(test)]
mod install_dir_tests {
    use super::dirs_overlap;

    #[test]
    fn nested_and_equal_folders_overlap() {
        let root = std::env::temp_dir().join("drop_settings_overlap_test");
        let _ = std::fs::remove_dir_all(&root);
        let games = root.join("games");
        let inner = games.join("Some Game");
        let sibling = root.join("other");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();

        assert!(dirs_overlap(&games, &games));
        assert!(dirs_overlap(&games, &inner));
        assert!(dirs_overlap(&inner, &games));
        assert!(dirs_overlap(&games, &games.join("..").join("games")));
        assert!(!dirs_overlap(&games, &sibling));
        // A folder that doesn't exist yet, inside an install folder.
        assert!(dirs_overlap(&root, &games.join("not-yet").join("drop-games")));
        assert!(!dirs_overlap(&sibling, &games.join("not-yet")));
        // A drive that isn't there at all.
        assert!(!dirs_overlap(&games, std::path::Path::new("/definitely-missing-drop-root/x")));
    }
}
