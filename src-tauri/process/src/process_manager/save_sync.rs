//! Pre-launch cloud-save synchronisation.
//!
//! Before a game starts, Drop pulls down any cloud saves newer than the
//! local copies and, if both sides changed, asks the user to resolve the
//! conflict. After the game exits, [`super::exit`] uploads whatever changed.
//!
//! Two discovery strategies exist:
//!
//!   - **Emulator games** ([`sync_emulator_saves`]) — saves live under the
//!     emulator install dir, found by [`remote::save_sync::scan_emu_saves`].
//!   - **PC/native games** ([`sync_pc_saves`]) — saves are scattered across
//!     the OS, discovered via Ludusavi keyed on the game's display name.
//!
//! Both produce a [`SaveSyncSnapshot`] of post-download file hashes; the
//! exit path diffs against it to decide what to upload. This module is split
//! out of the launch flow purely for size — the conflict-resolution dance
//! (emit event → block on an mpsc channel → apply choices) is long and was
//! drowning the actual process-spawn logic in `launch.rs`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use log::{info, warn};
use tauri::{AppHandle, Emitter as _};

use crate::process_manager::SaveSyncSnapshot;

/// How long a blocking save-sync network call may run before we give up and
/// launch the game anyway. These run on the launch thread with no lock held
/// (see [`super::launch::run_launch`]), but a flaky connection still must not
/// be able to stall a launch indefinitely.
const SYNC_NET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const UPLOAD_NET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Downloads get their own, much longer budget. `bulk_download` pulls up to a
/// 512 MiB base64 response and 10s could not finish a single large PC save, so
/// the pull timed out on exactly the saves that mattered most. A timeout here
/// now marks the snapshot `synced_ok = false`, so the worst case is a launch
/// that waited and synced nothing rather than one that overwrites the cloud.
const DOWNLOAD_NET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// How long we wait for the user to resolve a save conflict in the UI.
///
/// This was 300s, from when a timeout meant "keep all local" and waiting the
/// full five minutes was better than silently overwriting the cloud. A timeout
/// now syncs nothing, and the dialog shows this as a countdown, so the only
/// thing a long wait buys is a launch that appears hung. 45s is enough to read
/// two file sizes and pick one.
const CONFLICT_RESOLVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

// ── User-facing failure reporting ──────────────────────────────────────
//
// Every failure below used to be a bare `warn!` into drop.log. Cloud saves is
// the one feature where silence is indistinguishable from success: the
// manifest said "synced", the panel said "backed up", and the only record that
// nothing had crossed the wire was one line in a 58 MB log nobody reads.

/// Phase labels for the `save_sync_error` event. Short and stable so the UI
/// can group on them without parsing the message.
const PHASE_CHECK: &str = "check";
pub(crate) const PHASE_UPLOAD: &str = "upload";
const PHASE_DOWNLOAD: &str = "download";
const PHASE_WRITE: &str = "write";
const PHASE_CONFLICT: &str = "conflict";

/// This PC has no working network at all. Distinct from
/// [`MSG_SERVER_UNREACHABLE`] because the two need different things done: a
/// laptop off wifi is the user's own connection, a server that is down is
/// somebody's machine to go and start.
const MSG_OFFLINE: &str =
    "This PC is not on a network, so this game's saves were not synced. Your progress is safe \
     on this PC. Reconnect, then press Sync in the game's Cloud Saves panel.";

/// The network is up but the Drop server did not answer.
const MSG_SERVER_UNREACHABLE: &str =
    "Drop could not reach your server, so this game's saves were not synced. Your progress is \
     safe on this PC. Check the server is running, then press Sync in the game's Cloud Saves \
     panel.";

/// The user is out of cloud storage. Names the two things that actually free
/// some up: their own deletes, and their server admin.
const MSG_QUOTA_FULL: &str =
    "Your cloud save storage is full, so this save was not backed up. Delete saves you no longer \
     need from a game's Cloud Saves panel, or ask whoever runs your Drop server for more space.";

/// THIS PC ran out of disk. Emphatically not the quota message: these two used
/// to share a branch, so an ENOSPC while writing a restored save told the user
/// their *server* storage was full and sent them off to delete cloud saves,
/// which would not have freed a single byte on the disk that was actually
/// full.
const MSG_DISK_FULL: &str =
    "This PC has run out of disk space, so the save could not be written. Free up space on this \
     drive, then press Sync in the game's Cloud Saves panel.";

/// PC save discovery needs Ludusavi and Drop does not bundle it. Without this
/// the scan returns an empty list, which is indistinguishable from "this game
/// has no saves", so the feature simply never did anything and never said why.
const MSG_LUDUSAVI_MISSING: &str =
    "Drop needs Ludusavi to find where PC games keep their save files, and it is not installed, \
     so nothing was backed up for this game. Open the game's Cloud Saves panel and choose \
     Install Ludusavi.";

/// A download that ran out of time is worth its own line: the game launched on
/// whatever this PC already had, and nothing will be uploaded afterwards, so
/// the cloud copy is intact and the two are simply out of step.
const MSG_DOWNLOAD_TIMEOUT: &str =
    "Downloading this game's cloud saves took too long, so it started with the copy already on \
     this PC. Nothing was uploaded afterwards, so the cloud copy is untouched.";

/// Tell the frontend a save-sync step failed. Also logs, so drop.log keeps the
/// full picture for anyone reading it after the fact.
pub(crate) fn emit_sync_error(
    app: &AppHandle,
    game_id: &str,
    phase: &str,
    message: &str,
    retryable: bool,
) {
    warn!("[SAVE-SYNC] {phase} failed for {game_id}: {message}");
    let _ = app.emit(
        "save_sync_error",
        serde_json::json!({
            "gameId": game_id,
            "phase": phase,
            "message": message,
            "retryable": retryable,
        }),
    );
}

const MSG_SIGNED_OUT: &str =
    "Drop is signed out, so saves cannot be synced. Sign in again, then relaunch the game.";

/// Turn a raw failure into something a user can act on, plus whether trying
/// again could plausibly help.
///
/// Matching is on substrings because the three sources have no common type:
/// the server returns free text in its per-file `errors[]`, `std::io::Error`
/// formats its own, and `RemoteAccessError` already renders a sentence.
///
/// ORDER IS THE WHOLE DESIGN HERE. Every arm below is reachable from text that
/// would also match a later arm, and the earlier one is the cause the user can
/// act on. Two cases in particular are why this function is written out longhand
/// instead of collapsed:
///
///   * "not enough space" is a *local disk*, not the cloud quota. Both used to
///     share one branch, so running out of room on this PC told the user their
///     server storage was full.
///   * "network is unreachable" is this PC being off the network; "failed to
///     connect" is the server not answering. One is a wifi icon, the other is a
///     machine to go and start.
pub(crate) fn describe_failure(raw: &str) -> (String, bool) {
    let low = raw.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| low.contains(n));

    // A per-file size rejection is not a quota problem, and it was being
    // reported as one. The server's own message is "File too large (max
    // 100MB)"; telling the user to free space on the server will never let a
    // 120 MB save through. The real quota error reads "Save quota exceeded",
    // which the "quota" needle below already catches.
    if has(&["too large"]) {
        return (
            "This save is bigger than the server will accept, so it was not backed up."
                .to_string(),
            false,
        );
    }
    // Deliberately narrow. A bare "ludusavi" needle would also swallow a
    // Ludusavi that IS installed and failed to run, and tell the user to go
    // install the thing they already have.
    if has(&[
        "ludusavi not found",
        "ludusavi is not installed",
        "install ludusavi",
    ]) {
        return (MSG_LUDUSAVI_MISSING.to_string(), false);
    }
    // Local disk, checked before the quota arm: Windows ERROR_DISK_FULL is
    // "There is not enough space on the disk. (os error 112)" and POSIX ENOSPC
    // is "No space left on device (os error 28)". Neither says "disk".
    if has(&[
        "no space left",
        "not enough space",
        "os error 112",
        "os error 28",
        "disk full",
        "disk is full",
    ]) {
        return (MSG_DISK_FULL.to_string(), false);
    }
    if has(&["quota", "storage limit", "insufficient storage"]) {
        return (MSG_QUOTA_FULL.to_string(), false);
    }
    if has(&[
        "used by another process",
        "os error 32",
        "sharing violation",
        "resource busy",
        "text file busy",
    ]) {
        return (
            "The game still has that save file open, so Drop could not read it. Close the game, \
             then back the save up from its Cloud Saves panel."
                .to_string(),
            true,
        );
    }
    if has(&[
        "permission denied",
        "access is denied",
        "os error 5",
        "os error 13",
        "read-only",
    ]) {
        return (
            "Drop was not allowed to read or write that save file. Check the file's permissions, \
             then try again."
                .to_string(),
            false,
        );
    }
    if has(&["sign in", "no longer valid", "unauthori"]) {
        return (MSG_SIGNED_OUT.to_string(), false);
    }
    // Offline before unreachable: "network is unreachable" would otherwise be
    // caught by the "unreachable" needle below and blamed on the server.
    if has(&[
        "network is unreachable",
        "no such host",
        "failed to lookup address",
        "nodename nor servname",
        "dns",
        "offline",
        "os error 11001",
    ]) {
        return (MSG_OFFLINE.to_string(), true);
    }
    if has(&[
        "connect",
        "unreachable",
        "refused",
        "timed out",
        "timeout",
        "unavailable",
        "network",
    ]) {
        return (MSG_SERVER_UNREACHABLE.to_string(), true);
    }
    (raw.to_string(), false)
}

/// [`describe_failure`] for a typed server error.
///
/// The taxonomy answers two of these questions better than any substring can,
/// so they are read off the variant rather than the rendered sentence: whether
/// the user has to sign in again, and whether a retry is worth anything.
fn describe_remote_error(e: &remote::error::RemoteAccessError) -> (String, bool) {
    use remote::error::RemoteAccessError as E;
    if e.is_auth_error() {
        return (MSG_SIGNED_OUT.to_string(), false);
    }
    if matches!(e, E::ServerUnavailable(_) | E::Timeout) {
        return (MSG_SERVER_UNREACHABLE.to_string(), true);
    }
    let (message, retryable) = describe_failure(&e.to_string());
    (message, retryable || e.is_retryable())
}

/// One event for a batch of per-file upload rejections. Emitting one per file
/// would put a modal on screen for every save in a Switch title's NAND.
pub(crate) fn emit_upload_failures(
    app: &AppHandle,
    game_id: &str,
    failures: &[remote::save_sync::UploadFailure],
    total: usize,
    when: &str,
) {
    let Some(first) = failures.first() else {
        return;
    };
    let (reason, retryable) = describe_failure(&first.error);
    let message = format!(
        "{} of {} saves could not be backed up {}. {}",
        failures.len(),
        total,
        when,
        reason
    );
    emit_sync_error(app, game_id, PHASE_UPLOAD, &message, retryable);
}

// ── Confirming that something worked ───────────────────────────────────
//
// "Backed up 3 saves for Tony Hawk's Pro Skater 3+4" was an `info!` line in
// drop.log. It is the single most reassuring moment the feature has, and for a
// feature people have to trust with irreplaceable files, a backup nobody is
// told about is worth very little more than no backup at all.

/// Games already told about a missing Ludusavi during this app run.
///
/// The install is a one-off the user has to go and do; nagging on every launch
/// of every PC game is how a warning becomes wallpaper. Cleared on restart,
/// which is also when a fresh install would be picked up.
static LUDUSAVI_WARNED: std::sync::Mutex<Option<std::collections::HashSet<String>>> =
    std::sync::Mutex::new(None);

/// True the first time this game hits a missing Ludusavi this run.
fn warn_ludusavi_once(game_id: &str) -> bool {
    let mut guard = match LUDUSAVI_WARNED.lock() {
        Ok(g) => g,
        // A poisoned mutex here would mean losing the warning entirely. Saying
        // it again is the harmless side of this.
        Err(e) => e.into_inner(),
    };
    guard
        .get_or_insert_with(std::collections::HashSet::new)
        .insert(game_id.to_string())
}

/// The display name Drop knows this game by, for the confirmation toast.
///
/// Read from the same cached `Game` object the PC sync path already resolves
/// its Ludusavi search name from. `None` for a game that has never been
/// fetched, and the toast then just omits the name rather than inventing one.
pub(crate) fn game_display_name(game_id: &str) -> Option<String> {
    remote::cache::get_cached_object::<games::library::Game>(&format!("game/{game_id}"))
        .ok()
        .map(|g| g.m_name)
        .filter(|n| !n.trim().is_empty())
}

/// "Backed up 3 saves". `None` when nothing moved, so callers can pass their
/// count straight in without guarding first.
pub(crate) fn backed_up_message(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("Backed up 1 save".to_string()),
        n => Some(format!("Backed up {n} saves")),
    }
}

/// "Restored 2 saves from the cloud". `None` when nothing moved.
pub(crate) fn restored_message(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("Restored 1 save from the cloud".to_string()),
        n => Some(format!("Restored {n} saves from the cloud")),
    }
}

/// Tell the frontend saves actually moved, as `save_sync_complete`.
///
/// A separate topic from `save_sync_error` on purpose: that one already has a
/// listener that turns every payload into a modal, and a success has no
/// business interrupting anybody. `game_name` is its own field rather than
/// baked into `message` so the toast can render it as a second line.
pub(crate) fn emit_sync_complete(
    app: &AppHandle,
    game_id: &str,
    phase: &str,
    count: usize,
    message: &str,
) {
    info!("[SAVE-SYNC] {phase} complete for {game_id}: {message}");
    let _ = app.emit(
        "save_sync_complete",
        serde_json::json!({
            "gameId": game_id,
            "gameName": game_display_name(game_id),
            "phase": phase,
            "count": count,
            "message": message,
        }),
    );
}

/// Run a future to completion on the Tauri runtime, bounded by `timeout`.
/// On timeout the result is a `FailedDownload` error so callers fall back to
/// "launch without sync" rather than hanging.
fn block_with_timeout<F, T>(timeout: std::time::Duration, fut: F) -> Result<T, ()>
where
    F: std::future::Future<Output = T>,
{
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(timeout, fut).await.map_err(|_| ())
    })
}

/// Resolve a set of save conflicts, either by auto-picking `keep_local`
/// (streaming — the dialog would surface on the unattended host) or by
/// emitting a UI event and blocking on the resolution channel.
///
/// Returns `None` when the user never answered. That is deliberately NOT the
/// same as "keep local": keeping local uploads this device's copy over the
/// cloud row, i.e. the other device's save is destroyed because nobody was at
/// the keyboard. Losing an upload is recoverable, losing a cloud row is not.
fn resolve_conflicts(
    app: &AppHandle,
    game_id: &str,
    conflicts: &[remote::save_sync::SaveConflict],
    streaming: bool,
) -> Option<Vec<remote::save_sync::ConflictResolution>> {
    let keep_all_local = || {
        conflicts
            .iter()
            .map(|c| remote::save_sync::ConflictResolution {
                filename: c.filename.clone(),
                choice: "keep_local".to_string(),
            })
            .collect::<Vec<_>>()
    };

    if streaming {
        info!(
            "[SAVE-SYNC] Streaming mode — auto-resolving {} conflicts to keep_local",
            conflicts.len()
        );
        return Some(keep_all_local());
    }

    // Emit the conflict event so the UI can show its dialog. The topic is
    // global, not `save_sync_conflict/{game_id}`: the launch can come from
    // anywhere, and a per-game topic was only heard by a page mounted for that
    // exact game.
    let conflict_event = remote::save_sync::SaveConflictEvent {
        game_id: game_id.to_string(),
        conflicts: conflicts.to_vec(),
        timeout_secs: CONFLICT_RESOLVE_TIMEOUT.as_secs(),
    };
    let _ = app.emit(
        "save_sync_conflict",
        serde_json::to_value(&conflict_event).unwrap_or_default(),
    );

    // Register a resolution channel and block until the frontend answers
    // (or the timeout fires).
    let (tx, rx) = std::sync::mpsc::channel();
    crate::CONFLICT_CHANNELS
        .lock()
        .insert(game_id.to_string(), tx);
    info!("[SAVE-SYNC] Waiting for conflict resolution from UI...");

    let resolved = match rx.recv_timeout(CONFLICT_RESOLVE_TIMEOUT) {
        Ok(res) => Some(res),
        Err(_) => {
            emit_sync_error(
                app,
                game_id,
                PHASE_CONFLICT,
                "Nobody chose which save to keep, so nothing was synced for this game. Both \
                 copies are still there. Open the game's Cloud Saves panel to pick one.",
                true,
            );
            None
        }
    };
    crate::CONFLICT_CHANNELS.lock().remove(game_id);
    resolved
}

/// Push the local copies of `keep_filenames` to the cloud before the game
/// starts. Covers both halves of "this device's copy is the one that counts":
/// the files the user kept on a conflict, and the files the server said were
/// local-only (`action == "upload"`).
///
/// Returns the filenames that did NOT make it. The caller drops those from the
/// pre-launch snapshot so the exit path treats them as new and tries again —
/// otherwise a file that failed here and then went unchanged during the
/// session would never be uploaded at all.
fn upload_kept_local(
    app: &AppHandle,
    game_id: &str,
    local_saves: &[remote::save_sync::LocalSaveFile],
    keep_filenames: &[String],
) -> Vec<String> {
    let files: Vec<remote::save_sync::LocalSaveFile> = local_saves
        .iter()
        .filter(|f| keep_filenames.contains(&f.filename))
        .cloned()
        .collect();
    if files.is_empty() {
        return Vec::new();
    }
    let all_names = || files.iter().map(|f| f.filename.clone()).collect::<Vec<_>>();
    // Empty pre-hashes => everything is treated as changed and uploaded.
    let empty = HashMap::new();
    match block_with_timeout(
        UPLOAD_NET_TIMEOUT,
        remote::save_sync::upload_changed_saves(game_id, &empty, &files),
    ) {
        Ok(Ok((uploaded, errs))) => {
            info!(
                "[SAVE-SYNC] Pre-launch: uploaded {} of {} local saves",
                uploaded.len(),
                files.len()
            );
            emit_upload_failures(app, game_id, &errs, files.len(), "before launch");
            errs.into_iter().map(|e| e.filename).collect()
        }
        Ok(Err(e)) => {
            let (message, retryable) = describe_remote_error(&e);
            emit_sync_error(app, game_id, PHASE_UPLOAD, &message, retryable);
            all_names()
        }
        Err(()) => {
            emit_sync_error(app, game_id, PHASE_UPLOAD, MSG_OFFLINE, true);
            all_names()
        }
    }
}

/// Every filename the server verdicted `upload`: a local save with no cloud
/// counterpart at all.
///
/// This verdict was thrown away entirely. Nothing in the client handled the
/// string `"upload"`, so a save only ever reached the cloud if its MD5 changed
/// *during* a session — which meant a game finished in one sitting produced
/// zero backups, while the manifest recorded every file it had merely seen as
/// synced. The manifests were fiction.
///
/// Tombstoned filenames are excluded. `sync-check` deliberately returns both
/// the `upload` action and the tombstone when the user deleted a save from
/// another device; re-uploading it would resurrect the row they just deleted.
fn collect_upload_filenames(
    sync_result: &remote::save_sync::SyncCheckResponse,
) -> Vec<String> {
    let deleted: std::collections::HashSet<&str> = sync_result
        .tombstones
        .iter()
        .map(|t| t.filename.as_str())
        .collect();
    sync_result
        .actions
        .iter()
        .filter(|a| a.action == "upload")
        .map(|a| a.filename.clone())
        .filter(|filename| {
            if deleted.contains(filename.as_str()) {
                info!(
                    "[SAVE-SYNC] Not uploading {filename}: it was deleted from another device"
                );
                return false;
            }
            true
        })
        .collect()
}

/// The filenames the manifest must NOT claim are synced this round: the ones
/// whose pre-launch upload was rejected, plus — when the download leg failed —
/// every file we meant to pull, because no local copy mirrors the cloud row.
///
/// The manifest is the record of what is actually backed up. Stamping a file
/// as synced at a hash that never crossed the wire is what turned it into
/// fiction: an entry claiming five files synced at a timestamp whose log line
/// reads "No saves changed during session".
fn unsynced_filenames(
    sync_result: &remote::save_sync::SyncCheckResponse,
    failed_uploads: &[String],
    downloads_ok: bool,
) -> Vec<String> {
    let mut out = failed_uploads.to_vec();
    if !downloads_ok {
        out.extend(sync_result.cloud_only.iter().map(|c| c.filename.clone()));
        out.extend(
            sync_result
                .actions
                .iter()
                .filter(|a| a.action == "download")
                .map(|a| a.filename.clone()),
        );
    }
    out
}

/// Collect every cloud save id that needs downloading: cloud-only files,
/// explicit `download` actions, plus any extras from conflict resolution.
///
/// Denylisted filenames are dropped here. The local scan refuses to see
/// `.bak`, `.state.auto` and friends, so any of those already in the cloud
/// (uploaded before the scan denylist widened) is `cloudOnly` on every launch
/// for the rest of time. Pulling them wastes bandwidth at best and overwrites
/// the live file at worst — see `is_denylisted_cloud_filename`.
///
/// Switch NAND rows outside the launched title are dropped for the same
/// reason, via `switch_cloud_row_in_scope`: the scan is scoped to one title id
/// now, so every row the old whole-NAND sweep left behind reads as `cloudOnly`
/// on every launch and would be restored over another title's live save.
/// `switch_title_id` is `None` for PC games, which have no business holding
/// `switch__` rows at all.
fn collect_download_ids(
    sync_result: &remote::save_sync::SyncCheckResponse,
    extra: Vec<String>,
    switch_title_id: Option<&str>,
) -> Vec<String> {
    let wanted = |filename: &str| -> bool {
        if remote::save_sync::is_denylisted_cloud_filename(filename) {
            info!(
                "[SAVE-SYNC] Skipping cloud save {filename}: the local scan does not track this \
                 kind of file"
            );
            return false;
        }
        if !remote::save_sync::switch_cloud_row_in_scope(switch_title_id, filename) {
            info!(
                "[SAVE-SYNC] Skipping cloud save {filename}: it is outside the NAND directory of \
                 the title being launched"
            );
            return false;
        }
        // A PC save has no home in the emulator's save directory: it belongs
        // wherever Ludusavi says the game reads it from. Writing it here would
        // put a file the game never opens next to the real saves, and on a
        // filename match it would take a real save's place.
        if remote::save_sync::is_pc_namespaced_filename(filename) {
            info!(
                "[SAVE-SYNC] Skipping cloud save {filename}: PC saves do not restore into an \
                 emulator save directory"
            );
            return false;
        }
        true
    };

    let mut ids: Vec<String> = Vec::new();
    for save in &sync_result.cloud_only {
        if wanted(&save.filename) {
            ids.push(save.id.clone());
        }
    }
    for action in &sync_result.actions {
        if action.action == "download"
            && let Some(cloud) = &action.cloud_save
            && wanted(&cloud.filename)
        {
            ids.push(cloud.id.clone());
        }
    }
    ids.extend(extra);
    ids
}

/// Pre-launch sync for an **emulator** game. `emu_dir` is the emulator's
/// install directory; `rom_path` is the ROM being launched, the only place a
/// Switch title id can be read from (see
/// `remote::save_sync::switch_title_id_from_path`). Returns a snapshot of
/// post-download save hashes for the exit path to diff against.
pub fn sync_emulator_saves(
    app: &AppHandle,
    user_id: &str,
    game_id: &str,
    emu_dir: &str,
    rom_path: Option<&str>,
    streaming: bool,
) -> Option<SaveSyncSnapshot> {
    let emu_path = std::path::Path::new(emu_dir);
    let title_id = rom_path.and_then(remote::save_sync::switch_title_id_from_path);
    let local_saves =
        remote::save_sync::scan_emu_saves(emu_path, Some(user_id), game_id, title_id.as_deref());
    let pre_hashes = remote::save_sync::snapshot_hashes(&local_saves);

    // A failed/timed-out sync yields a snapshot flagged `synced_ok: false`:
    // the hashes are only "what was on this disk", never a baseline the cloud
    // agreed to, so the exit path must not upload against them.
    let local_only_snapshot = || SaveSyncSnapshot {
        emu_root: Some(emu_path.to_path_buf()),
        user_id: user_id.to_string(),
        game_id: game_id.to_string(),
        game_name: None,
        synced_ok: false,
        switch_title_id: title_id.clone(),
        pre_hashes: pre_hashes.clone(),
        pc_save_paths: HashMap::new(),
        wine_prefix: None,
    };

    let sync_result = match block_with_timeout(
        SYNC_NET_TIMEOUT,
        remote::save_sync::check_sync(game_id, &local_saves),
    ) {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            let (message, retryable) = describe_remote_error(&e);
            emit_sync_error(app, game_id, PHASE_CHECK, &message, retryable);
            return Some(local_only_snapshot());
        }
        Err(()) => {
            emit_sync_error(app, game_id, PHASE_CHECK, MSG_OFFLINE, true);
            return Some(local_only_snapshot());
        }
    };

    let conflict = handle_conflicts_and_collect(
        app,
        game_id,
        &sync_result,
        &local_saves,
        streaming,
    );
    let mut to_upload = conflict.uploads;
    to_upload.extend(collect_upload_filenames(&sync_result));
    let mut failed_uploads: Vec<String> = Vec::new();
    if !to_upload.is_empty() {
        info!(
            "[SAVE-SYNC] Backing up {} local save(s) for game {game_id} before launch",
            to_upload.len()
        );
        failed_uploads = upload_kept_local(app, game_id, &local_saves, &to_upload);
    }
    let download_ids =
        collect_download_ids(&sync_result, conflict.downloads, title_id.as_deref());

    let mut synced_ok = conflict.resolved;
    let mut failed_writes: Vec<String> = Vec::new();
    let mut restored = 0usize;
    if !download_ids.is_empty() {
        info!(
            "[SAVE-SYNC] Downloading {} cloud saves for game {game_id}",
            download_ids.len()
        );
        match block_with_timeout(
            DOWNLOAD_NET_TIMEOUT,
            remote::save_sync::bulk_download(&download_ids),
        ) {
            Ok(Ok(downloaded)) => {
                for (filename, save_type, hash, data) in &downloaded {
                    // Hard guard, not just the bandwidth filter in
                    // `collect_download_ids`: a file the scan deliberately
                    // refuses to see must never be written back.
                    if remote::save_sync::is_denylisted_cloud_filename(filename) {
                        warn!("[SAVE-SYNC] Refusing to write untracked cloud file {filename}");
                        continue;
                    }
                    // Same rule for a Switch row belonging to another title,
                    // the system NAND or sdmc: `write_downloaded_save` decodes
                    // the path and writes it anywhere under `emu_root`, so this
                    // is the guard that stops one game's launch rolling another
                    // game's save back to a stale cloud copy.
                    if !remote::save_sync::switch_cloud_row_in_scope(
                        title_id.as_deref(),
                        filename,
                    ) {
                        warn!(
                            "[SAVE-SYNC] Refusing to write {filename}: outside the NAND directory \
                             of the title being launched"
                        );
                        continue;
                    }
                    match remote::save_sync::write_downloaded_save(
                        emu_path,
                        Some(user_id),
                        game_id,
                        filename,
                        save_type,
                        data,
                        Some(hash),
                    ) {
                        Ok(path) => {
                            restored += 1;
                            info!("[SAVE-SYNC] Downloaded save: {}", path.display());
                        }
                        Err(e) => {
                            let (reason, retryable) = describe_failure(&e.to_string());
                            emit_sync_error(
                                app,
                                game_id,
                                PHASE_WRITE,
                                &format!("Drop could not restore {filename}. {reason}"),
                                retryable,
                            );
                            failed_writes.push(filename.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                let (message, retryable) = describe_remote_error(&e);
                emit_sync_error(app, game_id, PHASE_DOWNLOAD, &message, retryable);
                synced_ok = false;
            }
            Err(()) => {
                emit_sync_error(app, game_id, PHASE_DOWNLOAD, MSG_DOWNLOAD_TIMEOUT, true);
                synced_ok = false;
            }
        }
    }

    // A save we downloaded but could not write is the most dangerous state in
    // the whole flow: the cloud row is newer, this disk still holds the older
    // bytes, and leaving the round marked healthy would both stamp the stale
    // hash into the manifest and let the exit path push those older bytes back
    // over the row we just failed to apply.
    if !failed_writes.is_empty() {
        synced_ok = false;
    }

    // Counts writes that landed, not rows we asked for: a partial restore says
    // how much of it was real.
    if let Some(message) = restored_message(restored) {
        emit_sync_complete(app, game_id, PHASE_DOWNLOAD, restored, &message);
    }

    // Apply server tombstones: saves the user deleted from *another* device
    // get removed locally (after a backup). Runs AFTER downloads so a race
    // where the same filename is on both lists still ends up deleted. The
    // manifest is loaded first because applying a tombstone records it there,
    // which is what stops the server replaying the same delete every launch.
    let mut manifest = remote::save_sync::load_manifest(user_id, game_id);
    apply_emu_tombstones(emu_path, user_id, game_id, &sync_result.tombstones, &mut manifest);

    // Re-scan post-download and persist the manifest.
    let updated =
        remote::save_sync::scan_emu_saves(emu_path, Some(user_id), game_id, title_id.as_deref());
    let mut unsynced = unsynced_filenames(&sync_result, &failed_uploads, synced_ok);
    unsynced.extend(failed_writes);
    remote::save_sync::update_manifest_after_sync(
        &mut manifest,
        &updated,
        &sync_result,
        &unsynced,
    );
    if let Err(e) = remote::save_sync::save_manifest(&manifest) {
        warn!("[SAVE-SYNC] Failed to save manifest: {e}");
    }

    // A file we failed to push above has no cloud baseline, so it is dropped
    // from the snapshot: the exit path then treats it as new and retries,
    // instead of comparing it against itself and concluding nothing changed.
    let mut post_hashes = remote::save_sync::snapshot_hashes(&updated);
    post_hashes.retain(|name, _| !failed_uploads.contains(name));

    Some(SaveSyncSnapshot {
        emu_root: Some(emu_path.to_path_buf()),
        user_id: user_id.to_string(),
        game_id: game_id.to_string(),
        game_name: None,
        synced_ok,
        switch_title_id: title_id,
        pre_hashes: post_hashes,
        pc_save_paths: HashMap::new(),
        wine_prefix: None,
    })
}

/// Where a cloud PC save goes when this device has no copy of it and Ludusavi
/// found nothing on disk to anchor it to.
///
/// This is the fresh-machine case: a second computer that has never run the
/// game has no save files, so `common_save_root` has nothing to average and the
/// launch path used to refuse every download. Ludusavi's catalogue can answer
/// without anything on disk, which is exactly what
/// [`remote::save_sync::find_pc_save_destination`] is for, and it is what the
/// Cloud Saves panel's Restore button has been using all along.
///
/// It is cached because each resolution costs a full `backup --preview` run:
/// all of a game's PC saves share one directory, so the first answer is reduced
/// back to that directory and the rest of the batch is placed under it. A first
/// answer that cannot be reduced (an exact match somewhere unrelated) simply
/// leaves the cache empty and the next file pays for its own lookup.
struct CloudOnlyPcDest<'a> {
    game_name: &'a str,
    steam_app_id: Option<&'a str>,
    install_dir: Option<&'a Path>,
    wine_prefix: Option<&'a Path>,
    root: Option<PathBuf>,
}

impl CloudOnlyPcDest<'_> {
    fn resolve(&mut self, rel: &Path) -> Result<PathBuf, String> {
        if let Some(root) = &self.root {
            return Ok(root.join(rel));
        }
        let dest = remote::save_sync::find_pc_save_destination(
            self.game_name,
            self.steam_app_id,
            &rel.to_string_lossy(),
            self.install_dir,
            self.wine_prefix,
        )?;
        self.root = strip_rel_suffix(&dest, rel);
        Ok(dest)
    }
}

/// The directory `dest` would be, with `rel`'s components taken back off the
/// end. `None` when `dest` does not end with `rel`, which is the resolver
/// having matched an existing file somewhere the rest of the batch has no
/// business following it to.
fn strip_rel_suffix(dest: &Path, rel: &Path) -> Option<PathBuf> {
    let depth = rel.components().count();
    if depth == 0 || !dest.ends_with(rel) {
        return None;
    }
    let mut root = dest.to_path_buf();
    for _ in 0..depth {
        if !root.pop() {
            return None;
        }
    }
    Some(root)
}

/// The install directory the manifest's `<base>` placeholder anchors on, for a
/// game that saves next to its own executable. Absent when the game is not
/// installed on this device, and those catalogue patterns are then skipped.
fn install_dir_for(game_id: &str) -> Option<PathBuf> {
    match database::borrow_db_checked()
        .applications
        .game_statuses
        .get(game_id)
    {
        Some(database::GameDownloadStatus::Installed { install_dir, .. }) => {
            Some(PathBuf::from(install_dir))
        }
        _ => None,
    }
}

/// Pre-launch sync for a **PC/native** game discovered via Ludusavi.
/// `game_name` is the display name Ludusavi keys on.
/// `wine_prefix`, when present, is forwarded to Ludusavi via `--wine-prefix`
/// so saves under Drop's per-game prefix are visible (Linux host launching
/// a Windows-target game).
///
/// A game with no discoverable saves still gets a snapshot, with empty
/// `pre_hashes`. Returning `None` there meant the exit path was skipped
/// entirely, so the very first session — the one that CREATES the first save
/// file — never uploaded it. Anyone who finished a game in one sitting got
/// zero backups.
pub fn sync_pc_saves(
    app: &AppHandle,
    user_id: &str,
    game_id: &str,
    game_name: &str,
    wine_prefix: Option<PathBuf>,
    streaming: bool,
) -> Option<SaveSyncSnapshot> {
    // Without Ludusavi there is no PC save discovery at all, and the scan's
    // empty list is indistinguishable from "this game has no saves". Said once
    // per game per app run: the condition lasts until they install it, and a
    // modal on every launch would train them to dismiss the one that matters.
    if !remote::save_sync::ludusavi_available() && warn_ludusavi_once(game_id) {
        emit_sync_error(app, game_id, PHASE_CHECK, MSG_LUDUSAVI_MISSING, false);
    }

    let pc_saves = remote::save_sync::scan_pc_saves(
        game_name,
        None,
        wine_prefix.as_deref(),
    );

    let pre_hashes = remote::save_sync::snapshot_hashes(&pc_saves);
    let pc_paths: HashMap<String, PathBuf> = pc_saves
        .iter()
        .map(|f| (f.filename.clone(), f.path.clone()))
        .collect();

    let local_only_snapshot = || SaveSyncSnapshot {
        emu_root: None,
        user_id: user_id.to_string(),
        game_id: game_id.to_string(),
        game_name: Some(game_name.to_string()),
        synced_ok: false,
        switch_title_id: None,
        pre_hashes: pre_hashes.clone(),
        pc_save_paths: pc_paths.clone(),
        wine_prefix: wine_prefix.clone(),
    };

    let sync_result = match block_with_timeout(
        SYNC_NET_TIMEOUT,
        remote::save_sync::check_sync(game_id, &pc_saves),
    ) {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            let (message, retryable) = describe_remote_error(&e);
            emit_sync_error(app, game_id, PHASE_CHECK, &message, retryable);
            return Some(local_only_snapshot());
        }
        Err(()) => {
            emit_sync_error(app, game_id, PHASE_CHECK, MSG_OFFLINE, true);
            return Some(local_only_snapshot());
        }
    };

    let conflict =
        handle_conflicts_and_collect(app, game_id, &sync_result, &pc_saves, streaming);
    let mut to_upload = conflict.uploads;
    to_upload.extend(collect_upload_filenames(&sync_result));
    let mut failed_uploads: Vec<String> = Vec::new();
    if !to_upload.is_empty() {
        info!(
            "[SAVE-SYNC] Backing up {} local PC save(s) for game {game_id} before launch",
            to_upload.len()
        );
        failed_uploads = upload_kept_local(app, game_id, &pc_saves, &to_upload);
    }
    // No title id: a PC game's cloud must never hand this path a `switch__`
    // row, and decoding one here would aim an emulator NAND path at the PC
    // save root.
    let download_ids = collect_download_ids(&sync_result, conflict.downloads, None);

    // Cloud filenames are paths relative to the root the game's saves share,
    // so a cloud-only save (one with no local copy yet) lands in the right
    // subfolder of that real root instead of the dead-end fallback dir —
    // otherwise the game never reads it.
    let known: Vec<PathBuf> = pc_paths.values().cloned().collect();
    let save_root = remote::save_sync::common_save_root(&known);

    // The tier that makes a second computer work. Nothing on this disk can
    // place a save for a game that has never run here, so the catalogue is the
    // only thing that can, and it is the same resolver the Cloud Saves panel's
    // Restore button uses.
    let needs_catalogue = save_root.is_none() && !download_ids.is_empty();
    let install_dir = needs_catalogue.then(|| install_dir_for(game_id)).flatten();
    let steam_app_id = needs_catalogue
        .then(|| remote::save_sync::steam_app_id_for_game(game_id))
        .flatten();
    let mut catalogue_dest = CloudOnlyPcDest {
        game_name,
        steam_app_id: steam_app_id.as_deref(),
        install_dir: install_dir.as_deref(),
        wine_prefix: wine_prefix.as_deref(),
        root: None,
    };

    let mut synced_ok = conflict.resolved;
    let mut failed_writes: Vec<String> = Vec::new();
    // Saves the catalogue could not place. Collected rather than reported one
    // by one: the reason is the same for every file of a given game, and one
    // modal per file is the spam `emit_upload_failures` exists to avoid.
    let mut unplaceable: Vec<String> = Vec::new();
    let mut unplaceable_reason: Option<String> = None;
    let mut restored = 0usize;
    if !download_ids.is_empty() {
        match block_with_timeout(
            DOWNLOAD_NET_TIMEOUT,
            remote::save_sync::bulk_download(&download_ids),
        ) {
            Ok(Ok(downloaded)) => {
                for (filename, _save_type, hash, data) in &downloaded {
                    if remote::save_sync::is_denylisted_cloud_filename(filename) {
                        warn!("[SAVE-SYNC] Refusing to write untracked cloud file {filename}");
                        continue;
                    }
                    if !remote::save_sync::switch_cloud_row_in_scope(None, filename) {
                        warn!(
                            "[SAVE-SYNC] Refusing to write emulator NAND save {filename} into a \
                             PC game's save folder"
                        );
                        continue;
                    }
                    let dest: Option<PathBuf> = match pc_paths.get(filename.as_str()) {
                        Some(p) => Some(p.clone()),
                        None => match remote::save_sync::decode_pc_relpath(filename) {
                            Some(rel) => match &save_root {
                                Some(dir) => Some(dir.join(rel)),
                                None => match catalogue_dest.resolve(&rel) {
                                    Ok(p) => Some(p),
                                    Err(reason) => {
                                        unplaceable_reason.get_or_insert(reason);
                                        unplaceable.push(filename.clone());
                                        continue;
                                    }
                                },
                            },
                            None => {
                                warn!(
                                    "[SAVE-SYNC] Refusing cloud PC save with an unsafe name: \
                                     {filename}"
                                );
                                continue;
                            }
                        },
                    };
                    match remote::save_sync::write_downloaded_pc_save(
                        filename,
                        data,
                        dest.as_deref(),
                        Some(hash),
                    ) {
                        Ok(p) => {
                            restored += 1;
                            info!("[SAVE-SYNC] Downloaded PC save: {}", p.display());
                        }
                        Err(e) => {
                            let (reason, retryable) = describe_failure(&e.to_string());
                            emit_sync_error(
                                app,
                                game_id,
                                PHASE_WRITE,
                                &format!("Drop could not restore {filename}. {reason}"),
                                retryable,
                            );
                            failed_writes.push(filename.clone());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                let (message, retryable) = describe_remote_error(&e);
                emit_sync_error(app, game_id, PHASE_DOWNLOAD, &message, retryable);
                synced_ok = false;
            }
            Err(()) => {
                emit_sync_error(app, game_id, PHASE_DOWNLOAD, MSG_DOWNLOAD_TIMEOUT, true);
                synced_ok = false;
            }
        }
    }

    // One message for the whole batch, naming the count and the reason the
    // resolver gave for the first of them.
    if !unplaceable.is_empty() {
        let reason = unplaceable_reason.unwrap_or_else(|| {
            "Drop could not work out where this PC keeps this game's saves.".to_string()
        });
        emit_sync_error(
            app,
            game_id,
            PHASE_DOWNLOAD,
            &format!(
                "{} cloud save(s) for this game could not be put back on this PC. {reason}",
                unplaceable.len()
            ),
            false,
        );
        failed_writes.extend(unplaceable);
    }

    // Same rule as the emulator path: a cloud row this device downloaded but
    // failed to write must not be recorded as synced, and must not let the
    // exit path push the older local bytes back over it.
    if !failed_writes.is_empty() {
        synced_ok = false;
    }

    if let Some(message) = restored_message(restored) {
        emit_sync_complete(app, game_id, PHASE_DOWNLOAD, restored, &message);
    }

    // Apply server tombstones for PC saves. Resolve the local path via the
    // pre-launch scan map; if the filename isn't known locally, there's
    // nothing to delete and we just log.
    let mut manifest = remote::save_sync::load_manifest(user_id, game_id);
    apply_pc_tombstones(&pc_paths, &sync_result.tombstones, &mut manifest);

    let updated = remote::save_sync::scan_pc_saves(
        game_name,
        None,
        wine_prefix.as_deref(),
    );
    let mut unsynced = unsynced_filenames(&sync_result, &failed_uploads, synced_ok);
    unsynced.extend(failed_writes);
    remote::save_sync::update_manifest_after_sync(
        &mut manifest,
        &updated,
        &sync_result,
        &unsynced,
    );
    let _ = remote::save_sync::save_manifest(&manifest);

    let mut post_hashes = remote::save_sync::snapshot_hashes(&updated);
    post_hashes.retain(|name, _| !failed_uploads.contains(name));

    Some(SaveSyncSnapshot {
        emu_root: None,
        user_id: user_id.to_string(),
        game_id: game_id.to_string(),
        game_name: Some(game_name.to_string()),
        synced_ok,
        switch_title_id: None,
        pre_hashes: post_hashes,
        pc_save_paths: updated
            .iter()
            .map(|f| (f.filename.clone(), f.path.clone()))
            .collect(),
        wine_prefix,
    })
}

/// Narrow the raw server tombstone list down to the ones this device should
/// act on, recording the self-issued ones as handled on the way through.
///
/// `sync-check` sends back every tombstone for 30 days with no per-device
/// filter, including the ones this very device created. Applying that list as
/// given deletes the local file on the machine that issued the delete, and
/// then does it again on every subsequent launch — the second pass eating the
/// save the game wrote in between. See `remote::save_sync::tombstone`.
fn tombstones_for_this_device<'a>(
    tombstones: &'a [remote::save_sync::Tombstone],
    manifest: &mut remote::save_sync::SyncManifest,
) -> Vec<&'a remote::save_sync::Tombstone> {
    let this_device = remote::save_sync::machine_name();
    let plan = remote::save_sync::plan_tombstones(tombstones, manifest, &this_device);

    for t in &plan.self_issued {
        info!(
            "[SAVE-SYNC] Tombstone for {} was issued by this device ('{}'); leaving the local file alone",
            t.filename, t.deleted_from
        );
        remote::save_sync::record_applied(manifest, t);
    }
    if plan.replays > 0 {
        info!(
            "[SAVE-SYNC] Ignoring {} tombstone(s) already applied on this device",
            plan.replays
        );
    }
    plan.apply
}

/// Apply server-issued tombstones for an emulator game: back the local file
/// up, unlink it, and record the tombstone so it never applies twice. A
/// failed delete is logged and deliberately NOT recorded, so it retries next
/// launch; everything else is recorded, including "there was nothing here",
/// because the file found under that name next time is a different save.
fn apply_emu_tombstones(
    emu_path: &std::path::Path,
    user_id: &str,
    game_id: &str,
    tombstones: &[remote::save_sync::Tombstone],
    manifest: &mut remote::save_sync::SyncManifest,
) {
    if tombstones.is_empty() {
        return;
    }
    let to_apply = tombstones_for_this_device(tombstones, manifest);
    if to_apply.is_empty() {
        return;
    }
    info!(
        "[SAVE-SYNC] Applying {} tombstones for emulator game {game_id}",
        to_apply.len()
    );
    for t in to_apply {
        match remote::save_sync::delete_local_emu_save_for_tombstone(
            emu_path,
            Some(user_id),
            game_id,
            &t.filename,
        ) {
            Ok(Some(path)) => {
                info!(
                    "[SAVE-SYNC] Tombstone: deleted local save {} (deleted from '{}' at {})",
                    path.display(),
                    t.deleted_from,
                    t.deleted_at
                );
                remote::save_sync::record_applied(manifest, t);
            }
            Ok(None) => {
                info!(
                    "[SAVE-SYNC] Tombstone: no local copy of {} to delete",
                    t.filename
                );
                remote::save_sync::record_applied(manifest, t);
            }
            Err(e) => warn!(
                "[SAVE-SYNC] Tombstone: failed to delete {}: {e}",
                t.filename
            ),
        }
    }
}

/// Apply server tombstones for PC saves. Resolves each filename via the
/// pre-scan path map; filenames the local scan didn't see are logged and
/// skipped (the user may have already deleted them on this machine).
/// Same record-on-success rule as [`apply_emu_tombstones`].
fn apply_pc_tombstones(
    pc_paths: &HashMap<String, PathBuf>,
    tombstones: &[remote::save_sync::Tombstone],
    manifest: &mut remote::save_sync::SyncManifest,
) {
    if tombstones.is_empty() {
        return;
    }
    let to_apply = tombstones_for_this_device(tombstones, manifest);
    if to_apply.is_empty() {
        return;
    }
    info!("[SAVE-SYNC] Applying {} PC tombstones", to_apply.len());
    for t in to_apply {
        let Some(orig) = pc_paths.get(&t.filename) else {
            info!(
                "[SAVE-SYNC] PC tombstone: no local copy of {} (deleted from '{}'), skipping",
                t.filename, t.deleted_from
            );
            remote::save_sync::record_applied(manifest, t);
            continue;
        };
        match remote::save_sync::delete_local_pc_save_for_tombstone(orig) {
            Ok(true) => {
                info!(
                    "[SAVE-SYNC] PC tombstone: deleted {} (deleted from '{}' at {})",
                    orig.display(),
                    t.deleted_from,
                    t.deleted_at
                );
                remote::save_sync::record_applied(manifest, t);
            }
            Ok(false) => {
                info!("[SAVE-SYNC] PC tombstone: {} already gone", orig.display());
                remote::save_sync::record_applied(manifest, t);
            }
            Err(e) => warn!(
                "[SAVE-SYNC] PC tombstone: failed to delete {}: {e}",
                orig.display()
            ),
        }
    }
}

/// What the conflict dance produced. The caller does the pushing, so the
/// conflict uploads and the `action == "upload"` backups go out in one request
/// instead of two.
struct ConflictOutcome {
    /// Extra cloud ids to pull (the `keep_cloud` choices).
    downloads: Vec<String>,
    /// Extra local filenames to push (the `keep_local` choices).
    uploads: Vec<String>,
    /// False when the dialog went unanswered, either because it timed out or
    /// because the user dismissed it. Both lists are then empty, and the
    /// caller must treat the whole sync as incomplete: the conflicted files
    /// still differ from the cloud, so uploading whatever this session writes
    /// would overwrite the version nobody chose to discard.
    resolved: bool,
}

/// Shared conflict path for both emulator and PC syncs: extract conflicts,
/// resolve them (UI or auto), and turn the answers into work lists.
fn handle_conflicts_and_collect(
    app: &AppHandle,
    game_id: &str,
    sync_result: &remote::save_sync::SyncCheckResponse,
    local_saves: &[remote::save_sync::LocalSaveFile],
    streaming: bool,
) -> ConflictOutcome {
    let none = || ConflictOutcome {
        downloads: Vec::new(),
        uploads: Vec::new(),
        resolved: true,
    };
    let conflicts = remote::save_sync::extract_conflicts(sync_result, local_saves);
    if conflicts.is_empty() {
        return none();
    }
    info!(
        "[SAVE-SYNC] {} conflicts detected for game {game_id}",
        conflicts.len()
    );

    let Some(resolutions) = resolve_conflicts(app, game_id, &conflicts, streaming) else {
        return ConflictOutcome {
            resolved: false,
            ..none()
        };
    };
    // The dialog was closed without a choice (Esc, backdrop click, "Decide
    // later"). It answers with `skip` rather than leaving the channel silent,
    // so the launch continues immediately instead of stalling for the whole
    // resolve timeout, but the sync is still incomplete and nothing moves.
    if remote::save_sync::any_conflict_deferred(&conflicts, &resolutions) {
        info!(
            "[SAVE-SYNC] Conflict dialog dismissed for {game_id}; syncing nothing this launch"
        );
        return ConflictOutcome {
            resolved: false,
            ..none()
        };
    }

    let (downloads, uploads) =
        remote::save_sync::apply_conflict_resolutions(&conflicts, &resolutions);

    info!(
        "[SAVE-SYNC] Conflict resolution applied: {} resolutions",
        resolutions.len()
    );
    ConflictOutcome {
        downloads,
        uploads,
        resolved: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use remote::save_sync::{CloudSaveMeta, SyncAction, SyncCheckResponse, Tombstone};

    fn cloud(id: &str, filename: &str) -> CloudSaveMeta {
        CloudSaveMeta {
            id: id.to_string(),
            filename: filename.to_string(),
            save_type: "save".to_string(),
            data_hash: "cloudhash".to_string(),
            size: 1,
            uploaded_from: "other".to_string(),
            client_modified_at: String::new(),
            uploaded_at: String::new(),
            owned_by: String::new(),
            shadowed_save_id: None,
            also_held_by: Vec::new(),
        }
    }

    fn action(filename: &str, verdict: &str, cloud_save: Option<CloudSaveMeta>) -> SyncAction {
        SyncAction {
            filename: filename.to_string(),
            action: verdict.to_string(),
            cloud_save,
            local_hash: Some("localhash".to_string()),
        }
    }

    fn response(actions: Vec<SyncAction>, tombstones: Vec<Tombstone>) -> SyncCheckResponse {
        SyncCheckResponse {
            actions,
            cloud_only: Vec::new(),
            tombstones,
        }
    }

    /// The headline defect: `sync-check` returns `upload` for every local file
    /// with no cloud counterpart, and the client had no handler for the string
    /// at all. Nothing was ever backed up unless it changed mid-session.
    #[test]
    fn upload_verdicts_are_collected() {
        let res = response(
            vec![
                action("new.srm", "upload", None),
                action("same.srm", "synced", Some(cloud("c1", "same.srm"))),
                action("old.srm", "download", Some(cloud("c2", "old.srm"))),
                action("both.srm", "conflict", Some(cloud("c3", "both.srm"))),
                action("empty-cloud.srm", "upload", Some(cloud("c4", "empty-cloud.srm"))),
            ],
            Vec::new(),
        );
        let mut names = collect_upload_filenames(&res);
        names.sort();
        assert_eq!(names, vec!["empty-cloud.srm", "new.srm"]);
    }

    /// The server sends both the `upload` action and the tombstone when a save
    /// was deleted elsewhere. Uploading would resurrect the row the user just
    /// deleted, so the tombstone wins.
    #[test]
    fn a_tombstoned_file_is_not_re_uploaded() {
        let res = response(
            vec![
                action("deleted.srm", "upload", None),
                action("kept.srm", "upload", None),
            ],
            vec![Tombstone {
                filename: "deleted.srm".to_string(),
                deleted_at: "2026-06-10T17:00:24Z".to_string(),
                deleted_from: "Steam Deck".to_string(),
            }],
        );
        assert_eq!(collect_upload_filenames(&res), vec!["kept.srm"]);
    }

    /// The manifest must not claim a file is synced when the bytes never
    /// crossed the wire, in either direction.
    #[test]
    fn a_failed_round_marks_every_untransferred_file_unsynced() {
        let mut res = response(
            vec![action("old.srm", "download", Some(cloud("c2", "old.srm")))],
            Vec::new(),
        );
        res.cloud_only = vec![cloud("c3", "onlycloud.srm")];

        // Downloads succeeded: only the rejected upload is held back.
        assert_eq!(
            unsynced_filenames(&res, &["rejected.srm".to_string()], true),
            vec!["rejected.srm"]
        );

        // Downloads failed: nothing local mirrors those cloud rows either.
        let mut names = unsynced_filenames(&res, &["rejected.srm".to_string()], false);
        names.sort();
        assert_eq!(names, vec!["old.srm", "onlycloud.srm", "rejected.srm"]);
    }

    /// Every cause worth telling a user apart, recognised from the text its
    /// own layer produces: the server's per-file `errors[]`, a Windows sharing
    /// violation, a POSIX EACCES, and a transport failure.
    #[test]
    fn failures_map_to_the_cause_the_user_can_act_on() {
        let cases = [
            ("Save storage quota exceeded for this user", "storage is full", false),
            (
                "The process cannot access the file because it is being used by another \
                 process. (os error 32)",
                "still has that save file open",
                true,
            ),
            ("Permission denied (os error 13)", "not allowed to read", false),
            (
                "Your session is no longer valid. Please sign in to Drop again.",
                "signed out",
                false,
            ),
            (
                "Failed to connect to Drop server. Check if you access Drop through a browser.",
                "could not reach your server",
                true,
            ),
        ];
        for (raw, expected_fragment, expected_retryable) in cases {
            let (message, retryable) = describe_failure(raw);
            assert!(
                message.contains(expected_fragment),
                "{raw:?} produced {message:?}"
            );
            assert_eq!(retryable, expected_retryable, "{raw:?}");
        }
    }

    /// The five the user is most likely to hit, each landing on its own
    /// message. Grouped in one test because the value is in them being
    /// DIFFERENT from each other: before this they all produced silence, and
    /// two of the five, once they spoke, named the wrong cause.
    #[test]
    fn the_five_failures_that_matter_are_five_different_answers() {
        let offline = describe_failure("error sending request: dns error: failed to lookup address information").0;
        let unreachable =
            describe_failure("Failed to connect to Drop server. Check if you can access Drop through a browser.").0;
        let quota = describe_failure("Save quota exceeded: would be 1.20 GiB / 1.00 GiB").0;
        let ludusavi = describe_failure("Ludusavi not found on this system").0;
        let disk = describe_failure("There is not enough space on the disk. (os error 112)").0;

        let all = [&offline, &unreachable, &quota, &ludusavi, &disk];
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert_ne!(a, b, "two causes share one message");
            }
        }

        assert!(offline.contains("not on a network"), "{offline}");
        assert!(unreachable.contains("could not reach your server"), "{unreachable}");
        assert!(quota.contains("cloud save storage is full"), "{quota}");
        assert!(ludusavi.contains("Ludusavi"), "{ludusavi}");
        assert!(disk.contains("run out of disk space"), "{disk}");
    }

    /// A disk that filled up on THIS PC is not the cloud quota. These two
    /// shared a branch, so an ENOSPC writing a restored save told the user to
    /// go and delete cloud saves, which would not have freed one byte of the
    /// disk that was actually full.
    #[test]
    fn a_full_local_disk_is_not_reported_as_a_full_quota() {
        for raw in [
            "No space left on device (os error 28)",
            "There is not enough space on the disk. (os error 112)",
        ] {
            let (message, _) = describe_failure(raw);
            assert!(message.contains("This PC has run out of disk space"), "{raw}");
            assert!(!message.contains("cloud save storage"), "{raw} -> {message}");
        }
    }

    /// A Ludusavi that is installed and merely failed to run must not be
    /// reported as a Ludusavi that is missing.
    #[test]
    fn a_ludusavi_that_ran_and_failed_is_not_a_missing_ludusavi() {
        let (message, _) = describe_failure("Ludusavi restore failed: manifest is corrupt");
        assert!(!message.contains("Install Ludusavi"), "{message}");
        assert_eq!(message, "Ludusavi restore failed: manifest is corrupt");
    }

    /// Nothing moved means nothing to celebrate. Callers pass their raw count
    /// in, so a zero has to fall out here rather than at every call site.
    #[test]
    fn a_confirmation_is_only_produced_when_saves_actually_moved() {
        assert_eq!(backed_up_message(0), None);
        assert_eq!(restored_message(0), None);
        assert_eq!(backed_up_message(1).unwrap(), "Backed up 1 save");
        assert_eq!(backed_up_message(3).unwrap(), "Backed up 3 saves");
        assert_eq!(
            restored_message(1).unwrap(),
            "Restored 1 save from the cloud"
        );
        assert_eq!(
            restored_message(2).unwrap(),
            "Restored 2 saves from the cloud"
        );
    }

    /// The install prompt is a one-off errand, so it is said once per game per
    /// app run rather than on every launch.
    #[test]
    fn the_ludusavi_prompt_is_not_repeated_for_the_same_game() {
        assert!(warn_ludusavi_once("game-a"));
        assert!(!warn_ludusavi_once("game-a"));
        assert!(warn_ludusavi_once("game-b"));
    }

    /// Anything we cannot classify is passed through verbatim rather than
    /// replaced with a reassuring generic that hides the real cause.
    #[test]
    fn an_unrecognised_failure_is_passed_through() {
        let (message, retryable) = describe_failure("save row rejected by validator");
        assert_eq!(message, "save row rejected by validator");
        assert!(!retryable);
    }

    /// The user-facing copy must not carry em-dashes (project style) and must
    /// not be empty, or the modal renders a blank body.
    #[test]
    fn the_canned_messages_follow_the_copy_rules() {
        for msg in [
            MSG_OFFLINE,
            MSG_SERVER_UNREACHABLE,
            MSG_QUOTA_FULL,
            MSG_DISK_FULL,
            MSG_LUDUSAVI_MISSING,
            MSG_SIGNED_OUT,
            MSG_DOWNLOAD_TIMEOUT,
        ] {
            assert!(!msg.is_empty());
            assert!(!msg.contains('\u{2014}'), "em-dash in {msg:?}");
            // Every one of these has to say what to do next, not just what
            // broke. Each ends on an instruction, so it is at least two
            // sentences long.
            assert!(
                msg.matches(". ").count() >= 1,
                "no action sentence in {msg:?}"
            );
        }
    }

    #[test]
    fn downloads_still_only_come_from_cloud_only_and_download_verdicts() {
        let res = response(
            vec![
                action("new.srm", "upload", None),
                action("old.srm", "download", Some(cloud("c2", "old.srm"))),
            ],
            Vec::new(),
        );
        assert_eq!(collect_download_ids(&res, Vec::new(), None), vec!["c2"]);
    }

    /// Scoping the scan to one title id turned every NAND row the old
    /// whole-NAND sweep left behind into a permanent `cloudOnly`, and the
    /// download path would have written each one back over another title's
    /// live save on every single launch.
    #[test]
    fn switch_rows_outside_the_launched_title_are_never_downloaded() {
        let mine = "switch__user%2Fnand%2Fuser%2Fsave%2F0000%2Fuser1%2F0100aaaabbbb0000%2F00";
        let other = "switch__user%2Fnand%2Fuser%2Fsave%2F0000%2Fuser1%2F0100ccccdddd0000%2F00";
        let system = "switch__user%2Fnand%2Fsystem%2Fsave%2F8000000000000030";
        let sdmc = "switch__user%2Fsdmc%2FNintendo%2FAlbum%2Fshot.jpg";

        let mut res = response(
            vec![action(other, "download", Some(cloud("c-other", other)))],
            Vec::new(),
        );
        res.cloud_only = vec![
            cloud("c-mine", mine),
            cloud("c-system", system),
            cloud("c-sdmc", sdmc),
        ];

        assert_eq!(
            collect_download_ids(&res, Vec::new(), Some("0100aaaabbbb0000")),
            vec!["c-mine"]
        );
        // No title id means the scan reads nothing from the NAND, so nothing
        // from the NAND may be written back either.
        assert!(collect_download_ids(&res, Vec::new(), None).is_empty());
    }

    /// The bare `drop-saves` scheme is untouched by the Switch scoping, even
    /// for an emulator game that has a title id.
    ///
    /// A `pc__` row is a different matter. Nothing on the emulator path knows
    /// where the game really reads a PC save from, so restoring one here drops
    /// a file the game never opens into `drop-saves/…/saves`, and on a
    /// filename collision it takes a real emulator save's place.
    #[test]
    fn a_pc_row_never_restores_into_the_emulator_save_directory() {
        let mut res = response(Vec::new(), Vec::new());
        res.cloud_only = vec![
            cloud("c1", "Game.srm"),
            cloud("c2", "pc__slot1%2Fsave.dat"),
            cloud("c3", "pc/legacy.dat"),
        ];
        assert_eq!(
            collect_download_ids(&res, Vec::new(), Some("0100aaaabbbb0000")),
            vec!["c1"]
        );
    }

    /// A per-file size rejection is not a quota problem: freeing space on the
    /// server will never let an over-size save through.
    #[test]
    fn an_oversized_save_is_not_reported_as_a_full_quota() {
        let (message, retryable) = describe_failure("File too large (max 100MB)");
        assert!(message.contains("bigger than the server will accept"));
        assert!(!message.contains("storage is full"));
        assert!(!retryable);

        let (message, _) =
            describe_failure("Save quota exceeded: would be 1.20 GiB / 1.00 GiB");
        assert!(message.contains("storage is full"));
    }
}
