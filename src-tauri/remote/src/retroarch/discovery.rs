//! RetroArch install discovery.
//!
//! Detects whether an emulator install directory is a RetroArch install and
//! locates the AppImage portable-`$HOME` config directory used on Linux.

use log::{debug, info};
use std::path::{Path, PathBuf};

/// Returns `true` if `dir` looks like a RetroArch installation.
///
/// Checks, in order: well-known executable names, any file whose name
/// contains `retroarch`, a `retroarch.cfg`, or a `cores/` directory.
pub fn is_retroarch(dir: &Path) -> bool {
    const EXECUTABLES: &[&str] = &[
        "retroarch",
        "retroarch.exe",
        "RetroArch.exe",
        "retroarch.AppImage",
    ];
    for exe in EXECUTABLES {
        if dir.join(exe).exists() {
            info!("[RETROARCH] is_retroarch: matched exact name {exe:?}");
            return true;
        }
    }

    // Scan for any file whose name contains "retroarch" — catches variants
    // like "RetroArch-Linux-x86_64.AppImage".
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name_lower = entry.file_name().to_string_lossy().to_lowercase();
            if name_lower.contains("retroarch")
                && (name_lower.ends_with(".appimage")
                    || name_lower.ends_with(".exe")
                    || !name_lower.contains('.'))
            {
                info!("[RETROARCH] is_retroarch: matched by scan: {name_lower:?}");
                return true;
            }
        }
    }

    if dir.join("retroarch.cfg").exists() {
        info!("[RETROARCH] is_retroarch: matched via retroarch.cfg");
        return true;
    }
    if dir.join("cores").is_dir() {
        info!("[RETROARCH] is_retroarch: matched via cores/ directory");
        return true;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        let files: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        debug!("[RETROARCH] is_retroarch: NO match in {dir:?}, contents: {files:?}");
    }

    false
}

/// Finds the AppImage portable-`$HOME` config directory inside `emu_root`.
///
/// RetroArch AppImages create a portable `$HOME` at `<AppImage-filename>.home/`
/// and read config from `$HOME/.config/retroarch/`. Writing there makes Drop's
/// settings the *base* config rather than just an `--appendconfig` overlay —
/// critical for Gamescope/Steam Deck where the video driver must be right from
/// the start.
///
/// Returns the `.config/retroarch/` path, or `None` if no AppImage is present.
pub fn find_appimage_config_dir(emu_root: &Path) -> Option<PathBuf> {
    let appimage = find_appimage_binary(emu_root)?;
    let name = appimage.file_name()?.to_string_lossy().to_string();
    let config_dir = emu_root
        .join(format!("{name}.home"))
        .join(".config")
        .join("retroarch");
    info!("[RETROARCH] AppImage home config dir: {}", config_dir.display());
    Some(config_dir)
}

/// Finds the RetroArch AppImage binary in `emu_root`, if any. Used on every
/// platform for `.home` derivation; shader extraction (Linux-only) reuses it.
pub fn find_appimage_binary(emu_root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(emu_root).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("retroarch") && name.ends_with(".appimage") {
            return Some(entry.path());
        }
    }
    None
}
