use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::nonpoison::Mutex;

use bitcode::{Decode, Encode};
use database::{
    DownloadType, DownloadableMetadata, GameDownloadStatus, borrow_db_checked,
    borrow_db_mut_checked,
    models::data::{GameVersion, InstalledGameType, UserConfiguration}, platform::Platform,
};
use games::{
    collections::collection::Collection,
    downloads::error::LibraryError,
    exe_scan::{self, ExecutableCandidate},
    downloads::mod_agent::mods_changed_event,
    downloads::mod_data::{self, MODS_DIR},
    library::{
        FetchGameStruct, Game, clear_mod_install_state, get_current_meta, push_game_update,
        uninstall_game_logic,
    },
    state::{GameStatusManager, GameStatusWithTransient},
};
use log::{info, warn};
use utils::app_emit;
use process::PROCESS_MANAGER;
use process::parser::ParsedCommand;
use remote::{
    auth::generate_authorization_header,
    cache::{cache_object, cache_object_db, get_cached_object},
    error::{DropServerError, RemoteAccessError},
    offline,
    requests::{generate_url, remote_request, RemoteRequest},
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
    // Kept for signature parity with fetch_library_logic_offline (the offline!
    // macro passes it to both). No longer used here — it previously drove the
    // uninstall-on-cache-miss data-loss bug, now removed.
    _app_handle: AppHandle,
    hard_refresh: Option<bool>,
) -> Result<FetchLibraryResponse, RemoteAccessError> {
    let do_hard_refresh = hard_refresh.unwrap_or(false);
    if !do_hard_refresh && let Ok(library) = get_cached_object("library") {
        return Ok(library);
    }

    let response = generate_url(&["/api/v1/client/user/library"], &[])?;
    let auth_header = generate_authorization_header()?;
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

    // The write guard's Drop re-encrypts and rewrites the ENTIRE database to
    // disk before it releases, so taking it here made every library fetch a
    // full-DB write — and the sidebar fetches the library on every navigation.
    // Nothing below actually mutates the DB unless a game is new to the library,
    // so do the work under a read lock and only escalate when there is a status
    // row genuinely missing. `cache_object_db` writes its own files under
    // `cache_dir` and only borrows the database to find that path.
    let installed_metas = {
        let db_handle = borrow_db_checked();

        for game in &all_games {
            cache_object_db(&format!("game/{}", game.id), game, &db_handle)?;
        }

        let unseeded: Vec<String> = all_games
            .iter()
            .filter(|game| !db_handle.applications.game_statuses.contains_key(game.id()))
            .map(|game| game.id().clone())
            .collect();

        // Seeding only ever adds `game_statuses` rows, so reading the installed
        // versions before it is equivalent to reading them after.
        let installed_metas = db_handle
            .applications
            .installed_game_version
            .values()
            .cloned()
            .collect::<Vec<DownloadableMetadata>>();

        // Released before the write guard is taken: RwLock has no upgrade.
        drop(db_handle);

        if !unseeded.is_empty() {
            let mut db_handle = borrow_db_mut_checked();
            for id in unseeded {
                db_handle
                    .applications
                    .game_statuses
                    .entry(id)
                    .or_insert(GameDownloadStatus::Remote {});
            }
        }

        installed_metas
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
        // Metadata is cached under "game/{id}" (see the bulk-cache write above
        // and fetch_game_logic) — read the SAME key. A bare-id read here always
        // missed, which sent installed-but-delisted games into the delete path
        // below.
        let game = match get_cached_object::<Game>(&format!("game/{}", meta.id)) {
            Ok(game) => game,
            Err(err) => {
                // Metadata cache miss (e.g. a disk-scanned game never in the
                // server library and never opened). We can't render it without
                // its Game object — but DO NOT uninstall it. Deleting a real
                // on-disk install because a metadata lookup missed is
                // catastrophic data loss (this used to run remove_dir_all here).
                // Skip it for this list; it stays installed and reappears once
                // its metadata is cached.
                warn!(
                    "{} is installed but its metadata isn't cached ({err}); \
                     skipping it in the library list (NOT uninstalling).",
                    meta.id
                );
                continue;
            }
        };
        match game.game_type.as_str() {
            "Game" => missing.push(game),
            // Mods are managed on their parent game's page (store/library "Mods"
            // sections), never as standalone library tiles — and a mod's
            // install_dir is the PARENT's dir, so surfacing it here would also
            // offer a generic uninstall that deletes the base game.
            "Mod" => {}
            _ => other.push(game),
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
                .header("Authorization", generate_authorization_header()?)
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

    // Reading a game's status must not rewrite the whole DB. The write guard's
    // Drop re-encrypts and serializes the ENTIRE database to disk, so taking it
    // here — once per game — is what turned loading an 82-game library into 82
    // full-DB writes. The status row almost always already exists (fetch_library
    // seeds every library game as Remote), so take a read lock to check and only
    // fall back to a write lock to create a genuinely missing one.
    let status = {
        let db_handle = borrow_db_checked();
        if db_handle.applications.game_statuses.contains_key(&id) {
            GameStatusManager::fetch_state(&id, &db_handle)
        } else {
            drop(db_handle);
            let mut db_handle = borrow_db_mut_checked();
            db_handle
                .applications
                .game_statuses
                .entry(id.clone())
                .or_insert(GameDownloadStatus::Remote {});
            GameStatusManager::fetch_state(&id, &db_handle)
        }
    };

    let data = FetchGameStruct::new(game.clone(), status, version);

    cache_object(&format!("game/{}", id), &game)?;

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
struct VersionDownloadOptionRequiredMod {
    game_id: String,
    version_id: String,
    name: String,
    icon_object_id: String,
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
    // Mod placement (type=Mod versions). MUST be listed here or serde drops the
    // server's values when this struct is deserialized + re-serialized to the
    // frontend, so the mod would overlay at the install root. #[serde(default)]
    // keeps non-mod / older-server responses (which omit them) deserialising.
    #[serde(default)]
    pub mod_install_dir: String,
    #[serde(default)]
    pub launch_override: Option<String>,
    // Mod prerequisites (type=Mod versions): other mods this one requires. Same
    // serde(default) pass-through reasoning as the mod placement fields above.
    #[serde(default)]
    required_mods: Vec<VersionDownloadOptionRequiredMod>,
    // The version's current revision (in-place updates). Absent on servers
    // from before revisions; passed through to the frontend as `revision`.
    #[serde(default)]
    pub revision: Option<u32>,
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
    let previous_id = borrow_db_checked()
        .applications
        .installed_game_version
        .get(&game_id)
        .map(|v| v.version.clone());

    let url = generate_url(
        &["/api/v1/client/game", &game_id, "versions"],
        &[("previous", &previous_id.unwrap_or_default())],
    )?;
    // Route through the retrying helper. A cold size computation on the server
    // can exceed the client's 15s timeout on the first hit, but the server
    // caches the result, so an automatic retry lands on the warm path. This used
    // to be a single no-retry request, which is why installs needed manual
    // re-clicks to eventually succeed.
    let data: Vec<VersionDownloadOption> = remote_request(RemoteRequest::get(url)).await?;

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
    let game = get_cached_object::<Game>(&format!("game/{}", id))?;

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

/// One installed version of a game, for the multi-version install list on the
/// game page. The frontend maps `versionId` to a display name via the version
/// options it already fetches.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    pub version_id: String,
    /// "Installed" | "SetupRequired" | "PartiallyInstalled".
    pub install_type: String,
    pub update_available: bool,
}

/// Every installed version of a game (any state), so the UI can show them side
/// by side and launch/uninstall each. Empty when nothing is installed.
#[tauri::command]
pub fn fetch_game_installs(game_id: String) -> Vec<InstallInfo> {
    let db = borrow_db_checked();
    let mut installs: Vec<InstallInfo> = db
        .applications
        .installs_for_game(&game_id)
        .into_iter()
        .map(|r| InstallInfo {
            version_id: r.version_id.clone(),
            install_type: match &r.install_type {
                InstalledGameType::Installed => "Installed",
                InstalledGameType::SetupRequired => "SetupRequired",
                InstalledGameType::PartiallyInstalled { .. } => "PartiallyInstalled",
            }
            .to_string(),
            update_available: r.update_available,
        })
        .collect();
    installs.sort_by(|a, b| a.version_id.cmp(&b.version_id));
    installs
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
pub fn uninstall_game(
    game_id: String,
    // Which installed version to remove; omit to remove the game's current install.
    version: Option<String>,
    app_handle: AppHandle,
) -> Result<(), LibraryError> {
    let meta = match &version {
        Some(v) => {
            let db = borrow_db_checked();
            let install = db
                .applications
                .get_install(&game_id, v)
                .ok_or_else(|| LibraryError::MetaNotFound(game_id.clone()))?;
            DownloadableMetadata::new(
                game_id.clone(),
                install.version_id.clone(),
                install.target_platform,
                DownloadType::Game,
            )
        }
        None => match get_current_meta(&game_id) {
            Some(data) => data,
            None => return Err(LibraryError::MetaNotFound(game_id)),
        },
    };

    // A mod's install_dir is the PARENT game's directory, so uninstall_game_logic
    // would remove_dir_all() it and wipe the base game. Mods must go through
    // uninstall_mod, which removes only the mod's own overlay files.
    if meta.download_type == DownloadType::Mod {
        warn!("refusing to uninstall mod {game_id} as a game; use uninstall_mod");
        return Err(LibraryError::IsMod(game_id));
    }

    // An in-place update stages into and swaps files inside the install
    // folder; deleting it underneath would fail the update half-way.
    if games::downloads::update::update_active(&game_id) {
        warn!("refusing to uninstall {game_id} while an update of it is queued or running");
        return Err(LibraryError::GameBusy);
    }

    uninstall_game_logic(meta, &app_handle);

    Ok(())
}

/// The parent game's install directory. A mod overlays into it, and every mod's
/// ledger lives under `<install dir>/.mods/`. Both mod commands anchor on this
/// rather than the mod's own status, so mod state stays tied to the parent.
fn parent_install_dir(parent_game_id: &str) -> Result<PathBuf, LibraryError> {
    let db = borrow_db_checked();
    match db.applications.game_statuses.get(parent_game_id) {
        Some(GameDownloadStatus::Installed { install_dir, .. }) => Ok(PathBuf::from(install_dir)),
        _ => Err(LibraryError::MetaNotFound(parent_game_id.to_string())),
    }
}

/// Uninstall a mod, or remove what an unfinished install left behind: put back
/// every base-game file it overwrote, delete the files it added (leaving any
/// another mod still lists), delete its ledger, and reset its status. Refused
/// while the mod is downloading. If some files cannot be handled, the error
/// says so and the mod stays listed so the user can try again.
#[tauri::command]
pub fn uninstall_mod(
    mod_game_id: String,
    parent_game_id: String,
    app_handle: AppHandle,
) -> Result<(), LibraryError> {
    let parent_dir = parent_install_dir(&parent_game_id)?;
    // Mods on this game, by their ledgers. Read before the database lock.
    let siblings: Vec<String> = mod_data::installed_ledgers(&parent_dir)
        .map(|l| l.into_iter().map(|m| m.game_id).collect())
        .unwrap_or_default();
    {
        let db = borrow_db_checked();
        let busy = |id: &str| db.applications.transient_statuses.keys().any(|k| k.id == id);
        if busy(&mod_game_id) {
            warn!("refusing to uninstall mod {mod_game_id}: it is downloading");
            return Err(LibraryError::ModBusy);
        }
        // While a base-game or another mod's download runs (or waits in the
        // queue), which files are whose is changing under us, and a running
        // game has its files open. A stopped, failed or partial download is
        // not refused: the base game hands files back chunk by chunk before
        // recording each one, so the files a mod lists are still its own, and
        // removing a mod must stay possible whatever state the game is in.
        if busy(&parent_game_id) || siblings.iter().any(|id| busy(id)) {
            warn!("refusing to uninstall mod {mod_game_id}: game {parent_game_id} or one of its mods is busy");
            return Err(LibraryError::GameBusy);
        }
    }

    let result = mod_data::remove_mod(&parent_dir, &mod_game_id);
    app_emit!(&app_handle, &mods_changed_event(&parent_game_id), ());
    let outcome = result.map_err(|why| {
        warn!("uninstall of mod {mod_game_id} incomplete: {why}");
        LibraryError::ModFiles(why)
    })?;
    info!(
        "uninstalled mod {mod_game_id}: {} file(s) removed, {} original(s) restored, {} left to other mods",
        outcome.removed, outcome.restored, outcome.kept_for_other_mods
    );

    let mut db = borrow_db_mut_checked();
    clear_mod_install_state(&mut db, &mod_game_id);
    push_game_update(
        &app_handle,
        &mod_game_id,
        None,
        GameStatusManager::fetch_state(&mod_game_id, &db),
    );
    drop(db);
    app_emit!(&app_handle, "update_library", ());
    Ok(())
}

/// A mod found on the base game's disk, as surfaced to the client UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub game_id: String,
    pub version: String,
    pub file_count: usize,
    /// The download finished. False for a cancelled, failed or running one,
    /// which the UI offers to resume or remove instead of calling installed.
    pub complete: bool,
    /// A download of this mod is queued or running right now.
    pub downloading: bool,
    /// What `download_mod` needs to resume this exact install.
    pub platform: Platform,
    pub mod_install_dir: String,
    pub launch_override: Option<String>,
    /// Set when the mod has a launch override that a launch of the game, as
    /// it is installed now, would not use: "platform" (the mod has its own
    /// launch settings, none for the game's platform), "unfinished", or
    /// "otherMod" (then `launch_override_winner` names the mod whose is used).
    pub launch_override_skipped: Option<String>,
    pub launch_override_winner: Option<String>,
}

/// The platforms a mod version (by version id) has its own launch or setup
/// settings for, from the version recorded when it was installed. None when
/// that version was never recorded. See `mod_data::decide_launch_override`.
pub fn mod_version_platforms(db: &database::Database, version_id: &str) -> Option<Vec<Platform>> {
    db.applications.game_versions.get(version_id).map(|v| {
        v.launches
            .iter()
            .map(|l| l.platform)
            .chain(v.setups.iter().map(|s| s.platform))
            .collect()
    })
}

/// List the mods on a base game, finished or not, read from the `.moddata`
/// ledgers under the parent's install dir (the source of truth for what is on
/// disk). Errors when the folder exists but cannot be read, so the UI can tell
/// "no mods" from "could not look".
#[tauri::command]
pub fn list_installed_mods(parent_game_id: String) -> Result<Vec<InstalledMod>, LibraryError> {
    let parent_dir = parent_install_dir(&parent_game_id)?;
    let ledgers = mod_data::installed_ledgers(&parent_dir).map_err(|e| {
        warn!(
            "list_installed_mods: cannot read {} ({e})",
            parent_dir.join(MODS_DIR).display()
        );
        LibraryError::ModFiles(format!("could not read the mods folder: {e}"))
    })?;

    let db = borrow_db_checked();
    // Which overrides a launch of the game as installed now would skip, so the
    // Mods tab can say so instead of it only showing up in the log.
    let skipped: HashMap<String, mod_data::OverrideSkip> = db
        .applications
        .installed_game_version
        .get(&parent_game_id)
        .map(|parent| parent.target_platform)
        .and_then(|platform| {
            mod_data::decide_launch_override(&parent_dir, platform, &|v: &str| mod_version_platforms(&db, v))
                .map_err(|e| warn!("list_installed_mods: cannot check launch overrides: {e}"))
                .ok()
        })
        .map(|d| d.skipped.into_iter().map(|(id, _, why)| (id, why)).collect())
        .unwrap_or_default();
    let mods: Vec<InstalledMod> = ledgers
        .into_iter()
        .map(|m| InstalledMod {
            launch_override_skipped: skipped.get(&m.game_id).map(|why| {
                match why {
                    mod_data::OverrideSkip::Platform => "platform",
                    mod_data::OverrideSkip::Unfinished => "unfinished",
                    mod_data::OverrideSkip::OtherMod(_) => "otherMod",
                }
                .to_string()
            }),
            launch_override_winner: match skipped.get(&m.game_id) {
                Some(mod_data::OverrideSkip::OtherMod(winner)) => Some(winner.clone()),
                _ => None,
            },
            complete: mod_data::ledger_is_complete(&parent_dir, &m),
            downloading: db
                .applications
                .transient_statuses
                .keys()
                .any(|k| k.id == m.game_id),
            mod_install_dir: mod_data::overlay_rel(&parent_dir, &m).unwrap_or_default(),
            file_count: m.get_installed_files().len(),
            game_id: m.game_id,
            version: m.game_version,
            platform: m.target_platform,
            launch_override: m.launch_override,
        })
        .collect();
    info!(
        "list_installed_mods: {} mod(s) on parent {parent_game_id}, {} unfinished",
        mods.len(),
        mods.iter().filter(|m| !m.complete).count()
    );
    Ok(mods)
}

/// The platform the base game is installed for, so a mod is installed for the
/// same one when the server offers it (a mod version with its own launch
/// settings only applies its launch override to launches of those platforms).
/// None when the base game is not installed.
#[tauri::command]
pub fn mod_parent_platform(parent_game_id: String) -> Option<Platform> {
    borrow_db_checked()
        .applications
        .installed_game_version
        .get(&parent_game_id)
        .map(|m| m.target_platform)
}

#[tauri::command]
pub async fn fetch_game_version_options(
    game_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<VersionDownloadOption>, RemoteAccessError> {
    fetch_game_version_options_logic(game_id, state).await
}

/// A prerequisite mod (name + id), for the "Requires: X" hint on the mods list.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRequirement {
    pub game_id: String,
    pub name: String,
}

/// A mod available for a base game, as listed by the server's
/// `/client/game/{id}/mods` endpoint.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModListing {
    pub id: String,
    pub m_name: String,
    pub m_short_description: String,
    pub m_icon_object_id: String,
    // Prerequisite mods this mod needs (from its latest version). serde(default)
    // keeps older-server responses (which omit it) deserialising.
    #[serde(default)]
    pub required_mods: Vec<ModRequirement>,
    // The mod's newest version id, to offer Update when the installed ledger
    // is older. None from older servers, which simply never offer Update.
    #[serde(default)]
    pub latest_version_id: Option<String>,
}

/// List the mods available for a base game. Fetched via a command (not a raw
/// `server://` fetch) because the `/client/*` endpoints require the JWT client
/// auth header that `generate_authorization_header` adds — a browser fetch
/// through the server protocol arrives unauthenticated and 403s.
#[tauri::command]
pub async fn fetch_game_mods(game_id: String) -> Result<Vec<ModListing>, RemoteAccessError> {
    let client = DROP_CLIENT_ASYNC.clone();
    let url = generate_url(&["/api/v1/client/game", &game_id, "mods"], &[])?;
    let response = client
        .get(url)
        .header("Authorization", generate_authorization_header()?)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(RemoteAccessError::InvalidResponse(response.json().await?));
    }

    let data: Vec<ModListing> = response.json().await?;
    Ok(data)
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

/// Open an installed game's folder in the OS file manager. Game-id-keyed
/// (resolving the path server-side from `game_statuses`) rather than taking a
/// path, mirroring `open_download_dir` / `open_process_logs` — the codebase
/// deliberately keeps arbitrary-path openers off the IPC surface.
#[tauri::command]
pub fn open_game_install_dir(game_id: String, app_handle: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    let install_dir = {
        let db_lock = borrow_db_checked();
        match db_lock.applications.game_statuses.get(&game_id) {
            Some(GameDownloadStatus::Installed { install_dir, .. }) => install_dir.clone(),
            _ => return Err("Game is not installed.".to_string()),
        }
    };

    app_handle
        .opener()
        .open_path(install_dir, None::<&str>)
        .map_err(|e| format!("Failed to open install folder: {e}"))
}

#[tauri::command]
pub fn update_game_configuration(
    game_id: String,
    options: UserConfiguration,
) -> Result<(), LibraryError> {
    let mut handle = borrow_db_mut_checked();
    let version = handle
        .applications
        .installed_game_version
        .get(&game_id)
        .ok_or_else(|| LibraryError::MetaNotFound(game_id.clone()))?
        .version
        .clone();

    // A game imported by a disk scan has an `installed_game_version` but no
    // cached `GameVersion` yet (that map is only filled by a download or an
    // online library sync), so there is nothing to update — a plain `get` here
    // used to fail the save with `MetaNotFound`, which is why scanned ROMs were
    // locked out of the quality / CRT / aspect settings. Synthesize a minimal
    // entry from the installed metadata instead: emulated games launch through
    // the emulator (not these launches/setups), so the empty vecs are inert, and
    // an online library sync later replaces this with the full server version.
    let mut configuration = handle
        .applications
        .game_versions
        .get(&version)
        .cloned()
        .unwrap_or_else(|| GameVersion {
            game_id: game_id.clone(),
            version_id: version.clone(),
            display_name: None,
            version_path: String::new(),
            only_setup: false,
            version_index: 0,
            delta: false,
            user_configuration: UserConfiguration::default(),
            launches: Vec::new(),
            setups: Vec::new(),
        });

    configuration.user_configuration = options;

    handle
        .applications
        .game_versions
        .insert(version, configuration);

    Ok(())
}

/// What the "which executable does this game run" picker needs.
///
/// `unsupportedReason` is a code, not a sentence: the wording the user reads
/// lives in the two UI surfaces, so it can be edited without a rebuild of the
/// Rust side and stays consistent between them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableScanResult {
    pub supported: bool,
    /// One of `notInstalled`, `emulated`, `noVersionData` when unsupported.
    pub unsupported_reason: Option<&'static str>,
    /// The path the server's launch config runs, relative to the install dir.
    /// `None` when the command could not be tokenised.
    pub automatic: Option<String>,
    /// The override currently saved for this device, if any.
    pub selected: Option<String>,
    pub candidates: Vec<ExecutableCandidate>,
}

impl ExecutableScanResult {
    fn unsupported(reason: &'static str) -> Self {
        Self {
            supported: false,
            unsupported_reason: Some(reason),
            automatic: None,
            selected: None,
            candidates: Vec::new(),
        }
    }
}

/// List the executables inside a game's install directory so the user can pick
/// which one to launch.
///
/// The install directory is resolved from the database by game id — the caller
/// never supplies a path, so the webview cannot aim this at an arbitrary
/// directory. The walk itself runs on a blocking thread; a large install would
/// otherwise stall the async runtime while the UI waits.
#[tauri::command]
pub async fn scan_game_executables(game_id: String) -> ExecutableScanResult {
    struct ScanInputs {
        install_dir: PathBuf,
        target_platform: Platform,
        automatic: Option<String>,
        selected: Option<String>,
    }

    let inputs = {
        let db = borrow_db_checked();
        let (install_dir, version_id) = match db.applications.game_statuses.get(&game_id) {
            Some(GameDownloadStatus::Installed {
                install_dir,
                version_id,
                install_type: InstalledGameType::Installed,
                ..
            }) => (PathBuf::from(install_dir), version_id.clone()),
            _ => return ExecutableScanResult::unsupported("notInstalled"),
        };
        let Some(meta) = db.applications.installed_game_version.get(&game_id) else {
            return ExecutableScanResult::unsupported("notInstalled");
        };
        let target_platform = meta.target_platform;

        // A game imported by a disk scan has no cached GameVersion, so there is
        // no launch config to compare against and no per-game configuration to
        // save the pick into. Those are the emulated ROM imports in practice.
        let Some(version) = db.applications.game_versions.get(&version_id) else {
            return ExecutableScanResult::unsupported("noVersionData");
        };

        let launch = version
            .launches
            .iter()
            .find(|v| v.platform == target_platform);

        // An emulated game launches the emulator's binary, not anything inside
        // this install, so swapping the executable here would be meaningless.
        if launch.is_some_and(|l| l.emulator.as_ref().is_some_and(|e| !e.game_id.is_empty())) {
            return ExecutableScanResult::unsupported("emulated");
        }

        ScanInputs {
            install_dir,
            target_platform,
            automatic: launch
                .and_then(|l| ParsedCommand::parse(l.command.clone()).ok())
                .map(|p| p.command),
            selected: version.user_configuration.executable_override.clone(),
        }
    };

    let ScanInputs {
        install_dir,
        target_platform,
        automatic,
        selected,
    } = inputs;

    // The entry marked "in use" is the override when one is set, otherwise the
    // server's automatic pick.
    let current = selected.clone().or_else(|| automatic.clone());
    let scan_dir = install_dir.clone();
    let candidates = tokio::task::spawn_blocking(move || {
        exe_scan::scan_executables(&scan_dir, target_platform, current.as_deref())
    })
    .await
    .unwrap_or_default();

    ExecutableScanResult {
        supported: true,
        unsupported_reason: None,
        automatic,
        selected,
        candidates,
    }
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

/// Find the install root of the emulator that runs `game_id`.
///
/// Preference order:
///   1. An installed title that already has this user's `drop-saves/{user_id}/
///      {game_id}` on disk. That directory only exists once Drop has patched a
///      RetroArch config, so it is the strongest signal but far from universal.
///   2. `game_id` itself, if it's installed and has its own `drop-saves`.
///   3. The emulator named by the game's launch config.
///
/// Steps 1 and 2 ask [`remote::save_sync::resolve_emu_saves_root`] for the
/// path rather than joining it here, so discovery cannot drift from the writer
/// and start reporting "no saves" for a game that is saving perfectly well —
/// including in the window where the per-user move has not finished and the
/// saves are still in the legacy directory.
///
/// Step 3 is what makes Switch titles work at all. Their saves live in the
/// emulator's NAND under `user/`, never in `drop-saves`, so steps 1 and 2 find
/// nothing and every save-related command used to bail out with "Save
/// directory not found" — the panel showed zero local saves for exactly the
/// games whose saves are hardest to recover by hand.
fn find_emulator_root(game_id: &str) -> Option<std::path::PathBuf> {
    let db = borrow_db_checked();
    let user_id = remote::save_sync::current_user_id();
    let saves_dir = |root: &Path| -> std::path::PathBuf {
        remote::save_sync::resolve_emu_saves_root(root, user_id.as_deref(), game_id)
    };

    let installed_dir = |id: &str| -> Option<std::path::PathBuf> {
        match db.applications.game_statuses.get(id) {
            Some(GameDownloadStatus::Installed { install_dir, .. }) => {
                Some(std::path::PathBuf::from(install_dir))
            }
            _ => None,
        }
    };

    for (_id, status) in db.applications.game_statuses.iter() {
        if let GameDownloadStatus::Installed { install_dir, .. } = status {
            let root = Path::new(install_dir);
            if saves_dir(root).exists() {
                return Some(root.to_path_buf());
            }
        }
    }

    if let Some(root) = installed_dir(game_id)
        && saves_dir(&root).exists()
    {
        return Some(root);
    }

    // Fall back to the emulator this game is associated with, whether or not
    // it has ever written a `drop-saves` directory.
    //
    // `game_versions` is keyed by VERSION id, not game id, so looking it up
    // with the game id never matched and this whole step was dead — which is
    // why Switch titles still bailed out with "Save directory not found".
    // Resolve the installed version first, the same way `find_switch_title_id`
    // does.
    let version = &db.applications.installed_game_version.get(game_id)?.version;
    db.applications
        .game_versions
        .get(version)
        .and_then(|gv| gv.launches.iter().find_map(|l| l.emulator.as_ref()))
        .and_then(|emu| installed_dir(&emu.game_id))
}

/// Where Drop keeps a game's RetroArch-style saves: `{emu_root}/drop-saves/
/// {user_id}/{game_id}`. May not exist yet — callers that only read handle that
/// fine, and the ones that write create it.
fn find_emulator_saves_dir(game_id: &str) -> Option<std::path::PathBuf> {
    let user_id = remote::save_sync::current_user_id();
    find_emulator_root(game_id).map(|root| {
        remote::save_sync::resolve_emu_saves_root(&root, user_id.as_deref(), game_id)
    })
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
///
/// This is the target of every non-PC restore in the Cloud Saves panel and of
/// every "Keep cloud" answer to a conflict, so it is the single most
/// destructive command in the client. It delegates the write to
/// `remote::save_sync::write_downloaded_save` rather than doing its own
/// `fs::write`, which gets two things the hand-rolled version never had:
///
///   * a checked, timestamped backup of whatever it is about to replace, and
///   * `switch__` filename decoding. Without it a Switch save was written out
///     as one literal junk filename in `drop-saves/`, the real NAND save was
///     left untouched, and the panel then reported the row as Synced.
///
/// Refuses without a signed-in account, like every other sync path. Downloads
/// keep working while the identity is gone — `auth` lives in the encrypted
/// database and the user object lives in the on-disk cache, so one can be
/// readable while the other is not — and without this guard the restored bytes
/// landed in the shared legacy tree that the scanner never reads and that
/// another account can later adopt.
#[tauri::command]
pub fn write_save_file(
    game_id: String,
    filename: String,
    save_type: String,
    data: String,
) -> Result<(), String> {
    let user_id = remote::save_sync::current_user_id()
        .ok_or_else(|| "Sign in to restore saves.".to_string())?;
    let emu_root = find_emulator_root(&game_id)
        .ok_or_else(|| "Save directory not found".to_string())?;

    if !matches!(save_type.as_str(), "save" | "state") {
        return Err("Invalid save type".to_string());
    }

    // Security check: filename must be a plain leaf name with no traversal.
    // Switch names survive this — their separators are percent-encoded — and
    // `decode_switch_relpath` re-checks the decoded path for escapes.
    if filename.is_empty()
        || filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
    {
        return Err("Invalid filename".to_string());
    }

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    // No expected hash: this is a user-initiated restore of bytes the panel
    // already fetched, so it always writes (and always backs up first).
    remote::save_sync::write_downloaded_save(
        &emu_root,
        Some(&user_id),
        &game_id,
        &filename,
        &save_type,
        &bytes,
        None,
    )?;
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

/// List cloud saves for a game (current user). Routes through Rust so the
/// JWT/cert auth — the only auth `defineClientEventHandler` accepts — is
/// used; the `server://` Tauri protocol injects a `Bearer <web_token>`
/// instead, which those endpoints 403.
#[tauri::command]
pub async fn list_cloud_saves(
    game_id: String,
) -> Result<Vec<remote::save_sync::CloudSaveMeta>, String> {
    remote::save_sync::list_cloud_saves(&game_id)
        .await
        .map_err(|e| e.to_string())
}

/// One summary row per game the signed-in user has cloud saves for: file
/// count, size, and when it last reached the server.
///
/// `ownCount` / `ownBytes` are what any surface claiming "your saves are
/// backed up" counts. On a server that reads saves per account they equal
/// `fileCount` / `totalBytes`; an older server that shared PC saves across
/// accounts could report less.
///
/// Same auth reasoning as [`list_cloud_saves`]. This is what makes "are my
/// saves backed up" answerable for a whole library in one request, instead of
/// one collapsed panel per game each gated behind a Ludusavi scan.
#[tauri::command]
pub async fn list_cloud_save_summaries()
-> Result<Vec<remote::save_sync::CloudSaveGameSummary>, String> {
    remote::save_sync::list_cloud_save_summaries()
        .await
        .map_err(|e| e.to_string())
}

/// The signed-in user's cloud-save storage usage and cap.
///
/// The server has enforced this cap since the feature shipped; nothing in
/// either repo read it, so the only way to find out it existed was to have an
/// upload rejected.
#[tauri::command]
pub async fn cloud_save_quota() -> Result<remote::save_sync::CloudSaveQuota, String> {
    remote::save_sync::fetch_quota()
        .await
        .map_err(|e| e.to_string())
}

/// Download one cloud save by id; return the decoded bytes as base64 so
/// the frontend can forward them to `write_save_file` / `write_pc_save_file`
/// without a second round-trip.
#[tauri::command]
pub async fn download_cloud_save(id: String) -> Result<String, String> {
    let bytes = remote::save_sync::download_cloud_save(&id)
        .await
        .map_err(|e| e.to_string())?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Soft-delete one cloud save by id. Server records a tombstone keyed on
/// the deleting device so other clients delete their local copy on next
/// sync.
///
/// Returns whether the caller had a copy to delete. Only an older server, which
/// shared PC saves across accounts, can answer `false`.
#[tauri::command]
pub async fn delete_cloud_save(id: String) -> Result<bool, String> {
    remote::save_sync::delete_cloud_save(&id)
        .await
        .map_err(|e| e.to_string())
}

/// The previous versions the server kept of one cloud save (newest first),
/// plus the live one. Through Rust for the same auth reason as
/// [`list_cloud_saves`].
#[tauri::command]
pub async fn list_cloud_save_revisions(
    id: String,
) -> Result<remote::save_sync::CloudSaveHistory, String> {
    remote::save_sync::list_cloud_save_revisions(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Make a previous version the live cloud copy. The cloud only: the panel
/// downloads it afterwards if the user wants it on this device too. The
/// version being replaced goes into the history, so this can be undone.
#[tauri::command]
pub async fn restore_cloud_save_revision(
    revision_id: String,
) -> Result<remote::save_sync::CloudSaveRestoreResult, String> {
    remote::save_sync::restore_cloud_save_revision(&revision_id)
        .await
        .map_err(|e| e.to_string())
}

/// Scan every local save file for a game — PC (Ludusavi, name-normalized and
/// manifest-tag-filtered) plus emulator (Drop's drop-saves dir, if the
/// emulator is installed). Pure read: no upload, no disk mutation. Shared
/// by the manual-sync command and the "show local saves" scan so both use
/// identical detection.
///
/// The Ludusavi half uses the same context as the launch (Wine prefix and
/// Steam app id, see `remote::save_sync::pc_scan_context`). It used to pass no
/// prefix, so for a Proton game the panel looked somewhere other than where
/// the launch did and listed its saves as missing.
///
/// `Err` when Ludusavi is installed and the scan failed. That is not the same
/// as "no saves": a caller showing an empty list for it would tell the user
/// their saves are gone, and Sync would then pull every cloud copy down over
/// files the scan could not see. A missing Ludusavi is `Ok` (PC half empty);
/// the panels prompt for the install separately.
fn scan_all_local_saves(
    game_id: &str,
    game_name: &str,
) -> Result<Vec<remote::save_sync::LocalSaveFile>, String> {
    let mut out = Vec::new();

    let scan = remote::save_sync::pc_scan_context(game_id, None);
    match remote::save_sync::scan_pc_saves(
        game_name,
        scan.steam_app_id.as_deref(),
        scan.wine_prefix.as_deref(),
    ) {
        Ok(found) => out.extend(found),
        Err(remote::save_sync::PcScanError::LudusaviMissing) => {}
        Err(remote::save_sync::PcScanError::Failed(reason)) => return Err(reason),
    }

    // `scan_emu_saves` walks both `drop-saves/<user_id>/<game_id>` and the
    // Switch NAND roots, so it needs the emulator install root, not the saves
    // subdir.
    if let Some(emu_root) = find_emulator_root(game_id) {
        out.extend(remote::save_sync::scan_emu_saves(
            &emu_root,
            remote::save_sync::current_user_id().as_deref(),
            game_id,
            find_switch_title_id(game_id).as_deref(),
        ));
    }

    Ok(out)
}

/// Best-effort Switch title id for a game, read out of its launch command or
/// install directory (a dump usually carries the id in its filename).
///
/// Mirrors what the launch path passes to `scan_emu_saves`. Without it the
/// NAND is skipped, which is deliberate: the scan is filed under one gameId,
/// and the NAND is shared by every installed Switch title.
fn find_switch_title_id(game_id: &str) -> Option<String> {
    let db = borrow_db_checked();
    let version = &db.applications.installed_game_version.get(game_id)?.version;
    let from_launch = db
        .applications
        .game_versions
        .get(version)
        .and_then(|gv| {
            gv.launches
                .iter()
                .find_map(|l| remote::save_sync::switch_title_id_from_path(&l.command))
        });
    if from_launch.is_some() {
        return from_launch;
    }
    match db.applications.game_statuses.get(game_id) {
        Some(GameDownloadStatus::Installed { install_dir, .. }) => {
            remote::save_sync::switch_title_id_from_path(install_dir)
        }
        _ => None,
    }
}

/// One locally-detected save file, for the panel's unified status list.
///
/// `data_hash` lets the panel compare a local file against its cloud
/// counterpart (matched by `filename`). `synced_hash` / `synced_cloud_id` are
/// what this account's sync manifest recorded for the file at its last sync
/// (only from entries this build wrote, see `SyncFileEntry::three_way_base`),
/// which is the third point the panel needs to tell "only the cloud changed"
/// (download), "only this PC changed" (upload) and a real conflict apart. The
/// frontend applies the same rule as `remote::save_sync::three_way_verdict`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSaveEntry {
    pub filename: String,
    pub save_type: String,
    pub size: u64,
    pub modified_at: u64,
    pub data_hash: String,
    pub synced_hash: Option<String>,
    pub synced_cloud_id: Option<String>,
    /// Set when the file changed since this account's last sync and its bytes
    /// are what another Drop account on this device last synced: that
    /// account's name, or "" if unknown. Shown so the user knows whose copy
    /// the local file is.
    pub last_synced_by_other_account: Option<String>,
    /// Another Drop account also syncs this game on this device and this is
    /// a file they share (a PC save, the Switch NAND, or an emulator save still
    /// in the old shared folder). A local change can
    /// then be that person's progress, so the panels show it as a conflict
    /// rather than offering to back it up as this account's, and Sync does not
    /// back up a file only on this device: the same rule the launch sync
    /// applies.
    pub other_accounts_on_this_device: bool,
    /// The name an older build's upload of this file is stored under in the
    /// cloud, when that differs from `filename` (see
    /// `remote::save_sync::legacy_cloud_name`). The panel keeps such a row out
    /// of Sync's automatic download, as the launch sync does.
    pub legacy_cloud_name: Option<String>,
}

/// [`remote::save_sync::local_copy_last_synced_by`], asked only for a file
/// that changed since this account's last sync (the one case where the answer
/// changes what the panels show), so a large NAND scan does not read every
/// other account's manifest once per file.
fn other_account_copy(
    user_id: Option<&str>,
    game_id: &str,
    filename: &str,
    data_hash: &str,
    synced_hash: Option<&str>,
) -> Option<String> {
    let user_id = user_id?;
    let changed = synced_hash.is_some_and(|h| !h.eq_ignore_ascii_case(data_hash));
    if !changed {
        return None;
    }
    remote::save_sync::local_copy_last_synced_by(user_id, game_id, filename, data_hash)
}

/// Scan and return this game's local save files WITHOUT uploading — so the
/// Cloud Saves panel's refresh can show what's on disk (synced or not),
/// not just what's already in the cloud. Same detection as the manual
/// sync, so what shows here is exactly what "Sync now" would push.
#[tauri::command]
pub async fn scan_local_game_saves(
    game_id: String,
    game_name: String,
) -> Result<Vec<LocalSaveEntry>, String> {
    // Same multi-second Ludusavi scan as the manual sync — keep it off the
    // main thread so the panel's refresh never freezes the UI.
    tokio::task::spawn_blocking(move || -> Result<Vec<LocalSaveEntry>, String> {
        let user_id = remote::save_sync::current_user_id();
        let manifest = user_id
            .as_ref()
            .map(|user_id| remote::save_sync::load_manifest(user_id, &game_id));
        let shared = user_id
            .as_deref()
            .is_some_and(|u| remote::save_sync::other_accounts_have_synced(u, &game_id));
        // Emulator saves are shared too while they still sit in the old
        // shared folder; see `remote::save_sync::emu_saves_root_is_shared`.
        let emu_root_shared = find_emulator_root(&game_id).is_some_and(|root| {
            remote::save_sync::emu_saves_root_is_shared(&root, user_id.as_deref(), &game_id)
        });
        let found = scan_all_local_saves(&game_id, &game_name)?;
        Ok(found
            .into_iter()
            .map(|f| {
                let recorded = manifest.as_ref().and_then(|m| m.files.get(&f.filename));
                // Only entries this build wrote are a usable last-sync record
                // for the panels' three-way state; see
                // `SyncFileEntry::three_way_base`. Any entry is good enough to
                // say whose copy the file is.
                let synced = recorded.and_then(|e| e.trusted_base());
                LocalSaveEntry {
                    other_accounts_on_this_device: shared
                        && remote::save_sync::is_shared_between_accounts(
                            &f.filename,
                            emu_root_shared,
                        ),
                    legacy_cloud_name: remote::save_sync::legacy_cloud_name(&f.filename),
                    last_synced_by_other_account: other_account_copy(
                        user_id.as_deref(),
                        &game_id,
                        &f.filename,
                        &f.data_hash,
                        recorded.map(|e| e.synced_hash.as_str()),
                    ),
                    synced_hash: synced.map(|e| e.synced_hash.clone()),
                    synced_cloud_id: synced.and_then(|e| e.cloud_id.clone()),
                    filename: f.filename,
                    save_type: f.save_type,
                    size: f.size,
                    modified_at: f.modified_at,
                    data_hash: f.data_hash,
                }
            })
            .collect())
    })
    .await
    .map_err(|e| format!("Local save scan task failed: {e}"))?
}

/// Whether Drop knows where a game keeps its saves at all.
///
/// Drop's coverage is narrower than the library: PC titles in Ludusavi's
/// catalogue, RetroArch (because Drop redirects its save directory), and the
/// yuzu-family Switch NAND. Everything else scans to an empty list, which is
/// indistinguishable from a game nobody has played yet — so the panel told
/// people to "play the game once" when playing it a hundred times would
/// produce nothing Drop can see.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCoverage {
    pub ludusavi_installed: bool,
    /// Ludusavi's catalogue has an entry for this game, so Drop knows where
    /// its PC saves live even before any exist.
    pub known_to_ludusavi: bool,
    /// The catalogue title the display name resolved to.
    pub canonical_title: Option<String>,
    /// This game launches through an emulator.
    pub emulated: bool,
    /// That emulator is one whose saves Drop can find.
    pub emulator_supported: bool,
}

/// The emulator install root this game launches through, if any, and whether
/// Drop can find saves inside it.
fn emulator_save_support(game_id: &str) -> (bool, bool) {
    let emulated = {
        let db = borrow_db_checked();
        db.applications
            .installed_game_version
            .get(game_id)
            .and_then(|meta| db.applications.game_versions.get(&meta.version))
            .is_some_and(|gv| gv.launches.iter().any(|l| l.emulator.is_some()))
    };
    if !emulated {
        return (false, false);
    }
    // RetroArch is covered because Drop points its save directory at
    // `drop-saves`; the yuzu family is covered because the NAND scan knows its
    // layout. A standalone Dolphin or PCSX2 writes where it likes and Drop
    // never sees it.
    let supported = find_emulator_root(game_id).is_some_and(|root| {
        remote::retroarch::discovery::is_retroarch(&root)
            || remote::switchemu::discovery::detect_switch_emulator(&root).is_some()
    });
    (true, supported)
}

#[tauri::command]
pub async fn game_save_coverage(
    game_id: String,
    game_name: String,
) -> Result<SaveCoverage, String> {
    // `resolve_canonical_title` shells out to Ludusavi twice in the worst
    // case, and it reads a ~9 MB catalogue. Not on the UI thread.
    tokio::task::spawn_blocking(move || {
        let steam_app_id = find_steam_app_id(&game_id);
        let pc = remote::save_sync::pc_save_coverage(&game_name, steam_app_id.as_deref());
        let (emulated, emulator_supported) = emulator_save_support(&game_id);
        SaveCoverage {
            ludusavi_installed: pc.ludusavi_installed,
            known_to_ludusavi: pc.known_to_ludusavi,
            canonical_title: pc.canonical_title,
            emulated,
            emulator_supported,
        }
    })
    .await
    .map_err(|e| format!("Save coverage check failed: {e}"))
}

/// Drop the saves that will not fit under the user's cloud-save quota, before
/// a single byte is read off disk or sent, and say which those were.
///
/// `Err` only when there is no room for anything: one oversized save must not
/// stop the small ones going up, because the server itself would have stored
/// them. See `remote::save_sync::preflight_quota` for what the check does and
/// does not promise.
async fn trim_to_quota(
    game_id: &str,
    targets: Vec<remote::save_sync::LocalSaveFile>,
) -> Result<(Vec<remote::save_sync::LocalSaveFile>, Option<String>), String> {
    // The plan borrows `targets`, so everything it has to say is pulled out
    // here and the borrow ends before the list itself is consumed.
    let (skipped, message, nothing_fits) = {
        let all: Vec<&remote::save_sync::LocalSaveFile> = targets.iter().collect();
        let plan = remote::save_sync::preflight_quota(game_id, &all).await;
        let nothing_fits = plan.nothing_fits();
        (plan.skipped, plan.message, nothing_fits)
    };

    if nothing_fits {
        return Err(message
            .unwrap_or_else(|| "Your cloud save storage is full.".to_string()));
    }
    if skipped.is_empty() {
        return Ok((targets, None));
    }
    let skipped: HashSet<String> = skipped.into_iter().collect();
    Ok((
        targets
            .into_iter()
            .filter(|f| !skipped.contains(&f.filename))
            .collect(),
        message,
    ))
}

/// Manually scan + upload this game's saves to the cloud, on demand —
/// independent of whether the game is installed or has ever been launched.
///
/// This is the escape hatch for the most common "no cloud saves" case: a
/// game was played before save-sync worked (or under a display name
/// Ludusavi's exact matcher couldn't resolve), so the only scan/upload
/// triggers — game launch and exit — never produced anything, and the
/// game may since have been uninstalled. The save files are still on disk;
/// this re-scans with the current (name-normalized, manifest-tag-filtered)
/// Ludusavi logic and pushes whatever real saves it finds.
///
/// `game_name` is the Drop display name; `scan_pc_saves` resolves it to
/// Ludusavi's canonical manifest title internally. Returns the number of
/// files uploaded. Mirrors the post-exit upload path (empty baseline =>
/// every detected file counts as new) and persists the synced manifest so
/// later diffs stay correct.
#[tauri::command]
pub async fn sync_game_saves_now(
    game_id: String,
    game_name: String,
) -> Result<usize, String> {
    if !borrow_db_checked().settings.cloud_saves_enabled {
        return Err("Cloud saves are disabled in settings.".to_string());
    }
    // The server keys every cloud row by user id, and so does the local
    // manifest. Without an identity this would push saves into whichever
    // account happens to be paired, so refuse rather than guess.
    let user_id = remote::save_sync::current_user_id()
        .ok_or_else(|| "Sign in to sync saves.".to_string())?;

    // Same detection the panel's refresh shows — PC (Ludusavi) + emulator.
    let scanned = tokio::task::spawn_blocking({
        let (game_id, game_name) = (game_id.clone(), game_name.clone());
        move || scan_all_local_saves(&game_id, &game_name)
    })
    .await
    .map_err(|e| format!("Local save scan task failed: {e}"))??;

    if scanned.is_empty() {
        return Ok(0);
    }

    // Whatever the quota has room for still goes up. The rest is named in the
    // log rather than dropped silently; this command returns a bare count, so
    // there is nowhere else to put it.
    let (current_saves, quota_message) = trim_to_quota(&game_id, scanned).await?;
    if let Some(message) = &quota_message {
        warn!("[SAVE-SYNC] Manual sync: {message}");
    }

    // Empty baseline => every detected file is treated as new and uploaded.
    let baseline = std::collections::HashMap::new();
    let (uploaded, failures) =
        remote::save_sync::upload_changed_saves(&game_id, &baseline, &current_saves)
            .await
            .map_err(|e| format!("Upload failed: {e}"))?;

    for err in &failures {
        warn!("[SAVE-SYNC] Manual sync upload error: {err}");
    }

    // Persist the synced state so subsequent launch/exit diffs are correct.
    // `record_synced_files` skips files the server rejected — those never
    // reached the cloud and would otherwise be treated as already synced from
    // here on — and stamps the rest with their real cloud id.
    let count = uploaded.len();
    let cloud_ids: std::collections::HashMap<String, String> = uploaded.into_iter().collect();
    let unsynced: Vec<String> = failures.iter().map(|f| f.filename.clone()).collect();
    let mut manifest = remote::save_sync::load_manifest(&user_id, &game_id);
    remote::save_sync::record_synced_files(
        &mut manifest,
        &current_saves,
        &cloud_ids,
        &unsynced,
    );
    if let Err(e) = remote::save_sync::save_manifest(&manifest) {
        warn!("[SAVE-SYNC] Manual sync: failed to persist manifest: {e}");
    }

    Ok(count)
}

/// Back up a specific set of local save files to the cloud, identified by
/// their namespaced filenames (e.g. `"pc__gen.sav"` or `"Game.srm"`).
///
/// Powers the unified panel's per-row "Back up", the "Keep this PC" side of
/// a conflict (pass one filename), and the header Sync's push half (pass all
/// the not-backed-up filenames). Pushing an explicit allowlist — rather than
/// a blanket "upload everything" — is what keeps Sync safe: files the user
/// didn't ask to push (notably the cloud side of a conflict) are never
/// silently overwritten.
///
/// Scans once, keeps only the requested filenames, uploads them in a single
/// bulk call, and records them in the manifest so later diffs are correct.
#[tauri::command]
pub async fn backup_saves(
    game_id: String,
    game_name: String,
    filenames: Vec<String>,
) -> Result<BackupResult, String> {
    if !borrow_db_checked().settings.cloud_saves_enabled {
        return Err("Cloud saves are disabled in settings.".to_string());
    }
    let user_id = remote::save_sync::current_user_id()
        .ok_or_else(|| "Sign in to sync saves.".to_string())?;
    if filenames.is_empty() {
        return Ok(BackupResult::default());
    }

    let wanted: std::collections::HashSet<&str> =
        filenames.iter().map(|s| s.as_str()).collect();
    let found = tokio::task::spawn_blocking({
        let (game_id, game_name) = (game_id.clone(), game_name.clone());
        move || scan_all_local_saves(&game_id, &game_name)
    })
    .await
    .map_err(|e| format!("Local save scan task failed: {e}"))??;
    let mut targets: Vec<remote::save_sync::LocalSaveFile> = found
        .iter()
        .filter(|f| wanted.contains(f.filename.as_str()))
        .cloned()
        .collect();
    // Fall back to matching on the basename. Big Picture builds its request
    // from Ludusavi's own listing, which is basename-keyed, so it asks for
    // `pc__save.dat` where the scan now reports `pc__slot1%2Fsave.dat`. Only
    // used when nothing matched exactly, so a game with one save per folder
    // still resolves to exactly one file.
    if targets.is_empty() {
        targets = found
            .iter()
            .filter(|f| {
                remote::save_sync::decode_pc_relpath(&f.filename)
                    .and_then(|rel| {
                        rel.file_name()
                            .map(|n| format!("pc__{}", n.to_string_lossy()))
                    })
                    .is_some_and(|basename| wanted.contains(basename.as_str()))
            })
            .cloned()
            .collect();
    }
    if targets.is_empty() {
        return Err(
            "None of those saves are on this device anymore — try refreshing.".to_string(),
        );
    }

    // Anything the quota has room for still goes up; the rest comes back as a
    // row error rather than taking the whole request down with it.
    let (targets, quota_message) = trim_to_quota(&game_id, targets).await?;

    // Empty baseline => every file we hand it is treated as new and uploaded.
    let baseline = std::collections::HashMap::new();
    let (uploaded, failures) =
        remote::save_sync::upload_changed_saves(&game_id, &baseline, &targets)
            .await
            .map_err(|e| format!("Upload failed: {e}"))?;
    for err in &failures {
        warn!("[SAVE-SYNC] Backup error: {err}");
    }

    // Record the synced state for each pushed file so later diffs are correct.
    // Files the server rejected are left out: they are not in the cloud, and
    // marking them synced would stop the next session from retrying them.
    let count = uploaded.len();
    let cloud_ids: std::collections::HashMap<String, String> = uploaded.into_iter().collect();
    let unsynced: Vec<String> = failures.iter().map(|f| f.filename.clone()).collect();
    let mut manifest = remote::save_sync::load_manifest(&user_id, &game_id);
    remote::save_sync::record_synced_files(&mut manifest, &targets, &cloud_ids, &unsynced);
    if let Err(e) = remote::save_sync::save_manifest(&manifest) {
        warn!("[SAVE-SYNC] Backup: failed to persist manifest: {e}");
    }

    let mut errors: Vec<String> = failures
        .iter()
        .map(|f| format!("{}: {}", f.filename, f.error))
        .collect();
    if let Some(message) = quota_message {
        errors.push(message);
    }

    Ok(BackupResult {
        uploaded: count,
        errors,
    })
}

/// What [`backup_saves`] actually achieved.
///
/// This used to be a bare `usize`. The whole request can succeed while the
/// server rejects every file inside it (`errors[]` in the bulk-upload
/// response), and the panel then printed "Everything's already in sync." over
/// rows still reading "Not backed up" — the exact shape of failure cloud saves
/// keeps producing, reported as success.
#[derive(serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub uploaded: usize,
    /// One `filename: reason` line per file the server refused.
    pub errors: Vec<String>,
}

/// Resolve a PC cloud save back to its real on-disk location and write it.
///
/// The frontend's per-game Cloud Saves panel hits this for `saveType == "pc"`
/// entries. The cloud filename comes back namespaced (e.g. `"pc__<basename>"`);
/// we strip the prefix, look up the game name from the cache, build the same
/// Ludusavi context the launch uses (`remote::save_sync::pc_scan_context`), and
/// ask Ludusavi where that file would live on disk. Then `write_downloaded_pc_save` (which already
/// handles `.bak` backups and missing parent dirs) does the write.
///
/// Errors are user-facing — they end up in the panel's per-row toast.
#[tauri::command]
pub fn restore_pc_cloud_save(
    game_id: String,
    filename: String,
    data: String,
) -> Result<String, String> {
    use base64::Engine;

    // PC saves uploaded by the launch-time sync carry a namespace prefix so
    // they don't collide with emu save filenames, and the body is the save's
    // path relative to the game's save root. `decode_pc_relpath` handles the
    // current `pc__` prefix and the legacy `pc/` prefix, and a legacy bare
    // basename decodes to itself so those rows restore exactly as before.
    let rel = remote::save_sync::decode_pc_relpath(filename.as_str()).ok_or_else(|| {
        format!("That save's name is not one this device will write to disk: {filename}")
    })?;
    let rel_str = rel.to_string_lossy().to_string();

    // Game metadata is cached under `game/{id}` — fetch_library_logic and
    // fetch_game_logic both write that key now. The bare-`{id}` read is a
    // fallback for caches written by an older build (before the key was
    // unified) and becomes a no-op once those age out.
    let game_name = remote::cache::get_cached_object::<games::library::Game>(&format!(
        "game/{game_id}"
    ))
    .or_else(|_| remote::cache::get_cached_object::<games::library::Game>(&game_id))
    .map(|g| g.m_name)
    .map_err(|_| {
        "Game metadata not cached on this device. Open the game's library page once, then retry."
            .to_string()
    })?;

    // The install directory anchors the manifest's `<base>` placeholder, which
    // is how games that save next to their own executable resolve on a machine
    // that has never run them. Absent when the game isn't installed here, and
    // those patterns are simply skipped.
    let install_dir = match borrow_db_checked().applications.game_statuses.get(&game_id) {
        Some(GameDownloadStatus::Installed { install_dir, .. }) => {
            Some(PathBuf::from(install_dir))
        }
        _ => None,
    };

    // Same context every other Ludusavi call site uses: the Steam app id (so
    // the title resolves exactly, not by `--normalized`, which cannot tell a
    // game from its remaster) and the per-game Wine prefix for a Windows game
    // under Proton.
    let scan = remote::save_sync::pc_scan_context(&game_id, None);
    let steam_app_id = scan.steam_app_id;
    let wine_prefix = scan.wine_prefix;

    let dest = remote::save_sync::find_pc_save_destination(
        &game_name,
        steam_app_id.as_deref(),
        &rel_str,
        install_dir.as_deref(),
        wine_prefix.as_deref(),
    )?;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    let written =
        remote::save_sync::write_downloaded_pc_save(&filename, &bytes, Some(&dest), None)?;
    Ok(written.display().to_string())
}

/// Write base64-encoded data to a PC save file at its full path.
/// Used for restoring individual cloud saves to their original location.
///
/// `replace_save_file` takes a timestamped backup and only writes if that
/// backup succeeded, so a restore over the wrong save is recoverable and a
/// failed backup no longer means the original gets destroyed anyway.
#[tauri::command]
pub fn write_pc_save_file(file_path: String, data: String) -> Result<(), String> {
    let path = validate_pc_save_path(&file_path)?;
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| format!("Invalid base64: {e}"))?;
    remote::save_sync::replace_save_file(&path, &bytes)
}

// ── Ludusavi integration for PC game saves ────────────────────────────────

/// Result from Ludusavi backup/find operation.
#[derive(Serialize)]
pub struct LudusaviSaveInfo {
    pub files: Vec<LudusaviFile>,
    pub game_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LudusaviFile {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    /// The name this file has in the cloud: the same `pc__…` identity the
    /// launch sync uploads it under, which for a save in a subfolder is its
    /// path relative to the game's save root, not its basename. `None` for a
    /// file only the common-locations fallback found; launch sync does not
    /// upload those.
    pub cloud_filename: Option<String>,
    /// MD5 of the file, when it came from the sync scanner.
    pub data_hash: Option<String>,
    /// What this account's sync manifest recorded at the last sync, for the
    /// same three-way status the desktop panel shows.
    pub synced_hash: Option<String>,
    pub synced_cloud_id: Option<String>,
    /// As on `LocalSaveEntry`.
    pub last_synced_by_other_account: Option<String>,
    /// As on `LocalSaveEntry`.
    pub other_accounts_on_this_device: bool,
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
///
/// Same resolver the database uses, so this agrees with
/// `remote::save_sync`'s own lookup. Both used to hardcode `"drop"`; a debug
/// build's data root is `drop-debug`, so the installer put Ludusavi in one
/// place and the save scan looked in another.
fn tools_dir() -> std::path::PathBuf {
    database::db::DATA_ROOT_DIR.join("tools")
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

/// The Steam app id for a game, when Drop can work one out.
///
/// A thin alias over [`remote::save_sync::steam_app_id_for_game`], which is
/// where the lookup lives now: the launch and exit save-sync paths run in the
/// `process` crate and needed the same answer, and a second copy of it there
/// would be the third.
fn find_steam_app_id(game_id: &str) -> Option<String> {
    remote::save_sync::steam_app_id_for_game(game_id)
}

/// List PC game save locations using Ludusavi.
/// Returns the files Ludusavi finds for this game.
///
/// Built on the same scan the launch sync uses (`scan_pc_saves` with the
/// shared context: Steam app id and Wine prefix), so every file comes back
/// with the cloud filename it is synced under. Big Picture used to run its own
/// Ludusavi pass and key files on their basename, while the sync names a save
/// by its path relative to the game's save root, so any save in a subfolder
/// never matched its cloud row and could not show a status or be downloaded.
/// The cost: config-only files the sync deliberately skips (settings,
/// keybinds) are no longer listed either. "Backup All" still copies them.
///
/// `Err` when Ludusavi is missing or the scan failed, so the caller can show
/// why instead of an empty list.
#[tauri::command]
pub async fn list_pc_game_saves(
    game_id: String,
    game_name: String,
) -> Result<LudusaviSaveInfo, String> {
    // Ludusavi shells out to several multi-second filesystem scans. Run them on
    // a blocking thread so Tauri's main thread — and, in Big Picture Mode, the
    // gamepad poll loop — isn't frozen while they run.
    tokio::task::spawn_blocking(move || -> Result<LudusaviSaveInfo, String> {
        let scan = remote::save_sync::pc_scan_context(&game_id, None);
        let found = remote::save_sync::scan_pc_saves(
            &game_name,
            scan.steam_app_id.as_deref(),
            scan.wine_prefix.as_deref(),
        )
        .map_err(|e| match e {
            remote::save_sync::PcScanError::LudusaviMissing => {
                "Ludusavi not installed".to_string()
            }
            remote::save_sync::PcScanError::Failed(reason) => reason,
        })?;

        let user_id = remote::save_sync::current_user_id();
        let manifest = user_id
            .as_ref()
            .map(|user_id| remote::save_sync::load_manifest(user_id, &game_id));
        let shared = user_id
            .as_deref()
            .is_some_and(|u| remote::save_sync::other_accounts_have_synced(u, &game_id));
        let files: Vec<LudusaviFile> = found
            .into_iter()
            .map(|f| {
                // As in `scan_local_game_saves`: only a trusted entry is a
                // three-way base, any entry can say whose copy this is.
                let recorded = manifest.as_ref().and_then(|m| m.files.get(&f.filename));
                let synced = recorded.and_then(|e| e.trusted_base());
                LudusaviFile {
                    other_accounts_on_this_device: shared
                        && remote::save_sync::is_shared_between_accounts(&f.filename, false),
                    last_synced_by_other_account: other_account_copy(
                        user_id.as_deref(),
                        &game_id,
                        &f.filename,
                        &f.data_hash,
                        recorded.map(|e| e.synced_hash.as_str()),
                    ),
                    path: f.path.to_string_lossy().to_string(),
                    size: f.size,
                    modified: f.modified_at,
                    synced_hash: synced.map(|e| e.synced_hash.clone()),
                    synced_cloud_id: synced.and_then(|e| e.cloud_id.clone()),
                    cloud_filename: Some(f.filename),
                    data_hash: Some(f.data_hash),
                }
            })
            .collect();

        // If Ludusavi found nothing, try common save locations as a fallback.
        if files.is_empty() {
            log::info!(
                "[LUDUSAVI] No files found via Ludusavi, scanning common save locations for '{}'",
                game_name
            );
            let common_saves = scan_common_save_locations(&game_name, scan.steam_app_id.as_deref());
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
            game_name,
        })
    })
    .await
    .map_err(|e| format!("Ludusavi scan task failed: {e}"))?
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
            if let Some(appdata) = dirs::data_dir()
                && let Some(parent) = appdata.parent()
            {
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
        if let Some(id) = app_id
            && let Ok(program_files) = std::env::var("ProgramFiles(x86)")
        {
            let userdata = Path::new(&program_files).join("Steam").join("userdata");
            if let Ok(entries) = std::fs::read_dir(&userdata) {
                for entry in entries.flatten() {
                    search_dirs.push(entry.path().join(id).join("remote"));
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
                    cloud_filename: None,
                    data_hash: None,
                    synced_hash: None,
                    synced_cloud_id: None,
                    last_synced_by_other_account: None,
                    other_accounts_on_this_device: false,
                });
            }
        }
    }
}

/// Where Big Picture's "Backup All" keeps a game's Ludusavi backup, so that
/// "Restore" can find it again. Under Drop's data dir rather than the system
/// temp dir: SteamOS clears /tmp on reboot, which threw the backup away.
fn ludusavi_backup_dir(game_id: &str) -> Result<std::path::PathBuf, String> {
    // The id becomes a path component; refuse anything that could leave the
    // backups folder.
    if game_id.is_empty()
        || !game_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("Invalid game id: {game_id}"));
    }
    Ok(database::db::DATA_ROOT_DIR
        .join("save-backups")
        .join(game_id))
}

/// Where backups went before they moved into the data dir, on platforms where
/// that location is worth reading. On Windows the per-user temp dir survives
/// reboots, so it can still hold the only backup a player has. On Linux it is
/// the shared, world-writable /tmp, which SteamOS also clears on reboot: a
/// backup there is either gone or not provably ours, so it is not used.
/// Only read, never written.
fn legacy_ludusavi_backup_dir(game_id: &str) -> Option<std::path::PathBuf> {
    cfg!(windows).then(|| std::env::temp_dir().join(format!("drop-ludusavi-{game_id}")))
}

fn dir_has_entries(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

/// The backup Restore should use: the current one; else the previous one, if
/// a replacement was interrupted between moving it aside and moving the new
/// one in; else (Windows only) one left in the legacy temp location.
fn existing_ludusavi_backup(game_id: &str) -> Result<Option<std::path::PathBuf>, String> {
    let dir = ludusavi_backup_dir(game_id)?;
    Ok([Some(dir.clone()), Some(dir.with_extension("old")), legacy_ludusavi_backup_dir(game_id)]
        .into_iter()
        .flatten()
        .find(|candidate| dir_has_entries(candidate)))
}

/// Whether Ludusavi's `--api` output for a backup names at least one game.
/// A run that matched nothing still exits 0 on some versions, and replacing a
/// good backup with an empty one would lose it.
fn ludusavi_backed_up_anything(stdout: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(stdout)
        .ok()
        .and_then(|v| v.get("games")?.as_object().map(|g| !g.is_empty()))
        .unwrap_or(false)
}

/// Backup and Restore for one game must not interleave: both touch the same
/// folders, and a double press would otherwise race on the staging folder.
static LUDUSAVI_BACKUP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Back up a game's PC saves with Ludusavi into Drop's data dir. The previous
/// backup is only replaced once the new one has succeeded and found
/// something, and it is moved aside rather than deleted until the new one is
/// in place, so no failure leaves the player with no backup. Returns the
/// backup path.
#[tauri::command]
pub async fn backup_pc_game_saves(game_id: String, game_name: String) -> Result<String, String> {
    // Ludusavi scans for several seconds. Off the main thread, or the whole
    // UI (and in Big Picture, the controller) freezes until it finishes.
    tokio::task::spawn_blocking(move || backup_pc_game_saves_blocking(&game_id, &game_name))
        .await
        .map_err(|e| format!("Backup task failed: {e}"))?
}

fn backup_pc_game_saves_blocking(game_id: &str, game_name: &str) -> Result<String, String> {
    // The lock guards no data, so a panic in an earlier holder leaves nothing
    // inconsistent; recover rather than disable backups for the session.
    let _guard = LUDUSAVI_BACKUP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let ludusavi = find_ludusavi().ok_or("Ludusavi not installed")?;

    let backup_dir = ludusavi_backup_dir(game_id)?;
    let staging_dir = backup_dir.with_extension("new");
    let aside_dir = backup_dir.with_extension("old");

    // Finish an interrupted replacement: the previous backup was moved aside
    // but the new one never moved in.
    if !backup_dir.exists() && aside_dir.exists() {
        let _ = std::fs::rename(&aside_dir, &backup_dir);
    }
    let _ = std::fs::remove_dir_all(&staging_dir);
    if backup_dir.exists() {
        let _ = std::fs::remove_dir_all(&aside_dir);
    }
    let had_backup = dir_has_entries(&backup_dir);
    let kept = if had_backup { " The previous backup was kept." } else { "" };

    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create backup dir: {e}"))?;

    let game_id = game_id.to_string();
    let game_name = game_name.to_string();
    // Same context as the launch: without the Wine prefix, a Proton game's
    // saves (inside Drop's per-game prefix) were invisible to the backup.
    let scan = remote::save_sync::pc_scan_context(&game_id, None);
    let app_id = scan.steam_app_id.clone();

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

    let mut backup = std::process::Command::new(&ludusavi);
    backup.args(["backup", "--api", "--force", "--path"]).arg(&staging_dir);
    if let Some(prefix) = &scan.wine_prefix {
        backup.arg("--wine-prefix").arg(prefix);
    }
    let output = backup
        .arg(search_name)
        .output()
        .map_err(|e| format!("Failed to run Ludusavi: {e}"))?;

    let found_something = ludusavi_backed_up_anything(&output.stdout);
    if !output.status.success() || !found_something {
        let _ = std::fs::remove_dir_all(&staging_dir);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(match (output.status.success(), found_something) {
            (true, _) => format!("Ludusavi found no saves to back up.{kept}"),
            // Ludusavi exits non-zero with games listed when some files
            // could not be read. A partial backup must not replace a full one.
            (false, true) => format!(
                "Ludusavi could not read every save file. Is the game still running?{kept}"
            ),
            (false, false) => format!("Ludusavi backup failed: {stderr}{kept}"),
        });
    }

    if backup_dir.exists()
        && let Err(e) = std::fs::rename(&backup_dir, &aside_dir)
    {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(format!("Could not replace the previous backup ({e}).{kept}"));
    }
    if let Err(e) = std::fs::rename(&staging_dir, &backup_dir) {
        // Put the previous backup back. If even that fails it stays at
        // `aside_dir`, which Restore and the next Backup both know about.
        if aside_dir.exists() {
            let _ = std::fs::rename(&aside_dir, &backup_dir);
        }
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(format!("Could not save the backup ({e}).{kept}"));
    }
    if let Err(e) = std::fs::remove_dir_all(&aside_dir)
        && aside_dir.exists()
    {
        warn!("could not remove the replaced backup {}: {e}", aside_dir.display());
    }

    Ok(backup_dir.to_string_lossy().to_string())
}

/// Whether "Restore" has a backup to restore for this game on this device.
#[tauri::command]
pub fn has_pc_save_backup(game_id: String) -> bool {
    matches!(existing_ludusavi_backup(&game_id), Ok(Some(_)))
}

/// Restore a game's PC saves from the backup `backup_pc_game_saves` made.
///
/// No Wine prefix is passed here, deliberately: Ludusavi's `restore` takes
/// none. It puts each file back at the absolute path its backup recorded,
/// which for a Proton game is already inside the prefix the backup scanned.
#[tauri::command]
pub async fn restore_pc_game_saves(game_id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || restore_pc_game_saves_blocking(&game_id))
        .await
        .map_err(|e| format!("Restore task failed: {e}"))?
}

fn restore_pc_game_saves_blocking(game_id: &str) -> Result<(), String> {
    let _guard = LUDUSAVI_BACKUP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let ludusavi = find_ludusavi().ok_or("Ludusavi not installed")?;

    let Some(backup_dir) = existing_ludusavi_backup(game_id)? else {
        return Err("There is no backup of this game's saves on this device yet.".to_string());
    };

    let output = std::process::Command::new(&ludusavi)
        .args(["restore", "--api", "--force", "--path"])
        .arg(&backup_dir)
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
///
/// Routed through `remove_save_file` so the bytes are copied aside under a
/// timestamped name first, and so a failed backup aborts the delete instead of
/// preceding it. The UI confirms before calling this, but the BPM button is
/// also a gamepad action — one stray A-press on a couch controller lands here,
/// and a save file has no second copy anywhere else.
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

    remote::save_sync::remove_save_file(&canonical)?;
    Ok(())
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
    use database::{borrow_db_checked, GameDownloadStatus};

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

#[cfg(test)]
mod ludusavi_backup_tests {
    use super::*;

    #[test]
    fn backup_dir_rejects_ids_that_leave_the_folder() {
        assert!(ludusavi_backup_dir("0b6c1f2e-1a2b-4c3d-9e8f-001122334455").is_ok());
        assert!(ludusavi_backup_dir("").is_err());
        assert!(ludusavi_backup_dir("../../etc").is_err());
        assert!(ludusavi_backup_dir("a/b").is_err());
        assert!(ludusavi_backup_dir("a\\b").is_err());
    }

    #[test]
    fn backup_dir_is_not_the_temp_dir() {
        let dir = ludusavi_backup_dir("abc").unwrap();
        assert!(!dir.starts_with(std::env::temp_dir()));
        assert!(dir.ends_with("save-backups/abc"));
    }

    #[test]
    fn empty_or_unreadable_backup_output_counts_as_nothing() {
        assert!(ludusavi_backed_up_anything(
            br#"{"overall":{"totalGames":1},"games":{"Hades":{"files":{}}}}"#
        ));
        assert!(!ludusavi_backed_up_anything(br#"{"overall":{"totalGames":0},"games":{}}"#));
        assert!(!ludusavi_backed_up_anything(b""));
        assert!(!ludusavi_backed_up_anything(b"not json"));
    }
}

/// Result of [`clear_local_achievements`], for the reset UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearLocalAchievementsResult {
    /// `cleared` (files rewritten, possibly zero), `running` (left alone: the
    /// game would write its in-memory state back on exit), `not_installed`,
    /// or `no_emulator` (no Steam API DLL, so no Goldberg save to clear).
    pub status: String,
    /// Files rewritten with every achievement marked not earned.
    pub files_cleared: usize,
    /// Files found but not rewritten, as "path: error".
    pub failures: Vec<String>,
}

/// After an achievement reset on the server, mark the game's local Goldberg
/// save files as not earned, so the next launch doesn't re-report the old
/// unlocks and the player can earn them again.
///
/// The server's reset marker already ignores re-reported unlocks that carry
/// an earned time from before the reset; this covers the files that carry no
/// time, and lets the emulator fire the achievement again at all. Only
/// Goldberg-format JSON files are touched (see `clear_local_unlocks`).
///
/// Refuses while the game is running: the emulator holds its unlock state in
/// memory and writes it back on exit, which would undo the clear.
#[tauri::command]
pub async fn clear_local_achievements(
    game_id: String,
) -> Result<ClearLocalAchievementsResult, String> {
    clear_local_achievements_for(&game_id).await
}

/// Result of [`clear_all_local_achievements`].
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearAllLocalAchievementsResult {
    /// Installed games looked at.
    pub games_checked: usize,
    /// Files rewritten across all of them.
    pub files_cleared: usize,
    /// Games skipped because they were running.
    pub running: Vec<String>,
    /// "game id: error" or "path: error" for everything that failed.
    pub failures: Vec<String>,
}

/// [`clear_local_achievements`] for every game installed on this device,
/// after an "all games" reset. Uses the local install list, so it covers
/// every installed game however many the server's store lists.
#[tauri::command]
pub async fn clear_all_local_achievements() -> ClearAllLocalAchievementsResult {
    let game_ids: Vec<String> = {
        let db = borrow_db_checked();
        db.applications
            .game_statuses
            .iter()
            .filter(|(_, status)| matches!(status, GameDownloadStatus::Installed { .. }))
            .map(|(id, _)| id.clone())
            .collect()
    };
    let mut out = ClearAllLocalAchievementsResult {
        games_checked: game_ids.len(),
        files_cleared: 0,
        running: Vec::new(),
        failures: Vec::new(),
    };
    for game_id in game_ids {
        match clear_local_achievements_for(&game_id).await {
            Ok(r) => {
                out.files_cleared += r.files_cleared;
                if r.status == "running" {
                    out.running.push(game_id.clone());
                }
                out.failures.extend(r.failures);
            }
            Err(e) => out.failures.push(format!("{game_id}: {e}")),
        }
    }
    info!(
        "[ACH] clear_all_local_achievements: {} game(s), {} file(s) cleared, {} running, {} failure(s)",
        out.games_checked,
        out.files_cleared,
        out.running.len(),
        out.failures.len()
    );
    out
}

async fn clear_local_achievements_for(
    game_id: &str,
) -> Result<ClearLocalAchievementsResult, String> {
    let game_id = game_id.to_string();
    let result = |status: &str, files_cleared: usize, failures: Vec<String>| {
        ClearLocalAchievementsResult { status: status.to_string(), files_cleared, failures }
    };

    let running = {
        let id = game_id.clone();
        tokio::task::spawn_blocking(move || PROCESS_MANAGER.lock().is_game_running(&id))
            .await
            .map_err(|e| format!("Could not check whether the game is running: {e}"))?
    };
    if running {
        info!("[ACH] clear_local_achievements: {game_id} is running, leaving its files alone");
        return Ok(result("running", 0, Vec::new()));
    }

    let install_dir = {
        let db = borrow_db_checked();
        match db.applications.game_statuses.get(&game_id) {
            Some(GameDownloadStatus::Installed { install_dir, .. }) => {
                install_dir.clone()
            }
            _ => return Ok(result("not_installed", 0, Vec::new())),
        }
    };

    let Some(dll_dir) = remote::goldberg::discovery::find_steam_api_dir(Path::new(&install_dir))
    else {
        return Ok(result("no_emulator", 0, Vec::new()));
    };
    let dll_dir = dll_dir.to_string_lossy().to_string();

    // Every AppID the emulator may be saving under: what's on disk, plus the
    // server's Goldberg link (which can differ from the game's own file).
    let mut app_ids = remote::goldberg::local_app_ids(&dll_dir);
    match remote::achievements::fetch_achievement_config(&game_id).await {
        Ok(config) => {
            for link in config.external_links {
                if link.provider == "Goldberg" && !app_ids.contains(&link.external_game_id) {
                    app_ids.push(link.external_game_id);
                }
            }
        }
        // The on-disk AppIDs are usually the same ones; carry on with them.
        Err(e) => warn!("[ACH] clear_local_achievements: config fetch failed for {game_id}: {e}"),
    }

    let wine_prefix = remote::goldberg::wine_prefix_for_game(&game_id);
    let report = remote::goldberg::clear_local_unlocks(&app_ids, &dll_dir, wine_prefix.as_deref());
    info!(
        "[ACH] clear_local_achievements: {game_id}: AppIDs {app_ids:?}, cleared {} file(s), {} failure(s)",
        report.cleared.len(),
        report.failed.len()
    );
    Ok(result(
        "cleared",
        report.cleared.len(),
        report
            .failed
            .into_iter()
            .map(|(p, e)| format!("{}: {e}", p.display()))
            .collect(),
    ))
}
