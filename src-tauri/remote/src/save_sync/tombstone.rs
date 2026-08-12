//! Deciding which server tombstones this device should act on.
//!
//! `sync-check` returns every tombstone the user has for a game, unfiltered,
//! on every launch, and keeps doing so until the server GC purges the row 30
//! days later. Applying that list verbatim is destructive twice over:
//!
//! * The device that *issued* the delete gets its own tombstone back and
//!   deletes the local file the user never asked it to touch.
//! * The same tombstone replays on the next launch. By then the game has
//!   written a fresh save, so the replay backs that up over the previous
//!   backup and deletes it too. Two launches and both copies are gone.
//!
//! [`plan_tombstones`] applies both filters. It is pure — no disk, no
//! network, no `machine_name()` lookup — so the rules can be tested directly.

use std::collections::HashMap;

use super::{SyncManifest, Tombstone};

/// Does `deleted_from` name the device we are running on?
///
/// An empty `deletedFrom` comes from a server old enough not to record one.
/// We cannot tell whose delete it was, so it is treated as someone else's and
/// applied — the alternative silently drops real cross-device deletes.
/// Comparison ignores case because a hostname's casing is not stable across
/// the places it gets read from.
fn is_self_issued(deleted_from: &str, this_device: &str) -> bool {
    let issued_by = deleted_from.trim();
    let here = this_device.trim();
    !issued_by.is_empty() && !here.is_empty() && issued_by.eq_ignore_ascii_case(here)
}

/// Has this exact tombstone already been applied on this device? Keyed on
/// `deletedAt` as well as the filename so a *second*, genuinely new delete of
/// a re-uploaded save still gets applied.
fn already_applied(applied: &HashMap<String, String>, t: &Tombstone) -> bool {
    applied
        .get(&t.filename)
        .is_some_and(|deleted_at| deleted_at == &t.deleted_at)
}

/// What to do with one sync-check tombstone list.
#[derive(Debug, Default)]
pub struct TombstonePlan<'a> {
    /// Delete these local files (after backing them up).
    pub apply: Vec<&'a Tombstone>,
    /// This device issued these deletes. There is nothing to remove — the
    /// user already removed it here — but they get recorded as applied so a
    /// later device rename cannot resurrect them into the `apply` list.
    pub self_issued: Vec<&'a Tombstone>,
    /// Already applied on a previous launch. Ignored entirely.
    pub replays: usize,
}

/// Split `tombstones` into the ones this device should act on and the ones it
/// should only record. See the module docs for why both filters exist.
pub fn plan_tombstones<'a>(
    tombstones: &'a [Tombstone],
    manifest: &SyncManifest,
    this_device: &str,
) -> TombstonePlan<'a> {
    let mut plan = TombstonePlan::default();
    for t in tombstones {
        if already_applied(&manifest.applied_tombstones, t) {
            plan.replays += 1;
        } else if is_self_issued(&t.deleted_from, this_device) {
            plan.self_issued.push(t);
        } else {
            plan.apply.push(t);
        }
    }
    plan
}

/// Record a tombstone as handled so it is never applied a second time.
///
/// Called for a tombstone this device issued, and for one it applied — even
/// when there was no local file to delete, because the file the *next* launch
/// finds under that name is a new save, not the one the tombstone is about.
/// Deliberately NOT called when the delete failed, so a real failure retries.
pub fn record_applied(manifest: &mut SyncManifest, t: &Tombstone) {
    manifest
        .applied_tombstones
        .insert(t.filename.clone(), t.deleted_at.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tomb(filename: &str, deleted_at: &str, deleted_from: &str) -> Tombstone {
        Tombstone {
            filename: filename.to_string(),
            deleted_at: deleted_at.to_string(),
            deleted_from: deleted_from.to_string(),
        }
    }

    fn manifest() -> SyncManifest {
        SyncManifest {
            game_id: "g1".into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_delete_this_device_issued_is_not_applied_here() {
        let tombs = vec![tomb("gen.sav", "2026-08-01T00:00:00Z", "Marts PC")];
        let plan = plan_tombstones(&tombs, &manifest(), "Marts PC");
        assert!(plan.apply.is_empty());
        assert_eq!(plan.self_issued.len(), 1);
    }

    #[test]
    fn device_name_comparison_ignores_case_and_padding() {
        let tombs = vec![tomb("gen.sav", "t", " MARTS-pc ")];
        let plan = plan_tombstones(&tombs, &manifest(), "marts-PC");
        assert!(plan.apply.is_empty(), "{plan:?}");
    }

    #[test]
    fn a_delete_from_another_device_is_applied() {
        let tombs = vec![tomb("gen.sav", "t", "Steam Deck")];
        let plan = plan_tombstones(&tombs, &manifest(), "Marts PC");
        assert_eq!(plan.apply.len(), 1);
        assert!(plan.self_issued.is_empty());
    }

    #[test]
    fn an_unattributed_delete_is_applied() {
        // Pre-T5 servers don't send `deletedFrom`. Applying is the safe
        // default: skipping would break real cross-device deletes.
        let tombs = vec![tomb("gen.sav", "t", "")];
        let plan = plan_tombstones(&tombs, &manifest(), "Marts PC");
        assert_eq!(plan.apply.len(), 1);
    }

    #[test]
    fn the_same_tombstone_is_applied_only_once() {
        let tombs = vec![tomb("gen.sav", "2026-08-01T00:00:00Z", "Steam Deck")];
        let mut m = manifest();

        let plan = plan_tombstones(&tombs, &m, "Marts PC");
        assert_eq!(plan.apply.len(), 1);
        record_applied(&mut m, plan.apply[0]);

        // The server keeps re-sending it for 30 days; every later launch is
        // a no-op instead of a second delete.
        let replay = plan_tombstones(&tombs, &m, "Marts PC");
        assert!(replay.apply.is_empty());
        assert_eq!(replay.replays, 1);
    }

    #[test]
    fn a_second_delete_of_a_reuploaded_save_is_applied_again() {
        let mut m = manifest();
        let first = vec![tomb("gen.sav", "2026-08-01T00:00:00Z", "Steam Deck")];
        record_applied(&mut m, &first[0]);

        // Same filename, new delete: the user re-uploaded and deleted again.
        let second = vec![tomb("gen.sav", "2026-08-09T12:00:00Z", "Steam Deck")];
        assert_eq!(plan_tombstones(&second, &m, "Marts PC").apply.len(), 1);
    }

    #[test]
    fn self_issued_tombstones_survive_a_device_rename() {
        let tombs = vec![tomb("gen.sav", "t", "Marts PC")];
        let mut m = manifest();
        let plan = plan_tombstones(&tombs, &m, "Marts PC");
        for t in &plan.self_issued {
            record_applied(&mut m, t);
        }
        // Renaming the device would otherwise turn our own delete into a
        // foreign one and unlink the local file.
        let after_rename = plan_tombstones(&tombs, &m, "Marts Desktop");
        assert!(after_rename.apply.is_empty());
    }
}
