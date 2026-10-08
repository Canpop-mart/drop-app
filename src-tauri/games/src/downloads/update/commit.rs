//! Swapping a staged update into an install, crash-safely.
//!
//! Layout inside the install folder (so every move is a same-filesystem
//! rename):
//!
//! ```text
//! .drop-update/new/<path>     staged new files, as the server spells them
//! .drop-update/old/<n>        originals moved out of the way, by op number
//! .drop-update/journal.json   what the commit is doing
//! .drop-removed/<time>/<path> originals from mirrored folders, kept after
//!                             the commit (see `Original::recover_to`)
//! ```
//!
//! The commit writes the journal (phase `moving`), then per operation moves
//! the original (if any) to `old/` and then the staged file into place. Every
//! step is a rename, so after a crash each operation is in one of three
//! states that the filesystem itself tells apart, with the staged file's
//! recorded size and mtime as the proof that a file at the target is the one
//! the commit moved in (see [`rollback`]). Once all moves succeed the journal
//! is rewritten with phase `committed`; from then on the update is applied
//! and recovery rolls it forward instead of back (the mod hand-over, the
//! install record and the clean-up all happen after that point).
//!
//! Everything that moves or deletes inside a staging folder holds
//! [`lock_staging`].
//!
//! Durability: the journal is fsynced (file and folder) before anything moves
//! and again at `committed`. The moves themselves are not fsynced one by one;
//! a power cut can lose a rename that the journal does not yet record as
//! committed, which rollback handles the same way as a crash.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use log::{info, warn};
use serde::{Deserialize, Serialize};
use utils::path_guard;

use super::baseline::write_file_atomic;
use super::{RECOVERY_DIR, UPDATE_DIR};

pub const JOURNAL_FILE: &str = "journal.json";
pub const NEW_DIR: &str = "new";
pub const OLD_DIR: &str = "old";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Moving,
    Committed,
}

/// A file already in the install that an operation moves out of the way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Original {
    /// Install-relative, as found on disk.
    pub disk_path: String,
    /// Name under `.drop-update/old/`.
    pub slot: String,
    /// Keep it as `<disk_path>.bak` once committed, instead of deleting it.
    pub keep_as_bak: bool,
    /// Keep it here instead (install-relative, under `RECOVERY_DIR`), once
    /// committed: a file from a mirrored folder. Like every original it sits
    /// in `old/` until then, so a rollback puts it back. Falls back to `.bak`
    /// when the recovery folder can't be used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recover_to: Option<String>,
}

/// The recovery folder's name for an update made at `now`: the UTC time as
/// `YYYYMMDDTHHMMSSZ` (no `:`, which Windows does not allow in a name).
pub fn recovery_stamp(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

/// Where a file at `disk_path` goes in the recovery folder of the update
/// stamped `stamp` (install-relative, `/`-separated).
pub fn recovery_path(stamp: &str, disk_path: &str) -> String {
    format!("{RECOVERY_DIR}/{stamp}/{}", disk_path.replace('\\', "/"))
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalOp {
    /// Install-relative destination of the staged file (also its path under
    /// `new/`). For a pure delete, the deleted path.
    pub target: String,
    /// A staged file moves to `target`. False for a delete.
    pub staged: bool,
    pub original: Option<Original>,
    /// Size and mtime of the staged file, taken before the first move. A
    /// rename keeps both, so rollback moves a file back out of the install
    /// only when it is this one, never an original the commit didn't reach.
    #[serde(default)]
    pub staged_size: Option<u64>,
    #[serde(default)]
    pub staged_mtime: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Journal {
    pub game_id: String,
    pub from_version_id: String,
    pub to_version_id: String,
    pub to_revision: u32,
    pub phase: Phase,
    pub ops: Vec<JournalOp>,
    /// Folders the commit created, shallowest first. Rollback removes the
    /// ones still empty, deepest first.
    #[serde(default)]
    pub created_dirs: Vec<String>,
    /// The target `GameVersion` (JSON), so a roll-forward at startup can move
    /// the install record without the network.
    #[serde(default)]
    pub game_version: Option<serde_json::Value>,
    /// Files the update wrote (the mod hand-over runs on them after the
    /// commit point, again on a roll-forward).
    #[serde(default)]
    pub handover: Vec<String>,
    /// Baseline files the target dropped (every mod's backup of them is
    /// discarded after the commit point).
    #[serde(default)]
    pub dropped: Vec<String>,
}

static STAGING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Held by everything that moves or deletes inside a `.drop-update` folder:
/// the commit (through `finish`), rollback and recovery, and discarding a
/// staging folder. A cancel can then never delete a staging folder while a
/// commit is moving the player's files through it.
pub fn lock_staging() -> std::sync::MutexGuard<'static, ()> {
    STAGING.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn update_dir(install: &Path) -> PathBuf {
    install.join(UPDATE_DIR)
}

pub fn new_dir(install: &Path) -> PathBuf {
    update_dir(install).join(NEW_DIR)
}

pub fn old_dir(install: &Path) -> PathBuf {
    update_dir(install).join(OLD_DIR)
}

pub fn journal_path(install: &Path) -> PathBuf {
    update_dir(install).join(JOURNAL_FILE)
}

/// Whether an update commit is in progress or was interrupted here.
pub fn journal_present(install: &Path) -> bool {
    std::fs::symlink_metadata(journal_path(install)).is_ok()
}

pub fn read_journal(install: &Path) -> io::Result<Option<Journal>> {
    let bytes = match std::fs::read(journal_path(install)) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_journal(install: &Path, journal: &Journal) -> io::Result<()> {
    std::fs::create_dir_all(update_dir(install))?;
    let bytes = serde_json::to_vec_pretty(journal).map_err(io::Error::other)?;
    write_file_atomic(&journal_path(install), &bytes)
}

fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn join(base: &Path, rel: &str) -> Result<PathBuf, String> {
    path_guard::join_within(base, Path::new(rel)).map_err(|e| format!("unsafe path {rel:?}: {e}"))
}

#[derive(Debug)]
pub enum CommitError {
    /// Nothing was changed (failed before the first move).
    NotStarted(String),
    /// A move failed and everything was put back.
    RolledBack(String),
    /// A move failed AND putting things back failed too. The journal is left
    /// for recovery to try again; launching is blocked until it succeeds.
    RollbackFailed { cause: String, rollback: String },
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitError::NotStarted(why) | CommitError::RolledBack(why) => write!(f, "{why}"),
            CommitError::RollbackFailed { cause, rollback } => {
                write!(f, "{cause}; putting the old files back also failed: {rollback}")
            }
        }
    }
}

/// Folders that must be created for the staged files, shallowest first.
///
/// A folder whose path is a FILE now (an update that turns `data` into a
/// folder) counts as missing when an earlier op moves that file away; the
/// planner always removes it first. Anything else in the way is an error
/// before anything moves.
fn missing_dirs(install: &Path, journal: &Journal) -> Result<Vec<String>, String> {
    let moved_away: HashSet<String> = journal
        .ops
        .iter()
        .filter_map(|o| o.original.as_ref())
        .map(|o| o.disk_path.replace('\\', "/"))
        .collect();
    let mut out: Vec<String> = Vec::new();
    for op in journal.ops.iter().filter(|o| o.staged) {
        let normalised = path_guard::normalize_relative(Path::new(&op.target))
            .map_err(|e| format!("unsafe path {:?}: {e}", op.target))?;
        let mut chain: Vec<String> = Vec::new();
        let mut dir = normalised.parent();
        while let Some(d) = dir {
            if d.as_os_str().is_empty() {
                break;
            }
            let rel = d.to_string_lossy().replace('\\', "/");
            match std::fs::symlink_metadata(install.join(d)) {
                Ok(m) if m.is_dir() => break,
                Ok(_) if moved_away.contains(&rel) => chain.push(rel),
                Ok(_) => return Err(format!("{rel} is in the way of {}", op.target)),
                Err(_) => chain.push(rel),
            }
            dir = d.parent();
        }
        for d in chain.into_iter().rev() {
            if !out.contains(&d) {
                out.push(d);
            }
        }
    }
    Ok(out)
}

/// Check every path the commit touches stays inside the install, through
/// symlinks and junctions too, and record each staged file's stamp.
fn preflight(install: &Path, journal: &mut Journal) -> Result<(), String> {
    let base_real = install
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", install.display()))?;
    let new = new_dir(install);
    for op in &mut journal.ops {
        let target = join(install, &op.target)?;
        path_guard::ensure_parent_within(&base_real, &target)
            .map_err(|_| format!("{} leads outside the install folder", op.target))?;
        if op.staged {
            let staged = join(&new, &op.target)?;
            match std::fs::symlink_metadata(&staged) {
                Ok(m) if m.is_file() => {
                    op.staged_size = Some(m.len());
                    op.staged_mtime = super::baseline::mtime_nanos(&m);
                }
                _ => return Err(format!("the downloaded copy of {} is missing", op.target)),
            }
            if op.original.is_none() && exists(&target) {
                return Err(format!("{} appeared since the update was planned", op.target));
            }
        }
        if let Some(orig) = &op.original {
            let disk = join(install, &orig.disk_path)?;
            path_guard::ensure_parent_within(&base_real, &disk)
                .map_err(|_| format!("{} leads outside the install folder", orig.disk_path))?;
            if !exists(&disk) {
                return Err(format!("{} disappeared since the update was planned", orig.disk_path));
            }
        }
    }
    Ok(())
}

/// Create the listed folders `target` sits in that are not there yet (some
/// only become creatable once an earlier op moved a file of the same name).
fn make_parents(install: &Path, journal: &Journal, target: &str) -> Result<(), String> {
    let target_norm = target.replace('\\', "/");
    for d in &journal.created_dirs {
        if !target_norm.starts_with(&format!("{d}/")) {
            continue;
        }
        let p = join(install, d)?;
        if std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        std::fs::create_dir(&p).map_err(|e| format!("could not create {d}: {e}"))?;
    }
    Ok(())
}

fn do_moves(install: &Path, journal: &Journal) -> Result<(), String> {
    let new = new_dir(install);
    let old = old_dir(install);
    std::fs::create_dir_all(&old).map_err(|e| format!("could not create {}: {e}", old.display()))?;
    for op in &journal.ops {
        if let Some(orig) = &op.original {
            let from = join(install, &orig.disk_path)?;
            let to = join(&old, &orig.slot)?;
            std::fs::rename(&from, &to)
                .map_err(|e| format!("could not move {} aside: {e}", orig.disk_path))?;
        }
        if op.staged {
            let to = join(install, &op.target)?;
            // rename() replaces an existing file silently on every platform.
            // Nothing may be there now: the original (if any) was just moved.
            if exists(&to) {
                return Err(format!("{} appeared during the update", op.target));
            }
            make_parents(install, journal, &op.target)?;
            let from = join(&new, &op.target)?;
            std::fs::rename(&from, &to).map_err(|e| format!("could not move in {}: {e}", op.target))?;
        }
    }
    Ok(())
}

/// Apply `journal` (its phase is ignored and written as `moving`, then
/// `committed`). Hold [`lock_staging`].
///
/// On `Ok` the update is committed: the caller runs the mod hand-over, moves
/// the install record and calls [`finish`]. The journal stays until `finish`
/// so a crash in between is rolled forward. On any failure before the commit
/// point everything is put back.
pub fn commit(install: &Path, mut journal: Journal) -> Result<Journal, CommitError> {
    preflight(install, &mut journal).map_err(CommitError::NotStarted)?;
    journal.created_dirs = missing_dirs(install, &journal).map_err(CommitError::NotStarted)?;
    journal.phase = Phase::Moving;
    write_journal(install, &journal)
        .map_err(|e| CommitError::NotStarted(format!("could not write the update journal: {e}")))?;

    let result = do_moves(install, &journal).and_then(|()| {
        journal.phase = Phase::Committed;
        write_journal(install, &journal).map_err(|e| format!("could not record the update as done: {e}"))
    });
    match result {
        Ok(()) => {
            info!(
                "update of {} to {} rev {} committed in {}",
                journal.game_id,
                journal.to_version_id,
                journal.to_revision,
                install.display()
            );
            Ok(journal)
        }
        Err(cause) => {
            warn!("update commit in {} failed: {cause}; rolling back", install.display());
            journal.phase = Phase::Moving;
            match rollback(install, &journal) {
                Ok(()) => Err(CommitError::RolledBack(cause)),
                Err(rollback) => Err(CommitError::RollbackFailed { cause, rollback }),
            }
        }
    }
}

/// Whether `path` is the staged file `op` moved in (same size and mtime;
/// a rename keeps both). Journals without a stamp can't tell, and say yes.
fn is_moved_in(path: &Path, op: &JournalOp) -> bool {
    let Ok(m) = std::fs::symlink_metadata(path) else {
        return false;
    };
    match (op.staged_size, op.staged_mtime) {
        (Some(size), mtime) => {
            m.is_file() && m.len() == size && (mtime.is_none() || super::baseline::mtime_nanos(&m) == mtime)
        }
        (None, _) => true,
    }
}

/// Put an interrupted `moving` commit back exactly as it was. Hold
/// [`lock_staging`]. Works from whatever state a crash left, using only what
/// is on disk, per op in reverse:
///
/// - the staged file was moved in when: its original (if any) is in `old/`
///   (originals move first), its staged copy is gone from `new/`, and what
///   is at the target is that file (same size and mtime). Then it moves back
///   to `new/`, so a retry needs no re-download. Anything else at the target
///   is never moved: it is the original, or not ours;
/// - the original is in `old/` => it was moved aside: move it back (removing
///   an empty folder the commit created in its place first).
///
/// Created folders that are empty again are removed. The journal is deleted
/// only when everything was put back; otherwise it stays for the next try.
pub fn rollback(install: &Path, journal: &Journal) -> Result<(), String> {
    let new = new_dir(install);
    let old = old_dir(install);
    let created: HashSet<&str> = journal.created_dirs.iter().map(String::as_str).collect();
    let mut failures: Vec<String> = Vec::new();
    for op in journal.ops.iter().rev() {
        let Ok(target) = join(install, &op.target) else {
            continue;
        };
        let original_moved = op
            .original
            .as_ref()
            .is_none_or(|o| join(&old, &o.slot).is_ok_and(|slot| exists(&slot)));
        if op.staged {
            let staged = match join(&new, &op.target) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(e);
                    continue;
                }
            };
            if !exists(&staged) && exists(&target) {
                if original_moved && is_moved_in(&target, op) {
                    let moved = staged
                        .parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|()| std::fs::rename(&target, &staged));
                    if let Err(e) = moved {
                        failures.push(format!("{}: {e}", op.target));
                        continue;
                    }
                } else {
                    warn!(
                        "rollback: the downloaded copy of {} is gone; leaving what is at {} alone",
                        op.target,
                        target.display()
                    );
                }
            }
        }
        if let Some(orig) = &op.original {
            let (Ok(slot), Ok(disk)) = (join(&old, &orig.slot), join(install, &orig.disk_path)) else {
                continue;
            };
            if exists(&slot) {
                let rel = orig.disk_path.replace('\\', "/");
                if exists(&disk) && created.contains(rel.as_str()) {
                    // A folder the commit made where this file was; empty
                    // again once the files moved into it went back.
                    let _ = std::fs::remove_dir(&disk);
                }
                if exists(&disk) {
                    failures.push(format!(
                        "{}: something else is there now, the original is kept in {}",
                        orig.disk_path,
                        slot.display()
                    ));
                    continue;
                }
                if let Err(e) = std::fs::rename(&slot, &disk) {
                    failures.push(format!("{}: {e}", orig.disk_path));
                }
            }
        }
    }
    for d in journal.created_dirs.iter().rev() {
        if let Ok(p) = join(install, d) {
            // Only removes an empty folder; one that holds anything stays.
            let _ = std::fs::remove_dir(&p);
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    std::fs::remove_file(journal_path(install))
        .or_else(|e| if e.kind() == io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
        .map_err(|e| format!("everything was put back but the journal could not be removed: {e}"))?;
    info!("rolled back an unfinished update in {}", install.display());
    Ok(())
}

/// Move an original from its `old/` slot to `dest` (install-relative, under
/// the recovery folder), creating the folders on the way. Refuses when the
/// recovery folder is not a real folder, or the destination would leave the
/// install through a link. An existing file at `dest` is never replaced:
/// `<dest>.1`, `<dest>.2`, ... are used instead.
fn move_to_recovery(install: &Path, slot: &Path, dest: &str) -> io::Result<PathBuf> {
    let root = install.join(RECOVERY_DIR);
    if std::fs::symlink_metadata(&root).is_ok_and(|m| !m.is_dir()) {
        return Err(io::Error::other(format!("{RECOVERY_DIR} is not a folder")));
    }
    let to = join(install, dest).map_err(io::Error::other)?;
    if !to.starts_with(&root) {
        return Err(io::Error::other(format!("{dest} is not in {RECOVERY_DIR}")));
    }
    // Checked before creating anything (a link inside the recovery folder
    // must not get folders made outside the install), and again after.
    let base_real = install.canonicalize()?;
    let within = || {
        path_guard::ensure_parent_within(&base_real, &to)
            .map_err(|_| io::Error::other(format!("{dest} leads outside the install folder")))
    };
    within()?;
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    within()?;
    let free = if exists(&to) {
        let name = to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        (1..)
            .map(|n| to.with_file_name(format!("{name}.{n}")))
            .find(|p| !exists(p))
            .unwrap_or(to)
    } else {
        to
    };
    std::fs::rename(slot, &free)?;
    Ok(free)
}

/// The first free `<path>.bak`, `<path>.bak.1`, ... next to `disk`.
pub fn free_bak_path(disk: &Path) -> PathBuf {
    let name = disk.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let first = disk.with_file_name(format!("{name}.bak"));
    if !exists(&first) {
        return first;
    }
    (1..)
        .map(|n| disk.with_file_name(format!("{name}.bak.{n}")))
        .find(|p| !exists(p))
        .unwrap_or(first)
}

/// Whether a folder holds nothing but (empty) folders.
fn only_folders(dir: &Path) -> io::Result<bool> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !std::fs::symlink_metadata(entry.path())?.is_dir() || !only_folders(&entry.path())? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Remove a tree of empty folders, deepest first (never a file).
fn remove_empty_tree(dir: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        remove_empty_tree(&entry?.path())?;
    }
    std::fs::remove_dir(dir)
}

/// Remove an original the update replaced. A FOLDER is only ever removed
/// when it is empty (a folder the update turned into a file, whose files it
/// moved out first): anything that appeared in it since is the player's and
/// is never deleted. Then `Ok(Some(..))` says where it was kept instead.
fn remove_original(slot: &Path, disk: &Path) -> io::Result<Option<PathBuf>> {
    match std::fs::symlink_metadata(slot) {
        Ok(m) if m.is_dir() => {
            if only_folders(slot)? {
                remove_empty_tree(slot)?;
                Ok(None)
            } else {
                let bak = free_bak_path(disk);
                std::fs::rename(slot, &bak)?;
                Ok(Some(bak))
            }
        }
        _ => remove_any(slot).map(|()| None),
    }
}

fn remove_any(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(path),
        Ok(_m) => std::fs::remove_file(path).or_else(|e| {
            // Windows refuses to delete a read-only file.
            #[cfg(windows)]
            if e.kind() == io::ErrorKind::PermissionDenied && _m.permissions().readonly() {
                let mut perms = _m.permissions();
                perms.set_readonly(false);
                if std::fs::set_permissions(path, perms).is_ok() && std::fs::remove_file(path).is_ok() {
                    return Ok(());
                }
            }
            // A directory symlink/junction on Windows needs remove_dir.
            std::fs::remove_dir(path).map_err(|_| e)
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// After a committed update (mod hand-over done, install record moved): move
/// the originals of mirrored folders to the recovery folder, keep the other
/// originals the player asked to keep as `.bak`, delete the rest, then
/// remove the journal and the staging folder. Hold [`lock_staging`]. Safe to
/// run again after a crash. Returns the copies kept (`.bak` files and files
/// in the recovery folder).
pub fn finish(install: &Path, journal: &Journal) -> Result<Vec<PathBuf>, String> {
    let old = old_dir(install);
    let mut baks = Vec::new();
    let mut failures = Vec::new();
    for op in &journal.ops {
        let Some(orig) = &op.original else { continue };
        let (Ok(slot), Ok(disk)) = (join(&old, &orig.slot), join(install, &orig.disk_path)) else {
            continue;
        };
        if !exists(&slot) {
            continue;
        }
        if let Some(dest) = &orig.recover_to {
            match move_to_recovery(install, &slot, dest) {
                Ok(kept) => {
                    baks.push(kept);
                    continue;
                }
                // Kept as .bak instead (below): never lost, never stuck.
                Err(e) => warn!(
                    "could not move {} to {dest} ({e}); keeping it as .bak instead",
                    orig.disk_path
                ),
            }
        }
        if orig.keep_as_bak || orig.recover_to.is_some() {
            let bak = free_bak_path(&disk);
            match std::fs::rename(&slot, &bak) {
                Ok(()) => baks.push(bak),
                Err(e) => failures.push(format!("could not keep your copy of {}: {e}", orig.disk_path)),
            }
        } else {
            match remove_original(&slot, &disk) {
                Ok(None) => {}
                Ok(Some(kept)) => {
                    warn!(
                        "{} held files added after the update was reviewed; kept them in {}",
                        orig.disk_path,
                        kept.display()
                    );
                    baks.push(kept);
                }
                Err(e) => failures.push(format!("could not remove the old {}: {e}", orig.disk_path)),
            }
        }
    }
    if !failures.is_empty() {
        // The journal stays, so the next startup tries again; nothing the
        // player owns is lost meanwhile (it is still under old/).
        return Err(failures.join("; "));
    }
    std::fs::remove_file(journal_path(install))
        .or_else(|e| if e.kind() == io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
        .map_err(|e| format!("could not remove the update journal: {e}"))?;
    discard_staging(install).map_err(|e| format!("could not remove {UPDATE_DIR}: {e}"))?;
    Ok(baks)
}

/// [`discard_staging`] under [`lock_staging`].
pub fn discard_staging_locked(install: &Path) -> io::Result<()> {
    let _staging = lock_staging();
    discard_staging(install)
}

/// Remove the staging folder. Refuses while a journal is there: then the
/// folder may hold the player's original files. Hold [`lock_staging`] (or
/// use [`discard_staging_locked`]).
pub fn discard_staging(install: &Path) -> io::Result<()> {
    if journal_present(install) {
        return Err(io::Error::other("an update journal is present; recover it first"));
    }
    match std::fs::remove_dir_all(update_dir(install)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop-commit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(base: &Path, rel: &str, content: &str) {
        let p = base.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn read(base: &Path, rel: &str) -> Option<String> {
        std::fs::read_to_string(base.join(rel)).ok()
    }

    /// A small install: a replaced file, a new file in a new folder, a
    /// deleted file, a conflict kept as .bak, plus the player's own file.
    fn setup(name: &str) -> (PathBuf, Journal) {
        let install = scratch(name);
        put(&install, "mods/a.jar", "a1");
        put(&install, "mods/old.jar", "old");
        put(&install, "config/pack.toml", "player edit");
        put(&install, "mods/mine.jar", "player mod");
        let new = new_dir(&install);
        put(&new, "mods/a.jar", "a2");
        put(&new, "mods/sub/new.jar", "new");
        put(&new, "config/pack.toml", "pack v2");
        let journal = journal_of(vec![
            op("mods/a.jar", true, Some(("mods/a.jar", "0", false))),
            op("mods/sub/new.jar", true, None),
            op("mods/old.jar", false, Some(("mods/old.jar", "1", false))),
            op("config/pack.toml", true, Some(("config/pack.toml", "2", true))),
        ]);
        (install, journal)
    }

    fn op(target: &str, staged: bool, orig: Option<(&str, &str, bool)>) -> JournalOp {
        JournalOp {
            target: target.into(),
            staged,
            original: orig.map(|(d, s, b)| Original {
                disk_path: d.into(),
                slot: s.into(),
                keep_as_bak: b,
                recover_to: None,
            }),
            ..Default::default()
        }
    }

    fn journal_of(ops: Vec<JournalOp>) -> Journal {
        Journal {
            game_id: "g".into(),
            from_version_id: "v1".into(),
            to_version_id: "v2".into(),
            to_revision: 2,
            phase: Phase::Moving,
            ops,
            created_dirs: vec![],
            game_version: None,
            handover: vec![],
            dropped: vec![],
        }
    }

    fn assert_untouched(install: &Path) {
        assert_eq!(read(install, "mods/a.jar").as_deref(), Some("a1"));
        assert_eq!(read(install, "mods/old.jar").as_deref(), Some("old"));
        assert_eq!(read(install, "config/pack.toml").as_deref(), Some("player edit"));
        assert_eq!(read(install, "mods/mine.jar").as_deref(), Some("player mod"));
        assert!(!install.join("mods/sub").exists(), "created folder left behind");
        assert!(!install.join("config/pack.toml.bak").exists());
    }

    #[test]
    fn a_successful_commit_then_finish_applies_everything() {
        let (install, journal) = setup("ok");
        let journal = commit(&install, journal).unwrap();
        assert_eq!(read_journal(&install).unwrap().unwrap().phase, Phase::Committed);
        let baks = finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "mods/a.jar").as_deref(), Some("a2"));
        assert_eq!(read(&install, "mods/sub/new.jar").as_deref(), Some("new"));
        assert!(!install.join("mods/old.jar").exists());
        assert_eq!(read(&install, "config/pack.toml").as_deref(), Some("pack v2"));
        assert_eq!(read(&install, "config/pack.toml.bak").as_deref(), Some("player edit"));
        assert_eq!(baks, vec![install.join("config/pack.toml.bak")]);
        assert_eq!(read(&install, "mods/mine.jar").as_deref(), Some("player mod"));
        assert!(!update_dir(&install).exists());
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn a_failing_move_rolls_back() {
        let (install, mut journal) = setup("move");
        // The last op's slot is inside a folder that does not exist, so its
        // rename fails after every other op has moved.
        journal.ops.push(op("mods/zz.jar", false, Some(("mods/mine.jar", "3/nested", false))));
        let err = commit(&install, journal).unwrap_err();
        assert!(matches!(err, CommitError::RolledBack(_)), "{err}");
        assert_untouched(&install);
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn preflight_refuses_before_touching_anything() {
        let (install, journal) = setup("preflight");
        put(&install, "mods/sub/new.jar", "appeared");
        let err = commit(&install, journal).unwrap_err();
        assert!(matches!(err, CommitError::NotStarted(_)), "{err}");
        assert!(!journal_present(&install));
        assert_eq!(read(&install, "mods/a.jar").as_deref(), Some("a1"));
        let _ = std::fs::remove_dir_all(&install);
    }

    /// Start a commit by hand and stop after `n` renames, as a crash would:
    /// journal written, moves done in commit order. Returns the journal.
    fn moves_then_crash(install: &Path, mut journal: Journal, n: usize) -> Journal {
        preflight(install, &mut journal).unwrap();
        journal.created_dirs = missing_dirs(install, &journal).unwrap();
        write_journal(install, &journal).unwrap();
        std::fs::create_dir_all(old_dir(install)).unwrap();
        let mut renames = 0;
        for op in &journal.ops {
            if let Some(o) = &op.original {
                if renames == n {
                    return journal;
                }
                std::fs::rename(install.join(&o.disk_path), old_dir(install).join(&o.slot)).unwrap();
                renames += 1;
            }
            if op.staged {
                if renames == n {
                    return journal;
                }
                make_parents(install, &journal, &op.target).unwrap();
                std::fs::rename(new_dir(install).join(&op.target), install.join(&op.target)).unwrap();
                renames += 1;
            }
        }
        journal
    }

    /// Simulate a crash after `n` renames of the commit, then recover.
    fn crash_after(name: &str, n: usize) {
        let (install, journal) = setup(name);
        moves_then_crash(&install, journal, n);
        let on_disk = read_journal(&install).unwrap().unwrap();
        assert_eq!(on_disk.phase, Phase::Moving);
        rollback(&install, &on_disk).unwrap();
        assert_untouched(&install);
        assert!(!journal_present(&install));
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn a_crash_at_any_point_of_the_moves_rolls_back_exactly() {
        // 6 renames in total; n = 6 is "all moved, crashed before the commit
        // point".
        for n in 0..=6 {
            crash_after(&format!("crash{n}"), n);
        }
    }

    #[test]
    fn a_crash_after_the_commit_point_rolls_forward() {
        let (install, journal) = setup("forward");
        commit(&install, journal).unwrap();
        // Crash before finish: recovery reads the journal and finishes.
        let on_disk = read_journal(&install).unwrap().unwrap();
        assert_eq!(on_disk.phase, Phase::Committed);
        finish(&install, &on_disk).unwrap();
        assert_eq!(read(&install, "config/pack.toml.bak").as_deref(), Some("player edit"));
        // Running finish again (a crash during the first) is harmless.
        assert!(finish(&install, &on_disk).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn bak_names_never_overwrite_an_existing_bak() {
        let dir = scratch("bak");
        put(&dir, "a.cfg.bak", "older bak");
        assert_eq!(free_bak_path(&dir.join("a.cfg")), dir.join("a.cfg.bak.1"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn staging_is_not_discarded_while_a_journal_exists() {
        let (install, journal) = setup("discard");
        write_journal(&install, &journal).unwrap();
        assert!(discard_staging(&install).is_err());
        assert!(new_dir(&install).exists());
        let _ = std::fs::remove_dir_all(&install);
    }

    #[cfg(unix)]
    #[test]
    fn a_target_through_a_symlink_out_of_the_install_is_refused() {
        let (install, mut journal) = setup("symlink");
        let outside = scratch("symlink-outside");
        put(&outside, "save.dat", "precious");
        std::os::unix::fs::symlink(&outside, install.join("Saves")).unwrap();
        journal.ops.push(op("Saves/save.dat", false, Some(("Saves/save.dat", "9", false))));
        let err = commit(&install, journal).unwrap_err();
        assert!(matches!(err, CommitError::NotStarted(_)), "{err}");
        assert_eq!(read(&outside, "save.dat").as_deref(), Some("precious"));
        let _ = std::fs::remove_dir_all(&install);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn rollback_never_moves_an_original_whose_staged_copy_vanished() {
        // Crash after the first op (a.jar, 2 renames). Then the staged copy of
        // an op the commit never reached disappears (antivirus, say): its
        // target is still the player's original and must stay put.
        let (install, journal) = setup("vanished");
        moves_then_crash(&install, journal, 2);
        std::fs::remove_file(new_dir(&install).join("config/pack.toml")).unwrap();
        std::fs::remove_file(new_dir(&install).join("mods/sub/new.jar")).unwrap();
        let on_disk = read_journal(&install).unwrap().unwrap();
        rollback(&install, &on_disk).unwrap();
        assert_untouched(&install);
        assert!(!new_dir(&install).join("config/pack.toml").exists(), "original must not move into staging");
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn rollback_never_moves_a_file_that_is_not_the_staged_one() {
        // A pure add whose staged copy vanished, and something else appeared
        // at the target: not ours, left alone.
        let (install, journal) = setup("not-ours");
        let journal = moves_then_crash(&install, journal, 0);
        std::fs::remove_file(new_dir(&install).join("mods/sub/new.jar")).unwrap();
        put(&install, "mods/sub/new.jar", "someone else's file");
        rollback(&install, &journal).unwrap();
        assert_eq!(read(&install, "mods/sub/new.jar").as_deref(), Some("someone else's file"));
        let _ = std::fs::remove_dir_all(&install);
    }

    /// `data` was a file, the update makes it a folder holding `x`.
    fn file_to_folder(name: &str) -> (PathBuf, Journal) {
        let install = scratch(name);
        put(&install, "data", "old file");
        put(&new_dir(&install), "data/x", "x");
        let journal = journal_of(vec![
            op("data", false, Some(("data", "0", true))),
            op("data/x", true, None),
        ]);
        (install, journal)
    }

    #[test]
    fn a_file_becomes_a_folder_and_back_on_rollback() {
        let (install, journal) = file_to_folder("f2d-ok");
        let journal = commit(&install, journal).unwrap();
        assert_eq!(journal.created_dirs, vec!["data".to_string()]);
        finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "data/x").as_deref(), Some("x"));
        assert_eq!(read(&install, "data.bak").as_deref(), Some("old file"));
        let _ = std::fs::remove_dir_all(&install);

        for n in 0..=2 {
            let (install, journal) = file_to_folder(&format!("f2d-crash{n}"));
            let journal = moves_then_crash(&install, journal, n);
            rollback(&install, &journal).unwrap();
            assert_eq!(read(&install, "data").as_deref(), Some("old file"), "crash after {n}");
            assert!(!journal_present(&install));
            let _ = std::fs::remove_dir_all(&install);
        }
    }

    /// `data/x` was a file in a folder, the update makes `data` a file.
    fn folder_to_file(name: &str) -> (PathBuf, Journal) {
        let install = scratch(name);
        put(&install, "data/x", "x");
        put(&new_dir(&install), "data", "now a file");
        let journal = journal_of(vec![
            op("data/x", false, Some(("data/x", "0", false))),
            op("data", true, Some(("data", "1", false))),
        ]);
        (install, journal)
    }

    #[test]
    fn a_folder_becomes_a_file_and_back_on_rollback() {
        let (install, journal) = folder_to_file("d2f-ok");
        let journal = commit(&install, journal).unwrap();
        finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "data").as_deref(), Some("now a file"));
        let _ = std::fs::remove_dir_all(&install);

        for n in 0..=3 {
            let (install, journal) = folder_to_file(&format!("d2f-crash{n}"));
            let journal = moves_then_crash(&install, journal, n);
            rollback(&install, &journal).unwrap();
            assert_eq!(read(&install, "data/x").as_deref(), Some("x"), "crash after {n}");
            let _ = std::fs::remove_dir_all(&install);
        }
    }

    #[test]
    fn discarding_staging_waits_for_a_commit_holding_the_lock() {
        let (install, _) = setup("lock");
        let guard = lock_staging();
        let dir = install.clone();
        let t = std::thread::spawn(move || discard_staging_locked(&dir));
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(new_dir(&install).exists(), "discard ran while the lock was held");
        drop(guard);
        t.join().unwrap().unwrap();
        assert!(!update_dir(&install).exists());
        let _ = std::fs::remove_dir_all(&install);
    }


    #[test]
    fn a_replaced_folder_that_gained_files_is_kept_not_deleted() {
        // The folder turning into a file got a new player file after the
        // review (and past the commit's own check): finish keeps it.
        let (install, journal) = folder_to_file("d2f-late");
        put(&install, "data/late/mine.txt", "player file");
        std::fs::create_dir_all(install.join("data/empty/deeper")).unwrap();
        let journal = commit(&install, journal).unwrap();
        let baks = finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "data").as_deref(), Some("now a file"));
        assert_eq!(baks, vec![install.join("data.bak")]);
        assert_eq!(read(&install, "data.bak/late/mine.txt").as_deref(), Some("player file"));
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn an_emptied_folder_with_only_empty_subfolders_is_removed() {
        let (install, journal) = folder_to_file("d2f-empty");
        std::fs::create_dir_all(install.join("data/empty/deeper")).unwrap();
        let journal = commit(&install, journal).unwrap();
        assert!(finish(&install, &journal).unwrap().is_empty());
        assert!(!install.join("data.bak").exists());
        assert!(!update_dir(&install).exists());
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn the_recovery_folder_is_named_by_the_utc_time() {
        let at = |secs: u64| recovery_stamp(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs));
        assert_eq!(at(0), "19700101T000000Z");
        // 2026-10-08 12:34:56 UTC.
        assert_eq!(at(1_791_462_896), "20261008T123456Z");
        // 2024-02-29 23:59:59 UTC (a leap day).
        assert_eq!(at(1_709_251_199), "20240229T235959Z");
        assert_eq!(
            recovery_path("20261008T123456Z", "user\\mods\\a.jar"),
            ".drop-removed/20261008T123456Z/user/mods/a.jar"
        );
    }

    fn recovering(name: &str) -> (PathBuf, Journal) {
        let install = scratch(name);
        put(&install, "mods/mine.jar", "player mod");
        let mut o = op("mods/mine.jar", false, Some(("mods/mine.jar", "0", true)));
        o.original.as_mut().unwrap().recover_to = Some(recovery_path("20261008T123456Z", "mods/mine.jar"));
        (install, journal_of(vec![o]))
    }

    #[test]
    fn a_recovered_original_goes_to_the_recovery_folder_once_committed() {
        let (install, journal) = recovering("recover-ok");
        // A copy from an earlier update with the same name is never replaced.
        put(&install, ".drop-removed/20261008T123456Z/mods/mine.jar", "older");
        let journal = commit(&install, journal).unwrap();
        // Before the commit point is finished it is still in staging only.
        assert!(!install.join("mods/mine.jar").exists());
        let kept = finish(&install, &journal).unwrap();
        let to = install.join(".drop-removed/20261008T123456Z/mods/mine.jar.1");
        assert_eq!(kept, vec![to.clone()]);
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "player mod");
        assert_eq!(read(&install, ".drop-removed/20261008T123456Z/mods/mine.jar").as_deref(), Some("older"));
        assert!(!install.join("mods/mine.jar.bak").exists());
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn a_recovered_original_is_put_back_on_rollback() {
        let (install, journal) = recovering("recover-crash");
        let journal = moves_then_crash(&install, journal, 1);
        rollback(&install, &journal).unwrap();
        assert_eq!(read(&install, "mods/mine.jar").as_deref(), Some("player mod"));
        assert!(!install.join(".drop-removed").exists());
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn a_recovery_folder_that_is_not_a_folder_falls_back_to_bak() {
        let (install, journal) = recovering("recover-blocked");
        put(&install, ".drop-removed", "a file");
        let journal = commit(&install, journal).unwrap();
        let kept = finish(&install, &journal).unwrap();
        assert_eq!(kept, vec![install.join("mods/mine.jar.bak")]);
        assert_eq!(read(&install, "mods/mine.jar.bak").as_deref(), Some("player mod"));
        assert!(!journal_present(&install));
        let _ = std::fs::remove_dir_all(&install);
    }

    #[cfg(unix)]
    #[test]
    fn no_folder_is_made_outside_the_install_through_a_link_in_the_recovery_folder() {
        let (install, journal) = recovering("recover-inner-link");
        let outside = scratch("recover-inner-link-outside");
        std::fs::create_dir_all(install.join(".drop-removed")).unwrap();
        std::os::unix::fs::symlink(&outside, install.join(".drop-removed/20261008T123456Z")).unwrap();
        let journal = commit(&install, journal).unwrap();
        finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "mods/mine.jar.bak").as_deref(), Some("player mod"));
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none(), "a folder was made outside");
        let _ = std::fs::remove_dir_all(&install);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[cfg(unix)]
    #[test]
    fn a_recovery_folder_linked_out_of_the_install_is_not_used() {
        let (install, journal) = recovering("recover-linked");
        let outside = scratch("recover-linked-outside");
        std::os::unix::fs::symlink(&outside, install.join(".drop-removed")).unwrap();
        let journal = commit(&install, journal).unwrap();
        finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "mods/mine.jar.bak").as_deref(), Some("player mod"));
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(&install);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
