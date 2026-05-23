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
        let status = db.applications.transient_statuses.get(&meta);

        if status.is_some() {
            return Ok(());
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
        .expect("game somehow installed at root");

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
