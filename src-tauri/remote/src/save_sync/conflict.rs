//! Conflict detection and resolution for save sync.
//!
//! When the server's sync-check verdict for a file is `"conflict"` (both sides
//! changed since the last sync), the client cannot decide unilaterally — it
//! surfaces the conflict to the UI. This module turns a [`SyncCheckResponse`]
//! into UI-facing [`SaveConflict`]s and applies the user's
//! [`ConflictResolution`] choices back into download/upload work lists.

use std::collections::HashMap;

use super::{ConflictResolution, LocalSaveFile, SaveConflict, SyncCheckResponse};

/// Build a hashmap of filename → MD5 from a list of local save files.
/// Used to snapshot pre-launch state for change detection on exit.
pub fn snapshot_hashes(files: &[LocalSaveFile]) -> HashMap<String, String> {
    files
        .iter()
        .map(|f| (f.filename.clone(), f.data_hash.clone()))
        .collect()
}

/// Build the list of conflicts from the sync-check response + local file info.
/// Only `"conflict"` actions with both a matching local file and cloud save
/// become a [`SaveConflict`].
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
///
/// Returns `(download_ids, upload_filenames)` — the cloud-save IDs to download
/// for `"keep_cloud"` choices, and the local filenames to upload for
/// `"keep_local"`. An unrecognised/missing choice defaults to keeping local,
/// which is the safer option (the user does not lose current work).
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
