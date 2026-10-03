use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

// Monotonic suffix so two concurrent writers never share a temp filename and
// clobber each other's file. Mirrors DROPDATA_WRITE_SEQ in drop_data.rs.
static MODDATA_WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

use database::platform::Platform;
use log::{error, info, warn};
use utils::{lock, path_guard};

pub type ModData = v1::ModData;

/// Directory (relative to the parent game's install dir) that holds one ledger
/// file per installed mod. A mod's files overlay directly into the parent's
/// install dir, so the mod's own resume ledger cannot live at the parent's
/// `.dropdata` (that belongs to the base game). Each mod's ledger instead lives
/// at `<parent install dir>/.mods/<mod game id>.moddata`.
///
/// Next to the ledger, two more things may exist per mod (both plain files,
/// kept out of the pot-encoded ledger on purpose so its format never changes):
///  - `<mod id>.pending`: present from the moment a download starts touching
///    the install dir until it completes. A ledger with this marker is an
///    unfinished install, never an installed mod.
///  - `<mod id>/originals/<path>`: the base game's own copy of every file the
///    mod overwrote, at its path relative to the install dir, so uninstalling
///    the mod puts the game back the way it was.
///  - `<mod id>/stale-originals`: install-relative paths (one per line,
///    lower-cased) whose copy in `originals/` is out of date and must never be
///    put back, because the base game has since rewritten or dropped that
///    file and the outdated copy could not be deleted. Usually absent.
pub static MODS_DIR: &str = ".mods";

/// Build the ledger path for a mod given the parent install dir and the mod's
/// game id.
pub fn moddata_path(base_path: &Path, mod_game_id: &str) -> PathBuf {
    base_path.join(MODS_DIR).join(format!("{mod_game_id}.moddata"))
}

/// The "download in progress or interrupted" marker for a mod. See `MODS_DIR`.
pub fn pending_marker_path(install_dir: &Path, mod_game_id: &str) -> PathBuf {
    install_dir.join(MODS_DIR).join(format!("{mod_game_id}.pending"))
}

/// Per-mod state folder (`.mods/<mod id>/`), which holds `originals/`.
pub fn mod_state_dir(install_dir: &Path, mod_game_id: &str) -> PathBuf {
    install_dir.join(MODS_DIR).join(mod_game_id)
}

/// Where the base game's copies of files this mod overwrote are kept.
pub fn originals_dir(install_dir: &Path, mod_game_id: &str) -> PathBuf {
    mod_state_dir(install_dir, mod_game_id).join("originals")
}

/// The list of outdated originals for a mod. See `MODS_DIR`.
fn stale_originals_path(install_dir: &Path, mod_game_id: &str) -> PathBuf {
    mod_state_dir(install_dir, mod_game_id).join("stale-originals")
}

pub mod v1 {
    use std::{collections::HashMap, path::PathBuf, sync::Mutex};

    use database::platform::Platform;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ModData {
        // The mod's OWN game id/version (a mod is a Game with type=Mod).
        pub game_id: String,
        pub game_version: String,
        pub target_platform: Platform,
        // The base game this mod overlays onto.
        pub parent_game_id: String,
        // If set, the executable (relative to the base game's install dir) to
        // launch while this mod is installed, instead of the game's normal one.
        // None for content mods that don't change the launch.
        pub launch_override: Option<String>,
        // NOTE: deliberately NO UserConfiguration here. `pot` cannot round-trip
        // a struct with `#[serde(default)]` fields (UserConfiguration has
        // several), so embedding it makes `read()` fail to decode what `write()`
        // produced. Mods don't need per-mod config anyway — the agent carries
        // its own. (This is the same latent bug that affects DropData resume.)
        // Completed-chunk map (resume ledger), same shape as DropData.
        pub contexts: Mutex<HashMap<String, bool>>,
        // Where the mod's files are written: the PARENT game's install dir
        // joined with the version's modInstallDir.
        pub base_path: PathBuf,
        // Where THIS ledger is serialized: <install dir>/.mods/<game_id>.moddata.
        pub meta_path: PathBuf,
        // Every file this mod may have written into base_path, POSIX-relative
        // (forward slashes). Recorded from the manifest BEFORE the first chunk
        // is written (and, during an update, still including the previous
        // version's files until the update completes), so a cancelled or
        // failed install can be removed as cleanly as a finished one. Ledgers
        // written by older clients only filled this in on completion; for
        // those an empty list means "unfinished, file list unknown".
        pub installed_files: Mutex<Vec<String>>,
        pub previously_installed_version: Option<String>,
    }

    impl ModData {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            game_id: String,
            game_version: String,
            target_platform: Platform,
            parent_game_id: String,
            launch_override: Option<String>,
            base_path: PathBuf,
            meta_path: PathBuf,
            previously_installed_version: Option<String>,
        ) -> Self {
            Self {
                game_id,
                game_version,
                target_platform,
                parent_game_id,
                launch_override,
                contexts: Mutex::new(HashMap::new()),
                base_path,
                meta_path,
                installed_files: Mutex::new(Vec::new()),
                previously_installed_version,
            }
        }
    }
}

impl ModData {
    /// The ledger for a download about to be queued. Only decides what the
    /// download asks the server for (a resume, or a delta from the installed
    /// version); `ModDownloadAgent::prepare_install_dir` reads the ledger
    /// again when the download actually starts, because anything can happen
    /// to it while the download waits in the queue.
    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        game_id: String,
        game_version: String,
        target_platform: Platform,
        parent_game_id: String,
        launch_override: Option<String>,
        base_path: PathBuf,
        meta_path: PathBuf,
    ) -> Self {
        match ModData::read(&meta_path) {
            Ok(v) => {
                // The ledger's file paths are relative to the folder the old
                // install wrote into. When the new one writes somewhere else,
                // nothing carries over: the old install is removed first and
                // the new one is a fresh, full download.
                let fresh_placement = install_dir_of(&meta_path)
                    .map(|install| {
                        let new = ModData::new(
                            game_id.clone(),
                            game_version.clone(),
                            target_platform,
                            parent_game_id.clone(),
                            None,
                            base_path.clone(),
                            meta_path.clone(),
                            None,
                        );
                        overlay_rel(install, &v).ok() != overlay_rel(install, &new).ok()
                    })
                    .unwrap_or(true);
                if fresh_placement {
                    return ModData::new(
                        game_id,
                        game_version,
                        target_platform,
                        parent_game_id,
                        launch_override,
                        base_path,
                        meta_path,
                        None,
                    );
                }
                // A different mod version invalidates the resume ledger — start
                // fresh but remember the old version so the manifest delta can
                // be requested. The old version's file list is carried over:
                // until the update completes, those files are still this mod's
                // (to remove on uninstall, and to keep out of the base game's
                // stale-file sweep). Completion drops the ones the new version
                // no longer ships (see `release_recorded`).
                if v.game_id != game_id || v.game_version != game_version {
                    let fresh = ModData::new(
                        game_id,
                        game_version,
                        target_platform,
                        parent_game_id,
                        launch_override,
                        base_path,
                        meta_path,
                        Some(v.game_version.clone()),
                    );
                    fresh.set_installed_files(v.get_installed_files());
                    return fresh;
                }
                // Same version: resume. Placement comes from the caller, not
                // the old ledger, so a library that moved since the download
                // started writes into where the game is now.
                let mut v = v;
                v.base_path = base_path;
                v.meta_path = meta_path;
                v.launch_override = launch_override;
                v.target_platform = target_platform;
                v
            }
            Err(e) => {
                // Usually "no ledger yet" (a fresh install). If the file EXISTS
                // and still failed to read it is corrupt; the download refuses
                // to start over it (prepare_install_dir), since nothing says
                // which files on disk are the mod's.
                if meta_path.exists() {
                    error!("corrupt .moddata for {game_id}: {e}");
                }
                ModData::new(
                    game_id,
                    game_version,
                    target_platform,
                    parent_game_id,
                    launch_override,
                    base_path,
                    meta_path,
                    None,
                )
            }
        }
    }

    /// Read a ledger. Its recorded `meta_path` is replaced with `meta_path`,
    /// and its overlay folder re-anchored to the install dir it was read
    /// from, so a ledger from a library that has since moved writes back to
    /// where it is now rather than to the old location.
    pub fn read(meta_path: &Path) -> Result<Self, io::Error> {
        let mut file = File::open(meta_path)?;

        let mut s = Vec::new();
        file.read_to_end(&mut s)?;

        let mut ledger: ModData = pot::from_slice(&s).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to decode mod data: {e}"),
            )
        })?;
        ledger.rebase(meta_path);
        Ok(ledger)
    }

    fn rebase(&mut self, actual_meta: &Path) {
        if self.meta_path == actual_meta {
            return;
        }
        if let (Some(recorded), Some(actual)) =
            (install_dir_of(&self.meta_path), install_dir_of(actual_meta))
            && !self.base_path.starts_with(actual)
            && let Ok(rel) = self.base_path.strip_prefix(recorded)
        {
            self.base_path = actual.join(rel);
        }
        self.meta_path = actual_meta.to_path_buf();
    }

    /// Atomic write to meta_path (unique temp + rename), mirroring
    /// DropData::write. The caller must ensure the `.mods` directory exists.
    pub fn try_write(&self) -> Result<(), io::Error> {
        let manifest_raw = pot::to_vec(&self).map_err(|e| {
            io::Error::other(format!("failed to serialize .moddata for {}: {e}", self.game_id))
        })?;

        let final_path = &self.meta_path;
        let seq = MODDATA_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp_path = self.meta_path.with_extension(format!("moddata.tmp.{seq}"));

        if let Err(e) = std::fs::write(&tmp_path, &manifest_raw) {
            // Best-effort cleanup of a temp file that may not even exist.
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }
        if let Err(e) = std::fs::rename(&tmp_path, final_path) {
            // Best-effort cleanup; the rename error is what matters.
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }
        Ok(())
    }

    /// `try_write`, logging instead of returning a failure. Used for the
    /// per-chunk resume ledger, where a missed write only costs a re-download.
    pub fn write(&self) {
        if let Err(e) = self.try_write() {
            error!("failed to write .moddata for {}: {e}", self.game_id);
        }
    }

    pub fn set_contexts(&self, completed_contexts: &[(String, bool)]) {
        *lock!(self.contexts) = completed_contexts
            .iter()
            .map(|s| (s.0.clone(), s.1))
            .collect();
    }
    pub fn set_context(&self, context: String, state: bool) {
        lock!(self.contexts).entry(context).insert_entry(state);
    }
    pub fn get_contexts(&self) -> HashMap<String, bool> {
        lock!(self.contexts).clone()
    }

    pub fn set_installed_files(&self, files: Vec<String>) {
        *lock!(self.installed_files) = files;
    }
    pub fn get_installed_files(&self) -> Vec<String> {
        lock!(self.installed_files).clone()
    }
}

/// Whether this ledger describes a finished install: no `.pending` marker, and
/// a file list. (Ledgers from older clients never have the marker; for them an
/// empty file list is what meant "unfinished".)
pub fn ledger_is_complete(install_dir: &Path, ledger: &ModData) -> bool {
    !pending_marker_path(install_dir, &ledger.game_id).exists()
        && !lock!(ledger.installed_files).is_empty()
}

/// A ledger written by an older client for a download that never finished:
/// chunks were written, but the file list was only ever recorded on
/// completion, so nothing says which files on disk are the mod's.
fn is_legacy_unfinished(ledger: &ModData) -> bool {
    lock!(ledger.installed_files).is_empty() && !lock!(ledger.contexts).is_empty()
}

/// The mod's overlay folder relative to `install_dir`, as a POSIX prefix
/// (empty for the install root). The ledger records absolute paths; when the
/// library has moved since, they are re-anchored through the install dir the
/// ledger itself was written under (`meta_path` is `<install>/.mods/<id>.moddata`).
pub fn overlay_rel(install_dir: &Path, ledger: &ModData) -> Result<String, String> {
    let recorded_install = ledger.meta_path.parent().and_then(Path::parent);
    let rel = ledger
        .base_path
        .strip_prefix(install_dir)
        .ok()
        .or_else(|| recorded_install.and_then(|r| ledger.base_path.strip_prefix(r).ok()))
        .ok_or_else(|| {
            format!(
                "mod {} is recorded at {}, which is not inside {}",
                ledger.game_id,
                ledger.base_path.display(),
                install_dir.display()
            )
        })?;
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => {
                return Err(format!(
                    "mod {} has an unsafe install folder {}",
                    ledger.game_id,
                    rel.display()
                ));
            }
        }
    }
    Ok(parts.join("/"))
}

fn root_rel(prefix: &str, file: &str) -> String {
    if prefix.is_empty() {
        file.to_string()
    } else {
        format!("{prefix}/{file}")
    }
}

/// Every `.moddata` ledger under `install_dir/.mods`, sorted by file name so
/// callers that pick "the first" are deterministic. A missing `.mods` folder is
/// an empty list; a ledger that fails to decode is returned as its error.
fn read_ledgers(install_dir: &Path) -> Result<Vec<(String, io::Result<ModData>)>, io::Error> {
    let entries = match std::fs::read_dir(install_dir.join(MODS_DIR)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".moddata") || !entry.path().is_file() {
            continue;
        }
        out.push((name, ModData::read(&entry.path())));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Every installed (or installing) mod's ledger under `install_dir`, sorted by
/// file name. Unreadable ledgers are logged and skipped.
pub fn installed_ledgers(install_dir: &Path) -> Result<Vec<ModData>, io::Error> {
    Ok(read_ledgers(install_dir)?
        .into_iter()
        .filter_map(|(name, ledger)| match ledger {
            Ok(l) => Some(l),
            Err(e) => {
                warn!("mod ledger {name} under {} is unreadable: {e}", install_dir.display());
                None
            }
        })
        .collect())
}

/// `<install dir>` for a ledger at `<install dir>/.mods/<id>.moddata`.
fn install_dir_of(meta_path: &Path) -> Option<&Path> {
    meta_path.parent().and_then(Path::parent)
}

/// Every mod with state under `install_dir/.mods`: one per ledger (readable or
/// not) and one per `<mod id>/` state folder (which can outlive its ledger
/// after a crash). Sorted, no duplicates.
fn mod_ids_under(install_dir: &Path) -> Result<Vec<String>, io::Error> {
    let entries = match std::fs::read_dir(install_dir.join(MODS_DIR)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            ids.push(name);
        } else if let Some(id) = name.strip_suffix(".moddata") {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Install-relative paths whose backup in this mod's `originals/` must never be
/// put back (see `MODS_DIR`), lower-cased. Missing file: none.
fn read_stale(install_dir: &Path, mod_game_id: &str) -> Result<HashSet<String>, String> {
    let path = stale_originals_path(install_dir, mod_game_id);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_lowercase)
            .collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(HashSet::new()),
        Err(e) => Err(format!("could not read {}: {e}", path.display())),
    }
}

/// Replace the stale list (atomic: temp file + rename). Empty removes it.
fn write_stale(install_dir: &Path, mod_game_id: &str, stale: &HashSet<String>) -> Result<(), String> {
    let path = stale_originals_path(install_dir, mod_game_id);
    if stale.is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("could not update {}: {e}", path.display())),
        };
    }
    let mut lines: Vec<&String> = stale.iter().collect();
    lines.sort();
    let body = lines.into_iter().fold(String::new(), |mut acc, l| {
        acc.push_str(l);
        acc.push('\n');
        acc
    });
    let seq = MODDATA_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp.{seq}"));
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&tmp, body))
        .and_then(|()| std::fs::rename(&tmp, &path));
    if let Err(e) = result {
        // Best-effort cleanup of a temp file that may not even exist.
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("could not update {}: {e}", path.display()));
    }
    Ok(())
}

fn mark_stale(install_dir: &Path, mod_game_id: &str, rels: &[String]) -> Result<(), String> {
    let mut stale = read_stale(install_dir, mod_game_id)?;
    stale.extend(rels.iter().map(|r| r.to_lowercase()));
    write_stale(install_dir, mod_game_id, &stale)
}

fn unmark_stale(install_dir: &Path, mod_game_id: &str, rels: &[String]) -> Result<(), String> {
    let mut stale = read_stale(install_dir, mod_game_id)?;
    let before = stale.len();
    for r in rels {
        stale.remove(&r.to_lowercase());
    }
    if stale.len() == before {
        return Ok(());
    }
    write_stale(install_dir, mod_game_id, &stale)
}

/// Refuses paths that would leave the install dir through a symlink or
/// junction. `path_guard::join_within` only checks how a path is spelled;
/// this checks where its folder really is, before anything is deleted, moved
/// or overwritten there.
struct Contained {
    root_real: PathBuf,
}

impl Contained {
    fn new(install_dir: &Path) -> Result<Self, String> {
        install_dir
            .canonicalize()
            .map(|root_real| Self { root_real })
            .map_err(|e| format!("cannot resolve the game folder {}: {e}", install_dir.display()))
    }

    fn check(&self, path: &Path) -> Result<(), String> {
        path_guard::ensure_parent_within(&self.root_real, path)
            .map_err(|_| format!("{} is outside the game folder", path.display()))
    }
}

/// For each file another mod's ledger lists (lower-cased, relative to the
/// install dir), the ids of those mods in ledger order. `exclude` is the mod
/// asking. Unfinished installs count: their files are on disk too.
///
/// Fails when another mod's claims cannot be known: its ledger is unreadable,
/// its overlay folder is not inside `install_dir`, or it is an older client's
/// unfinished download with no file list. Any file might be that mod's then,
/// so callers must not back up, delete or restore anything.
fn claims_by_others(install_dir: &Path, exclude: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let ledgers = read_ledgers(install_dir)
        .map_err(|e| format!("could not list the other mods on this game: {e}"))?;
    let mut claims: HashMap<String, Vec<String>> = HashMap::new();
    for (name, other) in ledgers {
        let other = match other {
            Ok(l) => l,
            Err(e) => {
                let id = name.strip_suffix(".moddata").unwrap_or(&name);
                if id == exclude {
                    continue;
                }
                warn!("mod ledger {name} under {} is unreadable: {e}", install_dir.display());
                return Err(format!(
                    "another mod's file list ({MODS_DIR}/{name}) cannot be read, so Drop cannot tell which files it uses"
                ));
            }
        };
        if other.game_id == exclude {
            continue;
        }
        if is_legacy_unfinished(&other) {
            return Err(format!(
                "mod {} has an unfinished download with no file list, so Drop cannot tell which files it uses; remove it first",
                other.game_id
            ));
        }
        let prefix = overlay_rel(install_dir, &other)?;
        for f in other.get_installed_files() {
            claims
                .entry(root_rel(&prefix, &f).to_lowercase())
                .or_default()
                .push(other.game_id.clone());
        }
    }
    Ok(claims)
}

/// Every file the mods installed under `base_path` wrote there, as lower-cased
/// POSIX paths relative to `base_path`, so the base game's stale-file sweep can
/// leave them alone. A ledger records paths relative to the mod's own overlay
/// folder (`base_path/<modInstallDir>`, see mod_agent.rs), so they are prefixed
/// with that folder here. Lower-cased because a mod written into an existing
/// directory on Windows takes the directory's on-disk casing, not the ledger's.
///
/// An unfinished mod counts: its file list is recorded before its first chunk
/// is written. Returns `Err` with a reason when a ledger exists that cannot
/// answer the question: it fails to decode, it is an older client's unfinished
/// download (which never recorded a file list), or its overlay folder is not
/// inside `base_path`. The caller must then assume any unknown file might
/// belong to a mod.
pub fn mod_owned_files(base_path: &Path) -> Result<HashSet<String>, String> {
    Ok(mod_owned_files_spelled(base_path)?
        .into_iter()
        .map(|f| f.to_lowercase())
        .collect())
}

/// [`mod_owned_files`], spelt as the ledgers spell them rather than
/// lower-cased. The in-place updater matches these case-sensitively on
/// filesystems that are (Linux, the Deck), where `Config/x.cfg` claimed by a
/// mod and the player's `config/x.cfg` are two different files.
pub fn mod_owned_files_spelled(base_path: &Path) -> Result<HashSet<String>, String> {
    let ledgers = read_ledgers(base_path).map_err(|e| format!("could not read {MODS_DIR}: {e}"))?;

    let mut owned = HashSet::new();
    for (name, ledger) in ledgers {
        let ledger = ledger.map_err(|e| format!("mod ledger {name} is unreadable: {e}"))?;
        let files = ledger.get_installed_files();
        if files.is_empty() {
            return Err(format!(
                "mod {} has no file list (an older Drop started it and it did not finish)",
                ledger.game_id
            ));
        }
        let prefix = overlay_rel(base_path, &ledger)?;
        owned.extend(files.into_iter().map(|f| root_rel(&prefix, &f)));
    }
    Ok(owned)
}

/// Move aside every base-game file this mod is about to overwrite, into
/// `.mods/<mod id>/originals/`, so uninstall can put it back. Called once at
/// the start of every download run, BEFORE `incoming` is added to the ledger
/// and before any chunk is written.
///
/// A file is treated as the base game's (and backed up) when it exists, this
/// mod's ledger does not already list it (from an earlier attempt or the
/// version being updated from), no other mod lists it, and no other mod
/// still holds an original of it (then that is the game's file). A file that
/// already has a backup keeps the older one, which is the earlier original,
/// unless that backup is marked stale (the game has rewritten the file since):
/// then the stale copy is replaced by the current file.
///
/// Refuses to do anything when another mod's ledger cannot be read (see
/// `claims_by_others`): the file might be that mod's, and backing it up as
/// the game's would later "restore" mod content over the game.
///
/// Moving rather than copying also means the mod's version is written into a
/// fresh file instead of over a longer one.
///
/// Returns the files moved aside (paths relative to the install dir), so a
/// caller that fails afterwards can put them back with `undo_backups`. On
/// error, everything this call moved has already been put back.
pub fn back_up_originals(
    install_dir: &Path,
    ledger: &ModData,
    incoming: &[String],
) -> Result<Vec<String>, String> {
    if is_legacy_unfinished(ledger) {
        // Chunks from an older client's attempt are already on disk and
        // nothing says which files they were, so a "base game" file here may
        // really be the mod's own. Backing those up would later "restore" mod
        // content over the game; skip instead, as the older client did.
        warn!(
            "mod {}: resuming a download started by an older Drop; base-game files it overwrites are not backed up",
            ledger.game_id
        );
        return Ok(Vec::new());
    }
    let prefix = overlay_rel(install_dir, ledger)?;
    let overlay = install_dir.join(&prefix);
    let own: HashSet<String> = ledger
        .get_installed_files()
        .iter()
        .map(|f| root_rel(&prefix, f).to_lowercase())
        .collect();
    let claims = claims_by_others(install_dir, &ledger.game_id)?;
    let stale = read_stale(install_dir, &ledger.game_id)?;
    let contained = Contained::new(install_dir)?;
    let originals = originals_dir(install_dir, &ledger.game_id);
    // Other mods' current originals. One that another mod still holds for a
    // file it no longer lists (it could not be put back yet) is the game's
    // real file, so what is on disk now is not: backing that up as well would
    // later make ours look like a duplicate of the real one, and the real one
    // would be deleted in its favour (`hand_original_to`).
    let mut held_elsewhere: Vec<(PathBuf, HashSet<String>)> = Vec::new();
    for id in mod_ids_under(install_dir).map_err(|e| format!("could not list {MODS_DIR}: {e}"))? {
        if id == ledger.game_id {
            continue;
        }
        let dir = originals_dir(install_dir, &id);
        if std::fs::symlink_metadata(&dir).is_ok() {
            held_elsewhere.push((dir, read_stale(install_dir, &id)?));
        }
    }

    // Pass 1: which files are the base game's, and clear stale backups out of
    // their way. Nothing live is touched yet.
    let mut to_move: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    let mut reclaimed: Vec<String> = Vec::new();
    for file in incoming {
        let rel = root_rel(&prefix, file);
        let key = rel.to_lowercase();
        if own.contains(&key) || claims.contains_key(&key) {
            continue;
        }
        let held = held_elsewhere.iter().any(|(dir, their_stale)| {
            !their_stale.contains(&key)
                && path_guard::join_within(dir, Path::new(&rel))
                    .is_ok_and(|b| std::fs::symlink_metadata(b).is_ok())
        });
        if held {
            continue;
        }
        let Ok(live) = path_guard::join_within(&overlay, Path::new(file)) else {
            // The download itself refuses this name (download_logic.rs).
            continue;
        };
        let is_file = std::fs::symlink_metadata(&live)
            .map(|m| m.is_file())
            .unwrap_or(false);
        if !is_file {
            continue;
        }
        let Ok(backup) = path_guard::join_within(&originals, Path::new(&rel)) else {
            continue;
        };
        contained.check(&live)?;
        contained.check(&backup)?;
        if std::fs::symlink_metadata(&backup).is_ok() {
            if !stale.contains(&key) {
                continue;
            }
            std::fs::remove_file(&backup)
                .map_err(|e| format!("cannot replace the outdated copy of {rel}: {e}"))?;
            reclaimed.push(rel.clone());
        }
        to_move.push((rel, live, backup));
    }
    // A backup about to be taken must not stay listed as stale, or uninstall
    // would skip it. Recorded before the move, so a failure here leaves only
    // a stale entry with no backup, which is harmless.
    if !reclaimed.is_empty() {
        unmark_stale(install_dir, &ledger.game_id, &reclaimed)?;
    }

    // Pass 2: move them aside.
    let mut moved: Vec<String> = Vec::new();
    for (rel, live, backup) in to_move {
        if let Err(why) = move_aside(&live, &backup) {
            let undo = undo_backups(install_dir, &ledger.game_id, &moved);
            if !undo.is_empty() {
                error!(
                    "mod {}: could not put back {} file(s) after a failed backup: {undo:?}",
                    ledger.game_id,
                    undo.len()
                );
            }
            return Err(format!("cannot back up {rel}: {why}"));
        }
        moved.push(rel);
    }
    if !moved.is_empty() {
        info!(
            "mod {}: moved {} base-game file(s) to {}",
            ledger.game_id,
            moved.len(),
            originals.display()
        );
    }
    Ok(moved)
}

/// Move a file to `backup`, creating its folder. Rename on the same volume
/// (`.mods` sits inside the install dir), copy + delete otherwise.
fn move_aside(live: &Path, backup: &Path) -> Result<(), String> {
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if let Err(rename_err) = std::fs::rename(live, backup) {
        std::fs::copy(live, backup)
            .map_err(|e| format!("{rename_err}; copy also failed: {e}"))?;
        std::fs::remove_file(live).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Put back files `back_up_originals` moved aside (paths relative to the
/// install dir), for a download that failed before writing anything. Returns
/// the ones that could not be put back, with why.
pub fn undo_backups(install_dir: &Path, mod_game_id: &str, moved: &[String]) -> Vec<(String, String)> {
    let originals = originals_dir(install_dir, mod_game_id);
    let contained = match Contained::new(install_dir) {
        Ok(c) => c,
        Err(why) => return moved.iter().map(|rel| (rel.clone(), why.clone())).collect(),
    };
    let mut failed = Vec::new();
    for rel in moved {
        let (Ok(backup), Ok(live)) = (
            path_guard::join_within(&originals, Path::new(rel)),
            path_guard::join_within(install_dir, Path::new(rel)),
        ) else {
            continue;
        };
        let result = contained
            .check(&live)
            .and_then(|()| move_into_place(&backup, &live).map_err(|e| e.to_string()));
        if let Err(e) = result {
            failed.push((rel.clone(), e));
        }
    }
    failed
}

/// What happened when files were handed back to the base game.
#[derive(Debug, Default)]
pub struct ReleaseOutcome {
    /// Deleted (the mod added them; the base game never had them).
    pub removed: usize,
    /// The base game's original put back.
    pub restored: usize,
    /// Left in place because another mod still lists them.
    pub kept_for_other_mods: usize,
    /// Files that could not be handled, with the reason. They stay in the
    /// ledger so a retry can finish the job.
    pub failed: Vec<(String, String)>,
    /// Originals in `originals/` that could not be put back (or passed to
    /// another mod), install-relative, with the reason. They stay there, so
    /// a retry can finish the job; until then the game is missing them.
    pub failed_originals: Vec<(String, String)>,
}

impl ReleaseOutcome {
    /// The first thing that could not be handled, of either kind.
    pub fn first_failure(&self) -> Option<&(String, String)> {
        self.failed.first().or_else(|| self.failed_originals.first())
    }

    fn failure_count(&self) -> usize {
        self.failed.len() + self.failed_originals.len()
    }
}

/// Pass this mod's backup of `rel` (install-relative) to `heir`, another mod
/// that still lists the file, so the last mod out restores it. If the heir
/// already holds a current original, ours is a duplicate and is deleted; if
/// the heir's copy is stale, ours replaces it.
fn hand_original_to(
    install_dir: &Path,
    contained: &Contained,
    backup: &Path,
    heir: &str,
    rel: &str,
) -> Result<(), String> {
    let target = path_guard::join_within(&originals_dir(install_dir, heir), Path::new(rel))
        .map_err(|e| e.to_string())?;
    contained.check(backup)?;
    contained.check(&target)?;
    if std::fs::symlink_metadata(&target).is_err() {
        return move_into_place(backup, &target).map_err(|e| e.to_string());
    }
    if !read_stale(install_dir, heir)?.contains(&rel.to_lowercase()) {
        return std::fs::remove_file(backup).map_err(|e| e.to_string());
    }
    // Un-mark first: if the move then fails, the heir's outdated copy is no
    // longer marked, so mark it again before giving up.
    unmark_stale(install_dir, heir, &[rel.to_string()])?;
    if let Err(e) = move_into_place(backup, &target) {
        if let Err(again) = mark_stale(install_dir, heir, &[rel.to_string()]) {
            error!("mod {heir}: its outdated copy of {rel} is no longer marked as outdated: {again}");
        }
        return Err(e.to_string());
    }
    Ok(())
}

/// Give `files` (paths from `ledger`, relative to its overlay folder) back to
/// the base game: restore the original where one was backed up, delete the
/// file otherwise, and leave it alone when another mod also lists it (handing
/// any backed-up original to that mod, so the last one out restores it).
/// A backup marked stale is never restored.
///
/// Never touches anything outside `install_dir`, following symlinks. When
/// another mod's claims cannot be known, every file fails and nothing is
/// touched. Does not rewrite the ledger: callers go through
/// `release_recorded`, which keeps the ledger safe to retry from.
pub fn release_files(install_dir: &Path, ledger: &ModData, files: &[String]) -> ReleaseOutcome {
    let mut outcome = ReleaseOutcome::default();
    let setup = (|| {
        Ok::<_, String>((
            overlay_rel(install_dir, ledger)?,
            claims_by_others(install_dir, &ledger.game_id)?,
            read_stale(install_dir, &ledger.game_id)?,
            Contained::new(install_dir)?,
        ))
    })();
    let (prefix, claims, stale, contained) = match setup {
        Ok(v) => v,
        Err(why) => {
            outcome.failed = files.iter().map(|f| (f.clone(), why.clone())).collect();
            return outcome;
        }
    };
    let overlay = install_dir.join(&prefix);
    let originals = originals_dir(install_dir, &ledger.game_id);

    for file in files {
        let rel = root_rel(&prefix, file);
        let key = rel.to_lowercase();
        let (live, backup) = match (
            path_guard::join_within(&overlay, Path::new(file)),
            path_guard::join_within(&originals, Path::new(&rel)),
        ) {
            (Ok(l), Ok(b)) => (l, b),
            _ => {
                // Never written (the download refuses unsafe names), so there
                // is nothing to remove. Not a failure: it could never succeed.
                warn!("skipping unsafe mod file path {file:?}");
                continue;
            }
        };
        let has_backup = std::fs::symlink_metadata(&backup).is_ok() && !stale.contains(&key);

        if let Some(others) = claims.get(&key) {
            if has_backup
                && let Err(e) = hand_original_to(install_dir, &contained, &backup, &others[0], &rel)
            {
                outcome
                    .failed
                    .push((file.clone(), format!("could not hand its original to mod {}: {e}", others[0])));
                continue;
            }
            outcome.kept_for_other_mods += 1;
            continue;
        }

        if let Err(why) = contained.check(&live) {
            outcome.failed.push((file.clone(), why));
            continue;
        }

        if has_backup {
            let result = contained
                .check(&backup)
                .and_then(|()| move_into_place(&backup, &live).map_err(|e| e.to_string()));
            match result {
                Ok(()) => outcome.restored += 1,
                Err(e) => outcome
                    .failed
                    .push((file.clone(), format!("could not restore the original: {e}"))),
            }
            continue;
        }

        match std::fs::symlink_metadata(&live) {
            Ok(m) if m.is_dir() => {
                warn!("mod file {} is a directory now; leaving it", live.display());
            }
            Ok(_) => match std::fs::remove_file(&live) {
                Ok(()) => outcome.removed += 1,
                Err(e) => outcome.failed.push((file.clone(), format!("could not delete: {e}"))),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => outcome.failed.push((file.clone(), format!("could not check: {e}"))),
        }
    }
    outcome
}

/// Release `release` (some of the files `ledger` lists) and record what is
/// left, in an order that is safe to retry after any failure or crash.
///
/// The danger is a file whose original gets put back while the ledger on disk
/// still lists it: a retry would see a listed file with no backup and delete
/// the game's own file. So the ledger stops listing those files BEFORE they
/// are restored; from then on they are "unlisted originals", which
/// `settle_unlisted_originals` puts back (now, and again on any retry). Files
/// that are only deleted or left to other mods are safe to retry while still
/// listed, so they are dropped from the ledger afterwards.
///
/// An `Err` returned before any file is touched (the checks, or recording the
/// files about to be put back) means nothing was changed. An `Err` from the
/// final ledger write comes after files were deleted and put back; the
/// ledger on disk then still lists some deleted files, which a retry finds
/// gone and skips, and never lists a put-back original. Files that could not
/// be handled are in `outcome.failed` and stay listed; originals that could
/// not be put back are in `outcome.failed_originals` and stay in
/// `originals/`. `removing_ledger`: the caller deletes the ledger when
/// nothing failed, so the final write is skipped then.
pub(crate) fn release_recorded(
    install_dir: &Path,
    ledger: &ModData,
    release: &[String],
    removing_ledger: bool,
) -> Result<ReleaseOutcome, String> {
    // Everything that can stop this is checked before anything changes.
    let prefix = overlay_rel(install_dir, ledger)?;
    let claims = claims_by_others(install_dir, &ledger.game_id)?;
    let stale = read_stale(install_dir, &ledger.game_id)?;
    Contained::new(install_dir)?;
    let originals = originals_dir(install_dir, &ledger.game_id);

    let all = ledger.get_installed_files();
    let (restore, rest): (Vec<String>, Vec<String>) = release.iter().cloned().partition(|f| {
        let rel = root_rel(&prefix, f);
        let key = rel.to_lowercase();
        !claims.contains_key(&key)
            && !stale.contains(&key)
            && path_guard::join_within(&originals, Path::new(&rel))
                .is_ok_and(|b| std::fs::symlink_metadata(b).is_ok())
    });
    if !restore.is_empty() {
        let restore_set: HashSet<&String> = restore.iter().collect();
        ledger.set_installed_files(all.iter().filter(|f| !restore_set.contains(f)).cloned().collect());
        if let Err(e) = ledger.try_write() {
            ledger.set_installed_files(all);
            return Err(format!(
                "could not record the files about to be put back, so nothing was changed: {e}"
            ));
        }
    }

    let mut outcome = release_files(install_dir, ledger, &rest);

    let release_set: HashSet<&String> = release.iter().collect();
    let failed: HashSet<&String> = outcome.failed.iter().map(|(f, _)| f).collect();
    let remaining: Vec<String> = ledger
        .get_installed_files()
        .into_iter()
        .filter(|f| !release_set.contains(f) || failed.contains(f))
        .collect();
    let listed: HashSet<String> = remaining
        .iter()
        .map(|f| root_rel(&prefix, f).to_lowercase())
        .collect();
    ledger.set_installed_files(remaining);

    let settled = settle_unlisted_originals(install_dir, &ledger.game_id, &listed);
    outcome.restored += settled.restored;
    outcome.kept_for_other_mods += settled.kept_for_other_mods;
    outcome.failed_originals.extend(settled.failed_originals);

    if removing_ledger && outcome.first_failure().is_none() {
        return Ok(outcome);
    }
    ledger
        .try_write()
        .map_err(|e| format!("could not record which of the mod's files are left: {e}"))?;
    Ok(outcome)
}

/// Make `ledger` (as `ModData::generate` built it when the download was
/// queued) match the ledger on disk, right before the download writes
/// anything. Since it was queued a base-game update may have taken files back
/// from the mod (`hand_files_to_base_game`), or the mod may have been
/// removed. Writing the queued copy back would make the mod treat the game's
/// file as its own (no backup) and later delete it.
///
/// Anything that doesn't add up (a different version on disk than the
/// manifest was fetched for, an unreadable ledger) is an error and nothing is
/// changed. A ledger for a different install folder is this mod's old
/// placement: that install is removed completely first (`remove_mod`), and
/// this one starts from nothing (`generate` asked for a full manifest then).
pub fn reload_for_run(install_dir: &Path, ledger: &ModData) -> Result<(), String> {
    let path = moddata_path(install_dir, &ledger.game_id);
    let ours = overlay_rel(install_dir, ledger)?;
    let queued_previous = ledger.previously_installed_version.as_deref();
    let changed = || "it changed since the download was queued; start it again".to_string();
    match ModData::read(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if queued_previous.is_some() {
                // Queued as an update; the installed version is gone, so the
                // delta manifest is missing files.
                return Err(changed());
            }
            ledger.set_installed_files(Vec::new());
            ledger.set_contexts(&[]);
        }
        Err(e) => {
            return Err(format!(
                "its file list {} cannot be read ({e}), so Drop cannot tell which files on disk are the mod's",
                path.display()
            ));
        }
        Ok(disk) => {
            if overlay_rel(install_dir, &disk).ok().as_deref() != Some(ours.as_str()) {
                if queued_previous.is_some() {
                    return Err(changed());
                }
                info!(
                    "mod {}: install folder changed; removing the old install first",
                    ledger.game_id
                );
                remove_mod(install_dir, &ledger.game_id)
                    .map_err(|why| format!("could not remove it from its old folder first: {why}"))?;
                ledger.set_installed_files(Vec::new());
                ledger.set_contexts(&[]);
            } else if disk.game_version == ledger.game_version {
                if disk.previously_installed_version.as_deref() != queued_previous {
                    return Err(changed());
                }
                ledger.set_installed_files(disk.get_installed_files());
                let contexts: Vec<(String, bool)> = disk.get_contexts().into_iter().collect();
                ledger.set_contexts(&contexts);
            } else {
                if queued_previous != Some(disk.game_version.as_str()) {
                    return Err(changed());
                }
                ledger.set_installed_files(disk.get_installed_files());
                ledger.set_contexts(&[]);
            }
        }
    }
    Ok(())
}

/// Rename `from` to `to`, creating `to`'s folder first. Replaces a file
/// already at `to` (std::fs::rename does on both Windows and Linux).
fn move_into_place(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(from, to)
}

/// Put back every original in this mod's `originals/` that its ledger does
/// not list (`listed`: lower-cased install-relative paths it still lists).
/// Those are: files a removal or update has just stopped listing (see
/// `release_recorded`), and files moved aside by a download that crashed
/// before it could record them. Each is the base game's file, so it wins over
/// whatever is there now, unless it is marked stale (skipped; it is deleted
/// with the mod's state folder) or another mod lists the file now (the
/// original passes to that mod instead).
pub(crate) fn settle_unlisted_originals(
    install_dir: &Path,
    mod_game_id: &str,
    listed: &HashSet<String>,
) -> ReleaseOutcome {
    let mut outcome = ReleaseOutcome::default();
    let originals = originals_dir(install_dir, mod_game_id);
    match std::fs::symlink_metadata(&originals) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return outcome,
        _ => {}
    }
    let setup = (|| {
        Ok::<_, String>((
            claims_by_others(install_dir, mod_game_id)?,
            read_stale(install_dir, mod_game_id)?,
            Contained::new(install_dir)?,
        ))
    })();
    let (claims, stale, contained) = match setup {
        Ok(v) => v,
        Err(why) => {
            outcome.failed_originals.push((format!("{MODS_DIR}/{mod_game_id}"), why));
            return outcome;
        }
    };

    let mut stack = vec![originals.clone()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                outcome.failed_originals.push((dir.display().to_string(), e.to_string()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    outcome.failed_originals.push((dir.display().to_string(), e.to_string()));
                    continue;
                }
            };
            let path = entry.path();
            // Not followed: a symlinked folder in originals/ is moved as a
            // link, never walked into.
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                stack.push(path);
                continue;
            }
            let Ok(rel_path) = path.strip_prefix(&originals) else { continue };
            let rel = rel_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let key = rel.to_lowercase();
            if listed.contains(&key) || stale.contains(&key) {
                continue;
            }
            if let Some(others) = claims.get(&key) {
                match hand_original_to(install_dir, &contained, &path, &others[0], &rel) {
                    Ok(()) => outcome.kept_for_other_mods += 1,
                    Err(e) => outcome.failed_originals.push((rel, e)),
                }
                continue;
            }
            let live = install_dir.join(rel_path);
            let result = contained
                .check(&live)
                .and_then(|()| move_into_place(&path, &live).map_err(|e| e.to_string()));
            match result {
                Ok(()) => outcome.restored += 1,
                Err(e) => outcome.failed_originals.push((rel, e)),
            }
        }
    }
    outcome
}

/// Fully remove one mod from `install_dir`: hand every file it lists back to
/// the base game (see `release_recorded`), then delete its ledger, its
/// `.pending` marker and its `.mods/<id>/` folder. Works for unfinished
/// installs too, and for a mod whose ledger is missing but whose originals
/// are not (a download that crashed before recording its files).
///
/// If anything could not be handled, the ledger and the backups for those
/// files are kept and `Err` describes the first one, so running it again
/// picks up where it stopped. An original that was put back is never listed
/// any more, so a retry can never delete it.
pub fn remove_mod(install_dir: &Path, mod_game_id: &str) -> Result<ReleaseOutcome, String> {
    let meta_path = moddata_path(install_dir, mod_game_id);
    let outcome = match ModData::read(&meta_path) {
        Ok(ledger) => {
            let files = ledger.get_installed_files();
            release_recorded(install_dir, &ledger, &files, true)?
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            settle_unlisted_originals(install_dir, mod_game_id, &HashSet::new())
        }
        Err(e) => return Err(format!("the mod's file list is unreadable: {e}")),
    };
    if let Some((file, why)) = outcome.first_failure() {
        return Err(format!(
            "{} file(s) could not be removed, for example {file}: {why}",
            outcome.failure_count()
        ));
    }

    for (path, what) in [
        (meta_path, "ledger"),
        (pending_marker_path(install_dir, mod_game_id), "pending marker"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("could not delete the mod's {what} {}: {e}", path.display())),
        }
    }
    match std::fs::remove_dir_all(mod_state_dir(install_dir, mod_game_id)) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        // What it still holds is outdated originals (marked stale) and empty
        // folders; everything else was put back above.
        Err(e) => warn!("mod {mod_game_id}: could not delete its state folder: {e}"),
    }
    Ok(outcome)
}

/// Delete every mod's backup of `rels` (install-relative): the base game has
/// rewritten or dropped those files, so putting an old copy back on uninstall
/// would roll the game back or bring back a file it no longer ships. Applies
/// whether or not the mod's ledger lists the file, and to mods whose ledger is
/// unreadable or gone. A backup that cannot be deleted is marked stale
/// instead, which `release_files` and `settle_unlisted_originals` honour.
/// `Err` only when even that could not be recorded.
pub fn discard_originals(install_dir: &Path, rels: &[String]) -> Result<(), String> {
    if rels.is_empty() {
        return Ok(());
    }
    let ids = mod_ids_under(install_dir).map_err(|e| format!("could not list {MODS_DIR}: {e}"))?;
    if ids.is_empty() {
        return Ok(());
    }
    let contained = Contained::new(install_dir)?;
    // Every mod is handled even when one fails; the first failure is returned.
    let mut first_error: Option<String> = None;
    for id in ids {
        let originals = originals_dir(install_dir, &id);
        if std::fs::symlink_metadata(&originals).is_err() {
            continue;
        }
        let mut undeleted: Vec<String> = Vec::new();
        for rel in rels {
            let Ok(backup) = path_guard::join_within(&originals, Path::new(rel)) else {
                continue;
            };
            match std::fs::symlink_metadata(&backup) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => {
                    warn!("could not check outdated original {}: {e}", backup.display());
                    undeleted.push(rel.clone());
                    continue;
                }
                Ok(_) => {}
            }
            let result = contained
                .check(&backup)
                .and_then(|()| std::fs::remove_file(&backup).map_err(|e| e.to_string()));
            if let Err(e) = result {
                warn!("could not delete outdated original {}: {e}", backup.display());
                undeleted.push(rel.clone());
            }
        }
        if !undeleted.is_empty()
            && let Err(e) = mark_stale(install_dir, &id, &undeleted)
        {
            error!(
                "mod {id}: {} outdated original(s) could not be deleted or marked: {e}",
                undeleted.len()
            );
            first_error.get_or_insert(format!(
                "could not mark mod {id}'s outdated copies of the game's files: {e}"
            ));
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// The base game just wrote `written` (paths relative to the install dir, as
/// the game's manifest spells them) as part of a download, update or repair.
/// Called for each chunk BEFORE the chunk is recorded as done, so a crash in
/// between re-runs it with the chunk. Where a mod had overwritten one of those
/// files, the base game has now taken it back: the mod's ledger stops listing
/// it (written first, and a failure is an error that keeps the backups), then
/// every mod's backup of it is discarded (see `discard_originals`), because
/// restoring that old copy on uninstall would roll the game back. Returns how
/// many listed files were let go.
pub fn hand_files_to_base_game(install_dir: &Path, written: &[String]) -> Result<usize, String> {
    if written.is_empty() {
        return Ok(0);
    }
    let ledgers = read_ledgers(install_dir).map_err(|e| format!("could not check mods: {e}"))?;
    let written_lc: HashSet<String> = written.iter().map(|w| w.to_lowercase()).collect();
    let mut released = 0;
    for (name, ledger) in ledgers {
        let ledger = match ledger {
            Ok(l) => l,
            // Corrupt: it can never be edited (nor uninstalled), so it is
            // skipped. Its backups of these files are discarded below.
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                warn!("mod ledger {name} under {} is corrupt: {e}", install_dir.display());
                continue;
            }
            // Anything else (a locked or unreadable file) may pass: fail, so
            // the chunk is not recorded and the hand-over runs again.
            Err(e) => return Err(format!("could not read mod ledger {name}: {e}")),
        };
        let prefix = match overlay_rel(install_dir, &ledger) {
            Ok(p) => p,
            Err(why) => {
                warn!("not updating mod ledger {name}: {why}");
                continue;
            }
        };
        let (taken, kept): (Vec<String>, Vec<String>) = ledger
            .get_installed_files()
            .into_iter()
            .partition(|f| written_lc.contains(&root_rel(&prefix, f).to_lowercase()));
        if taken.is_empty() {
            continue;
        }
        ledger.set_installed_files(kept);
        ledger.try_write().map_err(|e| {
            format!(
                "could not record that the game took back {} file(s) from mod {}: {e}",
                taken.len(),
                ledger.game_id
            )
        })?;
        info!(
            "base game rewrote {} file(s) mod {} had overwritten; the mod no longer owns them",
            taken.len(),
            ledger.game_id
        );
        released += taken.len();
    }
    discard_originals(install_dir, written)?;
    Ok(released)
}

/// Why a mod's launch override is not used for a launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideSkip {
    /// The mod version has its own launch settings, none for this platform.
    Platform,
    /// The mod's download did not finish.
    Unfinished,
    /// Another mod's override is used instead (its id).
    OtherMod(String),
}

/// Which installed mod's launch override a launch uses, and which were passed
/// over and why.
#[derive(Debug, Default)]
pub struct OverrideDecision {
    /// (mod id, executable relative to the install dir).
    pub chosen: Option<(String, String)>,
    /// (mod id, its override, why it is not used).
    pub skipped: Vec<(String, String, OverrideSkip)>,
}

/// Decide the launch override for a launch of `platform` from the base game's
/// installed mods (`<install_dir>/.mods/*.moddata`).
///
/// `mod_platforms(version id)` gives the platforms the mod version has its own
/// launch or setup settings for. Only those mods are platform-specific. A mod
/// version with none (None or empty, the usual case: a pure file overlay like
/// SMAPI, which the server offers on every platform) applies to whatever
/// platform the base game launches as. The platform recorded in the ledger is
/// deliberately not used: older clients recorded whichever platform the
/// server listed first, which for an overlay says nothing.
///
/// Only finished installs count. When several mods qualify, the one whose
/// ledger sorts first (by mod id) wins every time.
pub fn decide_launch_override(
    install_dir: &Path,
    platform: Platform,
    mod_platforms: &dyn Fn(&str) -> Option<Vec<Platform>>,
) -> Result<OverrideDecision, io::Error> {
    let mut decision = OverrideDecision::default();
    for m in installed_ledgers(install_dir)? {
        let Some(ov) = m.launch_override.clone().filter(|o| !o.trim().is_empty()) else {
            continue;
        };
        let skip = if !mod_platforms(&m.game_version)
            .filter(|p| !p.is_empty())
            .is_none_or(|p| p.contains(&platform))
        {
            Some(OverrideSkip::Platform)
        } else if !ledger_is_complete(install_dir, &m) {
            Some(OverrideSkip::Unfinished)
        } else {
            decision
                .chosen
                .as_ref()
                .map(|(winner, _)| OverrideSkip::OtherMod(winner.clone()))
        };
        match skip {
            Some(why) => decision.skipped.push((m.game_id.clone(), ov, why)),
            None => decision.chosen = Some((m.game_id.clone(), ov)),
        }
    }
    Ok(decision)
}

/// The launch override for a launch of `platform` (see
/// `decide_launch_override`), logging every mod passed over. The launcher
/// calls this to swap the game's executable while such a mod is installed;
/// when the mod is uninstalled its ledger is gone, so this returns None and
/// the game launches normally again.
pub fn find_launch_override(
    install_dir: &Path,
    platform: Platform,
    mod_platforms: &dyn Fn(&str) -> Option<Vec<Platform>>,
) -> Option<String> {
    let decision = match decide_launch_override(install_dir, platform, mod_platforms) {
        Ok(d) => d,
        Err(e) => {
            warn!("[LAUNCH] could not read mods under {}: {e}", install_dir.display());
            return None;
        }
    };
    for (id, ov, why) in &decision.skipped {
        match why {
            OverrideSkip::Platform => warn!(
                "[LAUNCH] mod {id} launch override {ov} is not used: the mod has no launch settings for {platform:?}"
            ),
            OverrideSkip::Unfinished => warn!(
                "[LAUNCH] mod {id} launch override {ov} is not used: its install did not finish"
            ),
            OverrideSkip::OtherMod(winner) => warn!(
                "[LAUNCH] mod {id} also sets a launch override ({ov}); using mod {winner}'s"
            ),
        }
    }
    decision.chosen.map(|(_, ov)| ov)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ModData {
        let m = ModData::new(
            "gid".to_string(),
            "ver".to_string(),
            Platform::Windows,
            "parent".to_string(),
            Some("Stardew Valley/StardewModdingAPI.exe".to_string()),
            PathBuf::from("C:/base"),
            PathBuf::from("C:/base/.mods/gid.moddata"),
            None,
        );
        m.set_context("chunk1".to_string(), true);
        m.set_installed_files(vec!["a.dll".to_string(), "b/c.dll".to_string()]);
        m
    }

    /// The bug this guards against: a `.moddata` that `write()` produced could
    /// not be decoded by `read()`, so an installed mod always looked
    /// uninstalled. Root cause was embedding UserConfiguration, which `pot`
    /// can't round-trip (it has `#[serde(default)]` fields). Fixed by not
    /// storing config in ModData.
    #[test]
    fn moddata_roundtrips_through_pot() {
        let m = sample();
        let bytes = pot::to_vec(&m).expect("serialize");
        let decoded: ModData = pot::from_slice(&bytes).expect("deserialize");
        assert_eq!(decoded.game_id, "gid");
        assert_eq!(decoded.get_installed_files().len(), 2);
        assert_eq!(decoded.get_contexts().len(), 1);
        assert_eq!(
            decoded.launch_override.as_deref(),
            Some("Stardew Valley/StardewModdingAPI.exe")
        );
    }

    fn scratch_install(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "drop-mod-owned-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(MODS_DIR)).unwrap();
        dir
    }

    fn write_ledger(base: &Path, id: &str, files: &[&str]) -> ModData {
        write_ledger_at(base, base.to_path_buf(), id, files)
    }

    /// `overlay` is where the mod's files land (mod_agent.rs: the install dir
    /// joined with the version's `modInstallDir`); the ledger always lives in
    /// the install dir's `.mods/`.
    fn write_ledger_at(base: &Path, overlay: PathBuf, id: &str, files: &[&str]) -> ModData {
        let meta = moddata_path(base, id);
        let m = ModData::new(
            id.to_string(),
            "v1".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            overlay,
            meta,
            None,
        );
        m.set_installed_files(files.iter().map(|s| s.to_string()).collect());
        m.write();
        m
    }

    fn put(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn body(path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    #[test]
    fn owned_files_without_mods_dir_is_empty() {
        let dir = std::env::temp_dir().join(format!("drop-mod-owned-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(mod_owned_files(&dir).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_files_merges_ledgers_and_lowercases() {
        let dir = scratch_install("merge");
        write_ledger(&dir, "smapi", &["StardewModdingAPI.exe", "smapi-internal/SMAPI.dll"]);
        write_ledger(&dir, "ap", &["Mods/StardewArchipelago/manifest.json"]);
        // A leftover temp file from an interrupted write is not a ledger.
        std::fs::write(dir.join(MODS_DIR).join("x.moddata.tmp.3"), b"junk").unwrap();
        // Neither are the per-mod marker and state folder.
        std::fs::write(pending_marker_path(&dir, "ap"), b"").unwrap();
        std::fs::create_dir_all(originals_dir(&dir, "ap")).unwrap();

        let owned = mod_owned_files(&dir).unwrap();
        assert!(owned.contains("stardewmoddingapi.exe"));
        assert!(owned.contains("smapi-internal/smapi.dll"));
        assert!(owned.contains("mods/stardewarchipelago/manifest.json"));
        assert_eq!(owned.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_files_are_relative_to_the_install_dir_not_the_overlay() {
        let dir = scratch_install("overlay");
        // Imported with modInstallDir = "Mods": the ledger lists paths inside
        // Mods/, and the sweep sees them from the install root.
        write_ledger_at(
            &dir,
            dir.join("Mods"),
            "cp",
            &["ContentPatcher/ContentPatcher.dll", "ContentPatcher/manifest.json"],
        );
        // mod_agent joins an empty modInstallDir, giving a trailing separator.
        write_ledger_at(&dir, dir.join(""), "root", &["StardewModdingAPI.exe"]);

        let owned = mod_owned_files(&dir).unwrap();
        assert!(owned.contains("mods/contentpatcher/contentpatcher.dll"), "{owned:?}");
        assert!(owned.contains("mods/contentpatcher/manifest.json"));
        assert!(owned.contains("stardewmoddingapi.exe"));
        assert!(!owned.contains("contentpatcher/contentpatcher.dll"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_files_refuses_ledger_recorded_elsewhere() {
        let dir = scratch_install("moved");
        write_ledger_at(&dir, PathBuf::from("/somewhere/else/Mods"), "m", &["a.dll"]);
        assert!(mod_owned_files(&dir).unwrap_err().contains("not inside"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_files_follows_a_moved_library() {
        // Ledger written when the game lived at /old/game; the game (and its
        // .mods folder) now lives at `dir`.
        let dir = scratch_install("relocated");
        let m = ModData::new(
            "m".to_string(),
            "v1".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            PathBuf::from("/old/game/Mods"),
            PathBuf::from("/old/game/.mods/m.moddata"),
            None,
        );
        m.set_installed_files(vec!["X/a.dll".to_string()]);
        let bytes = pot::to_vec(&m).unwrap();
        std::fs::write(moddata_path(&dir, "m"), bytes).unwrap();
        let owned = mod_owned_files(&dir).unwrap();
        assert!(owned.contains("mods/x/a.dll"), "{owned:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_files_refuses_legacy_unfinished_ledger() {
        let dir = scratch_install("unfinished");
        write_ledger(&dir, "done", &["a.dll"]);
        // What an older client left after a cancelled download: chunks done,
        // no file list.
        let m = write_ledger_at(&dir, dir.clone(), "halfway", &[]);
        m.set_context("c1".to_string(), true);
        m.write();
        let err = mod_owned_files(&dir).unwrap_err();
        assert!(err.contains("halfway"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_files_lists_an_unfinished_install_that_recorded_its_files() {
        let dir = scratch_install("inflight");
        write_ledger(&dir, "inflight", &["Mods/A/a.dll"]);
        std::fs::write(pending_marker_path(&dir, "inflight"), b"").unwrap();
        let owned = mod_owned_files(&dir).unwrap();
        assert!(owned.contains("mods/a/a.dll"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn owned_files_refuses_corrupt_ledger() {
        let dir = scratch_install("corrupt");
        std::fs::write(moddata_path(&dir, "broken"), b"not pot").unwrap();
        assert!(mod_owned_files(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn completion_needs_no_pending_marker_and_a_file_list() {
        let dir = scratch_install("complete");
        let done = write_ledger(&dir, "done", &["a.dll"]);
        assert!(ledger_is_complete(&dir, &done));
        let running = write_ledger(&dir, "running", &["b.dll"]);
        std::fs::write(pending_marker_path(&dir, "running"), b"").unwrap();
        assert!(!ledger_is_complete(&dir, &running));
        // Older client, cancelled: no marker, no file list.
        let legacy = write_ledger(&dir, "legacy", &[]);
        assert!(!ledger_is_complete(&dir, &legacy));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_moves_only_base_game_files_and_uninstall_restores_them() {
        let dir = scratch_install("backup");
        put(&dir.join("Game.dll"), "base");
        put(&dir.join("Data/x.txt"), "base x");
        // Owned by another mod: not the base game's, so not backed up here.
        put(&dir.join("Shared.dll"), "other mod");
        write_ledger(&dir, "other", &["Shared.dll"]);

        let m = write_ledger(&dir, "mine", &[]);
        let incoming: Vec<String> = ["Game.dll", "Data/x.txt", "Shared.dll", "New.dll"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(back_up_originals(&dir, &m, &incoming).unwrap().len(), 2);
        assert_eq!(body(&originals_dir(&dir, "mine").join("Game.dll")).as_deref(), Some("base"));
        assert_eq!(body(&originals_dir(&dir, "mine").join("Data/x.txt")).as_deref(), Some("base x"));
        assert!(body(&dir.join("Game.dll")).is_none(), "moved aside, not copied");

        // The mod writes its files.
        m.set_installed_files(incoming.clone());
        m.write();
        for f in &incoming {
            put(&dir.join(f), "mod");
        }
        // A second run (resume) must not back up the mod's own files.
        assert_eq!(back_up_originals(&dir, &m, &incoming).unwrap().len(), 0);
        assert_eq!(body(&originals_dir(&dir, "mine").join("Game.dll")).as_deref(), Some("base"));

        let outcome = remove_mod(&dir, "mine").unwrap();
        assert_eq!(outcome.restored, 2);
        assert_eq!(outcome.removed, 1, "New.dll");
        assert_eq!(outcome.kept_for_other_mods, 1, "Shared.dll");
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base"));
        assert_eq!(body(&dir.join("Data/x.txt")).as_deref(), Some("base x"));
        assert!(body(&dir.join("New.dll")).is_none());
        assert_eq!(body(&dir.join("Shared.dll")).as_deref(), Some("mod"));
        assert!(!moddata_path(&dir, "mine").exists());
        assert!(!mod_state_dir(&dir, "mine").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_backup_puts_back_what_it_already_moved() {
        let dir = scratch_install("backup-fail");
        put(&dir.join("A.dll"), "base a");
        put(&dir.join("Data/x.txt"), "base x");
        // Something in the way of the second backup's folder.
        put(&originals_dir(&dir, "m").join("Data"), "in the way");
        let m = write_ledger(&dir, "m", &[]);
        let incoming = vec!["A.dll".to_string(), "Data/x.txt".to_string()];
        let err = back_up_originals(&dir, &m, &incoming).unwrap_err();
        assert!(err.contains("Data/x.txt"), "{err}");
        assert_eq!(body(&dir.join("A.dll")).as_deref(), Some("base a"));
        assert_eq!(body(&dir.join("Data/x.txt")).as_deref(), Some("base x"));
        assert!(!originals_dir(&dir, "m").join("A.dll").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn original_passes_to_the_mod_still_using_the_file() {
        let dir = scratch_install("handover");
        put(&dir.join("Game.dll"), "base");
        let first = write_ledger(&dir, "a-first", &[]);
        let incoming = vec!["Game.dll".to_string()];
        back_up_originals(&dir, &first, &incoming).unwrap();
        first.set_installed_files(incoming.clone());
        first.write();
        put(&dir.join("Game.dll"), "mod a");
        // Second mod overwrites the same file: it is mod a's, so no backup.
        let second = write_ledger(&dir, "b-second", &[]);
        assert_eq!(back_up_originals(&dir, &second, &incoming).unwrap().len(), 0);
        second.set_installed_files(incoming.clone());
        second.write();
        put(&dir.join("Game.dll"), "mod b");

        // Removing the first leaves the file to the second, with the original.
        let out = remove_mod(&dir, "a-first").unwrap();
        assert_eq!(out.kept_for_other_mods, 1);
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("mod b"));
        assert_eq!(body(&originals_dir(&dir, "b-second").join("Game.dll")).as_deref(), Some("base"));
        // Removing the last one restores the base game.
        let out = remove_mod(&dir, "b-second").unwrap();
        assert_eq!(out.restored, 1);
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overlay_folder_is_respected_for_backups_and_claims() {
        let dir = scratch_install("overlay-backup");
        put(&dir.join("Mods/Shared/config.json"), "base");
        let m = write_ledger_at(&dir, dir.join("Mods"), "m", &[]);
        let incoming = vec!["Shared/config.json".to_string()];
        assert_eq!(back_up_originals(&dir, &m, &incoming).unwrap().len(), 1);
        assert!(originals_dir(&dir, "m").join("Mods/Shared/config.json").exists());
        m.set_installed_files(incoming);
        m.write();
        put(&dir.join("Mods/Shared/config.json"), "mod");
        // A root-overlay mod listing the same file sees it as claimed.
        let other = write_ledger(&dir, "z", &[]);
        assert_eq!(
            back_up_originals(&dir, &other, &["Mods/Shared/config.json".to_string()])
                .unwrap()
                .len(),
            0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_removal_keeps_only_the_unfinished_files_listed() {
        let dir = scratch_install("partial-fail");
        put(&dir.join("Game.dll"), "base");
        put(&dir.join("Data/x.txt"), "base x");
        let m = write_ledger(&dir, "m", &[]);
        let incoming = vec!["Game.dll".to_string(), "Data/x.txt".to_string()];
        back_up_originals(&dir, &m, &incoming).unwrap();
        m.set_installed_files(incoming);
        m.write();
        put(&dir.join("Game.dll"), "mod");
        put(&dir.join("Data/x.txt"), "mod");
        // Something now sits where the Data folder was, so that original
        // cannot be put back.
        std::fs::remove_dir_all(dir.join("Data")).unwrap();
        std::fs::write(dir.join("Data"), b"in the way").unwrap();

        let err = remove_mod(&dir, "m").unwrap_err();
        assert!(err.contains("Data/x.txt"), "{err}");
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base"));
        // Both originals stopped being listed before anything was put back,
        // so no retry can mistake the restored Game.dll for the mod's file.
        // The one that could not go back waits in originals/ for the retry.
        let left = ModData::read(&moddata_path(&dir, "m")).unwrap();
        assert!(left.get_installed_files().is_empty());
        assert!(originals_dir(&dir, "m").join("Data/x.txt").exists());

        // Once the obstacle is gone a retry finishes, and never deletes the
        // Game.dll the first run already restored.
        std::fs::remove_file(dir.join("Data")).unwrap();
        let out = remove_mod(&dir, "m").unwrap();
        assert_eq!(out.restored, 1);
        assert_eq!(body(&dir.join("Data/x.txt")).as_deref(), Some("base x"));
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base"));
        assert!(!moddata_path(&dir, "m").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn originals_without_a_ledger_are_restored() {
        let dir = scratch_install("orphan-originals");
        put(&originals_dir(&dir, "m").join("Data/x.txt"), "base");
        put(&dir.join("Data/x.txt"), "half-written mod file");
        let out = remove_mod(&dir, "m").unwrap();
        assert_eq!(out.restored, 1);
        assert_eq!(body(&dir.join("Data/x.txt")).as_deref(), Some("base"));
        assert!(!mod_state_dir(&dir, "m").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_sweep_removes_dropped_files_and_restores_their_originals() {
        let dir = scratch_install("update");
        put(&dir.join("Game.dll"), "base");
        let v1 = write_ledger(&dir, "m", &[]);
        let files_v1 = vec!["Game.dll".to_string(), "old.dll".to_string(), "keep.dll".to_string()];
        back_up_originals(&dir, &v1, &files_v1).unwrap();
        v1.set_installed_files(files_v1.clone());
        v1.write();
        for f in &files_v1 {
            put(&dir.join(f), "mod v1");
        }
        // v2 ships keep.dll and new.dll only.
        let v2 = ModData::generate(
            "m".to_string(),
            "v2".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            dir.clone(),
            moddata_path(&dir, "m"),
        );
        assert_eq!(v2.previously_installed_version.as_deref(), Some("v1"));
        assert_eq!(v2.get_installed_files().len(), 3, "old files carried over");
        let files_v2 = vec!["keep.dll".to_string(), "new.dll".to_string()];
        assert_eq!(back_up_originals(&dir, &v2, &files_v2).unwrap().len(), 0);
        let stale: Vec<String> = v2
            .get_installed_files()
            .into_iter()
            .filter(|f| !files_v2.contains(f))
            .collect();
        let out = release_files(&dir, &v2, &stale);
        assert!(out.failed.is_empty());
        assert_eq!(out.restored, 1);
        assert_eq!(out.removed, 1);
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base"));
        assert!(body(&dir.join("old.dll")).is_none());
        assert_eq!(body(&dir.join("keep.dll")).as_deref(), Some("mod v1"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base_game_rewrite_drops_the_mods_claim_and_stale_original() {
        let dir = scratch_install("base-rewrite");
        put(&dir.join("Game.dll"), "base v1");
        let m = write_ledger(&dir, "m", &[]);
        let incoming = vec!["Game.dll".to_string(), "Extra.dll".to_string()];
        back_up_originals(&dir, &m, &incoming).unwrap();
        m.set_installed_files(incoming);
        m.write();
        // A base-game update rewrites Game.dll.
        put(&dir.join("Game.dll"), "base v2");
        assert_eq!(hand_files_to_base_game(&dir, &["Game.dll".to_string()]).unwrap(), 1);
        assert!(!originals_dir(&dir, "m").join("Game.dll").exists());
        let reread = ModData::read(&moddata_path(&dir, "m")).unwrap();
        assert_eq!(reread.get_installed_files(), vec!["Extra.dll".to_string()]);
        remove_mod(&dir, "m").unwrap();
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base v2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launch_override_respects_mod_platforms_completion_and_order() {
        let dir = scratch_install("override");
        let mk = |id: &str, version: &str, ledger_platform: Platform, ov: Option<&str>| {
            let m = ModData::new(
                id.to_string(),
                version.to_string(),
                ledger_platform,
                "parent".to_string(),
                ov.map(str::to_string),
                dir.clone(),
                moddata_path(&dir, id),
                None,
            );
            m.set_installed_files(vec![format!("{id}.bin")]);
            m.write();
        };
        // An overlay (no launch settings of its own) recorded as Windows by an
        // older client, the way existing SMAPI installs are.
        mk("c", "overlay", Platform::Windows, Some("C.exe"));
        mk("b", "win-only", Platform::Windows, Some("B.exe"));
        mk("a", "linux-only", Platform::Linux, Some("A.sh"));
        mk("0", "overlay", Platform::Windows, None);
        let platforms = |v: &str| match v {
            "overlay" => Some(Vec::new()),
            "win-only" => Some(vec![Platform::Windows]),
            "linux-only" => Some(vec![Platform::Linux]),
            _ => None,
        };
        assert_eq!(find_launch_override(&dir, Platform::Windows, &platforms).as_deref(), Some("B.exe"));
        assert_eq!(find_launch_override(&dir, Platform::Linux, &platforms).as_deref(), Some("A.sh"));
        let d = decide_launch_override(&dir, Platform::Windows, &platforms).unwrap();
        assert!(d.skipped.contains(&("a".to_string(), "A.sh".to_string(), OverrideSkip::Platform)));
        assert!(d.skipped.contains(&(
            "c".to_string(),
            "C.exe".to_string(),
            OverrideSkip::OtherMod("b".to_string())
        )));
        // An unfinished install's override is not used; the overlay's is,
        // although its ledger says Windows and the game launches as Linux.
        std::fs::write(pending_marker_path(&dir, "b"), b"").unwrap();
        std::fs::write(pending_marker_path(&dir, "a"), b"").unwrap();
        assert_eq!(find_launch_override(&dir, Platform::Linux, &platforms).as_deref(), Some("C.exe"));
        // A version the client never recorded counts as an overlay.
        assert_eq!(find_launch_override(&dir, Platform::Linux, &|_: &str| None).as_deref(), Some("C.exe"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_ledger_from_a_moved_library_is_read_and_written_where_it_is_now() {
        let dir = scratch_install("moved-write");
        let m = ModData::new(
            "m".to_string(),
            "v1".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            PathBuf::from("/old/game/Mods"),
            PathBuf::from("/old/game/.mods/m.moddata"),
            None,
        );
        m.set_installed_files(vec!["X/a.dll".to_string()]);
        std::fs::write(moddata_path(&dir, "m"), pot::to_vec(&m).unwrap()).unwrap();
        let read = ModData::read(&moddata_path(&dir, "m")).unwrap();
        assert_eq!(read.meta_path, moddata_path(&dir, "m"));
        assert_eq!(read.base_path, dir.join("Mods"));
        read.set_installed_files(Vec::new());
        read.try_write().unwrap();
        assert!(ModData::read(&moddata_path(&dir, "m")).unwrap().get_installed_files().is_empty());
        assert!(!Path::new("/old/game/.mods/m.moddata").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The queued copy of a ledger is stale once a base-game update has taken
    /// a file back. Reloading must pick up the disk's list, so the file is
    /// then the game's (backed up), not the mod's.
    #[test]
    fn a_queued_download_uses_the_ledger_on_disk_not_the_queued_copy() {
        let dir = scratch_install("requeue");
        put(&dir.join("Game.dll"), "base v1");
        let installed = write_ledger(&dir, "m", &[]);
        back_up_originals(&dir, &installed, &["Game.dll".to_string()]).unwrap();
        installed.set_installed_files(vec!["Game.dll".to_string(), "Extra.dll".to_string()]);
        installed.write();
        put(&dir.join("Game.dll"), "mod v1");
        put(&dir.join("Extra.dll"), "mod v1");

        // An update to v2 is queued...
        let queued = ModData::generate(
            "m".to_string(),
            "v2".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            dir.clone(),
            moddata_path(&dir, "m"),
        );
        assert_eq!(queued.get_installed_files().len(), 2);
        // ...then the base game updates Game.dll and takes it back.
        put(&dir.join("Game.dll"), "base v2");
        assert_eq!(hand_files_to_base_game(&dir, &["Game.dll".to_string()]).unwrap(), 1);

        reload_for_run(&dir, &queued).unwrap();
        assert_eq!(queued.get_installed_files(), vec!["Extra.dll".to_string()]);
        // So the run backs up the game's v2 file before overwriting it, and
        // uninstall puts it back instead of deleting it.
        let incoming = vec!["Game.dll".to_string(), "Extra.dll".to_string()];
        assert_eq!(back_up_originals(&dir, &queued, &incoming).unwrap(), vec!["Game.dll".to_string()]);
        queued.set_installed_files(incoming);
        queued.write();
        put(&dir.join("Game.dll"), "mod v2");
        remove_mod(&dir, "m").unwrap();
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base v2"));
        assert!(body(&dir.join("Extra.dll")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_refuses_a_ledger_that_changed_version_or_vanished_since_queueing() {
        let dir = scratch_install("reload-refuse");
        write_ledger(&dir, "m", &["a.dll"]);
        let queued = ModData::generate(
            "m".to_string(),
            "v2".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            dir.clone(),
            moddata_path(&dir, "m"),
        );
        assert_eq!(queued.previously_installed_version.as_deref(), Some("v1"));
        std::fs::remove_file(moddata_path(&dir, "m")).unwrap();
        assert!(reload_for_run(&dir, &queued).is_err());
        std::fs::write(moddata_path(&dir, "m"), b"not pot").unwrap();
        assert!(reload_for_run(&dir, &queued).unwrap_err().contains("cannot be read"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_install_folder_removes_the_old_install_first() {
        let dir = scratch_install("moved-overlay");
        put(&dir.join("Game.dll"), "base");
        let v1 = write_ledger(&dir, "m", &[]);
        back_up_originals(&dir, &v1, &["Game.dll".to_string()]).unwrap();
        v1.set_installed_files(vec!["Game.dll".to_string(), "Loader/a.dll".to_string()]);
        v1.write();
        put(&dir.join("Game.dll"), "mod v1");
        put(&dir.join("Loader/a.dll"), "mod v1");

        // v2 installs into Mods/ instead of the root.
        let v2 = ModData::generate(
            "m".to_string(),
            "v2".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            dir.join("Mods"),
            moddata_path(&dir, "m"),
        );
        assert_eq!(v2.previously_installed_version, None, "full download, not a delta");
        assert!(v2.get_installed_files().is_empty());
        reload_for_run(&dir, &v2).unwrap();
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base"));
        assert!(body(&dir.join("Loader/a.dll")).is_none());
        assert!(v2.get_installed_files().is_empty());
        assert!(!moddata_path(&dir, "m").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_the_game_dropped_is_not_put_back_on_uninstall() {
        let dir = scratch_install("dropped");
        put(&dir.join("Old.dll"), "base v1");
        let m = write_ledger(&dir, "m", &[]);
        back_up_originals(&dir, &m, &["Old.dll".to_string()]).unwrap();
        m.set_installed_files(vec!["Old.dll".to_string()]);
        m.write();
        put(&dir.join("Old.dll"), "mod");
        // Base v2 no longer ships Old.dll (the sweep keeps the mod's copy).
        discard_originals(&dir, &["Old.dll".to_string()]).unwrap();
        let out = remove_mod(&dir, "m").unwrap();
        assert_eq!(out.restored, 0);
        assert_eq!(out.removed, 1);
        assert!(body(&dir.join("Old.dll")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_original_stranded_by_a_crash_is_discarded_when_the_game_rewrites_the_file() {
        let dir = scratch_install("stranded-rewrite");
        // Moved aside, then the download crashed before recording its files.
        put(&originals_dir(&dir, "m").join("Game.dll"), "base v1");
        // The base game's update writes the file again.
        put(&dir.join("Game.dll"), "base v2");
        hand_files_to_base_game(&dir, &["Game.dll".to_string()]).unwrap();
        remove_mod(&dir, "m").unwrap();
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base v2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn an_original_that_cannot_be_deleted_is_marked_and_never_put_back() {
        let dir = scratch_install("stale-mark");
        let outside = std::env::temp_dir().join(format!("drop-mod-owned-stale-outside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&outside).unwrap();
        put(&outside.join("Game.dll"), "base v1");
        // originals/ leads out of the game folder, so its file can't be
        // deleted from here.
        std::fs::create_dir_all(mod_state_dir(&dir, "m")).unwrap();
        std::os::unix::fs::symlink(&outside, originals_dir(&dir, "m")).unwrap();
        put(&dir.join("Game.dll"), "base v2");
        hand_files_to_base_game(&dir, &["Game.dll".to_string()]).unwrap();
        assert!(read_stale(&dir, "m").unwrap().contains("game.dll"));
        assert_eq!(body(&outside.join("Game.dll")).as_deref(), Some("base v1"));
        let out = settle_unlisted_originals(&dir, "m", &HashSet::new());
        assert_eq!(out.restored, 0);
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base v2"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn a_stale_backup_is_replaced_by_the_current_file() {
        let dir = scratch_install("stale-reclaim");
        put(&originals_dir(&dir, "m").join("Game.dll"), "base v1");
        mark_stale(&dir, "m", &["Game.dll".to_string()]).unwrap();
        put(&dir.join("Game.dll"), "base v2");
        let m = write_ledger(&dir, "m", &[]);
        assert_eq!(back_up_originals(&dir, &m, &["Game.dll".to_string()]).unwrap().len(), 1);
        assert_eq!(body(&originals_dir(&dir, "m").join("Game.dll")).as_deref(), Some("base v2"));
        assert!(read_stale(&dir, "m").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_ledger_of_another_mod_stops_every_shared_file_operation() {
        let dir = scratch_install("unreadable-other");
        put(&dir.join("Game.dll"), "base");
        let m = write_ledger(&dir, "m", &[]);
        back_up_originals(&dir, &m, &["Game.dll".to_string()]).unwrap();
        m.set_installed_files(vec!["Game.dll".to_string()]);
        m.write();
        put(&dir.join("Game.dll"), "mod");
        std::fs::write(moddata_path(&dir, "broken"), b"not pot").unwrap();

        let err = remove_mod(&dir, "m").unwrap_err();
        assert!(err.contains("broken.moddata"), "{err}");
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("mod"));
        assert!(originals_dir(&dir, "m").join("Game.dll").exists());
        assert!(moddata_path(&dir, "m").exists());

        put(&dir.join("Other.dll"), "base");
        let n = write_ledger(&dir, "n", &[]);
        assert!(back_up_originals(&dir, &n, &["Other.dll".to_string()]).is_err());
        assert_eq!(body(&dir.join("Other.dll")).as_deref(), Some("base"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn removal_never_follows_a_symlink_out_of_the_game_folder() {
        let dir = scratch_install("symlink-escape");
        let outside = std::env::temp_dir().join(format!("drop-mod-owned-escape-outside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&outside).unwrap();
        put(&outside.join("save.dat"), "player save");
        std::os::unix::fs::symlink(&outside, dir.join("Saves")).unwrap();
        write_ledger(&dir, "m", &["Saves/save.dat"]);
        let err = remove_mod(&dir, "m").unwrap_err();
        assert!(err.contains("outside the game folder"), "{err}");
        assert_eq!(body(&outside.join("save.dat")).as_deref(), Some("player save"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// Mod a's original of Game.dll could not be put back when a's update
    /// dropped the file, so the live file is still a's copy. Mod b then
    /// overwrites it: b must not back up a's copy as the game's, or removing
    /// a later would delete the real original as a "duplicate".
    #[test]
    fn a_file_another_mod_still_holds_the_original_of_is_not_backed_up_again() {
        let dir = scratch_install("held-elsewhere");
        put(&originals_dir(&dir, "a").join("Game.dll"), "base");
        write_ledger(&dir, "a", &["other.dll"]);
        put(&dir.join("Game.dll"), "mod a");

        let b = write_ledger(&dir, "b", &[]);
        assert!(back_up_originals(&dir, &b, &["Game.dll".to_string()]).unwrap().is_empty());
        b.set_installed_files(vec!["Game.dll".to_string()]);
        b.write();
        put(&dir.join("Game.dll"), "mod b");

        // Removing a passes the real original to b, which still uses the file.
        remove_mod(&dir, "a").unwrap();
        assert_eq!(body(&originals_dir(&dir, "b").join("Game.dll")).as_deref(), Some("base"));
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("mod b"));
        remove_mod(&dir, "b").unwrap();
        assert_eq!(body(&dir.join("Game.dll")).as_deref(), Some("base"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
