use bitcode::{Decode, Encode};
use database::{
    ApplicationTransientStatus, Database, DownloadType, DownloadableMetadata, GameDownloadStatus,
    GameVersion, borrow_db_checked, borrow_db_mut_checked,
    models::data::{InstallRecord, InstalledGameType, UserConfiguration},
};
use log::{debug, error, warn};
use remote::{
    auth::generate_authorization_header, error::RemoteAccessError, requests::generate_url,
    utils::DROP_CLIENT_ASYNC,
};
use serde::{Deserialize, Serialize};
use std::fs::remove_dir_all;
use std::path::{Path, PathBuf};
use std::thread::spawn;
use tauri::AppHandle;
use utils::app_emit;

use crate::state::{GameStatusManager, GameStatusWithTransient};
use crate::status::{StatusKind, transition_from_db};

#[derive(Serialize, Deserialize, Debug)]
pub struct FetchGameStruct {
    pub game: Game,
    pub status: GameStatusWithTransient,
    pub version: Option<GameVersion>,
}

impl FetchGameStruct {
    pub fn new(game: Game, status: GameStatusWithTransient, version: Option<GameVersion>) -> Self {
        Self {
            game,
            status,
            version,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    #[serde(rename = "type")]
    pub game_type: String,
    pub m_name: String,
    pub m_short_description: String,
    pub m_description: String,
    // mDevelopers
    // mPublishers
    pub m_icon_object_id: String,
    pub m_banner_object_id: String,
    pub m_cover_object_id: String,
    pub m_image_library_object_ids: Vec<String>,
    pub m_image_carousel_object_ids: Vec<String>,
    // Optional metadata: gamepad support + HowLongToBeat times (minutes).
    // Absent on older servers and on games imported before these existed, so the
    // detail page hides them when None. They must live on this struct or the
    // native client silently drops them when deserialising the server payload,
    // which is why they showed on the web view but not in the app.
    #[serde(default)]
    pub m_controller_support: Option<String>,
    #[serde(default)]
    pub m_hltb_main: Option<i64>,
    #[serde(default)]
    pub m_hltb_main_sides: Option<i64>,
    #[serde(default)]
    pub m_hltb_completionist: Option<i64>,
    pub library_path: String,
}
impl Game {
    pub fn id(&self) -> &String {
        &self.id
    }
}
#[derive(serde::Serialize, Clone)]
pub struct GameUpdateEvent {
    pub game_id: String,
    pub status: (
        Option<GameDownloadStatus>,
        Option<ApplicationTransientStatus>,
    ),
    pub version: Option<GameVersion>,
}

/**
 * Called by:
 *  - on_cancel, when cancelled, for obvious reasons
 *  - when downloading, so if drop unexpectedly quits, we can resume the download. hidden by the "Downloading..." transient state, though
 *  - when scanning, to import the game
 */
pub fn set_partially_installed(
    meta: &DownloadableMetadata,
    install_dir: String,
    app_handle: Option<&AppHandle>,
    configuration: UserConfiguration,
) {
    set_partially_installed_db(&mut borrow_db_mut_checked(), meta, install_dir, app_handle, configuration);
}

pub fn set_partially_installed_db(
    db_lock: &mut Database,
    meta: &DownloadableMetadata,
    install_dir: String,
    app_handle: Option<&AppHandle>,
    configuration: UserConfiguration,
) {
    transition_from_db(db_lock, &meta.id, StatusKind::PartiallyInstalled);
    db_lock.applications.transient_statuses.remove(meta);
    db_lock.applications.game_statuses.insert(
        meta.id.clone(),
        GameDownloadStatus::Installed {
            install_type: InstalledGameType::PartiallyInstalled {
                configuration: configuration.clone(),
            },
            version_id: meta.version.clone(),
            install_dir: install_dir.clone(),
            update_available: false,
        },
    );
    db_lock
        .applications
        .installed_game_version
        .insert(meta.id.clone(), meta.clone());
    // Write-through to the per-install map (the multi-version source of truth).
    db_lock.applications.upsert_install(InstallRecord {
        game_id: meta.id.clone(),
        version_id: meta.version.clone(),
        target_platform: meta.target_platform.clone(),
        install_dir,
        install_type: InstalledGameType::PartiallyInstalled { configuration },
        update_available: false,
    });

    if let Some(app_handle) = app_handle {
        push_game_update(
            app_handle,
            &meta.id,
            None,
            GameStatusManager::fetch_state(&meta.id, db_lock),
        );
    }
}

/// Forget a mod's install state: status Remote, no installed version, no
/// per-install records, no transient status. Its files are handled by the
/// caller (`mod_data::remove_mod`, or the base game's folder being deleted).
pub fn clear_mod_install_state(db: &mut Database, mod_game_id: &str) {
    transition_from_db(db, mod_game_id, StatusKind::Remote);
    db.applications
        .transient_statuses
        .retain(|k, _| k.id != mod_game_id);
    let versions: Vec<String> = db
        .applications
        .installs_for_game(mod_game_id)
        .into_iter()
        .map(|r| r.version_id.clone())
        .collect();
    for version in versions {
        db.applications.remove_install(mod_game_id, &version);
    }
    db.applications.installed_game_version.remove(mod_game_id);
    db.applications
        .game_statuses
        .insert(mod_game_id.to_string(), GameDownloadStatus::Remote {});
}

/// Install folders of OTHER installs (any game, any version) that sit strictly
/// inside `install_dir`.
///
/// This happens for real: a game installed before multi-version support lives
/// at `<base>/<game>`, and a newer version of it installed since goes to
/// `<base>/<game>/<version>`, inside the old one. Deleting the old install's
/// whole folder used to delete the newer install with it.
fn installs_nested_in(db: &Database, install_dir: &Path) -> Vec<PathBuf> {
    installs_nested_in_with(db, install_dir, PATHS_IGNORE_CASE)
}

/// Windows compares paths without regard to case (`C:\Games\X` and
/// `c:\games\x` are one folder), so the nesting checks must too, or a
/// differently spelt record of a nested install goes unnoticed and is deleted.
const PATHS_IGNORE_CASE: bool = cfg!(windows);

fn path_parts(p: &Path, ignore_case: bool) -> Vec<String> {
    p.components()
        .map(|c| {
            let s = c.as_os_str().to_string_lossy().to_string();
            if ignore_case { s.to_lowercase() } else { s }
        })
        .collect()
}

fn same_path(a: &Path, b: &Path, ignore_case: bool) -> bool {
    path_parts(a, ignore_case) == path_parts(b, ignore_case)
}

/// `inner` is inside `outer` (or is it).
fn within(inner: &Path, outer: &Path, ignore_case: bool) -> bool {
    path_parts(inner, ignore_case).starts_with(&path_parts(outer, ignore_case))
}

fn installs_nested_in_with(db: &Database, install_dir: &Path, ignore_case: bool) -> Vec<PathBuf> {
    let mut nested: Vec<PathBuf> = db
        .applications
        .installs
        .values()
        .map(|r| PathBuf::from(&r.install_dir))
        .filter(|d| !same_path(d, install_dir, ignore_case) && within(d, install_dir, ignore_case))
        .collect();
    nested.sort();
    nested.dedup();
    nested
}

/// Delete an install folder, except the folders listed in `keep` (other
/// installs nested inside it) and the folders leading to them. With nothing to
/// keep this is `remove_dir_all`. Symlinks are removed, never followed.
fn remove_install_dir(dir: &Path, keep: &[PathBuf]) -> std::io::Result<()> {
    remove_install_dir_with(dir, keep, PATHS_IGNORE_CASE)
}

fn remove_install_dir_with(dir: &Path, keep: &[PathBuf], ignore_case: bool) -> std::io::Result<()> {
    if keep.is_empty() {
        return remove_dir_all(dir);
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if keep.iter().any(|k| same_path(k, &path, ignore_case)) {
            continue;
        }
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.is_dir() && keep.iter().any(|k| within(k, &path, ignore_case)) {
            remove_install_dir_with(&path, keep, ignore_case)?;
            continue;
        }
        if meta.is_dir() {
            remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path).or_else(|e| std::fs::remove_dir(&path).map_err(|_| e))?;
        }
    }
    Ok(())
}

/// Mods recorded as installed inside `install_dir`: every ledger under its
/// `.mods/`, plus any mod whose recorded install folder sits inside it (covers
/// a ledger that was lost). Read BEFORE the folder is deleted.
fn mods_installed_under(db: &Database, install_dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = match crate::downloads::mod_data::installed_ledgers(install_dir) {
        Ok(ledgers) => ledgers.into_iter().map(|l| l.game_id).collect(),
        Err(e) => {
            warn!("could not list mods under {}: {e}", install_dir.display());
            Vec::new()
        }
    };
    for (id, meta) in &db.applications.installed_game_version {
        if meta.download_type != DownloadType::Mod || ids.contains(id) {
            continue;
        }
        if let Some(GameDownloadStatus::Installed { install_dir: dir, .. }) =
            db.applications.game_statuses.get(id)
            && within(Path::new(dir), install_dir, PATHS_IGNORE_CASE)
            // A mod of another install nested inside this one stays with it.
            && !installs_nested_in(db, install_dir)
                .iter()
                .any(|nested| within(Path::new(dir), nested, PATHS_IGNORE_CASE))
        {
            ids.push(id.clone());
        }
    }
    ids
}

pub fn uninstall_game_logic(meta: DownloadableMetadata, app_handle: &AppHandle) {
    debug!("triggered uninstall for agent");
    let mut db_handle = borrow_db_mut_checked();
    transition_from_db(&db_handle, &meta.id, StatusKind::Uninstalling);
    db_handle
        .applications
        .transient_statuses
        .insert(meta.clone(), ApplicationTransientStatus::Uninstalling {});

    push_game_update(
        app_handle,
        &meta.id,
        None,
        GameStatusManager::fetch_state(&meta.id, &db_handle),
    );

    // The directory for THIS specific version (multi-version): prefer the
    // per-install record, fall back to the game-level status for a legacy
    // single install.
    let install_dir = db_handle
        .applications
        .get_install(&meta.id, &meta.version)
        .map(|r| r.install_dir.clone())
        .or_else(|| match db_handle.applications.game_statuses.get(&meta.id) {
            Some(GameDownloadStatus::Installed { install_dir, .. }) => Some(install_dir.clone()),
            _ => None,
        });
    let Some(install_dir) = install_dir else {
        warn!(
            "uninstall job for {} has no known install dir, failing silently",
            meta.id
        );
        return;
    };

    drop(db_handle);

    let app_handle = app_handle.clone();
    spawn(move || {
        // Mods overlay into this folder, so deleting it deletes them too. Note
        // which ones first: their own statuses would otherwise keep saying
        // "installed" with nothing on disk.
        let (orphaned_mods, nested) = {
            let db = borrow_db_checked();
            let dir = Path::new(&install_dir);
            (mods_installed_under(&db, dir), installs_nested_in(&db, dir))
        };
        for n in &nested {
            warn!(
                "uninstalling {} from {install_dir}: keeping {}, another install inside it",
                meta.id,
                n.display()
            );
        }
        let removed = remove_install_dir(Path::new(&install_dir), &nested);
        if let Err(e) = &removed {
            error!("{e}");
        }
        let mut db_handle = borrow_db_mut_checked();
        if removed.is_ok() {
            for mod_id in &orphaned_mods {
                debug!("clearing install state of mod {mod_id}: its base game was uninstalled");
                clear_mod_install_state(&mut db_handle, mod_id);
            }
        }
        db_handle.applications.transient_statuses.remove(&meta);
        db_handle
            .applications
            .remove_install(&meta.id, &meta.version);

        // Repoint the game-level status. Only touch it if it pointed at the
        // version we just removed (or was already gone): if another install of
        // this game remains, point game_statuses at it so the game still shows
        // installed; otherwise the game becomes fully Remote.
        let should_repoint = match db_handle.applications.game_statuses.get(&meta.id) {
            Some(GameDownloadStatus::Installed { version_id, .. }) => *version_id == meta.version,
            _ => true,
        };
        if should_repoint {
            let remaining = db_handle
                .applications
                .installs_for_game(&meta.id)
                .into_iter()
                .next()
                .cloned();
            match remaining {
                Some(rec) => {
                    let status = GameDownloadStatus::Installed {
                        install_type: rec.install_type.clone(),
                        version_id: rec.version_id.clone(),
                        install_dir: rec.install_dir.clone(),
                        update_available: rec.update_available,
                    };
                    transition_from_db(&db_handle, &meta.id, StatusKind::from_persistent(&status));
                    db_handle.applications.installed_game_version.insert(
                        meta.id.clone(),
                        DownloadableMetadata::new(
                            meta.id.clone(),
                            rec.version_id.clone(),
                            rec.target_platform.clone(),
                            DownloadType::Game,
                        ),
                    );
                    db_handle
                        .applications
                        .game_statuses
                        .insert(meta.id.clone(), status);
                }
                None => {
                    transition_from_db(&db_handle, &meta.id, StatusKind::Remote);
                    db_handle
                        .applications
                        .installed_game_version
                        .remove(&meta.id);
                    db_handle
                        .applications
                        .game_statuses
                        .insert(meta.id.clone(), GameDownloadStatus::Remote {});
                }
            }
        }

        push_game_update(
            &app_handle,
            &meta.id,
            None,
            GameStatusManager::fetch_state(&meta.id, &db_handle),
        );

        debug!("uninstalled game id {}", &meta.id);
        app_emit!(&app_handle, "update_library", ());
    });
}

pub fn get_current_meta(game_id: &String) -> Option<DownloadableMetadata> {
    borrow_db_checked()
        .applications
        .installed_game_version
        .get(game_id)
        .cloned()
}

pub async fn on_game_complete(
    meta: &DownloadableMetadata,
    configuration: UserConfiguration,
    install_dir: String,
    app_handle: &AppHandle,
) -> Result<(), RemoteAccessError> {
    // Fetch game version information from remote
    let response = generate_url(
        &["/api/v1/client/game", &meta.id, "version", &meta.version],
        &[],
    )?;
    let response = DROP_CLIENT_ASYNC
        .get(response)
        .header("Authorization", generate_authorization_header()?)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(RemoteAccessError::InvalidResponse(response.json().await?));
    }

    let mut game_version: GameVersion = response.json().await?;
    game_version.user_configuration = configuration;

    let mut handle = borrow_db_mut_checked();
    handle
        .applications
        .game_versions
        .insert(meta.version.clone(), game_version.clone());
    handle
        .applications
        .installed_game_version
        .insert(meta.id.clone(), meta.clone());

    drop(handle);

    let setup_configuration = game_version
        .setups
        .iter()
        .find(|v| v.platform == meta.target_platform);

    let install_type = if setup_configuration.is_none() {
        InstalledGameType::Installed
    } else {
        InstalledGameType::SetupRequired
    };
    let status = GameDownloadStatus::Installed {
        version_id: meta.version.clone(),
        install_dir: install_dir.clone(),
        install_type: install_type.clone(),
        update_available: false,
    };

    let mut db_handle = borrow_db_mut_checked();
    transition_from_db(
        &db_handle,
        &meta.id,
        StatusKind::from_persistent(&status),
    );
    db_handle
        .applications
        .game_statuses
        .insert(meta.id.clone(), status.clone());
    db_handle.applications.transient_statuses.remove(meta);
    // Write-through to the per-install map (the multi-version source of truth).
    db_handle.applications.upsert_install(InstallRecord {
        game_id: meta.id.clone(),
        version_id: meta.version.clone(),
        target_platform: meta.target_platform.clone(),
        install_dir,
        install_type,
        update_available: false,
    });
    drop(db_handle);
    app_emit!(
        app_handle,
        &format!("update_game/{}", meta.id),
        GameUpdateEvent {
            game_id: meta.id.clone(),
            status: (Some(status), None),
            version: Some(game_version),
        }
    );

    app_emit!(app_handle, "update_library", ());

    Ok(())
}

pub fn push_game_update(
    app_handle: &AppHandle,
    game_id: &String,
    version: Option<GameVersion>,
    status: GameStatusWithTransient,
) {
    // A disk-scanned game has no cached GameVersion (that map is only filled by a
    // download or an online library sync), so `version` is legitimately None for
    // it. We MUST still emit the status update: otherwise such a game stays stuck
    // on its transient `Running` in the UI after it exits, hiding every
    // Installed-gated action (Configure, Install runtimes, Uninstall) until an app
    // restart. The frontend keeps its existing version when the payload's is null
    // (see `useGame` in game.ts), so a version-less status emit is safe.
    if let Some(GameDownloadStatus::Installed {
        install_type: InstalledGameType::Installed | InstalledGameType::SetupRequired,
        ..
    }) = &status.0
        && version.is_none()
    {
        warn!(
            "push_game_update: installed game {} has no cached version; emitting a status-only update",
            game_id
        );
    }

    app_emit!(
        app_handle,
        &format!("update_game/{game_id}"),
        GameUpdateEvent {
            game_id: game_id.clone(),
            status,
            version,
        }
    );
}
#[cfg(test)]
mod uninstall_tests {
    use super::*;
    use database::platform::Platform;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop-uninstall-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record(game: &str, version: &str, dir: &Path) -> InstallRecord {
        InstallRecord {
            game_id: game.into(),
            version_id: version.into(),
            target_platform: Platform::Windows,
            install_dir: dir.to_string_lossy().to_string(),
            install_type: InstalledGameType::Installed,
            update_available: false,
        }
    }

    /// The layout that loses data: an install from before multi-version
    /// support at `<base>/<game>`, and a newer version installed since at
    /// `<base>/<game>/<version>` (download_agent.rs builds exactly that path
    /// for a fresh install).
    fn legacy_with_nested(name: &str) -> (PathBuf, PathBuf, PathBuf, Database) {
        let base = scratch(name);
        let legacy = base.join("MyGame");
        let nested = legacy.join("v2");
        std::fs::create_dir_all(nested.join("bin")).unwrap();
        std::fs::write(legacy.join("Game.exe"), b"v1").unwrap();
        std::fs::create_dir_all(legacy.join("data")).unwrap();
        std::fs::write(legacy.join("data/old.pak"), b"v1").unwrap();
        std::fs::write(nested.join("bin/Game.exe"), b"v2").unwrap();
        std::fs::write(nested.join("save.dat"), b"progress").unwrap();
        let mut db = Database::default();
        db.applications.upsert_install(record("g", "v1", &legacy));
        db.applications.upsert_install(record("g", "v2", &nested));
        (base, legacy, nested, db)
    }

    #[test]
    fn deleting_the_legacy_folder_whole_would_delete_the_newer_install() {
        let (base, legacy, nested, db) = legacy_with_nested("proof");
        assert_eq!(installs_nested_in(&db, &legacy), vec![nested.clone()]);
        // What uninstall did before the guard:
        remove_dir_all(&legacy).unwrap();
        assert!(!nested.join("save.dat").exists(), "the newer install went with it");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn uninstalling_the_legacy_install_keeps_the_nested_one() {
        let (base, legacy, nested, db) = legacy_with_nested("guard");
        let keep = installs_nested_in(&db, &legacy);
        remove_install_dir(&legacy, &keep).unwrap();
        assert!(!legacy.join("Game.exe").exists());
        assert!(!legacy.join("data").exists());
        assert_eq!(std::fs::read(nested.join("bin/Game.exe")).unwrap(), b"v2");
        assert_eq!(std::fs::read(nested.join("save.dat")).unwrap(), b"progress");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn uninstalling_the_nested_install_leaves_the_outer_one() {
        let (base, legacy, nested, db) = legacy_with_nested("inner");
        assert!(installs_nested_in(&db, &nested).is_empty());
        remove_install_dir(&nested, &[]).unwrap();
        assert!(legacy.join("Game.exe").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_sibling_with_a_shared_name_prefix_is_not_nested() {
        let base = scratch("prefix");
        let mut db = Database::default();
        db.applications.upsert_install(record("g", "v1", &base.join("Game")));
        db.applications.upsert_install(record("h", "v1", &base.join("Game2")));
        assert!(installs_nested_in(&db, &base.join("Game")).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn on_windows_a_differently_cased_record_is_still_nested() {
        let base = scratch("case");
        let legacy = base.join("MyGame");
        let nested = legacy.join("v2");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("save.dat"), b"progress").unwrap();
        std::fs::write(legacy.join("Game.exe"), b"v1").unwrap();
        let mut db = Database::default();
        db.applications.upsert_install(record("g", "v1", &legacy));
        // The nested record spells the folder differently.
        let respelt = base.join("MYGAME").join("v2");
        db.applications.upsert_install(record("g", "v2", &respelt));
        assert!(installs_nested_in_with(&db, &legacy, false).is_empty(), "case-sensitive: not nested");
        let keep = installs_nested_in_with(&db, &legacy, true);
        assert_eq!(keep, vec![respelt]);
        // Removal compares the same way, so the real folder survives.
        remove_install_dir_with(&legacy, &keep, true).unwrap();
        assert!(nested.join("save.dat").exists());
        assert!(!legacy.join("Game.exe").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
