use std::sync::nonpoison::Mutex;

use database::{borrow_db_checked, borrow_db_mut_checked};
use download_manager::DOWNLOAD_MANAGER;
use log::{debug, error};
use remote::requests::{generate_url, make_authenticated_get};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::AppState;

#[tauri::command]
pub fn fetch_state(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let guard = state.lock();
    let cloned_state = serde_json::to_string(&guard.clone()).map_err(|e| e.to_string())?;
    drop(guard);
    Ok(cloned_state)
}

#[tauri::command]
pub async fn quit(app: tauri::AppHandle) {
    cleanup_and_exit(&app).await;
}

pub async fn cleanup_and_exit(app: &AppHandle) {
    debug!("cleaning up and exiting application");
    match DOWNLOAD_MANAGER.ensure_terminated().await {
        Ok(()) => debug!("download manager terminated correctly"),
        Err(_) => error!("download manager failed to terminate correctly"),
    }

    app.exit(0);
}

#[tauri::command]
pub fn toggle_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
        debug!("enabled autostart");
    } else {
        manager.disable().map_err(|e| e.to_string())?;
        debug!("disabled autostart");
    }

    // Store the state in DB
    let mut db_handle = borrow_db_mut_checked();
    db_handle.settings.autostart = enabled;
    Ok(())
}

#[tauri::command]
pub fn get_autostart_enabled(app: AppHandle) -> Result<bool, tauri_plugin_autostart::Error> {
    let db_handle = borrow_db_checked();
    let db_state = db_handle.settings.autostart;
    drop(db_handle);

    // Get actual system state
    let manager = app.autolaunch();
    let system_state = manager.is_enabled()?;

    // If they don't match, sync to DB state
    if db_state != system_state {
        if db_state {
            manager.enable()?;
        } else {
            manager.disable()?;
        }
    }

    Ok(db_state)
}

#[tauri::command]
pub fn open_fs(path: String, app_handle: AppHandle) -> Result<(), tauri_plugin_opener::Error> {
    app_handle.opener().open_path(path, None::<&str>)
}


#[tauri::command]
pub async fn check_online() -> Result<bool, ()> {
    let online = make_authenticated_get(generate_url(&["/api/v1/"], &[]).unwrap()).await.is_ok();
    Ok(online)
}

/// Register Drop as a non-Steam game shortcut so it appears in SteamOS Game Mode.
#[cfg(target_os = "linux")]
#[tauri::command]
pub fn register_steam_shortcut() -> ::client::steam_shortcut::ShortcutResult {
    ::client::steam_shortcut::register_steam_shortcut()
}

/// Add a specific game to Steam as a non-Steam shortcut with artwork.
/// Downloads banner, cover, and icon from the Drop server's object store
/// and places them in Steam's grid directory.
#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn add_game_to_steam(
    game_id: String,
    game_name: String,
    banner_object_id: Option<String>,
    cover_object_id: Option<String>,
    icon_object_id: Option<String>,
) -> ::client::steam_shortcut::ShortcutResult {
    use ::client::steam_shortcut::{GameShortcutInfo, SteamArtworkType};
    use log::info;

    info!("[STEAM] Adding game '{}' ({}) to Steam", game_name, game_id);

    let mut artwork: Vec<(SteamArtworkType, Vec<u8>)> = Vec::new();

    // Download each artwork type from the object store
    let object_ids: Vec<(Option<String>, SteamArtworkType)> = vec![
        (banner_object_id, SteamArtworkType::Hero),
        (cover_object_id, SteamArtworkType::Grid),
        (icon_object_id, SteamArtworkType::Icon),
    ];

    for (obj_id_opt, art_type) in object_ids {
        if let Some(obj_id) = obj_id_opt {
            if obj_id.is_empty() {
                continue;
            }
            match download_object(&obj_id).await {
                Ok(bytes) => {
                    info!("[STEAM] Downloaded artwork {} ({} bytes)", obj_id, bytes.len());
                    artwork.push((art_type, bytes));
                }
                Err(e) => {
                    log::warn!("[STEAM] Failed to download artwork {}: {}", obj_id, e);
                }
            }
        }
    }

    // Also use the banner as the header (wide grid) image
    if let Some(banner_bytes) = artwork.iter().find(|(t, _)| matches!(t, SteamArtworkType::Hero)).map(|(_, b)| b.clone()) {
        artwork.push((SteamArtworkType::Header, banner_bytes));
    }

    let info = GameShortcutInfo {
        game_id,
        game_name,
        artwork,
    };

    ::client::steam_shortcut::add_game_to_steam(info)
}

/// Download an object from the Drop server's object store.
#[cfg(target_os = "linux")]
async fn download_object(object_id: &str) -> Result<Vec<u8>, String> {
    let url = generate_url(&["/api/v1/object", object_id], &[])
        .map_err(|e| format!("URL error: {}", e))?;
    let response = make_authenticated_get(url)
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Read error: {}", e))
}