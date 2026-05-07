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
            .filter(|v| seen.insert(v.platform))
            .map(|v| v.platform)
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

// ── Save state management ─────────────────────────────────────────────────

/// Information about a save file or save state.
#[derive(Serialize)]
pub struct SaveFileInfo {
    pub filename: String,
    pub size: u64,
    /// Unix timestamp in seconds
    pub modified: u64,
    /// "save" for battery saves (.srm), "state" for save states (.state)
    pub save_type: String,
}

/// Find the emulator install directory for a game by checking if it has
/// an emulator association via its launch config.
fn find_emulator_saves_dir(game_id: &str) -> Option<std::path::PathBuf> {
    let db = borrow_db_checked();

    // Check all installed game versions for one that has this game's saves
    for (emu_id, status) in db.applications.game_statuses.iter() {
        if let GameDownloadStatus::Installed { install_dir, .. } = status {
            let saves_path = Path::new(install_dir)
                .join("drop-saves")
                .join(game_id);
            if saves_path.exists() {
                return Some(saves_path);
            }
        }
    }

    // Also check if game_id IS the emulator (native game with saves)
    if let Some(GameDownloadStatus::Installed { install_dir, .. }) =
        db.applications.game_statuses.get(game_id)
    {
        let saves_path = Path::new(install_dir).join("drop-saves").join(game_id);
        if saves_path.exists() {
            return Some(saves_path);
        }
    }

    None
}

/// List all save files and save states for a game.
#[tauri::command]
pub fn list_game_saves(game_id: String) -> Vec<SaveFileInfo> {
    let saves_dir = match find_emulator_saves_dir(&game_id) {
        Some(dir) => dir,
        None => return vec![],
    };

    let mut results = Vec::new();

    // List battery saves
    let saves_path = saves_dir.join("saves");
    if let Ok(entries) = std::fs::read_dir(&saves_path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                results.push(SaveFileInfo {
                    filename: entry.file_name().to_string_lossy().to_string(),
                    size: meta.len(),
                    modified: meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    save_type: "save".to_string(),
                });
            }
        }
    }

    // List save states
    let states_path = saves_dir.join("states");
    if let Ok(entries) = std::fs::read_dir(&states_path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                results.push(SaveFileInfo {
                    filename: entry.file_name().to_string_lossy().to_string(),
                    size: meta.len(),
                    modified: meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    save_type: "state".to_string(),
                });
            }
        }
    }

    // Sort by modification time, newest first (Reverse for descending)
    results.sort_by_key(|b| std::cmp::Reverse(b.modified));
    results
}

/// Read a save file's content as base64 for cloud upload.
#[tauri::command]
pub fn read_save_file(
    game_id: String,
    filename: String,
    save_type: String,
) -> Result<String, String> {
    let saves_dir = find_emulator_saves_dir(&game_id)
        .ok_or_else(|| "Save directory not found".to_string())?;

    let subdir = match save_type.as_str() {
        "save" => "saves",
        "state" => "states",
        _ => return Err("Invalid save type".to_string()),
    };

    let file_path = saves_dir.join(subdir).join(&filename);
    let canonical = file_path
        .canonicalize()
        .map_err(|e| format!("File not found: {e}"))?;
    let base = saves_dir
        .join(subdir)
        .canonicalize()
        .map_err(|e| format!("Directory error: {e}"))?;
    if !canonical.starts_with(&base) {
        return Err("Invalid file path".to_string());
    }

    let data = std::fs::read(&canonical).map_err(|e| format!("Failed to read: {e}"))?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

/// Write base64-encoded save data to a local save file (for cloud download).
#[tauri::command]
pub fn write_save_file(
    game_id: String,
    filename: String,
    save_type: String,
    data: String,
) -> Result<(), String> {
    let saves_dir = find_emulator_saves_dir(&game_id)
        .ok_or_else(|| "Save directory not found".to_string())?;

    let subdir = match save_type.as_str() {
        "save" => "saves",
        "state" => "states",
        _ => return Err("Invalid save type".to_string()),
    };

    // Security check: filename must be a plain leaf name with no traversal.
    if filename.is_empty()
        || filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
    {
        return Err("Invalid filename".to_string());
    }

    let dir = saves_dir.join(subdir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {e}"))?;

    let file_path = dir.join(&filename);

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    std::fs::write(&file_path, &bytes).map_err(|e| format!("Failed to write: {e}"))?;
    Ok(())
}

// ── PC save file I/O (arbitrary paths from Ludusavi) ─────────────────────

/// Validate that a PC save path is absolute and contains no traversal
/// components. Returns the path if safe, or an error string otherwise.
fn validate_pc_save_path(file_path: &str) -> Result<std::path::PathBuf, String> {
    use std::path::{Component, PathBuf};
    let path = PathBuf::from(file_path);
    if !path.is_absolute() {
        return Err("File path must be absolute".to_string());
    }
    for comp in path.components() {
        if matches!(comp, Component::ParentDir) {
            return Err("File path contains invalid component".to_string());
        }
    }
    Ok(path)
}

/// Read a PC save file by its full path (as returned by Ludusavi) as base64.
#[tauri::command]
pub fn read_pc_save_file(file_path: String) -> Result<String, String> {
    let path = validate_pc_save_path(&file_path)?;
    if !path.exists() {
        return Err("File not found".to_string());
    }
    let data = std::fs::read(&path).map_err(|e| format!("Failed to read: {e}"))?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

/// Write base64-encoded data to a PC save file at its full path.
/// Used for restoring individual cloud saves to their original location.
#[tauri::command]
pub fn write_pc_save_file(file_path: String, data: String) -> Result<(), String> {
    let path = validate_pc_save_path(&file_path)?;
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {e}"))?;
    }
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| format!("Invalid base64: {e}"))?;
    // If file exists, create a backup first
    if path.exists() {
        let backup = path.with_extension(
            path.extension()
                .map(|e| format!("{}.bak", e.to_string_lossy()))
                .unwrap_or_else(|| "bak".to_string()),
        );
        let _ = std::fs::copy(&path, &backup); // best-effort backup
    }
    std::fs::write(&path, &bytes).map_err(|e| format!("Failed to write: {e}"))?;
    Ok(())
}

// ── Ludusavi integration for PC game saves ────────────────────────────────

/// Result from Ludusavi backup/find operation.
#[derive(Serialize)]
pub struct LudusaviSaveInfo {
    pub files: Vec<LudusaviFile>,
    pub game_name: String,
}

#[derive(Serialize)]
pub struct LudusaviFile {
    pub path: String,
    pub size: u64,
    pub modified: u64,
}

/// Ludusavi release info for auto-download.
/// Windows ships as .zip, Linux/macOS as .tar.gz — upstream convention,
/// not our choice. Mismatch here was the reason Deck installs 404'd.
const LUDUSAVI_VERSION: &str = "0.27.0";
#[cfg(target_os = "windows")]
const LUDUSAVI_ARCHIVE: &str = "ludusavi-v0.27.0-win64.zip";
#[cfg(target_os = "linux")]
const LUDUSAVI_ARCHIVE: &str = "ludusavi-v0.27.0-linux.tar.gz";
#[cfg(target_os = "macos")]
const LUDUSAVI_ARCHIVE: &str = "ludusavi-v0.27.0-mac.tar.gz";

/// Get the directory where Drop stores bundled tools.
fn tools_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("drop")
        .join("tools")
}

/// Find Ludusavi binary — check Drop's tools dir, then PATH, then common locations.
fn find_ludusavi() -> Option<std::path::PathBuf> {
    // Check Drop's bundled tools directory first
    let tools = tools_dir();
    #[cfg(target_os = "windows")]
    let bundled = tools.join("ludusavi").join("ludusavi.exe");
    #[cfg(not(target_os = "windows"))]
    let bundled = tools.join("ludusavi").join("ludusavi");

    if bundled.exists() {
        return Some(bundled);
    }

    // Check PATH
    if let Ok(output) = std::process::Command::new("ludusavi").arg("--version").output()
        && output.status.success()
    {
        return Some(std::path::PathBuf::from("ludusavi"));
    }

    // Check common install locations
    #[cfg(target_os = "windows")]
    {
        let paths = [
            dirs::data_local_dir().map(|d| d.join("Programs").join("ludusavi").join("ludusavi.exe")),
            dirs::home_dir().map(|d| d.join("scoop").join("apps").join("ludusavi").join("current").join("ludusavi.exe")),
        ];
        for path in paths.into_iter().flatten() {
            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let paths = [
            Some(std::path::PathBuf::from("/usr/bin/ludusavi")),
            dirs::home_dir().map(|d| d.join(".local").join("bin").join("ludusavi")),
        ];
        for path in paths.into_iter().flatten() {
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// Download and install Ludusavi to Drop's tools directory.
/// Returns the path to the installed binary.
#[tauri::command]
pub async fn install_ludusavi() -> Result<String, String> {
    use log::info;

    let download_url = format!(
        "https://github.com/mtkennerly/ludusavi/releases/download/v{}/{}",
        LUDUSAVI_VERSION, LUDUSAVI_ARCHIVE
    );

    let tools = tools_dir();
    let ludusavi_dir = tools.join("ludusavi");
    std::fs::create_dir_all(&ludusavi_dir)
        .map_err(|e| format!("Failed to create tools dir: {e}"))?;

    info!("[LUDUSAVI] Downloading from {}", download_url);
    info!("[LUDUSAVI] Target dir: {} (exists: {})", ludusavi_dir.display(), ludusavi_dir.exists());

    // Download the archive
    let response = reqwest::get(&download_url)
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    let status = response.status();
    info!("[LUDUSAVI] HTTP response: {}", status);
    if !status.is_success() {
        return Err(format!("Download failed: HTTP {}", status));
    }

    let bytes = response.bytes().await.map_err(|e| format!("Download failed: {e}"))?;
    info!("[LUDUSAVI] Downloaded {} bytes", bytes.len());

    #[cfg(target_os = "windows")]
    let out_path = extract_ludusavi_from_zip(&bytes, &ludusavi_dir)?;
    #[cfg(not(target_os = "windows"))]
    let out_path = extract_ludusavi_from_tar_gz(&bytes, &ludusavi_dir)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions on {}: {e}", out_path.display()))?;
        info!("[LUDUSAVI] Set executable permissions (0o755)");
    }

    // Verify the binary exists and is executable
    match std::fs::metadata(&out_path) {
        Ok(m) => info!("[LUDUSAVI] Installed to {} (size={})", out_path.display(), m.len()),
        Err(e) => log::warn!("[LUDUSAVI] Installed but can't stat: {e}"),
    }

    // Quick sanity check — try running --version
    match std::process::Command::new(&out_path).arg("--version").output() {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            info!(
                "[LUDUSAVI] Version check: status={}, stdout={:?}, stderr={:?}",
                o.status, stdout.trim(), stderr.trim()
            );
        }
        Err(e) => log::warn!("[LUDUSAVI] Version check failed (binary may not run on this platform): {e}"),
    }

    Ok(out_path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn extract_ludusavi_from_zip(
    bytes: &[u8],
    ludusavi_dir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    use log::{info, warn};

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("Failed to open zip archive: {e}"))?;

    info!("[LUDUSAVI] Zip contains {} entries", archive.len());
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Archive error: {e}"))?;
        let name = file.name().to_string();
        if !name.contains("ludusavi") || name.ends_with('/') {
            continue;
        }
        let out_name = if name.ends_with(".exe") { "ludusavi.exe" } else { "ludusavi" };
        let out_path = ludusavi_dir.join(out_name);

        // Write to a sibling temp file first. If ludusavi.exe is currently
        // running or its handle is still held by Windows (AV, Explorer
        // thumbnail cache, recently-exited `backup_pc_game_saves` child),
        // overwriting it directly with File::create yields
        // `os error 32: The process cannot access the file because it is
        // being used by another process`. Writing aside and then swapping
        // lets us fall back to a rename-old-aside strategy that works
        // even if the in-place binary is still in use.
        let tmp_path = ludusavi_dir.join(format!("{out_name}.new"));
        info!("[LUDUSAVI] Extracting to temp: {}", tmp_path.display());

        // Drain the zip entry into memory once — we may need to retry the
        // filesystem write without re-reading from the archive.
        let mut buf = Vec::with_capacity(file.size() as usize);
        std::io::copy(&mut file, &mut buf)
            .map_err(|e| format!("Failed to read zip entry {name}: {e}"))?;

        // Remove any stale .new from a previous half-failed install.
        let _ = std::fs::remove_file(&tmp_path);

        std::fs::write(&tmp_path, &buf)
            .map_err(|e| format!("Failed to write temp file {}: {e}", tmp_path.display()))?;

        // Try to swap tmp_path -> out_path with backoff. Windows will
        // block the rename while the target has open handles.
        let delays_ms = [0u64, 100, 250, 500, 1000, 2000];
        let mut swapped = false;
        let mut last_err: Option<String> = None;
        for (attempt, &delay) in delays_ms.iter().enumerate() {
            if delay > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            // If the target doesn't exist, a plain rename works.
            // If it exists, std::fs::rename on Windows is equivalent to
            // MoveFileEx without REPLACE_EXISTING and will fail. Use the
            // explicit two-step: remove, then rename.
            let pre_rename = if out_path.exists() {
                std::fs::remove_file(&out_path)
            } else {
                Ok(())
            };
            match pre_rename {
                Ok(()) => match std::fs::rename(&tmp_path, &out_path) {
                    Ok(()) => {
                        swapped = true;
                        info!(
                            "[LUDUSAVI] Swapped {} into place on attempt {}",
                            out_path.display(),
                            attempt + 1
                        );
                        break;
                    }
                    Err(e) => {
                        last_err = Some(format!("rename failed: {e}"));
                    }
                },
                Err(e) => {
                    last_err = Some(format!("remove-existing failed: {e}"));
                }
            }
        }

        if !swapped {
            // Final fallback: rename the locked binary aside so the new
            // one can take its place. Windows permits renaming a running
            // .exe (the handle sticks to the old name). The stale file
            // gets cleaned up next install or reboot.
            let aside = ludusavi_dir.join(format!(
                "{out_name}.old-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ));
            warn!(
                "[LUDUSAVI] Could not replace {} (last error: {:?}); trying rename-aside to {}",
                out_path.display(),
                last_err,
                aside.display()
            );
            if let Err(e) = std::fs::rename(&out_path, &aside) {
                return Err(format!(
                    "Failed to replace {} after retries ({:?}) and rename-aside also failed: {e}. \
                     Close any running Ludusavi instances and try again.",
                    out_path.display(),
                    last_err
                ));
            }
            std::fs::rename(&tmp_path, &out_path).map_err(|e| {
                format!(
                    "Failed to move new binary into place after rename-aside: {e}",
                )
            })?;
            info!(
                "[LUDUSAVI] Rename-aside succeeded; old binary parked at {}",
                aside.display()
            );
        }

        // Best-effort cleanup of prior rename-aside leftovers from any
        // earlier install where the old handle was still held.
        if let Ok(entries) = std::fs::read_dir(ludusavi_dir) {
            let stem = format!("{out_name}.old-");
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.starts_with(&stem) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }

        info!("[LUDUSAVI] Extracted to: {}", out_path.display());
        return Ok(out_path);
    }

    Err("Ludusavi binary not found in zip archive".to_string())
}

#[cfg(not(target_os = "windows"))]
fn extract_ludusavi_from_tar_gz(
    bytes: &[u8],
    ludusavi_dir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    use log::info;
    use std::io::Read;

    let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = tar::Archive::new(gz);

    for entry_result in archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {e}"))?
    {
        let mut entry =
            entry_result.map_err(|e| format!("Failed to iterate tar entry: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to read tar entry path: {e}"))?
            .into_owned();
        let name = path.to_string_lossy().to_string();

        // Upstream tar contains `ludusavi` at the root. Match it loosely so
        // we work even if the archive layout changes.
        let is_binary = path
            .file_name()
            .map(|f| f == "ludusavi")
            .unwrap_or(false);
        if !is_binary || entry.header().entry_type().is_dir() {
            continue;
        }

        let out_path = ludusavi_dir.join("ludusavi");
        info!("[LUDUSAVI] Extracting {} -> {}", name, out_path.display());
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read tar entry body: {e}"))?;
        std::fs::write(&out_path, &buf)
            .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))?;
        return Ok(out_path);
    }

    Err("Ludusavi binary not found in tar.gz archive".to_string())
}

/// Try to find the Steam App ID for a game from its install directory.
fn find_steam_app_id(game_id: &str) -> Option<String> {
    let db = borrow_db_checked();
    let install_dir = match db.applications.game_statuses.get(game_id) {
        Some(GameDownloadStatus::Installed { install_dir, .. }) => install_dir.clone(),
        _ => return None,
    };
    drop(db);

    // Check steam_appid.txt in the install directory
    let appid_path = Path::new(&install_dir).join("steam_appid.txt");
    if let Ok(contents) = std::fs::read_to_string(&appid_path) {
        let trimmed = contents.trim();
        if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit()) {
            return Some(trimmed.to_string());
        }
    }

    // Check inside drop-goldberg subdirectories (in install dir)
    let goldberg_dir = Path::new(&install_dir).join("drop-goldberg");
    if let Ok(entries) = std::fs::read_dir(&goldberg_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.chars().all(|c| c.is_ascii_digit()) {
                    return Some(name);
                }
            }
        }
    }

    // Check %AppData%/drop-goldberg/ (shared Goldberg save location)
    if let Some(appdata) = dirs::data_dir() {
        let shared_goldberg = appdata.join("drop-goldberg");
        if let Ok(entries) = std::fs::read_dir(&shared_goldberg) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.chars().all(|c| c.is_ascii_digit()) {
                        // Verify this AppID belongs to our game by checking
                        // if the achievements.json references exist
                        let ach_path = entry.path().join("achievements.json");
                        if ach_path.exists() || entry.path().join("stats.json").exists() {
                            // Can't be 100% sure it's this game, but if there's
                            // only one or the ID matches steam_appid.txt elsewhere,
                            // it's a good match.
                            log::info!("[LUDUSAVI] Found potential AppID {} in shared goldberg dir", name);
                            return Some(name);
                        }
                    }
                }
            }
        }
    }

    None
}

/// List PC game save locations using Ludusavi.
/// Returns the files Ludusavi finds for this game.
#[tauri::command]
pub fn list_pc_game_saves(game_id: String, game_name: String) -> Result<LudusaviSaveInfo, String> {
    let ludusavi = find_ludusavi().ok_or("Ludusavi not installed")?;

    // Try Steam App ID first (more accurate), fall back to game name
    let app_id = find_steam_app_id(&game_id);

    // Step 1: Use "find" to resolve the game's canonical name from Steam ID
    // Step 2: Use "backup --preview --api <name>" to scan for actual files
    let resolved_name = if let Some(ref id) = app_id {
        let find_output = std::process::Command::new(&ludusavi)
            .args(["find", "--api", "--steam-id", id])
            .output()
            .ok();
        find_output.and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            // Parse the game name from find output (first key in "games" object)
            serde_json::from_str::<serde_json::Value>(&s).ok()
                .and_then(|v| v.get("games")?.as_object()?.keys().next().map(|k| k.to_string()))
        })
    } else {
        None
    };

    let search_name = resolved_name.as_deref().unwrap_or(&game_name);
    log::info!("[LUDUSAVI] Resolved game name: '{}' (from Steam ID {:?})", search_name, app_id);

    // Use "backup --preview --api" which actually scans the filesystem.
    // Try the resolved name first, then the original name, then subtitle-stripped.
    let mut output = std::process::Command::new(&ludusavi)
        .args(["backup", "--preview", "--api", search_name])
        .output();

    // If the resolved name failed or found nothing, try name variants
    let needs_retry = output.as_ref()
        .map(|o| !o.status.success() || o.stdout.len() < 50)
        .unwrap_or(true);

    if needs_retry && search_name != game_name {
        log::info!("[LUDUSAVI] Retrying with original name: '{}'", game_name);
        output = std::process::Command::new(&ludusavi)
            .args(["backup", "--preview", "--api", &game_name])
            .output();
    }

    // Try subtitle-stripped version
    let short_name = game_name.split(" - ").next().unwrap_or(&game_name).trim();
    let needs_retry2 = output.as_ref()
        .map(|o| !o.status.success() || o.stdout.len() < 50)
        .unwrap_or(true);

    if needs_retry2 && short_name != game_name && short_name != search_name {
        log::info!("[LUDUSAVI] Retrying with short name: '{}'", short_name);
        output = std::process::Command::new(&ludusavi)
            .args(["backup", "--preview", "--api", short_name])
            .output();
    }

    let output = output.map_err(|e| format!("Failed to run Ludusavi: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    log::info!(
        "[LUDUSAVI] find command for '{}' (app_id: {:?}) — status: {}, stdout: [{}], stderr: {}",
        game_name,
        app_id,
        output.status,
        stdout.trim(),
        if stderr.is_empty() { "(empty)" } else { &stderr }
    );

    if !output.status.success() {
        // "No matching games" is not an error, just means no saves found
        if stderr.contains("No matching") || stdout.is_empty() {
            return Ok(LudusaviSaveInfo {
                files: vec![],
                game_name: game_name.clone(),
            });
        }
        return Err(format!("Ludusavi error: {}", stderr));
    }

    // Parse Ludusavi JSON API output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse Ludusavi output: {e}"))?;

    let mut files = Vec::new();
    let mut resolved_name = game_name.clone();

    if let Some(games) = json.get("games").and_then(|g| g.as_object()) {
        for (name, game_data) in games {
            resolved_name = name.clone();
            if let Some(game_files) = game_data.get("files").and_then(|f| f.as_object()) {
                for (path, file_data) in game_files {
                    let size = file_data.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                    files.push(LudusaviFile {
                        path: path.clone(),
                        size,
                        modified: 0, // Ludusavi doesn't always provide this
                    });
                }
            }
        }
    }

    // If Ludusavi found nothing, try common save locations as a fallback
    if files.is_empty() {
        log::info!("[LUDUSAVI] No files found via Ludusavi, scanning common save locations for '{}'", game_name);
        let common_saves = scan_common_save_locations(&game_name, app_id.as_deref());
        if !common_saves.is_empty() {
            log::info!("[LUDUSAVI] Found {} files in common locations", common_saves.len());
            return Ok(LudusaviSaveInfo {
                files: common_saves,
                game_name: game_name.clone(),
            });
        }
    }

    Ok(LudusaviSaveInfo {
        files,
        game_name: resolved_name,
    })
}

/// Scan common Windows/Linux save locations for a game that Ludusavi doesn't know about.
fn scan_common_save_locations(game_name: &str, app_id: Option<&str>) -> Vec<LudusaviFile> {
    let mut results = Vec::new();

    // Build name variations to search
    let mut name_variants: Vec<String> = vec![game_name.to_string()];
    // Without subtitle (e.g., "Retro Rewind - Video Store Simulator" → "Retro Rewind")
    if let Some(idx) = game_name.find(" - ") {
        name_variants.push(game_name[..idx].to_string());
    }
    if let Some(idx) = game_name.find(": ") {
        name_variants.push(game_name[..idx].to_string());
    }
    // Without special characters
    let clean = game_name.replace([':', '-', '\'', '!', '.', ','], "").replace("  ", " ").trim().to_string();
    if clean != game_name { name_variants.push(clean.clone()); }
    // No spaces (from full name and from each variant)
    let no_spaces = game_name.replace(' ', "");
    if no_spaces != game_name { name_variants.push(no_spaces); }
    // No spaces from subtitle-stripped version
    for variant in name_variants.clone() {
        let ns = variant.replace(' ', "");
        if ns != variant { name_variants.push(ns); }
    }

    // Deduplicate
    name_variants.sort();
    name_variants.dedup();

    log::info!("[LUDUSAVI:FALLBACK] Name variants: {:?}", name_variants);

    // Build a list of directories to check
    let mut search_dirs: Vec<std::path::PathBuf> = Vec::new();

    #[cfg(target_os = "windows")]
    {
        for name in &name_variants {
            if let Some(appdata) = dirs::data_local_dir() {
                search_dirs.push(appdata.join(name));
            }
            if let Some(appdata_roaming) = dirs::data_dir() {
                search_dirs.push(appdata_roaming.join(name));
            }
            // %AppData%/../LocalLow/ — check both direct and under company subfolders
            if let Some(appdata) = dirs::data_dir() {
                if let Some(parent) = appdata.parent() {
                    let local_low = parent.join("LocalLow");
                    search_dirs.push(local_low.join(name));

                    // Unity games: LocalLow/<CompanyName>/<GameName>/
                    // Scan all subdirs of LocalLow for a folder matching the game name
                    if let Ok(entries) = std::fs::read_dir(&local_low) {
                        for entry in entries.flatten() {
                            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                let sub = entry.path().join(name);
                                if sub.exists() {
                                    search_dirs.push(sub);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Also search Documents/My Games/
        for name in &name_variants {
            if let Some(docs) = dirs::document_dir() {
                search_dirs.push(docs.join("My Games").join(name));
            }
        }
        if let Some(docs) = dirs::document_dir() {
            search_dirs.push(docs.join("My Games").join(game_name));
        }
        // Steam userdata saves: %ProgramFiles(x86)%/Steam/userdata/*/
        if let Some(id) = app_id {
            if let Ok(program_files) = std::env::var("ProgramFiles(x86)") {
                let userdata = Path::new(&program_files).join("Steam").join("userdata");
                if let Ok(entries) = std::fs::read_dir(&userdata) {
                    for entry in entries.flatten() {
                        search_dirs.push(entry.path().join(id).join("remote"));
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            // ~/.local/share/<GameName>/
            search_dirs.push(home.join(".local").join("share").join(game_name));
            // ~/.config/unity3d/<GameName>/
            search_dirs.push(home.join(".config").join("unity3d").join(game_name));
        }
    }

    // Scan each directory for save-like files
    let save_extensions = ["sav", "save", "dat", "json", "xml", "db", "sqlite", "bin", "cfg"];

    for dir in &search_dirs {
        if !dir.is_dir() {
            continue;
        }
        log::info!("[LUDUSAVI:FALLBACK] Scanning: {}", dir.display());
        scan_dir_for_saves(dir, &save_extensions, &mut results, 2); // max depth 2
    }

    results
}

/// Recursively scan a directory for save files up to max_depth.
fn scan_dir_for_saves(
    dir: &Path,
    extensions: &[&str],
    results: &mut Vec<LudusaviFile>,
    max_depth: u32,
) {
    if max_depth == 0 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return; };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir_for_saves(&path, extensions, results, max_depth - 1);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if extensions.contains(&ext.as_str())
                && let Ok(meta) = entry.metadata()
            {
                results.push(LudusaviFile {
                    path: path.to_string_lossy().to_string(),
                    size: meta.len(),
                    modified: meta.modified().ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                });
            }
        }
    }
}

/// Backup PC game saves using Ludusavi to a temporary directory.
/// Returns the backup path for upload.
#[tauri::command]
pub fn backup_pc_game_saves(game_id: String, game_name: String) -> Result<String, String> {
    let ludusavi = find_ludusavi().ok_or("Ludusavi not installed")?;

    let backup_dir = std::env::temp_dir().join(format!("drop-ludusavi-{game_id}"));
    let _ = std::fs::remove_dir_all(&backup_dir); // Clean previous
    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("Failed to create backup dir: {e}"))?;

    let app_id = find_steam_app_id(&game_id);

    // Resolve canonical name from Steam ID (backup doesn't accept --steam-id)
    let resolved_name = if let Some(ref id) = app_id {
        std::process::Command::new(&ludusavi)
            .args(["find", "--api", "--steam-id", id])
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                serde_json::from_str::<serde_json::Value>(&s).ok()
                    .and_then(|v| v.get("games")?.as_object()?.keys().next().map(|k| k.to_string()))
            })
    } else {
        None
    };
    let search_name = resolved_name.as_deref().unwrap_or(&game_name);

    let mut cmd = std::process::Command::new(&ludusavi);
    cmd.args([
        "backup",
        "--api",
        "--force",
        "--path",
        &backup_dir.to_string_lossy(),
        search_name,
    ]);

    let output = cmd.output().map_err(|e| format!("Failed to run Ludusavi: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Ludusavi backup failed: {}", stderr));
    }

    Ok(backup_dir.to_string_lossy().to_string())
}

/// Restore PC game saves from a Ludusavi backup directory.
#[tauri::command]
pub fn restore_pc_game_saves(backup_path: String) -> Result<(), String> {
    let ludusavi = find_ludusavi().ok_or("Ludusavi not installed")?;

    let output = std::process::Command::new(&ludusavi)
        .args(["restore", "--api", "--force", "--path", &backup_path])
        .output()
        .map_err(|e| format!("Failed to run Ludusavi: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Ludusavi restore failed: {}", stderr));
    }

    Ok(())
}

/// Check if Ludusavi is available on the system.
/// Also updates its manifest (PCGamingWiki data) if it hasn't been updated recently.
#[tauri::command]
pub async fn check_ludusavi() -> bool {
    let ludusavi = match find_ludusavi() {
        Some(path) => path,
        None => return false,
    };

    // Update Ludusavi's game database from PCGamingWiki (runs in background)
    // This ensures newly added games are recognized.
    let marker = tools_dir().join("ludusavi").join(".last-update");
    let should_update = if let Ok(meta) = std::fs::metadata(&marker) {
        // Update at most once per day
        meta.modified().ok()
            .and_then(|t| t.elapsed().ok())
            .map(|d| d.as_secs() > 86400)
            .unwrap_or(true)
    } else {
        true
    };

    if should_update {
        let lud = ludusavi.clone();
        tokio::task::spawn_blocking(move || {
            log::info!("[LUDUSAVI] Updating manifest from PCGamingWiki...");
            let output = std::process::Command::new(&lud)
                .arg("manifest")
                .arg("update")
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    log::info!("[LUDUSAVI] Manifest updated successfully");
                    let _ = std::fs::write(&marker, b"updated");
                }
                Ok(o) => {
                    log::warn!("[LUDUSAVI] Manifest update failed: {}", String::from_utf8_lossy(&o.stderr));
                }
                Err(e) => log::warn!("[LUDUSAVI] Manifest update error: {e}"),
            }
        }).await.ok();
    }

    true
}

/// Delete a specific save file or save state.
#[tauri::command]
pub fn delete_game_save(
    game_id: String,
    filename: String,
    save_type: String,
) -> Result<(), String> {
    let saves_dir = find_emulator_saves_dir(&game_id)
        .ok_or_else(|| "Save directory not found".to_string())?;

    let subdir = match save_type.as_str() {
        "save" => "saves",
        "state" => "states",
        _ => return Err("Invalid save type".to_string()),
    };

    let file_path = saves_dir.join(subdir).join(&filename);

    // Security: ensure the resolved path is still inside the saves directory
    let canonical = file_path
        .canonicalize()
        .map_err(|e| format!("File not found: {e}"))?;
    let base = saves_dir
        .join(subdir)
        .canonicalize()
        .map_err(|e| format!("Directory error: {e}"))?;
    if !canonical.starts_with(&base) {
        return Err("Invalid file path".to_string());
    }

    std::fs::remove_file(&canonical).map_err(|e| format!("Failed to delete: {e}"))
}

/// Check whether a game's ROM hash matches RetroAchievements' known hashes.
///
/// This is the on-demand version callable from the UI — separate from the
/// automatic check that runs at launch time. Returns a JSON-serialisable
/// status enum.
#[tauri::command]
pub async fn check_ra_rom_hash(
    game_id: String,
) -> Result<serde_json::Value, String> {
    use database::{borrow_db_checked, GameDownloadStatus, models::data::InstalledGameType};

    // 1. Find the installed game version and its emulator
    let (install_dir, game_version) = {
        let db = borrow_db_checked();
        let status = db
            .applications
            .game_statuses
            .get(&game_id)
            .ok_or("Game not found")?
            .clone();

        let install_dir = match &status {
            GameDownloadStatus::Installed { install_dir, .. } => install_dir.clone(),
            _ => return Err("Game not installed".to_string()),
        };

        let gv = db
            .applications
            .game_versions
            .get(&game_id)
            .ok_or("Game version not found")?
            .clone();

        (install_dir, gv)
    };

    // 2. Find the first launch config with an emulator (RetroArch)
    let launch_config = game_version
        .launches
        .iter()
        .find(|l| l.emulator.is_some())
        .ok_or("No emulator launch config")?;

    let emulator_ref = launch_config.emulator.as_ref().unwrap();

    // 3. Resolve emulator install directory
    let emu_install_dir = {
        let db = borrow_db_checked();
        let emu_status = db
            .applications
            .game_statuses
            .get(&emulator_ref.game_id)
            .ok_or("Emulator not installed")?
            .clone();

        match emu_status {
            GameDownloadStatus::Installed { install_dir, .. } => install_dir,
            _ => return Err("Emulator not installed".to_string()),
        }
    };

    // 4. Resolve ROM path (same logic as process_manager)
    let rom_path = if launch_config.disc_paths.len() > 1 {
        // Multi-disc: use the first disc for hashing
        let game_dir = std::path::Path::new(&install_dir);
        game_dir
            .join(&launch_config.disc_paths[0])
            .to_string_lossy()
            .to_string()
    } else {
        // Single ROM: the launch command is the ROM filename (relative to install_dir).
        // Strip any surrounding quotes and use it directly.
        let cmd = launch_config.command.trim().trim_matches('"').trim_matches('\'');
        let rom = std::path::Path::new(&install_dir).join(cmd);
        rom.to_string_lossy().to_string()
    };

    // 5. Run the async hash check
    let result = remote::retroarch::check_rom_hash(
        std::path::Path::new(&emu_install_dir),
        &game_id,
        &rom_path,
    )
    .await;

    serde_json::to_value(&result).map_err(|e| format!("Serialization error: {e}"))
}
