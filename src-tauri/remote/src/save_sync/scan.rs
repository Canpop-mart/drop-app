//! Local save-file discovery and write-back.
//!
//! Two save sources are scanned:
//!
//! * **Emulator saves** — RetroArch keeps them under `{emu_root}/drop-saves/
//!   {game_id}/{saves,states}`; [`scan_emu_saves`] walks those directories.
//! * **PC saves** — discovered by shelling out to Ludusavi, whose database
//!   knows where each game stores its saves; [`scan_pc_saves`].
//!
//! [`write_downloaded_save`] / [`write_downloaded_pc_save`] put cloud copies
//! back, always backing up any existing file first.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use log::{info, warn};

use super::LocalSaveFile;

/// Compute the MD5 hash of a file on disk.
pub fn md5_file(path: &Path) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let digest = md5::compute(&data);
    Ok(format!("{:x}", digest))
}

/// Scan RetroArch save directories for a game.
/// Returns a list of local save files with their hashes.
pub fn scan_emu_saves(emu_root: &Path, game_id: &str) -> Vec<LocalSaveFile> {
    let saves_base = emu_root.join("drop-saves").join(game_id);
    let mut files = Vec::new();

    for (subdir, save_type) in &[("saves", "save"), ("states", "state")] {
        let dir = saves_base.join(subdir);
        if !dir.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let meta = match fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let modified_at = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let hash = match md5_file(&path) {
                    Ok(h) => h,
                    Err(e) => {
                        warn!("[SAVE-SYNC] Failed to hash {}: {}", path.display(), e);
                        continue;
                    }
                };
                files.push(LocalSaveFile {
                    filename: filename.clone(),
                    save_type: save_type.to_string(),
                    path,
                    data_hash: hash,
                    size: meta.len(),
                    modified_at,
                });
            }
        }
    }

    files
}

/// Write a downloaded save file to the correct local path.
pub fn write_downloaded_save(
    emu_root: &Path,
    game_id: &str,
    filename: &str,
    save_type: &str,
    data: &[u8],
) -> Result<PathBuf, String> {
    let subdir = match save_type {
        "save" => "saves",
        "state" => "states",
        _ => "saves", // fallback
    };
    let dir = emu_root.join("drop-saves").join(game_id).join(subdir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create save dir {}: {e}", dir.display()))?;
    let dest = dir.join(filename);

    // Create backup if file exists
    if dest.exists() {
        let bak = dest.with_extension(format!(
            "{}.bak",
            dest.extension().unwrap_or_default().to_string_lossy()
        ));
        let _ = fs::copy(&dest, &bak);
    }

    fs::write(&dest, data).map_err(|e| format!("Failed to write save {}: {e}", dest.display()))?;
    Ok(dest)
}

// ── Ludusavi PC save scanning ──────────────────────────────────────────

/// Find the Ludusavi binary (bundled in Drop's tools dir, or on PATH).
fn find_ludusavi() -> Option<PathBuf> {
    let tools = dirs::data_dir()?.join("drop").join("tools");
    #[cfg(target_os = "windows")]
    let bundled = tools.join("ludusavi").join("ludusavi.exe");
    #[cfg(not(target_os = "windows"))]
    let bundled = tools.join("ludusavi").join("ludusavi");

    if bundled.exists() {
        return Some(bundled);
    }

    // Check PATH
    if let Ok(output) = std::process::Command::new("ludusavi").arg("--version").output() {
        if output.status.success() {
            return Some(PathBuf::from("ludusavi"));
        }
    }

    None
}

/// Scan PC game saves using Ludusavi.
/// `game_name` is the display name to search for; `steam_app_id` is optional.
/// Returns files as `LocalSaveFile` with save_type = "pc".
pub fn scan_pc_saves(game_name: &str, steam_app_id: Option<&str>) -> Vec<LocalSaveFile> {
    let ludusavi = match find_ludusavi() {
        Some(p) => p,
        None => {
            info!("[SAVE-SYNC] Ludusavi not found, skipping PC save scan");
            return Vec::new();
        }
    };

    // Resolve canonical name from Steam ID if available
    let resolved_name = steam_app_id.and_then(|id| {
        let output = std::process::Command::new(&ludusavi)
            .args(["find", "--api", "--steam-id", id])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str::<serde_json::Value>(&s)
            .ok()
            .and_then(|v| v.get("games")?.as_object()?.keys().next().map(|k| k.to_string()))
    });

    let search_name = resolved_name.as_deref().unwrap_or(game_name);
    info!("[SAVE-SYNC] Ludusavi scanning for '{}'", search_name);

    // Run "backup --preview --api <name>" to discover save files
    let output = std::process::Command::new(&ludusavi)
        .args(["backup", "--preview", "--api", search_name])
        .output();

    // Retry with the original name if resolved name found nothing
    let output = match &output {
        Ok(o) if !o.status.success() || o.stdout.len() < 50 => {
            if search_name != game_name {
                info!(
                    "[SAVE-SYNC] Retrying Ludusavi with original name: '{}'",
                    game_name
                );
                std::process::Command::new(&ludusavi)
                    .args(["backup", "--preview", "--api", game_name])
                    .output()
            } else {
                output
            }
        }
        _ => output,
    };

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            warn!("[SAVE-SYNC] Ludusavi command failed: {e}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("No matching") {
            warn!("[SAVE-SYNC] Ludusavi error: {}", stderr);
        }
        return Vec::new();
    }

    // Parse the JSON output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            warn!("[SAVE-SYNC] Failed to parse Ludusavi output: {e}");
            return Vec::new();
        }
    };

    let mut files = Vec::new();

    if let Some(games) = json.get("games").and_then(|g| g.as_object()) {
        for (_name, game_data) in games {
            if let Some(game_files) = game_data.get("files").and_then(|f| f.as_object()) {
                for (file_path, file_data) in game_files {
                    let path = PathBuf::from(file_path);
                    if !path.is_file() {
                        continue;
                    }
                    let size = file_data.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                    let hash = match md5_file(&path) {
                        Ok(h) => h,
                        Err(e) => {
                            warn!(
                                "[SAVE-SYNC] Failed to hash PC save {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };
                    let modified_at = fs::metadata(&path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    // Use a "pc/" prefix so filenames don't collide with emu saves
                    let filename = format!(
                        "pc/{}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );

                    files.push(LocalSaveFile {
                        filename,
                        save_type: "pc".to_string(),
                        path,
                        data_hash: hash,
                        size,
                        modified_at,
                    });
                }
            }
        }
    }

    info!("[SAVE-SYNC] Ludusavi found {} PC save files", files.len());
    files
}

/// Write a downloaded PC save file back to its original location.
/// PC save filenames use a "pc/" prefix — strip it and restore to the original
/// path from the manifest, or use a fallback location.
pub fn write_downloaded_pc_save(
    filename: &str,
    data: &[u8],
    original_path: Option<&Path>,
) -> Result<PathBuf, String> {
    // If we know the original path (from manifest), use it
    if let Some(orig) = original_path {
        if let Some(parent) = orig.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir for PC save: {e}"))?;
        }
        // Backup existing
        if orig.exists() {
            let bak = orig.with_extension(format!(
                "{}.bak",
                orig.extension().unwrap_or_default().to_string_lossy()
            ));
            let _ = fs::copy(orig, &bak);
        }
        fs::write(orig, data).map_err(|e| format!("Failed to write PC save: {e}"))?;
        return Ok(orig.to_path_buf());
    }

    // Fallback: save to data_dir/drop/pc-saves/<filename>
    let clean_name = filename.strip_prefix("pc/").unwrap_or(filename);
    let fallback = dirs::data_dir()
        .ok_or("No data directory")?
        .join("drop")
        .join("pc-saves")
        .join(clean_name);
    if let Some(parent) = fallback.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create fallback dir: {e}"))?;
    }
    fs::write(&fallback, data).map_err(|e| format!("Failed to write PC save fallback: {e}"))?;
    Ok(fallback)
}
