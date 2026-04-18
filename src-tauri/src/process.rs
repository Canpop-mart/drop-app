use std::sync::Arc;

use process::{
    CONFLICT_CHANNELS, PROCESS_MANAGER,
    error::ProcessError,
    process_manager::{LaunchOption, ProcessManager},
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn get_launch_options(id: String) -> Result<Vec<LaunchOption>, ProcessError> {
    let launch_options = ProcessManager::get_launch_options(id)?;

    Ok(launch_options)
}

#[derive(Serialize)]
#[serde(tag = "result", content = "data")]
pub enum LaunchResult {
    Success,
    InstallRequired(String, String),
}

#[tauri::command]
pub fn launch_game(id: String, index: usize) -> Result<LaunchResult, ProcessError> {
    launch_game_inner(id, index, false, None)
}

/// Launch a game for streaming. Auto-resolves save conflicts and optionally
/// applies the remote client's user configuration (widescreen, quality, etc.).
pub fn launch_game_streaming(
    id: String,
    index: usize,
    config_override: Option<database::models::data::UserConfiguration>,
) -> Result<LaunchResult, ProcessError> {
    launch_game_inner(id, index, true, config_override)
}

fn launch_game_inner(
    id: String,
    index: usize,
    streaming: bool,
    config_override: Option<database::models::data::UserConfiguration>,
) -> Result<LaunchResult, ProcessError> {
    let result = {
        let mut process_manager_lock = PROCESS_MANAGER.lock();

        if streaming {
            process_manager_lock.launch_process_streaming(id, index, config_override)
        } else {
            process_manager_lock.launch_process(id, index)
        }
    };

    if let Err(err) = &result
        && let ProcessError::RequiredDependency(game_id, version_id) = err
    {
        return Ok(LaunchResult::InstallRequired(
            game_id.to_string(),
            version_id.to_string(),
        ));
    }

    result?;

    Ok(LaunchResult::Success)
}

#[tauri::command]
pub fn kill_game(game_id: String) -> Result<(), ProcessError> {
    Ok(PROCESS_MANAGER.lock().kill_game(game_id)?)
}

#[tauri::command]
pub fn open_process_logs(game_id: String, app_handle: AppHandle) -> Result<(), ProcessError> {
    let process_manager_lock = PROCESS_MANAGER.lock();

    let dir = process_manager_lock.get_log_dir(&game_id);
    app_handle
        .opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|v| ProcessError::OpenerError(Arc::new(v)))
}

/// Frontend sends this after the user resolves save conflicts.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionPayload {
    pub game_id: String,
    pub resolutions: Vec<ConflictResolutionEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionEntry {
    pub filename: String,
    pub choice: String, // "keep_local" | "keep_cloud"
}

#[tauri::command]
pub fn resolve_save_conflicts(payload: ConflictResolutionPayload) -> Result<(), String> {
    let sender = {
        let mut channels = CONFLICT_CHANNELS.lock();
        channels.remove(&payload.game_id)
    };

    match sender {
        Some(tx) => {
            let resolutions: Vec<remote::save_sync::ConflictResolution> = payload
                .resolutions
                .into_iter()
                .map(|r| remote::save_sync::ConflictResolution {
                    filename: r.filename,
                    choice: r.choice,
                })
                .collect();
            tx.send(resolutions)
                .map_err(|_| "Conflict resolution channel closed (launch may have timed out)".to_string())
        }
        None => Err(format!(
            "No pending conflict resolution for game {}",
            payload.game_id
        )),
    }
}
