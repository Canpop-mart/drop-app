use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    error::RemoteAccessError,
    requests::{generate_url, make_authenticated_post},
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

    let data: PlaytimeStartResponse = response.json().await?;
    info!("Started playtime session {} for game {}", data.session_id, game_id);
    Ok(data.session_id)
}

/// Stop a playtime session. This triggers server-side achievement sync.
pub async fn stop_playtime(session_id: &str) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/playtime/stop"], &[])?;
    let body = PlaytimeStopBody {
        session_id: session_id.to_string(),
    };

    let response = make_authenticated_post(url, &body).await?;

    if response.status() != 200 {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Failed to stop playtime session: {} - {}", status, text);
        return Err(RemoteAccessError::UnparseableResponse(
            format!("Failed to stop playtime: {status} - {text}"),
        ));
    }

    info!("Stopped playtime session {}", session_id);
    Ok(())
}
