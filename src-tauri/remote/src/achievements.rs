use bitcode::{Decode, Encode};
use log::info;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::RemoteAccessError,
    goldberg::EmulatorInfo,
    requests::{RemoteRequest, generate_url, remote_request, remote_request_ok},
};

/// Prefix for all achievement debug logs — makes grep/filter easy.
const TAG: &str = "[ACH]";

pub mod journal;
pub mod notification;
pub mod tracking;

#[derive(Serialize, Deserialize, Clone, Debug, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct AchievementItem {
    pub id: String,
    pub external_id: String,
    pub provider: String,
    pub title: String,
    pub description: String,
    pub icon_url: String,
    pub unlocked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLink {
    pub provider: String,
    pub external_game_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct AchievementConfigResponse {
    #[serde(default)]
    pub tracking_states: Vec<TrackingState>,
    pub achievements: Vec<AchievementItem>,
    #[serde(default)]
    pub tracking_version: u8,
    #[serde(default = "initial_epoch")]
    pub reset_epoch: String,
    #[serde(default)]
    pub external_links: Vec<ExternalLink>,
    /// Why the server has no achievements for this game (`no_link`,
    /// `steam_key_missing`, `ra_credentials_missing`, `not_scanned`). Older
    /// servers omit it.
    #[serde(default)]
    pub reason: Option<String>,
    /// The game tracks through RetroAchievements and this player has no RA
    /// account linked on the server, so nothing they earn will be recorded.
    #[serde(default)]
    pub ra_account_missing: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct TrackingState {
    #[serde(default)]
    pub title: String,
    pub achievement_id: String,
    pub mode: String,
    pub unlocked: bool,
    pub confirmed: bool,
    pub current: Option<f64>,
    pub target: Option<f64>,
    pub unlocked_at: Option<String>,
}

/// A single achievement report entry sent from the client to the server
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AchievementReportEntry {
    pub external_id: String,
    pub provider: String,
    pub unlocked_at: String,
}

/// The body sent to POST /api/v1/client/game/{id}/achievements-report
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AchievementReportBody {
    /// This machine's clock at send time. The unlock times come from this
    /// clock too, so the server uses the difference to correct them onto its
    /// own clock before comparing them with an achievement reset.
    client_now: String,
    achievements: Vec<AchievementReportEntry>,
}

/// Response from the achievements-report endpoint
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AchievementReportResponse {
    /// Reports the server matched to a stored definition.
    pub recorded: u32,
    /// Rows the server actually created on this call (first-time unlocks).
    #[serde(default)]
    pub newly_unlocked: u32,
    /// The rows counted by `newly_unlocked`, so the client toasts exactly what
    /// the server just recorded. `None` from servers that predate the field.
    #[serde(default)]
    pub unlocks: Option<Vec<ReportedUnlock>>,
    /// Reports with NO matching server definition — a silent drop
    /// (externalId/definition mismatch). Older servers omit this field.
    #[serde(default)]
    pub skipped: u32,
    /// Matched reports earned before the player reset this game's
    /// achievements, so ignored. Older servers omit this field.
    #[serde(default)]
    pub ignored_before_reset: u32,
}

/// One unlock the server recorded in response to a report.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReportedUnlock {
    pub id: String,
    pub external_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_url: String,
}

/// Helper to get current time in seconds
fn initial_epoch() -> String {
    "initial".into()
}

/// The JSON sidecar cache is scoped to server, Drop user and paired device.
/// Fetch failures never choose a permanent disabled mode in the local scanner.
pub async fn fetch_achievement_config(
    game_id: &str,
) -> Result<AchievementConfigResponse, RemoteAccessError> {
    let scope = journal::Scope::current().ok_or(RemoteAccessError::Unauthorized)?;
    fetch_config_scoped(&scope, game_id).await
}

async fn fetch_config_scoped(
    scope: &journal::Scope,
    game_id: &str,
) -> Result<AchievementConfigResponse, RemoteAccessError> {
    let url = scope.url(&format!(
        "/api/v1/client/game/{game_id}/achievement-config?protocol=2"
    ))?;
    remote_request(RemoteRequest::get(url).for_client(&scope.client_id)?).await
}

/// Report achievement unlocks to the server.
/// The server records them and pushes real-time notifications.
pub async fn report_achievements(
    game_id: &str,
    achievements: Vec<AchievementReportEntry>,
) -> Result<AchievementReportResponse, RemoteAccessError> {
    let scope = journal::Scope::current().ok_or(RemoteAccessError::Unauthorized)?;
    report_scoped(&scope, game_id, achievements).await
}

async fn report_scoped(
    scope: &journal::Scope,
    game_id: &str,
    achievements: Vec<AchievementReportEntry>,
) -> Result<AchievementReportResponse, RemoteAccessError> {
    info!(
        "{TAG} Reporting {} achievements for game {game_id}: {:?}",
        achievements.len(),
        achievements
            .iter()
            .map(|a| &a.external_id)
            .collect::<Vec<_>>()
    );
    let url = scope.url(&format!(
        "/api/v1/client/game/{game_id}/achievements-report"
    ))?;
    let body = AchievementReportBody {
        client_now: chrono::Utc::now().to_rfc3339(),
        achievements,
    };
    let data: AchievementReportResponse =
        remote_request(RemoteRequest::post(url, &body).for_client(&scope.client_id)?).await?;
    info!(
        "{TAG} Server report for {game_id}: matched {}, newly unlocked {}, not-found {}, \
         ignored (earned before a reset) {}",
        data.recorded, data.newly_unlocked, data.skipped, data.ignored_before_reset
    );
    // The on-disk config copy is deliberately NOT cleared here. It is only
    // read when the server can't be reached (fetch_achievement_config is
    // network-first), and a stale unlock state in it is harmless now that
    // toasts come from this response rather than from diffing a config. Keeping
    // it means a launch during a network blip still knows which AppIDs to
    // watch, and reports succeed once the connection is back.
    Ok(data)
}

/// Response from the server-side RA poll endpoint
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct RAPollResponse {
    newly_unlocked: Vec<RAPollUnlock>,
    /// Why the server read nothing (`no_link`, `no_account`,
    /// `no_credentials`, `empty_progress`, `error`). Older servers omit it.
    #[serde(default)]
    skipped: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct RAPollUnlock {
    id: String,
    external_id: String,
    title: String,
    description: String,
    icon_url: String,
}

/// Poll the server for newly unlocked RetroAchievements during gameplay.
/// The server handles RA API calls and returns any new unlocks.
async fn poll_ra(scope: &journal::Scope, game_id: &str) -> Result<Vec<RAPollUnlock>, String> {
    let url = scope
        .url(&format!("/api/v1/client/game/{game_id}/ra-poll"))
        .map_err(|e| e.to_string())?;
    #[derive(Serialize)]
    struct Empty {}
    let body = Empty {};
    let request = RemoteRequest::post(url, &body)
        .for_client(&scope.client_id)
        .map_err(|e| e.to_string())?;
    let data: RAPollResponse = remote_request(request).await.map_err(|e| e.to_string())?;
    if let Some(reason) = data.skipped {
        return Err(match reason.as_str() {
            "no_link" => "This game has no RetroAchievements link.".into(),
            "no_account" => "Link your RetroAchievements account to sync achievements.".into(),
            "no_credentials" => {
                "RetroAchievements credentials are unavailable. Check the linked account.".into()
            }
            "empty_progress" => {
                "RetroAchievements has not returned progress for this game yet.".into()
            }
            "error" => "RetroAchievements sync failed. Try again.".into(),
            _ => format!("RetroAchievements: {reason}"),
        });
    }
    Ok(data.newly_unlocked)
}

/// Notify the server that a game session has ended, triggering the
/// server-side RetroAchievements sync for this user + game. (Goldberg/Steam
/// emulator unlocks are not synced here: they only ever arrive through
/// `report_achievements`.)
pub async fn notify_session_end(game_id: &str) -> Result<(), RemoteAccessError> {
    let url = generate_url(
        &[&format!("/api/v1/client/game/{}/session-end", game_id)],
        &[],
    )?;
    // Empty body — the server identifies user via client auth
    #[derive(Serialize)]
    struct Empty {}
    remote_request_ok(RemoteRequest::post(url, &Empty {})).await?;
    info!("Session-end sync completed for game {}", game_id);
    Ok(())
}

/// Event-based local capture, independent of the durable network worker.
pub async fn poll_achievements(
    game_id: String,
    emulator_info: Option<EmulatorInfo>,
    prepared: Option<tracking::PreparedTracking>,
    cancel: Arc<tokio::sync::Notify>,
    on_new: impl Fn(AchievementItem) + Send + 'static,
) {
    tracking::start_worker();
    tracking::run(game_id, emulator_info, prepared, cancel, on_new).await;
}
