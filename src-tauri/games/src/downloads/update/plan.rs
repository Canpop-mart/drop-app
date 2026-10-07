//! The three-way update planner.
//!
//! Given what the client last installed (the baseline B), the revision being
//! installed (the target T) and what is on disk now (D), decide per file what
//! an in-place update does. Pure apart from the [`DiskView`] it is handed, so
//! every rule is unit-tested against an in-memory disk below.
//!
//! The rules, per path (see UPDATE_SPEC.md "Three-way planner"):
//!
//! | B vs T            | D                         | result                  |
//! |-------------------|---------------------------|-------------------------|
//! | in T, not in B    | missing                   | download                |
//! |                   | same as T                 | nothing                 |
//! |                   | anything else             | conflict `added_exists` |
//! | in both, changed  | same as B                 | replace                 |
//! |                   | same as T                 | nothing                 |
//! |                   | missing                   | download                |
//! |                   | anything else             | conflict `changed_both` |
//! | in both, same     | anything (B verified)     | nothing                 |
//! |                   | missing (B unverified)    | download                |
//! |                   | same as T (B unverified)  | nothing                 |
//! |                   | else (B unverified)       | replace, keep `.bak`    |
//! | in B, not in T    | same as B                 | delete                  |
//! |                   | missing                   | nothing                 |
//! |                   | anything else             | conflict `removed_edited` |
//! | in neither        | anything                  | never looked at         |
//!
//! Overrides that apply before the table:
//! - A path that is Drop runtime data (`is_drop_runtime_data`: GBE's
//!   `drop-goldberg/`, `steam_settings/` and save folders at any depth,
//!   RetroArch's `drop-saves/`, the mod ledgers in `.mods/`) is only ever
//!   written when nothing is on disk there. It is never replaced, deleted or
//!   reported as a conflict: Drop or GBE writes it at runtime whatever the
//!   game shipped.
//! - A path a Drop-managed mod has claimed follows the mod hand-over rules:
//!   when the target ships it new or changed, the base game takes it back
//!   (replace, no conflict; the mod's copy, which may hold player edits, is
//!   kept as `.bak`); when the target drops it, it stays the mod's (not
//!   deleted). Either way it is never a conflict.
//! - A baseline entry this client never confirmed on this disk can't tell
//!   whether the player changed the file or the install simply has an older
//!   copy: an unknown hash (`""`, the server's first snapshot of a folder
//!   edited before hashing), or no mtime (a baseline from the server, used
//!   by installs from before in-place updates, or one recorded without
//!   checking). The owner's rule: the update wins and the file on disk is
//!   kept as `.bak`, without a conflict (see [`Plan::backup_paths`]). Where
//!   the table says "conflict", such an entry gives that instead. Conflicts
//!   are only raised where Drop knows the player changed the file. Files the
//!   player chose "keep mine" for are the exception: their entries have no
//!   mtime on purpose, and they stay the player's.
//! - Except under a generic player-data folder (`is_player_data_folder`:
//!   top-level `user/`, `saves/`, `system/`, `nand/`, ...). Emulators keep
//!   saves, keys and settings there, and packs ship their own files there
//!   too (a Minecraft launcher keeps its whole instance under `user/`). A
//!   file the pack ships there (in B or T) follows the table, but where the
//!   rule above would replace or remove it with a `.bak` without asking:
//!   - a file the update changes or removes is a conflict instead
//!     (`changed_both` / `removed_edited`), so the player decides;
//!   - a file the update does not change stays as it is, silently, as for
//!     runtime data: emulators rewrite the configs they ship
//!     (`qt-config.ini`, Ryujinx `Config.json`), and a question per update
//!     would cost players their bindings to a `.bak`.
//!
//!   A file there that is byte-identical to the baseline is replaced or
//!   removed as usual: it is the pack's own copy. Files in neither list are
//!   never looked at, as anywhere else.
//! - A player-data folder that is a link (or holds one on the way to the
//!   file: on the Deck `user/` or `nand/` is often linked to the SD card)
//!   gets no operations at all: no write, replace or delete through it. The
//!   next baseline keeps the old baseline's entries there (a removed file's
//!   included, a newly added file gets none), so once the link is replaced
//!   by a real folder the next update (the next revision published; Drop
//!   offers none on its own for the skipped files) finds the same work to
//!   do. Folders with files to add, change or remove are listed in
//!   [`Plan::skipped_linked`] for the player. The rest of the
//!   update goes ahead; elsewhere a linked folder still stops the plan
//!   ([`PlanError::Linked`]).
//!
//! Healing what 6.1.0 and 6.1.1 left behind. Those builds never replaced or
//! removed files the pack ships under the player-data folders, and recorded
//! the new hash anyway. `PlanInput::previously_shipped` holds what earlier
//! revisions of the INSTALLED version shipped, up to the one installed; the
//! caller passes it once per install (until an update by this build has
//! run, see `Sidecar::healed_protected_folders`) and passes nothing after.
//! With it, and only under the player-data folders:
//! - Earlier copy: a file the pack ships (in B or T) whose baseline entry is
//!   unverified or absent, and that is byte-identical on disk to what an
//!   earlier revision shipped at that path, counts as the pack's own copy,
//!   like a match with B: replaced or removed without asking and without a
//!   `.bak`. A verified baseline entry is left to the table (the player may
//!   have rolled the file back on purpose).
//! - Leftovers: a path an earlier revision shipped that is in neither B nor
//!   T (in any letter case), on disk as a regular file (not through a link)
//!   byte-identical to what an earlier revision shipped there, is removed
//!   without asking, and kept as `.bak` (it may be the player's own copy of
//!   the same file).
//!
//! Neither applies to Drop runtime data, a mod's file, a file the player
//! chose "keep mine" for (including one the target had removed), an unknown
//! hash, or an empty file (every empty file has the same hash).
//!
//! Last: a file standing where the update needs a folder is
//! removed first (kept as `.bak` unless it is exactly the old shipped file),
//! a folder standing where it needs a file must be emptied by the update
//! itself, and no operation may pass through a symlinked or junctioned
//! folder. Each of those that can't be met stops the plan with an error that
//! says why, before anything is downloaded.
//!
//! Paths are matched with `\` treated as `/`, and case-insensitively only on
//! Windows (`PlanInput::case_insensitive`), where the same server path spelt
//! in two cases is one file. On Linux and the Deck those are two files. When
//! two files in one list differ only by case, those two are matched exactly.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// One file in a server revision snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// One file in the install-local baseline (`.drop-baseline.json`). `mtime`
/// is the file's modification time in nanoseconds since the Unix epoch when
/// the client last knew the file's content was `sha256`; `None` means the
/// file must be hashed to know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub mtime: Option<u64>,
}

impl BaselineFile {
    pub fn from_remote(file: &RemoteFile, mtime: Option<u64>) -> Self {
        Self {
            path: file.path.clone(),
            size: file.size,
            sha256: file.sha256.clone(),
            mtime,
        }
    }
}

/// What is at a path on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskStat {
    /// Nothing there. Also what a path under a FILE reads as (`data/x` when
    /// `data` is a file): nothing can be there until the file goes.
    Missing,
    File { size: u64, mtime: Option<u64> },
    /// A real directory (not a link to one).
    Dir,
    /// A symlink, junction or anything else that is not a regular file or
    /// directory. Never equal to a shipped file.
    Other,
}

/// The planner's only window onto the disk. Paths are install-relative and
/// spelt as the server spells them.
pub trait DiskView {
    fn stat(&self, rel: &str) -> DiskStat;
    fn sha256(&self, rel: &str) -> std::io::Result<String>;
    /// Every entry under the directory `rel`, recursively, as install-relative
    /// `/` paths, without following links (a link is one entry). Directories
    /// themselves are not listed, only what they hold.
    fn files_under(&self, rel: &str) -> std::io::Result<Vec<String>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    AddedExists,
    ChangedBoth,
    RemovedEdited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    TakeUpdate,
    KeepMine,
}

/// What the file on disk must still be when the update is committed. The
/// plan is made before a download that can take an hour; anything that
/// changed in between makes the commit refuse rather than act on stale facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    /// Nothing may be there.
    Missing,
    /// The content the plan relied on. `size`/`mtime` are what the plan saw,
    /// so an untouched file is confirmed without hashing it again.
    Content {
        sha256: String,
        size: u64,
        mtime: Option<u64>,
    },
    /// Something must be there, whatever it is: it is kept as `.bak`, or it
    /// is a mod's file the game takes back.
    Present,
    /// A folder the update replaces with a file. It may hold only these
    /// entries (as `DiskView::files_under` lists them), all of which the
    /// update removes first; anything added since the review stops the
    /// commit.
    Folder { files: Vec<String> },
}

/// A file already on disk that an operation moves out of the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Existing {
    /// Where it is, spelt as the plan found it (may differ in case from the
    /// target spelling).
    pub disk_path: String,
    pub expect: Expect,
}

/// Write a target file (from the download) into the install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOp {
    pub file: RemoteFile,
    /// The baseline did not have this path.
    pub added: bool,
    /// The file the new one replaces, moved aside first.
    pub existing: Option<Existing>,
    pub conflict: Option<ConflictKind>,
    /// Keep the replaced file next to the new one as `<file>.bak`.
    pub keep_bak: bool,
    /// Kept as `.bak` without asking, because Drop can't tell whether the
    /// player changed it (see [`Plan::backup_paths`]).
    pub backup: bool,
}

/// Remove a file the target no longer ships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteOp {
    pub existing: Existing,
    pub conflict: Option<ConflictKind>,
    pub keep_bak: bool,
    /// As [`WriteOp::backup`].
    pub backup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub path: String,
    pub kind: ConflictKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// Writes, conflicts included (`conflict` is `Some`).
    pub writes: Vec<WriteOp>,
    /// Deletes, conflicts included.
    pub deletes: Vec<DeleteOp>,
    /// Target files the update leaves as they are, with the mtime the next
    /// baseline records for them (`None`: unknown, hash next time). Under a
    /// linked player-data folder, the old baseline entry instead (removed
    /// files included; see the module docs).
    pub kept: Vec<(RemoteFile, Option<u64>)>,
    /// Player-data folders that are links (or reached through one), where
    /// this update had files to add, change or remove and left them as they
    /// are. Sorted, no duplicates.
    pub skipped_linked: Vec<String>,
    /// The 6.1.x healing was due (`PlanInput::previously_shipped` not empty)
    /// but a linked player-data folder kept it from checking a file it
    /// should have: the next baseline must not be marked healed.
    pub heal_incomplete: bool,
    /// Baseline paths the target no longer ships (Drop runtime data
    /// excluded), and leftovers an earlier revision shipped that are removed.
    /// Every mod's backup of these is discarded, as the Phase 0 sweep does.
    pub dropped: Vec<String>,
}

impl Plan {
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut out: Vec<Conflict> = self
            .writes
            .iter()
            .filter_map(|w| {
                w.conflict.map(|kind| Conflict {
                    path: w.file.path.clone(),
                    kind,
                })
            })
            .chain(self.deletes.iter().filter_map(|d| {
                d.conflict.map(|kind| Conflict {
                    path: d.existing.disk_path.clone(),
                    kind,
                })
            }))
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    /// Files that will be replaced or removed with the old copy kept as
    /// `<file>.bak`, without asking, because Drop can't tell whether the
    /// player changed them:
    /// - the server doesn't know what the install originally had there
    ///   (`sha256: ""` in the baseline, from a folder edited before hashing),
    ///   or this client never checked the baseline entry on this disk (no
    ///   mtime: a baseline from the server, or an older install's), so the
    ///   file may be an older copy rather than the player's edit;
    /// - a Drop-managed mod had replaced it (it may also hold player edits);
    /// - a folder of the update has to go where the file is.
    ///
    /// Spelt as on disk (where the `.bak` goes). Never also a conflict.
    pub fn backup_paths(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .writes
            .iter()
            .filter(|w| w.backup && w.conflict.is_none())
            .filter_map(|w| w.existing.as_ref().map(|e| e.disk_path.clone()))
            .chain(
                self.deletes
                    .iter()
                    .filter(|d| d.backup && d.conflict.is_none())
                    .map(|d| d.existing.disk_path.clone()),
            )
            .collect();
        out.sort();
        out
    }

    /// (added, updated, removed) among the operations that are not conflicts
    /// (backups included).
    pub fn counts(&self) -> (usize, usize, usize) {
        let added = self
            .writes
            .iter()
            .filter(|w| w.conflict.is_none() && w.added)
            .count();
        let updated = self
            .writes
            .iter()
            .filter(|w| w.conflict.is_none() && !w.added)
            .count();
        let removed = self.deletes.iter().filter(|d| d.conflict.is_none()).count();
        (added, updated, removed)
    }
}

#[derive(Debug)]
pub enum PlanError {
    /// A server path that would leave the install folder, or is empty.
    BadPath(String),
    /// The same path twice in one list.
    Duplicate(String),
    /// A file could not be read to hash it.
    Io(String, std::io::Error),
    /// The update puts a folder where protected player data is (a file under
    /// a saves folder, say), or a file where a folder holds files Drop must
    /// keep. Nothing sensible can be done without the player.
    InTheWay(String),
    /// A folder the update has to change files in is a symlink or junction
    /// (on the Deck, often a link to the SD card). Moving files through it
    /// could act outside the install, so the update is refused up front,
    /// before anything is downloaded.
    Linked(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::BadPath(p) => write!(f, "the server listed an unsafe file path: {p:?}"),
            PlanError::Duplicate(p) => write!(f, "the server listed {p:?} twice"),
            PlanError::Io(p, e) => write!(f, "could not read {p}: {e}"),
            PlanError::InTheWay(p) => write!(
                f,
                "the update needs {p} to change between a file and a folder, but it holds files \
                 Drop must not remove. Move them out of the game folder and try again"
            ),
            PlanError::Linked(p) => write!(
                f,
                "{p} in the game folder is a link to another folder or drive, and Drop can't \
                 update files through a link. Replace the link with a real folder, or \
                 reinstall the game, then try again"
            ),
        }
    }
}

/// Whether two hashes name the same content. The server sends `""` for a
/// file whose hash it does not know (bytes that changed before the first
/// revision snapshot was taken). An unknown hash equals nothing, not even
/// another unknown one.
pub fn same_hash(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && a.eq_ignore_ascii_case(b)
}

/// Install-root names Drop itself owns. A server path naming one is never
/// planned (it would overwrite Drop's own state).
const RESERVED_TOP_LEVEL: &[&str] = &[
    super::UPDATE_DIR,
    super::BASELINE_FILE,
    super::super::drop_data::DROPDATA_PATH,
    super::super::mod_data::MODS_DIR,
];

/// `\` to `/`, `.` and empty segments dropped. Rejects `..`, absolute paths
/// and drive prefixes. The result is only used to MATCH paths; disk access
/// keeps the server's own spelling, as the downloader does.
pub fn normalize_path(path: &str) -> Result<String, PlanError> {
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(PlanError::BadPath(path.to_string()));
    }
    let parts: Vec<&str> = path
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let bad = parts.is_empty()
        || parts.contains(&"..")
        || parts.first().is_some_and(|p| p.ends_with(':'));
    if bad {
        return Err(PlanError::BadPath(path.to_string()));
    }
    Ok(parts.join("/"))
}

fn is_reserved(normalized: &str) -> bool {
    let top = normalized.split('/').next().unwrap_or("");
    RESERVED_TOP_LEVEL
        .iter()
        .any(|r| r.eq_ignore_ascii_case(top))
}

/// How paths compare on the install's filesystem: case-insensitively on
/// Windows (NTFS), exactly elsewhere, where `Config/x.cfg` and `config/x.cfg`
/// are two different files.
#[derive(Clone, Copy)]
struct Fold(bool);

impl Fold {
    fn key(self, normalized: &str) -> String {
        if self.0 {
            normalized.to_lowercase()
        } else {
            normalized.to_string()
        }
    }
}

/// Folded normalised paths that occur more than once in one list.
fn case_collisions<'a>(
    paths: impl Iterator<Item = &'a str>,
    fold: Fold,
) -> Result<HashSet<String>, PlanError> {
    let mut seen_exact: HashSet<String> = HashSet::new();
    let mut seen_folded: HashSet<String> = HashSet::new();
    let mut collided = HashSet::new();
    for p in paths {
        let n = normalize_path(p)?;
        if !seen_exact.insert(n.clone()) {
            return Err(PlanError::Duplicate(p.to_string()));
        }
        let k = fold.key(&n);
        if !seen_folded.insert(k.clone()) {
            collided.insert(k);
        }
    }
    Ok(collided)
}

fn match_key(path: &str, collided: &HashSet<String>, fold: Fold) -> Result<String, PlanError> {
    let n = normalize_path(path)?;
    let k = fold.key(&n);
    // Exact keys get a prefix no folded key can have, so a collided pair
    // never accidentally pairs with an unrelated folded key.
    Ok(if collided.contains(&k) { format!("\0{n}") } else { k })
}

pub struct PlanInput<'a> {
    pub baseline: &'a [BaselineFile],
    pub target: &'a [RemoteFile],
    pub disk: &'a dyn DiskView,
    /// Drop runtime data (`is_drop_runtime_data`): only written where nothing
    /// is on disk, never replaced, deleted or a conflict.
    pub is_runtime_data: &'a dyn Fn(&str) -> bool,
    /// Generic player-data folders (`is_player_data_folder`): the files the
    /// pack ships there are updated, but asked about where Drop can't prove
    /// the copy on disk is the pack's (see the module docs).
    pub is_player_data: &'a dyn Fn(&str) -> bool,
    /// Every path an earlier revision of the installed version shipped (up
    /// to the installed one), spelt as the server spells it, with every
    /// `(size, sha256)` shipped for it, for the one-shot healing of what
    /// 6.1.0/6.1.1 left behind (see the module docs). Empty once an install
    /// has been healed.
    pub previously_shipped: &'a HashMap<String, HashSet<(u64, String)>>,
    /// Files Drop-managed mods have claimed, install-relative, as spelt in
    /// the mod ledgers (`mod_owned_files_spelled`).
    pub mod_owned: &'a HashSet<String>,
    /// Whether the install's filesystem ignores case (`cfg!(windows)`).
    pub case_insensitive: bool,
    /// Files the player chose "keep mine" for in an earlier update (the
    /// local baseline's `keptMine`), spelt as the target (or, for a file the
    /// target removed, the baseline) spelt them. Their baseline entry has no
    /// mtime on purpose; see `plan_unchanged`. They are never treated as the
    /// pack's copy, nor removed as leftovers.
    pub kept_mine: &'a HashSet<String>,
}

/// How the disk compares to the baseline and target versions of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OnDisk {
    Missing,
    Base,
    Target,
    /// Byte-identical to what an earlier revision shipped at this path: the
    /// pack's own copy, as safe to replace or remove as `Base`.
    Earlier,
    Other,
}

struct Seen {
    state: OnDisk,
    stat: DiskStat,
    /// The hash, when one was taken.
    sha256: Option<String>,
}

/// `earlier`: what earlier revisions shipped at this path, when such a copy
/// may count as the pack's (`Ctx::earlier_copies`).
fn look(
    disk: &dyn DiskView,
    path: &str,
    base: Option<&BaselineFile>,
    target: Option<&RemoteFile>,
    earlier: Option<&HashSet<(u64, String)>>,
) -> Result<Seen, PlanError> {
    let stat = disk.stat(path);
    let (size, mtime) = match stat {
        DiskStat::Missing => {
            return Ok(Seen {
                state: OnDisk::Missing,
                stat,
                sha256: None,
            });
        }
        DiskStat::Dir | DiskStat::Other => {
            return Ok(Seen {
                state: OnDisk::Other,
                stat,
                sha256: None,
            });
        }
        DiskStat::File { size, mtime } => (size, mtime),
    };
    // The baseline recorded the file's mtime when it knew the content: same
    // size and mtime means it has not been written since.
    if let Some(b) = base
        && !b.sha256.is_empty()
        && b.mtime.is_some()
        && b.mtime == mtime
        && b.size == size
    {
        return Ok(Seen {
            state: OnDisk::Base,
            stat,
            sha256: None,
        });
    }
    // An unknown hash can't match, so there is nothing to hash for.
    let could_be_base = base.is_some_and(|b| b.size == size && !b.sha256.is_empty());
    let could_be_target = target.is_some_and(|t| t.size == size && !t.sha256.is_empty());
    let could_be_earlier = earlier.is_some_and(|e| e.iter().any(|(s, h)| *s == size && !h.is_empty()));
    if !could_be_base && !could_be_target && !could_be_earlier {
        return Ok(Seen {
            state: OnDisk::Other,
            stat,
            sha256: None,
        });
    }
    let hash = disk
        .sha256(path)
        .map_err(|e| PlanError::Io(path.to_string(), e))?;
    let state = if base.is_some_and(|b| same_hash(&b.sha256, &hash)) {
        OnDisk::Base
    } else if target.is_some_and(|t| same_hash(&t.sha256, &hash)) {
        OnDisk::Target
    } else if earlier.is_some_and(|e| e.iter().any(|(_, h)| same_hash(h, &hash))) {
        OnDisk::Earlier
    } else {
        OnDisk::Other
    };
    Ok(Seen {
        state,
        stat,
        sha256: Some(hash),
    })
}

fn stat_mtime(stat: DiskStat) -> Option<u64> {
    match stat {
        DiskStat::File { mtime, .. } => mtime,
        _ => None,
    }
}

fn expect_content(sha256: &str, stat: DiskStat) -> Expect {
    match stat {
        DiskStat::File { size, mtime } => Expect::Content {
            sha256: sha256.to_string(),
            size,
            mtime,
        },
        _ => Expect::Present,
    }
}

struct Ctx<'a> {
    disk: &'a dyn DiskView,
    is_runtime_data: &'a dyn Fn(&str) -> bool,
    is_player_data: &'a dyn Fn(&str) -> bool,
    mod_owned: HashSet<String>,
    kept_mine: HashSet<String>,
    /// `previously_shipped` by `Fold::key`: one spelling to reach the file
    /// by (the first, sorted) and every (size, hash) any spelling shipped.
    /// Unknown hashes left out.
    earlier: BTreeMap<String, (String, HashSet<(u64, String)>)>,
    /// Folders already checked by `linked_player_data_folder`.
    linked: RefCell<HashMap<String, bool>>,
    fold: Fold,
}

impl Ctx<'_> {
    fn runtime_data(&self, p: &str) -> bool {
        (self.is_runtime_data)(p)
    }

    /// Under a generic player-data folder (and not Drop runtime data, which
    /// is decided before this is asked).
    fn player_data(&self, p: &str) -> bool {
        (self.is_player_data)(p)
    }

    fn owned_by_mod(&self, p: &str) -> bool {
        normalize_path(p)
            .map(|n| self.mod_owned.contains(&self.fold.key(&n)))
            .unwrap_or(false)
    }

    /// What earlier revisions shipped at `p`, when a copy matching one may
    /// count as the pack's: only under a player-data folder, never for Drop
    /// runtime data, nor for a file the player chose to keep.
    fn earlier_copies(&self, p: &str) -> Option<&HashSet<(u64, String)>> {
        if !self.player_data(p) || self.runtime_data(p) || self.kept_by_player(p) {
            return None;
        }
        let n = normalize_path(p).ok()?;
        self.earlier.get(&self.fold.key(&n)).map(|(_, set)| set)
    }

    /// The linked folder `p` is reached through, when `p` is under a
    /// player-data folder that is a link or holds one on the way (see the
    /// module docs). Each folder is looked at once.
    fn linked_player_data_folder(&self, p: &str) -> Option<String> {
        if !self.player_data(p) {
            return None;
        }
        ancestors(p).into_iter().find(|folder| {
            if let Some(&linked) = self.linked.borrow().get(folder) {
                return linked;
            }
            let linked = self.disk.stat(folder) == DiskStat::Other;
            self.linked.borrow_mut().insert(folder.clone(), linked);
            linked
        })
    }

    /// As [`Self::linked_player_data_folder`] for any spelling of a file.
    fn linked_folder_of(&self, paths: &[&str]) -> Option<String> {
        paths.iter().find_map(|p| self.linked_player_data_folder(p))
    }

    /// Whether this plan heals what 6.1.x left behind (see the module docs).
    fn healing(&self) -> bool {
        !self.earlier.is_empty()
    }

    fn kept_by_player(&self, p: &str) -> bool {
        normalize_path(p)
            .map(|n| self.kept_mine.contains(&self.fold.key(&n)))
            .unwrap_or(false)
    }

    /// Whether this client never confirmed the baseline entry against this
    /// disk, so a different file there can't be told apart from an older
    /// copy the install was given. True for an unknown hash, and for an entry
    /// with no mtime: a baseline from the server, or one an earlier update
    /// recorded without checking. Not for files the player chose "keep mine"
    /// for, whose entries have no mtime on purpose.
    fn unverified(&self, b: &BaselineFile) -> bool {
        b.sha256.is_empty() || (b.mtime.is_none() && !self.kept_by_player(&b.path))
    }
}

/// Plan an in-place update. See the module docs for the rules.
pub fn plan(input: PlanInput<'_>) -> Result<Plan, PlanError> {
    let PlanInput {
        baseline,
        target,
        disk,
        is_runtime_data,
        is_player_data,
        mod_owned,
        case_insensitive,
        kept_mine,
        previously_shipped,
    } = input;
    let fold = Fold(case_insensitive);

    let mut collided = case_collisions(baseline.iter().map(|b| b.path.as_str()), fold)?;
    collided.extend(case_collisions(target.iter().map(|t| t.path.as_str()), fold)?);

    let mut by_key: BTreeMap<String, (Option<&BaselineFile>, Option<&RemoteFile>)> = BTreeMap::new();
    for b in baseline {
        by_key.entry(match_key(&b.path, &collided, fold)?).or_default().0 = Some(b);
    }
    for t in target {
        by_key.entry(match_key(&t.path, &collided, fold)?).or_default().1 = Some(t);
    }

    let ctx = Ctx {
        disk,
        is_runtime_data,
        is_player_data,
        mod_owned: mod_owned
            .iter()
            .filter_map(|p| normalize_path(p).ok())
            .map(|n| fold.key(&n))
            .collect(),
        kept_mine: kept_mine
            .iter()
            .filter_map(|p| normalize_path(p).ok())
            .map(|n| fold.key(&n))
            .collect(),
        earlier: earlier_by_key(previously_shipped, fold),
        linked: RefCell::new(HashMap::new()),
        fold,
    };

    // Every path B or T lists, in any letter case: a leftover is never one
    // of them, even on a disk this build thinks is case-sensitive (an exFAT
    // SD card on Linux is not).
    let listed: HashSet<String> = baseline
        .iter()
        .map(|b| b.path.as_str())
        .chain(target.iter().map(|t| t.path.as_str()))
        .filter_map(|p| normalize_path(p).ok())
        .map(|n| n.to_lowercase())
        .collect();

    let mut plan = Plan::default();
    for (key, pair) in by_key {
        let key_path = key.trim_start_matches('\0');
        if is_reserved(&key_path.to_lowercase()) {
            log::warn!("update plan: ignoring server path {key_path:?}, which names Drop's own data");
            continue;
        }
        match pair {
            (None, None) => {}
            (None, Some(t)) => plan_added(&mut plan, &ctx, t)?,
            (Some(b), Some(t)) => plan_both(&mut plan, &ctx, b, t)?,
            (Some(b), None) => plan_removed(&mut plan, &ctx, b)?,
        }
    }
    plan_leftovers(&mut plan, &ctx, &listed);
    plan.skipped_linked.sort();
    plan.skipped_linked.dedup();
    refuse_links(&plan, &ctx)?;
    clear_way_for_folders(&mut plan, &ctx)?;
    clear_way_for_files(&mut plan, &ctx)?;
    Ok(plan)
}

/// Every folder an operation goes through must be a real folder (or not
/// exist yet): see [`PlanError::Linked`].
fn refuse_links(plan: &Plan, ctx: &Ctx<'_>) -> Result<(), PlanError> {
    let paths = plan
        .writes
        .iter()
        .flat_map(|w| std::iter::once(w.file.path.as_str()).chain(w.existing.as_ref().map(|e| e.disk_path.as_str())))
        .chain(plan.deletes.iter().map(|d| d.existing.disk_path.as_str()));
    let mut folders: Vec<String> = paths.flat_map(ancestors).collect();
    folders.sort();
    folders.dedup();
    for folder in folders {
        if ctx.disk.stat(&folder) == DiskStat::Other {
            return Err(PlanError::Linked(folder));
        }
    }
    Ok(())
}

/// A baseline entry as a next-baseline entry: what was there before, for a
/// file this update leaves untouched without knowing what it holds now.
fn baseline_entry(b: &BaselineFile) -> RemoteFile {
    RemoteFile {
        path: b.path.clone(),
        size: b.size,
        sha256: b.sha256.clone(),
    }
}

fn write(file: &RemoteFile, added: bool, existing: Option<Existing>, conflict: Option<ConflictKind>) -> WriteOp {
    WriteOp {
        file: file.clone(),
        added,
        existing,
        conflict,
        keep_bak: false,
        backup: false,
    }
}

/// A write that replaces `disk_path` and keeps the old copy as `.bak`
/// without asking (see [`Plan::backup_paths`]).
fn write_with_backup(file: &RemoteFile, added: bool, disk_path: String) -> WriteOp {
    WriteOp {
        file: file.clone(),
        added,
        existing: Some(Existing {
            disk_path,
            expect: Expect::Present,
        }),
        conflict: None,
        keep_bak: true,
        backup: true,
    }
}

fn plan_added(plan: &mut Plan, ctx: &Ctx<'_>, t: &RemoteFile) -> Result<(), PlanError> {
    // Through a linked player-data folder: not written, and not recorded,
    // so a later update adds it again.
    if let Some(folder) = ctx.linked_folder_of(&[&t.path]) {
        plan.skipped_linked.push(folder);
        plan.heal_incomplete |= ctx.healing();
        return Ok(());
    }
    let stat = ctx.disk.stat(&t.path);
    if stat == DiskStat::Missing {
        plan.writes.push(write(t, true, None, None));
        return Ok(());
    }
    if ctx.runtime_data(&t.path) {
        plan.kept.push((t.clone(), None));
        return Ok(());
    }
    if ctx.owned_by_mod(&t.path) {
        // The base game now ships a file a mod added: the game takes it back.
        // The mod's copy (which may hold the player's edits) is kept as .bak.
        plan.writes.push(write_with_backup(t, true, t.path.clone()));
        return Ok(());
    }
    let seen = look(ctx.disk, &t.path, None, Some(t), ctx.earlier_copies(&t.path))?;
    match seen.state {
        OnDisk::Missing => plan.writes.push(write(t, true, None, None)),
        OnDisk::Target => plan.kept.push((t.clone(), stat_mtime(seen.stat))),
        // An earlier revision's copy (the pack shipped this path before, and
        // an earlier update left it): the pack's own, replaced.
        OnDisk::Earlier => {
            let existing = Existing {
                disk_path: t.path.clone(),
                expect: expect_content(seen.sha256.as_deref().unwrap_or_default(), seen.stat),
            };
            plan.writes.push(write(t, true, Some(existing), None));
        }
        OnDisk::Base | OnDisk::Other => {
            let existing = Existing {
                disk_path: t.path.clone(),
                expect: Expect::Present,
            };
            plan.writes
                .push(write(t, true, Some(existing), Some(ConflictKind::AddedExists)));
        }
    }
    Ok(())
}

fn plan_both(plan: &mut Plan, ctx: &Ctx<'_>, b: &BaselineFile, t: &RemoteFile) -> Result<(), PlanError> {
    // A different spelling of the same path (`\\` or `./`) is the same file;
    // a different case (only matched on a case-insensitive disk) is a rename.
    let renamed = normalize_path(&b.path)? != normalize_path(&t.path)?;
    let changed = !same_hash(&b.sha256, &t.sha256) || renamed;
    // Through a linked player-data folder: untouched, and the baseline
    // keeps saying what was there before, not what the target ships.
    // A file the update doesn't change is not reported as skipped. One
    // never checked on this disk can't be healed through the link either.
    if let Some(folder) = ctx.linked_folder_of(&[&b.path, &t.path]) {
        if changed {
            plan.skipped_linked.push(folder);
        }
        plan.heal_incomplete |= ctx.healing() && ctx.unverified(b);
        plan.kept.push((baseline_entry(b), b.mtime));
        return Ok(());
    }
    if !changed {
        return plan_unchanged(plan, ctx, b, t);
    }

    let disk_path = b.path.clone();
    let stat = ctx.disk.stat(&disk_path);
    if stat == DiskStat::Missing {
        plan.writes.push(write(t, false, None, None));
        return Ok(());
    }
    if ctx.runtime_data(&t.path) || ctx.runtime_data(&b.path) {
        plan.kept.push((t.clone(), None));
        return Ok(());
    }
    if ctx.owned_by_mod(&disk_path) || ctx.owned_by_mod(&t.path) {
        // The game rewrites a file a mod replaced: hand-over, no conflict. The
        // mod's copy (which may hold the player's edits) is kept as .bak.
        plan.writes.push(write_with_backup(t, false, disk_path));
        return Ok(());
    }

    // An earlier copy only counts where the baseline can't vouch for the
    // disk (see the module docs).
    let earlier = if ctx.unverified(b) && !ctx.kept_by_player(&t.path) {
        ctx.earlier_copies(&disk_path)
    } else {
        None
    };
    let seen = look(ctx.disk, &disk_path, Some(b), Some(t), earlier)?;
    let unknown_base = ctx.unverified(b) && !ctx.kept_by_player(&t.path);
    let player_data = ctx.player_data(&b.path) || ctx.player_data(&t.path);
    match seen.state {
        OnDisk::Missing => plan.writes.push(write(t, false, None, None)),
        OnDisk::Base | OnDisk::Earlier => {
            let sha = seen.sha256.as_deref().unwrap_or(&b.sha256);
            let existing = Existing {
                disk_path,
                expect: expect_content(sha, seen.stat),
            };
            plan.writes.push(write(t, false, Some(existing), None));
        }
        OnDisk::Target if !renamed => {
            plan.kept.push((t.clone(), stat_mtime(seen.stat)));
        }
        OnDisk::Target => {
            // Right content under the old spelling: put it under the
            // target's spelling, or validation can't find it.
            let sha = seen.sha256.as_deref().unwrap_or(&t.sha256);
            let existing = Existing {
                disk_path,
                expect: expect_content(sha, seen.stat),
            };
            plan.writes.push(write(t, false, Some(existing), None));
        }
        // What the install had here is unknown or was never checked on this
        // disk (see `Ctx::unverified`): the update wins and the file on disk
        // is kept as .bak, without a conflict to decide. Not under a
        // player-data folder, where it may be an emulator's save or setting:
        // that falls through to a conflict.
        OnDisk::Other if unknown_base && !player_data => plan.writes.push(write_with_backup(t, false, disk_path)),
        OnDisk::Other => {
            let existing = Existing {
                disk_path,
                expect: Expect::Present,
            };
            plan.writes
                .push(write(t, false, Some(existing), Some(ConflictKind::ChangedBoth)));
        }
    }
    Ok(())
}

/// A file the update does not change (same hash in B and T).
///
/// When this client verified the baseline entry on this disk (it has an
/// mtime), what is on disk now is either that content or the player's own
/// edit, and either way it stays: the disk is not even looked at. The mtime
/// is carried over, so an edit made before this update still reads as one.
///
/// Without an mtime the entry is only the server's word for what the install
/// has: a baseline fetched from the server (installs from before in-place
/// updates), or one an earlier update recorded without checking. That says
/// nothing about this disk, which may still hold an older copy of the file
/// (the version's folder changed before its fingerprints were recorded, and
/// this install was made before that). So the file is checked:
/// - missing: downloaded;
/// - the shipped content: kept, and the next baseline records its mtime;
/// - an older copy an earlier revision shipped at this path: replaced, no
///   `.bak` (it is the pack's own content);
/// - anything else: the update's copy is written and the one on disk is kept
///   as `.bak` (Drop can't tell an older copy from a player edit; see
///   [`Plan::backup_paths`]). Under a player-data folder it stays as it is
///   (emulators rewrite the settings files they ship).
///
/// Files the player chose "keep mine" for also have no mtime, on purpose:
/// they stay as they are. So do Drop runtime data and files a Drop-managed
/// mod has claimed, as for any unchanged file.
fn plan_unchanged(plan: &mut Plan, ctx: &Ctx<'_>, b: &BaselineFile, t: &RemoteFile) -> Result<(), PlanError> {
    if !ctx.unverified(b)
        || ctx.kept_by_player(&t.path)
        || ctx.runtime_data(&t.path)
        || ctx.runtime_data(&b.path)
        || ctx.owned_by_mod(&b.path)
        || ctx.owned_by_mod(&t.path)
    {
        plan.kept.push((t.clone(), b.mtime));
        return Ok(());
    }
    let seen = look(ctx.disk, &b.path, Some(b), Some(t), ctx.earlier_copies(&b.path))?;
    match seen.state {
        OnDisk::Missing => plan.writes.push(write(t, false, None, None)),
        OnDisk::Base | OnDisk::Target => plan.kept.push((t.clone(), stat_mtime(seen.stat))),
        // An older copy the pack itself shipped: replaced, nothing to keep.
        OnDisk::Earlier => {
            let existing = Existing {
                disk_path: b.path.clone(),
                expect: expect_content(seen.sha256.as_deref().unwrap_or_default(), seen.stat),
            };
            plan.writes.push(write(t, false, Some(existing), None));
        }
        // A different file under a player-data folder: an emulator may have
        // rewritten it (its settings, say). Left alone, as before.
        OnDisk::Other if ctx.player_data(&b.path) || ctx.player_data(&t.path) => {
            plan.kept.push((t.clone(), b.mtime));
        }
        // A different file, or a folder or link where the file should be.
        OnDisk::Other => plan.writes.push(write_with_backup(t, false, b.path.clone())),
    }
    Ok(())
}

fn plan_removed(plan: &mut Plan, ctx: &Ctx<'_>, b: &BaselineFile) -> Result<(), PlanError> {
    if ctx.runtime_data(&b.path) {
        return Ok(());
    }
    // Through a linked player-data folder: not removed, and kept in the
    // baseline so a later update still removes it.
    if let Some(folder) = ctx.linked_folder_of(&[&b.path]) {
        plan.skipped_linked.push(folder);
        plan.heal_incomplete |= ctx.healing() && ctx.unverified(b);
        plan.kept.push((baseline_entry(b), b.mtime));
        return Ok(());
    }
    plan.dropped.push(b.path.clone());
    if ctx.owned_by_mod(&b.path) {
        // A mod replaced it; the file is the mod's now and stays.
        return Ok(());
    }
    let earlier = if ctx.unverified(b) { ctx.earlier_copies(&b.path) } else { None };
    let seen = look(ctx.disk, &b.path, Some(b), None, earlier)?;
    let existing = |expect| Existing {
        disk_path: b.path.clone(),
        expect,
    };
    match seen.state {
        OnDisk::Missing => {}
        OnDisk::Base | OnDisk::Earlier => {
            let sha = seen.sha256.as_deref().unwrap_or(&b.sha256);
            plan.deletes.push(DeleteOp {
                existing: existing(expect_content(sha, seen.stat)),
                conflict: None,
                keep_bak: false,
                backup: false,
            });
        }
        // Unknown or unchecked original (see `Ctx::unverified`): removed, but
        // the file on disk is kept as .bak. Under a player-data folder it is
        // asked about instead (below).
        OnDisk::Target | OnDisk::Other if ctx.unverified(b) && !ctx.player_data(&b.path) => plan.deletes.push(DeleteOp {
            existing: existing(Expect::Present),
            conflict: None,
            keep_bak: true,
            backup: true,
        }),
        OnDisk::Target | OnDisk::Other => plan.deletes.push(DeleteOp {
            existing: existing(Expect::Present),
            conflict: Some(ConflictKind::RemovedEdited),
            keep_bak: false,
            backup: false,
        }),
    }
    Ok(())
}

/// `previously_shipped` keyed as `Ctx::earlier`.
fn earlier_by_key(
    previously_shipped: &HashMap<String, HashSet<(u64, String)>>,
    fold: Fold,
) -> BTreeMap<String, (String, HashSet<(u64, String)>)> {
    let mut out: BTreeMap<String, (String, HashSet<(u64, String)>)> = BTreeMap::new();
    for (path, copies) in previously_shipped {
        let Ok(n) = normalize_path(path) else {
            log::warn!("update plan: ignoring unsafe path {path:?} from an earlier revision");
            continue;
        };
        if is_reserved(&n.to_lowercase()) {
            continue;
        }
        // Spellings that differ only in case are one file on Windows: their
        // copies are pooled, reached by the first spelling in sort order.
        let entry = out.entry(fold.key(&n)).or_insert_with(|| (path.clone(), HashSet::new()));
        if *path < entry.0 {
            entry.0 = path.clone();
        }
        // Unknown hashes match nothing, and every empty file has the same
        // hash: neither says the file is the pack's.
        entry.1.extend(copies.iter().filter(|(size, h)| !h.is_empty() && *size > 0).cloned());
    }
    out
}

/// Remove pack files an earlier update left behind: see "leftovers" in the
/// module docs. `listed` holds every B and T path as `Fold::key` keys them.
///
/// Best effort: a leftover that can't be read is left where it is (and
/// logged) rather than failing the update. Only regular files whose size
/// matches an earlier copy are hashed. `listed` holds every B and T path,
/// normalised and lower-cased.
fn plan_leftovers(plan: &mut Plan, ctx: &Ctx<'_>, listed: &HashSet<String>) {
    for (key, (path, copies)) in &ctx.earlier {
        if listed.contains(&key.to_lowercase())
            || copies.is_empty()
            || !ctx.player_data(path)
            || ctx.runtime_data(path)
            || ctx.owned_by_mod(path)
            || ctx.kept_by_player(path)
        {
            continue;
        }
        let stat = ctx.disk.stat(path);
        let DiskStat::File { size, .. } = stat else {
            continue;
        };
        if !copies.iter().any(|(s, _)| *s == size) {
            continue;
        }
        // Healing work, not the update's: not reported, but tried again.
        if ctx.linked_folder_of(&[path]).is_some() {
            plan.heal_incomplete = true;
            continue;
        }
        // Not through a link (nor through anything else that is not a real
        // folder): the file must really be inside the install.
        if ancestors(path).iter().any(|a| ctx.disk.stat(a) != DiskStat::Dir) {
            continue;
        }
        let hash = match ctx.disk.sha256(path) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("update plan: could not read {path} to check whether it is a leftover ({e}); leaving it");
                continue;
            }
        };
        if !copies.iter().any(|(_, h)| same_hash(h, &hash)) {
            continue;
        }
        log::info!("update plan: setting aside {path} as .bak, a file an earlier revision shipped and this one does not");
        plan.deletes.push(DeleteOp {
            existing: Existing {
                disk_path: path.clone(),
                expect: expect_content(&hash, stat),
            },
            conflict: None,
            keep_bak: true,
            backup: true,
        });
        plan.dropped.push(path.clone());
    }
}

/// The folders `rel` sits in, shallowest first (`a/b/c` -> `a`, `a/b`).
fn ancestors(rel: &str) -> Vec<String> {
    let parts: Vec<&str> = rel.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".").collect();
    (1..parts.len()).map(|n| parts[..n].join("/")).collect()
}

/// A file goes where the update needs a folder (`data` was a file, the
/// target has `data/x`). The file has to go first: a planned delete of it
/// loses its conflict (the folder can't be kept out) and keeps a .bak; a file
/// the plan wasn't going to touch is removed with a .bak. Protected player
/// data in the way stops the plan.
fn clear_way_for_folders(plan: &mut Plan, ctx: &Ctx<'_>) -> Result<(), PlanError> {
    let mut needed: Vec<String> = plan
        .writes
        .iter()
        .flat_map(|w| ancestors(&w.file.path))
        .collect();
    needed.sort();
    needed.dedup();
    for folder in needed {
        match ctx.disk.stat(&folder) {
            DiskStat::Missing | DiskStat::Dir => continue,
            DiskStat::Other => return Err(PlanError::Linked(folder)),
            DiskStat::File { .. } => {}
        }
        let key = ctx.fold.key(&normalize_path(&folder)?);
        let planned = plan.deletes.iter_mut().find(|d| {
            normalize_path(&d.existing.disk_path)
                .map(|n| ctx.fold.key(&n) == key)
                .unwrap_or(false)
        });
        match planned {
            Some(d) => {
                if d.conflict.is_some() {
                    d.conflict = None;
                    d.keep_bak = true;
                    d.backup = true;
                }
            }
            // A file in neither list under a runtime or player-data folder
            // is never touched.
            None if ctx.runtime_data(&folder) || ctx.player_data(&folder) => {
                return Err(PlanError::InTheWay(folder));
            }
            None => plan.deletes.push(DeleteOp {
                existing: Existing {
                    disk_path: folder,
                    expect: Expect::Present,
                },
                conflict: None,
                keep_bak: true,
                backup: true,
            }),
        }
    }
    Ok(())
}

/// A folder is where the update needs a file (`data/x` became a file
/// `data`). When everything in the folder is being removed cleanly anyway,
/// the empty folder simply makes way: no conflict. Anything else in it (the
/// player's files, conflicts, .bak copies) would have to move with the
/// folder, so the plan stops and says so rather than guess.
fn clear_way_for_files(plan: &mut Plan, ctx: &Ctx<'_>) -> Result<(), PlanError> {
    let clean_deletes: HashSet<String> = plan
        .deletes
        .iter()
        .filter(|d| d.conflict.is_none() && !d.keep_bak)
        .filter_map(|d| normalize_path(&d.existing.disk_path).ok())
        .map(|n| ctx.fold.key(&n))
        .collect();
    for w in &mut plan.writes {
        let Some(existing) = &mut w.existing else { continue };
        if ctx.disk.stat(&existing.disk_path) != DiskStat::Dir {
            continue;
        }
        let inside = ctx
            .disk
            .files_under(&existing.disk_path)
            .map_err(|e| PlanError::Io(existing.disk_path.clone(), e))?;
        let all_going = inside.iter().all(|f| {
            normalize_path(f)
                .map(|n| clean_deletes.contains(&ctx.fold.key(&n)))
                .unwrap_or(false)
        });
        if !all_going {
            return Err(PlanError::InTheWay(existing.disk_path.clone()));
        }
        existing.expect = Expect::Folder { files: inside };
        w.conflict = None;
        w.keep_bak = false;
        w.backup = false;
    }
    Ok(())
}

/// How the next baseline learns a target file's mtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextMtime {
    /// The update writes it: read the staged file's mtime (a rename keeps it).
    AfterWrite,
    Known(Option<u64>),
}

/// A plan with every conflict decided.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resolved {
    pub writes: Vec<WriteOp>,
    pub deletes: Vec<DeleteOp>,
    /// Every entry of the next baseline.
    pub next_baseline: Vec<(RemoteFile, NextMtime)>,
    pub dropped: Vec<String>,
    /// Files the player chose to keep their own copy of: target files, and
    /// files the target removed (`removed_edited`). Recorded in the baseline
    /// so a later repair keeps a .bak before restoring them, and so no later
    /// update treats them as the pack's copy or removes them as leftovers.
    pub kept_mine: Vec<String>,
    /// As [`Plan::skipped_linked`].
    pub skipped_linked: Vec<String>,
    /// As [`Plan::heal_incomplete`].
    pub heal_incomplete: bool,
}

/// Apply the player's decisions. `Err` lists the conflicts with no decision.
/// Decisions for paths that are not conflicts are ignored.
///
/// - take update: the player's file is moved aside and kept as `<file>.bak`
///   (for `removed_edited` too: it leaves its path but is not destroyed);
/// - keep mine: the file is not touched. For `changed_both`/`added_exists`
///   the next baseline records the TARGET hash, so the file keeps reading as
///   the player's change; for `removed_edited` the path leaves the baseline
///   and the file becomes the player's own. Either way the path is recorded
///   in `kept_mine`.
pub fn resolve(plan: Plan, resolutions: &HashMap<String, Resolution>) -> Result<Resolved, Vec<String>> {
    let mut unresolved: BTreeSet<String> = BTreeSet::new();
    let mut out = Resolved {
        dropped: plan.dropped,
        skipped_linked: plan.skipped_linked,
        heal_incomplete: plan.heal_incomplete,
        ..Default::default()
    };
    for (file, mtime) in plan.kept {
        out.next_baseline.push((file, NextMtime::Known(mtime)));
    }
    for mut w in plan.writes {
        match w.conflict {
            None => {
                out.next_baseline.push((w.file.clone(), NextMtime::AfterWrite));
                out.writes.push(w);
            }
            Some(_) => match resolutions.get(&w.file.path) {
                Some(Resolution::TakeUpdate) => {
                    w.keep_bak = true;
                    out.next_baseline.push((w.file.clone(), NextMtime::AfterWrite));
                    out.writes.push(w);
                }
                Some(Resolution::KeepMine) => {
                    out.kept_mine.push(w.file.path.clone());
                    out.next_baseline.push((w.file, NextMtime::Known(None)));
                }
                None => {
                    unresolved.insert(w.file.path);
                }
            },
        }
    }
    for mut d in plan.deletes {
        match d.conflict {
            None => out.deletes.push(d),
            Some(_) => match resolutions.get(&d.existing.disk_path) {
                Some(Resolution::TakeUpdate) => {
                    d.keep_bak = true;
                    out.deletes.push(d);
                }
                Some(Resolution::KeepMine) => out.kept_mine.push(d.existing.disk_path),
                None => {
                    unresolved.insert(d.existing.disk_path);
                }
            },
        }
    }
    if !unresolved.is_empty() {
        return Err(unresolved.into_iter().collect());
    }
    out.next_baseline.sort_by(|a, b| a.0.path.cmp(&b.0.path));
    out.kept_mine.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// In-memory disk: path -> (content, mtime). `case_insensitive` mimics
    /// NTFS. Counts hashes so the mtime shortcut can be checked.
    #[derive(Default)]
    struct MemDisk {
        files: HashMap<String, (String, Option<u64>)>,
        dirs: HashSet<String>,
        links: HashSet<String>,
        case_insensitive: bool,
        hashed: RefCell<Vec<String>>,
    }

    impl MemDisk {
        fn with(files: &[(&str, &str)]) -> Self {
            let mut d = MemDisk::default();
            for (p, c) in files {
                d.files.insert(p.to_string(), (c.to_string(), Some(1)));
            }
            d
        }
        fn eq(&self, a: &str, b: &str) -> bool {
            if self.case_insensitive { a.eq_ignore_ascii_case(b) } else { a == b }
        }
        fn find(&self, rel: &str) -> Option<&(String, Option<u64>)> {
            self.files.iter().find(|(k, _)| self.eq(k, rel)).map(|(_, v)| v)
        }
        fn is_dir(&self, rel: &str) -> bool {
            let prefix = format!("{}/", rel.to_lowercase());
            self.dirs.iter().any(|d| self.eq(d, rel))
                || self.files.keys().any(|k| {
                    if self.case_insensitive {
                        k.to_lowercase().starts_with(&prefix)
                    } else {
                        k.starts_with(&format!("{rel}/"))
                    }
                })
        }
    }

    impl DiskView for MemDisk {
        fn stat(&self, rel: &str) -> DiskStat {
            if self.links.iter().any(|l| self.eq(l, rel)) {
                return DiskStat::Other;
            }
            // Under a file: nothing can be there (ENOTDIR reads as missing).
            if ancestors(rel).iter().any(|a| self.find(a).is_some()) {
                return DiskStat::Missing;
            }
            if let Some((c, m)) = self.find(rel) {
                return DiskStat::File {
                    size: c.len() as u64,
                    mtime: *m,
                };
            }
            if self.is_dir(rel) {
                return DiskStat::Dir;
            }
            DiskStat::Missing
        }
        fn sha256(&self, rel: &str) -> std::io::Result<String> {
            self.hashed.borrow_mut().push(rel.to_string());
            self.find(rel)
                .map(|(c, _)| h(c))
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        }
        fn files_under(&self, rel: &str) -> std::io::Result<Vec<String>> {
            Ok(self
                .files
                .keys()
                .filter(|k| ancestors(k).iter().any(|a| self.eq(a, rel)))
                .cloned()
                .collect())
        }
    }

    /// A fake "hash": the content itself, prefixed. Sizes still come from the
    /// content length, so size short-cuts behave as they would for real.
    fn h(content: &str) -> String {
        format!("h:{content}")
    }

    fn remote(path: &str, content: &str) -> RemoteFile {
        RemoteFile {
            path: path.to_string(),
            size: content.len() as u64,
            sha256: h(content),
        }
    }

    fn base(path: &str, content: &str, mtime: Option<u64>) -> BaselineFile {
        BaselineFile {
            path: path.to_string(),
            size: content.len() as u64,
            sha256: h(content),
            mtime,
        }
    }

    fn never_protected(_: &str) -> bool {
        false
    }

    fn run(b: &[BaselineFile], t: &[RemoteFile], d: &MemDisk) -> Plan {
        run_with(b, t, d, &never_protected, &HashSet::new())
    }

    fn run_with(
        b: &[BaselineFile],
        t: &[RemoteFile],
        d: &MemDisk,
        protected: &dyn Fn(&str) -> bool,
        mods: &HashSet<String>,
    ) -> Plan {
        plan(PlanInput {
            baseline: b,
            target: t,
            disk: d,
            is_runtime_data: protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: mods,
            case_insensitive: d.case_insensitive,
            kept_mine: &HashSet::new(),
        })
        .expect("plan")
    }

    fn run_kept(b: &[BaselineFile], t: &[RemoteFile], d: &MemDisk, kept_mine: &[&str]) -> Plan {
        let kept: HashSet<String> = kept_mine.iter().map(|s| s.to_string()).collect();
        plan(PlanInput {
            baseline: b,
            target: t,
            disk: d,
            is_runtime_data: &never_protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: d.case_insensitive,
            kept_mine: &kept,
        })
        .expect("plan")
    }

    // ── Unchanged files whose baseline was never checked on this disk ──
    //
    // The bug these cover (6.1.0, a Minecraft pack): the install was made
    // before the version's fingerprints were recorded and still had an older
    // mods/gates.jar. Its first in-place update took the server's earliest
    // snapshot as the baseline, which already listed the new gates.jar; the
    // update did not change it, so the disk was never looked at, and the
    // baseline written afterwards claimed the new jar was installed. Every
    // later update then believed it.

    #[test]
    fn an_unverified_unchanged_file_that_differs_on_disk_is_replaced_keeping_a_bak() {
        let b = [base("mods/gates.jar", "gates-new", None)];
        let t = [remote("mods/gates.jar", "gates-new"), remote("mods/burrowers.jar", "burrow")];
        let d = MemDisk::with(&[("mods/gates.jar", "gates-old-build")]);
        let p = run(&b, &t, &d);
        let w = p
            .writes
            .iter()
            .find(|w| w.file.path == "mods/gates.jar")
            .expect("gates.jar must be written");
        assert!(w.backup && w.conflict.is_none(), "{w:#?}");
        assert_eq!(p.backup_paths(), vec!["mods/gates.jar".to_string()]);
        assert!(p.writes.iter().any(|w| w.file.path == "mods/burrowers.jar" && w.added));
        assert!(p.conflicts().is_empty());
    }

    #[test]
    fn an_unverified_unchanged_file_already_right_is_kept_and_gets_verified() {
        let b = [base("mods/gates.jar", "gates-new", None)];
        let t = [remote("mods/gates.jar", "gates-new")];
        let mut d = MemDisk::with(&[("mods/gates.jar", "gates-new")]);
        d.files.get_mut("mods/gates.jar").unwrap().1 = Some(42);
        let p = run(&b, &t, &d);
        assert!(p.writes.is_empty(), "{p:#?}");
        assert_eq!(p.kept, vec![(remote("mods/gates.jar", "gates-new"), Some(42))]);
    }

    #[test]
    fn an_unverified_unchanged_file_that_is_missing_is_downloaded() {
        let b = [base("mods/gates.jar", "gates-new", None)];
        let t = [remote("mods/gates.jar", "gates-new")];
        let d = MemDisk::with(&[]);
        let p = run(&b, &t, &d);
        let w = only_write(&p);
        assert!(w.existing.is_none() && !w.backup && w.conflict.is_none(), "{w:#?}");
    }

    #[test]
    fn a_verified_unchanged_file_the_player_edited_stays_theirs_without_hashing() {
        let b = [base("config/x.cfg", "pack", Some(1))];
        let t = [remote("config/x.cfg", "pack")];
        let mut d = MemDisk::with(&[("config/x.cfg", "player edit")]);
        d.files.get_mut("config/x.cfg").unwrap().1 = Some(99);
        let p = run(&b, &t, &d);
        assert!(p.writes.is_empty(), "{p:#?}");
        assert!(d.hashed.borrow().is_empty());
    }

    #[test]
    fn an_unverified_changed_file_that_differs_on_disk_is_replaced_not_asked() {
        // Next revision changes the jar while this install still has a copy
        // older than its (server) baseline: not the player's edit as far as
        // Drop can tell, so no conflict; the update wins, .bak kept.
        let b = [base("mods/gates.jar", "gates-new", None)];
        let t = [remote("mods/gates.jar", "gates-newer")];
        let d = MemDisk::with(&[("mods/gates.jar", "gates-old-build")]);
        let p = run(&b, &t, &d);
        let w = only_write(&p);
        assert!(w.backup && w.conflict.is_none(), "{w:#?}");
        assert!(p.conflicts().is_empty());
    }

    #[test]
    fn an_unverified_removed_file_that_differs_on_disk_is_removed_keeping_a_bak() {
        // gates-1.0.0.jar renamed to gates-1.0.1.jar on the server: the old,
        // drifted jar must not stay loadable next to the new one.
        let b = [base("mods/gates-1.0.0.jar", "gates-new", None)];
        let t = [remote("mods/gates-1.0.1.jar", "gates-101")];
        let d = MemDisk::with(&[("mods/gates-1.0.0.jar", "gates-old-build")]);
        let p = run(&b, &t, &d);
        assert_eq!(p.deletes.len(), 1, "{p:#?}");
        assert!(p.deletes[0].keep_bak && p.deletes[0].backup && p.deletes[0].conflict.is_none());
        assert!(p.conflicts().is_empty());
    }

    #[test]
    fn a_kept_mine_file_the_update_changes_is_still_asked_about() {
        let b = [base("config/x.cfg", "pack", None)];
        let t = [remote("config/x.cfg", "pack2")];
        let d = MemDisk::with(&[("config/x.cfg", "player edit")]);
        let p = run_kept(&b, &t, &d, &["config/x.cfg"]);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
    }

    #[test]
    fn a_kept_mine_file_is_not_replaced_even_though_its_entry_has_no_mtime() {
        let b = [base("config/x.cfg", "pack", None)];
        let t = [remote("config/x.cfg", "pack")];
        let d = MemDisk::with(&[("config/x.cfg", "player edit")]);
        let p = run_kept(&b, &t, &d, &["config/x.cfg"]);
        assert!(p.writes.is_empty(), "{p:#?}");
        assert_eq!(p.kept, vec![(remote("config/x.cfg", "pack"), None)]);
    }

    #[test]
    fn an_unverified_unchanged_protected_file_is_left_alone() {
        let b = [base("drop-saves/slot1.dat", "pack", None)];
        let t = [remote("drop-saves/slot1.dat", "pack")];
        let d = MemDisk::with(&[("drop-saves/slot1.dat", "progress")]);
        let p = run_with(&b, &t, &d, &|p: &str| p.starts_with("drop-saves/"), &HashSet::new());
        assert!(p.writes.is_empty(), "{p:#?}");
    }

    fn only_write(p: &Plan) -> &WriteOp {
        assert_eq!(p.writes.len(), 1, "{p:#?}");
        assert!(p.deletes.is_empty(), "{p:#?}");
        &p.writes[0]
    }

    // ---- in T, not in B ----

    #[test]
    fn added_and_not_on_disk_is_downloaded() {
        let p = run(&[], &[remote("mods/new.jar", "N")], &MemDisk::default());
        let w = only_write(&p);
        assert!(w.added && w.existing.is_none() && w.conflict.is_none());
        assert_eq!(p.counts(), (1, 0, 0));
    }

    #[test]
    fn added_and_already_identical_on_disk_is_left_alone() {
        let d = MemDisk::with(&[("mods/new.jar", "N")]);
        let p = run(&[], &[remote("mods/new.jar", "N")], &d);
        assert!(p.writes.is_empty() && p.deletes.is_empty());
        assert_eq!(p.kept, vec![(remote("mods/new.jar", "N"), Some(1))]);
    }

    #[test]
    fn added_over_a_different_player_file_is_a_conflict() {
        let d = MemDisk::with(&[("options.txt", "mine")]);
        let p = run(&[], &[remote("options.txt", "pack")], &d);
        let w = only_write(&p);
        assert_eq!(w.conflict, Some(ConflictKind::AddedExists));
        assert_eq!(w.existing.as_ref().unwrap().expect, Expect::Present);
        assert_eq!(
            p.conflicts(),
            vec![Conflict {
                path: "options.txt".into(),
                kind: ConflictKind::AddedExists
            }]
        );
        assert_eq!(p.counts(), (0, 0, 0));
    }

    #[test]
    fn an_empty_folder_where_a_file_is_added_makes_way() {
        let mut d = MemDisk::default();
        d.dirs.insert("config".into());
        let p = run(&[], &[remote("config", "file")], &d);
        let w = only_write(&p);
        assert!(w.conflict.is_none() && !w.keep_bak);
        assert_eq!(w.existing.as_ref().unwrap().disk_path, "config");
    }

    #[test]
    fn a_folder_holding_player_files_where_a_file_is_added_stops_the_plan() {
        let d = MemDisk::with(&[("config/mine.cfg", "player")]);
        let r = plan(PlanInput {
            baseline: &[],
            target: &[remote("config", "file")],
            disk: &d,
            is_runtime_data: &never_protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: false,
            kept_mine: &HashSet::new(),
        });
        assert!(matches!(r, Err(PlanError::InTheWay(ref p)) if p == "config"), "{r:?}");
    }

    // ---- in both, changed ----

    #[test]
    fn changed_and_untouched_on_disk_is_replaced() {
        let d = MemDisk::with(&[("mods/a.jar", "v1")]);
        let p = run(&[base("mods/a.jar", "v1", None)], &[remote("mods/a.jar", "v2")], &d);
        let w = only_write(&p);
        assert!(!w.added && w.conflict.is_none());
        let e = w.existing.as_ref().unwrap();
        assert_eq!(e.disk_path, "mods/a.jar");
        assert_eq!(
            e.expect,
            Expect::Content {
                sha256: h("v1"),
                size: 2,
                mtime: Some(1)
            }
        );
        assert_eq!(p.counts(), (0, 1, 0));
    }

    #[test]
    fn changed_and_already_the_new_content_is_left_alone() {
        let d = MemDisk::with(&[("mods/a.jar", "v2")]);
        let p = run(&[base("mods/a.jar", "v1", None)], &[remote("mods/a.jar", "v2")], &d);
        assert!(p.writes.is_empty() && p.deletes.is_empty());
        assert_eq!(p.kept.len(), 1);
    }

    #[test]
    fn changed_and_missing_on_disk_is_downloaded() {
        let p = run(
            &[base("mods/a.jar", "v1", None)],
            &[remote("mods/a.jar", "v2")],
            &MemDisk::default(),
        );
        let w = only_write(&p);
        assert!(w.existing.is_none() && w.conflict.is_none() && !w.added);
    }

    #[test]
    fn changed_by_both_is_a_conflict() {
        let d = MemDisk::with(&[("config/pack.toml", "player")]);
        let p = run(
            &[base("config/pack.toml", "v1", Some(1))],
            &[remote("config/pack.toml", "v2")],
            &d,
        );
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
    }

    #[test]
    fn a_size_matching_neither_side_is_a_conflict_without_hashing() {
        let d = MemDisk::with(&[("big.pak", "a much longer edited file")]);
        let p = run(&[base("big.pak", "v1", Some(1))], &[remote("big.pak", "v2")], &d);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
        assert!(d.hashed.borrow().is_empty());
    }

    // ---- in both, unchanged ----

    #[test]
    fn unchanged_files_are_never_looked_at_even_when_edited() {
        let d = MemDisk::with(&[("options.txt", "player edit")]);
        let p = run(
            &[base("options.txt", "pack", Some(7))],
            &[remote("options.txt", "pack")],
            &d,
        );
        assert!(p.writes.is_empty() && p.deletes.is_empty());
        assert!(d.hashed.borrow().is_empty());
        // The old mtime is carried, so the edit still reads as one next time.
        assert_eq!(p.kept, vec![(remote("options.txt", "pack"), Some(7))]);
    }

    // ---- in B, not in T ----

    #[test]
    fn removed_and_untouched_is_deleted() {
        let d = MemDisk::with(&[("mods/old.jar", "o")]);
        let p = run(&[base("mods/old.jar", "o", None)], &[], &d);
        assert_eq!(p.deletes.len(), 1);
        assert!(p.deletes[0].conflict.is_none());
        assert_eq!(p.dropped, vec!["mods/old.jar".to_string()]);
        assert_eq!(p.counts(), (0, 0, 1));
    }

    #[test]
    fn removed_and_already_gone_is_nothing() {
        let p = run(&[base("mods/old.jar", "o", None)], &[], &MemDisk::default());
        assert!(p.writes.is_empty() && p.deletes.is_empty());
    }

    #[test]
    fn removed_but_edited_is_a_conflict() {
        let d = MemDisk::with(&[("config/old.cfg", "edited")]);
        let p = run(&[base("config/old.cfg", "orig", Some(1))], &[], &d);
        assert_eq!(p.deletes[0].conflict, Some(ConflictKind::RemovedEdited));
        assert_eq!(p.deletes[0].existing.expect, Expect::Present);
    }

    // ---- in neither ----

    #[test]
    fn player_files_in_neither_list_are_never_consulted() {
        struct Panicky;
        impl DiskView for Panicky {
            fn stat(&self, rel: &str) -> DiskStat {
                panic!("looked at {rel}")
            }
            fn sha256(&self, rel: &str) -> std::io::Result<String> {
                panic!("hashed {rel}")
            }
            fn files_under(&self, rel: &str) -> std::io::Result<Vec<String>> {
                panic!("listed {rel}")
            }
        }
        // Verified on this disk (it has an mtime), so the unchanged file is
        // not looked at either.
        let b = [base("same.txt", "x", Some(1))];
        let t = [remote("same.txt", "x")];
        let p = plan(PlanInput {
            baseline: &b,
            target: &t,
            disk: &Panicky,
            is_runtime_data: &never_protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: false,
            kept_mine: &HashSet::new(),
        })
        .unwrap();
        assert!(p.writes.is_empty() && p.deletes.is_empty() && p.dropped.is_empty());
    }

    // ---- mtime shortcut ----

    #[test]
    fn matching_size_and_mtime_is_trusted_without_hashing() {
        let d = MemDisk::with(&[("a.dll", "v1")]); // mtime 1
        let p = run(&[base("a.dll", "v1", Some(1))], &[remote("a.dll", "v2")], &d);
        assert!(only_write(&p).conflict.is_none());
        assert!(d.hashed.borrow().is_empty());
    }

    #[test]
    fn a_changed_mtime_is_hashed_and_still_matches_by_content() {
        let d = MemDisk::with(&[("a.dll", "v1")]); // mtime 1
        let p = run(&[base("a.dll", "v1", Some(99))], &[remote("a.dll", "v2")], &d);
        assert!(only_write(&p).conflict.is_none());
        assert_eq!(*d.hashed.borrow(), vec!["a.dll".to_string()]);
    }

    #[test]
    fn a_server_baseline_has_no_mtime_and_is_always_hashed() {
        let d = MemDisk::with(&[("a.dll", "v1")]);
        run(&[base("a.dll", "v1", None)], &[remote("a.dll", "v2")], &d);
        assert_eq!(d.hashed.borrow().len(), 1);
    }

    // ---- resolutions and the next baseline ----

    fn conflicts_plan() -> (Plan, MemDisk) {
        let d = MemDisk::with(&[
            ("added.cfg", "mine"),
            ("changed.cfg", "mine"),
            ("removed.cfg", "mine"),
            ("plain.jar", "v1"),
        ]);
        let p = run(
            &[
                base("changed.cfg", "c1", Some(1)),
                base("removed.cfg", "r1", Some(1)),
                base("plain.jar", "v1", Some(1)),
            ],
            &[
                remote("added.cfg", "a2"),
                remote("changed.cfg", "c2"),
                remote("plain.jar", "v2"),
            ],
            &d,
        );
        (p, d)
    }

    #[test]
    fn every_conflict_needs_a_decision() {
        let (p, _) = conflicts_plan();
        let mut r = HashMap::new();
        r.insert("added.cfg".to_string(), Resolution::KeepMine);
        r.insert("not-a-conflict".to_string(), Resolution::TakeUpdate);
        let err = resolve(p, &r).unwrap_err();
        assert_eq!(err, vec!["changed.cfg".to_string(), "removed.cfg".to_string()]);
    }

    #[test]
    fn keep_mine_records_the_target_hash_or_drops_the_path() {
        let (p, _) = conflicts_plan();
        let r: HashMap<String, Resolution> = ["added.cfg", "changed.cfg", "removed.cfg"]
            .iter()
            .map(|p| (p.to_string(), Resolution::KeepMine))
            .collect();
        let res = resolve(p, &r).unwrap();
        // Only the plain update is written; nothing is deleted.
        assert_eq!(res.writes.len(), 1);
        assert_eq!(res.writes[0].file.path, "plain.jar");
        assert!(res.deletes.is_empty());
        let next: HashMap<&str, (&str, NextMtime)> = res
            .next_baseline
            .iter()
            .map(|(f, m)| (f.path.as_str(), (f.sha256.as_str(), *m)))
            .collect();
        assert_eq!(next["added.cfg"], (h("a2").as_str(), NextMtime::Known(None)));
        assert_eq!(next["changed.cfg"], (h("c2").as_str(), NextMtime::Known(None)));
        assert_eq!(next["plain.jar"].1, NextMtime::AfterWrite);
        assert!(!next.contains_key("removed.cfg"));
    }

    #[test]
    fn keep_mine_is_still_the_players_change_on_the_next_update() {
        // Next update after keeping a changed_both file: baseline holds the
        // previous target hash with no mtime, the disk still has the player's
        // content. A further change to the file conflicts again; no change
        // leaves it alone.
        // The sidecar lists it in keptMine, which is what keeps it the
        // player's even though its entry has no mtime.
        let d = MemDisk::with(&[("changed.cfg", "mine")]);
        let b = [base("changed.cfg", "c2", None)];
        let p = run_kept(&b, &[remote("changed.cfg", "c3")], &d, &["changed.cfg"]);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
        let p = run_kept(&b, &[remote("changed.cfg", "c2")], &d, &["changed.cfg"]);
        assert!(p.writes.is_empty());
    }

    #[test]
    fn take_update_keeps_the_players_copy_as_bak() {
        let (p, _) = conflicts_plan();
        let r: HashMap<String, Resolution> = ["added.cfg", "changed.cfg", "removed.cfg"]
            .iter()
            .map(|p| (p.to_string(), Resolution::TakeUpdate))
            .collect();
        let res = resolve(p, &r).unwrap();
        let bak: Vec<&str> = res
            .writes
            .iter()
            .filter(|w| w.keep_bak)
            .map(|w| w.file.path.as_str())
            .collect();
        assert_eq!(bak, vec!["added.cfg", "changed.cfg"]);
        assert_eq!(res.deletes.len(), 1);
        assert!(res.deletes[0].keep_bak);
        assert!(res.next_baseline.iter().all(|(f, _)| f.path != "removed.cfg"));
    }

    // ---- protected data and mods ----

    /// Stands in for `is_drop_runtime_data`.
    fn saves_protected(p: &str) -> bool {
        p.starts_with("drop-saves/")
    }

    #[test]
    fn protected_paths_are_never_replaced_deleted_or_conflicted() {
        let d = MemDisk::with(&[("drop-saves/slot1.sav", "progress"), ("drop-saves/old.sav", "x")]);
        let p = run_with(
            &[base("drop-saves/slot1.sav", "default", None), base("drop-saves/old.sav", "x", None)],
            &[remote("drop-saves/slot1.sav", "new default"), remote("drop-saves/new.sav", "n")],
            &d,
            &saves_protected,
            &HashSet::new(),
        );
        // Only the missing protected file is written.
        let w = only_write(&p);
        assert_eq!(w.file.path, "drop-saves/new.sav");
        assert!(w.existing.is_none());
        assert!(p.dropped.is_empty());
        assert!(d.hashed.borrow().is_empty());
    }

    #[test]
    fn a_protected_file_that_is_missing_is_still_written() {
        let p = run_with(
            &[base("drop-saves/slot1.sav", "v1", None)],
            &[remote("drop-saves/slot1.sav", "v2")],
            &MemDisk::default(),
            &saves_protected,
            &HashSet::new(),
        );
        assert_eq!(only_write(&p).file.path, "drop-saves/slot1.sav");
    }

    #[test]
    fn mod_owned_files_follow_the_hand_over_rules() {
        let mut d = MemDisk::with(&[
            ("Mods/Loader.dll", "mod copy"),
            ("mods/extra.dll", "mod file"),
            ("mods/gone.dll", "mod copy of a dropped file"),
            ("mods/keep.dll", "mod copy"),
        ]);
        // Windows: the ledger's spelling and the server's differ in case.
        d.case_insensitive = true;
        let owned: HashSet<String> = ["mods/loader.dll", "mods/extra.dll", "mods/gone.dll", "mods/keep.dll"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let p = run_with(
            &[
                base("Mods/Loader.dll", "v1", None),
                base("mods/gone.dll", "g", None),
                base("mods/keep.dll", "k", None),
            ],
            &[
                remote("Mods/Loader.dll", "v2"),
                remote("mods/extra.dll", "game now ships it"),
                remote("mods/keep.dll", "k"),
            ],
            &d,
            &never_protected,
            &owned,
        );
        assert!(p.conflicts().is_empty(), "{p:#?}");
        let written: Vec<&str> = p.writes.iter().map(|w| w.file.path.as_str()).collect();
        // In path order, case-insensitively.
        assert_eq!(written, vec!["mods/extra.dll", "Mods/Loader.dll"]);
        // Hand-over replaces the mod's copy, which may hold the player's
        // edits, so it is kept as .bak, without a conflict.
        assert!(p.writes.iter().all(|w| w.existing.is_some() && w.keep_bak && w.backup));
        assert_eq!(p.backup_paths(), vec!["Mods/Loader.dll".to_string(), "mods/extra.dll".to_string()]);
        // The dropped file stays the mod's, but its backups are discarded.
        assert!(p.deletes.is_empty());
        assert_eq!(p.dropped, vec!["mods/gone.dll".to_string()]);
        // Unchanged mod-owned file: untouched.
        assert!(p.kept.iter().any(|(f, _)| f.path == "mods/keep.dll"));
    }

    // ---- legacy installs ----

    #[test]
    fn with_an_empty_baseline_differences_conflict_and_nothing_is_removed() {
        let d = MemDisk::with(&[
            ("a.jar", "same"),
            ("b.jar", "old"),
            ("mine.jar", "player mod"),
        ]);
        let p = run(&[], &[remote("a.jar", "same"), remote("b.jar", "new"), remote("c.jar", "c")], &d);
        assert_eq!(
            p.conflicts(),
            vec![Conflict {
                path: "b.jar".into(),
                kind: ConflictKind::AddedExists
            }]
        );
        assert!(p.deletes.is_empty() && p.dropped.is_empty());
        assert_eq!(p.counts(), (1, 0, 0));
    }

    #[test]
    fn a_server_baseline_for_a_legacy_install_plans_normally() {
        let d = MemDisk::with(&[("a.jar", "v1"), ("old.jar", "o"), ("mine.jar", "player mod")]);
        let p = run(
            &[base("a.jar", "v1", None), base("old.jar", "o", None)],
            &[remote("a.jar", "v2")],
            &d,
        );
        assert!(p.conflicts().is_empty());
        assert_eq!(p.counts(), (0, 1, 1));
    }

    // ---- path matching ----

    #[test]
    fn backslashes_and_dot_segments_match_their_slash_spelling() {
        let d = MemDisk::with(&[("mods\\a.jar", "v1")]);
        let p = run(&[base("mods\\a.jar", "v1", None)], &[remote("./mods/a.jar", "v1")], &d);
        assert!(p.writes.is_empty() && p.deletes.is_empty(), "{p:#?}");
    }

    #[test]
    fn a_case_only_rename_is_one_file_not_an_add_and_a_delete() {
        // Case-insensitive disk (Windows): deleting "a.jar" would delete the
        // renamed file too.
        let mut d = MemDisk::with(&[("mods/a.jar", "v1")]);
        d.case_insensitive = true;
        let p = run(&[base("mods/a.jar", "v1", None)], &[remote("mods/A.jar", "v1")], &d);
        assert!(p.deletes.is_empty(), "{p:#?}");
        let w = only_write(&p);
        assert_eq!(w.file.path, "mods/A.jar");
        assert_eq!(w.existing.as_ref().unwrap().disk_path, "mods/a.jar");
        assert!(w.conflict.is_none());
    }

    #[test]
    fn on_a_case_sensitive_disk_a_case_only_rename_is_two_files() {
        // Linux: `a.jar` and `A.jar` are different files.
        let d = MemDisk::with(&[("mods/a.jar", "v1")]);
        let p = run(&[base("mods/a.jar", "v1", None)], &[remote("mods/A.jar", "v1")], &d);
        assert_eq!(p.deletes.len(), 1);
        assert_eq!(p.deletes[0].existing.disk_path, "mods/a.jar");
        assert_eq!(p.writes.len(), 1);
        assert!(p.writes[0].existing.is_none());
        assert!(p.conflicts().is_empty());
    }

    #[test]
    fn on_a_case_sensitive_disk_a_mod_claim_does_not_cover_a_differently_cased_file() {
        // The mod claims Config/x.cfg; the player's config/x.cfg is theirs.
        let d = MemDisk::with(&[("Config/x.cfg", "mod"), ("config/x.cfg", "player")]);
        let owned: HashSet<String> = ["Config/x.cfg".to_string()].into();
        let p = run_with(
            &[base("config/x.cfg", "v1", Some(1))],
            &[remote("config/x.cfg", "v2")],
            &d,
            &never_protected,
            &owned,
        );
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
    }

    #[test]
    fn two_files_differing_only_by_case_are_matched_exactly() {
        let d = MemDisk::with(&[("Readme.txt", "R"), ("README.txt", "r")]);
        let p = run(
            &[base("Readme.txt", "R", None), base("README.txt", "r", None)],
            &[remote("Readme.txt", "R"), remote("README.txt", "r2")],
            &d,
        );
        let w = only_write(&p);
        assert_eq!(w.file.path, "README.txt");
        assert_eq!(w.existing.as_ref().unwrap().disk_path, "README.txt");
        assert!(p.deletes.is_empty());
    }

    #[test]
    fn mod_ownership_is_matched_case_insensitively_on_windows_and_across_separators() {
        let mut d = MemDisk::with(&[("BepInEx\\plugins\\X.dll", "mod")]);
        d.case_insensitive = true;
        let owned: HashSet<String> = ["bepinex/plugins/x.dll".to_string()].into();
        let p = run_with(
            &[base("BepInEx\\plugins\\X.dll", "v1", None)],
            &[],
            &d,
            &never_protected,
            &owned,
        );
        assert!(p.deletes.is_empty());
    }

    #[test]
    fn unsafe_server_paths_are_refused() {
        for bad in ["../escape.txt", "a/../../b", "/etc/passwd", "\\\\server\\share", "C:/x", ""] {
            let r = plan(PlanInput {
                baseline: &[],
                target: &[remote(bad, "x")],
                disk: &MemDisk::default(),
                is_runtime_data: &never_protected,
                is_player_data: &never_protected,
                previously_shipped: &HashMap::new(),
                mod_owned: &HashSet::new(),
                case_insensitive: false,
                kept_mine: &HashSet::new(),
            });
            assert!(matches!(r, Err(PlanError::BadPath(_))), "{bad:?}");
        }
    }

    #[test]
    fn a_duplicate_path_is_refused() {
        let r = plan(PlanInput {
            baseline: &[],
            target: &[remote("a", "1"), remote("./a", "2")],
            disk: &MemDisk::default(),
            is_runtime_data: &never_protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: false,
            kept_mine: &HashSet::new(),
        });
        assert!(matches!(r, Err(PlanError::Duplicate(_))));
    }

    #[test]
    fn drops_own_files_are_never_planned() {
        let d = MemDisk::with(&[(".dropdata", "ledger")]);
        let p = run(
            &[base(".dropdata", "x", None)],
            &[remote(".drop-baseline.json", "x"), remote(".drop-update/new/a", "x")],
            &d,
        );
        assert_eq!(p, Plan::default());
    }

    // ---- unknown hashes ("" from the server) ----

    fn unknown_base(path: &str, size: u64, mtime: Option<u64>) -> BaselineFile {
        BaselineFile {
            path: path.into(),
            size,
            sha256: String::new(),
            mtime,
        }
    }

    fn unknown_remote(path: &str, size: u64) -> RemoteFile {
        RemoteFile {
            path: path.into(),
            size,
            sha256: String::new(),
        }
    }

    #[test]
    fn same_hash_never_matches_an_unknown_hash() {
        assert!(same_hash("AB", "ab"));
        assert!(!same_hash("", ""));
        assert!(!same_hash("", "ab"));
        assert!(!same_hash("ab", ""));
    }

    #[test]
    fn an_unknown_baseline_hash_means_the_update_wins_and_a_bak_is_kept() {
        // Even with size and mtime matching, "" can't vouch for the disk, and
        // the owner's rule for this case is "update wins, keep a .bak", not a
        // question per file.
        let d = MemDisk::with(&[("mods/a.jar", "v1")]); // mtime 1
        let p = run(&[unknown_base("mods/a.jar", 2, Some(1))], &[remote("mods/a.jar", "v2")], &d);
        let w = only_write(&p);
        assert!(w.conflict.is_none() && w.keep_bak && w.backup);
        assert_eq!(w.existing.as_ref().unwrap().expect, Expect::Present);
        assert!(p.conflicts().is_empty());
        assert_eq!(p.backup_paths(), vec!["mods/a.jar".to_string()]);
        assert_eq!(p.counts(), (0, 1, 0));
    }

    #[test]
    fn an_unknown_baseline_hash_on_a_removed_file_deletes_it_keeping_a_bak() {
        let d = MemDisk::with(&[("mods/old.jar", "o")]);
        let p = run(&[unknown_base("mods/old.jar", 1, Some(1))], &[], &d);
        assert_eq!(p.deletes.len(), 1);
        assert!(p.deletes[0].conflict.is_none() && p.deletes[0].keep_bak && p.deletes[0].backup);
        assert_eq!(p.backup_paths(), vec!["mods/old.jar".to_string()]);
        assert_eq!(p.counts(), (0, 0, 1));
        // Already gone: nothing to do.
        let p = run(&[unknown_base("mods/old.jar", 1, None)], &[], &MemDisk::default());
        assert!(p.deletes.is_empty());
    }

    #[test]
    fn an_unknown_baseline_hash_still_sees_the_new_content_already_on_disk() {
        let d = MemDisk::with(&[("mods/a.jar", "v2")]);
        let p = run(&[unknown_base("mods/a.jar", 2, None)], &[remote("mods/a.jar", "v2")], &d);
        assert!(p.writes.is_empty() && p.deletes.is_empty());
    }

    #[test]
    fn two_unknown_hashes_are_not_the_same_file_content() {
        let d = MemDisk::with(&[("a.pak", "x")]);
        let p = run(&[unknown_base("a.pak", 1, Some(1))], &[unknown_remote("a.pak", 1)], &d);
        let w = only_write(&p);
        assert!(w.conflict.is_none() && w.backup);
        // Missing on disk: just download it.
        let p = run(&[unknown_base("a.pak", 1, None)], &[unknown_remote("a.pak", 1)], &MemDisk::default());
        let w = only_write(&p);
        assert!(w.conflict.is_none() && !w.backup && w.existing.is_none());
    }

    #[test]
    fn an_unknown_target_hash_over_an_existing_file_is_a_conflict() {
        let d = MemDisk::with(&[("new.cfg", "x")]);
        let p = run(&[], &[unknown_remote("new.cfg", 1)], &d);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::AddedExists));
        let p = run(&[], &[unknown_remote("new.cfg", 1)], &MemDisk::default());
        assert!(only_write(&p).conflict.is_none());
    }


    // ---- a file becomes a folder, and back ----

    #[test]
    fn a_file_that_becomes_a_folder_is_removed_first() {
        // Linux reads data/x under the file `data` as missing (ENOTDIR).
        let d = MemDisk::with(&[("data", "old file")]);
        let p = run(&[base("data", "old file", None)], &[remote("data/x", "x")], &d);
        assert_eq!(p.deletes.len(), 1);
        assert!(p.deletes[0].conflict.is_none() && !p.deletes[0].keep_bak);
        let w = only_write_of(&p);
        assert!(w.existing.is_none() && w.conflict.is_none());
    }

    fn only_write_of(p: &Plan) -> &WriteOp {
        assert_eq!(p.writes.len(), 1, "{p:#?}");
        &p.writes[0]
    }

    #[test]
    fn an_edited_file_in_the_way_of_a_new_folder_is_kept_as_bak_not_asked() {
        let d = MemDisk::with(&[("data", "player edit")]);
        let p = run(&[base("data", "old file", None)], &[remote("data/x", "x")], &d);
        assert!(p.conflicts().is_empty(), "keeping it would block the folder");
        assert!(p.deletes[0].keep_bak && p.deletes[0].backup);
        // A player's own file (in neither list) in the way: same.
        let d = MemDisk::with(&[("data", "player file")]);
        let p = run(&[], &[remote("data/x", "x")], &d);
        assert_eq!(p.deletes.len(), 1);
        assert_eq!(p.backup_paths(), vec!["data".to_string()]);
    }

    #[test]
    fn protected_data_in_the_way_of_a_new_folder_stops_the_plan() {
        let d = MemDisk::with(&[("drop-saves/slot", "progress")]);
        let r = plan(PlanInput {
            baseline: &[],
            target: &[remote("drop-saves/slot/new.dat", "x")],
            disk: &d,
            is_runtime_data: &saves_protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: false,
            kept_mine: &HashSet::new(),
        });
        assert!(matches!(r, Err(PlanError::InTheWay(ref p)) if p == "drop-saves/slot"), "{r:?}");
    }

    #[test]
    fn a_folder_that_becomes_a_file_makes_way_once_its_files_go() {
        let d = MemDisk::with(&[("data/x", "x1"), ("data/y", "y1")]);
        let p = run(
            &[base("data/x", "x1", None), base("data/y", "y1", None)],
            &[remote("data", "now a file")],
            &d,
        );
        assert_eq!(p.deletes.len(), 2);
        let w = only_write_of(&p);
        assert!(w.conflict.is_none() && !w.keep_bak);
        assert_eq!(w.existing.as_ref().unwrap().disk_path, "data");
        // The commit re-checks that nothing was added to it since.
        assert!(matches!(&w.existing.as_ref().unwrap().expect, Expect::Folder { files } if files.len() == 2));
    }

    #[test]
    fn a_folder_that_becomes_a_file_but_holds_player_files_stops_the_plan() {
        let d = MemDisk::with(&[("data/x", "x1"), ("data/mine.txt", "player")]);
        let r = plan(PlanInput {
            baseline: &[base("data/x", "x1", None)],
            target: &[remote("data", "now a file")],
            disk: &d,
            is_runtime_data: &never_protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: false,
            kept_mine: &HashSet::new(),
        });
        assert!(matches!(r, Err(PlanError::InTheWay(ref p)) if p == "data"), "{r:?}");
    }

    // ---- player-data folders and leftovers ----
    //
    // The field case (6.1.0/6.1.1, a Minecraft pack): every pack file lives
    // under user/instances/<pack>/minecraft/. `user/` was treated as an
    // emulator's save folder, so a changed jar was silently kept and jars the
    // pack removed were never deleted, then dropped from the baseline. The
    // player ended up with two versions of the same mod.

    use crate::downloads::download_agent::{is_drop_runtime_data, is_player_data_folder};

    fn mc(name: &str) -> String {
        format!("user/instances/P/minecraft/mods/{name}")
    }

    fn run_real(
        b: &[BaselineFile],
        t: &[RemoteFile],
        d: &MemDisk,
        prev: &HashMap<String, HashSet<(u64, String)>>,
    ) -> Plan {
        run_real_kept(b, t, d, prev, &[])
    }

    fn run_real_kept(
        b: &[BaselineFile],
        t: &[RemoteFile],
        d: &MemDisk,
        prev: &HashMap<String, HashSet<(u64, String)>>,
        kept_mine: &[&str],
    ) -> Plan {
        let kept: HashSet<String> = kept_mine.iter().map(|s| s.to_string()).collect();
        plan(PlanInput {
            baseline: b,
            target: t,
            disk: d,
            is_runtime_data: &is_drop_runtime_data,
            is_player_data: &is_player_data_folder,
            mod_owned: &HashSet::new(),
            case_insensitive: d.case_insensitive,
            kept_mine: &kept,
            previously_shipped: prev,
        })
        .expect("plan")
    }

    fn shipped(entries: &[(&str, &[&str])]) -> HashMap<String, HashSet<(u64, String)>> {
        entries
            .iter()
            .map(|(p, contents)| (p.to_string(), contents.iter().map(|c| (c.len() as u64, h(c))).collect()))
            .collect()
    }

    fn disk_of(files: &[(&str, &str)]) -> MemDisk {
        let mut d = MemDisk::default();
        for (p, c) in files {
            d.files.insert(p.to_string(), (c.to_string(), Some(1)));
        }
        d
    }

    #[test]
    fn a_changed_pack_jar_under_user_with_a_verified_baseline_is_replaced() {
        let jar = mc("sodium.jar");
        let d = disk_of(&[(&jar, "sodium-1")]);
        let p = run_real(&[base(&jar, "sodium-1", Some(1))], &[remote(&jar, "sodium-2")], &d, &HashMap::new());
        let w = only_write(&p);
        assert!(w.conflict.is_none() && !w.backup && !w.keep_bak, "{w:#?}");
        assert_eq!(w.existing.as_ref().unwrap().disk_path, jar);
        assert_eq!(p.counts(), (0, 1, 0));
        // A server baseline (no mtime) whose hash matches the disk is the
        // pack's own copy too: replaced, nothing asked.
        let p = run_real(&[base(&jar, "sodium-1", None)], &[remote(&jar, "sodium-2")], &d, &HashMap::new());
        assert!(only_write(&p).conflict.is_none() && p.backup_paths().is_empty());
    }

    #[test]
    fn a_jar_removed_from_the_pack_under_user_is_deleted() {
        let (old, new) = (mc("multiverse_gates-1.1.0.jar"), mc("multiverse_gates-1.2.0.jar"));
        let d = disk_of(&[(&old, "gates-110")]);
        let p = run_real(&[base(&old, "gates-110", Some(1))], &[remote(&new, "gates-120")], &d, &HashMap::new());
        assert_eq!(p.deletes.len(), 1, "{p:#?}");
        let del = &p.deletes[0];
        assert_eq!(del.existing.disk_path, old);
        assert!(del.conflict.is_none() && !del.keep_bak && !del.backup);
        assert_eq!(p.dropped, vec![old.clone()]);
        assert!(p.writes.iter().any(|w| w.file.path == new && w.added));
        assert_eq!(p.counts(), (1, 0, 1));
    }

    #[test]
    fn an_unverified_pack_file_under_user_that_differs_is_asked_about_not_replaced() {
        let jar = mc("sodium.jar");
        let d = disk_of(&[(&jar, "sodium-old-build")]);
        // Changed by the update.
        let p = run_real(&[base(&jar, "sodium-1", None)], &[remote(&jar, "sodium-2")], &d, &HashMap::new());
        let w = only_write(&p);
        assert_eq!(w.conflict, Some(ConflictKind::ChangedBoth), "{w:#?}");
        assert!(!w.backup && !w.keep_bak);
        assert!(p.backup_paths().is_empty());
        // Not changed by the update, but different on disk: left alone (see
        // the test below).
        let p = run_real(&[base(&jar, "sodium-1", None)], &[remote(&jar, "sodium-1")], &d, &HashMap::new());
        assert!(p.writes.is_empty() && p.conflicts().is_empty(), "{p:#?}");
        // Removed by the update.
        let p = run_real(&[base(&jar, "sodium-1", None)], &[], &d, &HashMap::new());
        assert_eq!(p.deletes.len(), 1);
        assert_eq!(p.deletes[0].conflict, Some(ConflictKind::RemovedEdited));
        assert!(!p.deletes[0].backup && !p.deletes[0].keep_bak);
        assert_eq!(p.counts(), (0, 0, 0));
        // An unknown baseline hash ("") can't vouch for the disk either.
        let p = run_real(&[unknown_base(&jar, 16, Some(1))], &[], &d, &HashMap::new());
        assert_eq!(p.deletes[0].conflict, Some(ConflictKind::RemovedEdited));
        let p = run_real(&[unknown_base(&jar, 16, Some(1))], &[remote(&jar, "sodium-2")], &d, &HashMap::new());
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
        // Added over a file already there: asked, as anywhere.
        let p = run_real(&[], &[remote(&jar, "sodium-2")], &d, &HashMap::new());
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::AddedExists));
        // Outside the player-data folders the old rule stands: replaced,
        // .bak kept, nothing asked.
        let d = disk_of(&[("mods/sodium.jar", "sodium-old-build")]);
        let p = run_real(
            &[base("mods/sodium.jar", "sodium-1", None)],
            &[remote("mods/sodium.jar", "sodium-2")],
            &d,
            &HashMap::new(),
        );
        assert!(only_write(&p).backup && p.conflicts().is_empty());
    }

    #[test]
    fn an_unchanged_unverified_file_under_user_that_differs_is_kept_silently() {
        // Eden rewrites the qt-config.ini the pack ships with the player's
        // bindings. A server baseline (no mtime) can't tell that from an old
        // copy; under a player-data folder the update leaves it alone, as
        // before player-data folders were updated at all.
        let cfg = "user/config/qt-config.ini";
        let d = disk_of(&[(cfg, "player's bindings")]);
        let p = run_real(&[base(cfg, "pack default", None)], &[remote(cfg, "pack default")], &d, &HashMap::new());
        assert!(p.writes.is_empty() && p.deletes.is_empty() && p.conflicts().is_empty(), "{p:#?}");
        assert_eq!(p.kept, vec![(remote(cfg, "pack default"), None)]);
        // Outside the player-data folders the old rule stands.
        let d = disk_of(&[("config/qt-config.ini", "player's bindings")]);
        let p = run_real(
            &[base("config/qt-config.ini", "pack default", None)],
            &[remote("config/qt-config.ini", "pack default")],
            &d,
            &HashMap::new(),
        );
        assert!(only_write(&p).backup);
    }

    #[test]
    fn a_linked_player_data_folder_gets_no_operations_and_the_rest_updates() {
        // On the Deck user/ is often a link to the SD card.
        let (changed, removed, added, same, verified) =
            (mc("a.jar"), mc("b.jar"), mc("c.jar"), mc("d.jar"), mc("e.jar"));
        for link in ["user", "user/instances/P"] {
            let mut d = disk_of(&[
                (&changed, "a1"),
                (&removed, "b1"),
                (&same, "d-player"),
                (&verified, "e1"),
                ("Game.exe", "e1"),
            ]);
            d.links.insert(link.into());
            let b = [
                base(&changed, "a1", Some(1)),
                base(&removed, "b1", Some(1)),
                base(&same, "d1", None),
                base(&verified, "e1", Some(1)),
                base("Game.exe", "e1", Some(1)),
            ];
            let t = [
                remote(&changed, "a2"),
                remote(&added, "c"),
                remote(&same, "d1"),
                remote(&verified, "e1"),
                remote("Game.exe", "e2"),
            ];
            let p = run_real(&b, &t, &d, &shipped(&[(&mc("old.jar"), &["o"])]));
            assert_eq!(only_write(&p).file.path, "Game.exe", "{link}: {p:#?}");
            assert!(p.dropped.is_empty() && p.conflicts().is_empty());
            assert_eq!(p.skipped_linked, vec![link.to_string()]);
            // Healing was due and `same` (never checked) could not be.
            assert!(p.heal_incomplete, "{link}: {p:#?}");
            // The next baseline keeps what the old one said there (the
            // removed file too), and nothing for the file never written.
            let next: HashMap<&str, (&str, Option<u64>)> =
                p.kept.iter().map(|(f, m)| (f.path.as_str(), (f.sha256.as_str(), *m))).collect();
            assert_eq!(next[changed.as_str()], (h("a1").as_str(), Some(1)));
            assert_eq!(next[removed.as_str()], (h("b1").as_str(), Some(1)));
            assert_eq!(next[same.as_str()], (h("d1").as_str(), None));
            assert!(!next.contains_key(added.as_str()));
            // Nothing to do under the link: not reported as skipped.
            let p = run_real(&b[3..], &t[3..], &d, &HashMap::new());
            assert!(p.skipped_linked.is_empty() && !p.heal_incomplete, "{p:#?}");
            // An unchanged file never checked on this disk is not the
            // update's work either; only healing, when due, misses it.
            let p = run_real(&b[2..], &t[2..], &d, &HashMap::new());
            assert!(p.skipped_linked.is_empty() && !p.heal_incomplete, "{p:#?}");
            let p = run_real(&b[2..], &t[2..], &d, &shipped(&[(&mc("old.jar"), &["o"])]));
            assert!(p.skipped_linked.is_empty() && p.heal_incomplete, "{p:#?}");
        }
        // Elsewhere a linked folder still stops the plan.
        let mut d = disk_of(&[("Data/a.pak", "v1")]);
        d.links.insert("Data".into());
        let r = plan(PlanInput {
            baseline: &[base("Data/a.pak", "v1", None)],
            target: &[remote("Data/a.pak", "v2")],
            disk: &d,
            is_runtime_data: &is_drop_runtime_data,
            is_player_data: &is_player_data_folder,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: false,
            kept_mine: &HashSet::new(),
        });
        assert!(matches!(r, Err(PlanError::Linked(_))), "{r:?}");
    }

    #[test]
    fn a_leftover_pack_jar_from_an_earlier_revision_is_removed() {
        // Revisions 1 and 2 shipped gates 1.1.0 and 1.2.0; 6.1.x never
        // removed either and dropped both from the baseline. The baseline
        // and target now only know 1.3.0 / 1.4.0.
        let (g110, g120, g130, g140) = (
            mc("multiverse_gates-1.1.0.jar"),
            mc("multiverse_gates-1.2.0.jar"),
            mc("multiverse_gates-1.3.0.jar"),
            mc("multiverse_gates-1.4.0.jar"),
        );
        let (mine, rebuilt) = (mc("players-own.jar"), mc("rebuilt.jar"));
        let d = disk_of(&[
            (&g110, "gates-110"),
            (&g120, "gates-120"),
            (&g130, "gates-130"),
            (&mine, "player's mod"),
            // Same name as an earlier pack file, but the player's content.
            (&rebuilt, "player's rebuild"),
        ]);
        let prev = shipped(&[
            (&g110, &["gates-110"]),
            (&g120, &["gates-120"]),
            (&g130, &["gates-130"]),
            (&rebuilt, &["pack build"]),
            (&mc("gone-from-disk.jar"), &["x"]),
        ]);
        let p = run_real(&[base(&g130, "gates-130", Some(1))], &[remote(&g140, "gates-140")], &d, &prev);
        let mut deleted: Vec<&str> = p.deletes.iter().map(|d| d.existing.disk_path.as_str()).collect();
        deleted.sort();
        assert_eq!(deleted, vec![g110.as_str(), g120.as_str(), g130.as_str()], "{p:#?}");
        assert!(p.deletes.iter().all(|d| d.conflict.is_none()));
        // Leftovers are set aside as .bak (it may be the player's own copy of
        // the same file); the baseline's own removal is a plain delete.
        assert_eq!(p.backup_paths(), vec![g110.clone(), g120.clone()]);
        // Commit re-checks the leftover is still that exact file.
        let leftover = p.deletes.iter().find(|d| d.existing.disk_path == g110).unwrap();
        assert!(matches!(&leftover.existing.expect, Expect::Content { sha256, .. } if *sha256 == h("gates-110")));
        assert!(p.conflicts().is_empty());
        assert_eq!(p.counts(), (1, 0, 3));
        // The player's own jar and the rebuilt one are never deleted, and the
        // player's own (named in no revision) is never even read.
        assert!(!deleted.contains(&mine.as_str()) && !deleted.contains(&rebuilt.as_str()));
        assert!(!d.hashed.borrow().contains(&mine));
    }

    #[test]
    fn leftovers_skip_unknown_hashes_links_and_files_the_lists_still_name() {
        let jar = mc("a.jar");
        // Unknown hash: never matches.
        let d = disk_of(&[(&jar, "a")]);
        let mut prev: HashMap<String, HashSet<(u64, String)>> = HashMap::new();
        prev.insert(jar.clone(), [(1, String::new())].into());
        assert!(run_real(&[], &[], &d, &prev).deletes.is_empty());
        // Through a linked folder: left alone (and the plan is not refused).
        let mut d = disk_of(&[(&jar, "a")]);
        d.links.insert("user/instances/P".into());
        let p = run_real(&[], &[remote("Game.exe", "e")], &d, &shipped(&[(&jar, &["a"])]));
        assert!(p.deletes.is_empty(), "{p:#?}");
        // A link itself is not a regular file.
        let mut d = MemDisk::default();
        d.links.insert(jar.clone());
        assert!(run_real(&[], &[], &d, &shipped(&[(&jar, &["a"])])).deletes.is_empty());
        // Windows: the target names the same file in another case.
        let mut d = disk_of(&[(&jar, "a")]);
        d.case_insensitive = true;
        let upper = jar.to_uppercase();
        let p = run_real(&[], &[remote(&upper, "a")], &d, &shipped(&[(&jar, &["a"])]));
        assert!(p.deletes.is_empty(), "{p:#?}");
        // Linux with an exFAT SD card: case_insensitive is false but the
        // disk is not. Another case in T is still the same file.
        let d = disk_of(&[(&jar, "a")]);
        let other_case = jar.replace("a.jar", "A.jar");
        let p = run_real(&[], &[remote(&other_case, "A")], &d, &shipped(&[(&jar, &["a"])]));
        assert!(p.deletes.is_empty(), "{p:#?}");
        // Empty files all share one hash: never taken for the pack's.
        let d = disk_of(&[(&jar, "")]);
        assert!(run_real(&[], &[], &d, &shipped(&[(&jar, &[""])])).deletes.is_empty());
        // Outside the player-data folders: never a leftover.
        let d = disk_of(&[("mods/a.jar", "a")]);
        assert!(run_real(&[], &[], &d, &shipped(&[("mods/a.jar", &["a"])])).deletes.is_empty());
    }

    #[test]
    fn drop_runtime_data_is_still_never_replaced_deleted_or_cleaned_up() {
        let (gbe, settings, nested) = (
            "Binaries/Win64/drop-goldberg/480/achievements.json",
            "steam_settings/configs.user.ini",
            "user/drop-goldberg/480/remote/save.sav",
        );
        let d = disk_of(&[(gbe, "earned"), (settings, "written at launch"), (nested, "progress")]);
        let p = run_real(
            &[base(gbe, "shipped", None), base(settings, "shipped", None), base(nested, "shipped", None)],
            &[remote(gbe, "shipped v2"), remote(nested, "shipped")],
            &d,
            &shipped(&[(gbe, &["earned"]), (settings, &["written at launch"])]),
        );
        assert!(p.writes.is_empty() && p.deletes.is_empty() && p.dropped.is_empty(), "{p:#?}");
        assert!(d.hashed.borrow().is_empty());
        // A leftover that matches an earlier revision is not removed either.
        let leftover = "Binaries/Win64/steam_settings/old.txt";
        let d = disk_of(&[(leftover, "x")]);
        assert!(run_real(&[], &[], &d, &shipped(&[(leftover, &["x"])])).deletes.is_empty());
    }

    // ---- a copy an earlier revision shipped is the pack's own ----

    #[test]
    fn an_old_same_name_jar_6_1_kept_is_replaced_without_a_conflict() {
        // 6.1.x kept the revision 1 jar on disk but recorded revision 2's
        // hash, without an mtime. The next update must fix it, not ask.
        let jar = mc("sodium.jar");
        let d = disk_of(&[(&jar, "sodium-rev1-build")]);
        let prev = shipped(&[(&jar, &["sodium-rev1-build", "sodium-2"])]);
        let b = [base(&jar, "sodium-2", None)];
        for t in [remote(&jar, "sodium-2"), remote(&jar, "sodium-3")] {
            let p = run_real(&b, std::slice::from_ref(&t), &d, &prev);
            let w = only_write(&p);
            assert!(w.conflict.is_none() && !w.backup && !w.keep_bak, "{w:#?}");
            assert!(
                matches!(&w.existing.as_ref().unwrap().expect, Expect::Content { sha256, .. } if *sha256 == h("sodium-rev1-build"))
            );
            assert!(p.backup_paths().is_empty());
            assert_eq!(p.counts(), (0, 1, 0));
        }
        // Removed by the target: deleted, no question.
        let p = run_real(&b, &[], &d, &prev);
        assert_eq!(p.deletes.len(), 1);
        assert!(p.deletes[0].conflict.is_none() && !p.deletes[0].keep_bak);
        // Added back by the target over the old copy: replaced.
        let p = run_real(&[], &[remote(&jar, "sodium-3")], &d, &prev);
        let w = only_write(&p);
        assert!(w.added && w.conflict.is_none() && !w.backup, "{w:#?}");
    }

    #[test]
    fn an_earlier_copy_never_overrides_a_verified_baseline_or_a_normal_path() {
        // Verified on this disk, then rolled back by the player to an older
        // shipped build: the player's choice, so the table decides.
        let jar = mc("sodium.jar");
        let d = disk_of(&[(&jar, "sodium-rev1")]);
        let prev = shipped(&[(&jar, &["sodium-rev1"])]);
        let p = run_real(&[base(&jar, "sodium-r2", Some(7))], &[remote(&jar, "sodium-r3")], &d, &prev);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
        let p = run_real(&[base(&jar, "sodium-r2", Some(7))], &[], &d, &prev);
        assert_eq!(p.deletes[0].conflict, Some(ConflictKind::RemovedEdited));
        // 6.1.x never held back files outside the player-data folders: the
        // usual unverified rule (.bak kept) applies there.
        let d = disk_of(&[("config/a.cfg", "pack rev1")]);
        let p = run_real(
            &[base("config/a.cfg", "pack rev2", None)],
            &[remote("config/a.cfg", "pack rev3")],
            &d,
            &shipped(&[("config/a.cfg", &["pack rev1"])]),
        );
        assert!(only_write(&p).backup && p.conflicts().is_empty());
    }

    #[test]
    fn a_player_edit_matching_no_shipped_revision_is_still_a_conflict() {
        let jar = mc("sodium.jar");
        let d = disk_of(&[(&jar, "player's own build")]);
        let prev = shipped(&[(&jar, &["sodium-rev1-build"])]);
        let p = run_real(&[base(&jar, "sodium-2", None)], &[remote(&jar, "sodium-3")], &d, &prev);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
        let p = run_real(&[base(&jar, "sodium-2", None)], &[], &d, &prev);
        assert_eq!(p.deletes[0].conflict, Some(ConflictKind::RemovedEdited));
    }

    #[test]
    fn an_earlier_copy_is_not_the_packs_for_runtime_data_kept_files_or_unknown_hashes() {
        // Drop runtime data: never touched.
        let gbe = "drop-goldberg/480/achievements.json";
        let d = disk_of(&[(gbe, "rev1")]);
        let p = run_real(&[base(gbe, "rev2", None)], &[remote(gbe, "rev3")], &d, &shipped(&[(gbe, &["rev1"])]));
        assert!(p.writes.is_empty(), "{p:#?}");
        // A file the player chose to keep: still theirs, still asked.
        let cfg = "user/config/a.cfg";
        let d = disk_of(&[(cfg, "rev1")]);
        let prev = shipped(&[(cfg, &["rev1"])]);
        let p = run_real_kept(&[base(cfg, "rev2", None)], &[remote(cfg, "rev3")], &d, &prev, &[cfg]);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
        let p = run_real_kept(&[base(cfg, "rev2", None)], &[remote(cfg, "rev2")], &d, &prev, &[cfg]);
        assert!(p.writes.is_empty(), "{p:#?}");
        // An unknown earlier hash matches nothing.
        let mut prev: HashMap<String, HashSet<(u64, String)>> = HashMap::new();
        prev.insert(cfg.into(), [(4, String::new())].into());
        let p = run_real(&[base(cfg, "rev2", None)], &[remote(cfg, "rev3")], &d, &prev);
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
        // Nor does an empty file.
        let d = disk_of(&[(cfg, "")]);
        let p = run_real(&[base(cfg, "rev2", None)], &[remote(cfg, "rev3")], &d, &shipped(&[(cfg, &[""])]));
        assert_eq!(only_write(&p).conflict, Some(ConflictKind::ChangedBoth));
    }

    #[test]
    fn a_removed_file_the_player_kept_is_never_cleaned_up_later() {
        let jar = mc("gates-1.1.0.jar");
        let d = disk_of(&[(&jar, "gates-110")]);
        // Same content as revision 1's jar, but the player kept it.
        let prev = shipped(&[(&jar, &["gates-110"])]);
        let p = run_real_kept(&[], &[remote(&mc("gates-1.2.0.jar"), "gates-120")], &d, &prev, &[&jar]);
        assert!(p.deletes.is_empty(), "{p:#?}");
        let p = run_real(&[], &[remote(&mc("gates-1.2.0.jar"), "gates-120")], &d, &prev);
        assert_eq!(p.deletes.len(), 1, "without the record it is a leftover");
    }

    #[test]
    fn keep_mine_on_a_removed_file_is_recorded() {
        let (p, _) = conflicts_plan();
        let r: HashMap<String, Resolution> = ["added.cfg", "changed.cfg", "removed.cfg"]
            .iter()
            .map(|p| (p.to_string(), Resolution::KeepMine))
            .collect();
        let res = resolve(p, &r).unwrap();
        assert_eq!(res.kept_mine, vec!["added.cfg", "changed.cfg", "removed.cfg"]);
        assert!(res.next_baseline.iter().all(|(f, _)| f.path != "removed.cfg"));
    }

    #[test]
    fn a_switch_save_under_user_that_no_list_names_is_untouched() {
        let save = "user/nand/user/save/0000000000000000/slot.bin";
        let d = disk_of(&[("Eden.exe", "e1"), (save, "progress")]);
        let p = run_real(&[base("Eden.exe", "e1", Some(1))], &[remote("Eden.exe", "e2")], &d, &HashMap::new());
        assert_eq!(only_write(&p).file.path, "Eden.exe");
        assert!(!d.hashed.borrow().iter().any(|p| p == save));
    }

    // ---- links ----

    #[test]
    fn a_change_through_a_linked_folder_is_refused_before_downloading() {
        let mut d = MemDisk::with(&[("Data/a.pak", "v1")]);
        d.links.insert("Data".into());
        let r = plan(PlanInput {
            baseline: &[base("Data/a.pak", "v1", None)],
            target: &[remote("Data/a.pak", "v2"), remote("Game.exe", "e")],
            disk: &d,
            is_runtime_data: &never_protected,
            is_player_data: &never_protected,
            previously_shipped: &HashMap::new(),
            mod_owned: &HashSet::new(),
            case_insensitive: false,
            kept_mine: &HashSet::new(),
        });
        assert!(matches!(r, Err(PlanError::Linked(ref p)) if p == "Data"), "{r:?}");
        // Files under the link the update does not change are fine.
        let p = run(&[base("Data/a.pak", "v1", None)], &[remote("Data/a.pak", "v1"), remote("Game.exe", "e")], &d);
        assert_eq!(only_write_of(&p).file.path, "Game.exe");
    }

}
