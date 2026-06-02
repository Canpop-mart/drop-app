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
pub async fn launch_game(
    id: String,
    index: usize,
    incognito: Option<bool>,
) -> Result<LaunchResult, ProcessError> {
    // launch_game_inner holds the PROCESS_MANAGER lock and, on a save-sync
    // conflict, blocks on the UI-resolution channel for up to 5 minutes
    // (see process_manager::save_sync). As a *synchronous* command this ran
    // on the WebView/main thread, so the conflict dialog could never paint
    // and the whole app froze. Make the command async (Tauri runs it off the
    // main thread) and run the blocking work on a dedicated OS thread — NOT
    // spawn_blocking — because launch_game_inner internally calls
    // tauri::async_runtime::block_on, which panics on a Tokio runtime thread
    // but is fine on a plain std thread (as it was on the main thread).
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(launch_game_inner(
            id,
            index,
            false,
            None,
            incognito.unwrap_or(false),
        ));
    });
    rx.await.unwrap_or_else(|_| {
        Err(ProcessError::IOError(Arc::new(std::io::Error::other(
            "launch thread terminated before returning a result",
        ))))
    })
}

/// Launch a game for streaming. Auto-resolves save conflicts and optionally
/// applies the remote client's user configuration (widescreen, quality, etc.).
/// Streaming never runs incognito — the remote client wants the play credited.
pub fn launch_game_streaming(
    id: String,
    index: usize,
    config_override: Option<database::models::data::UserConfiguration>,
) -> Result<LaunchResult, ProcessError> {
    launch_game_inner(id, index, true, config_override, false)
}

fn launch_game_inner(
    id: String,
    index: usize,
    streaming: bool,
    config_override: Option<database::models::data::UserConfiguration>,
    incognito: bool,
) -> Result<LaunchResult, ProcessError> {
    let result = {
        let mut process_manager_lock = PROCESS_MANAGER.lock();

        if streaming {
            process_manager_lock.launch_process_streaming(id, index, config_override)
        } else {
            process_manager_lock.launch_process(id, index, incognito)
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

/// Read the tail of the most recent per-launch log for a game. Used by the
/// BPM launch error dialog so the user can see why the process died without
/// leaving Big Picture Mode.
#[tauri::command]
pub fn read_latest_launch_log(
    game_id: String,
    max_lines: Option<usize>,
    stderr: Option<bool>,
) -> Result<LaunchLogTail, String> {
    let dir = {
        let lock = PROCESS_MANAGER.lock();
        lock.get_log_dir(&game_id)
    };
    if !dir.exists() {
        return Ok(LaunchLogTail {
            path: dir.display().to_string(),
            tail: String::new(),
            truncated: false,
        });
    }

    // Find the newest .log file. If `stderr` is true, prefer *-error.log; else
    // prefer the plain log. Fall back to any if the preferred suffix is missing.
    let want_stderr = stderr.unwrap_or(false);
    let mut best: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return Err(format!("read_dir({}): {e}", dir.display())),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".log") {
            continue;
        }
        let is_error = name.ends_with("-error.log");
        if want_stderr != is_error {
            continue;
        }
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        match &best {
            None => best = Some((path, mtime)),
            Some((_, prev)) if mtime > *prev => best = Some((path, mtime)),
            _ => {}
        }
    }

    let Some((path, _)) = best else {
        return Ok(LaunchLogTail {
            path: dir.display().to_string(),
            tail: String::new(),
            truncated: false,
        });
    };

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let max = max_lines.unwrap_or(400);
    let lines: Vec<&str> = content.lines().collect();
    let truncated = lines.len() > max;
    let start = if truncated { lines.len() - max } else { 0 };
    Ok(LaunchLogTail {
        path: path.display().to_string(),
        tail: lines[start..].join("\n"),
        truncated,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchLogTail {
    pub path: String,
    pub tail: String,
    pub truncated: bool,
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

/// Diagnostic: list controllers visible to Drop's gilrs view. Used by the
/// emulator-controller troubleshoot flow so a user can confirm Drop and the
/// OS agree on which pads are plugged in before complaining that the game
/// can't see them. Delegates to the `process` crate where gilrs already lives.
#[tauri::command]
pub fn list_gamepads() -> Vec<process::gamepad::DetectedGamepad> {
    process::gamepad::list_connected_gamepads()
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
