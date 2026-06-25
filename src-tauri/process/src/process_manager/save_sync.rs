//! Pre-launch cloud-save synchronisation.
//!
//! Before a game starts, Drop pulls down any cloud saves newer than the
//! local copies and, if both sides changed, asks the user to resolve the
//! conflict. After the game exits, [`super::exit`] uploads whatever changed.
//!
//! Two discovery strategies exist:
//!
//!   - **Emulator games** ([`sync_emulator_saves`]) — saves live under the
//!     emulator install dir, found by [`remote::save_sync::scan_emu_saves`].
//!   - **PC/native games** ([`sync_pc_saves`]) — saves are scattered across
//!     the OS, discovered via Ludusavi keyed on the game's display name.
//!
//! Both produce a [`SaveSyncSnapshot`] of post-download file hashes; the
//! exit path diffs against it to decide what to upload. This module is split
//! out of the launch flow purely for size — the conflict-resolution dance
//! (emit event → block on an mpsc channel → apply choices) is long and was
//! drowning the actual process-spawn logic in `launch.rs`.

use std::{collections::HashMap, path::PathBuf};

use log::{info, warn};
use tauri::{AppHandle, Emitter as _};

use crate::process_manager::SaveSyncSnapshot;

/// How long a blocking save-sync network call may run before we give up and
/// launch the game anyway. The PROCESS_MANAGER lock is held across these, so
/// a flaky connection must not be able to freeze the whole app.
const SYNC_NET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const UPLOAD_NET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// How long we wait for the user to resolve a save conflict in the UI.
const CONFLICT_RESOLVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Run a future to completion on the Tauri runtime, bounded by `timeout`.
/// On timeout the result is a `FailedDownload` error so callers fall back to
/// "launch without sync" rather than hanging.
fn block_with_timeout<F, T>(timeout: std::time::Duration, fut: F) -> Result<T, ()>
where
    F: std::future::Future<Output = T>,
{
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(timeout, fut).await.map_err(|_| ())
    })
}

/// Resolve a set of save conflicts, either by auto-picking `keep_local`
/// (streaming — the dialog would surface on the unattended host) or by
/// emitting a UI event and blocking on the resolution channel.
fn resolve_conflicts(
    app: &AppHandle,
    game_id: &str,
    conflicts: &[remote::save_sync::SaveConflict],
    streaming: bool,
) -> Vec<remote::save_sync::ConflictResolution> {
    let keep_all_local = || {
        conflicts
            .iter()
            .map(|c| remote::save_sync::ConflictResolution {
                filename: c.filename.clone(),
                choice: "keep_local".to_string(),
            })
            .collect::<Vec<_>>()
    };

    if streaming {
        info!(
            "[SAVE-SYNC] Streaming mode — auto-resolving {} conflicts to keep_local",
            conflicts.len()
        );
        return keep_all_local();
    }

    // Emit the conflict event so the UI can show its dialog.
    let conflict_event = remote::save_sync::SaveConflictEvent {
        game_id: game_id.to_string(),
        conflicts: conflicts.to_vec(),
    };
    let _ = app.emit(
        &format!("save_sync_conflict/{game_id}"),
        serde_json::to_value(&conflict_event).unwrap_or_default(),
    );

    // Register a resolution channel and block until the frontend answers
    // (or the timeout fires).
    let (tx, rx) = std::sync::mpsc::channel();
    crate::CONFLICT_CHANNELS
        .lock()
        .insert(game_id.to_string(), tx);
    info!("[SAVE-SYNC] Waiting for conflict resolution from UI...");

    let resolved = match rx.recv_timeout(CONFLICT_RESOLVE_TIMEOUT) {
        Ok(res) => res,
        Err(_) => {
            warn!("[SAVE-SYNC] Conflict resolution timed out, defaulting to keep_local");
            keep_all_local()
        }
    };
    crate::CONFLICT_CHANNELS.lock().remove(game_id);
    resolved
}

/// Upload the local copies of files the user chose to keep on a conflict.
fn upload_kept_local(
    game_id: &str,
    local_saves: &[remote::save_sync::LocalSaveFile],
    keep_filenames: &[String],
) {
    let files: Vec<remote::save_sync::LocalSaveFile> = local_saves
        .iter()
        .filter(|f| keep_filenames.contains(&f.filename))
        .cloned()
        .collect();
    if files.is_empty() {
        return;
    }
    // Empty pre-hashes => everything is treated as changed and uploaded.
    let empty = HashMap::new();
    match block_with_timeout(
        UPLOAD_NET_TIMEOUT,
        remote::save_sync::upload_changed_saves(game_id, &empty, &files),
    ) {
        Ok(Ok((count, errs))) => {
            info!("[SAVE-SYNC] Conflict: uploaded {count} local saves");
            for err in &errs {
                warn!("[SAVE-SYNC] Conflict upload error: {err}");
            }
        }
        Ok(Err(e)) => warn!("[SAVE-SYNC] Conflict upload failed: {e}"),
        Err(()) => warn!("[SAVE-SYNC] Conflict upload timed out"),
    }
}

/// Collect every cloud save id that needs downloading: cloud-only files,
/// explicit `download` actions, plus any extras from conflict resolution.
fn collect_download_ids(
    sync_result: &remote::save_sync::SyncCheckResponse,
    extra: Vec<String>,
) -> Vec<String> {
    let mut ids: Vec<String> = sync_result
        .cloud_only
        .iter()
        .map(|s| s.id.clone())
        .collect();
    for action in &sync_result.actions {
        if action.action == "download"
            && let Some(cloud) = &action.cloud_save {
                ids.push(cloud.id.clone());
            }
    }
    ids.extend(extra);
    ids
}

/// Pre-launch sync for an **emulator** game. `emu_dir` is the emulator's
/// install directory. Returns a snapshot of post-download save hashes for
/// the exit path to diff against, or `None` if there are no saves at all.
pub fn sync_emulator_saves(
    app: &AppHandle,
    game_id: &str,
    emu_dir: &str,
    streaming: bool,
) -> Option<SaveSyncSnapshot> {
    let emu_path = std::path::Path::new(emu_dir);
    let local_saves = remote::save_sync::scan_emu_saves(emu_path, game_id);
    let pre_hashes = remote::save_sync::snapshot_hashes(&local_saves);

    // A failed/timed-out sync still yields a snapshot of local hashes so the
    // exit path can upload anything that changed during the session.
    let local_only_snapshot = || SaveSyncSnapshot {
        emu_root: Some(emu_path.to_path_buf()),
        game_id: game_id.to_string(),
        game_name: None,
        pre_hashes: pre_hashes.clone(),
        pc_save_paths: HashMap::new(),
        wine_prefix: None,
    };

    let sync_result = match block_with_timeout(
        SYNC_NET_TIMEOUT,
        remote::save_sync::check_sync(game_id, &local_saves),
    ) {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            warn!("[SAVE-SYNC] Sync check failed (continuing without sync): {e}");
            return Some(local_only_snapshot());
        }
        Err(()) => {
            warn!("[SAVE-SYNC] Sync check timed out (continuing without sync)");
            return Some(local_only_snapshot());
        }
    };

    let extra_downloads = handle_conflicts_and_collect(
        app,
        game_id,
        &sync_result,
        &local_saves,
        streaming,
    );
    let download_ids = collect_download_ids(&sync_result, extra_downloads);

    if !download_ids.is_empty() {
        info!(
            "[SAVE-SYNC] Downloading {} cloud saves for game {game_id}",
            download_ids.len()
        );
        match block_with_timeout(
            UPLOAD_NET_TIMEOUT,
            remote::save_sync::bulk_download(&download_ids),
        ) {
            Ok(Ok(downloaded)) => {
                for (filename, save_type, _hash, data) in &downloaded {
                    match remote::save_sync::write_downloaded_save(
                        emu_path, game_id, filename, save_type, data,
                    ) {
                        Ok(path) => info!("[SAVE-SYNC] Downloaded save: {}", path.display()),
                        Err(e) => warn!("[SAVE-SYNC] Failed to write save {filename}: {e}"),
                    }
                }
            }
            Ok(Err(e)) => warn!("[SAVE-SYNC] Bulk download failed: {e}"),
            Err(()) => warn!("[SAVE-SYNC] Bulk download timed out"),
        }
    }

    // Apply server tombstones: saves the user deleted from another device
    // get removed locally (with a `.bak` backup). Runs AFTER downloads so a
    // race where the same filename is on both lists still ends up deleted.
    apply_emu_tombstones(emu_path, game_id, &sync_result.tombstones);

    // Re-scan post-download and persist the manifest.
    let updated = remote::save_sync::scan_emu_saves(emu_path, game_id);
    let mut manifest = remote::save_sync::load_manifest(game_id);
    remote::save_sync::update_manifest_after_sync(&mut manifest, &updated, &sync_result);
    if let Err(e) = remote::save_sync::save_manifest(&manifest) {
        warn!("[SAVE-SYNC] Failed to save manifest: {e}");
    }

    Some(SaveSyncSnapshot {
        emu_root: Some(emu_path.to_path_buf()),
        game_id: game_id.to_string(),
        game_name: None,
        pre_hashes: remote::save_sync::snapshot_hashes(&updated),
        pc_save_paths: HashMap::new(),
        wine_prefix: None,
    })
}

/// Pre-launch sync for a **PC/native** game discovered via Ludusavi.
/// `game_name` is the display name Ludusavi keys on.
/// `wine_prefix`, when present, is forwarded to Ludusavi via `--wine-prefix`
/// so saves under Drop's per-game prefix are visible (Linux host launching
/// a Windows-target game). Returns `None` if the game has no discoverable
/// saves.
pub fn sync_pc_saves(
    app: &AppHandle,
    game_id: &str,
    game_name: &str,
    wine_prefix: Option<PathBuf>,
    streaming: bool,
) -> Option<SaveSyncSnapshot> {
    let pc_saves = remote::save_sync::scan_pc_saves(
        game_name,
        None,
        wine_prefix.as_deref(),
    );
    if pc_saves.is_empty() {
        return None;
    }

    let pre_hashes = remote::save_sync::snapshot_hashes(&pc_saves);
    let pc_paths: HashMap<String, PathBuf> = pc_saves
        .iter()
        .map(|f| (f.filename.clone(), f.path.clone()))
        .collect();

    let local_only_snapshot = || SaveSyncSnapshot {
        emu_root: None,
        game_id: game_id.to_string(),
        game_name: Some(game_name.to_string()),
        pre_hashes: pre_hashes.clone(),
        pc_save_paths: pc_paths.clone(),
        wine_prefix: wine_prefix.clone(),
    };

    let sync_result = match block_with_timeout(
        SYNC_NET_TIMEOUT,
        remote::save_sync::check_sync(game_id, &pc_saves),
    ) {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            warn!("[SAVE-SYNC] PC sync check failed: {e}");
            return Some(local_only_snapshot());
        }
        Err(()) => {
            warn!("[SAVE-SYNC] PC sync check timed out");
            return Some(local_only_snapshot());
        }
    };

    let extra_downloads =
        handle_conflicts_and_collect(app, game_id, &sync_result, &pc_saves, streaming);
    let download_ids = collect_download_ids(&sync_result, extra_downloads);

    if !download_ids.is_empty() {
        match block_with_timeout(
            UPLOAD_NET_TIMEOUT,
            remote::save_sync::bulk_download(&download_ids),
        ) {
            Ok(Ok(downloaded)) => {
                // The game's PC saves all live in one directory. Derive it
                // from the pre-launch scan so a cloud-only save (one with no
                // local copy yet) lands in that real folder instead of the
                // dead-end fallback dir — otherwise the game never reads it.
                let save_dir: Option<PathBuf> = pc_paths
                    .values()
                    .next()
                    .and_then(|p| p.parent())
                    .map(|parent| parent.to_path_buf());
                for (filename, _save_type, _hash, data) in &downloaded {
                    let dest: Option<PathBuf> = match pc_paths.get(filename.as_str()) {
                        Some(p) => Some(p.clone()),
                        None => save_dir.as_ref().map(|dir| {
                            dir.join(remote::save_sync::scan::strip_pc_prefix(filename))
                        }),
                    };
                    match remote::save_sync::write_downloaded_pc_save(
                        filename,
                        data,
                        dest.as_deref(),
                    ) {
                        Ok(p) => info!("[SAVE-SYNC] Downloaded PC save: {}", p.display()),
                        Err(e) => warn!("[SAVE-SYNC] Failed to write PC save {filename}: {e}"),
                    }
                }
            }
            Ok(Err(e)) => warn!("[SAVE-SYNC] PC bulk download failed: {e}"),
            Err(()) => warn!("[SAVE-SYNC] PC bulk download timed out"),
        }
    }

    // Apply server tombstones for PC saves. Resolve the local path via the
    // pre-launch scan map; if the filename isn't known locally, there's
    // nothing to delete and we just log.
    apply_pc_tombstones(&pc_paths, &sync_result.tombstones);

    let updated = remote::save_sync::scan_pc_saves(
        game_name,
        None,
        wine_prefix.as_deref(),
    );
    let mut manifest = remote::save_sync::load_manifest(game_id);
    remote::save_sync::update_manifest_after_sync(&mut manifest, &updated, &sync_result);
    let _ = remote::save_sync::save_manifest(&manifest);

    Some(SaveSyncSnapshot {
        emu_root: None,
        game_id: game_id.to_string(),
        game_name: Some(game_name.to_string()),
        pre_hashes: remote::save_sync::snapshot_hashes(&updated),
        pc_save_paths: updated
            .iter()
            .map(|f| (f.filename.clone(), f.path.clone()))
            .collect(),
        wine_prefix,
    })
}

/// Apply server-issued tombstones for an emulator game. For each tombstone
/// we back up the local file to `<file>.<ext>.bak` and unlink it. Errors are
/// logged but don't abort the sync — best-effort, the next sync-check will
/// surface the same tombstones if anything went wrong.
fn apply_emu_tombstones(
    emu_path: &std::path::Path,
    game_id: &str,
    tombstones: &[remote::save_sync::Tombstone],
) {
    if tombstones.is_empty() {
        return;
    }
    info!(
        "[SAVE-SYNC] Applying {} tombstones for emulator game {game_id}",
        tombstones.len()
    );
    for t in tombstones {
        match remote::save_sync::delete_local_emu_save_for_tombstone(
            emu_path,
            game_id,
            &t.filename,
        ) {
            Ok(Some(path)) => info!(
                "[SAVE-SYNC] Tombstone: deleted local save {} (deleted from '{}' at {})",
                path.display(),
                t.deleted_from,
                t.deleted_at
            ),
            Ok(None) => info!(
                "[SAVE-SYNC] Tombstone: no local copy of {} to delete",
                t.filename
            ),
            Err(e) => warn!(
                "[SAVE-SYNC] Tombstone: failed to delete {}: {e}",
                t.filename
            ),
        }
    }
}

/// Apply server tombstones for PC saves. Resolves each filename via the
/// pre-scan path map; filenames the local scan didn't see are logged and
/// skipped (the user may have already deleted them on this machine).
fn apply_pc_tombstones(
    pc_paths: &HashMap<String, PathBuf>,
    tombstones: &[remote::save_sync::Tombstone],
) {
    if tombstones.is_empty() {
        return;
    }
    info!(
        "[SAVE-SYNC] Applying {} PC tombstones",
        tombstones.len()
    );
    for t in tombstones {
        let Some(orig) = pc_paths.get(&t.filename) else {
            info!(
                "[SAVE-SYNC] PC tombstone: no local copy of {} (deleted from '{}'), skipping",
                t.filename, t.deleted_from
            );
            continue;
        };
        match remote::save_sync::delete_local_pc_save_for_tombstone(orig) {
            Ok(true) => info!(
                "[SAVE-SYNC] PC tombstone: deleted {} (deleted from '{}' at {})",
                orig.display(),
                t.deleted_from,
                t.deleted_at
            ),
            Ok(false) => info!(
                "[SAVE-SYNC] PC tombstone: {} already gone",
                orig.display()
            ),
            Err(e) => warn!(
                "[SAVE-SYNC] PC tombstone: failed to delete {}: {e}",
                orig.display()
            ),
        }
    }
}

/// Shared conflict path for both emulator and PC syncs: extract conflicts,
/// resolve them (UI or auto), apply the resolutions (uploading kept-local
/// files), and return the list of extra cloud ids the user chose to pull.
fn handle_conflicts_and_collect(
    app: &AppHandle,
    game_id: &str,
    sync_result: &remote::save_sync::SyncCheckResponse,
    local_saves: &[remote::save_sync::LocalSaveFile],
    streaming: bool,
) -> Vec<String> {
    let conflicts = remote::save_sync::extract_conflicts(sync_result, local_saves);
    if conflicts.is_empty() {
        return Vec::new();
    }
    info!(
        "[SAVE-SYNC] {} conflicts detected for game {game_id}",
        conflicts.len()
    );

    let resolutions = resolve_conflicts(app, game_id, &conflicts, streaming);
    let (extra_downloads, extra_uploads) =
        remote::save_sync::apply_conflict_resolutions(&conflicts, &resolutions);

    if !extra_uploads.is_empty() {
        upload_kept_local(game_id, local_saves, &extra_uploads);
    }
    info!(
        "[SAVE-SYNC] Conflict resolution applied: {} resolutions",
        resolutions.len()
    );
    extra_downloads
}
