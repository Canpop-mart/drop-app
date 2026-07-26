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
        if let Err(e) = remove_dir_all(install_dir) {
            error!("{e}");
        }
        let mut db_handle = borrow_db_mut_checked();
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
        .header("Authorization", generate_authorization_header())
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
    if let Some(GameDownloadStatus::Installed {
        install_type: InstalledGameType::Installed | InstalledGameType::SetupRequired,
        ..
    }) = &status.0
        && version.is_none()
    {
        warn!("push_game_update called for installed game {} without version information, skipping", game_id);
        return;
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