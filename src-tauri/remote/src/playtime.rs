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
/// `client_duration_secs` — if provided, the server uses this measured duration
/// instead of computing it from timestamps. More accurate when the server clock
/// is unreliable (NAS sleep, network delays, etc.).
pub async fn stop_playtime(session_id: &str, client_duration_secs: Option<u32>) -> Result<(), RemoteAccessError> {
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
            return Ok(());
        }

        // 400 "Session already ended" — not an error, just bail
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if status.as_u16() == 400 && text.contains("already ended") {
            info!("Playtime session {} was already ended", session_id);
            return Ok(());
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

    Err(last_err.unwrap_or_else(|| {
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
pub fn queue_pending_stop(session_id: &str, duration_secs: u32) {
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

/// Walks the pending-stops directory, replays each queued stop, and deletes
/// the file on success. Failed retries are left in place for the next drain.
///
/// Intended to be called once at startup after auth is confirmed, off the
/// hot path of app init. Returns the count of (succeeded, failed) for log
/// visibility; callers can ignore it.
pub async fn drain_pending_stops() -> (usize, usize) {
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

        match stop_playtime(&pending.session_id, Some(pending.duration_secs)).await {
            Ok(()) => {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Could not delete drained pending stop {path:?}: {e}");
                }
                info!(
                    "Drained pending playtime stop for session {} ({}s)",
                    pending.session_id, pending.duration_secs
                );
                succeeded += 1;
            }
            Err(e) => {
                warn!(
                    "Failed to drain pending stop for session {}: {} — will retry next launch",
                    pending.session_id, e
                );
                failed += 1;
            }
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
