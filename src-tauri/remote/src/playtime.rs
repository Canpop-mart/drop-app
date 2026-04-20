use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    error::RemoteAccessError,
    requests::{generate_url, make_authenticated_post},
    utils::{bounded_json, DEFAULT_JSON_CAP_BYTES},
};

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
pub async fn start_playtime(game_id: &str) -> Result<String, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/playtime/start"], &[])?;
    let body = PlaytimeStartBody {
        game_id: game_id.to_string(),
    };

    let response = make_authenticated_post(url, &body).await?;

    if response.status() != 200 {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to start playtime session: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(
            format!("Failed to start playtime: {status} - {text}"),
        ));
    }

    let data: PlaytimeStartResponse = bounded_json(response, DEFAULT_JSON_CAP_BYTES).await?;
    info!("Started playtime session {} for game {}", data.session_id, game_id);
    Ok(data.session_id)
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

        let response = match make_authenticated_post(url, &body).await {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Network error stopping playtime session {} (attempt {}): {}",
                    session_id,
                    attempt + 1,
                    e
                );
                last_err = Some(RemoteAccessError::UnparseableResponse(e.to_string()));
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

    match make_authenticated_post(url, &body).await {
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
