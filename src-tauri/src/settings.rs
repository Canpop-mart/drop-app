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
use games::scan::scan_install_dirs;
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
pub fn delete_download_dir(index: usize) {
    let mut lock = borrow_db_mut_checked();
    lock.applications.install_dirs.remove(index);
}

#[tauri::command]
pub fn add_download_dir(new_dir: PathBuf) -> Result<(), DownloadManagerError<()>> {
    // Check the new directory is all good
    let new_dir_path = Path::new(&new_dir);
    if new_dir_path.exists() {
        let dir_contents = new_dir_path.read_dir()?;
        if dir_contents.count() != 0 {
            return Err(Error::new(
                ErrorKind::DirectoryNotEmpty,
                "Selected directory cannot contain any existing files",
            )
            .into());
        }
    } else {
        create_dir_all(new_dir_path)?;
    }

    // Add it to the dictionary
    let mut lock = borrow_db_mut_checked();
    if lock.applications.install_dirs.contains(&new_dir) {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            "Selected directory already exists in database",
        )
        .into());
    }
    lock.applications.install_dirs.push(new_dir);
    drop(lock);

    scan_install_dirs();

    Ok(())
}

/// Keys the frontend is allowed to patch via `update_settings`. Must stay in
/// sync with `Settings` field names (camelCase serde form).
const ALLOWED_SETTINGS_KEYS: &[&str] = &[
    "autostart",
    "maxDownloadThreads",
    "forceOffline",
    "globalMangohud",
    "sunshineUsername",
    "sunshinePassword",
    "raUsername",
    "raToken",
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

/// Exchange RA username + password for a Connect token and save both to
/// local settings. The password never hits disk. Called from the BPM
/// Achievements settings when the user links their RA account.
#[tauri::command]
pub async fn ra_login_and_save(username: String, password: String) -> Result<String, String> {
    if username.trim().is_empty() || password.is_empty() {
        return Err("Username and password required".to_string());
    }

    #[derive(serde::Deserialize)]
    struct RALoginResponse {
        #[serde(rename = "Success")]
        success: Option<bool>,
        #[serde(rename = "Token")]
        token: Option<String>,
        #[serde(rename = "Error")]
        error: Option<String>,
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .post("https://retroachievements.org/dorequest.php")
        .form(&[("r", "login2"), ("u", username.trim()), ("p", &password)])
        .send()
        .await
        .map_err(|e| format!("Network error contacting RetroAchievements: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("RetroAchievements returned HTTP {}", response.status()));
    }

    let body: RALoginResponse = response
        .json()
        .await
        .map_err(|e| format!("Could not parse RA response: {e}"))?;

    if body.success != Some(true) {
        return Err(body.error.unwrap_or_else(|| "Login rejected".to_string()));
    }

    let token = body.token.ok_or_else(|| "RA response missing Token".to_string())?;
    if token.is_empty() {
        return Err("RA returned empty Token".to_string());
    }

    {
        let mut db = borrow_db_mut_checked();
        db.settings.ra_username = username.trim().to_string();
        db.settings.ra_token = token;
    }

    Ok(username.trim().to_string())
}

/// Unlink the locally-configured RA account. Leaves the server-linked
/// account (if any) intact so RetroArch falls back to that.
#[tauri::command]
pub fn ra_clear_credentials() {
    let mut db = borrow_db_mut_checked();
    db.settings.ra_username = String::new();
    db.settings.ra_token = String::new();
}

/// Describes how the app was packaged, affecting update behavior.
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
    { return PackageFormat::WindowsInstaller; }

    #[cfg(target_os = "macos")]
    { return PackageFormat::MacOsBundle; }

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
