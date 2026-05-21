//! The on-disk per-game save-sync manifest: load, persist, repair.
//!
//! A manifest records, for each save file, the MD5 it had at the last
//! successful sync — that's how the post-exit pass knows which files changed.
//! It is plain metadata, so a corrupt or absurdly large file is treated as
//! "no manifest" (backed up, then regenerated) rather than a hard error.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use log::warn;

use super::{now_iso, LocalSaveFile, SyncCheckResponse, SyncFileEntry, SyncManifest};

/// Maximum size of a sync manifest on disk. Manifests are metadata (hashes,
/// timestamps, paths) so even libraries with thousands of save files should
/// stay well under 64 MiB. Anything larger is corruption or tampering.
const MANIFEST_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Get the manifest path for a game.
pub fn manifest_path(game_id: &str) -> Option<PathBuf> {
    dirs::data_dir()
        .map(|d| d.join("drop").join("sync-manifests").join(format!("{}.json", game_id)))
}

/// Load a sync manifest from disk, or return a fresh empty one. A manifest
/// that is corrupt or oversized is moved aside (see [`backup_corrupt_manifest`])
/// and a clean one returned — sync should never hard-fail on a bad manifest.
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
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create manifest dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize manifest: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &json).map_err(|e| format!("Failed to write manifest tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Failed to rename manifest: {e}"))?;
    Ok(())
}

/// Update the manifest after a successful sync round — record the current hash
/// of every local file plus any cloud-only saves that were downloaded.
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
