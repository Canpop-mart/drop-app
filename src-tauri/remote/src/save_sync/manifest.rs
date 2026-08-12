//! The on-disk per-game save-sync manifest: load, persist, repair.
//!
//! A manifest records, for each save file, the MD5 it had at the last
//! successful sync — that's how the post-exit pass knows which files changed.
//! It is plain metadata, so a corrupt or absurdly large file is treated as
//! "no manifest" (backed up, then regenerated) rather than a hard error.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use log::warn;

use super::{now_iso, LocalSaveFile, SyncCheckResponse, SyncFileEntry, SyncManifest};

/// Maximum size of a sync manifest on disk. Manifests are metadata (hashes,
/// timestamps, paths) so even libraries with thousands of save files should
/// stay well under 64 MiB. Anything larger is corruption or tampering.
const MANIFEST_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// The directory, under the data root, holding every user's manifests.
pub const MANIFEST_DIR: &str = "sync-manifests";

/// Get the manifest path for one user's copy of a game's sync state.
///
/// Scoped by user id because the server keys every cloud row by user: two Drop
/// accounts on one PC sharing a manifest meant each one's sync state described
/// the other's cloud library.
///
/// Built on `DATA_ROOT_DIR` rather than a hardcoded `"drop"` — a debug build's
/// data root is `drop-debug`, so the old path wrote dev manifests into the
/// release install's directory and then read them back as if they were its own.
pub fn manifest_path(user_id: &str, game_id: &str) -> Option<PathBuf> {
    if user_id.is_empty() {
        return None;
    }
    Some(
        database::db::DATA_ROOT_DIR
            .join(MANIFEST_DIR)
            .join(user_id)
            .join(format!("{game_id}.json")),
    )
}

/// Load a sync manifest from disk, or return a fresh empty one. A manifest
/// that is corrupt or oversized is moved aside (see [`backup_corrupt_manifest`])
/// and a clean one returned — sync should never hard-fail on a bad manifest.
///
/// A manifest whose `user_id` names a *different* account is discarded rather
/// than adopted: the only way one gets here is a hand-copied file or a botched
/// migration, and inheriting it would tell this account that another person's
/// saves are already backed up under their id. A blank `user_id` is the
/// pre-scoping shape and IS adopted — the migration moving it into this user's
/// directory is what decided whose it is.
pub fn load_manifest(user_id: &str, game_id: &str) -> SyncManifest {
    if let Some(path) = manifest_path(user_id, game_id)
        && path.exists() {
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
                        Ok(mut m) if manifest_belongs_to(&m, user_id) => {
                            m.user_id = user_id.to_string();
                            return m;
                        }
                        Ok(m) => warn!(
                            "[SAVE-SYNC] Manifest for {} in {}'s directory claims user {}; \
                             ignoring it rather than adopting another account's sync state",
                            game_id, user_id, m.user_id
                        ),
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
    SyncManifest {
        user_id: user_id.to_string(),
        game_id: game_id.to_string(),
        ..Default::default()
    }
}

/// Whether `manifest` may be used as `user_id`'s sync state.
///
/// Blank means "written before per-user scoping existed"; the migration put it
/// in this user's directory, so it is theirs. Anything else must match exactly.
pub(crate) fn manifest_belongs_to(manifest: &SyncManifest, user_id: &str) -> bool {
    manifest.user_id.is_empty() || manifest.user_id == user_id
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
    let path = manifest_path(&manifest.user_id, &manifest.game_id).ok_or_else(|| {
        "Refusing to write a sync manifest with no user id — it would not belong to anyone"
            .to_string()
    })?;
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

/// Record `files` as synced, skipping any whose upload failed.
///
/// This is the ONLY place a [`SyncFileEntry`] is written. Every call site used
/// to hand-roll the same loop, and they drifted: some inserted an entry for
/// every file the scan saw whether or not it reached the cloud, and all of
/// them hard-coded `cloud_id: None`. A file marked synced that was never
/// uploaded hash-matches next session, is never seen as changed, and the
/// user's save silently never reaches the cloud — which is how a manifest can
/// claim five files synced at a timestamp whose log line reads "No saves
/// changed during session".
///
/// `cloud_ids` maps filename → cloud row id for files this round actually
/// pushed or resolved. A file with no fresh id keeps whatever id the manifest
/// already had, so an unchanged file does not lose its handle on the row.
///
/// `unsynced` names the files known NOT to have reached the cloud this round —
/// upload rejections, and anything a failed download left unmirrored.
///
/// Returns the number of entries written.
pub fn record_synced_files(
    manifest: &mut SyncManifest,
    files: &[LocalSaveFile],
    cloud_ids: &HashMap<String, String>,
    unsynced: &[String],
) -> usize {
    let now = now_iso();
    let failed: HashSet<&str> = unsynced.iter().map(String::as_str).collect();
    let mut written = 0usize;

    for file in files {
        if failed.contains(file.filename.as_str()) {
            warn!(
                "[SAVE-SYNC] Not recording {} as synced — its upload failed; \
                 it will be retried next session",
                file.filename
            );
            continue;
        }
        let cloud_id = cloud_ids.get(&file.filename).cloned().or_else(|| {
            manifest
                .files
                .get(&file.filename)
                .and_then(|e| e.cloud_id.clone())
        });
        manifest.files.insert(
            file.filename.clone(),
            SyncFileEntry {
                save_type: file.save_type.clone(),
                synced_hash: file.data_hash.clone(),
                cloud_id,
                synced_at: now.clone(),
            },
        );
        written += 1;
    }

    manifest.last_synced_at = Some(now);
    written
}

/// Update the manifest after a sync round — record the current hash of every
/// local file plus any cloud-only saves that were downloaded.
///
/// `unsynced` names files that did NOT reach (or come from) the cloud this
/// round, so they are left out rather than stamped with a hash they never
/// agreed on.
pub fn update_manifest_after_sync(
    manifest: &mut SyncManifest,
    local_files: &[LocalSaveFile],
    sync_response: &SyncCheckResponse,
    unsynced: &[String],
) {
    let cloud_ids: HashMap<String, String> = sync_response
        .actions
        .iter()
        .filter_map(|a| {
            a.cloud_save
                .as_ref()
                .map(|c| (a.filename.clone(), c.id.clone()))
        })
        .collect();
    record_synced_files(manifest, local_files, &cloud_ids, unsynced);

    // Add cloud-only saves that were downloaded
    let now = now_iso();
    let skip: HashSet<&str> = unsynced.iter().map(String::as_str).collect();
    for cloud in sync_response
        .cloud_only
        .iter()
        .filter(|c| !skip.contains(c.filename.as_str()))
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file(name: &str, hash: &str) -> LocalSaveFile {
        LocalSaveFile {
            filename: name.to_string(),
            save_type: "pc".to_string(),
            path: PathBuf::from(name),
            data_hash: hash.to_string(),
            size: 1,
            modified_at: 0,
        }
    }

    /// The headline manifest bug: a file the server rejected was still written
    /// as synced, so its hash matched next session, it never looked changed,
    /// and the save never reached the cloud.
    #[test]
    fn a_failed_upload_is_never_recorded_as_synced() {
        let mut manifest = SyncManifest::default();
        let files = [file("a.sav", "aaa"), file("b.sav", "bbb")];
        let written = record_synced_files(
            &mut manifest,
            &files,
            &HashMap::new(),
            &["b.sav".to_string()],
        );

        assert_eq!(written, 1);
        assert!(manifest.files.contains_key("a.sav"));
        assert!(
            !manifest.files.contains_key("b.sav"),
            "a rejected upload was recorded as synced"
        );
    }

    #[test]
    fn cloud_ids_from_the_upload_response_are_stored() {
        let mut manifest = SyncManifest::default();
        let ids = HashMap::from([("a.sav".to_string(), "cloud-1".to_string())]);
        record_synced_files(&mut manifest, &[file("a.sav", "aaa")], &ids, &[]);
        assert_eq!(
            manifest.files["a.sav"].cloud_id.as_deref(),
            Some("cloud-1")
        );
    }

    /// An unchanged file is not in the upload response, so it has no fresh id.
    /// It must keep the one it already had rather than being reset to null.
    #[test]
    fn an_unchanged_file_keeps_the_cloud_id_it_already_had() {
        let mut manifest = SyncManifest::default();
        let ids = HashMap::from([("a.sav".to_string(), "cloud-1".to_string())]);
        record_synced_files(&mut manifest, &[file("a.sav", "aaa")], &ids, &[]);
        record_synced_files(&mut manifest, &[file("a.sav", "aaa")], &HashMap::new(), &[]);
        assert_eq!(
            manifest.files["a.sav"].cloud_id.as_deref(),
            Some("cloud-1")
        );
    }

    /// A unique-per-run id so these tests can use the real data root without
    /// touching (or racing) the manifests actually on this machine.
    fn test_user(tag: &str) -> String {
        format!("test-user-{tag}-{}", std::process::id())
    }

    fn cleanup(user_id: &str) {
        let _ = fs::remove_dir_all(
            database::db::DATA_ROOT_DIR.join(MANIFEST_DIR).join(user_id),
        );
    }

    /// The manifest path must come from the same resolver the database uses.
    /// It was hardcoded to `"drop"`, but a debug build's data root is
    /// `drop-debug` — so a dev build wrote its manifests into the release
    /// install's directory and then read them back as if they were its own.
    #[test]
    fn the_manifest_path_follows_the_databases_data_root() {
        let expected_dir = if cfg!(debug_assertions) {
            "drop-debug"
        } else {
            "drop"
        };
        assert_eq!(
            database::db::DATA_ROOT_DIR
                .file_name()
                .and_then(|n| n.to_str()),
            Some(expected_dir)
        );

        let path = manifest_path("user-a", "g1").unwrap();
        assert!(
            path.starts_with(database::db::DATA_ROOT_DIR.as_path()),
            "{}",
            path.display()
        );
        assert!(path.ends_with(Path::new("sync-manifests/user-a/g1.json")), "{}", path.display());

        // No identity, no path — a manifest that belongs to nobody must not
        // be writable at all.
        assert!(manifest_path("", "g1").is_none());
    }

    /// A manifest naming a different account is ignored, not adopted: it
    /// describes someone else's cloud rows, and inheriting it would tell this
    /// account their saves are already backed up when they are not.
    #[test]
    fn a_manifest_belonging_to_another_account_is_discarded() {
        let user = test_user("mismatch");
        let path = manifest_path(&user, "g1").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"userId":"someone-else","gameId":"g1","files":{"gen.srm":{
                "saveType":"save","syncedHash":"abc","cloudId":"cloud-1",
                "syncedAt":"2026-06-10T17:00:24Z"}}}"#,
        )
        .unwrap();

        let loaded = load_manifest(&user, "g1");
        assert!(loaded.files.is_empty(), "adopted another account's sync state");
        assert_eq!(loaded.user_id, user);
        cleanup(&user);
    }

    /// The 18 manifests already on the user's disk have no `userId` at all.
    /// The migration moving one into this user's directory is what decided
    /// whose it is, so a blank id is adopted and stamped rather than thrown
    /// away — throwing it away would re-upload every save as new.
    #[test]
    fn a_pre_scoping_manifest_is_adopted_by_the_directory_it_sits_in() {
        let user = test_user("adopt");
        let path = manifest_path(&user, "g1").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"gameId":"g1","files":{"gen.srm":{"saveType":"save",
                "syncedHash":"abc","cloudId":"cloud-1",
                "syncedAt":"2026-06-10T17:00:24Z"}}}"#,
        )
        .unwrap();

        let loaded = load_manifest(&user, "g1");
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files["gen.srm"].synced_hash, "abc");
        assert_eq!(loaded.user_id, user, "the adopted manifest was not stamped");

        // And it round-trips back to disk under the id it was adopted into.
        save_manifest(&loaded).unwrap();
        assert_eq!(load_manifest(&user, "g1").user_id, user);
        cleanup(&user);
    }

    #[test]
    fn a_manifest_with_no_user_id_is_never_written() {
        let manifest = SyncManifest {
            game_id: "g1".to_string(),
            ..Default::default()
        };
        assert!(save_manifest(&manifest).is_err());
    }

    /// Manifests on disk predate `appliedTombstones` and are keyed by the old
    /// flat filenames. Loading one must neither panic nor drop entries.
    #[test]
    fn an_old_shape_manifest_still_loads() {
        let json = r#"{
            "gameId": "g1",
            "lastSyncedAt": "2026-06-10T17:00:24Z",
            "files": {
                "gen.srm": {
                    "saveType": "save",
                    "syncedHash": "abc123",
                    "cloudId": null,
                    "syncedAt": "2026-06-10T17:00:24Z"
                },
                "pc__slot.sav": {
                    "saveType": "pc",
                    "syncedHash": "def456",
                    "cloudId": "cloud-9",
                    "syncedAt": "2026-06-10T17:00:24Z"
                }
            }
        }"#;
        let manifest: SyncManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.game_id, "g1");
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(manifest.files["gen.srm"].synced_hash, "abc123");
        assert_eq!(
            manifest.files["pc__slot.sav"].cloud_id.as_deref(),
            Some("cloud-9")
        );
        assert!(manifest.applied_tombstones.is_empty());

        // And it round-trips back out with the new field present.
        let round_tripped: SyncManifest =
            serde_json::from_str(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert_eq!(round_tripped.files.len(), 2);
    }
}
