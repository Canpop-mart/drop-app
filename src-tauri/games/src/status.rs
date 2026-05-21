//! Centralised game-status state machine.
//!
//! Before this module, game status was a grab-bag: ~6 call sites across
//! `library.rs`, `download_agent.rs`, `scan.rs` and the Tauri command layer
//! each inserted directly into `db.applications.game_statuses` /
//! `transient_statuses`. There was no single place that knew the legal
//! transitions, so an invalid one (e.g. `Remote -> Running`) was simply
//! whatever the last writer happened to do.
//!
//! ## The two-layer model
//!
//! Drop splits a game's status into two layers:
//!
//!   - **Persistent** ([`GameDownloadStatus`]) — written to disk. Either
//!     `Remote {}` (not installed) or `Installed { install_type, .. }` where
//!     `install_type` is `Installed`, `SetupRequired` or `PartiallyInstalled`.
//!   - **Transient** ([`ApplicationTransientStatus`]) — `#[serde(skip)]`, so
//!     it never survives a process restart: `Queued`, `Downloading`,
//!     `Validating`, `Updating`, `Uninstalling`, `Running`.
//!
//! When a transient status is present it *masks* the persistent one in the UI
//! (see [`GameStatusManager::fetch_state`]).
//!
//! ## Why transient state cannot get "stuck"
//!
//! Because the transient map is never serialised, an app crash mid-
//! `Validating` (or `Downloading`/`Updating`) loses the transient status on
//! the next launch and the game falls back to its persistent status. The one
//! thing a crash *can* leave inconsistent is the persistent status vs. the
//! actual files on disk — e.g. a crash mid-`Uninstalling` leaves a half-
//! deleted directory while the persistent status is still `Installed`. That
//! is what [`reconcile_on_startup`] exists to repair.
//!
//! All status writes should go through [`transition`] / the helpers here so
//! every change is logged and validated against [`is_valid_transition`].

use std::path::Path;

use database::{
    ApplicationTransientStatus, Database, DownloadType, DownloadableMetadata, GameDownloadStatus,
    models::data::InstalledGameType,
};
use log::{info, warn};

use crate::state::GameStatusWithTransient;

/// A flattened, comparable view of a game's status used purely for logging
/// and transition validation. The real persisted/transient enums carry
/// payloads (version ids, configs) that we do not want to compare on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Not installed — `GameDownloadStatus::Remote`.
    Remote,
    /// Queued for download — transient.
    Queued,
    /// Actively downloading — transient.
    Downloading,
    /// Post-download manifest validation — transient.
    Validating,
    /// Updating to a newer version — transient.
    Updating,
    /// Being removed from disk — transient.
    Uninstalling,
    /// Running — transient.
    Running,
    /// Installed and complete.
    Installed,
    /// Installed but a post-install setup step still has to run.
    SetupRequired,
    /// On disk but incomplete (cancelled download, failed validation, or a
    /// scan that could not confirm the files match the manifest).
    PartiallyInstalled,
}

impl StatusKind {
    /// Flatten a `(persistent, transient)` pair into a single kind. The
    /// transient layer always wins because it masks the persistent layer in
    /// the UI (mirrors [`GameStatusManager::fetch_state`]).
    pub fn from_state(state: &GameStatusWithTransient) -> Option<Self> {
        if let Some(transient) = &state.1 {
            return Some(Self::from_transient(transient));
        }
        state.0.as_ref().map(Self::from_persistent)
    }

    pub fn from_persistent(status: &GameDownloadStatus) -> Self {
        match status {
            GameDownloadStatus::Remote {} => Self::Remote,
            GameDownloadStatus::Installed { install_type, .. } => match install_type {
                InstalledGameType::Installed => Self::Installed,
                InstalledGameType::SetupRequired => Self::SetupRequired,
                InstalledGameType::PartiallyInstalled { .. } => Self::PartiallyInstalled,
            },
        }
    }

    pub fn from_transient(status: &ApplicationTransientStatus) -> Self {
        match status {
            ApplicationTransientStatus::Queued { .. } => Self::Queued,
            ApplicationTransientStatus::Downloading { .. } => Self::Downloading,
            ApplicationTransientStatus::Validating { .. } => Self::Validating,
            ApplicationTransientStatus::Updating { .. } => Self::Updating,
            ApplicationTransientStatus::Uninstalling {} => Self::Uninstalling,
            ApplicationTransientStatus::Running {} => Self::Running,
        }
    }

    /// Is this a persistent (disk-backed) status?
    pub fn is_persistent(self) -> bool {
        matches!(
            self,
            Self::Remote | Self::Installed | Self::SetupRequired | Self::PartiallyInstalled
        )
    }
}

/// Whether a game may legally move `from` one status `to` another.
///
/// This is intentionally permissive about *re-entering* the same state (a
/// repeated `Downloading -> Downloading` is fine — e.g. progress refreshes)
/// and only rejects transitions that indicate a genuine logic error. It is
/// used by [`transition`] to log loudly; it does not hard-abort, because a
/// panic here would be far worse than a mislabelled status.
pub fn is_valid_transition(from: StatusKind, to: StatusKind) -> bool {
    use StatusKind::*;

    if from == to {
        return true;
    }

    match (from, to) {
        // Starting / restarting an install.
        (Remote | PartiallyInstalled, Queued | Downloading) => true,
        // Re-validating or repairing an existing install.
        (Installed | SetupRequired | PartiallyInstalled, Queued | Downloading | Updating) => true,
        // Download lifecycle.
        (Queued, Downloading) => true,
        (Downloading, Queued | Validating) => true,
        (Updating, Queued | Downloading | Validating) => true,
        (Validating, Installed | SetupRequired | PartiallyInstalled) => true,
        // A paused/cancelled download drops back to a partial install.
        (Queued | Downloading | Updating | Validating, PartiallyInstalled | Remote) => true,
        // Install completion can also land directly (scan import path).
        (Remote, Installed | SetupRequired | PartiallyInstalled) => true,
        // Launching / exiting a game.
        (Installed | SetupRequired, Running) => true,
        (Running, Installed | SetupRequired) => true,
        // Uninstall.
        (Installed | SetupRequired | PartiallyInstalled, Uninstalling) => true,
        (Uninstalling, Remote) => true,
        // A failed/abandoned uninstall must be allowed to fall back.
        (Uninstalling, Installed | SetupRequired | PartiallyInstalled) => true,
        _ => false,
    }
}

/// Log a status transition, flagging it loudly if it is not in the legal set.
///
/// Call this immediately before mutating the database. It does not perform
/// the mutation itself — the persistent vs. transient split means the actual
/// write differs per call site — but it gives every transition a single,
/// greppable log line (`[game-status]`).
pub fn transition(game_id: &str, from: Option<StatusKind>, to: StatusKind) {
    match from {
        Some(from) if !is_valid_transition(from, to) => {
            warn!(
                "[game-status] INVALID transition for {game_id}: {from:?} -> {to:?} \
                 (proceeding, but this indicates a logic error)"
            );
        }
        Some(from) => {
            info!("[game-status] {game_id}: {from:?} -> {to:?}");
        }
        None => {
            info!("[game-status] {game_id}: <unknown> -> {to:?}");
        }
    }
}

/// Log a transition derived from the current DB state, then return the
/// flattened `to` kind. Convenience for the common pattern of "look up where
/// the game is now, then move it".
pub fn transition_from_db(db: &Database, game_id: &str, to: StatusKind) {
    let current = StatusKind::from_state(&crate::state::GameStatusManager::fetch_state(
        &game_id.to_string(),
        db,
    ));
    transition(game_id, current, to);
}

/// Outcome of [`reconcile_on_startup`], surfaced so the caller can log a
/// one-line summary.
#[derive(Debug, Default)]
pub struct ReconcileReport {
    /// Games whose install directory vanished — demoted to `Remote`.
    pub missing_dir: Vec<String>,
    /// Games whose `.dropdata` is gone (install never finished or was
    /// partially removed) — demoted to `PartiallyInstalled`.
    pub demoted_partial: Vec<String>,
    /// Stale transient statuses found in memory (should be impossible since
    /// the map is `#[serde(skip)]`, but cleared defensively).
    pub cleared_transient: usize,
}

impl ReconcileReport {
    pub fn is_empty(&self) -> bool {
        self.missing_dir.is_empty()
            && self.demoted_partial.is_empty()
            && self.cleared_transient == 0
    }
}

/// Repair impossible / stuck game states on startup.
///
/// This is the safety net for the one inconsistency a crash *can* leave:
/// the persisted [`GameDownloadStatus`] disagreeing with what is actually on
/// disk. It runs once, early in `setup()`, before the frontend can read
/// status.
///
/// Rules applied to every game marked `Installed` (any `install_type`):
///   1. **Install directory missing** → demote to `Remote {}`. The game was
///      uninstalled out-of-band, or a crash mid-`Uninstalling` got far
///      enough to delete the directory.
///   2. **Directory present but `.dropdata` missing** → demote to
///      `PartiallyInstalled`. A `.dropdata`-less directory cannot be
///      validated or resumed safely, and a crash mid-uninstall that deleted
///      files but not the folder lands here. `PartiallyInstalled` lets the
///      user resume/repair rather than launch a broken game.
///
/// Any leftover transient statuses are also cleared — they are
/// `#[serde(skip)]` so a fresh load never has them, but clearing defends
/// against a future change that accidentally persists them.
pub fn reconcile_on_startup(db: &mut Database) -> ReconcileReport {
    let mut report = ReconcileReport::default();

    // 1. Transient statuses must never survive a restart.
    if !db.applications.transient_statuses.is_empty() {
        report.cleared_transient = db.applications.transient_statuses.len();
        warn!(
            "[game-status] reconcile: clearing {} stale transient status(es) at startup",
            report.cleared_transient
        );
        db.applications.transient_statuses.clear();
    }

    // 2. Cross-check every "installed" game against the filesystem.
    let game_ids: Vec<String> = db.applications.game_statuses.keys().cloned().collect();
    for game_id in game_ids {
        let Some(GameDownloadStatus::Installed {
            install_dir,
            version_id,
            install_type,
            update_available,
        }) = db.applications.game_statuses.get(&game_id)
        else {
            continue;
        };

        let install_dir = install_dir.clone();
        let version_id = version_id.clone();
        let update_available = *update_available;
        let dir_path = Path::new(&install_dir);

        if !dir_path.exists() {
            transition(&game_id, Some(StatusKind::from_persistent(
                &GameDownloadStatus::Installed {
                    install_dir: install_dir.clone(),
                    version_id: version_id.clone(),
                    install_type: install_type.clone(),
                    update_available,
                },
            )), StatusKind::Remote);
            warn!(
                "[game-status] reconcile: install dir for {game_id} is gone ({}), \
                 demoting Installed -> Remote",
                install_dir
            );
            db.applications
                .game_statuses
                .insert(game_id.clone(), GameDownloadStatus::Remote {});
            db.applications.installed_game_version.remove(&game_id);
            report.missing_dir.push(game_id);
            continue;
        }

        // Directory exists. If it is already PartiallyInstalled there is
        // nothing to downgrade — leave it for the user to resume.
        if matches!(install_type, InstalledGameType::PartiallyInstalled { .. }) {
            continue;
        }

        // A complete install always carries a `.dropdata` marker (written by
        // the download agent / scan). Its absence means the directory is not
        // a trustworthy install.
        let dropdata = dir_path.join(crate::downloads::drop_data::DROPDATA_PATH);
        if !dropdata.exists() {
            let prev = StatusKind::from_persistent(&GameDownloadStatus::Installed {
                install_dir: install_dir.clone(),
                version_id: version_id.clone(),
                install_type: install_type.clone(),
                update_available,
            });
            transition(&game_id, Some(prev), StatusKind::PartiallyInstalled);
            warn!(
                "[game-status] reconcile: {game_id} has install dir but no .dropdata, \
                 demoting {prev:?} -> PartiallyInstalled"
            );
            db.applications.game_statuses.insert(
                game_id.clone(),
                GameDownloadStatus::Installed {
                    install_type: InstalledGameType::PartiallyInstalled {
                        configuration: Default::default(),
                    },
                    version_id,
                    install_dir,
                    update_available,
                },
            );
            report.demoted_partial.push(game_id);
        }
    }

    if report.is_empty() {
        info!("[game-status] reconcile: all game states consistent at startup");
    } else {
        warn!(
            "[game-status] reconcile complete: {} missing dir, {} demoted to partial, \
             {} stale transient cleared",
            report.missing_dir.len(),
            report.demoted_partial.len(),
            report.cleared_transient
        );
    }

    report
}

/// Helper for the scan path: given a directory that has a readable
/// `.dropdata`, decide whether it is a trustworthy complete install.
///
/// Returns `true` only if every `expected_file` exists inside `install_dir`.
/// The scan does not have manifest checksums (those come from the server),
/// so this is a *presence* check — the strongest test possible offline.
/// A missing file means the directory is incomplete and the caller should
/// import it as `PartiallyInstalled`.
pub fn scanned_install_is_complete(install_dir: &Path, expected_files: &[String]) -> bool {
    for rel in expected_files {
        if !install_dir.join(rel).exists() {
            warn!(
                "[game-status] scan: '{}' expected by manifest but missing under {}",
                rel,
                install_dir.display()
            );
            return false;
        }
    }
    true
}

/// Build a `DownloadableMetadata` for a `Game`-type download. Small helper to
/// keep the scan path readable.
pub fn game_meta(
    id: String,
    version: String,
    platform: database::platform::Platform,
) -> DownloadableMetadata {
    DownloadableMetadata::new(id, version, platform, DownloadType::Game)
}
