use std::{
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
            if user_media.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&user_media) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            results.push(path);
                        }
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
