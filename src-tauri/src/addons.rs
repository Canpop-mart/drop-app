//! Tauri commands for addon shared saves (the league check-out / check-in
//! buttons). The file work lives in `remote::addons`; this layer makes sure
//! the game can't be running, or start, while its save changes.

use remote::addons::{self, AddonError, InstallOutcome, SaveSlot};
use serde::Serialize;

fn game_busy() -> AddonError {
    AddonError::new(
        "game_running",
        "Close the game first. Drop can't change its save while it's running.",
    )
}

fn refused(r: process::AddonHoldRefused) -> AddonError {
    match r {
        process::AddonHoldRefused::GameActive => game_busy(),
        process::AddonHoldRefused::AlreadyHeld => AddonError::new(
            "busy",
            "Drop is already working on this save. Wait for it to finish.",
        ),
    }
}

/// The game isn't running, through Drop or otherwise.
fn check_not_running(slot: &SaveSlot) -> Result<(), AddonError> {
    if game_process_running(&slot.process_names) {
        return Err(game_busy());
    }
    Ok(())
}

/// Pure: whether a process (its name, exe path and first argument as the OS
/// reports them) is one of `wanted`. Case-insensitive, either separator, and
/// allowing Linux's 15-character truncation of process names, which is how a
/// Proton game's `ncaafootball14.exe` shows up as `ncaafootball14.`.
pub fn process_matches(name: &str, exe: Option<&str>, arg0: Option<&str>, wanted: &[String]) -> bool {
    let base = |s: &str| {
        s.rsplit(['/', '\\'])
            .next()
            .unwrap_or(s)
            .trim()
            .to_ascii_lowercase()
    };
    let name = base(name);
    let candidates = [Some(name.clone()), exe.map(base), arg0.map(base)];
    wanted.iter().any(|w| {
        let w = w.to_ascii_lowercase();
        candidates.iter().flatten().any(|c| *c == w)
            || (name.len() == 15 && w.len() > 15 && w.starts_with(&name))
    })
}

/// Whether one of the slot's game processes is running, inside Drop or not
/// (a player might start the game some other way).
fn game_process_running(wanted: &[String]) -> bool {
    if wanted.is_empty() {
        return false;
    }
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system.processes().values().any(|p| {
        let exe = p.exe().map(|e| e.to_string_lossy().into_owned());
        let arg0 = p.cmd().first().map(|a| a.to_string_lossy().into_owned());
        process_matches(
            &p.name().to_string_lossy(),
            exe.as_deref(),
            arg0.as_deref(),
            wanted,
        )
    })
}

/// Hold the game against launches, after checking nothing is running it.
fn hold(game_id: &str, slot: &SaveSlot) -> Result<process::AddonHold, AddonError> {
    if !addons::valid_game_id(game_id) {
        return Err(AddonError::new("unsupported", "Bad game id."));
    }
    addons::check_slot(slot)?;
    let held = process::hold_for_addon(game_id).map_err(refused)?;
    // Checked after the hold is taken, so Drop can't start it in between.
    check_not_running(slot)?;
    Ok(held)
}

/// Put the version just checked out into the game's save folder.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn addon_install_save(
    game_id: String,
    addon: String,
    context: String,
    version: u64,
    sha1: String,
    slot: SaveSlot,
    replace_unsent: bool,
) -> Result<InstallOutcome, AddonError> {
    let _hold = hold(&game_id, &slot)?;
    let names = slot.process_names.clone();
    let still_safe = move || {
        if game_process_running(&names) {
            Err(game_busy())
        } else {
            Ok(())
        }
    };
    addons::install_save(
        &game_id,
        &addon,
        &context,
        version,
        &sha1,
        &slot,
        replace_unsent,
        &still_safe,
    )
    .await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    /// The save here has play never checked in; checking out would replace
    /// it, so ask first.
    pub unsent: bool,
}

/// Everything that would stop a check-out on this device, checked BEFORE the
/// league's lock is taken, so a player isn't left holding the save because
/// of something local (game open, no save folder yet, several profiles).
#[tauri::command]
pub async fn addon_preflight(
    game_id: String,
    addon: String,
    context: String,
    slot: SaveSlot,
) -> Result<Preflight, AddonError> {
    if !addons::valid_addon_name(&addon) || !addons::valid_context(&context) {
        return Err(AddonError::new("unsupported", "Bad addon details."));
    }
    if let Some(r) = process::addon_hold_blocked(&game_id) {
        return Err(refused(r));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let resolved = addons::resolve_save(&game_id, &slot)?;
        check_not_running(&slot)?;
        let unsent = addons::local_unsent(&addon, &context, &resolved.file)?;
        Ok(Preflight { unsent })
    })
    .await
    .map_err(|e| AddonError::new("io", e.to_string()))?
}

/// Hand the local save in. Returns drop-server's answer unchanged.
#[tauri::command]
pub async fn addon_check_in(
    game_id: String,
    addon: String,
    context: String,
    slot: SaveSlot,
) -> Result<serde_json::Value, AddonError> {
    let _hold = hold(&game_id, &slot)?;
    addons::upload_save(&game_id, &addon, &context, &slot).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSaveState {
    /// The version this device checked out, if it has a receipt.
    pub base_version: Option<u64>,
    pub handed_in: bool,
    pub save_exists: bool,
    /// The save differs from what this device last wrote or handed in: play
    /// that hasn't been checked in yet.
    pub unsent: bool,
    /// Why the save couldn't be looked at (no folder yet, several profiles).
    pub problem: Option<AddonError>,
}

/// What this device knows about its copy of a league save. Read only.
#[tauri::command]
pub async fn addon_local_state(
    game_id: String,
    addon: String,
    context: String,
    slot: SaveSlot,
) -> Result<LocalSaveState, AddonError> {
    if !addons::valid_addon_name(&addon) || !addons::valid_context(&context) {
        return Err(AddonError::new("unsupported", "Bad addon details."));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let receipt = addons::read_receipt(&addon, &context);
        let mut state = LocalSaveState {
            base_version: receipt.base_version,
            handed_in: receipt.handed_in,
            save_exists: false,
            unsent: false,
            problem: None,
        };
        match addons::resolve_save(&game_id, &slot) {
            Ok(resolved) => {
                state.save_exists = resolved.file.exists();
                if state.save_exists && receipt.base_version.is_some() {
                    match addons::local_unsent(&addon, &context, &resolved.file) {
                        Ok(u) => state.unsent = u,
                        Err(e) => state.problem = Some(e),
                    }
                }
            }
            Err(e) => state.problem = Some(e),
        }
        state
    })
    .await
    .map_err(|e| AddonError::new("io", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::process_matches;

    #[test]
    fn matches_game_processes() {
        let w = vec!["ncaafootball14.exe".to_string()];
        assert!(process_matches("ncaafootball14.exe", None, None, &w));
        assert!(process_matches("NCAAFootball14.EXE", None, None, &w));
        // Linux truncates comm to 15 characters
        assert!(process_matches("ncaafootball14.", None, None, &w));
        assert!(process_matches("wine-preloader", None, Some("C:\\Games\\NCAA\\ncaafootball14.exe"), &w));
        assert!(process_matches("x", Some("/games/ncaa/ncaafootball14.exe"), None, &w));
        assert!(!process_matches("ncaafootball13.exe", None, None, &w));
        assert!(!process_matches("ncaafootball1", None, None, &w));
        assert!(!process_matches("ncaafootball14.exe", None, None, &[]));
    }
}
