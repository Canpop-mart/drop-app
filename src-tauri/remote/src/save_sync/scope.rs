//! Who owns this device's legacy save tree, and which directory a game's
//! emulator saves are actually in right now.
//!
//! [`super::scan::emu_saves_root`] spells out the *target* layout,
//! `{emu}/drop-saves/{user_id}/{game_id}`. This module answers the harder
//! question the writer and the scanner both have to agree on: what to use
//! while the one-time move into that layout has not finished.
//!
//! Everything the migration needs to make that call lives here rather than in
//! the migration itself, because the launch path needs the same answer. A
//! writer that jumps to the per-user path the moment a user signs in, while
//! the saves are still sitting in the legacy tree, boots the game against an
//! empty directory and then writes a second, divergent copy of the save.
//!
//! The claim file is the whole ownership model. The legacy tree carries
//! nothing that says who played those games, so whoever migrates first claims
//! it and everyone else is isolated into their own empty root.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use log::warn;

use super::scan::{DROP_SAVES_DIR, emu_saves_root};

/// Bump when the on-disk save layout changes again. Stored per device in
/// `settings.save_scope_migration_version`.
pub const SAVE_SCOPE_MIGRATION_VERSION: u32 = 1;

/// Marks which account has adopted an emulator's legacy `drop-saves` tree.
///
/// The legacy tree can only belong to one person. Whoever migrates first
/// claims it and this file records that, so a second account on the same PC
/// gets a clean empty directory instead of inheriting saves that are not
/// theirs.
pub const OWNER_CLAIM_FILE: &str = ".drop-owner";

/// Dropped inside every `drop-saves/{userId}` root so a later pass can tell a
/// user root from a legacy `drop-saves/{gameId}` directory.
///
/// Only one account can be named in [`OWNER_CLAIM_FILE`], but an isolated
/// second account still has a root here. Without this marker the adopting
/// account's next pass would see that sibling as a stray game directory and
/// file another person's saves under its own id.
pub const USER_ROOT_MARKER: &str = ".drop-user";

/// What to do with an emulator's legacy `drop-saves/{gameId}` directories.
#[derive(Debug, PartialEq, Eq)]
pub enum ClaimVerdict {
    /// Nobody has claimed this tree, or we already did: the legacy directories
    /// are ours to move (and to read from until they have moved).
    Adopt,
    /// Someone else claimed it, or the claim names nobody at all. Leave every
    /// legacy byte exactly where it is.
    Isolate,
}

/// Decide what a claim file's contents mean for `user_id`.
///
/// A present-but-blank claim is [`ClaimVerdict::Isolate`], not Adopt. A blank
/// claim means the file exists and names nobody, and the one thing we know for
/// certain is that *somebody* ran a migration here. Adopting on that basis is
/// how account B ends up owning account A's saves, which is the single failure
/// this file exists to prevent. [`write_claim`] makes sure Drop can never
/// produce a blank one itself; deleting the file by hand re-opens adoption.
pub fn claim_verdict(existing_owner: Option<&str>, user_id: &str) -> ClaimVerdict {
    match existing_owner.map(str::trim) {
        None => ClaimVerdict::Adopt,
        Some(owner) if owner == user_id => ClaimVerdict::Adopt,
        Some(_) => ClaimVerdict::Isolate,
    }
}

/// Read `{drop_saves}/.drop-owner`, or `None` when there is no claim file.
pub fn read_claim(drop_saves: &Path) -> Option<String> {
    fs::read_to_string(drop_saves.join(OWNER_CLAIM_FILE)).ok()
}

/// Write the owner claim durably: full contents to a temp file, flushed to the
/// platter, then renamed over the real name.
///
/// A plain `fs::write` is not good enough here. NTFS journals metadata but not
/// file data, so a power loss right after the claim is created leaves a
/// zero-length `.drop-owner` on disk with the legacy directories still
/// unmoved. The rename is atomic and the bytes are already on disk before it
/// runs, so the file is either absent or complete.
pub fn write_claim(drop_saves: &Path, user_id: &str) -> Result<(), String> {
    let final_path = drop_saves.join(OWNER_CLAIM_FILE);
    let tmp_path = drop_saves.join(format!("{OWNER_CLAIM_FILE}.tmp"));
    let mut file = fs::File::create(&tmp_path)
        .map_err(|e| format!("could not create {}: {e}", tmp_path.display()))?;
    file.write_all(user_id.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("could not write {}: {e}", tmp_path.display()))?;
    drop(file);
    fs::rename(&tmp_path, &final_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("could not write {}: {e}", final_path.display())
    })
}

/// Create `{emu_root}/drop-saves/{user_id}` and mark it as a user root.
///
/// Called from the migration *and* from the RetroArch writer. The marker used
/// to be written only by the migration, so an account that first signed in
/// after the migration had already finished got an unmarked root — and the
/// next layout bump would have swept that person's saves into whichever
/// account ran the sweep.
pub fn ensure_user_root(emu_root: &Path, user_id: &str) -> Result<PathBuf, String> {
    let dest = emu_root.join(DROP_SAVES_DIR).join(user_id);
    fs::create_dir_all(&dest)
        .map_err(|e| format!("could not create {}: {e}", dest.display()))?;
    let marker = dest.join(USER_ROOT_MARKER);
    if !marker.exists()
        && let Err(e) = fs::write(&marker, "")
    {
        return Err(format!("could not write {}: {e}", marker.display()));
    }
    Ok(dest)
}

/// Whether `path` is a per-user root rather than a legacy game directory.
pub fn is_user_root(path: &Path) -> bool {
    path.join(USER_ROOT_MARKER).exists()
}

/// The directory a game's emulator saves are in *right now*, which is not
/// always the directory they are supposed to end up in.
///
/// Every producer and consumer of an emulator save path goes through here so
/// they cannot disagree: the RetroArch config writer, the scanner, the
/// restorer, the tombstone deleter and both halves of the panel's discovery.
///
/// Normally this is just [`emu_saves_root`]. The exception is a
/// `drop-saves/{game_id}` directory the one-time move never got to — it
/// errored on a locked file, nobody was signed in when it ran, or the
/// emulator was uninstalled (or its drive offline) at the time, which takes it
/// out of the set of directories the migration can even see. Pointing the
/// emulator at the per-user path while the real save sits there would boot the
/// game from a blank save and then write a second divergent copy next to it.
///
/// The answer is taken from the disk every time rather than from a
/// "migration finished" flag. The flag was wrong in exactly the case that
/// costs a save: a tree the migration never scanned cannot make the migration
/// report failure, so the flag flipped to done and retired the fallback while
/// the bytes were still sitting in the legacy directory. What is on disk
/// cannot lie about that. It costs one `exists()` on the normal path.
///
/// The fallback is gated on the claim file, so an account that was isolated
/// out of the legacy tree never reads it.
pub fn resolve_emu_saves_root(
    emu_root: &Path,
    user_id: Option<&str>,
    game_id: &str,
) -> PathBuf {
    let scoped = emu_saves_root(emu_root, user_id, game_id);
    let Some(user_id) = user_id else {
        return scoped;
    };
    // A per-user directory that already exists is where this game's saves
    // live; the legacy tree is only interesting while nothing has been written
    // under the user root yet.
    if scoped.exists() {
        return scoped;
    }

    let legacy = emu_saves_root(emu_root, None, game_id);
    if !legacy.is_dir() || is_user_root(&legacy) {
        return scoped;
    }
    let drop_saves = emu_root.join(DROP_SAVES_DIR);
    if claim_verdict(read_claim(&drop_saves).as_deref(), user_id) == ClaimVerdict::Isolate {
        return scoped;
    }

    warn!(
        "[SAVE-SCOPE] Using the un-migrated save directory {} for game {game_id}: these save \
         bytes have not been moved into a per-user directory on this device yet",
        legacy.display()
    );
    legacy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("drop-save-scope-mod-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_blank_claim_is_never_adopted() {
        assert_eq!(claim_verdict(None, "user-a"), ClaimVerdict::Adopt);
        assert_eq!(claim_verdict(Some("user-a"), "user-a"), ClaimVerdict::Adopt);
        assert_eq!(
            claim_verdict(Some("user-a\n"), "user-a"),
            ClaimVerdict::Adopt
        );
        assert_eq!(
            claim_verdict(Some("user-b"), "user-a"),
            ClaimVerdict::Isolate
        );
        // A zero-length claim means somebody ran a migration here and we
        // cannot tell who. Adopting would hand us their saves.
        assert_eq!(claim_verdict(Some(""), "user-a"), ClaimVerdict::Isolate);
        assert_eq!(claim_verdict(Some("  \n"), "user-a"), ClaimVerdict::Isolate);
    }

    #[test]
    fn a_claim_is_written_whole_and_leaves_no_temp_file() {
        let drop_saves = tmpdir("claim");
        write_claim(&drop_saves, "user-a").unwrap();
        assert_eq!(read_claim(&drop_saves).as_deref(), Some("user-a"));
        assert!(!drop_saves.join(format!("{OWNER_CLAIM_FILE}.tmp")).exists());

        // Overwriting an existing claim replaces it rather than appending.
        write_claim(&drop_saves, "user-b").unwrap();
        assert_eq!(read_claim(&drop_saves).as_deref(), Some("user-b"));
    }

    #[test]
    fn an_unmigrated_legacy_tree_is_used_until_its_bytes_actually_move() {
        let emu_root = tmpdir("resolve");
        let legacy = emu_root.join(DROP_SAVES_DIR).join("game-1");
        fs::create_dir_all(legacy.join("saves")).unwrap();
        let scoped = emu_saves_root(&emu_root, Some("user-a"), "game-1");

        assert_eq!(
            resolve_emu_saves_root(&emu_root, Some("user-a"), "game-1"),
            legacy,
            "the writer must not point at an empty per-user directory while the real save \
             is still in the legacy tree"
        );

        // Somebody else owns the tree: stay out of it entirely.
        write_claim(&emu_root.join(DROP_SAVES_DIR), "user-b").unwrap();
        assert_eq!(
            resolve_emu_saves_root(&emu_root, Some("user-a"), "game-1"),
            scoped
        );

        // The owner keeps reading the legacy directory right up until the
        // bytes are somewhere else. Nothing but the disk gets a vote: a tree
        // the migration never scanned (an uninstalled emulator, a drive that
        // was offline) used to be retired by the version bump alone, and the
        // game then booted from a blank save.
        write_claim(&emu_root.join(DROP_SAVES_DIR), "user-a").unwrap();
        assert_eq!(
            resolve_emu_saves_root(&emu_root, Some("user-a"), "game-1"),
            legacy
        );

        fs::create_dir_all(scoped.join("saves")).unwrap();
        assert_eq!(
            resolve_emu_saves_root(&emu_root, Some("user-a"), "game-1"),
            scoped,
            "once the per-user directory exists it wins, even with legacy debris beside it"
        );
    }

    #[test]
    fn a_sibling_user_root_is_never_mistaken_for_a_legacy_game_directory() {
        let emu_root = tmpdir("sibling");
        ensure_user_root(&emu_root, "user-b").unwrap();

        // "user-b" as a game id is absurd, but the marker is what makes the
        // answer safe rather than lucky.
        assert_eq!(
            resolve_emu_saves_root(&emu_root, Some("user-a"), "user-b"),
            emu_saves_root(&emu_root, Some("user-a"), "user-b")
        );
        assert!(is_user_root(&emu_root.join(DROP_SAVES_DIR).join("user-b")));
    }
}
