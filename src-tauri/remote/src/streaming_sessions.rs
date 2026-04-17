use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    error::RemoteAccessError,
    requests::{generate_url, make_authenticated_get, make_authenticated_post},
};

// ── Request bodies ──────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartSessionBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sunshine_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_local_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_external_ip: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadyBody {
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pairing_pin: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionIdBody {
    session_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HeartbeatBody {
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestStreamBody {
    game_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_client_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptRequestBody {
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sunshine_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_local_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pairing_pin: Option<String>,
}

// ── Response types ──────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionResponse {
    pub session_id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StreamingSessionHost {
    pub id: String,
    pub name: String,
    pub platform: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StreamingSessionGame {
    pub id: String,
    #[serde(rename = "mName")]
    pub m_name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StreamingSession {
    pub id: String,
    pub status: String,
    pub host_client: StreamingSessionHost,
    pub game: Option<StreamingSessionGame>,
    pub sunshine_port: u16,
    pub host_local_ip: Option<String>,
    pub host_external_ip: Option<String>,
    pub has_pairing_pin: bool,
    pub created_at: String,
    pub last_heartbeat: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StreamingConnectionInfo {
    pub id: String,
    pub status: String,
    pub host_client: StreamingSessionHost,
    pub game: Option<StreamingSessionGame>,
    pub sunshine_port: u16,
    pub host_local_ip: Option<String>,
    pub host_external_ip: Option<String>,
    pub pairing_pin: Option<String>,
}

// ── API functions ───────────────────────────────────────────────────

/// Register a new streaming session on the server.
pub async fn start_streaming_session(
    game_id: Option<&str>,
    host_local_ip: Option<&str>,
) -> Result<String, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/start"], &[])?;
    let body = StartSessionBody {
        game_id: game_id.map(|s| s.to_string()),
        sunshine_port: None,
        host_local_ip: host_local_ip.map(|s| s.to_string()),
        host_external_ip: None,
    };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to start streaming session: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to start streaming session: {status} - {text}"
        )));
    }

    let data: StartSessionResponse = response.json().await?;
    info!("Started streaming session {}", data.session_id);
    Ok(data.session_id)
}

/// Mark a streaming session as ready (Sunshine is up and accepting connections).
pub async fn mark_session_ready(
    session_id: &str,
    pairing_pin: Option<&str>,
) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/ready"], &[])?;
    let body = ReadyBody {
        session_id: session_id.to_string(),
        pairing_pin: pairing_pin.map(|s| s.to_string()),
    };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to mark session ready: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to mark session ready: {status} - {text}"
        )));
    }

    info!("Marked streaming session {} as ready", session_id);
    Ok(())
}

/// Stop a streaming session.
pub async fn stop_streaming_session(session_id: &str) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/stop"], &[])?;
    let body = SessionIdBody {
        session_id: session_id.to_string(),
    };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to stop streaming session: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to stop streaming session: {status} - {text}"
        )));
    }

    info!("Stopped streaming session {}", session_id);
    Ok(())
}

/// Send a heartbeat for an active streaming session.
pub async fn heartbeat_streaming(
    session_id: &str,
    status: Option<&str>,
) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/heartbeat"], &[])?;
    let body = HeartbeatBody {
        session_id: session_id.to_string(),
        status: status.map(|s| s.to_string()),
    };

    let response = make_authenticated_post(url, &body).await?;

    if response.status().is_success() {
        info!("Heartbeat sent for streaming session {}", session_id);
        Ok(())
    } else {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!(
            "Heartbeat failed for streaming session {}: {} - {}",
            session_id, status, text
        );
        Err(RemoteAccessError::UnparseableResponse(format!(
            "Heartbeat failed: {status} - {text}"
        )))
    }
}

/// List all active streaming sessions for this user.
pub async fn list_streaming_sessions() -> Result<Vec<StreamingSession>, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/sessions"], &[])?;
    let response = make_authenticated_get(url).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to list streaming sessions: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to list streaming sessions: {status} - {text}"
        )));
    }

    let sessions: Vec<StreamingSession> = response.json().await?;
    info!("Fetched {} streaming sessions", sessions.len());
    Ok(sessions)
}

/// Get connection info for joining a streaming session.
pub async fn get_streaming_connection_info(
    session_id: &str,
) -> Result<StreamingConnectionInfo, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/connect"], &[])?;
    let body = SessionIdBody {
        session_id: session_id.to_string(),
    };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to get connection info: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to get connection info: {status} - {text}"
        )));
    }

    let info: StreamingConnectionInfo = response.json().await?;
    info!("Got connection info for session {}", session_id);
    Ok(info)
}

// ── Push-based streaming (client requests, host fulfills) ──────────

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PendingStreamRequest {
    pub session_id: String,
    pub game_id: Option<String>,
    pub game: Option<StreamingSessionGame>,
    pub requesting_client: Option<StreamingSessionHost>,
    pub created_at: String,
}

/// Request a stream from another client (called by the receiving device).
/// If `target_client_id` is provided, only that specific device will see the request.
pub async fn request_stream(
    game_id: &str,
    target_client_id: Option<&str>,
) -> Result<String, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/request"], &[])?;
    let body = RequestStreamBody {
        game_id: game_id.to_string(),
        target_client_id: target_client_id.map(|s| s.to_string()),
    };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to request stream: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to request stream: {status} - {text}"
        )));
    }

    let data: StartSessionResponse = response.json().await?;
    info!("Requested stream, session {}", data.session_id);
    Ok(data.session_id)
}

/// Poll for pending stream requests from other clients.
pub async fn poll_pending_requests() -> Result<Vec<PendingStreamRequest>, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/pending-requests"], &[])?;
    let response = make_authenticated_get(url).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to poll pending requests: {status} - {text}"
        )));
    }

    let requests: Vec<PendingStreamRequest> = response.json().await?;
    Ok(requests)
}

/// Accept a pending stream request (called by the host).
pub async fn accept_stream_request(
    session_id: &str,
    pairing_pin: Option<&str>,
    host_local_ip: Option<&str>,
) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/accept"], &[])?;
    let body = AcceptRequestBody {
        session_id: session_id.to_string(),
        sunshine_port: None,
        host_local_ip: host_local_ip.map(|s| s.to_string()),
        pairing_pin: pairing_pin.map(|s| s.to_string()),
    };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to accept stream request: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to accept stream request: {status} - {text}"
        )));
    }

    info!("Accepted stream request {}", session_id);
    Ok(())
}

// ── Device listing ───────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub last_connected: String,
    pub is_self: bool,
    #[serde(default)]
    pub has_game: Option<bool>,
}

/// List all registered client devices for the current user.
/// If `game_id` is provided, each device includes `has_game` indicating install status.
pub async fn list_devices(game_id: Option<&str>) -> Result<Vec<ClientDevice>, RemoteAccessError> {
    let query: Vec<(&str, &str)> = match game_id {
        Some(gid) => vec![("gameId", gid)],
        None => vec![],
    };
    let url = generate_url(&["/api/v1/client/devices"], &query)?;
    let response = make_authenticated_get(url).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to list devices: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to list devices: {status} - {text}"
        )));
    }

    let devices: Vec<ClientDevice> = response.json().await?;
    info!("Fetched {} devices", devices.len());
    Ok(devices)
}

// ── Remote install ───────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteInstallBody {
    game_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_client_id: Option<String>,
}

/// Request a remote install of a game on another device.
pub async fn request_remote_install(
    game_id: &str,
    target_client_id: Option<&str>,
) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/streaming/remote-install"], &[])?;
    let body = RemoteInstallBody {
        game_id: game_id.to_string(),
        target_client_id: target_client_id.map(|s| s.to_string()),
    };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to request remote install: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to request remote install: {status} - {text}"
        )));
    }

    info!("Requested remote install for game {}", game_id);
    Ok(())
}

// ── Installed games sync ─────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncInstalledBody {
    game_ids: Vec<String>,
}

/// Report this client's installed game IDs to the server.
pub async fn sync_installed_games(game_ids: Vec<String>) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/sync-installed"], &[])?;
    let body = SyncInstalledBody { game_ids };

    let response = make_authenticated_post(url, &body).await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to sync installed games: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to sync installed games: {status} - {text}"
        )));
    }

    info!("Synced installed games to server");
    Ok(())
}
