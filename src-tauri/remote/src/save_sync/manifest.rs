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
                three_way_base: true,
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
/// agreed on. That includes every conflict nobody resolved: recording one
/// would make the local copy look like the agreed baseline, and the next
/// launch's three-way check (see [`super::decide`]) would then quietly
/// download the cloud copy over it.
///
/// `uploaded_ids` maps filename to cloud row id for files pushed before
/// launch. The three-way check only trusts an entry that names its cloud row,
/// so a file first uploaded here would otherwise never get one.
pub fn update_manifest_after_sync(
    manifest: &mut SyncManifest,
    local_files: &[LocalSaveFile],
    sync_response: &SyncCheckResponse,
    uploaded_ids: &HashMap<String, String>,
    unsynced: &[String],
) {
    let mut cloud_ids: HashMap<String, String> = sync_response
        .actions
        .iter()
        .filter_map(|a| {
            a.cloud_save
                .as_ref()
                .map(|c| (a.filename.clone(), c.id.clone()))
        })
        .collect();
    cloud_ids.extend(uploaded_ids.iter().map(|(k, v)| (k.clone(), v.clone())));
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
                three_way_base: true,
            },
        );
    }

    manifest.last_synced_at = Some(now);
}

/// File, inside one account's manifest directory, naming that account.
///
/// A dotfile with no `.json` extension so nothing that walks the manifest
/// directories mistakes it for a game's manifest.
const ACCOUNT_NAME_FILE: &str = ".account-name";

/// Remember `display_name` as the name of the account whose sync state lives
/// under `user_id`, so another account signed in on this PC later can be told
/// whose copy a local save is. Best-effort: losing the label only costs the
/// conflict prompt a name.
pub fn record_account_name(user_id: &str, display_name: &str) {
    let name = display_name.trim();
    if user_id.is_empty() || name.is_empty() {
        return;
    }
    let dir = database::db::DATA_ROOT_DIR.join(MANIFEST_DIR).join(user_id);
    let path = dir.join(ACCOUNT_NAME_FILE);
    if fs::read_to_string(&path).is_ok_and(|held| held.trim() == name) {
        return;
    }
    if let Err(e) = fs::create_dir_all(&dir).and_then(|()| fs::write(&path, name)) {
        warn!("[SAVE-SYNC] Could not record this account's name for its saves: {e}");
    }
}

/// Whether any other Drop account has sync state for `game_id` on this device.
///
/// When one has, a save that changed here since this account's last sync may
/// be that person's progress, and nothing can prove otherwise: their sync may
/// have failed, so their manifest need not record the bytes. Callers then
/// refuse to treat a local change as this account's own (no automatic upload)
/// and ask instead.
///
/// Fails closed: only a manifest directory that does not exist means "no
/// other account". One that exists but cannot be listed counts as "another
/// account may share these files".
///
/// An account that has not synced anything on this device for
/// [`OTHER_ACCOUNT_ACTIVE_FOR`] (a deleted account, a guest who visited once)
/// no longer counts, so the rule does not stay on forever. Every sync rewrites
/// that account's manifest for the game it launched, which is the activity
/// measured.
pub fn other_accounts_have_synced(current_user_id: &str, game_id: &str) -> bool {
    other_accounts_have_synced_in(
        &database::db::DATA_ROOT_DIR.join(MANIFEST_DIR),
        current_user_id,
        game_id,
        SystemTime::now(),
    )
}

/// How long another account's sync state on this device keeps the
/// other-account rule on after its last sync here.
pub const OTHER_ACCOUNT_ACTIVE_FOR: std::time::Duration =
    std::time::Duration::from_secs(180 * 24 * 60 * 60);

/// Whether anything in an account's manifest directory changed within
/// [`OTHER_ACCOUNT_ACTIVE_FOR`] of `now`. Unreadable counts as active.
fn account_recently_active(dir: &Path, now: SystemTime) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return true;
    };
    let cutoff = now.checked_sub(OTHER_ACCOUNT_ACTIVE_FOR).unwrap_or(UNIX_EPOCH);
    entries.into_iter().any(|entry| match entry.and_then(|e| e.metadata()) {
        Ok(meta) => meta.modified().map_or(true, |m| m >= cutoff),
        Err(_) => true,
    })
}

fn other_accounts_have_synced_in(
    root: &Path,
    current_user_id: &str,
    game_id: &str,
    now: SystemTime,
) -> bool {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return false,
        Err(e) => {
            warn!(
                "[SAVE-SYNC] Could not list {} ({e}); assuming another Drop account may share \
                 this game's saves",
                root.display()
            );
            return true;
        }
    };
    let manifest_name = format!("{game_id}.json");
    entries.into_iter().any(|entry| match entry {
        Ok(entry) => {
            entry.file_name().to_string_lossy() != current_user_id
                && entry.file_type().map_or(true, |t| t.is_dir())
                && !matches!(entry.path().join(&manifest_name).try_exists(), Ok(false))
                && account_recently_active(&entry.path(), now)
        }
        // An entry that cannot be read could be another account's.
        Err(_) => true,
    })
}

/// Which *other* Drop account on this PC last synced exactly these bytes.
///
/// Two accounts on one PC share its PC save files and Switch NAND on disk,
/// while each keeps its own manifest. If the file on disk hashes to what
/// another account's manifest recorded at its last sync, that account's
/// session is where the file came from. Returns that account's display name,
/// or an empty string when it is known to be another account but its name was
/// never recorded. `None` when no other account's manifest matches.
pub fn local_copy_last_synced_by(
    current_user_id: &str,
    game_id: &str,
    filename: &str,
    local_hash: &str,
) -> Option<String> {
    if local_hash.is_empty() {
        return None;
    }
    let root = database::db::DATA_ROOT_DIR.join(MANIFEST_DIR);
    let entries = fs::read_dir(&root).ok()?;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let other = entry.file_name().to_string_lossy().to_string();
        if other == current_user_id {
            continue;
        }
        let manifest = entry.path().join(format!("{game_id}.json"));
        if fs::metadata(&manifest).map_or(true, |m| m.len() > MANIFEST_MAX_BYTES) {
            continue;
        }
        let Ok(json) = fs::read_to_string(&manifest) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<SyncManifest>(&json) else {
            continue;
        };
        let matches = parsed
            .files
            .get(filename)
            .is_some_and(|f| f.synced_hash.eq_ignore_ascii_case(local_hash));
        if matches {
            let name = fs::read_to_string(entry.path().join(ACCOUNT_NAME_FILE))
                .map(|n| n.trim().to_string())
                .unwrap_or_default();
            return Some(name);
        }
    }
    None
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

    #[test]
    fn another_accounts_manifest_for_the_game_means_shared() {
        let root = std::env::temp_dir()
            .join(format!("drop-manifest-shared-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // No manifest directory at all: nobody else.
        assert!(!other_accounts_have_synced_in(&root, "me", "g1", SystemTime::now()));
        fs::create_dir_all(root.join("me")).unwrap();
        fs::write(root.join("me").join("g1.json"), "{}").unwrap();
        assert!(!other_accounts_have_synced_in(&root, "me", "g1", SystemTime::now()));
        fs::create_dir_all(root.join("other")).unwrap();
        fs::write(root.join("other").join("g2.json"), "{}").unwrap();
        assert!(!other_accounts_have_synced_in(&root, "me", "g1", SystemTime::now()));
        fs::write(root.join("other").join("g1.json"), "{}").unwrap();
        assert!(other_accounts_have_synced_in(&root, "me", "g1", SystemTime::now()));
        // An account that has synced nothing here for half a year stops
        // counting.
        let later = SystemTime::now() + OTHER_ACCOUNT_ACTIVE_FOR + std::time::Duration::from_secs(60);
        assert!(!other_accounts_have_synced_in(&root, "me", "g1", later));
        let _ = fs::remove_dir_all(&root);
        // A path that exists but is not a listable directory fails closed.
        let file = std::env::temp_dir()
            .join(format!("drop-manifest-notadir-{}", std::process::id()));
        fs::write(&file, "x").unwrap();
        assert!(other_accounts_have_synced_in(&file, "me", "g1", SystemTime::now()));
        let _ = fs::remove_file(&file);
    }

    /// Entries written before the three-way check existed must load, and
    /// load as untrusted: some of them stamped a declined conflict as synced.
    /// New entries round-trip with the marker set.
    #[test]
    fn entries_from_older_builds_load_untrusted_and_new_ones_round_trip() {
        let old = r#"{"gameId":"g1","files":{"a.srm":{"saveType":"save",
            "syncedHash":"abc","cloudId":"row","syncedAt":"2026-06-10T17:00:24Z"}}}"#;
        let manifest: SyncManifest = serde_json::from_str(old).unwrap();
        assert!(!manifest.files["a.srm"].three_way_base);
        assert!(manifest.files["a.srm"].trusted_base().is_none());

        let mut fresh = SyncManifest::default();
        record_synced_files(&mut fresh, &[file("a.srm", "abc")], &HashMap::new(), &[]);
        let json = serde_json::to_string(&fresh).unwrap();
        assert!(json.contains("\"threeWayBase\":true"), "{json}");
        let back: SyncManifest = serde_json::from_str(&json).unwrap();
        assert!(back.files["a.srm"].trusted_base().is_some());
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
