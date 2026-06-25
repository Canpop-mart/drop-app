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

use std::fs;

use database::{
    GameDownloadStatus, borrow_db_mut_checked,
    models::data::InstalledGameType,
};
use log::{info, warn};

use crate::{
    downloads::drop_data::{DROPDATA_PATH, DropData},
    library::set_partially_installed_db,
    status::{StatusKind, game_meta, transition},
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

pub fn scan_install_dirs() {
    let mut db_lock = borrow_db_mut_checked();
    // Clone only the install_dirs paths (small Vec), not the entire DB,
    // since we need mutable access to db_lock in the loop body.
    let install_dirs = db_lock.applications.install_dirs.clone();
    let mut imported = 0usize;
    let mut imported_partial = 0usize;

    for install_dir in &install_dirs {
        let Ok(files) = fs::read_dir(install_dir) else {
            continue;
        };
        for game in files.into_iter().flatten() {
            let drop_data_file = game.path().join(DROPDATA_PATH);
            if !drop_data_file.exists() {
                continue;
            }
            let drop_data = match DropData::read(&game.path()) {
                Ok(v) => v,
                Err(err) => {
                    warn!(
                        ".dropdata exists for {}, but couldn't read it. is it corrupted? {:?}",
                        game.file_name().display(),
                        err
                    );
                    continue;
                }
            };
            // Skip games already known — the DB is the source of truth.
            if db_lock
                .applications
                .game_statuses
                .contains_key(&drop_data.game_id)
            {
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
                transition(&metadata.id, None, StatusKind::Installed);
                info!(
                    "[game-status] scan: importing {} as Installed ({} chunks all complete)",
                    metadata.id,
                    drop_data.get_contexts().len()
                );
                // A scanned complete install has no SetupConfiguration data
                // available offline, so it is imported as plain `Installed`.
                // The full GameVersion (and any SetupRequired flag) is
                // re-fetched the first time the library syncs online.
                db_lock.applications.game_statuses.insert(
                    metadata.id.clone(),
                    GameDownloadStatus::Installed {
                        install_type: InstalledGameType::Installed,
                        version_id: metadata.version.clone(),
                        install_dir: install_dir_str,
                        update_available: false,
                    },
                );
                db_lock
                    .applications
                    .installed_game_version
                    .insert(metadata.id.clone(), metadata.clone());
                imported += 1;
            } else {
                warn!(
                    "[game-status] scan: {} has .dropdata but incomplete chunk flags, \
                     importing as PartiallyInstalled",
                    metadata.id
                );
                set_partially_installed_db(
                    &mut db_lock,
                    &metadata,
                    install_dir_str,
                    None,
                    drop_data.configuration.clone(),
                );
                imported_partial += 1;
            }
        }
    }

    if imported + imported_partial > 0 {
        info!(
            "[game-status] scan complete: imported {imported} installed, \
             {imported_partial} partially-installed game(s) from disk"
        );
    }
}
