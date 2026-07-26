use std::{path::PathBuf, sync::Arc};

use database::{
    DownloadType, DownloadableMetadata, GameDownloadStatus, PendingQueueEntry, borrow_db_checked,
    borrow_db_mut_checked,
    models::data::{InstalledGameType, UserConfiguration},
    platform::Platform,
};
use download_manager::{
    DOWNLOAD_MANAGER, downloadable::Downloadable, error::ApplicationDownloadError,
};
use games::downloads::download_agent::GameDownloadAgent;
use games::downloads::mod_agent::ModDownloadAgent;
use log::{info, warn};

/// Shared core for `download_game` (Tauri command) and `restore_pending_queue`
/// (startup recovery). Creates a `GameDownloadAgent` from the persisted shape
/// and queues it through the same path either caller would take.
///
/// `persist` controls whether the entry is appended to
/// `DatabaseApplications::pending_queue`: `true` for fresh Tauri-command
/// calls, `false` during restore (the entry's already there — re-appending
/// would duplicate it on every relaunch).
async fn enqueue_game_impl(
    game_id: String,
    version_id: String,
    target_platform: Platform,
    install_dir: usize,
    enable_updates: bool,
    persist: bool,
) -> Result<(), ApplicationDownloadError> {
    let sender = { DOWNLOAD_MANAGER.get_sender().clone() };

    let meta = DownloadableMetadata {
        id: game_id,
        version: version_id,
        target_platform,
        download_type: DownloadType::Game,
    };

    {
        let db = borrow_db_checked();

        // Already downloading or queued — don't double-queue.
        if db.applications.transient_statuses.get(&meta).is_some() {
            return Ok(());
        }

        // Already fully installed at this exact version — skip the re-download.
        // Re-running the download agent reconciles the install dir against the
        // server manifest and deletes anything not in it, which for a shared
        // standalone emulator (Eden/Yuzu/Cemu) means the player's saves: that
        // NAND/user data lives inside the install dir and is never in the
        // manifest. Installing a second game that depends on an
        // already-installed emulator must not re-trigger that sweep. A genuine
        // update targets a different version_id, and a partial/interrupted
        // install resumes via `resume_download`, so neither is skipped here.
        if let Some(install) = db.applications.get_install(&meta.id, &meta.version) {
            let complete = !matches!(
                install.install_type,
                InstalledGameType::PartiallyInstalled { .. }
            );
            if complete {
                info!(
                    "skipping download for {} — already installed at version {}",
                    meta.id, meta.version
                );
                return Ok(());
            }
        }
    };

    let configuration = UserConfiguration {
        enable_updates,
        ..Default::default()
    };

    let base_dir = {
        let db_lock = borrow_db_checked();

        db_lock
            .applications
            .install_dirs
            .get(install_dir)
            .cloned()
            .ok_or(ApplicationDownloadError::InvalidCommand)?
    };

    let game_download_agent = GameDownloadAgent::new(
        meta.clone(),
        base_dir,
        sender,
        DOWNLOAD_MANAGER.clone_depot_manager(),
        configuration,
    )
    .await?;

    let game_download_agent =
        Arc::new(Box::new(game_download_agent) as Box<dyn Downloadable + Send + Sync>);

    DOWNLOAD_MANAGER
        .queue_download(game_download_agent.clone())
        .await
        .map_err(|e| ApplicationDownloadError::ChannelBroken(e.to_string()))?;

    if persist {
        let mut db = borrow_db_mut_checked();
        db.applications.pending_queue.push(PendingQueueEntry {
            meta,
            install_dir,
            enable_updates,
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn download_game(
    game_id: String,
    version_id: String,
    target_platform: Platform,
    install_dir: usize,
    enable_updates: bool,
) -> Result<(), ApplicationDownloadError> {
    enqueue_game_impl(
        game_id,
        version_id,
        target_platform,
        install_dir,
        enable_updates,
        true,
    )
    .await
}

/// Download (or resume) a mod onto an installed base game. A mod is a Game with
/// `type = Mod`; its files overlay into the parent's install dir. Unlike
/// `download_game` this resolves the target dir from the PARENT's install
/// status (the base game must be fully installed) and never persists to the
/// startup pending-queue — a paused mod resumes from its `.moddata` ledger the
/// next time this command runs.
#[tauri::command]
pub async fn download_mod(
    mod_game_id: String,
    parent_game_id: String,
    version_id: String,
    target_platform: Platform,
    // Subdirectory (relative to the base game's install dir) to overlay into;
    // empty for the install root. From the mod version's modInstallDir.
    mod_install_dir: String,
    // Executable to launch while this mod is installed, or null. From the mod
    // version's launchOverride.
    launch_override: Option<String>,
) -> Result<(), ApplicationDownloadError> {
    let sender = { DOWNLOAD_MANAGER.get_sender().clone() };

    let meta = DownloadableMetadata {
        id: mod_game_id,
        version: version_id,
        target_platform,
        download_type: DownloadType::Mod,
    };

    let parent_install_dir = {
        let db = borrow_db_checked();

        // Already downloading or queued — don't double-queue.
        if db.applications.transient_statuses.get(&meta).is_some() {
            return Ok(());
        }

        // The base game must be fully installed; the mod overlays into its dir.
        match db.applications.game_statuses.get(&parent_game_id) {
            Some(GameDownloadStatus::Installed {
                install_type,
                install_dir,
                ..
            }) if !matches!(install_type, InstalledGameType::PartiallyInstalled { .. }) => {
                PathBuf::from(install_dir.clone())
            }
            _ => {
                warn!(
                    "cannot install mod {} — parent game {} is not fully installed",
                    meta.id, parent_game_id
                );
                return Err(ApplicationDownloadError::InvalidCommand);
            }
        }
    };

    let mod_download_agent = ModDownloadAgent::new(
        meta.clone(),
        parent_game_id,
        parent_install_dir,
        mod_install_dir,
        launch_override,
        sender,
        DOWNLOAD_MANAGER.clone_depot_manager(),
        UserConfiguration::default(),
    )
    .await?;

    let mod_download_agent =
        Arc::new(Box::new(mod_download_agent) as Box<dyn Downloadable + Send + Sync>);

    DOWNLOAD_MANAGER
        .queue_download(mod_download_agent)
        .await
        .map_err(|e| ApplicationDownloadError::ChannelBroken(e.to_string()))?;

    Ok(())
}

/// On startup, re-queue any downloads that were still pending at last
/// crash/exit. Each entry is fed back through the same `enqueue_game_impl`
/// path the Tauri command uses, just without re-persisting (the entry's
/// already in `pending_queue`). A per-entry failure is logged and skipped
/// so one broken game can't take the whole restore down.
///
/// Called from `lib.rs::setup` after auth confirms the user is signed in —
/// without a working token, `GameDownloadAgent::new` would fail to fetch
/// manifests from the server and the restore would all-fail anyway.
pub async fn restore_pending_queue() {
    let entries: Vec<PendingQueueEntry> = {
        let db = borrow_db_checked();
        db.applications.pending_queue.clone()
    };

    if entries.is_empty() {
        return;
    }

    info!(
        "restoring {} pending download(s) from last session",
        entries.len()
    );

    for entry in entries {
        let label = format!("{}@{}", entry.meta.id, entry.meta.version);
        if let Err(e) = enqueue_game_impl(
            entry.meta.id,
            entry.meta.version,
            entry.meta.target_platform,
            entry.install_dir,
            entry.enable_updates,
            false,
        )
        .await
        {
            warn!("could not restore queued download {}: {:?}", label, e);
        }
    }
}

#[tauri::command]
pub async fn resume_download(game_id: String) -> Result<(), ApplicationDownloadError> {
    let (meta, (install_dir, configuration)) = {
        let db_lock = borrow_db_checked();
        let status = db_lock
            .applications
            .game_statuses
            .get(&game_id)
            .ok_or(ApplicationDownloadError::InvalidCommand)?
            .clone();

        let meta = db_lock
            .applications
            .installed_game_version
            .get(&game_id)
            .ok_or(ApplicationDownloadError::InvalidCommand)?
            .clone();

        // A mod must never be resumed through this path: it would rebuild a
        // GameDownloadAgent over the parent's install dir and run the reconcile
        // sweep, deleting the whole base game. Mods resume via `download_mod`,
        // which uses the additive (no-sweep) ModDownloadAgent. In practice a mod
        // is never left PartiallyInstalled (see mod_agent's cancel/validate), so
        // the match below would reject it anyway — this is defense in depth.
        if meta.download_type == DownloadType::Mod {
            warn!("refusing to resume mod {game_id} as a game; use download_mod");
            return Err(ApplicationDownloadError::InvalidCommand);
        }

        let install_dir = match status {
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::PartiallyInstalled { configuration },
                install_dir,
                ..
            } => Ok((install_dir, configuration)),
            _ => Err(ApplicationDownloadError::InvalidCommand),
        }?;
        (meta, install_dir)
    };

    let sender = DOWNLOAD_MANAGER.get_sender();

    let install_dir = PathBuf::from(install_dir);
    let install_dir = install_dir
        .parent()
        .ok_or(ApplicationDownloadError::InvalidCommand)?;

    let game_download_agent = Arc::new(Box::new(
        GameDownloadAgent::new(
            meta,
            install_dir.to_path_buf(),
            sender,
            DOWNLOAD_MANAGER.clone_depot_manager(),
            configuration,
        )
        .await?,
    ) as Box<dyn Downloadable + Send + Sync>);

    DOWNLOAD_MANAGER
        .queue_download(game_download_agent)
        .await
        .map_err(|e| ApplicationDownloadError::ChannelBroken(e.to_string()))?;
    Ok(())
}
