//! Backing up and replacing local save files without a window where the only
//! copy can be lost.
//!
//! Every destructive local write in save-sync goes through here. Two rules,
//! both learned the hard way:
//!
//! 1. **The backup is checked.** The old code did `let _ = fs::copy(...)` and
//!    then overwrote (or unlinked) the original regardless, so a failed copy
//!    was followed by the destruction it was supposed to insure against.
//!    [`backup_existing`] returns the error and the caller must not proceed.
//! 2. **The backup name is unique.** A fixed `<file>.bak` slot is one save
//!    deep: two restores and the original is gone, because the second backup
//!    overwrites the first. Names carry a unix timestamp (the same shape
//!    [`super::manifest::backup_corrupt_manifest`] uses) plus a counter for
//!    the same-second case.
//!
//! Replacements land via a temp file + rename so a crash or a full disk
//! cannot leave a half-written save on top of a good one.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How many backups of one file we will distinguish inside a single second.
/// Two restores in one second is already unusual; a thousand means something
/// is looping, and filling the disk with copies is worse than failing.
const MAX_SAME_SECOND_BACKUPS: u32 = 1000;

/// Suffix of the sibling temp file [`write_atomic`] renames into place.
/// Carries the pid so two Drop processes cannot collide on it.
fn temp_suffix() -> String {
    format!(".drop-tmp.{}", std::process::id())
}

/// Build the backup path for `path` at `unix_secs`, with `dedupe`
/// distinguishing backups taken in the same second (`0` = no suffix).
///
/// The stamp is appended to the whole filename rather than swapped into the
/// extension. `Path::with_extension` mangles the extension-less files that
/// matter most here — Switch `data` became `data..bak`, and `pcLastUsedProfile`
/// became `pcLastUsedProfile..bak` — and it also eats a real extension, so
/// `save.dat` and `save.sav` would back up to the same name.
pub fn backup_name(path: &Path, unix_secs: u64, dedupe: u32) -> PathBuf {
    let mut name: OsString = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    if dedupe == 0 {
        name.push(format!(".bak.{unix_secs}"));
    } else {
        name.push(format!(".bak.{unix_secs}-{dedupe}"));
    }
    path.with_file_name(name)
}

/// Returns `true` if `name` is a file Drop itself wrote alongside a save:
/// a backup (`<file>.bak` from the old fixed slot, or `<file>.bak.<secs>`
/// with the optional `-<n>` counter) or a leftover atomic-write temp file.
///
/// Save scans must skip these. A backup that gets scanned is uploaded as a
/// brand-new cloud save, downloaded onto every other device, and backed up
/// again there as `.bak.bak` on the next restore.
pub fn is_backup_artifact(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".bak") {
        return true;
    }
    if lower.contains(".drop-tmp.") {
        return true;
    }
    let Some(idx) = lower.rfind(".bak.") else {
        return false;
    };
    let tail = &lower[idx + ".bak.".len()..];
    !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit() || c == '-')
}

/// Copy `path` aside before it is overwritten or unlinked.
///
/// `Ok(None)` means there was nothing to back up. An `Err` means the original
/// is still intact and **must be left that way** — do not fall through to the
/// destructive operation.
pub fn backup_existing(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    for dedupe in 0..MAX_SAME_SECOND_BACKUPS {
        let candidate = backup_name(path, ts, dedupe);
        if candidate.exists() {
            continue;
        }
        fs::copy(path, &candidate).map_err(|e| {
            format!(
                "Failed to back up {} to {}: {e}",
                path.display(),
                candidate.display()
            )
        })?;
        return Ok(Some(candidate));
    }
    Err(format!(
        "Refusing to overwrite {}: {MAX_SAME_SECOND_BACKUPS} backups of it already exist for this second",
        path.display()
    ))
}

/// Write `data` to `path` via a sibling temp file and a rename.
///
/// A rename replaces the destination on both Windows and Unix, so a reader
/// sees either the old bytes or the new ones — never a truncated save,
/// which is what a direct `fs::write` leaves behind if the disk fills or the
/// process dies mid-write.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!(
            "Cannot write save to {}: it has no parent directory",
            path.display()
        ));
    };
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create save dir {}: {e}", parent.display()))?;
    }

    let mut tmp_name: OsString = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    tmp_name.push(temp_suffix());
    let tmp = path.with_file_name(tmp_name);

    fs::write(&tmp, data).map_err(|e| format!("Failed to write {}: {e}", tmp.display()))?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Failed to replace {}: {e}", path.display()));
    }
    Ok(())
}

/// Back up whatever is at `path`, then atomically put `data` there.
/// The single entry point for "overwrite a local save with different bytes".
pub fn replace_save_file(path: &Path, data: &[u8]) -> Result<(), String> {
    backup_existing(path)?;
    write_atomic(path, data)
}

/// Back up whatever is at `path`, then unlink it. `Ok(false)` means there was
/// no file there to begin with.
pub fn remove_save_file(path: &Path) -> Result<bool, String> {
    if backup_existing(path)?.is_none() {
        return Ok(false);
    }
    fs::remove_file(path).map_err(|e| format!("Failed to delete {}: {e}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "drop-save-backup-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn backup_name_keeps_the_whole_filename() {
        let p = Path::new("/games/saves/gen.sav");
        assert_eq!(
            backup_name(p, 1700000000, 0),
            Path::new("/games/saves/gen.sav.bak.1700000000")
        );
    }

    #[test]
    fn backup_name_handles_extensionless_files() {
        // Switch NAND saves are literally called `data`; the old
        // `with_extension(".bak")` produced `data..bak` for these.
        for name in ["data", "pcLastUsedProfile"] {
            let p = Path::new("/nand").join(name);
            let bak = backup_name(&p, 42, 0);
            assert_eq!(
                bak.file_name().unwrap().to_str().unwrap(),
                format!("{name}.bak.42")
            );
        }
    }

    #[test]
    fn backup_name_disambiguates_within_one_second() {
        let p = Path::new("/s/a.srm");
        assert_eq!(
            backup_name(p, 7, 1),
            Path::new("/s/a.srm.bak.7-1")
        );
        assert_ne!(backup_name(p, 7, 0), backup_name(p, 7, 1));
    }

    #[test]
    fn backup_name_does_not_collide_across_extensions() {
        // `with_extension` collapsed these two onto one name.
        let a = backup_name(Path::new("/s/save.dat"), 9, 0);
        let b = backup_name(Path::new("/s/save.sav"), 9, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn backup_artifacts_are_recognised() {
        for name in [
            "gen.sav.bak",
            "gen.sav.BAK",
            "gen.sav.bak.1700000000",
            "gen.sav.bak.1700000000-3",
            "data.bak.42",
            "gen.sav.drop-tmp.9182",
        ] {
            assert!(is_backup_artifact(name), "{name} should be denylisted");
        }
    }

    #[test]
    fn real_saves_are_not_mistaken_for_backups() {
        for name in [
            "gen.sav",
            "data",
            "Backup Quest.srm",
            "bakery.sav",
            "save.bak.notatimestamp",
        ] {
            assert!(!is_backup_artifact(name), "{name} should be kept");
        }
    }

    #[test]
    fn backup_then_replace_never_clobbers_an_earlier_backup() {
        let dir = tmpdir("rotate");
        let save = dir.join("gen.sav");
        fs::write(&save, b"v1").unwrap();

        replace_save_file(&save, b"v2").unwrap();
        replace_save_file(&save, b"v3").unwrap();

        assert_eq!(fs::read(&save).unwrap(), b"v3");
        let mut backed_up: Vec<Vec<u8>> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| is_backup_artifact(&e.file_name().to_string_lossy()))
            .map(|e| fs::read(e.path()).unwrap())
            .collect();
        backed_up.sort();
        // Both previous versions survive — the old fixed `.bak` slot kept
        // only "v2" here, and the original "v1" was unrecoverable.
        assert_eq!(backed_up, vec![b"v1".to_vec(), b"v2".to_vec()]);
    }

    #[test]
    fn remove_save_file_reports_a_missing_file() {
        let dir = tmpdir("remove");
        assert!(!remove_save_file(&dir.join("nope.sav")).unwrap());

        let save = dir.join("bye.sav");
        fs::write(&save, b"bytes").unwrap();
        assert!(remove_save_file(&save).unwrap());
        assert!(!save.exists());
        assert!(backup_name(&save, 0, 0).parent().unwrap().exists());
    }

    #[test]
    fn write_atomic_leaves_no_temp_file_behind() {
        let dir = tmpdir("atomic");
        let save = dir.join("new.sav");
        write_atomic(&save, b"hello").unwrap();
        assert_eq!(fs::read(&save).unwrap(), b"hello");
        let leftovers: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".drop-tmp."))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
