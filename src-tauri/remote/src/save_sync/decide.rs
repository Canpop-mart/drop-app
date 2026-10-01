//! The three-way sync verdict: local copy, cloud copy, and what both looked
//! like at the last successful sync.
//!
//! The server's `sync-check` only sees two of those three. Whenever the local
//! and cloud hashes differ it answers "conflict", so playing on the Deck and
//! then launching on the PC (only the cloud changed) asked the user to pick a
//! side exactly as if both had changed. The client keeps the third hash, the
//! manifest's `synced_hash`, and uses it here to downgrade a server conflict
//! to the plain download or upload it really is.
//!
//! Done client-side rather than by sending a base hash to the server: the
//! manifest already lives here, the rule is a pure function that can be
//! tested in isolation, and it works unchanged against servers that predate
//! it.

use super::{SyncCheckResponse, SyncFileEntry, SyncManifest};

/// What should happen to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncVerdict {
    /// Both sides hold the same bytes.
    Synced,
    /// Only the cloud changed since the last sync: pull it down.
    Download,
    /// Only this device changed since the last sync: push it up.
    Upload,
    /// Both changed, or there is no trustworthy record of the last sync.
    Conflict,
}

impl SyncVerdict {
    /// The action string `sync-check` uses for the same verdict.
    pub fn as_action(self) -> &'static str {
        match self {
            SyncVerdict::Synced => "synced",
            SyncVerdict::Download => "download",
            SyncVerdict::Upload => "upload",
            SyncVerdict::Conflict => "conflict",
        }
    }
}

fn same_hash(a: &str, b: &str) -> bool {
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

/// Decide one file from its local hash, its cloud row, and the manifest entry
/// recorded at the last sync.
///
/// | local vs last sync | cloud vs last sync | verdict  |
/// |--------------------|--------------------|----------|
/// | unchanged          | changed            | Download |
/// | changed            | unchanged          | Upload   |
/// | changed            | changed            | Conflict |
///
/// Local equal to cloud is `Synced` whatever the manifest says.
///
/// An entry is only used when this build's rules wrote it
/// ([`SyncFileEntry::trusted_base`]): older builds stamped declined conflicts
/// as synced. It is further only trusted when it names the same cloud
/// row (`cloud_id`) the server returned. Builds before the manifest fixes wrote
/// entries for files that never reached the cloud, always with no cloud id;
/// trusting one of those would turn a real conflict into a silent download
/// over local progress. Without a trustworthy base the answer stays
/// `Conflict`, which is exactly what happened before this existed.
pub fn three_way_verdict(
    local_hash: &str,
    cloud_hash: &str,
    cloud_id: &str,
    base: Option<&SyncFileEntry>,
) -> SyncVerdict {
    if same_hash(local_hash, cloud_hash) {
        return SyncVerdict::Synced;
    }
    let Some(base) = base.and_then(SyncFileEntry::trusted_base) else {
        return SyncVerdict::Conflict;
    };
    if base.cloud_id.as_deref() != Some(cloud_id) || base.synced_hash.is_empty() {
        return SyncVerdict::Conflict;
    }
    let local_changed = !same_hash(local_hash, &base.synced_hash);
    let cloud_changed = !same_hash(cloud_hash, &base.synced_hash);
    match (local_changed, cloud_changed) {
        (false, true) => SyncVerdict::Download,
        (true, false) => SyncVerdict::Upload,
        (true, true) => SyncVerdict::Conflict,
        // Local equals the base and so does the cloud, so they equal each
        // other, which the first check already returned for.
        (false, false) => SyncVerdict::Synced,
    }
}

/// Rewrite every `"conflict"` action in a sync-check response using
/// [`three_way_verdict`] against `manifest`. Returns how many changed.
///
/// Only conflicts are touched: the server's other verdicts already mean what
/// they say. An action with no echoed local hash or no cloud row is left as a
/// conflict, the safe answer.
pub fn reclassify_with_manifest(response: &mut SyncCheckResponse, manifest: &SyncManifest) -> usize {
    let mut changed = 0;
    for action in response.actions.iter_mut().filter(|a| a.action == "conflict") {
        let (Some(local_hash), Some(cloud)) = (&action.local_hash, &action.cloud_save) else {
            continue;
        };
        let verdict = three_way_verdict(
            local_hash,
            &cloud.data_hash,
            &cloud.id,
            manifest.files.get(&action.filename),
        );
        if verdict != SyncVerdict::Conflict {
            log::info!(
                "[SAVE-SYNC] {}: only one side changed since the last sync, {} instead of asking",
                action.filename,
                verdict.as_action()
            );
            action.action = verdict.as_action().to_string();
            changed += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save_sync::{CloudSaveMeta, SyncAction};

    fn base(hash: &str, cloud_id: Option<&str>) -> SyncFileEntry {
        SyncFileEntry {
            save_type: "save".into(),
            synced_hash: hash.into(),
            cloud_id: cloud_id.map(str::to_string),
            synced_at: "2026-09-01T00:00:00Z".into(),
            three_way_base: true,
        }
    }

    #[test]
    fn only_the_cloud_changed_downloads() {
        let b = base("aaa", Some("row"));
        assert_eq!(three_way_verdict("aaa", "bbb", "row", Some(&b)), SyncVerdict::Download);
    }

    #[test]
    fn only_this_device_changed_uploads() {
        let b = base("aaa", Some("row"));
        assert_eq!(three_way_verdict("ccc", "aaa", "row", Some(&b)), SyncVerdict::Upload);
    }

    #[test]
    fn both_changed_is_a_conflict() {
        let b = base("aaa", Some("row"));
        assert_eq!(three_way_verdict("ccc", "bbb", "row", Some(&b)), SyncVerdict::Conflict);
    }

    #[test]
    fn identical_copies_are_synced_whatever_the_manifest_says() {
        let b = base("zzz", Some("row"));
        assert_eq!(three_way_verdict("abc", "ABC", "row", Some(&b)), SyncVerdict::Synced);
        assert_eq!(three_way_verdict("abc", "abc", "row", None), SyncVerdict::Synced);
    }

    /// First sync on this device: no record, so the old behaviour stands.
    #[test]
    fn no_record_of_a_sync_stays_a_conflict() {
        assert_eq!(three_way_verdict("aaa", "bbb", "row", None), SyncVerdict::Conflict);
    }

    /// An entry from the era when manifests recorded files that never
    /// reached the cloud has no cloud id. Trusting it would download over
    /// local progress without asking.
    #[test]
    fn a_record_without_a_cloud_id_is_not_trusted() {
        let b = base("aaa", None);
        assert_eq!(three_way_verdict("aaa", "bbb", "row", Some(&b)), SyncVerdict::Conflict);
    }

    /// A record about a different cloud row (deleted and re-created since)
    /// says nothing about this one.
    #[test]
    fn a_record_for_another_cloud_row_is_not_trusted() {
        let b = base("aaa", Some("old-row"));
        assert_eq!(three_way_verdict("aaa", "bbb", "row", Some(&b)), SyncVerdict::Conflict);
    }

    #[test]
    fn an_empty_recorded_hash_is_not_trusted() {
        let b = base("", Some("row"));
        assert_eq!(three_way_verdict("aaa", "bbb", "row", Some(&b)), SyncVerdict::Conflict);
    }

    fn cloud(id: &str, filename: &str, hash: &str) -> CloudSaveMeta {
        CloudSaveMeta {
            id: id.into(),
            filename: filename.into(),
            save_type: "save".into(),
            data_hash: hash.into(),
            size: 1,
            uploaded_from: "Deck".into(),
            client_modified_at: String::new(),
            uploaded_at: String::new(),
            owned_by: String::new(),
            shadowed_save_id: None,
            also_held_by: Vec::new(),
        }
    }

    fn action(filename: &str, verdict: &str, local: &str, cloud_save: CloudSaveMeta) -> SyncAction {
        SyncAction {
            filename: filename.into(),
            action: verdict.into(),
            cloud_save: Some(cloud_save),
            local_hash: Some(local.into()),
        }
    }

    #[test]
    fn server_conflicts_are_rewritten_and_nothing_else_is() {
        let mut manifest = SyncManifest::default();
        manifest.files.insert("pull.srm".into(), base("old", Some("c1")));
        manifest.files.insert("push.srm".into(), base("old", Some("c2")));
        manifest.files.insert("both.srm".into(), base("old", Some("c3")));
        // A server "synced" is left alone even if the manifest is stale.
        manifest.files.insert("same.srm".into(), base("stale", Some("c4")));

        let mut response = SyncCheckResponse {
            actions: vec![
                action("pull.srm", "conflict", "old", cloud("c1", "pull.srm", "new")),
                action("push.srm", "conflict", "new", cloud("c2", "push.srm", "old")),
                action("both.srm", "conflict", "mine", cloud("c3", "both.srm", "theirs")),
                action("same.srm", "synced", "x", cloud("c4", "same.srm", "x")),
                action("unknown.srm", "conflict", "a", cloud("c5", "unknown.srm", "b")),
            ],
            cloud_only: Vec::new(),
            tombstones: Vec::new(),
        };

        assert_eq!(reclassify_with_manifest(&mut response, &manifest), 2);
        let verdicts: Vec<&str> = response.actions.iter().map(|a| a.action.as_str()).collect();
        assert_eq!(verdicts, vec!["download", "upload", "conflict", "synced", "conflict"]);
    }

    /// The upgrade case: an older build recorded a declined conflict as
    /// synced (local hash, conflicting row). It must still ask, not download.
    #[test]
    fn an_entry_from_an_older_build_is_not_used_as_a_base() {
        let mut manifest = SyncManifest::default();
        manifest.files.insert(
            "kept.srm".into(),
            SyncFileEntry {
                three_way_base: false,
                ..base("mine", Some("c1"))
            },
        );
        let mut response = SyncCheckResponse {
            actions: vec![action("kept.srm", "conflict", "mine", cloud("c1", "kept.srm", "theirs"))],
            cloud_only: Vec::new(),
            tombstones: Vec::new(),
        };
        assert_eq!(reclassify_with_manifest(&mut response, &manifest), 0);
        assert_eq!(response.actions[0].action, "conflict");
        // Called directly, the verdict applies the same rule.
        let old = SyncFileEntry {
            three_way_base: false,
            ..base("mine", Some("c1"))
        };
        assert_eq!(three_way_verdict("mine", "theirs", "c1", Some(&old)), SyncVerdict::Conflict);
    }
}
