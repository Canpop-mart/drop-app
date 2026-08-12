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
///
/// `"skip"` is the dismissal answer and produces no work at all: the dialog was
/// closed (Esc, backdrop click, "Decide later") without the user picking a
/// side, so neither copy is touched. Callers must also treat a skip as an
/// unresolved sync — see [`any_conflict_deferred`].
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
            Some(&"skip") => {}
            _ => {
                // Default: keep local (safer — user doesn't lose current work)
                upload_filenames.push(conflict.filename.clone());
            }
        }
    }

    (download_ids, upload_filenames)
}

/// True when at least one conflict came back `"skip"`, i.e. the user dismissed
/// the dialog instead of answering it.
///
/// A skipped file still differs from its cloud row, so the post-exit upload
/// must not run: it diffs against the pre-launch hashes and would push this
/// session's bytes over the copy the user never chose to discard. Treating a
/// dismissal exactly like the resolve timeout is what makes closing the dialog
/// safe to do by accident.
pub fn any_conflict_deferred(
    conflicts: &[SaveConflict],
    resolutions: &[ConflictResolution],
) -> bool {
    let resolution_map: HashMap<&str, &str> = resolutions
        .iter()
        .map(|r| (r.filename.as_str(), r.choice.as_str()))
        .collect();
    conflicts
        .iter()
        .any(|c| resolution_map.get(c.filename.as_str()) == Some(&"skip"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conflict(filename: &str) -> SaveConflict {
        SaveConflict {
            filename: filename.to_string(),
            save_type: "save".to_string(),
            local_hash: "local".to_string(),
            local_size: 1,
            local_modified_at: 0,
            cloud_id: format!("cloud-{filename}"),
            cloud_hash: "cloud".to_string(),
            cloud_size: 1,
            cloud_modified_at: String::new(),
            cloud_uploaded_from: "other".to_string(),
        }
    }

    fn resolution(filename: &str, choice: &str) -> ConflictResolution {
        ConflictResolution {
            filename: filename.to_string(),
            choice: choice.to_string(),
        }
    }

    /// Dismissing the dialog must move no bytes in either direction. Before
    /// this, closing the modal recorded nothing at all and the launch simply
    /// blocked for the full resolve timeout.
    #[test]
    fn a_skipped_conflict_produces_no_work() {
        let conflicts = vec![conflict("a.srm"), conflict("b.srm")];
        let resolutions = vec![resolution("a.srm", "skip"), resolution("b.srm", "skip")];
        let (downloads, uploads) = apply_conflict_resolutions(&conflicts, &resolutions);
        assert!(downloads.is_empty());
        assert!(uploads.is_empty());
        assert!(any_conflict_deferred(&conflicts, &resolutions));
    }

    /// A real answer is unchanged, and must not read as deferred — otherwise
    /// every resolved conflict would block the post-exit upload.
    #[test]
    fn answered_conflicts_still_produce_work() {
        let conflicts = vec![conflict("a.srm"), conflict("b.srm")];
        let resolutions = vec![
            resolution("a.srm", "keep_cloud"),
            resolution("b.srm", "keep_local"),
        ];
        let (downloads, uploads) = apply_conflict_resolutions(&conflicts, &resolutions);
        assert_eq!(downloads, vec!["cloud-a.srm"]);
        assert_eq!(uploads, vec!["b.srm"]);
        assert!(!any_conflict_deferred(&conflicts, &resolutions));
    }

    /// One skip among real answers still defers the whole sync: the skipped
    /// file is diverged, and the upload gate is per-session not per-file.
    #[test]
    fn one_skip_defers_the_whole_sync() {
        let conflicts = vec![conflict("a.srm"), conflict("b.srm")];
        let resolutions = vec![
            resolution("a.srm", "keep_local"),
            resolution("b.srm", "skip"),
        ];
        assert!(any_conflict_deferred(&conflicts, &resolutions));
    }

    /// A missing answer keeps the historical default (keep local), so an older
    /// frontend that sends a short list behaves exactly as it did.
    #[test]
    fn a_missing_answer_still_defaults_to_keep_local() {
        let conflicts = vec![conflict("a.srm")];
        let (downloads, uploads) = apply_conflict_resolutions(&conflicts, &[]);
        assert!(downloads.is_empty());
        assert_eq!(uploads, vec!["a.srm"]);
        assert!(!any_conflict_deferred(&conflicts, &[]));
    }
}
