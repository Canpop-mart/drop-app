use serde::Serialize;

use crate::{app_status::AppStatus, user::User};

#[derive(Clone, Serialize, PartialEq, Eq)]
pub enum UmuState {
    NotNeeded,
    NotInstalled,
    NoDefault,
    Installed,
}

/// Describes the display session environment the app is running in.
/// Used by the frontend to decide whether to auto-enter Big Picture Mode.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionType {
    /// Standard desktop session (X11/Wayland compositor)
    Desktop,
    /// Running inside Gamescope (SteamOS Game Mode or standalone gamescope)
    Gamescope,
    /// Steam Deck hardware detected but not in Game Mode (Desktop Mode)
    SteamDeckDesktop,
}

impl SessionType {
    /// Detect the current session type from environment variables and system info.
    pub fn detect() -> Self {
        // Check for Gamescope compositor (SteamOS Game Mode)
        if std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok() {
            return Self::Gamescope;
        }

        // XDG_CURRENT_DESKTOP=gamescope is set in SteamOS Game Mode
        if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
            if desktop.to_lowercase().contains("gamescope") {
                return Self::Gamescope;
            }
        }

        // SteamOS sets this in Game Mode sessions
        if std::env::var("SteamGamepadUI").is_ok() {
            return Self::Gamescope;
        }

        // Detect Steam Deck hardware even if not in Game Mode
        // The Deck's DMI board name is "Jupiter" (LCD) or "Galileo" (OLED)
        #[cfg(target_os = "linux")]
        {
            if let Ok(board) = std::fs::read_to_string("/sys/devices/virtual/dmi/id/board_name") {
                let board = board.trim().to_lowercase();
                if board == "jupiter" || board == "galileo" {
                    return Self::SteamDeckDesktop;
                }
            }
        }

        Self::Desktop
    }

    /// Returns true if this session should auto-enter Big Picture Mode.
    pub fn should_auto_bpm(&self) -> bool {
        matches!(self, Self::Gamescope)
    }

    /// Returns true if running on Steam Deck hardware (any mode).
    pub fn is_steam_deck_hardware(&self) -> bool {
        matches!(self, Self::Gamescope | Self::SteamDeckDesktop)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub status: AppStatus,
    pub user: Option<User>,
    pub umu_state: UmuState,
    pub session_type: SessionType,
}
