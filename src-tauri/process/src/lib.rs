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
mod parser;
pub mod prefix_prep;
pub mod process_handlers;
pub mod process_manager;

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
