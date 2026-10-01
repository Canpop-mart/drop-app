use std::path::PathBuf;

use database::db::DATA_ROOT_DIR;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    error::RemoteAccessError,
    requests::{generate_url, RemoteRequest},
    utils::{bounded_json, DEFAULT_JSON_CAP_BYTES},
};

/// Playtime owns its *own* retry loop (with queue-on-failure and "session
/// already ended" handling), so it sends through the shared request core with
/// the core's retry disabled — otherwise the two retry layers would compound
/// into ~9 attempts. The shared core is still used for the consistent
/// timeout, per-attempt JWT and `AutoOffline` middleware.
fn playtime_post<T: serde::Serialize>(url: url::Url, body: &T) -> RemoteRequest<'_, T> {
    RemoteRequest::post(url, body).with_max_attempts(1)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaytimeStartBody {
    game_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaytimeStartResponse {
    session_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaytimeStopBody {
    session_id: String,
    /// Client-measured process duration in seconds. More accurate than
    /// server-side timestamp arithmetic when clock drift or NAS sleep occurs.
    #[serde(skip_serializing_if = "Option::is_none")]
    client_duration_secs: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaytimeHeartbeatBody {
    session_id: String,
}

/// Start a playtime session for a game. Returns the session ID.
///
/// Retries up to 3 times with exponential backoff (1s, 2s, 4s). A single
/// network blip at launch shouldn't lose the entire session — the user
/// expects playtime to be recorded even if they're on flaky wifi.
pub async fn start_playtime(game_id: &str) -> Result<String, RemoteAccessError> {
    let body = PlaytimeStartBody {
        game_id: game_id.to_string(),
    };

    let max_retries = 3u32;
    let mut last_err = None;

    for attempt in 0..max_retries {
        if attempt > 0 {
            let delay = std::time::Duration::from_secs(2u64.pow(attempt));
            info!(
                "Retrying playtime start for game {} (attempt {}/{}), waiting {:?}",
                game_id,
                attempt + 1,
                max_retries,
                delay
            );
            tokio::time::sleep(delay).await;
        }

        let url = match generate_url(&["/api/v1/client/playtime/start"], &[]) {
            Ok(u) => u,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };

        let response = match playtime_post(url, &body).send_raw().await {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Network error starting playtime session for {} (attempt {}): {}",
                    game_id,
                    attempt + 1,
                    e
                );
                last_err = Some(e);
                continue;
            }
        };

        if response.status() != 200 {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "Failed to start playtime session for {} (attempt {}): {} - {}",
                game_id,
                attempt + 1,
                status,
                text
            );
            last_err = Some(RemoteAccessError::UnparseableResponse(format!(
                "Failed to start playtime: {status} - {text}"
            )));
            // 4xx errors (other than transient ones) won't fix themselves on
            // retry — bail rather than burn the full 7s of backoff.
            if status.is_client_error() && status.as_u16() != 429 {
                break;
            }
            continue;
        }

        let data: PlaytimeStartResponse = bounded_json(response, DEFAULT_JSON_CAP_BYTES).await?;
        info!(
            "Started playtime session {} for game {}",
            data.session_id, game_id
        );
        return Ok(data.session_id);
    }

    Err(last_err.unwrap_or_else(|| {
        RemoteAccessError::UnparseableResponse("All retries exhausted".to_string())
    }))
}

/// Stop a playtime session. Retries up to 3 times with backoff on failure.
/// This triggers server-side achievement sync.
///
/// A server that has already closed the session (orphan cleanup, after a stop
/// that could not get through) accepts the measured duration and replaces its
/// own estimate with it. An older server answers 400 "already ended" instead,
/// which is treated as done.
///
/// `client_duration_secs` — if provided, the server uses this measured duration
/// instead of computing it from timestamps. More accurate when the server clock
/// is unreliable (NAS sleep, network delays, etc.).
///
/// A stop the route itself refuses for good ([`stop_refusal_is_final`]) is
/// logged and returns `Ok`: queueing it would only replay the same refusal.
pub async fn stop_playtime(session_id: &str, client_duration_secs: Option<u32>) -> Result<(), RemoteAccessError> {
    match send_stop(session_id, client_duration_secs).await {
        SendOutcome::Done | SendOutcome::Rejected(_) => Ok(()),
        SendOutcome::Failed(e) => Err(e),
    }
}

/// How a playtime request ended, as far as a queue is concerned.
enum SendOutcome {
    Done,
    /// The route itself refused the request for good. Retrying can never
    /// succeed, so a queued item is dropped rather than replayed on every
    /// start forever.
    Rejected(String),
    /// Anything else: network trouble, a server error, an expired sign-in
    /// (401/403), or a server without the route. Kept and retried later.
    Failed(RemoteAccessError),
}

/// Whether a failed `playtime/stop` answer is final. A 400 is the route's
/// own (validation, or a session already closed too long ago to take a
/// measured length). A 404 is final only when it is the route's own "Session
/// not found." and not, say, a proxy in front of the server. 401/403 are
/// sign-in or token problems, or a session another account owns, which the
/// right account signing in can still fix.
fn stop_refusal_is_final(status: u16, body: &str) -> bool {
    status == 400 || (status == 404 && body.contains("Session not found."))
}

/// Whether a failed `playtime/record` answer is final. A 400 is the route's
/// own validation (bad or too-old start time). A 409 is final only for a day
/// that is already full: its other 409, a span wholly covered by other play,
/// may change once a queued stop corrects what is in the way, so that one is
/// retried (the 30-day age limit ends it). A 404 is final only when it is the route's own "Game not found." /
/// "Not found.": a server too old to have the route at all also answers 404,
/// and the record must wait for that server to be updated. 401/403 are
/// sign-in or token problems and are retried.
fn record_refusal_is_final(status: u16, body: &str) -> bool {
    status == 400
        || (status == 409 && body.contains("No playtime left"))
        || (status == 404 && (body.contains("Game not found.") || body.contains("\"Not found.\"")))
}

async fn send_stop(session_id: &str, client_duration_secs: Option<u32>) -> SendOutcome {
    let body = PlaytimeStopBody {
        session_id: session_id.to_string(),
        client_duration_secs,
    };

    let max_retries = 3u32;
    let mut last_err = None;

    for attempt in 0..max_retries {
        if attempt > 0 {
            let delay = std::time::Duration::from_secs(2u64.pow(attempt));
            info!(
                "Retrying playtime stop for session {} (attempt {}/{}), waiting {:?}",
                session_id,
                attempt + 1,
                max_retries,
                delay
            );
            tokio::time::sleep(delay).await;
        }

        let url = match generate_url(&["/api/v1/client/playtime/stop"], &[]) {
            Ok(u) => u,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };

        let response = match playtime_post(url, &body).send_raw().await {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Network error stopping playtime session {} (attempt {}): {}",
                    session_id,
                    attempt + 1,
                    e
                );
                last_err = Some(e);
                continue;
            }
        };

        if response.status() == 200 {
            info!("Stopped playtime session {}", session_id);
            return SendOutcome::Done;
        }

        // 400 "Session already ended" (an older server, or a replay too long
        // after the start) — not an error, just bail
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if status.as_u16() == 400 && text.contains("already ended") {
            info!("Playtime session {} was already ended", session_id);
            return SendOutcome::Done;
        }
        if stop_refusal_is_final(status.as_u16(), &text) {
            warn!("Server refused the stop for playtime session {session_id}: {status} - {text}");
            return SendOutcome::Rejected(format!("{status} - {text}"));
        }

        warn!(
            "Failed to stop playtime session {} (attempt {}): {} - {}",
            session_id,
            attempt + 1,
            status,
            text
        );
        last_err = Some(RemoteAccessError::UnparseableResponse(format!(
            "Failed to stop playtime: {status} - {text}"
        )));
    }

    SendOutcome::Failed(last_err.unwrap_or_else(|| {
        RemoteAccessError::UnparseableResponse("All retries exhausted".to_string())
    }))
}

/// Send a heartbeat for an active playtime session.
/// Called periodically (~5 min) so the server can cap orphaned sessions
/// at the last heartbeat instead of assuming the full elapsed time.
pub async fn heartbeat_playtime(session_id: &str) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/playtime/heartbeat"], &[])?;
    let body = PlaytimeHeartbeatBody {
        session_id: session_id.to_string(),
    };

    match playtime_post(url, &body).send_raw().await {
        Ok(response) => {
            if response.status() == 200 {
                info!("Heartbeat sent for playtime session {}", session_id);
            } else {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                warn!("Heartbeat failed for session {}: {} - {}", session_id, status, text);
            }
        }
        Err(e) => {
            warn!("Network error sending heartbeat for session {}: {}", session_id, e);
        }
    }

    // Heartbeat failures are non-fatal — don't propagate errors
    Ok(())
}

// ── Whole-session records ───────────────────────────────────────────────────
//
// For a launch whose `start_playtime` never landed (offline, server down, or a
// game that exited before the retries finished). There is no session to stop,
// so the exit path sends the whole session in one go instead of dropping it.

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PlaytimeRecordBody {
    /// Generated here, so replaying a queued record cannot count it twice.
    session_id: String,
    game_id: String,
    /// RFC 3339, this machine's clock when the game started.
    started_at: String,
    client_duration_secs: u32,
}

/// A fresh, UUID-shaped id for a client-recorded session. The server requires
/// UUID form and treats a repeat of the same id as the same session. Built
/// from an MD5 of the game, the time and this process rather than a new
/// dependency; uniqueness only has to hold per user.
fn new_session_id(game_id: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seed = format!(
        "{game_id}|{nanos}|{}|{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let mut b = md5::compute(seed.as_bytes()).0;
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
    let h = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

async fn send_record(body: &PlaytimeRecordBody) -> SendOutcome {
    let url = match generate_url(&["/api/v1/client/playtime/record"], &[]) {
        Ok(u) => u,
        Err(e) => return SendOutcome::Failed(e),
    };
    let response = match playtime_post(url, body).send_raw().await {
        Ok(r) => r,
        Err(e) => return SendOutcome::Failed(e),
    };
    let status = response.status();
    if status == 200 {
        return SendOutcome::Done;
    }
    let text = response.text().await.unwrap_or_default();
    if record_refusal_is_final(status.as_u16(), &text) {
        SendOutcome::Rejected(format!("{status} - {text}"))
    } else {
        SendOutcome::Failed(RemoteAccessError::UnparseableResponse(format!(
            "Failed to record playtime: {status} - {text}"
        )))
    }
}

/// A whole-session record waiting on disk, with the account it belongs to.
///
/// The request itself carries no user (the server credits whoever signs the
/// request), so the queue has to: a record is only ever sent while that same
/// account is signed in, and waits otherwise.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingRecord {
    user_id: String,
    record: PlaytimeRecordBody,
}

/// Record a finished session whose start never reached the server, for
/// `user_id` (the account that played). On failure it is queued on disk and
/// retried by [`drain_pending_stops`] (at startup, after a sign-in, and when
/// the app comes back online) while that account is signed in. A definitive refusal is logged and not queued.
pub async fn record_session(
    user_id: Option<&str>,
    game_id: &str,
    started_at: chrono::DateTime<chrono::Utc>,
    duration_secs: u32,
) {
    let body = PlaytimeRecordBody {
        session_id: new_session_id(game_id),
        game_id: game_id.to_string(),
        started_at: started_at.to_rfc3339(),
        client_duration_secs: duration_secs,
    };
    match send_record(&body).await {
        SendOutcome::Done => info!(
            "Recorded {duration_secs}s of playtime for game {game_id} (its session never started)"
        ),
        SendOutcome::Rejected(reason) => {
            warn!("Server refused the playtime record for game {game_id}, dropping it: {reason}")
        }
        SendOutcome::Failed(e) => match user_id {
            Some(user_id) => {
                warn!("Could not record playtime for game {game_id}, queuing it: {e}");
                queue_pending_record(&PendingRecord {
                    user_id: user_id.to_string(),
                    record: body,
                });
            }
            // Nobody to credit it to later. Signed-out play reaches nothing
            // server-side anyway, so this is the same as before.
            None => warn!(
                "Could not record playtime for game {game_id} and nobody is signed in to \
                 queue it for: {e}"
            ),
        },
    }
}

fn pending_records_dir() -> PathBuf {
    DATA_ROOT_DIR.join("pending-playtime-records")
}

fn queue_pending_record(pending: &PendingRecord) {
    let dir = pending_records_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("Could not create pending-playtime-records dir at {dir:?}: {e}");
        return;
    }
    let path = dir.join(format!("{}.json", pending.record.session_id));
    match serde_json::to_vec_pretty(pending) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&path, bytes) {
                warn!("Could not write pending playtime record to {path:?}: {e}");
            } else {
                info!("Queued pending playtime record at {path:?}");
            }
        }
        Err(e) => warn!("Could not serialize pending playtime record: {e}"),
    }
}

/// Replay queued whole-session records. A file is deleted once the server has
/// it, or has refused it outright; a record for an account other than the one
/// signed in now is left for when that account signs in.
async fn drain_pending_records(signed_in: Option<&str>) -> (usize, usize) {
    let dir = pending_records_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (0, 0);
    };
    let (mut ok, mut failed) = (0usize, 0usize);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let pending: PendingRecord = match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
        {
            Ok(b) => b,
            Err(e) => {
                warn!("Could not read pending playtime record {path:?}: {e}");
                failed += 1;
                continue;
            }
        };
        if signed_in != Some(pending.user_id.as_str()) {
            // Someone else's play. Sending it now would credit the account
            // that happens to be signed in.
            continue;
        }
        if !still_signed_in_as(signed_in) {
            info!("Account changed during the playtime drain; leaving the rest queued");
            break;
        }
        let body = &pending.record;
        match send_record(body).await {
            SendOutcome::Done => {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Could not delete drained playtime record {path:?}: {e}");
                }
                ok += 1;
            }
            SendOutcome::Rejected(reason) => {
                warn!(
                    "Server refused the queued playtime record for game {}, dropping it: {reason}",
                    body.game_id
                );
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Could not delete refused playtime record {path:?}: {e}");
                }
                failed += 1;
            }
            SendOutcome::Failed(e) => {
                warn!(
                    "Failed to drain playtime record for game {}: {e} - will retry at the next drain",
                    body.game_id
                );
                failed += 1;
            }
        }
    }
    (ok, failed)
}

// ── Pending-stop persistence ────────────────────────────────────────────────
//
// `stop_playtime` retries 3× with exponential backoff before giving up. If
// the user's network is down at game exit OR Drop is closed before the
// async stop task finishes, the session never reaches the server and the
// playtime is lost.
//
// To plug the gap we persist failed stops to disk and drain the queue on
// next app launch. Each queued stop is one JSON file
//   {DATA_ROOT_DIR}/pending-playtime-stops/{session_id}.json
// containing the session id, client-measured duration, and a queued-at
// timestamp (debug info only — the server uses the duration field).

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingStop {
    session_id: String,
    duration_secs: u32,
    /// The account whose session this is. A stop is only replayed while that
    /// account is signed in. `None` on files queued before this was recorded;
    /// those are sent as before (the server answers 403 for a session that is
    /// not the caller's, and the file is kept for the right account).
    #[serde(default)]
    user_id: Option<String>,
    /// Unix seconds when the stop was queued. Diagnostic only — useful when
    /// reading the file by hand to see how long ago a session was abandoned.
    queued_at: u64,
}

fn pending_stops_dir() -> PathBuf {
    DATA_ROOT_DIR.join("pending-playtime-stops")
}

fn pending_stop_path(session_id: &str) -> PathBuf {
    pending_stops_dir().join(format!("{session_id}.json"))
}

/// Persist a failed stop to disk so the next app launch can retry. Best-
/// effort: any I/O error is logged and swallowed — better to lose this one
/// session than to fail the whole on_process_finish path.
pub fn queue_pending_stop(session_id: &str, duration_secs: u32, user_id: Option<&str>) {
    let dir = pending_stops_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("Could not create pending-playtime-stops dir at {dir:?}: {e}");
        return;
    }

    let queued_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let entry = PendingStop {
        session_id: session_id.to_string(),
        duration_secs,
        user_id: user_id.map(str::to_string),
        queued_at,
    };
    let path = pending_stop_path(session_id);
    let body = match serde_json::to_vec_pretty(&entry) {
        Ok(v) => v,
        Err(e) => {
            warn!("Could not serialize pending stop for {session_id}: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::write(&path, body) {
        warn!("Could not write pending stop to {path:?}: {e}");
        return;
    }
    info!("Queued pending playtime stop at {path:?} ({duration_secs}s)");
}

/// Set while a drain runs, so the startup drain, a sign-in and a return to
/// online cannot replay the same files at the same time.
static DRAINING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Set by a drain that found another one running. The running drain goes
/// round again when it finishes, so a sign-in that arrived mid-drain still
/// gets its own account's items sent.
static DRAIN_AGAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Claim the drain, or ask the running one to go round again. True when the
/// caller now owns it.
fn begin_drain() -> bool {
    use std::sync::atomic::Ordering;
    if DRAINING.swap(true, Ordering::AcqRel) {
        DRAIN_AGAIN.store(true, Ordering::Release);
        return false;
    }
    DRAIN_AGAIN.store(false, Ordering::Release);
    true
}

/// Release the drain. True when another caller asked for a further round
/// meanwhile; the caller then tries [`begin_drain`] again. Checked after the
/// release, so a request that lands either side of it is never lost: before,
/// it is seen here; after, that caller claimed the drain itself.
fn end_drain() -> bool {
    use std::sync::atomic::Ordering;
    DRAINING.store(false, Ordering::Release);
    DRAIN_AGAIN.swap(false, Ordering::AcqRel)
}

/// Whether the account signed in now is still `expected`. Checked right
/// before every send: the request is signed with whoever is signed in at
/// that moment, and a sign-out and sign-in as someone else can happen in the
/// middle of a slow drain.
fn still_signed_in_as(expected: Option<&str>) -> bool {
    crate::save_sync::current_user_id().as_deref() == expected
}

/// Replays every queued stop, then every queued whole-session record (see
/// [`record_session`]), for the account signed in now. A file is deleted once
/// the server has it or has refused it for good; the rest stay for the next
/// drain. The counts cover both queues. Each item is only sent while the
/// account it was read for is still the one signed in; when that changes,
/// the drain stops and the rest waits.
///
/// Stops go first: a stop replaces the server's estimate for a session that
/// ended while the server was unreachable, and a record of the play that
/// followed is then measured against the real end instead of the estimate.
///
/// Called at startup, after a sign-in (or a switch to another account) and
/// when the app comes back online, always off the hot path. A call while
/// another drain is running returns (0, 0) at once and makes the running one
/// go round again, for whoever is signed in by then. Returns the count of
/// (succeeded, failed) for log visibility; callers can ignore it.
pub async fn drain_pending_stops() -> (usize, usize) {
    let (mut succeeded, mut failed) = (0usize, 0usize);
    while begin_drain() {
        // Released on panic too, or no drain would ever run again.
        struct Release(bool);
        impl Drop for Release {
            fn drop(&mut self) {
                if !self.0 {
                    DRAINING.store(false, std::sync::atomic::Ordering::Release);
                }
            }
        }
        let mut release = Release(false);

        let signed_in = crate::save_sync::current_user_id();
        let (stops_ok, stops_failed) = drain_queued_stops(signed_in.as_deref()).await;
        let (records_ok, records_failed) = drain_pending_records(signed_in.as_deref()).await;
        succeeded += stops_ok + records_ok;
        failed += stops_failed + records_failed;

        release.0 = true;
        if !end_drain() {
            break;
        }
    }
    if succeeded + failed > 0 {
        info!(
            "drain_pending_stops: {} succeeded, {} failed",
            succeeded, failed
        );
    }
    (succeeded, failed)
}

/// The stop half of [`drain_pending_stops`].
async fn drain_queued_stops(signed_in: Option<&str>) -> (usize, usize) {
    let dir = pending_stops_dir();
    if !dir.exists() {
        return (0, 0);
    }

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Could not read pending-playtime-stops dir at {dir:?}: {e}");
            return (0, 0);
        }
    };

    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let body = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                warn!("Could not read pending stop {path:?}: {e}");
                failed += 1;
                continue;
            }
        };
        let pending: PendingStop = match serde_json::from_slice(&body) {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "Could not parse pending stop {path:?} (deleting corrupt file): {e}"
                );
                let _ = std::fs::remove_file(&path);
                failed += 1;
                continue;
            }
        };

        if let Some(owner) = pending.user_id.as_deref()
            && signed_in != Some(owner)
        {
            // Another account's session: leave it for when they sign in.
            continue;
        }
        if !still_signed_in_as(signed_in) {
            info!("Account changed during the playtime drain; leaving the rest queued");
            break;
        }

        match send_stop(&pending.session_id, Some(pending.duration_secs)).await {
            SendOutcome::Rejected(reason) => {
                warn!(
                    "Server refused the queued stop for session {}, dropping it: {reason}",
                    pending.session_id
                );
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Could not delete refused pending stop {path:?}: {e}");
                }
                failed += 1;
            }
            SendOutcome::Done => {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Could not delete drained pending stop {path:?}: {e}");
                }
                info!(
                    "Drained pending playtime stop for session {} ({}s)",
                    pending.session_id, pending.duration_secs
                );
                succeeded += 1;
            }
            SendOutcome::Failed(e) => {
                warn!(
                    "Failed to drain pending stop for session {}: {} — will retry at the next drain",
                    pending.session_id, e
                );
                failed += 1;
            }
        }
    }

    (succeeded, failed)
}

#[cfg(test)]
mod tests {
    use super::{
        PendingRecord, PendingStop, begin_drain, end_drain, new_session_id,
        record_refusal_is_final, stop_refusal_is_final,
    };

    /// A drain asked for while one runs is not lost: the running one goes
    /// round again, and only one ever runs at a time.
    #[test]
    fn a_drain_requested_mid_drain_runs_afterwards() {
        assert!(begin_drain());
        // A sign-in arrives while the first drain is still sending.
        assert!(!begin_drain());
        // The first drain finishes, sees the request and claims a new round.
        assert!(end_drain());
        assert!(begin_drain());
        // Nobody asked during the second round: done.
        assert!(!end_drain());
        // And the drain is free again.
        assert!(begin_drain());
        assert!(!end_drain());
    }

    /// Only the routes' own definitive answers drop queued playtime. A server
    /// too old to have the record route, an expired sign-in or a token
    /// problem must leave it queued.
    #[test]
    fn only_the_routes_own_refusals_drop_queued_playtime() {
        assert!(record_refusal_is_final(400, r#"{"statusMessage":"Session is too old to record."}"#));
        assert!(record_refusal_is_final(404, r#"{"statusMessage":"Game not found."}"#));
        assert!(record_refusal_is_final(404, r#"{"statusCode":404,"statusMessage":"Not found."}"#));
        assert!(!record_refusal_is_final(
            409,
            r#"{"statusMessage":"Session is covered by other play."}"#
        ));
        assert!(record_refusal_is_final(
            409,
            r#"{"statusMessage":"No playtime left to credit for that day."}"#
        ));
        // A server without the route.
        assert!(!record_refusal_is_final(
            404,
            r#"{"statusCode":404,"statusMessage":"Page Not Found: /api/v1/client/playtime/record"}"#
        ));
        assert!(!record_refusal_is_final(404, ""));
        assert!(!record_refusal_is_final(409, "conflict"));
        assert!(!record_refusal_is_final(401, "Unauthorized"));
        assert!(!record_refusal_is_final(403, ""));
        assert!(!record_refusal_is_final(429, ""));
        assert!(!record_refusal_is_final(500, "boom"));

        assert!(stop_refusal_is_final(400, r#"{"statusMessage":"Session already ended."}"#));
        assert!(stop_refusal_is_final(404, r#"{"statusMessage":"Session not found."}"#));
        assert!(!stop_refusal_is_final(404, "Not Found"));
        assert!(!stop_refusal_is_final(403, r#"{"statusMessage":"Not your session."}"#));
        assert!(!stop_refusal_is_final(401, ""));
        assert!(!stop_refusal_is_final(502, ""));
    }

    /// Queued items carry the account that played. A stop file from before
    /// that field existed still loads.
    #[test]
    fn queued_items_keep_their_account() {
        let stop: PendingStop =
            serde_json::from_str(r#"{"sessionId":"s","durationSecs":5,"queuedAt":1}"#).unwrap();
        assert!(stop.user_id.is_none());
        let stop: PendingStop = serde_json::from_str(
            r#"{"sessionId":"s","durationSecs":5,"userId":"u1","queuedAt":1}"#,
        )
        .unwrap();
        assert_eq!(stop.user_id.as_deref(), Some("u1"));

        let json = r#"{"userId":"u1","record":{"sessionId":"s","gameId":"g",
            "startedAt":"t","clientDurationSecs":5}}"#;
        let record: PendingRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.user_id, "u1");
        let sent = serde_json::to_value(&record.record).unwrap();
        assert!(sent.get("userId").is_none());
    }

    /// The server validates the id as a UUID; anything else is a 400 that
    /// would leave the record queued forever.
    #[test]
    fn recorded_session_ids_are_uuid_shaped_and_distinct() {
        let a = new_session_id("game");
        let b = new_session_id("game");
        assert_ne!(a, b);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(a.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
        assert_eq!(&parts[2][..1], "4");
    }
}
