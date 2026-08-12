//! Local save-file discovery and write-back.
//!
//! Two save sources are scanned:
//!
//! * **Emulator saves** — RetroArch keeps them under `{emu_root}/drop-saves/
//!   {game_id}/{saves,states}`; [`scan_emu_saves`] walks those directories.
//! * **PC saves** — discovered by shelling out to Ludusavi, whose database
//!   knows where each game stores its saves; [`scan_pc_saves`].
//!
//! [`write_downloaded_save`] / [`write_downloaded_pc_save`] put cloud copies
//! back. Every destructive write here goes through [`super::backup`], which
//! takes a checked, timestamped backup first and replaces the file atomically.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use log::{info, warn};
use once_cell::sync::Lazy;
use serde::Serialize;

use super::LocalSaveFile;
use super::backup;

/// The directory, inside an emulator install, that Drop owns for save data.
pub const DROP_SAVES_DIR: &str = "drop-saves";

/// The one place a game's emulator save directory is spelled out:
/// `{emu_root}/drop-saves/{user_id}/{game_id}`.
///
/// This is the *target* layout. Call sites that touch real save bytes go
/// through [`super::scope::resolve_emu_saves_root`] instead, which returns
/// this path once the one-time move into it has finished and the legacy
/// directory until then. Using this function directly on a launch path is how
/// a game ends up booting from an empty directory while its real save sits
/// unmigrated next door.
///
/// `user_id: None` is the signed-out layout `{emu_root}/drop-saves/{game_id}`,
/// byte-identical to what every build before per-user scoping wrote. Saves
/// there are never synced (see the sign-in guards on the sync paths); the
/// migration adopts the tree into the first account that claims it.
pub fn emu_saves_root(emu_root: &Path, user_id: Option<&str>, game_id: &str) -> PathBuf {
    let base = emu_root.join(DROP_SAVES_DIR);
    match user_id {
        Some(user_id) => base.join(user_id).join(game_id),
        None => base.join(game_id),
    }
}

/// Filename prefix that namespaces PC saves away from emulator saves.
///
/// Must be free of path separators: the server sanitizes upload filenames with
/// `sanitize-filename`, which strips `/` and `\`. The legacy `"pc/"` prefix was
/// mangled to `"pc"` on the server (`"pc/gen.sav"` → `"pcgen.sav"`), so the same
/// save no longer matched its local counterpart. `"pc__"` survives sanitization
/// intact, so a PC save keeps one stable identity across the upload round-trip.
pub const PC_SAVE_PREFIX: &str = "pc__";

/// Strip the PC-save namespace prefix to recover the encoded body.
///
/// Accepts the current `"pc__"` prefix and the legacy `"pc/"` prefix so saves
/// uploaded before the change still restore correctly. For a legacy row the
/// body IS the on-disk basename; for a row written by [`encode_pc_filename`]
/// it is an escaped relative path — run it through [`decode_pc_relpath`] when
/// you need a path rather than a display string.
pub fn strip_pc_prefix(filename: &str) -> &str {
    filename
        .strip_prefix(PC_SAVE_PREFIX)
        .or_else(|| filename.strip_prefix("pc/"))
        .unwrap_or(filename)
}

/// Whether a cloud filename was written by [`encode_pc_filename`].
///
/// The emulator sync path uses this to leave PC rows alone: a game can have
/// both kinds (Drop runs Ludusavi for emulator titles too when the user asks
/// it to), and a PC save restored into `drop-saves/…/saves` is in a directory
/// the game does not read. The server applies the same rule to decide which
/// rows are shareable across accounts, so the two definitions must agree.
pub fn is_pc_namespaced_filename(filename: &str) -> bool {
    filename.starts_with(PC_SAVE_PREFIX) || filename.starts_with("pc/")
}

/// Encode a PC save's path relative to `save_root` (the deepest directory all
/// of the game's discovered saves share), namespaced with [`PC_SAVE_PREFIX`].
///
/// Keying on the basename alone collapsed `slot1/save.dat` and
/// `slot2/save.dat` into one cloud row — the server key is
/// `(gameId, userId, filename)` — so whichever uploaded last won and a restore
/// put it back in the wrong slot.
///
/// When every save sits in one directory (by far the common case) the relative
/// path IS the basename and the encoded name is byte-identical to what earlier
/// builds wrote, so existing cloud rows keep matching. The encoding only kicks
/// in for games whose saves span subdirectories, which are exactly the ones
/// that were broken.
pub fn encode_pc_filename(save_root: Option<&Path>, path: &Path) -> String {
    let rel = save_root
        .and_then(|root| path.strip_prefix(root).ok())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(path.file_name().unwrap_or_default()));
    format!("{PC_SAVE_PREFIX}{}", escape_relpath(&rel))
}

/// Decode a PC cloud filename back to a path relative to the game's save root.
///
/// A legacy row (bare basename, with or without a prefix) contains no escape
/// sequences, so it decodes to itself and restores exactly where it always
/// did. Returns `None` for anything that would escape the save root.
pub fn decode_pc_relpath(filename: &str) -> Option<PathBuf> {
    unescape_relpath(strip_pc_prefix(filename))
}

/// The deepest directory every path in `paths` lives under, used as the anchor
/// for [`encode_pc_filename`].
///
/// Ludusavi reports absolute paths, and a game's saves usually share a single
/// folder — in that case this returns that folder and relative paths collapse
/// to basenames. It is deliberately derived from the scan rather than pinned
/// somewhere: there is no per-game "save root" in Ludusavi's API output. The
/// cost is that a game whose saves span two unrelated roots re-anchors if one
/// root appears or disappears between scans; the benefit is that two saves in
/// different subfolders stop overwriting each other in the cloud.
pub fn common_save_root(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut iter = paths.iter();
    let mut shared: Vec<std::path::Component<'_>> =
        iter.next()?.parent()?.components().collect();
    for path in iter {
        let parts: Vec<std::path::Component<'_>> = path.parent()?.components().collect();
        let keep = shared
            .iter()
            .zip(parts.iter())
            .take_while(|(a, b)| a == b)
            .count();
        shared.truncate(keep);
        if shared.is_empty() {
            return None;
        }
    }
    Some(shared.iter().collect())
}

/// Compute the MD5 hash of a file on disk.
pub fn md5_file(path: &Path) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let digest = md5::compute(&data);
    Ok(format!("{:x}", digest))
}

/// The one directory under a yuzu-family emulator's portable root that holds
/// per-title save data, relative to the emulator install root.
///
/// Everything lives under the portable `user/` directory (see
/// `crate::switchemu::discovery`). Under this root the emulator nests saves as
/// `<space>/<user>/<titleid>` today and as `account/<user>/<titleid>` +
/// `device/<titleid>` in the layout the save-data factory prefers when it
/// already exists (eden `src/core/file_sys/savedata_factory.cpp:26-54,
/// 108-148`), which is why [`switch_title_save_dirs`] searches for the title
/// directory rather than hard-coding one depth.
///
/// `nand/system/save` and `sdmc` used to be swept too, and `nand/temp` was
/// deliberately absent. All three are gone now: the scan is tagged with
/// whichever gameId triggered the launch, so anything outside the launched
/// title's own directory ends up filed under the wrong game — every Switch
/// game's cloud rows held every other Switch game's saves, and launching game
/// A restored its snapshot of the whole NAND over game B's current saves.
const SWITCH_SAVE_ROOT: &str = "user/nand/user/save";

/// How deep under [`SWITCH_SAVE_ROOT`] a title-id directory may sit.
/// `<space>/<user>/<titleid>` and `account/<user>/<titleid>` are 3;
/// `device/<titleid>` is 2. Anything deeper is not a save-data root.
const SWITCH_TITLE_DIR_MAX_DEPTH: usize = 3;

/// Pull a Switch title id out of a ROM path (or any string containing one).
///
/// Dumps almost universally carry the id in the filename — `Game
/// [0100ABCDEF012000][v0].nsp` — and Drop has no other source for it: nothing
/// in the library metadata records a title id, and reading it out of an NSP
/// means parsing PFS0 plus an encrypted NCA header.
///
/// The match is deliberately narrow. A token must be a *maximal* run of
/// exactly 16 hex digits (so a 32-char MD5 in a filename cannot match), start
/// with `01` and end with `000`, which is the shape of a base title id
/// (updates end `800`, DLC `xxx` indices). If the path yields two different
/// candidates it is ambiguous and this returns `None` — the caller then scans
/// nothing, which is the safe outcome.
pub fn switch_title_id_from_path(path: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for token in path.split(|c: char| !c.is_ascii_hexdigit()) {
        if token.len() != 16 {
            continue;
        }
        let id = token.to_ascii_lowercase();
        if !id.starts_with("01") || !id.ends_with("000") {
            continue;
        }
        match &found {
            Some(existing) if existing != &id => {
                warn!(
                    "[SAVE-SYNC] Ambiguous Switch title id in {path:?} ({existing} and {id}); \
                     refusing to guess"
                );
                return None;
            }
            Some(_) => {}
            None => found = Some(id),
        }
    }
    found
}

/// Locate the save-data directories belonging to `title_id` under the
/// emulator's NAND. Returns every directory named `<title_id>` no more than
/// [`SWITCH_TITLE_DIR_MAX_DEPTH`] below [`SWITCH_SAVE_ROOT`], which covers the
/// `<space>/<user>/` and `account/` + `device/` layouts without assuming
/// which one this build uses.
fn switch_title_save_dirs(emu_root: &Path, title_id: &str) -> Vec<PathBuf> {
    fn descend(dir: &Path, title_id: &str, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > SWITCH_TITLE_DIR_MAX_DEPTH {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let path = entry.path();
            let is_title = entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(title_id));
            if is_title {
                out.push(path);
            } else {
                descend(&path, title_id, depth + 1, out);
            }
        }
    }

    let root = emu_root.join(SWITCH_SAVE_ROOT);
    if !root.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    descend(&root, title_id, 1, &mut out);
    out
}

/// Prefix that namespaces Switch saves and marks a filename as carrying an
/// encoded *relative path* rather than a bare basename.
///
/// Switch saves are nested many levels deep under a title id and a user id,
/// and their basenames collide constantly (every game has a `00` file). The
/// path is therefore encoded into the filename, which is the only thing the
/// server round-trips. `/` becomes `%2F` and `%` becomes `%25` so the encoding
/// survives the server's `sanitize-filename` pass (which strips path
/// separators) and decodes unambiguously — the same problem, and the same kind
/// of fix, as [`PC_SAVE_PREFIX`].
pub const SWITCH_SAVE_PREFIX: &str = "switch__";

/// Depth limit for the recursive emulator save walk. The deepest real Switch
/// save path is `user/nand/user/save/<space>/<user>/<title>/<game dirs…>`;
/// this leaves generous room for a game's own nesting without letting a
/// pathological tree (or a symlink loop) stall a launch.
const EMU_SCAN_MAX_DEPTH: usize = 12;

/// File-count ceiling for one emulator save scan. Every file gets MD5'd, so
/// this is the real cost bound. A NAND dump can hold tens of thousands of
/// files; hitting the ceiling logs and stops rather than hanging the launch.
const EMU_SCAN_MAX_FILES: usize = 4000;

/// Escape a relative path into a separator-free, reversible string.
///
/// `/` becomes `%2F` and `%` becomes `%25` so the result survives the server's
/// `sanitize-filename` pass (which strips path separators) and still decodes
/// unambiguously. A path with a single component and no `%` escapes to itself,
/// which is what keeps rows written before nested paths were encoded matching
/// the names the scan produces today.
fn escape_relpath(rel: &Path) -> String {
    let joined = rel.to_string_lossy().replace('\\', "/");
    joined.replace('%', "%25").replace('/', "%2F")
}

/// Reverse [`escape_relpath`]. Returns `None` if the body is empty or decodes
/// to something that escapes its base directory (rooted, absolute, or
/// containing `..`).
///
/// `is_absolute()` on its own is not enough on Windows. There, "absolute"
/// means root **and** a drive prefix, so `\Windows\System32\evil.dll` reports
/// `is_absolute() == false` while still having a root — and `Path::join`
/// discards everything after the drive letter when the joined path is rooted,
/// so `C:\Emus\Eden`.join(that) lands in `C:\Windows\System32`. The name
/// reaching here is server-supplied (a cloud row name behind a Restore
/// button), so the root check has to be explicit.
fn unescape_relpath(body: &str) -> Option<PathBuf> {
    if body.is_empty() {
        return None;
    }
    // Decode %2F first, then %25, so a literal "%2F" in a real filename
    // (encoded as "%252F") does not turn into a separator.
    let decoded = body.replace("%2F", "/").replace("%25", "%");
    let rel = PathBuf::from(&decoded);
    if rel.is_absolute()
        || rel.has_root()
        || rel.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        warn!("[SAVE-SYNC] Refusing save path that escapes its root: {decoded}");
        return None;
    }
    Some(rel)
}

/// Decode a Switch save name — [`SWITCH_SAVE_PREFIX`] plus a path escaped by
/// [`escape_relpath`] — back to a path relative to the emulator root. `None`
/// if `filename` is not a Switch save name or would escape that root.
pub fn decode_switch_relpath(filename: &str) -> Option<PathBuf> {
    unescape_relpath(filename.strip_prefix(SWITCH_SAVE_PREFIX)?)
}

/// Whether a cloud row named `filename` may be written back into the emulator
/// root for the title this launch is scoped to.
///
/// Non-Switch names are always in scope — this only gates `switch__` rows.
///
/// Scoping the *scan* to one title id only closed half the hole. Every cloud
/// row written before that (the old sweep covered `nand/system/save`, `sdmc`
/// and every other title's directory under `nand/user/save`) is now a name the
/// scan can never report again, so sync-check calls it `cloudOnly` on every
/// single launch. Downloading one writes game A's stale snapshot straight over
/// game B's current save, over the emulator's system NAND, or over `sdmc` —
/// and it is never re-uploaded, because the scan still refuses to see it. Same
/// rule as [`is_denylisted_cloud_filename`]: a file the scan deliberately
/// refuses to see must never be written back either.
///
/// The check is structural rather than a directory listing, so a title whose
/// NAND directory does not exist on this device yet (a first restore) still
/// downloads: the path must sit under [`SWITCH_SAVE_ROOT`], below a component
/// named `title_id` no more than [`SWITCH_TITLE_DIR_MAX_DEPTH`] deep, which is
/// exactly the set [`switch_title_save_dirs`] can ever return.
///
/// With no resolvable title id every `switch__` row is out of scope, matching
/// [`scan_emu_saves`], which scans nothing from the NAND in that case.
pub fn switch_cloud_row_in_scope(title_id: Option<&str>, filename: &str) -> bool {
    if !filename.starts_with(SWITCH_SAVE_PREFIX) {
        return true;
    }
    let (Some(title_id), Some(rel)) = (title_id, decode_switch_relpath(filename)) else {
        return false;
    };
    let Ok(under) = rel.strip_prefix(SWITCH_SAVE_ROOT) else {
        return false;
    };
    let components: Vec<_> = under.components().collect();
    components
        .iter()
        .take(SWITCH_TITLE_DIR_MAX_DEPTH)
        .enumerate()
        .any(|(depth, component)| {
            // A row must be a file *inside* the title directory, never the
            // directory entry itself.
            depth + 1 < components.len()
                && matches!(
                    component,
                    std::path::Component::Normal(name) if name.eq_ignore_ascii_case(title_id)
                )
        })
}

/// Decode a `drop-saves` cloud filename back to its path relative to the
/// game's `saves/` or `states/` directory.
///
/// The walk under `drop-saves/<game_id>` recurses (some cores create a
/// per-game subdirectory), but it used to key every file on its basename, so
/// two cores' `save.dat` collapsed onto one cloud row and a restore always
/// wrote back to the flat top level. Names are relative paths now; a legacy
/// row has no escapes and decodes to itself, so it still restores to the top
/// level exactly as before.
pub fn decode_emu_relpath(filename: &str) -> Option<PathBuf> {
    if filename.starts_with(SWITCH_SAVE_PREFIX) {
        return None;
    }
    unescape_relpath(filename)
}

/// Recursively collect files under `dir`, appending to `out`.
///
/// Filenames are the file's path relative to `base`, escaped by
/// [`escape_relpath`] and carrying `prefix` (empty for `drop-saves`,
/// [`SWITCH_SAVE_PREFIX`] for the NAND). `budget` is the shared remaining file
/// count across every root in one scan.
fn walk_saves(
    dir: &Path,
    base: &Path,
    save_type: &str,
    prefix: &str,
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<LocalSaveFile>,
) {
    if depth > EMU_SCAN_MAX_DEPTH || *budget == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            warn!(
                "[SAVE-SYNC] Emulator save scan hit its {EMU_SCAN_MAX_FILES}-file ceiling; \
                 stopping early"
            );
            return;
        }
        let path = entry.path();
        // `file_type` does not follow symlinks, so a loop cannot be entered.
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_saves(&path, base, save_type, prefix, depth + 1, budget, out);
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        // Same filter the Ludusavi walk uses. Without it every `.bak` a
        // restore leaves behind is uploaded as a new save, pulled down on the
        // other devices, and backed up there in turn as `.bak.bak`.
        if is_save_denylisted(&path) {
            continue;
        }

        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let filename = format!("{prefix}{}", escape_relpath(rel));

        let Ok(meta) = fs::metadata(&path) else { continue };
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let hash = match md5_file(&path) {
            Ok(h) => h,
            Err(e) => {
                warn!("[SAVE-SYNC] Failed to hash {}: {}", path.display(), e);
                continue;
            }
        };

        *budget -= 1;
        out.push(LocalSaveFile {
            filename,
            save_type: save_type.to_string(),
            path,
            data_hash: hash,
            size: meta.len(),
            modified_at,
        });
    }
}

/// Scan an emulator install for a game's save files.
///
/// Two sources:
///
/// * `drop-saves/<user_id>/<game_id>/{saves,states}` — where Drop points RetroArch. The
///   walk recurses: some cores create a per-game subdirectory, and the old
///   single-level `read_dir` skipped those entirely. Names are paths relative
///   to the `saves`/`states` directory (see [`decode_emu_relpath`]).
/// * The Switch emulator NAND, **scoped to `switch_title_id`**. These names
///   carry their full path relative to `emu_root` (see [`SWITCH_SAVE_PREFIX`])
///   because a Switch save is identified by its title/user directory, not its
///   basename.
///
/// `user_id` scopes only the `drop-saves` half. The Switch NAND belongs to the
/// emulator, not to Drop, and moving it would break the emulator — so two
/// accounts on one PC still share the NAND *bytes*. What they no longer share
/// is the sync state: the manifest and every cloud row are per user, so each
/// account syncs that NAND against its own cloud library instead of one
/// account's launch pushing the other's progress upstream.
///
/// `switch_title_id` is the launched title's base id, from
/// [`switch_title_id_from_path`]. `None` means the NAND is skipped entirely:
/// this scan gets filed under one gameId, so sweeping a NAND shared by every
/// installed Switch title would put game B's saves in game A's cloud rows and
/// then restore them over each other. Syncing nothing is the lesser failure.
///
/// Bounded by [`EMU_SCAN_MAX_DEPTH`] and [`EMU_SCAN_MAX_FILES`] so a large
/// NAND tree cannot stall the launch that triggers it.
pub fn scan_emu_saves(
    emu_root: &Path,
    user_id: Option<&str>,
    game_id: &str,
    switch_title_id: Option<&str>,
) -> Vec<LocalSaveFile> {
    let saves_base = super::scope::resolve_emu_saves_root(emu_root, user_id, game_id);
    let mut files = Vec::new();
    let mut budget = EMU_SCAN_MAX_FILES;

    for (subdir, save_type) in &[("saves", "save"), ("states", "state")] {
        let dir = saves_base.join(subdir);
        if !dir.is_dir() {
            continue;
        }
        walk_saves(&dir, &dir, save_type, "", 0, &mut budget, &mut files);
    }

    let has_nand = emu_root.join(SWITCH_SAVE_ROOT).is_dir();
    match switch_title_id {
        Some(title_id) => {
            let dirs = switch_title_save_dirs(emu_root, title_id);
            if has_nand && dirs.is_empty() {
                info!(
                    "[SAVE-SYNC] No NAND save directory for title {title_id} yet (game {game_id})"
                );
            }
            for dir in dirs {
                walk_saves(&dir, emu_root, "save", SWITCH_SAVE_PREFIX, 0, &mut budget, &mut files);
            }
        }
        None if has_nand => warn!(
            "[SAVE-SYNC] Skipping the Switch NAND for game {game_id}: no title id in its ROM \
             path, and syncing the whole NAND under one game would mix every installed title's \
             saves together. Rename the ROM to include its title id (e.g. \
             \"Game [0100ABCDEF012000].nsp\") to enable cloud saves for it."
        ),
        None => {}
    }

    files
}

/// True when `path` already holds exactly `hash`, so a download of those bytes
/// has nothing to write.
///
/// `replace_save_file` always takes a backup, even when the new bytes are
/// identical. That was a bounded no-op while backups shared one fixed `.bak`
/// slot; with unique timestamped names it is one new file per write. A cloud
/// save the scan refuses to look at is reported as cloud-only on *every*
/// launch, so "one per write" becomes one junk file per launch forever, next
/// to the user's real saves — and RetroArch `.state.auto` files are tens of
/// megabytes. Skipping the identical write also leaves the on-disk mtime
/// alone, which is what the next sync-check compares.
fn already_matches(path: &Path, hash: Option<&str>) -> bool {
    let Some(hash) = hash else { return false };
    if hash.is_empty() || !path.is_file() {
        return false;
    }
    md5_file(path).is_ok_and(|have| have.eq_ignore_ascii_case(hash))
}

/// Write a downloaded save file to the correct local path.
///
/// `expected_hash` is the server's MD5 for `data` when the caller has it. If
/// the destination already hashes to it the write is skipped entirely — see
/// [`already_matches`]. Pass `None` to always write.
///
/// `save_type` must be `"save"` or `"state"`; anything else is refused. It
/// used to fall through to `saves/`, and `saveType` is entirely client-supplied
/// on upload with no game-ownership check, so any account could plant a `pc`
/// row against an emulator game's id and have this function file it in another
/// account's per-user emulator tree on a filename match.
pub fn write_downloaded_save(
    emu_root: &Path,
    user_id: Option<&str>,
    game_id: &str,
    filename: &str,
    save_type: &str,
    data: &[u8],
    expected_hash: Option<&str>,
) -> Result<PathBuf, String> {
    // Switch saves carry their whole relative path in the filename and live in
    // the emulator's NAND, not in Drop's per-game save directory.
    if let Some(rel) = decode_switch_relpath(filename) {
        let dest = emu_root.join(rel);
        if !already_matches(&dest, expected_hash) {
            backup::replace_save_file(&dest, data)?;
        }
        return Ok(dest);
    }

    let subdir = match save_type {
        "save" => "saves",
        "state" => "states",
        other => {
            return Err(format!(
                "Refusing to write {filename} into the emulator save directory: \
                 unexpected save type {other:?}"
            ));
        }
    };
    // The name is a path relative to the save/state directory, so a file a
    // core wrote in its own subdirectory goes back into that subdirectory
    // instead of being flattened onto the top level.
    let rel = decode_emu_relpath(filename)
        .ok_or_else(|| format!("Refusing to write save with an unsafe name: {filename}"))?;
    let dest = super::scope::resolve_emu_saves_root(emu_root, user_id, game_id)
        .join(subdir)
        .join(rel);
    if !already_matches(&dest, expected_hash) {
        backup::replace_save_file(&dest, data)?;
    }
    Ok(dest)
}

/// Delete a local emulator save (or save state) in response to a server
/// tombstone, after backing it up. Returns:
///   * `Ok(Some(path))` — the deleted file's original path,
///   * `Ok(None)`       — no local copy existed (nothing to do),
///   * `Err(msg)`       — backup or delete failed, and the file is untouched.
///
/// Callers must only reach this for a tombstone that passed
/// [`super::tombstone::plan_tombstones`]; applying the raw server list deletes
/// files this device never agreed to lose.
///
/// Blast radius worth stating for the `switch__` branch: those names resolve
/// into the emulator's own NAND, which every account on this PC shares. The
/// server only ever hands a client tombstones for its own rows, so one account
/// cannot cause another's delete — but applying your own tombstone does unlink
/// the NAND file a second local account is also playing against. The bytes go
/// to a timestamped backup first.
pub fn delete_local_emu_save_for_tombstone(
    emu_root: &Path,
    user_id: Option<&str>,
    game_id: &str,
    filename: &str,
) -> Result<Option<PathBuf>, String> {
    let saves_base = super::scope::resolve_emu_saves_root(emu_root, user_id, game_id);
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(rel) = decode_switch_relpath(filename) {
        candidates.push(emu_root.join(rel));
    } else if let Some(rel) = decode_emu_relpath(filename) {
        // The server tombstone doesn't tell us "save" vs "state"; try both.
        for subdir in &["saves", "states"] {
            candidates.push(saves_base.join(subdir).join(&rel));
        }
    }
    for candidate in candidates {
        if candidate.is_file() && backup::remove_save_file(&candidate)? {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Delete a PC save in response to a server tombstone. The caller passes the
/// resolved local path (from the manifest / `pc_save_paths` map); we don't
/// re-scan because the file may have been already deleted by the user on this
/// machine too. Backs the bytes up before unlinking, and refuses to unlink if
/// that backup fails.
pub fn delete_local_pc_save_for_tombstone(
    original_path: &Path,
) -> Result<bool, String> {
    backup::remove_save_file(original_path)
}

// ── Ludusavi PC save scanning ──────────────────────────────────────────

/// Filenames that Ludusavi will sometimes report (typically via
/// Unity-engine `<home>/AppData/LocalLow/<Company>/<Product>` directory
/// matches tagged as "config") but which are NEVER actual save data:
///
/// * `Player.log` / `Player-prev.log` — Unity's per-run diagnostic log files
///   that Unity rewrites on every launch.  Hashes change every session even
///   when the user didn't save anything, so they pollute the cloud save
///   panel and waste bandwidth.
///
/// Matching is case-insensitive (Windows) but only against the *exact*
/// basename — we deliberately do not filter on substrings or extensions
/// because real saves sometimes have `.log` in their name and there is no
/// reliable heuristic beyond a tiny denylist.  When in doubt, keep the file.
const PC_SAVE_BASENAME_DENYLIST: &[&str] = &["Player.log", "Player-prev.log"];

/// Returns `true` if `path` is something no save walk should pick up: a
/// backup or temp file Drop itself wrote, RetroArch's auto-state churn, or
/// one of the well-known non-save basenames in [`PC_SAVE_BASENAME_DENYLIST`].
///
/// Used by both walks. It was PC-only, which is how emulator restores ended
/// up round-tripping their own `.bak` files into the cloud and back out again
/// as `.bak.bak`.
pub fn is_save_denylisted(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // Backups Drop itself writes when restoring/overwriting a save. They sit
    // right next to the real save, so the next scan re-discovers them as
    // brand-new saves — that's what made the save count climb every time a
    // restore ran and filled the panel with phantom ".bak" rows.
    if backup::is_backup_artifact(name) {
        return true;
    }
    // RetroArch's auto-save slot and its thumbnail, rewritten on every close
    // whether or not the player saved. Syncing them means a new blob upload
    // and a fresh download on every other device, every single session, and
    // the user never asked for either file.
    let lower = name.to_ascii_lowercase();
    let without_png = lower.strip_suffix(".png").unwrap_or(&lower);
    if without_png.ends_with(".state.auto") || lower.ends_with(".state.png") {
        return true;
    }
    PC_SAVE_BASENAME_DENYLIST
        .iter()
        .any(|deny| name.eq_ignore_ascii_case(deny))
}

/// [`is_save_denylisted`] for a *cloud* filename, applied before the bytes are
/// written back to disk.
///
/// The scan side of the denylist is only half the job. Widening it means these
/// files can never appear in `local_saves` again, so a `.bak` or `.state.auto`
/// row already sitting in the user's cloud (uploaded before the denylist
/// existed) is classified `cloudOnly` by sync-check on every single launch,
/// forever. Downloading that is actively destructive: the stale cloud copy of
/// `<game>.state.auto` overwrites the player's current auto-resume state each
/// time, and it is never re-uploaded because the scan skips it. A file the
/// scan deliberately refuses to see must never be written back either.
///
/// Handles both namespaced forms — the check runs against the decoded leaf
/// name, not the wire name.
pub fn is_denylisted_cloud_filename(filename: &str) -> bool {
    let leaf = decode_switch_relpath(filename)
        .or_else(|| decode_pc_relpath(filename))
        .and_then(|rel| rel.file_name().map(|n| n.to_os_string()))
        .unwrap_or_else(|| strip_pc_prefix(filename).into());
    is_save_denylisted(Path::new(&leaf))
}

/// Whether PC save discovery can run at all on this machine.
///
/// [`scan_pc_saves`] returns an empty list when Ludusavi is missing, which is
/// indistinguishable from "this game has no saves". The sync path needs to be
/// able to tell those apart so it can say which one happened instead of
/// quietly backing nothing up forever.
pub fn ludusavi_available() -> bool {
    find_ludusavi().is_some()
}

/// What Drop is able to discover for one PC game on this machine.
///
/// Drop's PC coverage is exactly "games in Ludusavi's catalogue". For anything
/// else [`scan_pc_saves`] returns an empty list, which reads identically to a
/// game that simply hasn't been played yet — so the panel showed "no saves
/// yet, play the game once" to people whose game will never produce a save no
/// matter how long they play it. This is what lets the two be told apart.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcSaveCoverage {
    pub ludusavi_installed: bool,
    /// Ludusavi's catalogue has an entry for this game, so Drop knows where
    /// its saves live even if none exist yet.
    pub known_to_ludusavi: bool,
    /// The catalogue title the game's display name resolved to. Shown so a
    /// wrong match is visible rather than mysterious.
    pub canonical_title: Option<String>,
}

/// Answer [`PcSaveCoverage`] for a game. Runs Ludusavi, so keep it off the
/// main thread.
pub fn pc_save_coverage(game_name: &str, steam_app_id: Option<&str>) -> PcSaveCoverage {
    let Some(ludusavi) = find_ludusavi() else {
        return PcSaveCoverage {
            ludusavi_installed: false,
            known_to_ludusavi: false,
            canonical_title: None,
        };
    };
    let canonical = resolve_canonical_title(&ludusavi, game_name, steam_app_id);
    PcSaveCoverage {
        ludusavi_installed: true,
        known_to_ludusavi: canonical.is_some(),
        canonical_title: canonical,
    }
}

/// Find the Ludusavi binary (bundled in Drop's tools dir, or on PATH).
fn find_ludusavi() -> Option<PathBuf> {
    // `DATA_ROOT_DIR`, not a hardcoded "drop": a debug build's data root is
    // `drop-debug`, so the hardcoded name looked for the bundled binary in the
    // release install's tools dir and fell through to PATH (usually nothing),
    // silently disabling PC save detection for every dev build.
    let tools = database::db::DATA_ROOT_DIR.join("tools");
    #[cfg(target_os = "windows")]
    let bundled = tools.join("ludusavi").join("ludusavi.exe");
    #[cfg(not(target_os = "windows"))]
    let bundled = tools.join("ludusavi").join("ludusavi");

    if bundled.exists() {
        return Some(bundled);
    }

    // Check PATH
    if let Ok(output) = std::process::Command::new("ludusavi").arg("--version").output()
        && output.status.success() {
            return Some(PathBuf::from("ludusavi"));
        }

    None
}

/// Pull the game title out of a `ludusavi find --api` JSON blob.
/// The shape is `{ "games": { "<Canonical Title>": {...} }, ... }`.
///
/// One key is the answer. Several keys used to resolve to whichever one came
/// first, which is a coin toss: `--normalized` ignores year and edition
/// suffixes, so a query for "DOOM" matches both `DOOM` and `DOOM (2016)` and
/// the loser's save directory is a different game's. When the blob names more
/// than one game we only accept a key that equals `query` outright; anything
/// else returns `None`, the same refuse-to-guess rule
/// [`switch_title_id_from_path`] already follows. Restoring nothing is
/// recoverable; restoring into another game's folder is not.
fn resolve_found_title(stdout: &[u8], query: &str) -> Option<String> {
    let s = String::from_utf8_lossy(stdout);
    let value: serde_json::Value = serde_json::from_str(&s).ok()?;
    let games = value.get("games")?.as_object()?;

    let mut keys = games.keys();
    let first = keys.next()?;
    if keys.next().is_none() {
        return Some(first.to_string());
    }

    let mut exact = games
        .keys()
        .filter(|k| k.eq_ignore_ascii_case(query))
        .map(|k| k.to_string());
    match (exact.next(), exact.next()) {
        (Some(only), None) => Some(only),
        _ => {
            warn!(
                "[SAVE-SYNC] Ludusavi matched {} games for {query:?} and none of them is an \
                 exact match, so Drop will not guess which one it is",
                games.len()
            );
            None
        }
    }
}

/// Resolve a game's Drop **display name** to the canonical title that
/// Ludusavi's manifest actually uses.
///
/// This is the crux of why PC save detection was flaky. Ludusavi's
/// `backup` subcommand matches game names **exactly and case-sensitively**
/// against its manifest. Drop's display names carry trademark symbols
/// (`®`/`™`), all-caps branding ("LEGO"), and "edition"/year suffixes that
/// byte-differ from the manifest's canonical title — e.g. the manifest
/// has `Lego Batman: Legacy of the Dark Knight` but Drop stores
/// `LEGO® Batman™: Legacy of the Dark Knight`. A raw exact `backup` on
/// the display name therefore matches nothing and silently returns zero
/// saves.
///
/// `ludusavi find` resolves by precedence (Steam ID → GOG ID → exact →
/// normalized), and `--normalized` "ignores capitalization, 'edition'
/// suffixes, year suffixes, and some special symbols" — exactly the
/// noise in Drop's display names. We try the Steam ID first (an exact
/// identifier match, most reliable), then fall back to normalized-name
/// resolution. Returns the canonical manifest title, or `None` if
/// Ludusavi doesn't know the game under any of these.
fn resolve_canonical_title(
    ludusavi: &Path,
    game_name: &str,
    steam_app_id: Option<&str>,
) -> Option<String> {
    // 1) Steam ID — highest-precedence, exact identifier match.
    if let Some(id) = steam_app_id
        && let Ok(output) = std::process::Command::new(ludusavi)
            .args(["find", "--api", "--steam-id", id])
            .output()
            && output.status.success()
            && let Some(name) = resolve_found_title(&output.stdout, game_name)
        {
            return Some(name);
        }

    // 2) Normalized display name — collapses caps / ®™ / edition+year
    //    suffixes onto the manifest's canonical title.
    if let Ok(output) = std::process::Command::new(ludusavi)
        .args(["find", "--api", "--normalized", game_name])
        .output()
        && output.status.success()
        && let Some(name) = resolve_found_title(&output.stdout, game_name)
    {
        return Some(name);
    }

    None
}

/// The Steam app id Drop can work out for a game, for Ludusavi's
/// highest-precedence resolution tier.
///
/// Lives here rather than beside the Tauri commands because every Ludusavi
/// call site needs it, including the two in the `process` crate that run on the
/// launch and exit paths. Those had no way to reach the copy in `games.rs` and
/// passed `None`, which drops resolution down to `--normalized` — the tier that
/// cannot tell "DOOM" from "DOOM (2016)".
///
/// Best-effort by nature: an id is only a hint that makes the match exact when
/// it is there, so every lookup below is allowed to come up empty.
pub fn steam_app_id_for_game(game_id: &str) -> Option<String> {
    let install_dir = {
        let db = database::borrow_db_checked();
        match db.applications.game_statuses.get(game_id) {
            Some(database::GameDownloadStatus::Installed { install_dir, .. }) => {
                install_dir.clone()
            }
            _ => return None,
        }
    };

    // The game's own id file, dropped next to the executable by Steam builds
    // and by Drop's Goldberg setup.
    let appid_path = Path::new(&install_dir).join("steam_appid.txt");
    if let Ok(contents) = fs::read_to_string(&appid_path) {
        let trimmed = contents.trim();
        if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit()) {
            return Some(trimmed.to_string());
        }
    }

    // Drop's per-game Goldberg tree: `drop-goldberg/<appid>/`.
    let goldberg_dir = Path::new(&install_dir).join("drop-goldberg");
    if let Ok(entries) = fs::read_dir(&goldberg_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.chars().all(|c| c.is_ascii_digit()) {
                    return Some(name);
                }
            }
        }
    }

    // The shared Goldberg location under %AppData%. Nothing here ties a
    // directory to this game, so only a directory that holds real Goldberg
    // state counts, and even then it is a guess.
    if let Some(appdata) = dirs::data_dir() {
        let shared_goldberg = appdata.join("drop-goldberg");
        if let Ok(entries) = fs::read_dir(&shared_goldberg) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.chars().all(|c| c.is_ascii_digit())
                        && (entry.path().join("achievements.json").exists()
                            || entry.path().join("stats.json").exists())
                    {
                        info!(
                            "[SAVE-SYNC] Using Steam app id {name} from the shared Goldberg \
                             directory for game {game_id}"
                        );
                        return Some(name);
                    }
                }
            }
        }
    }

    None
}

// ── Manifest tag awareness ─────────────────────────────────────────────
//
// Ludusavi's `backup --api` output does NOT carry the per-file save/config
// tag — verified against its `schema general-output`: each file (ApiFile)
// has only bytes/change/duplicatedBy/failed/ignored. The save/config tags
// live ONLY in the manifest. The real Ludusavi GUI reads them to separate
// real saves from settings; to match that, we read the manifest ourselves
// and drop files that fall under a *config-only* path (e.g. Grim Dawn's
// "My Games/Grim Dawn/Settings", which holds keybindings.txt + options.txt
// rather than character saves).
//
// We pull the manifest as JSON via `manifest show --api` so it parses with
// serde_json (no YAML dependency), and cache the raw text for the process
// lifetime — the manifest only changes on `manifest update`.
static MANIFEST_JSON_CACHE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// One save-classification rule from a game's manifest entry: the literal
/// (placeholder/wildcard-free) path fragment to substring-match a scanned
/// file against, plus whether that manifest path is tagged config-and-not-
/// save (i.e. should be excluded from save backup).
struct ManifestPathRule {
    needle: String,
    config_only: bool,
}

/// Reduce a Ludusavi manifest path pattern to its longest leading
/// placeholder-free, wildcard-free literal fragment, for a substring match
/// against an absolute scanned path. Separator-normalized to `/`, and
/// lowercased on Windows for case-insensitive matching.
///
///   `<winDocuments>/My Games/Grim Dawn/Settings` → `my games/grim dawn/settings`
///   `<winLocalAppData>/XV83/Saved/SaveGames/**/*.sav` → `xv83/saved/savegames`
fn literal_fragment(pattern: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in pattern.split(['/', '\\']) {
        if seg.contains('<') || seg.contains('*') || seg.contains('[') {
            if out.is_empty() {
                continue; // skip leading placeholder segment(s)
            }
            break; // a wildcard terminates the literal tail
        }
        if !seg.is_empty() {
            out.push(seg);
        }
    }
    let joined = out.join("/");
    if cfg!(target_os = "windows") {
        joined.to_lowercase()
    } else {
        joined
    }
}

/// Return the substring covering the balanced `{...}` object at the start
/// of `s` (skipping any leading whitespace to the first `{`). Respects
/// JSON strings + escapes so braces inside string values don't miscount.
fn slice_balanced_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'{')?;
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for i in start..bytes.len() {
        let c = bytes[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Load (and cache) the manifest as JSON and slice out one game's entry.
///
/// The manifest is one big JSON object keyed by game title. We find this
/// game's value by its JSON-encoded key, then brace-match to slice out just
/// that entry — parsing the whole ~9 MB blob into a `Value` for one lookup is
/// what this avoids. `None` when Ludusavi fails or the title isn't listed.
fn manifest_entry(ludusavi: &Path, title: &str) -> Option<serde_json::Value> {
    let mut guard = MANIFEST_JSON_CACHE.lock().ok()?;
    if guard.is_none() {
        match std::process::Command::new(ludusavi)
            .args(["manifest", "show", "--api"])
            .output()
        {
            Ok(o) if o.status.success() => {
                *guard = Some(String::from_utf8_lossy(&o.stdout).into_owned());
            }
            _ => {
                warn!("[SAVE-SYNC] Could not load Ludusavi manifest");
                return None;
            }
        }
    }
    let raw = guard.as_ref()?;

    let key = serde_json::to_string(title).ok()?;
    let needle = format!("{key}:");
    let key_pos = raw.find(&needle)?;
    let after_key = &raw[key_pos + needle.len()..];
    let obj = slice_balanced_object(after_key)?;
    serde_json::from_str::<serde_json::Value>(obj).ok()
}

/// The `files` block of a game's manifest entry, as (pattern, metadata) pairs.
/// Empty when the game isn't listed or stores nothing.
fn manifest_file_patterns(ludusavi: &Path, title: &str) -> Vec<(String, serde_json::Value)> {
    let Some(entry) = manifest_entry(ludusavi, title) else {
        return Vec::new();
    };
    entry
        .get("files")
        .and_then(|f| f.as_object())
        .map(|files| {
            files
                .iter()
                .map(|(pattern, meta)| (pattern.clone(), meta.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// True when a manifest `files` entry is tagged config and NOT save.
///
/// Untagged paths default to save data per the manifest spec, so an absent
/// `tags` array must not be read as config.
fn entry_is_config_only(meta: &serde_json::Value) -> bool {
    let tags: Vec<&str> = meta
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| arr.iter().filter_map(|t| t.as_str()).collect())
        .unwrap_or_default();
    tags.contains(&"config") && !tags.contains(&"save")
}

/// Extract the path classification rules for `title`. Empty vec if Ludusavi
/// fails, the game isn't in the manifest, or it has no `files` block —
/// callers treat "no rules" as "don't filter".
fn manifest_path_rules(ludusavi: &Path, title: &str) -> Vec<ManifestPathRule> {
    manifest_file_patterns(ludusavi, title)
        .into_iter()
        .filter_map(|(pattern, meta)| {
            let frag = literal_fragment(&pattern);
            if frag.is_empty() {
                return None;
            }
            Some(ManifestPathRule {
                needle: frag,
                config_only: entry_is_config_only(&meta),
            })
        })
        .collect()
}

/// Decide whether a scanned PC file is a *save* (vs config) per the game's
/// manifest tags. Dropped only when it matches a config-only path AND no
/// save/untagged path. No rules, or no match, => keep (never silently drop
/// a file the manifest doesn't classify).
fn is_save_tagged(path: &Path, rules: &[ManifestPathRule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let hay = {
        let p = path.to_string_lossy().replace('\\', "/");
        if cfg!(target_os = "windows") {
            p.to_lowercase()
        } else {
            p
        }
    };
    let mut matched_config_only = false;
    let mut matched_keep = false;
    for rule in rules {
        if hay.contains(&rule.needle) {
            if rule.config_only {
                matched_config_only = true;
            } else {
                matched_keep = true;
            }
        }
    }
    if matched_keep {
        true
    } else {
        !matched_config_only
    }
}

/// Scan PC game saves using Ludusavi.
/// `game_name` is the display name to search for; `steam_app_id` is optional.
/// `wine_prefix`, when supplied, is passed to Ludusavi via `--wine-prefix`
/// so it scans Drop's per-game Wine prefix in addition to its default
/// scan locations (Steam compatdata, Lutris, Heroic). On native Linux
/// games (and on Windows hosts) pass `None` to keep the default behaviour.
/// Returns files as `LocalSaveFile` with save_type = "pc".
pub fn scan_pc_saves(
    game_name: &str,
    steam_app_id: Option<&str>,
    wine_prefix: Option<&Path>,
) -> Vec<LocalSaveFile> {
    let ludusavi = match find_ludusavi() {
        Some(p) => p,
        None => {
            info!("[SAVE-SYNC] Ludusavi not found, skipping PC save scan");
            return Vec::new();
        }
    };

    // Resolve the display name to Ludusavi's canonical manifest title
    // (Steam ID, else normalized name). Without this, branded display
    // names like "LEGO® Batman™: …" never match the manifest's
    // "Lego Batman: …" and the scan returns nothing.
    let resolved_name = resolve_canonical_title(&ludusavi, game_name, steam_app_id);

    let search_name = resolved_name.as_deref().unwrap_or(game_name);
    let wine_prefix_str = wine_prefix.map(|p| p.to_string_lossy().to_string());
    if let Some(p) = wine_prefix_str.as_deref() {
        info!("[SAVE-SYNC] Ludusavi scanning for '{}' (wine prefix: {})", search_name, p);
    } else {
        info!("[SAVE-SYNC] Ludusavi scanning for '{}'", search_name);
    }

    // Build args once; injected `--wine-prefix <path>` precedes the game
    // name so it applies to the backup subcommand.
    let build_args = |name: &str| -> Vec<String> {
        let mut args: Vec<String> = vec![
            "backup".into(),
            "--preview".into(),
            "--api".into(),
        ];
        if let Some(p) = wine_prefix_str.as_deref() {
            args.push("--wine-prefix".into());
            args.push(p.to_string());
        }
        args.push(name.to_string());
        args
    };

    // Run "backup --preview --api [--wine-prefix <path>] <name>"
    let output = std::process::Command::new(&ludusavi)
        .args(build_args(search_name))
        .output();

    // Retry with the original name if resolved name found nothing
    let output = match &output {
        Ok(o) if !o.status.success() || o.stdout.len() < 50 => {
            if search_name != game_name {
                info!(
                    "[SAVE-SYNC] Retrying Ludusavi with original name: '{}'",
                    game_name
                );
                std::process::Command::new(&ludusavi)
                    .args(build_args(game_name))
                    .output()
            } else {
                output
            }
        }
        _ => output,
    };

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            warn!("[SAVE-SYNC] Ludusavi command failed: {e}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("No matching") {
            warn!("[SAVE-SYNC] Ludusavi error: {}", stderr);
        }
        return Vec::new();
    }

    // Parse the JSON output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            warn!("[SAVE-SYNC] Failed to parse Ludusavi output: {e}");
            return Vec::new();
        }
    };

    // Collected before names are assigned: a PC save's cloud identity is its
    // path relative to the root every discovered save shares, which is only
    // knowable once the whole scan is in.
    let mut found: Vec<(PathBuf, u64, String, u64)> = Vec::new();

    // Per-game manifest tag rules — lets us drop config-only files
    // (settings, keybinds) the same way the real Ludusavi GUI does. The
    // CLI strips tags from its output, so we read them from the manifest
    // here. `search_name` is the canonical title we resolved above, which
    // is exactly the manifest key.
    let tag_rules = manifest_path_rules(&ludusavi, search_name);
    let mut tag_dropped = 0usize;

    if let Some(games) = json.get("games").and_then(|g| g.as_object()) {
        for (_name, game_data) in games {
            if let Some(game_files) = game_data.get("files").and_then(|f| f.as_object()) {
                for (file_path, file_data) in game_files {
                    let path = PathBuf::from(file_path);
                    if !path.is_file() {
                        continue;
                    }
                    // Drop known non-save files (Unity diagnostic logs etc.)
                    // that Ludusavi's directory-wildcard manifest entries pick
                    // up alongside the real save data.
                    if is_save_denylisted(&path) {
                        info!(
                            "[SAVE-SYNC] Skipping denylisted PC save file: {}",
                            path.display()
                        );
                        continue;
                    }
                    // Honor the manifest's save/config tags: skip files that
                    // live only under a config-tagged path (e.g. Grim Dawn's
                    // Settings/ folder). Matches how the real Ludusavi GUI
                    // separates saves from settings.
                    if !is_save_tagged(&path, &tag_rules) {
                        tag_dropped += 1;
                        info!(
                            "[SAVE-SYNC] Skipping config-tagged file: {}",
                            path.display()
                        );
                        continue;
                    }
                    // Ludusavi's `backup --api` reports file size under the `bytes`
                    // key (not `size`); reading the wrong key reported every file as 0 B.
                    let size = file_data.get("bytes").and_then(|s| s.as_u64()).unwrap_or(0);
                    let hash = match md5_file(&path) {
                        Ok(h) => h,
                        Err(e) => {
                            warn!(
                                "[SAVE-SYNC] Failed to hash PC save {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };
                    let modified_at = fs::metadata(&path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    found.push((path, size, hash, modified_at));
                }
            }
        }
    }

    // Namespace PC saves so their filenames don't collide with emu saves. Use
    // a "pc__" prefix (NOT "pc/"): the server runs the upload filename through
    // `sanitize-filename`, which strips path separators — "pc/gen.sav" became
    // "pcgen.sav" on the server while the local scan still reported
    // "pc/gen.sav", so the same save showed up twice (once per side). A
    // separator-free prefix round-trips unchanged, giving every save one
    // stable identity.
    let paths: Vec<PathBuf> = found.iter().map(|(p, ..)| p.clone()).collect();
    let save_root = common_save_root(&paths);
    let files: Vec<LocalSaveFile> = found
        .into_iter()
        .map(|(path, size, hash, modified_at)| LocalSaveFile {
            filename: encode_pc_filename(save_root.as_deref(), &path),
            save_type: "pc".to_string(),
            path,
            data_hash: hash,
            size,
            modified_at,
        })
        .collect();

    info!(
        "[SAVE-SYNC] Ludusavi found {} PC save file(s) for '{}' ({} config-tagged file(s) filtered out)",
        files.len(),
        search_name,
        tag_dropped
    );
    files
}

// ── Manifest-driven restore destinations ───────────────────────────────
//
// Everything above resolves a restore destination from files that already
// exist on this machine. That is exactly what a fresh machine does not have:
// after a Windows reinstall the game has never run, so Ludusavi's scan finds
// nothing and there is no sibling save to place a download next to — which is
// the one situation cloud saves exist for.
//
// The manifest itself already knows where each game writes, as path patterns
// against a small set of placeholders. Expanding those gives a destination
// with no local save required.

/// Real directories for the placeholders Ludusavi's manifest paths are
/// written against. `None` means "this machine has no such directory", and
/// any pattern rooted at it is skipped rather than guessed at.
#[derive(Debug, Clone, Default)]
struct ManifestRoots {
    base: Option<PathBuf>,
    home: Option<PathBuf>,
    win_app_data: Option<PathBuf>,
    win_local_app_data: Option<PathBuf>,
    win_documents: Option<PathBuf>,
    win_public: Option<PathBuf>,
    win_program_data: Option<PathBuf>,
    win_dir: Option<PathBuf>,
    xdg_data: Option<PathBuf>,
    xdg_config: Option<PathBuf>,
    os_user_name: Option<String>,
}

/// One directory the manifest says a game writes saves into, already resolved
/// to a real path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestSaveDir {
    dir: PathBuf,
    /// The concrete filename the pattern names, when it names one rather than
    /// a directory or a glob.
    file_name: Option<String>,
    /// The extension the pattern's leaf implies (`*.sav` → `sav`), used to
    /// prefer the folder that holds files of the same kind.
    leaf_ext: Option<String>,
    /// How many path segments followed the placeholder. Deeper is more
    /// specific, and a more specific pattern is the better guess.
    depth: usize,
}

/// The OS name Ludusavi's manifest uses for this host.
fn host_manifest_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

/// Where a Wine prefix keeps the Windows user profile. Proton names it
/// `steamuser`; other prefixes use the real login name, so fall back to
/// whatever single user directory is there.
fn wine_user_dir(prefix: &Path) -> Option<PathBuf> {
    let users = prefix.join("drive_c").join("users");
    let steamuser = users.join("steamuser");
    if steamuser.is_dir() {
        return Some(steamuser);
    }
    fs::read_dir(&users).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() && !name.eq_ignore_ascii_case("Public") {
            Some(path)
        } else {
            None
        }
    })
}

/// Resolve every placeholder for this machine.
///
/// With a Wine prefix the Windows placeholders point inside the prefix, which
/// is where a Windows game running under Proton actually writes. Without one
/// they point at the host's own directories, and the placeholders that don't
/// exist on this OS resolve to `None`.
fn manifest_roots(install_dir: Option<&Path>, wine_prefix: Option<&Path>) -> ManifestRoots {
    let base = install_dir.map(|p| p.to_path_buf());

    if let Some(prefix) = wine_prefix {
        let drive_c = prefix.join("drive_c");
        let home = wine_user_dir(prefix);
        return ManifestRoots {
            base,
            win_app_data: home.as_ref().map(|h| h.join("AppData").join("Roaming")),
            win_local_app_data: home.as_ref().map(|h| h.join("AppData").join("Local")),
            win_documents: home.as_ref().map(|h| h.join("Documents")),
            win_public: Some(drive_c.join("users").join("Public")),
            win_program_data: Some(drive_c.join("ProgramData")),
            win_dir: Some(drive_c.join("windows")),
            os_user_name: home
                .as_ref()
                .and_then(|h| h.file_name())
                .map(|n| n.to_string_lossy().into_owned()),
            home,
            // A Windows game in a prefix never reads the host's XDG dirs.
            xdg_data: None,
            xdg_config: None,
        };
    }

    let home = dirs::home_dir();
    ManifestRoots {
        base,
        win_app_data: cfg!(target_os = "windows").then(dirs::data_dir).flatten(),
        win_local_app_data: cfg!(target_os = "windows")
            .then(dirs::data_local_dir)
            .flatten(),
        win_documents: cfg!(target_os = "windows")
            .then(dirs::document_dir)
            .flatten(),
        win_public: std::env::var_os("PUBLIC").map(PathBuf::from),
        win_program_data: std::env::var_os("ProgramData").map(PathBuf::from),
        win_dir: std::env::var_os("windir").map(PathBuf::from),
        xdg_data: (!cfg!(target_os = "windows"))
            .then(dirs::data_dir)
            .flatten(),
        xdg_config: (!cfg!(target_os = "windows"))
            .then(dirs::config_dir)
            .flatten(),
        os_user_name: std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .ok()
            .filter(|n| !n.is_empty()),
        home,
    }
}

/// Map a leading `<placeholder>` segment onto its real directory.
///
/// `<root>` / `<game>` are deliberately unresolved: they mean "wherever the
/// launcher installed this", which is a set of directories we would be
/// guessing at. `<base>` is the game's own install directory, which Drop does
/// know when the game is installed here.
fn resolve_root_placeholder(segment: &str, roots: &ManifestRoots) -> Option<PathBuf> {
    match segment {
        "<base>" => roots.base.clone(),
        "<home>" => roots.home.clone(),
        "<winAppData>" => roots.win_app_data.clone(),
        "<winLocalAppData>" => roots.win_local_app_data.clone(),
        "<winDocuments>" => roots.win_documents.clone(),
        "<winPublic>" => roots.win_public.clone(),
        "<winProgramData>" => roots.win_program_data.clone(),
        "<winDir>" => roots.win_dir.clone(),
        "<xdgData>" => roots.xdg_data.clone(),
        "<xdgConfig>" => roots.xdg_config.clone(),
        _ => None,
    }
}

/// True when a path segment still holds something we cannot turn into a
/// literal directory name: an unresolved placeholder or a glob.
fn segment_is_unresolved(segment: &str) -> bool {
    segment.contains(['<', '*', '?', '['])
}

/// Turn one manifest path pattern into a concrete directory on this machine.
///
/// Returns `None` when the pattern is rooted at a placeholder this machine has
/// no value for — a Windows path on a native Linux host, or `<base>` for a
/// game that isn't installed. Everything from the first glob onward is
/// dropped, so `<winLocalAppData>/XV83/Saved/SaveGames/**/*.sav` resolves to
/// `…/XV83/Saved/SaveGames` and remembers that its leaf was a `.sav`.
fn expand_manifest_pattern(pattern: &str, roots: &ManifestRoots) -> Option<ManifestSaveDir> {
    let mut segments = pattern.split(['/', '\\']).filter(|s| !s.is_empty());
    let root = resolve_root_placeholder(segments.next()?, roots)?;

    let mut literal: Vec<String> = Vec::new();
    let mut leaf_ext: Option<String> = None;
    let mut hit_glob = false;
    for segment in segments {
        // `<osUserName>` is the one placeholder that shows up mid-path, and
        // we do know the answer to it.
        let segment = match roots.os_user_name.as_deref() {
            Some(user) => segment.replace("<osUserName>", user),
            None => segment.to_string(),
        };
        if segment_is_unresolved(&segment) {
            hit_glob = true;
            // `*.sav` still tells us what kind of file lives here; `**` and
            // an unresolved `<placeholder>` tell us nothing.
            leaf_ext = segment
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .filter(|ext| !ext.is_empty() && !segment_is_unresolved(ext));
            break;
        }
        literal.push(segment);
    }

    let depth = literal.len();
    let mut file_name = None;
    // A fully literal pattern's last segment is either the save file itself or
    // the directory holding the saves. An extension is the only signal the
    // manifest gives, and treating "Foo.dat" as a folder would restore into a
    // directory the game has never heard of. `extension()` and not "contains a
    // dot": a dotfile directory like `.minecraft` has no extension, and
    // demoting it to a filename would restore a whole save tree one level too
    // high.
    if !hit_glob
        && let Some(ext) = literal
            .last()
            .and_then(|last| Path::new(last).extension())
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
    {
        leaf_ext = Some(ext);
        file_name = literal.pop();
    }

    Some(ManifestSaveDir {
        dir: literal.iter().fold(root, |acc, seg| acc.join(seg)),
        file_name,
        leaf_ext,
        depth,
    })
}

/// True when `ancestor` is `path` or a parent of it, matched the way this
/// platform matches filenames.
fn path_starts_with(path: &Path, ancestor: &Path) -> bool {
    let parts: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let head: Vec<String> = ancestor
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.len() >= head.len()
        && parts
            .iter()
            .zip(head.iter())
            .all(|(a, b)| same_path_name(a, b))
}

/// Compare two path segments the way the host filesystem does.
fn same_path_name(a: &str, b: &str) -> bool {
    if cfg!(target_os = "windows") {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Choose which of a game's manifest save directories `rel` belongs in.
///
/// A game can list several: one for saves, one for screenshots, one per store.
/// Ranking, strongest first:
///
///   * the pattern names this exact file,
///   * the directory contains saves Ludusavi actually found here (so a machine
///     that *does* have the game keeps landing where it always did),
///   * the directory already exists,
///   * the pattern's leaf has the same extension,
///   * the more specific pattern.
///
/// `dir_exists` is injected so the ranking stays testable without a filesystem.
fn choose_manifest_dir<'a>(
    candidates: &'a [ManifestSaveDir],
    rel: &Path,
    found_dirs: &[PathBuf],
    dir_exists: impl Fn(&Path) -> bool,
) -> Option<&'a ManifestSaveDir> {
    let basename = rel.file_name()?.to_string_lossy().into_owned();
    let rel_ext = Path::new(&basename)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());

    let mut best: Option<(i64, &ManifestSaveDir)> = None;
    for candidate in candidates {
        let mut score = candidate.depth as i64;
        match candidate.file_name.as_deref() {
            Some(name) if same_path_name(name, &basename) => score += 2000,
            // The pattern names some other file. Its folder is still a
            // plausible home, just a weaker one than a folder-shaped pattern.
            Some(_) => score -= 10,
            None => {}
        }
        if found_dirs.iter().any(|d| path_starts_with(d, &candidate.dir)) {
            score += 1000;
        } else if dir_exists(&candidate.dir) {
            score += 500;
        }
        if candidate.leaf_ext.is_some() && candidate.leaf_ext == rel_ext {
            score += 100;
        }
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, candidate));
        }
    }

    best.map(|(_, candidate)| candidate)
}

/// Whether a manifest `files` entry applies on `os`.
///
/// A `when` clause without an `os` key restricts by store, not platform, so it
/// applies everywhere. No `when` at all also applies everywhere.
fn entry_applies_to_os(meta: &serde_json::Value, os: &str) -> bool {
    let Some(when) = meta.get("when").and_then(|w| w.as_array()) else {
        return true;
    };
    if when.is_empty() {
        return true;
    }
    when.iter().any(|clause| {
        clause
            .get("os")
            .and_then(|o| o.as_str())
            .is_none_or(|clause_os| clause_os == os)
    })
}

/// Every save directory the manifest lists for `title`, resolved for this
/// machine. Config-only paths and paths for another platform are dropped.
fn manifest_save_dirs(
    ludusavi: &Path,
    title: &str,
    install_dir: Option<&Path>,
    wine_prefix: Option<&Path>,
) -> Vec<ManifestSaveDir> {
    let roots = manifest_roots(install_dir, wine_prefix);
    // A game running in a Wine prefix is a Windows game whatever the host is.
    let target_os = if wine_prefix.is_some() {
        "windows"
    } else {
        host_manifest_os()
    };
    manifest_file_patterns(ludusavi, title)
        .into_iter()
        .filter(|(_, meta)| !entry_is_config_only(meta) && entry_applies_to_os(meta, target_os))
        .filter_map(|(pattern, _)| expand_manifest_pattern(&pattern, &roots))
        .collect()
}

/// Resolve the on-disk destination path for a PC save, using Ludusavi's
/// catalogue.
///
/// `rel` is the save's path relative to the game's save root — for the legacy
/// rows that make up most of a user's cloud it is just a basename, and every
/// tier below behaves exactly as it did when that was the only accepted form.
///
/// `steam_app_id` is what keeps tier 3 honest. Pass it wherever
/// [`steam_app_id_for_game`] can produce one: it is an exact identifier match
/// in Ludusavi's catalogue, and tier 3 writes to a directory that nothing on
/// this disk can contradict.
///
/// Resolution is four-tier:
///   1. **Exact match** — if Ludusavi reports a file whose path ends with
///      `rel`, use it (handles conflicts / re-restores of an existing save).
///   2. **Save root** — for a nested `rel`, anchor it on the root the game's
///      discovered saves share, so `slot2/save.dat` lands in `slot2/`.
///   3. **Manifest** — expand the save locations Ludusavi's own catalogue
///      lists for the game and pick the best fit. This tier needs nothing on
///      disk, which is the whole point: on a machine the game has never run on
///      there is no save to match and no sibling to sit beside, and that is
///      exactly when someone is restoring a backup.
///   4. **Sibling directory** — otherwise place the save next to the game's
///      other saves (they all live in one folder).
///
/// `Err` when the game is not in Ludusavi's catalogue, when its catalogue
/// entry names no location this machine has, or when Ludusavi can't be found
/// or fails — each with its own message, because "install the game and launch
/// it once" is useless advice for a game Ludusavi has never heard of. Matching
/// is case-sensitive on Unix, case-insensitive on Windows.
pub fn find_pc_save_destination(
    game_name: &str,
    steam_app_id: Option<&str>,
    rel: &str,
    install_dir: Option<&Path>,
    wine_prefix: Option<&Path>,
) -> Result<PathBuf, String> {
    let ludusavi = find_ludusavi().ok_or_else(|| {
        "Drop needs Ludusavi to know where PC games keep their save files, and it is not \
         installed. Open the game's Cloud Saves panel and choose Install Ludusavi."
            .to_string()
    })?;

    // Resolve to the canonical manifest title before the exact-match
    // backup, same as scan_pc_saves — otherwise a branded display name
    // restores to nowhere even though the save exists. The Steam app id is the
    // tier that matters here: this resolver is the only one that will write to
    // a directory nothing on disk vouches for, so it must not be the one
    // running on `--normalized` alone.
    let canonical = resolve_canonical_title(&ludusavi, game_name, steam_app_id);
    let search_name = canonical.as_deref().unwrap_or(game_name);

    let wine_prefix_str = wine_prefix.map(|p| p.to_string_lossy().to_string());
    let mut args: Vec<String> = vec!["backup".into(), "--preview".into(), "--api".into()];
    if let Some(p) = wine_prefix_str.as_deref() {
        args.push("--wine-prefix".into());
        args.push(p.to_string());
    }
    args.push(search_name.to_string());

    let output = std::process::Command::new(&ludusavi)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run Ludusavi: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Ludusavi error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse Ludusavi output: {e}"))?;

    // Single pass: short-circuit on a path that ends with `rel`, otherwise
    // gather fallback directories so a not-yet-existing save can be dropped
    // next to the game's other saves. Prefer a directory that already holds a
    // file with the same extension (so e.g. a ".sav" lands among ".sav" files,
    // not in a sibling "config" folder) and fall back to the first dir we saw.
    let rel_path = PathBuf::from(rel.replace('\\', "/"));
    let basename = rel_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(rel)
        .to_string();
    let rel_tail: Vec<String> = rel_path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    // A path "ends with" rel when its trailing components match one for one.
    let ends_with_rel = |path: &Path| -> bool {
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        parts.len() >= rel_tail.len()
            && parts[parts.len() - rel_tail.len()..]
                .iter()
                .zip(rel_tail.iter())
                .all(|(a, b)| same_path_name(a, b))
    };

    let target_ext = Path::new(&basename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    let mut sibling_dir: Option<PathBuf> = None;
    let mut ext_match_dir: Option<PathBuf> = None;
    let mut all_paths: Vec<PathBuf> = Vec::new();
    if let Some(games) = json.get("games").and_then(|g| g.as_object()) {
        for (_name, game_data) in games {
            if let Some(game_files) = game_data.get("files").and_then(|f| f.as_object()) {
                for (file_path, _file_data) in game_files {
                    let path = PathBuf::from(file_path);
                    if is_save_denylisted(&path) {
                        continue;
                    }
                    if ends_with_rel(&path) {
                        return Ok(path);
                    }
                    if let Some(parent) = path.parent() {
                        if sibling_dir.is_none() {
                            sibling_dir = Some(parent.to_path_buf());
                        }
                        if ext_match_dir.is_none()
                            && target_ext.is_some()
                            && path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_ascii_lowercase())
                                == target_ext
                        {
                            ext_match_dir = Some(parent.to_path_buf());
                        }
                    }
                    all_paths.push(path);
                }
            }
        }
    }

    // A nested name only means anything relative to the root the game's saves
    // share — dropping `slot2/save.dat` beside some other slot's file would
    // restore it into the wrong slot.
    if rel_tail.len() > 1
        && let Some(root) = common_save_root(&all_paths)
    {
        return Ok(root.join(&rel_path));
    }

    // Nothing on disk matched. Ask the catalogue where this game writes, and
    // rank its answers against whatever Ludusavi did find here. On a machine
    // that has played the game this lands on the same folder the sibling
    // fallback would have; on a fresh one it is the only tier that can answer
    // at all.
    let found_dirs: Vec<PathBuf> = all_paths
        .iter()
        .filter_map(|p| p.parent().map(|d| d.to_path_buf()))
        .collect();
    let candidates = manifest_save_dirs(&ludusavi, search_name, install_dir, wine_prefix);
    if let Some(chosen) = choose_manifest_dir(&candidates, &rel_path, &found_dirs, |dir| {
        dir.is_dir()
    }) {
        // The catalogue is a prediction; saves on this disk are an
        // observation. Take the prediction when there is nothing here to
        // contradict it, or when the two agree. When they disagree — a store
        // layout the catalogue expresses with a placeholder we cannot resolve,
        // say — the folder the game demonstrably writes to wins.
        let agrees_with_disk = found_dirs.iter().any(|d| path_starts_with(d, &chosen.dir));
        if found_dirs.is_empty() || agrees_with_disk {
            return Ok(chosen.dir.join(&rel_path));
        }
    }

    // The catalogue had nothing usable. All of a game's PC saves share one
    // directory, so place it next to the ones Ludusavi did find.
    if let Some(dir) = ext_match_dir.or(sibling_dir) {
        return Ok(dir.join(&rel_path));
    }

    // Say which of the two dead ends this is. They need different things from
    // the user, and one generic message sent people to relaunch a game that
    // was never going to help.
    if canonical.is_none() {
        return Err(format!(
            "Ludusavi's list of games does not include {game_name:?}, so Drop does not know where \
             this PC keeps {basename:?} and cannot put it back. Your save is still on your Drop \
             server, and nothing on this PC was changed."
        ));
    }
    Err(format!(
        "Ludusavi lists {game_name:?}, but none of the save locations it names exist on this PC, \
         so Drop could not work out where to put {basename:?}. Install the game and launch it \
         once, then press Restore again. Your save stays on your Drop server either way."
    ))
}

/// Write a downloaded PC save file back to its original location.
/// PC save filenames carry a namespace prefix — strip it and restore to the
/// original path from the manifest, or use a fallback location.
pub fn write_downloaded_pc_save(
    filename: &str,
    data: &[u8],
    original_path: Option<&Path>,
    expected_hash: Option<&str>,
) -> Result<PathBuf, String> {
    // If we know the original path (from manifest), use it
    if let Some(orig) = original_path {
        if !already_matches(orig, expected_hash) {
            backup::replace_save_file(orig, data)?;
        }
        return Ok(orig.to_path_buf());
    }

    // Fallback: save to {DATA_ROOT_DIR}/pc-saves/<relative path>. Nothing reads
    // this directory — it is a holding pen for bytes we could not place, kept
    // so a download is never simply discarded. Uses the same resolver the DB
    // does so a debug build writes under `drop-debug` rather than scattering
    // orphans into the release install's data dir.
    let clean_name = decode_pc_relpath(filename)
        .ok_or_else(|| format!("Refusing to write save with an unsafe name: {filename}"))?;
    let fallback = database::db::DATA_ROOT_DIR
        .join("pc-saves")
        .join(clean_name);
    if !already_matches(&fallback, expected_hash) {
        backup::replace_save_file(&fallback, data)?;
    }
    Ok(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("drop-save-scan-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn encode_switch_relpath(rel: &Path) -> String {
        format!("{SWITCH_SAVE_PREFIX}{}", escape_relpath(rel))
    }

    /// `--normalized` strips year and edition suffixes, so one query can match
    /// two catalogue entries. Picking the first key wrote a restored save into
    /// the other game's save folder, and on a machine with nothing on disk
    /// there was no observation left to contradict it.
    #[test]
    fn an_ambiguous_ludusavi_match_is_refused_rather_than_guessed() {
        let one = br#"{"games":{"DOOM (2016)":{}}}"#;
        assert_eq!(
            resolve_found_title(one, "DOOM"),
            Some("DOOM (2016)".to_string()),
            "a single match is still the answer"
        );

        let two = br#"{"games":{"DOOM":{},"DOOM (2016)":{}}}"#;
        assert_eq!(
            resolve_found_title(two, "DOOM"),
            Some("DOOM".to_string()),
            "an exact match among several is unambiguous"
        );

        let neither = br#"{"games":{"DOOM (2016)":{},"DOOM Eternal":{}}}"#;
        assert_eq!(
            resolve_found_title(neither, "DOOM"),
            None,
            "two candidates and no exact match is not an answer"
        );

        assert_eq!(resolve_found_title(br#"{"games":{}}"#, "DOOM"), None);
        assert_eq!(resolve_found_title(b"not json", "DOOM"), None);
    }

    /// The five emulator-save call sites that share
    /// [`super::scope::resolve_emu_saves_root`] —
    /// writer (what RetroArch is pointed at), scanner, restorer, tombstone
    /// deleter, and discovery — must all land on the same directory. A
    /// disagreement here is invisible: the game saves happily into a tree the
    /// scan never looks at, and the user finds out when their progress is gone.
    #[test]
    fn every_emulator_save_path_agrees_on_the_user_scoped_root() {
        let root = tmpdir("user-scoped");
        let user = "user-a";

        // Discovery + writer: the path RetroArch's savefile_directory gets.
        let base = emu_saves_root(&root, Some(user), "g1");
        assert_eq!(base, root.join("drop-saves").join(user).join("g1"));
        let save = base.join("saves").join("gen.srm");
        write(&save, "v1");

        // Scanner.
        let found = scan_emu_saves(&root, Some(user), "g1", None);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].filename, "gen.srm");
        assert_eq!(found[0].path, save);

        // Restorer.
        let dest =
            write_downloaded_save(&root, Some(user), "g1", "gen.srm", "save", b"v2", None)
                .unwrap();
        assert_eq!(dest, save);
        assert_eq!(fs::read_to_string(&save).unwrap(), "v2");

        // Tombstone deleter.
        let deleted =
            delete_local_emu_save_for_tombstone(&root, Some(user), "g1", "gen.srm").unwrap();
        assert_eq!(deleted.as_deref(), Some(save.as_path()));
        assert!(!save.exists());
    }

    /// The headline defect this scoping fixes: with one shared PC, account B's
    /// launch must not see, overwrite, or delete account A's saves.
    #[test]
    fn one_accounts_emulator_saves_are_invisible_to_another() {
        let root = tmpdir("two-users");
        let a_save = emu_saves_root(&root, Some("user-a"), "g1")
            .join("saves")
            .join("gen.srm");
        write(&a_save, "user a's progress");

        assert!(
            scan_emu_saves(&root, Some("user-b"), "g1", None).is_empty(),
            "account B can see account A's saves"
        );

        // B has no copy of this save, so B acting on a tombstone for it must
        // find nothing rather than reaching across into A's tree.
        assert_eq!(
            delete_local_emu_save_for_tombstone(&root, Some("user-b"), "g1", "gen.srm").unwrap(),
            None
        );
        assert!(a_save.is_file(), "B's tombstone deleted A's save");

        let dest =
            write_downloaded_save(&root, Some("user-b"), "g1", "gen.srm", "save", b"b", None)
                .unwrap();
        assert_ne!(dest, a_save);
        assert_eq!(fs::read_to_string(&a_save).unwrap(), "user a's progress");
    }

    /// `saveType` is entirely client-supplied on upload and the server does no
    /// game-ownership check, so any account can create a `pc` row against an
    /// emulator game's id. That row used to fall through to `saves/` and land
    /// inside the launching account's own per-user emulator tree, where a
    /// filename match drives a conflict or an overwrite of a real save.
    #[test]
    fn a_foreign_save_type_is_never_written_into_the_emulator_tree() {
        let root = tmpdir("bad-save-type");
        let err = write_downloaded_save(&root, Some("user-a"), "g1", "gen.srm", "pc", b"x", None)
            .unwrap_err();
        assert!(err.contains("unexpected save type"), "{err}");
        assert!(
            !emu_saves_root(&root, Some("user-a"), "g1").join("saves").exists(),
            "a foreign row created a directory in another account's emulator tree"
        );

        for save_type in ["save", "state"] {
            write_downloaded_save(&root, Some("user-a"), "g1", "gen.srm", save_type, b"x", None)
                .unwrap();
        }
    }

    /// Signed out there is no identity to scope by, so the path is exactly what
    /// every build before per-user scoping wrote. Those saves are never synced
    /// (the sync paths refuse without an identity); the migration adopts them.
    #[test]
    fn the_signed_out_layout_is_the_legacy_path() {
        let root = Path::new("/emulators/retroarch");
        assert_eq!(
            emu_saves_root(root, None, "g1"),
            root.join("drop-saves").join("g1")
        );
    }

    #[test]
    fn switch_relpath_round_trips() {
        let rel = Path::new("user/nand/user/save/0000000000000000/abc/0100000000010000/data%1");
        let encoded = encode_switch_relpath(rel);
        assert!(!encoded.contains('/'), "encoded name still has a separator");
        assert_eq!(decode_switch_relpath(&encoded).unwrap(), rel);
    }

    #[test]
    fn switch_relpath_rejects_traversal() {
        let escaping = format!("{SWITCH_SAVE_PREFIX}..%2F..%2Fetc%2Fpasswd");
        assert!(decode_switch_relpath(&escaping).is_none());
        // A plain RetroArch save name is not a Switch name.
        assert!(decode_switch_relpath("Game Name.srm").is_none());
    }

    #[test]
    fn switch_relpath_rejects_a_leading_separator() {
        // On Windows a rooted-but-prefixless path is NOT `is_absolute()`, so
        // the `..`-only guard let it through and `Path::join` then dropped
        // everything after the drive letter: `C:\Emus\Eden`.join("/etc/passwd")
        // is `C:/etc/passwd`, and `\Windows\System32\evil.dll` writes straight
        // into System32.
        for body in [
            "%2Fetc%2Fpasswd",
            "%2FWindows%2FSystem32%2Fevil.dll",
            "%2FUsers%2Fme%2FStartup%2Fx.exe",
        ] {
            let name = format!("{SWITCH_SAVE_PREFIX}{body}");
            assert!(decode_switch_relpath(&name).is_none(), "{name}");
        }
    }

    /// The write-back side of the title-id scoping. `scan_emu_saves` only ever
    /// reports files under the launched title's own NAND directory, so those
    /// are the only NAND rows that may be written back — anything else is a
    /// leftover of the old whole-NAND sweep and restoring it clobbers another
    /// title's live save, the system NAND, or sdmc.
    #[test]
    fn only_the_launched_titles_nand_rows_may_be_written_back() {
        let title = "0100aaaabbbb0000";
        let in_scope = [
            // <space>/<user>/<titleid>
            "user/nand/user/save/0000000000000000/user1/0100AAAABBBB0000/00",
            // account/<user>/<titleid>
            "user/nand/user/save/account/user1/0100aaaabbbb0000/sub/dir/file",
            // device/<titleid>
            "user/nand/user/save/device/0100aaaabbbb0000/00",
        ];
        for rel in in_scope {
            let name = encode_switch_relpath(Path::new(rel));
            assert!(
                switch_cloud_row_in_scope(Some(title), &name),
                "should be in scope: {rel}"
            );
            assert!(
                !switch_cloud_row_in_scope(None, &name),
                "nothing from the NAND is in scope without a title id: {rel}"
            );
        }

        let out_of_scope = [
            // Another title.
            "user/nand/user/save/0000000000000000/user1/0100ccccdddd0000/00",
            // The emulator's own system NAND.
            "user/nand/system/save/8000000000000030",
            // sdmc (screenshots, mods, homebrew).
            "user/sdmc/Nintendo/Album/shot.jpg",
            // Right name, too deep to be a save-data root.
            "user/nand/user/save/a/b/c/0100aaaabbbb0000/00",
            // The title directory itself, with no file under it.
            "user/nand/user/save/device/0100aaaabbbb0000",
        ];
        for rel in out_of_scope {
            let name = encode_switch_relpath(Path::new(rel));
            assert!(
                !switch_cloud_row_in_scope(Some(title), &name),
                "should be out of scope: {rel}"
            );
        }

        // Non-Switch names are not this check's business.
        assert!(switch_cloud_row_in_scope(None, "Game.srm"));
        assert!(switch_cloud_row_in_scope(Some(title), "pc__slot1%2Fsave.dat"));
    }

    #[test]
    fn cloud_filenames_inherit_the_scan_denylist() {
        for name in [
            "gen.srm.bak",
            "gen.srm.bak.1700000000",
            "Game.state.auto",
            "Game.state.auto.png",
            "Game.state.png",
            "pc__Player.log",
            &format!("{SWITCH_SAVE_PREFIX}user%2Fnand%2Fsave%2Fdata.bak.42"),
        ] {
            assert!(is_denylisted_cloud_filename(name), "{name} should be refused");
        }
        for name in [
            "gen.srm",
            "Game.state1",
            "pc__gen.sav",
            &format!("{SWITCH_SAVE_PREFIX}user%2Fnand%2Fsave%2Fdata"),
        ] {
            assert!(!is_denylisted_cloud_filename(name), "{name} should be kept");
        }
    }

    #[test]
    fn an_identical_download_writes_nothing_and_leaves_no_backup() {
        let root = tmpdir("same-bytes");
        let save = root.join("drop-saves/g1/saves/gen.srm");
        write(&save, "same");
        let hash = md5_file(&save).unwrap();

        write_downloaded_save(&root, None, "g1", "gen.srm", "save", b"same", Some(&hash)).unwrap();

        let backups: Vec<String> = fs::read_dir(save.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| backup::is_backup_artifact(n))
            .collect();
        assert!(backups.is_empty(), "{backups:?}");
        assert_eq!(fs::read_to_string(&save).unwrap(), "same");

        // A genuinely different payload still writes, and still backs up.
        write_downloaded_save(&root, None, "g1", "gen.srm", "save", b"new", Some("deadbeef")).unwrap();
        assert_eq!(fs::read_to_string(&save).unwrap(), "new");
    }

    #[test]
    fn emu_scan_recurses_into_subdirectories() {
        let root = tmpdir("recurse");
        write(&root.join("drop-saves/g1/saves/top.srm"), "a");
        write(&root.join("drop-saves/g1/saves/nested/deep/inner.srm"), "b");
        let found = scan_emu_saves(&root, None, "g1", None);
        let names: Vec<&str> = found.iter().map(|f| f.filename.as_str()).collect();
        assert!(names.contains(&"top.srm"), "{names:?}");
        // Nested files keep their path, not just their basename — two cores'
        // "inner.srm" used to collapse onto one cloud row.
        assert!(names.contains(&"nested%2Fdeep%2Finner.srm"), "{names:?}");
    }

    /// Two subdirectories with the same basename must stay two cloud rows,
    /// and each must restore into its own subdirectory.
    #[test]
    fn nested_drop_saves_keep_separate_identities() {
        let root = tmpdir("nested-identity");
        write(&root.join("drop-saves/g1/saves/slot1/save.dat"), "one");
        write(&root.join("drop-saves/g1/saves/slot2/save.dat"), "two");

        let mut names: Vec<String> = scan_emu_saves(&root, None, "g1", None)
            .into_iter()
            .map(|f| f.filename)
            .collect();
        names.sort();
        assert_eq!(names, vec!["slot1%2Fsave.dat", "slot2%2Fsave.dat"]);

        let dest =
            write_downloaded_save(&root, None, "g1", &names[1], "save", b"restored", None).unwrap();
        assert_eq!(dest, root.join("drop-saves/g1/saves/slot2/save.dat"));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "restored");
        // The other slot is untouched.
        assert_eq!(
            fs::read_to_string(root.join("drop-saves/g1/saves/slot1/save.dat")).unwrap(),
            "one"
        );
    }

    /// Rows written before nested paths were encoded are bare basenames.
    /// They must still restore to the flat top level.
    #[test]
    fn legacy_flat_emu_names_still_restore() {
        let root = tmpdir("legacy-flat");
        let dest = write_downloaded_save(&root, None, "g1", "gen.srm", "save", b"v1", None).unwrap();
        assert_eq!(dest, root.join("drop-saves/g1/saves/gen.srm"));
    }

    #[test]
    fn emu_relpath_refuses_to_escape_the_saves_dir() {
        for name in ["..%2F..%2Fevil.dll", "%2FWindows%2FSystem32%2Fevil.dll"] {
            assert!(decode_emu_relpath(name).is_none(), "{name}");
        }
        // A Switch name is not an emulator-relative name.
        assert!(decode_emu_relpath(&format!("{SWITCH_SAVE_PREFIX}a")).is_none());
    }

    #[test]
    fn switch_nand_saves_are_scanned_and_restorable() {
        let root = tmpdir("switch");
        let save = root.join(
            "user/nand/user/save/0000000000000000/00000000000000000000000000000000/0100000000010000/data",
        );
        write(&save, "save bytes");

        let found = scan_emu_saves(&root, None, "g1", Some("0100000000010000"));
        assert_eq!(found.len(), 1, "{found:?}");
        let entry = &found[0];
        assert!(entry.filename.starts_with(SWITCH_SAVE_PREFIX));
        assert_eq!(entry.path, save);

        // The encoded name must restore to exactly where it came from.
        let dest = write_downloaded_save(
            &root,
            None,
            "g1",
            &entry.filename,
            &entry.save_type,
            b"new bytes",
            None,
        )
        .unwrap();
        assert_eq!(dest, save);
        assert_eq!(fs::read_to_string(&save).unwrap(), "new bytes");
    }

    #[test]
    fn emu_walk_skips_backups_and_retroarch_churn() {
        let root = tmpdir("denylist");
        write(&root.join("drop-saves/g1/saves/real.srm"), "keep");
        write(&root.join("drop-saves/g1/saves/real.srm.bak"), "old slot");
        write(&root.join("drop-saves/g1/saves/real.srm.bak.1700000000"), "ts");
        write(&root.join("drop-saves/g1/saves/real.srm.drop-tmp.1"), "tmp");
        write(&root.join("drop-saves/g1/states/real.state.auto"), "auto");
        write(&root.join("drop-saves/g1/states/real.state.auto.png"), "shot");
        write(&root.join("drop-saves/g1/states/real.state.png"), "shot");
        write(&root.join("drop-saves/g1/states/real.state1"), "keep");

        let mut names: Vec<String> = scan_emu_saves(&root, None, "g1", None)
            .into_iter()
            .map(|f| f.filename)
            .collect();
        names.sort();
        assert_eq!(names, vec!["real.srm", "real.state1"], "{names:?}");
    }

    #[test]
    fn denylist_keeps_saves_that_merely_look_like_artifacts() {
        for name in ["Backup Quest.srm", "state.auto.sav", "my.state1"] {
            assert!(
                !is_save_denylisted(Path::new(name)),
                "{name} should be kept"
            );
        }
    }

    #[test]
    fn restoring_twice_keeps_both_previous_versions() {
        let root = tmpdir("restore-twice");
        let save = root.join("drop-saves/g1/saves/gen.srm");
        write(&save, "v1");

        write_downloaded_save(&root, None, "g1", "gen.srm", "save", b"v2", None).unwrap();
        write_downloaded_save(&root, None, "g1", "gen.srm", "save", b"v3", None).unwrap();

        assert_eq!(fs::read_to_string(&save).unwrap(), "v3");
        let mut backups: Vec<String> = fs::read_dir(save.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| backup::is_backup_artifact(&e.file_name().to_string_lossy()))
            .map(|e| fs::read_to_string(e.path()).unwrap())
            .collect();
        backups.sort();
        assert_eq!(backups, vec!["v1", "v2"]);
    }

    #[test]
    fn emu_scan_is_bounded_by_depth() {
        let root = tmpdir("depth");
        let mut deep = root.join("drop-saves/g1/saves");
        for _ in 0..(EMU_SCAN_MAX_DEPTH + 3) {
            deep = deep.join("d");
        }
        write(&deep.join("too-deep.srm"), "x");
        write(&root.join("drop-saves/g1/saves/shallow.srm"), "x");
        let found = scan_emu_saves(&root, None, "g1", None);
        let names: Vec<&str> = found.iter().map(|f| f.filename.as_str()).collect();
        assert_eq!(names, vec!["shallow.srm"], "{names:?}");
    }

    // ── Switch NAND scoping (D6) ───────────────────────────────────────

    /// Build a NAND holding two titles' saves plus a system save and an sdmc
    /// file, all under one emulator install — the shape that made every
    /// Switch game's cloud rows contain every other Switch game's saves.
    fn switch_nand(tag: &str) -> PathBuf {
        let root = tmpdir(tag);
        let user = "00000000000000000000000000000000";
        write(
            &root.join(format!(
                "user/nand/user/save/0000000000000000/{user}/0100000000010000/data"
            )),
            "game A",
        );
        write(
            &root.join(format!(
                "user/nand/user/save/0000000000000000/{user}/010000000002a000/data"
            )),
            "game B",
        );
        write(&root.join("user/nand/system/save/8000000000000030"), "system");
        write(&root.join("user/sdmc/Nintendo/album/pic.jpg"), "screenshot");
        root
    }

    #[test]
    fn switch_scan_sees_only_the_launched_title() {
        let root = switch_nand("switch-scope");
        let found = scan_emu_saves(&root, None, "gameA", Some("0100000000010000"));
        let names: Vec<&str> = found.iter().map(|f| f.filename.as_str()).collect();
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].contains("0100000000010000"), "{names:?}");
        // The other title, the system saves and sdmc are all out of scope.
        assert!(!names[0].contains("010000000002a000"), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("system")), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("sdmc")), "{names:?}");
    }

    /// The whole point of the fix: with no title id, syncing nothing beats
    /// filing every installed title's saves under one game.
    #[test]
    fn switch_scan_takes_nothing_when_the_title_id_is_unknown() {
        let root = switch_nand("switch-unknown");
        assert!(scan_emu_saves(&root, None, "gameA", None).is_empty());
        // A drop-saves file for the same game is still picked up.
        write(&root.join("drop-saves/gameA/saves/gen.srm"), "x");
        let names: Vec<String> = scan_emu_saves(&root, None, "gameA", None)
            .into_iter()
            .map(|f| f.filename)
            .collect();
        assert_eq!(names, vec!["gen.srm"]);
    }

    #[test]
    fn switch_scan_finds_the_account_and_device_layouts() {
        let root = tmpdir("switch-future-layout");
        write(
            &root.join("user/nand/user/save/account/abc/0100000000010000/data"),
            "account save",
        );
        write(
            &root.join("user/nand/user/save/device/0100000000010000/data"),
            "device save",
        );
        write(
            &root.join("user/nand/user/save/device/010000000002a000/data"),
            "other title",
        );
        let found = scan_emu_saves(&root, None, "gameA", Some("0100000000010000"));
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found.iter().all(|f| f.filename.contains("0100000000010000")),
            "{found:?}"
        );
    }

    #[test]
    fn title_ids_are_read_out_of_rom_paths() {
        assert_eq!(
            switch_title_id_from_path("D:/ROMs/Some Game [0100ABCDEF012000][v0].nsp").as_deref(),
            Some("0100abcdef012000")
        );
        assert_eq!(
            switch_title_id_from_path(r"C:\ROMs\0100000000010000\game.xci").as_deref(),
            Some("0100000000010000")
        );
    }

    #[test]
    fn title_id_extraction_refuses_anything_it_is_not_sure_about() {
        for path in [
            // No id at all.
            "D:/ROMs/Some Game.nsp",
            // A 32-char hash is a longer run, not a title id.
            "D:/ROMs/Game.0123456789abcdef0123456789abcdef.nsp",
            // Update title id (ends 800), not a base id.
            "D:/ROMs/Game [0100abcdef012800].nsp",
            // Does not start with 01.
            "D:/ROMs/Game [0500abcdef012000].nsp",
            // Two different base ids: ambiguous, so refuse to guess.
            "D:/ROMs/0100abcdef012000/Game [0100111122223000].nsp",
        ] {
            assert!(switch_title_id_from_path(path).is_none(), "{path}");
        }
    }

    // ── PC save path identity (D7) ─────────────────────────────────────

    #[test]
    fn pc_saves_in_one_directory_keep_their_legacy_names() {
        let root = Path::new("/home/me/.local/share/Game");
        let paths = vec![root.join("slot.sav"), root.join("prefs.sav")];
        let save_root = common_save_root(&paths).unwrap();
        assert_eq!(save_root, root);
        assert_eq!(encode_pc_filename(Some(&save_root), &paths[0]), "pc__slot.sav");
    }

    #[test]
    fn pc_saves_in_sibling_directories_stay_distinct() {
        let root = Path::new("/home/me/.local/share/Game");
        let paths = vec![
            root.join("slot1/save.dat"),
            root.join("slot2/save.dat"),
        ];
        let save_root = common_save_root(&paths).unwrap();
        assert_eq!(save_root, root);
        let a = encode_pc_filename(Some(&save_root), &paths[0]);
        let b = encode_pc_filename(Some(&save_root), &paths[1]);
        assert_ne!(a, b, "two distinct saves collapsed onto one cloud row");
        assert!(!a.contains('/') && !a.contains('\\'), "{a}");
        assert_eq!(decode_pc_relpath(&a).unwrap(), Path::new("slot1/save.dat"));
        assert_eq!(decode_pc_relpath(&b).unwrap(), Path::new("slot2/save.dat"));
    }

    #[test]
    fn legacy_pc_names_decode_to_themselves() {
        for (name, expected) in [
            ("pc__gen.sav", "gen.sav"),
            ("pc/gen.sav", "gen.sav"),
            ("gen.sav", "gen.sav"),
            ("pc__100%.sav", "100%.sav"),
        ] {
            assert_eq!(
                decode_pc_relpath(name).unwrap(),
                Path::new(expected),
                "{name}"
            );
        }
        assert!(decode_pc_relpath("pc__..%2F..%2Fevil.dll").is_none());
    }

    #[test]
    fn a_percent_in_a_real_name_survives_the_round_trip() {
        let root = Path::new("/saves");
        let path = root.join("100%2Fnot-a-separator.sav");
        let encoded = encode_pc_filename(Some(root), &path);
        assert_eq!(
            decode_pc_relpath(&encoded).unwrap(),
            Path::new("100%2Fnot-a-separator.sav")
        );
    }

    // ── Manifest-driven restore destinations ───────────────────────────
    //
    // These are the fresh-machine path: no game has run, so nothing below
    // may depend on a file existing.

    fn test_roots() -> ManifestRoots {
        ManifestRoots {
            base: Some(PathBuf::from("/games/Foo")),
            home: Some(PathBuf::from("/home/p")),
            win_app_data: Some(PathBuf::from("/home/p/AppData/Roaming")),
            win_local_app_data: Some(PathBuf::from("/home/p/AppData/Local")),
            win_documents: Some(PathBuf::from("/home/p/Documents")),
            os_user_name: Some("p".into()),
            ..Default::default()
        }
    }

    #[test]
    fn a_glob_pattern_resolves_to_the_directory_above_it() {
        let dir =
            expand_manifest_pattern("<winLocalAppData>/XV83/Saved/SaveGames/**/*.sav", &test_roots())
                .unwrap();
        assert_eq!(
            dir.dir,
            PathBuf::from("/home/p/AppData/Local/XV83/Saved/SaveGames")
        );
        assert_eq!(dir.file_name, None);
        // `**` is the first unresolved segment, and it says nothing about the
        // kind of file underneath it.
        assert_eq!(dir.leaf_ext, None);
        assert_eq!(dir.depth, 3);
    }

    #[test]
    fn a_leaf_glob_keeps_its_extension() {
        let dir = expand_manifest_pattern("<winDocuments>/My Games/Foo/*.sav", &test_roots())
            .unwrap();
        assert_eq!(
            dir.dir,
            PathBuf::from("/home/p/Documents/My Games/Foo")
        );
        assert_eq!(dir.leaf_ext.as_deref(), Some("sav"));
    }

    #[test]
    fn a_named_file_is_split_off_but_a_dotfile_directory_is_not() {
        let file = expand_manifest_pattern("<winAppData>/Foo/save.dat", &test_roots()).unwrap();
        assert_eq!(file.dir, PathBuf::from("/home/p/AppData/Roaming/Foo"));
        assert_eq!(file.file_name.as_deref(), Some("save.dat"));

        // `.minecraft` is a directory. Reading its leading dot as an extension
        // would restore a whole save tree one level too high.
        let dotted = expand_manifest_pattern("<home>/.minecraft", &test_roots()).unwrap();
        assert_eq!(dotted.dir, PathBuf::from("/home/p/.minecraft"));
        assert_eq!(dotted.file_name, None);
    }

    #[test]
    fn a_placeholder_this_machine_has_no_value_for_is_skipped() {
        let roots = ManifestRoots {
            home: Some(PathBuf::from("/home/p")),
            ..Default::default()
        };
        // No Wine prefix and no Windows host: there is no %APPDATA% here, and
        // guessing one would restore a save into a directory nothing reads.
        assert!(expand_manifest_pattern("<winAppData>/Foo/save.dat", &roots).is_none());
        // `<root>` and `<game>` mean "wherever a launcher put this", which is
        // not a question the manifest answers.
        assert!(expand_manifest_pattern("<root>/Foo/save.dat", &roots).is_none());
        assert!(expand_manifest_pattern("<base>/save.dat", &roots).is_none());
    }

    #[test]
    fn the_user_name_placeholder_is_substituted_mid_path() {
        let dir =
            expand_manifest_pattern("<winDocuments>/Foo/<osUserName>/slot1", &test_roots())
                .unwrap();
        assert_eq!(dir.dir, PathBuf::from("/home/p/Documents/Foo/p/slot1"));
    }

    /// The fresh-machine case in one test: nothing exists on disk, Ludusavi
    /// found nothing, and the restore still lands somewhere sensible.
    #[test]
    fn a_destination_is_chosen_with_nothing_on_disk() {
        let candidates = vec![
            ManifestSaveDir {
                dir: PathBuf::from("/home/p/Documents/Foo/Screenshots"),
                file_name: None,
                leaf_ext: Some("png".into()),
                depth: 2,
            },
            ManifestSaveDir {
                dir: PathBuf::from("/home/p/Documents/Foo/Saves"),
                file_name: None,
                leaf_ext: Some("sav".into()),
                depth: 2,
            },
        ];
        let chosen = choose_manifest_dir(&candidates, Path::new("slot1.sav"), &[], |_| false);
        assert_eq!(
            chosen.map(|c| c.dir.join("slot1.sav")),
            Some(PathBuf::from("/home/p/Documents/Foo/Saves/slot1.sav"))
        );
    }

    #[test]
    fn a_pattern_naming_this_exact_file_wins() {
        let candidates = vec![
            ManifestSaveDir {
                dir: PathBuf::from("/home/p/Documents/Foo/Saves"),
                file_name: None,
                leaf_ext: Some("sav".into()),
                depth: 9,
            },
            ManifestSaveDir {
                dir: PathBuf::from("/home/p/AppData/Roaming/Foo"),
                file_name: Some("profile.sav".into()),
                leaf_ext: Some("sav".into()),
                depth: 1,
            },
        ];
        let chosen = choose_manifest_dir(&candidates, Path::new("profile.sav"), &[], |_| false);
        assert_eq!(
            chosen.map(|c| c.dir.join("profile.sav")),
            Some(PathBuf::from("/home/p/AppData/Roaming/Foo/profile.sav"))
        );
    }

    /// A machine that HAS played the game must keep landing where it always
    /// did. Real saves on disk outrank every catalogue guess.
    #[test]
    fn saves_found_on_this_machine_outrank_the_catalogue() {
        let candidates = vec![
            ManifestSaveDir {
                dir: PathBuf::from("/home/p/Documents/Foo/Saves"),
                file_name: None,
                leaf_ext: Some("sav".into()),
                depth: 2,
            },
            ManifestSaveDir {
                dir: PathBuf::from("/home/p/AppData/Roaming/Foo"),
                file_name: None,
                leaf_ext: None,
                depth: 1,
            },
        ];
        let found = vec![PathBuf::from("/home/p/AppData/Roaming/Foo/other.sav")];
        let chosen = choose_manifest_dir(&candidates, Path::new("slot1.sav"), &found, |_| false);
        assert_eq!(
            chosen.map(|c| c.dir.join("slot1.sav")),
            Some(PathBuf::from("/home/p/AppData/Roaming/Foo/slot1.sav"))
        );
    }

    #[test]
    fn a_nested_save_name_keeps_its_subdirectory() {
        let candidates = vec![ManifestSaveDir {
            dir: PathBuf::from("/home/p/Documents/Foo"),
            file_name: None,
            leaf_ext: None,
            depth: 1,
        }];
        let chosen =
            choose_manifest_dir(&candidates, Path::new("slot2/save.dat"), &[], |_| false);
        assert_eq!(
            chosen.map(|c| c.dir.join("slot2/save.dat")),
            Some(PathBuf::from("/home/p/Documents/Foo/slot2/save.dat"))
        );
    }

    #[test]
    fn no_candidates_means_no_destination() {
        assert!(choose_manifest_dir(&[], Path::new("slot1.sav"), &[], |_| true).is_none());
    }

    #[test]
    fn a_when_clause_scopes_an_entry_to_one_platform() {
        let windows_only = serde_json::json!({ "when": [{ "os": "windows" }] });
        assert!(entry_applies_to_os(&windows_only, "windows"));
        assert!(!entry_applies_to_os(&windows_only, "linux"));

        // A store-only clause restricts where the game came from, not which
        // platform it runs on.
        let store_only = serde_json::json!({ "when": [{ "store": "steam" }] });
        assert!(entry_applies_to_os(&store_only, "linux"));

        assert!(entry_applies_to_os(&serde_json::json!({}), "linux"));
    }

    #[test]
    fn config_only_entries_are_the_ones_tagged_config_and_not_save() {
        assert!(entry_is_config_only(
            &serde_json::json!({ "tags": ["config"] })
        ));
        assert!(!entry_is_config_only(
            &serde_json::json!({ "tags": ["config", "save"] })
        ));
        // Untagged defaults to save data per the manifest spec.
        assert!(!entry_is_config_only(&serde_json::json!({})));
    }
}
