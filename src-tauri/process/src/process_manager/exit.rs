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
        // The exact version that was launched — used to clear the right
        // transient status and report the right version, since the running
        // version may not be the game's current install (multi-version).
        let meta = process.meta.clone();
        let exit_kind = describe_exit(&result, manually_killed);
        info!(
            "[EXIT] game {game_id} exited after {}s — {exit_kind}",
            elapsed.as_secs()
        );

        // Notify listeners (streaming auto-stop) that the process is gone.
        let _ = self.app_handle.emit("game_process_exited", &game_id);

        // RetroAchievements expiry check. Runs before the DB write below —
        // it takes the write lock itself, and the lock is not reentrant.
        if let Some(ra) = &process.retroarch {
            check_ra_credentials(&self.app_handle, ra);
        }

        // Read RetroArch's own log for a fatal video-driver failure before the
        // suspicious-exit report below, so the report can carry it.
        let retroarch_video_failure = process
            .retroarch
            .as_ref()
            .filter(|_| !manually_killed)
            .and_then(|ra| {
                remote::retroarch::detect_fatal_video_error(&ra.emu_root, ra.launched_at)
                    .map(|line| (line, ra.video_driver.clone()))
            });

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
            self.app_handle.clone(),
            process.playtime_session_id,
            process.save_snapshot,
            &game_id,
            elapsed,
        );

        // ── Status + DB cleanup ────────────────────────────────────────────
        let mut db_handle = borrow_db_mut_checked();

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
            if let Some((line, driver)) = &retroarch_video_failure {
                warn!(
                    "[EXIT] {game_id}: RetroArch could not start its video driver \
                     (video_driver={driver:?}): {line}"
                );
            }
            // Legacy string-payload event (desktop modal listener).
            let _ = self.app_handle.emit("launch_external_error", &game_id);
            // Detailed event for the launch-failure dialogs on both surfaces.
            let _ = self.app_handle.emit(
                "launch_external_error_detail",
                serde_json::json!({
                    "gameId": &game_id,
                    "exitCode": result.as_ref().ok().and_then(|s| s.code()),
                    "elapsedSecs": elapsed.as_secs(),
                    "ioError": result.as_ref().err().map(|e| e.to_string()),
                    "retroarchVideoError": retroarch_video_failure
                        .as_ref()
                        .map(|(line, _)| line.clone()),
                    "retroarchVideoDriver": retroarch_video_failure
                        .as_ref()
                        .and_then(|(_, driver)| driver.clone()),
                }),
            );
        }

        // Auto-report this launch's outcome to the compat dataset (source
        // "launch") so real plays build the per-version, multiplayer-aware
        // compatibility picture — not just the test worker. Fire-and-forget;
        // the main-crate listener turns this into the actual POST.
        let _ = self.app_handle.emit(
            "game_launch_outcome",
            serde_json::json!({
                "gameId": &game_id,
                "versionId": &meta.version,
                "exitCode": result.as_ref().ok().and_then(|s| s.code()),
                "terminatedBySignal": result
                    .as_ref()
                    .map(|s| s.code().is_none())
                    .unwrap_or(false),
                "manuallyKilled": manually_killed,
                "elapsedSecs": elapsed.as_secs(),
                "waitFailed": result.is_err(),
            }),
        );

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

/// Read RetroArch's log for a RetroAchievements login rejection and, if the
/// token was refused, record it and tell the frontend.
///
/// RA's Connect token expires after roughly 45 to 60 days and cannot be
/// refreshed. RetroArch reports the rejection only in its own log — the game
/// still runs, achievements just never unlock — so this is the only moment
/// Drop can notice. Recording the dead token stops the next launch injecting
/// it again, which is what previously made expiry unrecoverable by hand.
///
/// Only a log this session wrote counts, hence `ra.launched_at`: a rejection
/// left behind by an earlier session would otherwise be re-read on every exit
/// and re-pinned to whatever token is current, which no re-link could clear.
fn check_ra_credentials(
    app_handle: &tauri::AppHandle,
    ra: &crate::process_manager::RetroArchSession,
) {
    // No token injected means no token to be rejected — a session for a user
    // who never linked an account must not latch the expiry flag.
    let Some(connect_token) = &ra.connect_token else {
        return;
    };
    let Some(line) = remote::retroarch::detect_ra_login_failure(&ra.emu_root, ra.launched_at)
    else {
        return;
    };

    warn!(
        "[EXIT] RetroAchievements rejected the Connect token — it has expired \
         and cannot be refreshed. RetroArch said: {line}"
    );
    remote::retroarch::mark_credentials_expired(connect_token);
    let _ = app_handle.emit(
        "ra_credentials_expired",
        serde_json::json!({ "reason": line }),
    );
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
    app_handle: tauri::AppHandle,
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
        //
        // In incognito mode no `start_playtime` was ever launched, so the
        // slot will time out empty. We use that absence as the signal to
        // also skip the achievement-session-end notify below: an incognito
        // launch leaves zero server-side traces.
        let session_id = wait_for_session_id(&session_slot, Duration::from_secs(3)).await;
        match &session_id {
            Some(sid) => {
                if let Err(e) =
                    remote::playtime::stop_playtime(sid, Some(actual_duration_secs)).await
                {
                    // In-process retries exhausted — persist so the next
                    // launch can retry instead of dropping the playtime.
                    warn!(
                        "[EXIT] playtime stop failed after retries; queuing for later: {e}"
                    );
                    remote::playtime::queue_pending_stop(sid, actual_duration_secs);
                }
            }
            None => log::info!(
                "[EXIT] no playtime session for {sync_game_id} ({actual_duration_secs}s) — \
                 incognito launch, start_playtime failure, or fast-exit before start landed"
            ),
        }

        // Achievement session-end notify only matters if we opened a session.
        // Otherwise (incognito, or start_playtime never landed) there's
        // nothing on the server side to reconcile.
        if session_id.is_some()
            && let Err(e) = remote::achievements::notify_session_end(&sync_game_id).await {
                warn!("[EXIT] failed to notify session end for {sync_game_id}: {e}");
            }

        if let Some(snap) = snapshot {
            upload_changed_saves_for(&app_handle, &snap).await;
        }
    });
}

/// The two conditions the post-exit upload needs, both of which are about not
/// destroying something irreplaceable.
///
/// `cloud_saves_enabled` is a belt-and-braces re-check: the pre-launch path
/// already produces no snapshot when the setting is off, so it only matters if
/// the user toggled it off mid-session.
///
/// `synced_ok` is the one that prevents cross-device loss. The exit path
/// uploads every file whose hash differs from `pre_hashes`, but when the
/// pre-launch sync-check or download failed or timed out, `pre_hashes` is just
/// "what happened to be on this disk" — not a baseline the cloud agreed to.
/// Uploading against it pushes stale local bytes over whatever another machine
/// wrote, so one slow connection here would eat a save made somewhere else.
/// The session's changes stay on disk and the next successful sync surfaces
/// them as a normal conflict instead.
fn post_exit_upload_allowed(cloud_saves_enabled: bool, synced_ok: bool) -> bool {
    cloud_saves_enabled && synced_ok
}

/// Upload whatever saves changed during the session, comparing against the
/// pre-launch snapshot, then update the local manifest.
async fn upload_changed_saves_for(
    app_handle: &tauri::AppHandle,
    snap: &crate::process_manager::SaveSyncSnapshot,
) {
    use crate::process_manager::save_sync::{
        PHASE_UPLOAD, backed_up_message, describe_failure, emit_sync_complete, emit_sync_error,
        emit_upload_failures,
    };

    let enabled = database::borrow_db_checked().settings.cloud_saves_enabled;
    if !post_exit_upload_allowed(enabled, snap.synced_ok) {
        if !enabled {
            info!(
                "[SAVE-SYNC] cloud_saves_enabled=false — skipping post-exit upload for {}",
                snap.game_id
            );
        } else {
            // The one skip the user has to hear about: their session ended and
            // nothing was backed up, for a reason that happened minutes ago at
            // launch and left no trace on screen.
            emit_sync_error(
                app_handle,
                &snap.game_id,
                PHASE_UPLOAD,
                "This session's saves were not backed up, because the sync before launch did \
                 not finish. Your progress is still on this PC. Back it up from the game's \
                 Cloud Saves panel.",
                true,
            );
        }
        return;
    }

    let mut current_saves = Vec::new();
    if let Some(emu_root) = &snap.emu_root {
        current_saves.extend(remote::save_sync::scan_emu_saves(
            emu_root,
            Some(&snap.user_id),
            &snap.game_id,
            snap.switch_title_id.as_deref(),
        ));
    }
    if let Some(name) = &snap.game_name {
        current_saves.extend(remote::save_sync::scan_pc_saves(
            name,
            None,
            snap.wine_prefix.as_deref(),
        ));
    }

    // Ask what will fit before pushing it. The 413 the server would otherwise
    // return arrives after the bytes have crossed the wire and after the only
    // moment the user could have freed some room, and it lands as "this session
    // was not backed up" with no warning that it was coming.
    //
    // Per file, not per batch: the server stores everything that fits and
    // rejects only the rest, so refusing the whole session over one oversized
    // save would leave progress on this disk that the server would have taken.
    let changed = remote::save_sync::changed_files(&snap.pre_hashes, &current_saves);
    let plan = remote::save_sync::preflight_quota(&snap.game_id, &changed).await;
    if let Some(message) = &plan.message {
        emit_sync_error(app_handle, &snap.game_id, PHASE_UPLOAD, message, false);
    }
    if plan.nothing_fits() {
        return;
    }
    let skipped: std::collections::HashSet<String> = plan.skipped.iter().cloned().collect();
    let uploadable: Vec<remote::save_sync::LocalSaveFile> = current_saves
        .iter()
        .filter(|f| !skipped.contains(&f.filename))
        .cloned()
        .collect();

    match remote::save_sync::upload_changed_saves(
        &snap.game_id,
        &snap.pre_hashes,
        &uploadable,
    )
    .await
    {
        Ok((uploaded, failures)) => {
            // The one moment in the whole feature that says "your progress is
            // safe". It was an info! line in a log nobody reads.
            if let Some(message) = backed_up_message(uploaded.len()) {
                emit_sync_complete(
                    app_handle,
                    &snap.game_id,
                    PHASE_UPLOAD,
                    uploaded.len(),
                    &message,
                );
            }
            // Denominator is what we actually tried to push (the files that
            // changed this session), not every save on disk — "2 of 200"
            // reads like a disaster when 198 of them simply did not change.
            emit_upload_failures(
                app_handle,
                &snap.game_id,
                &failures,
                uploaded.len() + failures.len(),
                "after this session",
            );
            // Persist the final synced state via the shared recorder, which
            // skips files that only appear in `failures` and stamps the rest
            // with the cloud id the server just handed back.
            let cloud_ids: std::collections::HashMap<String, String> =
                uploaded.into_iter().collect();
            // Quota-skipped files belong here for the same reason server
            // rejections do: they are not in the cloud, and stamping them with
            // this session's hash would stop every future session retrying them.
            let mut unsynced: Vec<String> =
                failures.iter().map(|f| f.filename.clone()).collect();
            unsynced.extend(plan.skipped.iter().cloned());
            let mut manifest =
                remote::save_sync::load_manifest(&snap.user_id, &snap.game_id);
            remote::save_sync::record_synced_files(
                &mut manifest,
                &current_saves,
                &cloud_ids,
                &unsynced,
            );
            if let Err(e) = remote::save_sync::save_manifest(&manifest) {
                warn!("[SAVE-SYNC] Failed to save manifest: {e}");
            }
        }
        Err(e) => {
            let (message, retryable) = describe_failure(&e.to_string());
            emit_sync_error(
                app_handle,
                &snap.game_id,
                PHASE_UPLOAD,
                &format!("This session's saves could not be backed up. {message}"),
                retryable || e.is_retryable(),
            );
        }
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

/// Send `heartbeat_playtime` for the given session every 60 seconds until
/// `cancel` is notified. Stops cleanly on cancellation. Each heartbeat is
/// best-effort — `heartbeat_playtime` swallows network errors internally so
/// a flaky connection doesn't kill the loop.
///
/// The 60s interval is intentionally well below the server's 5-minute
/// `STALE_AFTER_MS` cutoff for the now-playing endpoint, so a single dropped
/// heartbeat (network blip) can't push the session past the stale window and
/// silently disappear from the community presence strip. With this margin a
/// session can lose up to four heartbeats in a row before going stale —
/// robust to typical residential wifi hiccups while keeping the round-trip
/// rate low. Orphan-cleanup overcount worst case shrinks correspondingly to
/// at most 60s.
pub(crate) async fn run_playtime_heartbeat_loop(session_id: String, cancel: Arc<Notify>) {
    const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
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

#[cfg(test)]
mod tests {
    use super::post_exit_upload_allowed;

    /// A pre-launch sync that never completed leaves `pre_hashes` describing
    /// this disk rather than the cloud, so uploading against it overwrites
    /// another device's row. Losing this upload is recoverable; that is not.
    #[test]
    fn an_incomplete_pre_launch_sync_blocks_the_upload() {
        assert!(post_exit_upload_allowed(true, true));
        assert!(!post_exit_upload_allowed(true, false));
        assert!(!post_exit_upload_allowed(false, true));
        assert!(!post_exit_upload_allowed(false, false));
    }
}
