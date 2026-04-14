use std::path::Path;
use std::sync::nonpoison::Mutex;

use bitcode::{Decode, Encode};
use database::{
    DownloadableMetadata, GameDownloadStatus, borrow_db_checked, borrow_db_mut_checked,
    models::data::{InstalledGameType, UserConfiguration}, platform::Platform,
};
use games::{
    collections::collection::Collection,
    downloads::error::LibraryError,
    library::{FetchGameStruct, Game, get_current_meta, uninstall_game_logic},
    state::{GameStatusManager, GameStatusWithTransient},
};
use log::warn;
use process::PROCESS_MANAGER;
use remote::{
    auth::generate_authorization_header,
    cache::{cache_object, cache_object_db, get_cached_object},
    error::{DropServerError, RemoteAccessError},
    offline,
    requests::generate_url,
    utils::DROP_CLIENT_ASYNC,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{AppState, collections::fetch_collections};

#[tauri::command]
pub async fn fetch_library(
    state: tauri::State<'_, Mutex<AppState>>,
    app_handle: AppHandle,
    hard_refresh: Option<bool>,
) -> Result<FetchLibraryResponse, RemoteAccessError> {
    offline!(
        state,
        fetch_library_logic,
        fetch_library_logic_offline,
        state,
        app_handle,
        hard_refresh
    )
    .await
}

#[derive(Encode, Decode, Serialize)]
pub struct FetchLibraryResponse {
    library: Vec<Game>,
    collections: Vec<Collection>,
    other: Vec<Game>,
    missing: Vec<Game>,
}

pub async fn fetch_library_logic(
    state: tauri::State<'_, Mutex<AppState>>,
    app_handle: AppHandle,
    hard_refresh: Option<bool>,
) -> Result<FetchLibraryResponse, RemoteAccessError> {
    let do_hard_refresh = hard_refresh.unwrap_or(false);
    if !do_hard_refresh && let Ok(library) = get_cached_object("library") {
        return Ok(library);
    }

    let response = generate_url(&["/api/v1/client/user/library"], &[])?;
    let auth_header = generate_authorization_header();
    let response = DROP_CLIENT_ASYNC
        .get(response)
        .header("Authorization", auth_header)
        .send()
        .await?;

    if response.status() != 200 {
        let err = response.json().await.unwrap_or(DropServerError {
            status_code: 500,
            status_message: "Server Error".to_owned(),
            message: "Invalid response from server.".to_owned(),
        });
        warn!("{err:?}");
        return Err(RemoteAccessError::InvalidResponse(err));
    }

    let library: Vec<Game> = response.json().await?;
    let collections = fetch_collections(state, hard_refresh).await?;

    let mut all_games = library.clone();
    all_games.extend(
        collections
            .iter()
            .flat_map(|v| v.entries.iter().map(|v| v.game.clone())),
    );

    let installed_metas = {
        let mut db_handle = borrow_db_mut_checked();

        for game in &all_games {
            if !db_handle.applications.game_statuses.contains_key(game.id()) {
                db_handle
                    .applications
                    .game_statuses
                    .insert(game.id().clone(), GameDownloadStatus::Remote {});
            }
            cache_object_db(&format!("game/{}", game.id), game, &db_handle)?;
        }

        db_handle
            .applications
            .installed_game_version
            .values()
            .cloned()
            .collect::<Vec<DownloadableMetadata>>()
    };

    // Add games that are installed but no longer in library
    // Use a HashSet for O(1) lookups instead of O(n) linear scan per meta
    let all_game_ids: std::collections::HashSet<&str> =
        all_games.iter().map(|g| g.id().as_str()).collect();
    let mut other = Vec::new();
    let mut missing = Vec::new();
    for meta in installed_metas {
        if all_game_ids.contains(meta.id.as_str()) {
            continue;
        }
        // We should always have a cache of the object
        // Pass db_handle because otherwise we get a gridlock
        let game = match get_cached_object::<Game>(&meta.id.clone()) {
            Ok(game) => game,
            Err(err) => {
                warn!(
                    "{} is installed, but encountered error fetching its error: {}.",
                    meta.id, err
                );
                /*
                 * We can't return a dummy object here because it needs to be in the cache to work
                 * So we uninstall the game so we don't "lose" it
                 */
                uninstall_game_logic(meta.clone(), &app_handle);
                continue;
            }
        };
        if game.game_type == "Game" {
            missing.push(game);
        } else {
            other.push(game);
        }
    }

    let response = FetchLibraryResponse {
        library,
        collections,
        other,
        missing,
    };

    cache_object("library", &response)?;

    Ok(response)
}
pub async fn fetch_library_logic_offline(
    _state: tauri::State<'_, Mutex<AppState>>,
    _app_handle: AppHandle,
    _hard_refresh: Option<bool>,
) -> Result<FetchLibraryResponse, RemoteAccessError> {
    let mut response: FetchLibraryResponse = get_cached_object("library")?;

    let db_handle = borrow_db_checked();

    let retain_filter = |game: &Game| {
        matches!(
            &db_handle
                .applications
                .game_statuses
                .get(game.id())
                .unwrap_or(&GameDownloadStatus::Remote {}),
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::Installed | InstalledGameType::SetupRequired,
                ..
            }
        )
    };

    response.library.retain(retain_filter);
    response.other.retain(retain_filter);
    response.missing.retain(retain_filter);
    response
        .collections
        .iter_mut()
        .for_each(|k| k.entries.retain(|object| retain_filter(&object.game)));

    Ok(response)
}
pub async fn fetch_game_logic(
    id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<FetchGameStruct, RemoteAccessError> {
    let version = {
        let db_lock = borrow_db_checked();

        let metadata_option = db_lock.applications.installed_game_version.get(&id);

        match metadata_option {
            None => None,
            Some(metadata) => db_lock
                .applications
                .game_versions
                .get(&metadata.version)
                .cloned(),
        }
    };

    let game = match get_cached_object::<Game>(&format!("game/{}", id)) {
        Ok(value) => value,
        Err(_) => {
            let client = DROP_CLIENT_ASYNC.clone();
            let response = generate_url(&["/api/v1/client/game", &id], &[])?;
            let response = client
                .get(response)
                .header("Authorization", generate_authorization_header())
                .send()
                .await?;

            if response.status() == 404 {
                let offline_fetch = fetch_game_logic_offline(id.clone(), state).await;
                if let Ok(fetch_data) = offline_fetch {
                    return Ok(fetch_data);
                }

                return Err(RemoteAccessError::GameNotFound(id));
            }
            if response.status() != 200 {
                let err = response.json().await?;
                warn!("{err:?}");
                return Err(RemoteAccessError::InvalidResponse(err));
            }

            let game: Game = response.json().await?;
            game
        }
    };

    let mut db_handle = borrow_db_mut_checked();

    db_handle
        .applications
        .game_statuses
        .entry(id.clone())
        .or_insert(GameDownloadStatus::Remote {});

    let status = GameStatusManager::fetch_state(&id, &db_handle);

    drop(db_handle);

    let data = FetchGameStruct::new(game.clone(), status, version);

    cache_object(&id, &game)?;

    Ok(data)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionDownloadOptionRequiredContent {
    game_id: String,
    version_id: String,
    name: String,
    icon_object_id: String,
    short_description: String,
    size: GameSize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDownloadOption {
    pub game_id: String,
    pub version_id: String,
    display_name: Option<String>,
    version_path: String,
    pub platform: Platform,
    size: GameSize,
    required_content: Vec<VersionDownloadOptionRequiredContent>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSize {
    install_size: usize,
    download_size: usize,
}

pub async fn fetch_game_version_options_logic(
    game_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<VersionDownloadOption>, RemoteAccessError> {
    let client = DROP_CLIENT_ASYNC.clone();

    let previous_id = borrow_db_checked()
        .applications
        .installed_game_version
        .get(&game_id)
        .map(|v| v.version.clone());

    let response = generate_url(
        &["/api/v1/client/game", &game_id, "versions"],
        &[("previous", &previous_id.unwrap_or(String::new()))],
    )?;
    let response = client
        .get(response)
        .header("Authorization", generate_authorization_header())
        .send()
        .await?;

    if response.status() != 200 {
        let err = response.json().await?;
        warn!("{err:?}");
        return Err(RemoteAccessError::InvalidResponse(err));
    }

    let data: Vec<VersionDownloadOption> = response.json().await?;

    // Collect unique platforms from the response, then check validity
    // with locks held briefly, then filter without locks.
    let unique_platforms: Vec<Platform> = {
        let mut seen = std::collections::HashSet::new();
        data.iter()
            .filter(|v| seen.insert(v.platform.clone()))
            .map(|v| v.platform.clone())
            .collect()
    };
    let valid_platforms: std::collections::HashSet<Platform> = {
        let _state_lock = state.lock();
        let pm = PROCESS_MANAGER.lock();
        unique_platforms
            .into_iter()
            .filter(|p| pm.valid_platform(p))
            .collect()
    };
    let data: Vec<VersionDownloadOption> = data
        .into_iter()
        .filter(|v| valid_platforms.contains(&v.platform))
        .collect();

    Ok(data)
}

pub async fn fetch_game_logic_offline(
    id: String,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<FetchGameStruct, RemoteAccessError> {
    let db_handle = borrow_db_checked();
    let metadata_option = db_handle.applications.installed_game_version.get(&id);
    let version = match metadata_option {
        None => None,
        Some(metadata) => db_handle
            .applications
            .game_versions
            .get(&metadata.version)
            .cloned(),
    };

    let status = GameStatusManager::fetch_state(&id, &db_handle);
    let game = get_cached_object::<Game>(&id)?;

    drop(db_handle);

    Ok(FetchGameStruct::new(game, status, version))
}

#[tauri::command]
pub async fn fetch_game(
    game_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<FetchGameStruct, RemoteAccessError> {
    offline!(
        state,
        fetch_game_logic,
        fetch_game_logic_offline,
        game_id,
        state
    )
    .await
}

#[tauri::command]
pub fn fetch_game_status(id: String) -> GameStatusWithTransient {
    let db_handle = borrow_db_checked();
    GameStatusManager::fetch_state(&id, &db_handle)
}

/// Batch-fetch statuses for many games in a single IPC call.
/// Returns a Vec of (id, status) pairs in the same order as the input.
#[tauri::command]
pub fn fetch_game_statuses(ids: Vec<String>) -> Vec<(String, GameStatusWithTransient)> {
    let db_handle = borrow_db_checked();
    ids.into_iter()
        .map(|id| {
            let status = GameStatusManager::fetch_state(&id, &db_handle);
            (id, status)
        })
        .collect()
}

#[tauri::command]
pub fn uninstall_game(game_id: String, app_handle: AppHandle) -> Result<(), LibraryError> {
    let meta = match get_current_meta(&game_id) {
        Some(data) => data,
        None => return Err(LibraryError::MetaNotFound(game_id)),
    };
    uninstall_game_logic(meta, &app_handle);

    Ok(())
}

#[tauri::command]
pub async fn fetch_game_version_options(
    game_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<VersionDownloadOption>, RemoteAccessError> {
    fetch_game_version_options_logic(game_id, state).await
}

/// Configures the Steam emulator (GBE/Goldberg) for an installed game.
/// Writes the user's display name as the in-game profile name and ensures
/// save paths are set correctly. Called from the cog menu on the game page.
#[tauri::command]
pub fn configure_game_emulator(game_id: String) -> Result<String, LibraryError> {
    let db_lock = borrow_db_checked();
    let install_dir = match db_lock
        .applications
        .game_statuses
        .get(&game_id)
        .ok_or(LibraryError::MetaNotFound(game_id.clone()))?
    {
        GameDownloadStatus::Installed { install_dir, .. } => install_dir.clone(),
        _ => return Err(LibraryError::MetaNotFound(game_id)),
    };

    // Get the current user's display name from the cache
    let display_name = get_cached_object::<client::user::User>("user")
        .ok()
        .map(|u| u.display_name().to_string());

    let result = remote::goldberg::configure_saves_for_game(
        &install_dir,
        display_name.as_deref(),
    );

    match result {
        Some(info) => {
            let emu_type = match &info.emulator {
                remote::goldberg::SteamEmulator::Goldberg { .. } => "Goldberg/GBE",
                remote::goldberg::SteamEmulator::SmartSteamEmu { .. } => "SmartSteamEmu",
                remote::goldberg::SteamEmulator::Unknown { .. } => "Unknown",
            };
            Ok(format!(
                "Configured {} emulator. Profile name set to: {}",
                emu_type,
                display_name.as_deref().unwrap_or("<default>")
            ))
        }
        None => Ok("No Steam emulator detected for this game.".to_string()),
    }
}

#[tauri::command]
pub fn update_game_configuration(
    game_id: String,
    options: UserConfiguration,
) -> Result<(), LibraryError> {
    let mut handle = borrow_db_mut_checked();
    let installed_version = handle
        .applications
        .installed_game_version
        .get(&game_id)
        .ok_or(LibraryError::MetaNotFound(game_id))?;

    let _id = installed_version.id.clone();
    let version = installed_version.version.clone();

    let mut existing_configuration = handle
        .applications
        .game_versions
        .get(&version)
        .ok_or(LibraryError::MetaNotFound(version.clone()))?
        .clone();

    existing_configuration.user_configuration = options;

    handle
        .applications
        .game_versions
        .insert(version.to_string(), existing_configuration);

    Ok(())
}

/// Returns the total size (in bytes) of a game's install directory.
/// Walks the directory tree recursively, summing file sizes.
/// Runs on a blocking thread to avoid freezing the async runtime.
#[tauri::command]
pub async fn get_install_size(game_id: String) -> u64 {
    let install_dir = {
        let db = borrow_db_checked();
        match db.applications.game_statuses.get(&game_id) {
            Some(GameDownloadStatus::Installed { install_dir, .. }) => install_dir.clone(),
            _ => return 0,
        }
    }; // db lock released here

    tokio::task::spawn_blocking(move || {
        fn dir_size(path: &Path) -> u64 {
            let mut total: u64 = 0;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_dir() {
                            total += dir_size(&entry.path());
                        } else {
                            total += meta.len();
                        }
                    }
                }
            }
            total
        }
        dir_size(Path::new(&install_dir))
    })
    .await
    .unwrap_or(0)
}
