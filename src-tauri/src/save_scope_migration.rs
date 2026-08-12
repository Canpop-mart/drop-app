//! One-time move of this device's save state into per-user directories.
//!
//! Local save state used to carry no identity at all: the manifest lived at
//! `{DATA_ROOT_DIR}/sync-manifests/{gameId}.json` and emulator saves at
//! `{emu}/drop-saves/{gameId}`, while the server keyed every cloud row by user
//! id. Two Drop accounts on one PC therefore poured their saves into each
//! other's cloud library. The paths are per-user now; this brings the data
//! that already exists along with them.
//!
//! Order matters, and the two halves are deliberately unequal in risk:
//!
//! * **Manifests** are pure metadata. Nothing reads their contents to find a
//!   save, so a mistake here costs a re-hash, not a save file. They move first.
//! * **Emulator directories** are the user's actual save bytes. They move only
//!   under a claim file, only while nothing is running, and only by
//!   `fs::rename`. There is no copy-then-delete fallback: a half-copied save
//!   that then deletes its original is exactly the failure this whole stage
//!   exists to prevent.
//!
//! Nothing here ever deletes. The one thing a pass can do wrong is leave save
//! bytes somewhere no later code path looks, so any directory this pass could
//! not move keeps the migration *unfinished*: the version does not bump and the
//! next start retries it.
//!
//! The version is a "should this pass run again" record and nothing more.
//! [`remote::save_sync::resolve_emu_saves_root`] decides where an emulator's
//! saves are by looking at the disk, never by reading this version, because a
//! tree this pass could not even see (an uninstalled emulator, a drive that was
//! offline at startup) cannot report itself as unfinished work.
//!
//! The ownership rules (the claim file, the user-root marker) live in
//! [`remote::save_sync::scope`], not here, because the launch path needs the
//! same answers.
//!
//! Cloud rows need no migration — the server has always keyed them by user id.

use std::fs;
use std::path::{Path, PathBuf};

use database::{
    GameDownloadStatus, borrow_db_checked, borrow_db_mut_checked, db::DATA_ROOT_DIR,
};
use log::{info, warn};
use process::PROCESS_MANAGER;
use remote::save_sync::{
    ClaimVerdict, DROP_SAVES_DIR, OWNER_CLAIM_FILE, SAVE_SCOPE_MIGRATION_VERSION, claim_verdict,
    ensure_user_root, is_user_root, manifest::MANIFEST_DIR, read_claim, write_claim,
};

/// Run the migration if it has not already run on this device.
///
/// Safe to call on every startup and after every sign-in. With nobody signed
/// in it does nothing at all and leaves the version un-bumped, so the next
/// sign-in retries — there is no correct answer to "whose saves are these"
/// while signed out, and inventing one is the bug.
pub fn run() {
    if borrow_db_checked().settings.save_scope_migration_version
        >= SAVE_SCOPE_MIGRATION_VERSION
    {
        return;
    }

    let Some(user_id) = remote::save_sync::current_user_id() else {
        info!(
            "[SAVE-SCOPE] Nobody is signed in; deferring the save-scope migration until sign-in"
        );
        return;
    };

    let (moved, stranded_manifests) =
        migrate_manifests(&DATA_ROOT_DIR.join(MANIFEST_DIR), &user_id);
    if moved > 0 {
        info!("[SAVE-SCOPE] Moved {moved} sync manifest(s) into {user_id}/");
    }
    // A stranded manifest is recoverable (the next sync re-hashes and
    // re-uploads) but it takes `applied_tombstones` with it, so a still-live
    // tombstone from another device re-applies. Same rule as a stranded save
    // directory: retry rather than declare victory.
    let mut all_ok = stranded_manifests == 0;

    // The install directories are collected before the process lock is taken:
    // the launch path holds the process lock while it reads the database, so
    // taking them in the other order here is the one combination that could
    // deadlock.
    let emu_roots = emulator_install_dirs();

    // The guard is *held* for the whole emulator pass, not just sampled for
    // it. Reading `any_game_active()` into a bool and letting the temporary
    // guard drop would leave a launch free to start between two renames and
    // have its save directory moved out from under it, which is the exact
    // corruption `migrate_emu_tree`'s `games_running` check exists to prevent.
    // The pass below only touches the filesystem, so nothing it calls can
    // re-enter the process manager.
    let process_manager = PROCESS_MANAGER.lock();
    let games_running = process_manager.any_game_active();
    for emu_root in emu_roots {
        let drop_saves = emu_root.join(DROP_SAVES_DIR);
        if !drop_saves.is_dir() {
            continue;
        }
        match migrate_emu_tree(&emu_root, &user_id, games_running) {
            Ok(outcome) => {
                info!("[SAVE-SCOPE] {}: {outcome:?}", drop_saves.display());
                if outcome.left_work_undone() {
                    warn!(
                        "[SAVE-SCOPE] {} still holds save directories this pass could not \
                         adopt. They are untouched and Drop keeps reading them where they \
                         are, but the move is not finished.",
                        drop_saves.display()
                    );
                    all_ok = false;
                }
            }
            Err(e) => {
                warn!("[SAVE-SCOPE] {}: {e}", drop_saves.display());
                all_ok = false;
            }
        }
    }
    drop(process_manager);

    if !all_ok {
        warn!(
            "[SAVE-SCOPE] Save-scope migration left work undone; it will retry on the next \
             start. Nothing was deleted."
        );
        return;
    }

    borrow_db_mut_checked().settings.save_scope_migration_version =
        SAVE_SCOPE_MIGRATION_VERSION;
    info!("[SAVE-SCOPE] Save-scope migration complete for user {user_id}");
}

/// Every installed game's directory. Emulators are ordinary installed titles
/// in Drop, so this is also the set of places a `drop-saves` tree can exist.
///
/// Only `Installed` titles: a `drop-saves` tree under a title Drop has since
/// demoted to `Remote` (uninstalled but left on disk, or on a drive that was
/// offline at startup) is not visible here, and `Remote` carries no install
/// path, so this pass cannot even find it to report it as undone work. That
/// tree is left exactly where it is and stays readable through the un-migrated
/// fallback in [`remote::save_sync::resolve_emu_saves_root`], which is why that
/// fallback is gated on the disk rather than on this migration's version.
fn emulator_install_dirs() -> Vec<PathBuf> {
    borrow_db_checked()
        .applications
        .game_statuses
        .values()
        .filter_map(|status| match status {
            GameDownloadStatus::Installed { install_dir, .. } => {
                Some(PathBuf::from(install_dir))
            }
            GameDownloadStatus::Remote {} => None,
        })
        .collect()
}

/// Move `sync-manifests/{gameId}.json` into `sync-manifests/{userId}/`.
///
/// Returns `(moved, stranded)`. A manifest that fails to move is left where it
/// is; nothing is ever deleted, and an existing destination is never clobbered
/// (that source is stale debris, not a failure). `stranded` counts only real
/// failures, and a non-zero count keeps the migration unfinished so the next
/// start retries.
pub(crate) fn migrate_manifests(manifest_dir: &Path, user_id: &str) -> (usize, usize) {
    let Ok(entries) = fs::read_dir(manifest_dir) else {
        return (0, 0);
    };
    let dest_dir = manifest_dir.join(user_id);
    let mut moved = 0usize;
    let mut stranded = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        // Only the manifests themselves. `.json.bak.<ts>` and `.json.tmp`
        // leftovers stay put: they are debris, and moving debris into the
        // live directory is how it stops looking like debris.
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let dest = dest_dir.join(name);
        if dest.exists() {
            continue;
        }
        if let Err(e) = fs::create_dir_all(&dest_dir) {
            warn!("[SAVE-SCOPE] Could not create {}: {e}", dest_dir.display());
            return (moved, stranded + 1);
        }
        match fs::rename(&path, &dest) {
            Ok(()) => moved += 1,
            Err(e) => {
                warn!(
                    "[SAVE-SCOPE] Could not move manifest {}: {e}",
                    path.display()
                );
                stranded += 1;
            }
        }
    }
    (moved, stranded)
}

/// What one emulator's `drop-saves` tree ended up doing.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EmuOutcome {
    /// Legacy game directories moved under this user. `skipped` counts the
    /// ones left behind because a directory of the same name already existed
    /// under this user — merging two trees means picking which copy of a save
    /// wins, which is not a choice a migration gets to make silently.
    Adopted { moved: usize, skipped: usize },
    /// Another account owns the legacy tree; this user got an empty directory.
    Isolated,
    /// The claim file exists but names nobody, so there is no way to tell whose
    /// saves these are. `legacy_dirs` is how many directories are stuck behind
    /// that ambiguity.
    Unclaimable { legacy_dirs: usize },
}

impl EmuOutcome {
    /// Whether this pass left legacy save bytes sitting outside a user root.
    ///
    /// The version must not bump while this is true, so the next start tries
    /// again. Until it succeeds those saves are only reachable through the
    /// un-migrated fallback in `resolve_emu_saves_root`, which is a
    /// compatibility path, not the layout the rest of the sync code is written
    /// against.
    pub(crate) fn left_work_undone(&self) -> bool {
        match self {
            EmuOutcome::Adopted { skipped, .. } => *skipped > 0,
            EmuOutcome::Isolated => false,
            EmuOutcome::Unclaimable { legacy_dirs } => *legacy_dirs > 0,
        }
    }
}

/// Adopt (or isolate from) one emulator's legacy `drop-saves` tree.
///
/// Refuses outright while a game is active: the rename would pull the save
/// directory out from under a running emulator that has the old path in its
/// config. Every move is an `fs::rename` on the same volume — if one fails,
/// that directory is left exactly as it was and the error is returned, so the
/// migration retries next start rather than half-copying a save.
pub(crate) fn migrate_emu_tree(
    emu_root: &Path,
    user_id: &str,
    games_running: bool,
) -> Result<EmuOutcome, String> {
    if games_running {
        return Err(
            "a game is running, so save directories must not be moved right now".to_string(),
        );
    }

    let drop_saves = &emu_root.join(DROP_SAVES_DIR);
    let existing_owner = read_claim(drop_saves);

    if claim_verdict(existing_owner.as_deref(), user_id) == ClaimVerdict::Isolate {
        // Somebody else's tree. Create this user's own root and touch nothing
        // else — not even to read it.
        ensure_user_root(emu_root, user_id)?;
        // A claim that names nobody is not the same as a claim that names
        // someone else. Say so, and keep the migration open: the legacy
        // directories are still there, and deleting the claim file by hand is
        // the one thing that lets them be adopted.
        if existing_owner.as_deref().map(str::trim) == Some("") {
            let legacy_dirs = count_legacy_dirs(drop_saves, user_id);
            if legacy_dirs > 0 {
                warn!(
                    "[SAVE-SCOPE] {} names no owner, so {legacy_dirs} save director(ies) in \
                     {} cannot be adopted without guessing whose they are. They are untouched. \
                     Deleting that file lets the next sign-in claim them.",
                    drop_saves.join(OWNER_CLAIM_FILE).display(),
                    drop_saves.display()
                );
            }
            return Ok(EmuOutcome::Unclaimable { legacy_dirs });
        }
        return Ok(EmuOutcome::Isolated);
    }

    let dest = ensure_user_root(emu_root, user_id)?;
    // Claim before moving, not after. A crash mid-move then leaves a tree
    // owned by us and half-migrated, which the next run finishes; the other
    // order would leave an unclaimed half-migrated tree that a second account
    // could then adopt.
    if existing_owner.is_none() {
        write_claim(drop_saves, user_id)?;
    }

    let entries = fs::read_dir(drop_saves)
        .map_err(|e| format!("could not read {}: {e}", drop_saves.display()))?;
    let mut moved = 0usize;
    let mut skipped = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Skip every user root, not just ours. An account that was isolated
        // here has a sibling root, and sweeping it into our tree would hand us
        // their saves — the exact thing this migration exists to prevent.
        if name == user_id || is_user_root(&path) {
            continue;
        }
        let target = dest.join(name);
        if target.exists() {
            // Already migrated once and re-created by a signed-out session.
            // Merging two trees means choosing which copy of a save wins,
            // which is not a choice a migration gets to make silently. Counted,
            // not just logged: a skipped directory used to still report a clean
            // adopt, so the version bumped and nothing ever came back for it.
            warn!(
                "[SAVE-SCOPE] {} already exists; leaving {} alone",
                target.display(),
                path.display()
            );
            skipped += 1;
            continue;
        }
        fs::rename(&path, &target).map_err(|e| {
            format!(
                "could not move {} to {}: {e} (left untouched)",
                path.display(),
                target.display()
            )
        })?;
        moved += 1;
    }

    Ok(EmuOutcome::Adopted { moved, skipped })
}

/// How many legacy game directories are sitting in `drop_saves` — everything
/// that is neither a user root nor this user's own.
fn count_legacy_dirs(drop_saves: &Path, user_id: &str) -> usize {
    let Ok(entries) = fs::read_dir(drop_saves) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|entry| {
            let path = entry.path();
            path.file_name().and_then(|n| n.to_str()) != Some(user_id) && !is_user_root(&path)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use remote::save_sync::USER_ROOT_MARKER;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("drop-save-scope-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn the_first_account_adopts_the_legacy_tree() {
        let root = tmpdir("adopt");
        let drop_saves = root.join("drop-saves");
        write(&drop_saves.join("game-1/saves/gen.srm"), "save bytes");
        write(&drop_saves.join("game-2/states/a.state1"), "state bytes");

        let outcome = migrate_emu_tree(&root, "user-a", false).unwrap();
        assert_eq!(
            outcome,
            EmuOutcome::Adopted {
                moved: 2,
                skipped: 0
            }
        );
        assert!(!outcome.left_work_undone());
        assert_eq!(
            fs::read_to_string(drop_saves.join("user-a/game-1/saves/gen.srm")).unwrap(),
            "save bytes"
        );
        assert!(!drop_saves.join("game-1").exists());
        assert_eq!(
            fs::read_to_string(drop_saves.join(OWNER_CLAIM_FILE)).unwrap(),
            "user-a"
        );

        // Re-running is a no-op, not a second move.
        assert_eq!(
            migrate_emu_tree(&root, "user-a", false).unwrap(),
            EmuOutcome::Adopted {
                moved: 0,
                skipped: 0
            }
        );
    }

    /// The orphan case. A directory the pass cannot adopt must keep the
    /// migration unfinished, or nothing ever comes back to move those bytes
    /// and they are stuck on the compatibility path forever.
    #[test]
    fn a_directory_that_cannot_be_adopted_keeps_the_migration_unfinished() {
        let root = tmpdir("skip");
        let drop_saves = root.join("drop-saves");
        write(&drop_saves.join("game-1/saves/gen.srm"), "the real save");
        // The destination already exists — exactly what a launch does when it
        // creates `drop-saves/{user}/{game}/saves` before the migration has
        // managed to run.
        write(&drop_saves.join("user-a/game-1/saves/other.srm"), "other");

        let outcome = migrate_emu_tree(&root, "user-a", false).unwrap();
        assert_eq!(
            outcome,
            EmuOutcome::Adopted {
                moved: 0,
                skipped: 1
            }
        );
        assert!(
            outcome.left_work_undone(),
            "a skipped directory reported success, so the version would bump and no later \
             pass would ever come back for these saves"
        );
        assert_eq!(
            fs::read_to_string(drop_saves.join("game-1/saves/gen.srm")).unwrap(),
            "the real save"
        );
    }

    #[test]
    fn a_second_account_gets_an_empty_tree_and_touches_nothing() {
        let root = tmpdir("isolate");
        let drop_saves = root.join("drop-saves");
        write(&drop_saves.join("game-1/saves/gen.srm"), "user a's save");
        migrate_emu_tree(&root, "user-a", false).unwrap();

        let outcome = migrate_emu_tree(&root, "user-b", false).unwrap();
        assert_eq!(outcome, EmuOutcome::Isolated);
        assert!(drop_saves.join("user-b").is_dir());
        let inherited: Vec<String> = fs::read_dir(drop_saves.join("user-b"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != USER_ROOT_MARKER)
            .collect();
        assert!(
            inherited.is_empty(),
            "the second account inherited saves that are not theirs: {inherited:?}"
        );
        // User A's adopted tree is untouched.
        assert_eq!(
            fs::read_to_string(drop_saves.join("user-a/game-1/saves/gen.srm")).unwrap(),
            "user a's save"
        );
        assert_eq!(
            fs::read_to_string(drop_saves.join(OWNER_CLAIM_FILE)).unwrap(),
            "user-a"
        );
    }

    /// A tree claimed by someone else that still has legacy directories in it
    /// (a signed-out session wrote there after the claim) must be left alone
    /// entirely, not swept into whoever runs next.
    #[test]
    fn a_foreign_claim_leaves_legacy_directories_where_they_are() {
        let root = tmpdir("foreign-claim");
        let drop_saves = root.join("drop-saves");
        write(&drop_saves.join(OWNER_CLAIM_FILE), "user-a");
        write(&drop_saves.join("game-1/saves/gen.srm"), "not user b's");

        assert_eq!(
            migrate_emu_tree(&root, "user-b", false).unwrap(),
            EmuOutcome::Isolated
        );
        assert_eq!(
            fs::read_to_string(drop_saves.join("game-1/saves/gen.srm")).unwrap(),
            "not user b's"
        );
        assert!(!drop_saves.join("user-b/game-1").exists());
    }

    /// A zero-length claim is what a power loss during the claim write used to
    /// leave behind. It must never be read as "unclaimed" — that is how account
    /// B ends up owning account A's saves.
    #[test]
    fn a_claim_that_names_nobody_is_never_adopted() {
        let root = tmpdir("blank-claim");
        let drop_saves = root.join("drop-saves");
        write(&drop_saves.join(OWNER_CLAIM_FILE), "");
        write(&drop_saves.join("game-1/saves/gen.srm"), "somebody's save");

        let outcome = migrate_emu_tree(&root, "user-b", false).unwrap();
        assert_eq!(outcome, EmuOutcome::Unclaimable { legacy_dirs: 1 });
        assert!(
            outcome.left_work_undone(),
            "the version must not bump while a tree is stuck behind a blank claim"
        );
        assert_eq!(
            fs::read_to_string(drop_saves.join("game-1/saves/gen.srm")).unwrap(),
            "somebody's save"
        );
        assert!(!drop_saves.join("user-b/game-1").exists());
        // The blank claim is left alone rather than repaired: rewriting it
        // would be this account asserting an ownership it cannot know it has.
        assert_eq!(
            fs::read_to_string(drop_saves.join(OWNER_CLAIM_FILE)).unwrap(),
            ""
        );
    }

    /// After B has been isolated, A's next pass sees `drop-saves/user-b` as a
    /// sibling directory. Sweeping it into `drop-saves/user-a/user-b` would
    /// hand A every save B has made since.
    #[test]
    fn a_later_pass_does_not_sweep_another_accounts_root_into_its_own() {
        let root = tmpdir("sibling-root");
        let drop_saves = root.join("drop-saves");
        write(&drop_saves.join("game-1/saves/gen.srm"), "user a's save");
        migrate_emu_tree(&root, "user-a", false).unwrap();
        migrate_emu_tree(&root, "user-b", false).unwrap();
        write(&drop_saves.join("user-b/game-9/saves/b.srm"), "user b's save");

        assert_eq!(
            migrate_emu_tree(&root, "user-a", false).unwrap(),
            EmuOutcome::Adopted {
                moved: 0,
                skipped: 0
            }
        );
        assert_eq!(
            fs::read_to_string(drop_saves.join("user-b/game-9/saves/b.srm")).unwrap(),
            "user b's save"
        );
        assert!(!drop_saves.join("user-a/user-b").exists());
    }

    #[test]
    fn a_running_game_blocks_the_move_entirely() {
        let root = tmpdir("running");
        let drop_saves = root.join("drop-saves");
        write(&drop_saves.join("game-1/saves/gen.srm"), "live save");

        let err = migrate_emu_tree(&root, "user-a", true).unwrap_err();
        assert!(err.contains("a game is running"), "{err}");
        // Nothing at all happened: no claim, no destination, no move.
        assert!(drop_saves.join("game-1/saves/gen.srm").is_file());
        assert!(!drop_saves.join(OWNER_CLAIM_FILE).exists());
        assert!(!drop_saves.join("user-a").exists());
    }

    #[test]
    fn manifests_move_into_the_user_directory_without_losing_any() {
        let root = tmpdir("manifests");
        write(&root.join("game-1.json"), "{}");
        write(&root.join("game-2.json"), "{}");
        // Debris stays put.
        write(&root.join("game-3.json.bak.1700000000"), "{}");
        write(&root.join("game-4.json.tmp"), "{}");

        assert_eq!(migrate_manifests(&root, "user-a"), (2, 0));
        assert!(root.join("user-a/game-1.json").is_file());
        assert!(root.join("user-a/game-2.json").is_file());
        assert!(!root.join("game-1.json").exists());
        assert!(root.join("game-3.json.bak.1700000000").is_file());
        assert!(root.join("game-4.json.tmp").is_file());

        // Idempotent, and never clobbers a manifest already in place. The
        // leftover source is stale debris, not stranded work.
        write(&root.join("game-1.json"), "newer, unmigrated");
        assert_eq!(migrate_manifests(&root, "user-a"), (0, 0));
        assert_eq!(
            fs::read_to_string(root.join("game-1.json")).unwrap(),
            "newer, unmigrated",
            "an existing destination must not be clobbered, and the source must survive"
        );
    }

    #[test]
    fn migrating_manifests_with_no_directory_is_a_no_op() {
        let root = tmpdir("no-manifests");
        assert_eq!(migrate_manifests(&root.join("nope"), "user-a"), (0, 0));
    }
}
