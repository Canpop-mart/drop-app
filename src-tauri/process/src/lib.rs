#![feature(nonpoison_mutex)]
#![feature(sync_nonpoison)]
#![feature(extend_one)]
#![cfg_attr(target_os = "linux", feature(vec_try_remove))]

use std::{
    collections::HashMap,
    ops::Deref,
    sync::{OnceLock, nonpoison::Mutex},
};

use tauri::AppHandle;

use crate::process_manager::ProcessManager;

pub static PROCESS_MANAGER: ProcessManagerWrapper = ProcessManagerWrapper::new();

/// Process-launch AppHandle, set once during init. Lets launch-path code that
/// has no AppHandle of its own (e.g. `prefix_prep`) emit Tauri events so the
/// UI can surface a "Preparing…" status during slow one-time prefix setup.
pub static LAUNCH_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Global registry of pending conflict resolution channels.
/// Key: game_id → Sender that unblocks the pre-launch sync.
pub static CONFLICT_CHANNELS: std::sync::LazyLock<
    Mutex<HashMap<String, std::sync::mpsc::Sender<Vec<remote::save_sync::ConflictResolution>>>>,
> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(target_os = "linux")]
pub mod compat;
pub mod error;
pub mod format;
pub mod gamepad;
pub mod m3u;
/// Public so the client layer can read the executable out of a server launch
/// command with exactly the same tokenising rules the launch path uses.
pub mod parser;
pub mod prefix_prep;
pub mod process_handlers;
pub mod process_manager;

/// Held while an addon changes a game's save; launching that game is refused
/// until it drops. See [`hold_for_addon`].
#[must_use = "the hold ends when this is dropped"]
pub struct AddonHold(String);

impl Drop for AddonHold {
    fn drop(&mut self) {
        PROCESS_MANAGER.lock().addon_holds.remove(&self.0);
    }
}

/// Why [`hold_for_addon`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddonHoldRefused {
    /// The game is running or part-way through launching.
    GameActive,
    /// Another addon step for this game is already running.
    AlreadyHeld,
}

/// Hold `game_id` so it can't launch through Drop while an addon changes its
/// save. Checked and taken under the process manager's lock, which is the
/// lock `prepare_launch` runs under, so no Drop launch can slip in between.
/// It can't stop the game being started outside Drop; callers check for its
/// process separately.
pub fn hold_for_addon(game_id: &str) -> Result<AddonHold, AddonHoldRefused> {
    let mut pm = PROCESS_MANAGER.lock();
    if pm.is_game_active(game_id) {
        return Err(AddonHoldRefused::GameActive);
    }
    if pm.addon_holds.contains(game_id) {
        return Err(AddonHoldRefused::AlreadyHeld);
    }
    pm.addon_holds.insert(game_id.to_string());
    Ok(AddonHold(game_id.to_string()))
}

/// Whether the game is running or launching through Drop, or an addon step
/// holds it. Read only.
pub fn addon_hold_blocked(game_id: &str) -> Option<AddonHoldRefused> {
    let pm = PROCESS_MANAGER.lock();
    if pm.is_game_active(game_id) {
        Some(AddonHoldRefused::GameActive)
    } else if pm.addon_holds.contains(game_id) {
        Some(AddonHoldRefused::AlreadyHeld)
    } else {
        None
    }
}

pub struct ProcessManagerWrapper(OnceLock<Mutex<ProcessManager<'static>>>);
impl ProcessManagerWrapper {
    const fn new() -> Self {
        ProcessManagerWrapper(OnceLock::new())
    }
    pub fn init(app_handle: AppHandle) {
        let _ = LAUNCH_APP_HANDLE.set(app_handle.clone());
        if PROCESS_MANAGER
            .0
            .set(Mutex::new(ProcessManager::new(app_handle)))
            .is_err()
        {
            log::error!("Failed to initialise Process Manager: already initialised");
        }
    }
}
impl Deref for ProcessManagerWrapper {
    type Target = Mutex<ProcessManager<'static>>;

    fn deref(&self) -> &Self::Target {
        match self.0.get() {
            Some(process_manager) => process_manager,
            None => unreachable!("Process manager should always be initialised"),
        }
    }
}
