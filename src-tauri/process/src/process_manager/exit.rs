//! Process-exit handling: the back half of a game's lifecycle.
//!
//! When a game's wait thread observes the process tree exit, it calls
//! [`ProcessManager::on_process_finish`]. That single function:
//!
//!   1. removes the [`RunningProcess`] from the process table,
//!   2. cancels the playtime heartbeat and achievement-poll tasks,
//!   3. reports the playtime stop and uploads any changed saves (async),
//!   4. **transitions the game out of `Running`** via [`games::status`] —
//!      a clean exit lands on `Installed`, mirroring the persistent status,
//!   5. fires a launch-failed event if the game died suspiciously fast.
//!
//! Step 4 is the important correctness fix: previously the transient
//! `Running` status was simply `remove`d from the DB with no logged,
//! validated transition. Routing it through [`games::status::transition`]
//! means a crash, a clean exit and a kill all produce a single greppable
//! `[game-status]` line and can never silently leave a game `Running`.

use std::{
    process::ExitStatus,
    sync::Arc,
    time::{Duration, Instant},
};

use database::{
    GameDownloadStatus, borrow_db_mut_checked, models::data::InstalledGameType,
};
use games::{
    library::push_game_update,
    state::GameStatusManager,
    status::{StatusKind, transition},
};
use log::{debug, info, warn};
use tauri::Emitter as _;
use tokio::sync::Notify;

use crate::{error::ProcessError, process_manager::ProcessManager};

/// A game that exits within this window (and was not killed by the user) is
/// treated as a failed launch — long enough to rule out an instant umu/Proton
/// failure, short enough that a real quick session isn't misflagged.
const SUSPICIOUS_EXIT_SECS: u64 = 2;

impl ProcessManager<'_> {
    /// Handle a game process exiting. Called from the per-launch wait thread.
    ///
    /// `result` is the `wait()` outcome — `Ok(status)` for a real exit (which
    /// may still be a non-zero crash code), `Err` if the wait syscall itself
    /// failed. Either way the game is removed from the running set and
    /// transitioned out of `Running`.
    pub(crate) fn on_process_finish(
        &mut self,
        game_id: String,
        result: Result<ExitStatus, std::io::Error>,
    ) -> Result<(), ProcessError> {
        let Some(process) = self.processes.remove(&game_id) else {
            // The wait thread can fire after kill cleanup already ran, or
            // for a game id that was never tracked — both are harmless.
            warn!(
                "[EXIT] on_process_finish for untracked game {game_id} \
                 (result: {result:?}) — nothing to clean up"
            );
            return Ok(());
        };

        let elapsed = process.start.elapsed();
        // Capture the kill flag before `process` is partially moved below.
        let manually_killed = process.manually_killed;
        let exit_kind = describe_exit(&result, manually_killed);
        info!(
            "[EXIT] game {game_id} exited after {}s — {exit_kind}",
            elapsed.as_secs()
        );

        // Notify listeners (streaming auto-stop) that the process is gone.
        let _ = self.app_handle.emit("game_process_exited", &game_id);

        // Stop the periodic playtime heartbeat and achievement polling.
        process.playtime_heartbeat_cancel.notify_one();
        if let Some(cancel) = &process.achievement_poll_cancel {
            cancel.notify_one();
        }

        // Report playtime stop, trigger achievement sync, upload saves —
        // all off-thread so a slow network never blocks process cleanup.
        // Consumes the per-process fields it needs (the process is already
        // out of the table, so moving them is safe).
        spawn_post_exit_sync(
            process.playtime_session_id,
            process.save_snapshot,
            &game_id,
            elapsed,
        );

        // ── Status + DB cleanup ────────────────────────────────────────────
        let mut db_handle = borrow_db_mut_checked();
        let Some(meta) = db_handle
            .applications
            .installed_game_version
            .get(&game_id)
            .cloned()
        else {
            warn!("[EXIT] no installed version for {game_id}; skipping status cleanup");
            return Ok(());
        };

        // Route the Running -> Installed transition through the central
        // state machine so it is logged and validated. `from` is read from
        // the live DB state before we mutate it.
        let from = StatusKind::from_state(&GameStatusManager::fetch_state(&game_id, &db_handle));
        transition(&game_id, from, StatusKind::Installed);

        // Drop the transient `Running` status — this is what actually moves
        // the game back to its persistent status in the UI.
        db_handle.applications.transient_statuses.remove(&meta);

        // A clean exit from a SetupRequired install means setup completed —
        // promote it to a normal Installed game.
        if let Some(GameDownloadStatus::Installed { install_type, .. }) =
            db_handle.applications.game_statuses.get_mut(&game_id)
            && matches!(result, Ok(ref code) if code.success())
            && matches!(install_type, InstalledGameType::SetupRequired)
        {
            info!("[EXIT] {game_id}: clean exit from SetupRequired — marking Installed");
            *install_type = InstalledGameType::Installed;
        }

        // ── Suspicious-exit detection ──────────────────────────────────────
        // A fast exit or a non-zero code (that the user did not trigger)
        // signals the game failed to launch — surface it to the UI.
        let crashed = result.as_ref().map_or(true, |r| !r.success());
        if !manually_killed && (elapsed.as_secs() <= SUSPICIOUS_EXIT_SECS || crashed) {
            warn!("[EXIT] {game_id} likely failed to launch ({exit_kind})");
            // Legacy string-payload event (desktop modal listener).
            let _ = self.app_handle.emit("launch_external_error", &game_id);
            // Detailed event for the BPM error dialog.
            let _ = self.app_handle.emit(
                "launch_external_error_detail",
                serde_json::json!({
                    "gameId": &game_id,
                    "exitCode": result.as_ref().ok().and_then(|s| s.code()),
                    "elapsedSecs": elapsed.as_secs(),
                    "ioError": result.as_ref().err().map(|e| e.to_string()),
                }),
            );
        }

        // Push the post-exit status to the frontend.
        let version_data = db_handle
            .applications
            .game_versions
            .get(&meta.version)
            .cloned();
        if version_data.is_none() {
            warn!(
                "[EXIT] game_versions missing version {} (game {game_id}); \
                 pushing status update without version",
                meta.version
            );
        }
        let status = GameStatusManager::fetch_state(&game_id, &db_handle);
        drop(db_handle);

        push_game_update(&self.app_handle, &game_id, version_data, status);
        Ok(())
    }
}

/// Human-readable one-liner describing how a process ended, used in logs and
/// to distinguish a crash from a clean exit from a user kill.
fn describe_exit(result: &Result<ExitStatus, std::io::Error>, manually_killed: bool) -> String {
    if manually_killed {
        return "killed by user".to_string();
    }
    match result {
        Ok(status) if status.success() => "clean exit (code 0)".to_string(),
        Ok(status) => match status.code() {
            Some(code) => format!("CRASH/non-zero exit (code {code})"),
            // No code on Unix => terminated by a signal.
            None => "CRASH (terminated by signal)".to_string(),
        },
        Err(e) => format!("wait() failed: {e}"),
    }
}

/// Spawn the async post-exit work: report the playtime stop, notify the
/// server the session ended, and upload any saves that changed. Split out so
/// [`ProcessManager::on_process_finish`] stays focused on state cleanup.
///
/// Takes ownership of the session-id slot and save snapshot — a fast-exiting
/// game can reach here before the async start task has stored the id, so the
/// task below retries reading it; moving the `Arc<Mutex>` itself (rather than
/// snapshotting the `Option`) preserves that race resolution.
fn spawn_post_exit_sync(
    session_slot: Arc<std::sync::Mutex<Option<String>>>,
    snapshot: Option<crate::process_manager::SaveSyncSnapshot>,
    game_id: &str,
    elapsed: Duration,
) {
    let sync_game_id = game_id.to_string();
    let actual_duration_secs = elapsed.as_secs() as u32;

    tauri::async_runtime::spawn(async move {
        // start_playtime can take up to ~7s when retrying, but the first
        // attempt usually lands sub-second — wait ~3s for the id.
        match wait_for_session_id(&session_slot, Duration::from_secs(3)).await {
            Some(session_id) => {
                if let Err(e) =
                    remote::playtime::stop_playtime(&session_id, Some(actual_duration_secs)).await
                {
                    // In-process retries exhausted — persist so the next
                    // launch can retry instead of dropping the playtime.
                    warn!(
                        "[EXIT] playtime stop failed after retries; queuing for later: {e}"
                    );
                    remote::playtime::queue_pending_stop(&session_id, actual_duration_secs);
                }
            }
            None => warn!(
                "[EXIT] playtime stop skipped for {sync_game_id} ({actual_duration_secs}s): \
                 session_id never populated (start_playtime failed or game exited too fast)"
            ),
        }

        if let Err(e) = remote::achievements::notify_session_end(&sync_game_id).await {
            warn!("[EXIT] failed to notify session end for {sync_game_id}: {e}");
        }

        if let Some(snap) = snapshot {
            upload_changed_saves_for(&snap).await;
        }
    });
}

/// Upload whatever saves changed during the session, comparing against the
/// pre-launch snapshot, then update the local manifest.
async fn upload_changed_saves_for(snap: &crate::process_manager::SaveSyncSnapshot) {
    // Belt-and-braces gate. The pre-launch path already short-circuits when
    // cloud_saves_enabled=false (no snapshot is produced), so this branch is
    // normally unreachable — but if a snapshot was created and the user
    // toggled the setting off mid-session, we honour that here too.
    if !database::borrow_db_checked().settings.cloud_saves_enabled {
        info!(
            "[SAVE-SYNC] cloud_saves_enabled=false — skipping post-exit upload for {}",
            snap.game_id
        );
        return;
    }

    let mut current_saves = Vec::new();
    if let Some(emu_root) = &snap.emu_root {
        current_saves.extend(remote::save_sync::scan_emu_saves(emu_root, &snap.game_id));
    }
    if let Some(name) = &snap.game_name {
        current_saves.extend(remote::save_sync::scan_pc_saves(
            name,
            None,
            snap.wine_prefix.as_deref(),
        ));
    }

    match remote::save_sync::upload_changed_saves(
        &snap.game_id,
        &snap.pre_hashes,
        &current_saves,
    )
    .await
    {
        Ok((count, errors)) => {
            if count > 0 {
                info!("[SAVE-SYNC] Uploaded {count} saves for game {}", snap.game_id);
            }
            for err in &errors {
                warn!("[SAVE-SYNC] Upload error: {err}");
            }
            // Persist the final synced state.
            let mut manifest = remote::save_sync::load_manifest(&snap.game_id);
            for file in &current_saves {
                manifest.files.insert(
                    file.filename.clone(),
                    remote::save_sync::SyncFileEntry {
                        save_type: file.save_type.clone(),
                        synced_hash: file.data_hash.clone(),
                        cloud_id: None,
                        synced_at: chrono::Utc::now().to_rfc3339(),
                    },
                );
            }
            manifest.last_synced_at = Some(chrono::Utc::now().to_rfc3339());
            if let Err(e) = remote::save_sync::save_manifest(&manifest) {
                warn!("[SAVE-SYNC] Failed to save manifest: {e}");
            }
        }
        Err(e) => warn!("[SAVE-SYNC] Post-exit sync failed: {e}"),
    }
}

/// Poll the playtime session-id mutex up to `timeout`, returning the id as
/// soon as it is populated. Used by the stop path to dodge the race where a
/// game exits before the async `start_playtime` task has stored the id.
///
/// Poll interval is 100ms — fast enough that a typical sub-second start
/// barely delays the stop, slow enough that the wait doesn't burn CPU.
pub(crate) async fn wait_for_session_id(
    slot: &Arc<std::sync::Mutex<Option<String>>>,
    timeout: Duration,
) -> Option<String> {
    let start = Instant::now();
    loop {
        if let Ok(guard) = slot.lock()
            && let Some(id) = guard.clone()
        {
            return Some(id);
        }
        if start.elapsed() >= timeout {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Send `heartbeat_playtime` for the given session every five minutes until
/// `cancel` is notified. Stops cleanly on cancellation. Each heartbeat is
/// best-effort — `heartbeat_playtime` swallows network errors internally so
/// a flaky connection doesn't kill the loop.
///
/// Five minutes balances two costs: each heartbeat is a network round-trip
/// (so not every few seconds), but the worst-case orphaned-session error is
/// "duration assumed to be the gap between start and last heartbeat" — i.e.
/// up to 5 minutes of overcount. Reasonable for an unattended-quit edge case.
pub(crate) async fn run_playtime_heartbeat_loop(session_id: String, cancel: Arc<Notify>) {
    const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5 * 60);
    let mut ticker = tokio::time::interval(HEARTBEAT_INTERVAL);
    // Skip the immediate tick — we just started, no need to heartbeat yet.
    ticker.tick().await;

    loop {
        tokio::select! {
            _ = cancel.notified() => {
                debug!("[EXIT] playtime heartbeat loop cancelled for session {session_id}");
                return;
            }
            _ = ticker.tick() => {
                let _ = remote::playtime::heartbeat_playtime(&session_id).await;
            }
        }
    }
}
