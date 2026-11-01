//! Startup disk scan for installed games.
//!
//! Drop persists game status in its encrypted DB, but a user can also have
//! game directories the DB doesn't know about — a fresh DB after a reset, a
//! library folder copied from another machine, etc. Every directory that
//! carries a `.dropdata` marker is a candidate install; this module imports
//! the ones the DB has not already recorded.
//!
//! ## Why a scanned install is NOT trusted as `Installed`
//!
//! A directory existing on disk says nothing about whether its bytes are
//! complete. The LWIW incident (see `downloads::validate`) showed a 62 MB
//! partial of a 660 MB game sitting on disk looking installed. The scan has
//! no server manifest to checksum against, so it cannot *prove* an install
//! is whole. What it *can* read is the `.dropdata` `contexts` map: the
//! per-chunk completion flags the downloader writes as bytes land.
//!
//! Scan policy:
//!   - `.dropdata` present, `contexts` non-empty and **every** flag `true`
//!     → import as a complete install (`InstalledGameType::Installed`).
//!   - `.dropdata` present but `contexts` empty or **any** flag `false`
//!     → import as `PartiallyInstalled` so the user resumes/repairs rather
//!     than launching a broken game.
//!
//! The DB still owns the source of truth; the scan only fills gaps. A game
//! already in `game_statuses` is left untouched.

use std::{
    fs,
    path::{Path, PathBuf},
};

use database::{
    GameDownloadStatus, borrow_db_mut_checked,
    models::data::{DatabaseApplications, InstallRecord, InstalledGameType},
};
use log::{info, warn};
use serde::Serialize;

use crate::{
    downloads::drop_data::{DROPDATA_PATH, DropData},
    library::set_partially_installed_db,
    status::{StatusKind, game_meta, transition_from_db},
};

/// True when the `.dropdata` completion map proves every chunk landed.
///
/// An empty map means the downloader never recorded progress (the directory
/// may have been created but the download never ran) — treated as
/// incomplete. Any `false` flag is an explicitly unfinished chunk.
fn dropdata_reports_complete(drop_data: &DropData) -> bool {
    let contexts = drop_data.get_contexts();
    !contexts.is_empty() && contexts.values().all(|done| *done)
}

/// What one scan imported. `add_download_dir` returns it so the UI can say how
/// many games it found in a folder that already had some.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub installed: usize,
    pub partial: usize,
    /// Installs found that were already in the library (or left alone because
    /// another version of the game is installed or busy).
    pub known: usize,
}

/// Folders under an install dir that hold a `.dropdata`, in a stable order.
///
/// Two layouts exist on disk: `<dir>/<game>` for installs made before
/// multi-version support, and `<dir>/<game>/<version>` for every install made
/// since (`GameDownloadAgent::new`). Both levels are checked on every game
/// folder, because an old single-version folder can also hold a newer
/// per-version install inside it. Dot-folders are Drop's own working folders
/// (`.mods`, `.drop-update`, `.drop-removed`, ...) and never an install.
fn candidate_dirs(install_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for game in subdirs(install_dir) {
        if game.join(DROPDATA_PATH).is_file() {
            found.push(game.clone());
        }
        for version in subdirs(&game) {
            if version.join(DROPDATA_PATH).is_file() {
                found.push(version);
            }
        }
    }
    // Per-version installs first: that layout is newer than `<dir>/<game>`,
    // so when both hold the same game the current one is the one imported.
    found.sort_by_key(|p| (std::cmp::Reverse(p.components().count()), p.clone()));
    found
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

/// Why an install found on disk is left alone, or `None` to import it.
///
/// A `Remote` status is not a reason: it is what an uninstalled or demoted
/// game is left with, and those are exactly the folders worth picking back up.
fn skip_reason(apps: &DatabaseApplications, game_id: &str, version: &str) -> Option<&'static str> {
    if apps.transient_statuses.keys().any(|m| m.id == game_id) {
        return Some("it is downloading or busy");
    }
    if apps.get_install(game_id, version).is_some() {
        return Some("this version is already recorded");
    }
    match apps.game_statuses.get(game_id) {
        Some(GameDownloadStatus::Installed { .. }) => {
            Some("another version of this game is already installed")
        }
        _ => None,
    }
}

/// Scan every configured install dir. Runs at startup.
pub fn scan_install_dirs() -> ScanReport {
    let install_dirs = borrow_db_mut_checked().applications.install_dirs.clone();
    scan_dirs(&install_dirs)
}

/// Scan one install dir, for when it has just been added.
pub fn scan_install_dir(install_dir: &Path) -> ScanReport {
    scan_dirs(&[install_dir.to_path_buf()])
}

fn scan_dirs(install_dirs: &[PathBuf]) -> ScanReport {
    // The folder walk happens before the DB lock is taken, so a slow card or
    // network share doesn't hold up everything else that needs the DB.
    let candidates: Vec<PathBuf> = install_dirs.iter().flat_map(|d| candidate_dirs(d)).collect();
    let mut db_lock = borrow_db_mut_checked();
    let mut report = ScanReport::default();

    {
        for game_path in candidates {
            // `read` points `base_path` at `game_path`, wherever the folder was
            // when the file was written, so a copied or moved library imports
            // at its real location.
            let drop_data = match DropData::read(&game_path) {
                Ok(v) => v,
                Err(err) => {
                    warn!(
                        ".dropdata exists in {}, but couldn't read it. is it corrupted? {:?}",
                        game_path.display(),
                        err
                    );
                    continue;
                }
            };
            if let Some(reason) =
                skip_reason(&db_lock.applications, &drop_data.game_id, &drop_data.game_version)
            {
                if !db_lock
                    .applications
                    .get_install(&drop_data.game_id, &drop_data.game_version)
                    .is_some_and(|r| Path::new(&r.install_dir) == game_path)
                {
                    info!(
                        "[game-status] scan: not importing {} from {}: {reason}",
                        drop_data.game_id,
                        game_path.display()
                    );
                }
                report.known += 1;
                continue;
            }

            let metadata = game_meta(
                drop_data.game_id.clone(),
                drop_data.game_version.clone(),
                drop_data.target_platform,
            );
            let install_dir_str = drop_data.base_path.to_string_lossy().to_string();

            // Decide complete vs. partial from the on-disk chunk flags.
            if dropdata_reports_complete(&drop_data) {
                transition_from_db(&db_lock, &metadata.id, StatusKind::Installed);
                info!(
                    "[game-status] scan: importing {} as Installed from {} ({} chunks all complete)",
                    metadata.id,
                    game_path.display(),
                    drop_data.get_contexts().len()
                );
                // A scanned complete install has no SetupConfiguration data
                // available offline, so it is imported as plain `Installed`.
                // Its GameVersion (launches, setups) is fetched by
                // `library::backfill_game_versions` once the device is online:
                // after a folder is added, and after each online library fetch.
                // Until then launching it fails with `InvalidVersion`.
                db_lock.applications.game_statuses.insert(
                    metadata.id.clone(),
                    GameDownloadStatus::Installed {
                        install_type: InstalledGameType::Installed,
                        version_id: metadata.version.clone(),
                        install_dir: install_dir_str.clone(),
                        update_available: false,
                    },
                );
                db_lock
                    .applications
                    .installed_game_version
                    .insert(metadata.id.clone(), metadata.clone());
                db_lock.applications.upsert_install(InstallRecord {
                    game_id: metadata.id.clone(),
                    version_id: metadata.version.clone(),
                    target_platform: metadata.target_platform,
                    install_dir: install_dir_str,
                    install_type: InstalledGameType::Installed,
                    update_available: false,
                });
                report.installed += 1;
            } else {
                warn!(
                    "[game-status] scan: {} in {} has .dropdata but incomplete chunk flags, \
                     importing as PartiallyInstalled",
                    metadata.id,
                    game_path.display()
                );
                set_partially_installed_db(
                    &mut db_lock,
                    &metadata,
                    install_dir_str,
                    None,
                    // Offline scan can't recover the download-time config; the
                    // GameVersion is fetched later (see the Installed branch).
                    Default::default(),
                );
                report.partial += 1;
            }
        }
    }

    if report.installed + report.partial > 0 {
        info!(
            "[game-status] scan complete: imported {} installed, \
             {} partially-installed game(s) from disk",
            report.installed, report.partial
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop_scan_test_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn mark(dir: &Path) {
        fs::create_dir_all(dir).expect("create install dir");
        fs::write(dir.join(DROPDATA_PATH), b"x").expect("write marker");
    }

    /// Both on-disk layouts are found: the legacy `<dir>/<game>` and the
    /// per-version `<dir>/<game>/<version>` every current install uses,
    /// including a per-version install nested inside a legacy one.
    #[test]
    fn finds_legacy_and_per_version_installs() {
        let root = scratch("layouts");
        mark(&root.join("Legacy"));
        mark(&root.join("Legacy").join("v2"));
        mark(&root.join("Modern").join("v1"));
        fs::create_dir_all(root.join("Modern").join("v1").join("Data")).unwrap();

        let found = candidate_dirs(&root);
        assert_eq!(
            found,
            vec![
                root.join("Legacy").join("v2"),
                root.join("Modern").join("v1"),
                root.join("Legacy"),
            ]
        );
    }

    /// Folders with no `.dropdata`, Drop's own dot-folders and plain files are
    /// not installs, and nothing deeper than two levels is looked at.
    #[test]
    fn ignores_non_installs() {
        let root = scratch("non_installs");
        fs::create_dir_all(root.join("Empty").join("v1")).unwrap();
        mark(&root.join(".drop-update"));
        mark(&root.join("Game").join(".mods"));
        mark(&root.join("Game").join("v1").join("deep"));
        fs::write(root.join("notes.txt"), b"hi").unwrap();

        assert!(candidate_dirs(&root).is_empty());
    }

    /// A missing install dir (unplugged SD card) finds nothing and doesn't panic.
    #[test]
    fn missing_dir_finds_nothing() {
        let root = scratch("missing").join("gone");
        assert!(candidate_dirs(&root).is_empty());
    }
}
