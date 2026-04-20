//! Cloud save synchronisation — automatic pre-launch download and post-exit upload.
//!
//! # Flow
//!
//! **Pre-launch** (`pre_launch_sync`):
//!   1. Scan local save files (RetroArch `drop-saves` + Ludusavi PC saves)
//!   2. Compute MD5 of each file
//!   3. POST to `/api/v1/client/saves/sync-check` with local state
//!   4. Server compares hashes and returns verdicts: download / upload / conflict / synced
//!   5. If conflicts: emit a Tauri event and **block** until the UI resolves them
//!   6. Download cloud saves that are newer
//!   7. Update local sync manifest
//!
//! **Post-exit** (`post_exit_sync`):
//!   1. Re-scan local saves, compare MD5 against pre-launch snapshot
//!   2. Upload any files that changed during the session
//!   3. Update manifest — non-blocking, runs in background

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{info, warn};
use md5;
use serde::{Deserialize, Serialize};

use crate::error::RemoteAccessError;
use crate::requests::{generate_url, make_authenticated_post};
use crate::utils::{bounded_json, DEFAULT_JSON_CAP_BYTES};

/// Cloud save sync responses can include large binary blobs (base64). Allow
/// up to 512 MiB to cover archives built from many large PC save files.
const SAVE_SYNC_RESPONSE_CAP: u64 = 512 * 1024 * 1024;

// ── Manifest (persisted to disk between sessions) ──────────────────────

/// Per-game sync manifest stored at `{data_dir}/drop/sync-manifests/{game_id}.json`.
/// Tracks which files were last synced and their hashes at sync time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncManifest {
    pub game_id: String,
    pub last_synced_at: Option<String>,
    /// Map of filename → per-file sync state
    pub files: HashMap<String, SyncFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFileEntry {
    pub save_type: String,
    /// MD5 hash of the file at last successful sync
    pub synced_hash: String,
    /// Cloud save ID (for download references)
    pub cloud_id: Option<String>,
    /// Timestamp of last successful sync (ISO 8601)
    pub synced_at: String,
}

// ── Local file snapshot ────────────────────────────────────────────────

/// A snapshot of a local save file — path, hash, and metadata.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSaveFile {
    /// Filename used as the key (e.g. "Game Name.srm" or "pc/save0.dat")
    pub filename: String,
    pub save_type: String,
    /// Full path on disk (needed for reading/writing)
    pub path: PathBuf,
    pub data_hash: String,
    pub size: u64,
    pub modified_at: u64, // unix timestamp
}

// ── Server request / response types ────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncCheckBody {
    game_id: String,
    local_saves: Vec<SyncCheckLocalEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncCheckLocalEntry {
    filename: String,
    save_type: String,
    data_hash: String,
    client_modified_at: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncCheckResponse {
    pub actions: Vec<SyncAction>,
    pub cloud_only: Vec<CloudSaveMeta>,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAction {
    pub filename: String,
    pub action: String, // "download" | "upload" | "conflict" | "synced"
    pub cloud_save: Option<CloudSaveMeta>,
    pub local_hash: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSaveMeta {
    pub id: String,
    pub filename: String,
    pub save_type: String,
    pub data_hash: String,
    pub size: i64,
    pub uploaded_from: String,
    pub client_modified_at: String,
    pub uploaded_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkDownloadBody {
    save_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkDownloadResponse {
    saves: Vec<DownloadedSave>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadedSave {
    id: String,
    filename: String,
    save_type: String,
    data_hash: String,
    data: String, // base64
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadBody {
    game_id: String,
    uploaded_from: String,
    saves: Vec<BulkUploadEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadEntry {
    filename: String,
    save_type: String,
    data: String, // base64
    client_modified_at: String,
    data_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadResponse {
    results: Vec<BulkUploadResult>,
    errors: Vec<BulkUploadError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadResult {
    filename: String,
    id: String,
    size: i64,
    data_hash: String,
    uploaded_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadError {
    filename: String,
    error: String,
}

// ── Event payloads (sent to frontend for conflict UI) ──────────────────

/// Emitted as `save_sync_conflict/{game_id}` when conflicts are detected.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConflictEvent {
    pub game_id: String,
    pub conflicts: Vec<SaveConflict>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConflict {
    pub filename: String,
    pub save_type: String,
    /// Local file info
    pub local_hash: String,
    pub local_size: u64,
    pub local_modified_at: u64,
    /// Cloud file info
    pub cloud_id: String,
    pub cloud_hash: String,
    pub cloud_size: i64,
    pub cloud_modified_at: String,
    pub cloud_uploaded_from: String,
}

/// The frontend sends this back after the user resolves conflicts.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolution {
    pub filename: String,
    /// "keep_local" or "keep_cloud"
    pub choice: String,
}

// ── Pre-launch sync result ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreLaunchSyncResult {
    pub downloaded: usize,
    pub conflicts_resolved: usize,
    pub pending_uploads: usize,
    pub errors: Vec<String>,
}

// ── Core functions ─────────────────────────────────────────────────────

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

/// Get the manifest path for a game.
pub fn manifest_path(game_id: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("drop").join("sync-manifests").join(format!("{}.json", game_id)))
}

/// Maximum size of a sync manifest on disk. Manifests are metadata (hashes,
/// timestamps, paths) so even libraries with thousands of save files should
/// stay well under 64 MiB. Anything larger is corruption or tampering.
const MANIFEST_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Load a sync manifest from disk, or return a fresh empty one.
pub fn load_manifest(game_id: &str) -> SyncManifest {
    if let Some(path) = manifest_path(game_id) {
        if path.exists() {
            let oversize = fs::metadata(&path)
                .map(|m| m.len() > MANIFEST_MAX_BYTES)
                .unwrap_or(false);
            if oversize {
                warn!(
                    "[SAVE-SYNC] Manifest for {} exceeds {} bytes, treating as corrupt",
                    game_id, MANIFEST_MAX_BYTES
                );
                backup_corrupt_manifest(&path);
            } else {
                match fs::read_to_string(&path) {
                    Ok(json) => match serde_json::from_str::<SyncManifest>(&json) {
                        Ok(m) => return m,
                        Err(e) => {
                            warn!(
                                "[SAVE-SYNC] Corrupt manifest for {}, resetting: {}",
                                game_id, e
                            );
                            backup_corrupt_manifest(&path);
                        }
                    },
                    Err(e) => {
                        warn!("[SAVE-SYNC] Could not read manifest for {}: {}", game_id, e)
                    }
                }
            }
        }
    }
    SyncManifest {
        game_id: game_id.to_string(),
        ..Default::default()
    }
}

/// Move a corrupt manifest aside so we don't clobber earlier backups.
fn backup_corrupt_manifest(path: &Path) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_extension(format!("json.bak.{ts}"));
    if let Err(e) = fs::rename(path, &backup) {
        warn!(
            "[SAVE-SYNC] Could not back up corrupt manifest at {}: {}",
            path.display(),
            e
        );
    }
}

/// Persist a manifest to disk atomically (write tmp + rename).
pub fn save_manifest(manifest: &SyncManifest) -> Result<(), String> {
    let path = manifest_path(&manifest.game_id)
        .ok_or_else(|| "Could not determine data directory".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create manifest dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize manifest: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &json).map_err(|e| format!("Failed to write manifest tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Failed to rename manifest: {e}"))?;
    Ok(())
}

/// Get the local machine hostname for `uploadedFrom`.
pub fn machine_name() -> String {
    gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| "unknown".into())
}

/// Get current time as ISO 8601 string.
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ── Pre-launch sync ────────────────────────────────────────────────────

/// Call the server's sync-check endpoint with local save state.
pub async fn check_sync(
    game_id: &str,
    local_saves: &[LocalSaveFile],
) -> Result<SyncCheckResponse, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/saves/sync-check"], &[])?;
    let body = SyncCheckBody {
        game_id: game_id.to_string(),
        local_saves: local_saves
            .iter()
            .map(|f| SyncCheckLocalEntry {
                filename: f.filename.clone(),
                save_type: f.save_type.clone(),
                data_hash: f.data_hash.clone(),
                client_modified_at: chrono::DateTime::from_timestamp(f.modified_at as i64, 0)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default(),
            })
            .collect(),
    };

    let resp = make_authenticated_post(url, &body).await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(RemoteAccessError::UnparseableResponse(
            format!("sync-check failed: {status} - {text}"),
        ));
    }

    let data: SyncCheckResponse = bounded_json(resp, DEFAULT_JSON_CAP_BYTES).await?;
    Ok(data)
}

/// Download cloud saves by their IDs.
pub async fn bulk_download(save_ids: &[String]) -> Result<Vec<(String, String, String, Vec<u8>)>, RemoteAccessError> {
    if save_ids.is_empty() {
        return Ok(Vec::new());
    }

    let url = generate_url(&["/api/v1/client/saves/bulk-download"], &[])?;
    let body = BulkDownloadBody {
        save_ids: save_ids.to_vec(),
    };

    let resp = make_authenticated_post(url, &body).await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(RemoteAccessError::UnparseableResponse(
            format!("bulk-download failed: {status} - {text}"),
        ));
    }

    let data: BulkDownloadResponse = bounded_json(resp, SAVE_SYNC_RESPONSE_CAP).await?;
    let mut results = Vec::new();
    for save in data.saves {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&save.data)
            .map_err(|e| RemoteAccessError::UnparseableResponse(format!("base64 decode: {e}")))?;
        results.push((save.filename, save.save_type, save.data_hash, bytes));
    }
    Ok(results)
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

    fs::write(&dest, data)
        .map_err(|e| format!("Failed to write save {}: {e}", dest.display()))?;
    Ok(dest)
}

// ── Post-exit sync ─────────────────────────────────────────────────────

/// Upload all changed saves after a game session ends.
pub async fn upload_changed_saves(
    game_id: &str,
    pre_launch_hashes: &HashMap<String, String>,
    current_files: &[LocalSaveFile],
) -> Result<(usize, Vec<String>), RemoteAccessError> {
    let mut to_upload = Vec::new();

    for file in current_files {
        let old_hash = pre_launch_hashes.get(&file.filename);
        let changed = match old_hash {
            Some(h) => h != &file.data_hash,
            None => true, // new file
        };
        if !changed {
            continue;
        }

        // Read file data
        let data = match fs::read(&file.path) {
            Ok(d) => d,
            Err(e) => {
                warn!("[SAVE-SYNC] Failed to read {} for upload: {}", file.path.display(), e);
                continue;
            }
        };

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);

        to_upload.push(BulkUploadEntry {
            filename: file.filename.clone(),
            save_type: file.save_type.clone(),
            data: b64,
            client_modified_at: chrono::DateTime::from_timestamp(file.modified_at as i64, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default(),
            data_hash: file.data_hash.clone(),
        });
    }

    if to_upload.is_empty() {
        info!("[SAVE-SYNC] No saves changed during session for game {}", game_id);
        return Ok((0, Vec::new()));
    }

    info!("[SAVE-SYNC] Uploading {} changed saves for game {}", to_upload.len(), game_id);

    let url = generate_url(&["/api/v1/client/saves/bulk-upload"], &[])?;
    let body = BulkUploadBody {
        game_id: game_id.to_string(),
        uploaded_from: machine_name(),
        saves: to_upload,
    };

    let resp = make_authenticated_post(url, &body).await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(RemoteAccessError::UnparseableResponse(
            format!("bulk-upload failed: {status} - {text}"),
        ));
    }

    let data: BulkUploadResponse = bounded_json(resp, DEFAULT_JSON_CAP_BYTES).await?;
    let uploaded_count = data.results.len();
    let errors: Vec<String> = data.errors.iter().map(|e| format!("{}: {}", e.filename, e.error)).collect();
    for err in &errors {
        warn!("[SAVE-SYNC] Upload error: {}", err);
    }

    Ok((uploaded_count, errors))
}

/// Build a hashmap of filename → MD5 from a list of local save files.
/// Used to snapshot pre-launch state for change detection on exit.
pub fn snapshot_hashes(files: &[LocalSaveFile]) -> HashMap<String, String> {
    files
        .iter()
        .map(|f| (f.filename.clone(), f.data_hash.clone()))
        .collect()
}

/// Build the list of conflicts from the sync-check response + local file info.
pub fn extract_conflicts(
    sync_response: &SyncCheckResponse,
    local_files: &[LocalSaveFile],
) -> Vec<SaveConflict> {
    let local_by_name: HashMap<&str, &LocalSaveFile> =
        local_files.iter().map(|f| (f.filename.as_str(), f)).collect();

    sync_response
        .actions
        .iter()
        .filter(|a| a.action == "conflict")
        .filter_map(|a| {
            let local = local_by_name.get(a.filename.as_str())?;
            let cloud = a.cloud_save.as_ref()?;
            Some(SaveConflict {
                filename: a.filename.clone(),
                save_type: local.save_type.clone(),
                local_hash: local.data_hash.clone(),
                local_size: local.size,
                local_modified_at: local.modified_at,
                cloud_id: cloud.id.clone(),
                cloud_hash: cloud.data_hash.clone(),
                cloud_size: cloud.size,
                cloud_modified_at: cloud.client_modified_at.clone(),
                cloud_uploaded_from: cloud.uploaded_from.clone(),
            })
        })
        .collect()
}

/// After the user resolves conflicts, apply their choices.
/// Returns the IDs of cloud saves to download (for "keep_cloud" choices).
pub fn apply_conflict_resolutions(
    conflicts: &[SaveConflict],
    resolutions: &[ConflictResolution],
) -> (Vec<String>, Vec<String>) {
    let resolution_map: HashMap<&str, &str> = resolutions
        .iter()
        .map(|r| (r.filename.as_str(), r.choice.as_str()))
        .collect();

    let mut download_ids = Vec::new(); // cloud save IDs to download
    let mut upload_filenames = Vec::new(); // local files to upload

    for conflict in conflicts {
        match resolution_map.get(conflict.filename.as_str()) {
            Some(&"keep_cloud") => {
                download_ids.push(conflict.cloud_id.clone());
            }
            Some(&"keep_local") => {
                upload_filenames.push(conflict.filename.clone());
            }
            _ => {
                // Default: keep local (safer — user doesn't lose current work)
                upload_filenames.push(conflict.filename.clone());
            }
        }
    }

    (download_ids, upload_filenames)
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
                info!("[SAVE-SYNC] Retrying Ludusavi with original name: '{}'", game_name);
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
                            warn!("[SAVE-SYNC] Failed to hash PC save {}: {}", path.display(), e);
                            continue;
                        }
                    };
                    let modified_at = fs::metadata(&path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    // Use a "pc/" prefix so filenames don't collide with emu saves
                    let filename = format!("pc/{}", path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy());

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
/// PC save filenames use a "pc/" prefix — strip it and restore to the original path
/// from the manifest or use a fallback location.
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
        fs::write(orig, data)
            .map_err(|e| format!("Failed to write PC save: {e}"))?;
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
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create fallback dir: {e}"))?;
    }
    fs::write(&fallback, data)
        .map_err(|e| format!("Failed to write PC save fallback: {e}"))?;
    Ok(fallback)
}

/// Update the manifest after a successful sync round.
pub fn update_manifest_after_sync(
    manifest: &mut SyncManifest,
    local_files: &[LocalSaveFile],
    sync_response: &SyncCheckResponse,
) {
    let now = now_iso();

    // Update entries for synced/downloaded/uploaded files
    for file in local_files {
        manifest.files.insert(
            file.filename.clone(),
            SyncFileEntry {
                save_type: file.save_type.clone(),
                synced_hash: file.data_hash.clone(),
                cloud_id: sync_response
                    .actions
                    .iter()
                    .find(|a| a.filename == file.filename)
                    .and_then(|a| a.cloud_save.as_ref())
                    .map(|c| c.id.clone()),
                synced_at: now.clone(),
            },
        );
    }

    // Add cloud-only saves that were downloaded
    for cloud in &sync_response.cloud_only {
        manifest.files.insert(
            cloud.filename.clone(),
            SyncFileEntry {
                save_type: cloud.save_type.clone(),
                synced_hash: cloud.data_hash.clone(),
                cloud_id: Some(cloud.id.clone()),
                synced_at: now.clone(),
            },
        );
    }

    manifest.last_synced_at = Some(now);
}
