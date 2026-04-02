use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::{
    error::RemoteAccessError,
    goldberg,
    requests::{generate_url, make_authenticated_get, make_authenticated_post},
};

#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLink {
    pub provider: String,
    pub external_game_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AchievementConfigResponse {
    pub achievements: Vec<AchievementItem>,
    #[serde(default)]
    pub external_links: Vec<ExternalLink>,
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
    achievements: Vec<AchievementReportEntry>,
}

/// Response from the achievements-report endpoint
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AchievementReportResponse {
    pub recorded: u32,
}

/// Fetch achievement config for a game from the server
pub async fn fetch_achievement_config(
    game_id: &str,
) -> Result<AchievementConfigResponse, RemoteAccessError> {
    let url = generate_url(
        &[&format!(
            "/api/v1/client/game/{}/achievement-config",
            game_id
        )],
        &[],
    )?;
    let response = make_authenticated_get(url).await?;

    if response.status() != 200 {
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to fetch achievement config: {}",
            response.status()
        )));
    }

    let data: AchievementConfigResponse = response.json().await?;
    Ok(data)
}

/// Report achievement unlocks to the server.
/// The server records them and pushes real-time notifications.
pub async fn report_achievements(
    game_id: &str,
    achievements: Vec<AchievementReportEntry>,
) -> Result<AchievementReportResponse, RemoteAccessError> {
    let url = generate_url(
        &[&format!(
            "/api/v1/client/game/{}/achievements-report",
            game_id
        )],
        &[],
    )?;
    let body = AchievementReportBody { achievements };
    let response = make_authenticated_post(url, &body).await?;

    if response.status() != 200 {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to report achievements: {status} - {text}"
        )));
    }

    let data: AchievementReportResponse = response.json().await?;
    Ok(data)
}

/// Notify the server that a game session has ended, triggering server-side
/// achievement sync (Steam, RetroAchievements) for this user + game.
pub async fn notify_session_end(game_id: &str) -> Result<(), RemoteAccessError> {
    let url = generate_url(
        &[&format!("/api/v1/client/game/{}/session-end", game_id)],
        &[],
    )?;
    // Empty body — the server identifies user via client auth
    #[derive(Serialize)]
    struct Empty {}
    let response = make_authenticated_post(url, &Empty {}).await?;

    if response.status() != 200 {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Session-end sync failed: {status} - {text}");
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to notify session end: {status} - {text}"
        )));
    }

    info!("Session-end sync completed for game {}", game_id);
    Ok(())
}

/// Checks local Goldberg GSE save files for newly earned achievements and
/// reports them to the server. Returns the number of newly reported unlocks.
async fn check_and_report_goldberg(
    game_id: &str,
    goldberg_app_ids: &[String],
    known_unlocked_external_ids: &HashSet<String>,
) -> Vec<AchievementReportEntry> {
    let mut new_reports = Vec::new();

    for app_id in goldberg_app_ids {
        let earned = goldberg::read_goldberg_earned(app_id);
        for ach in earned {
            if known_unlocked_external_ids.contains(&ach.name) {
                continue;
            }

            // Convert unix timestamp to ISO 8601
            let unlocked_at = chrono::DateTime::from_timestamp(ach.earned_time as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            new_reports.push(AchievementReportEntry {
                external_id: ach.name.clone(),
                provider: "Goldberg".to_string(),
                unlocked_at,
            });
        }
    }

    new_reports
}

/// Polls for new achievement unlocks while a game is running.
/// Combines server-side polling with local Goldberg file checking.
/// Returns newly unlocked achievements via the callback.
pub async fn poll_achievements(
    game_id: String,
    cancel: Arc<tokio::sync::Notify>,
    on_new_achievement: impl Fn(AchievementItem) + Send + 'static,
) {
    // Fetch initial state from server (single request)
    let (mut known_unlocked, mut known_unlocked_external_ids, goldberg_app_ids) =
        match fetch_achievement_config(&game_id).await {
            Ok(data) => {
                let unlocked_ids: HashSet<String> = data
                    .achievements
                    .iter()
                    .filter(|a| a.unlocked)
                    .map(|a| a.id.clone())
                    .collect();

                let unlocked_ext_ids: HashSet<String> = data
                    .achievements
                    .iter()
                    .filter(|a| a.unlocked)
                    .map(|a| a.external_id.clone())
                    .collect();

                // Extract Goldberg AppIDs from external links
                let app_ids: Vec<String> = data
                    .external_links
                    .iter()
                    .filter(|l| l.provider == "Goldberg")
                    .map(|l| l.external_game_id.clone())
                    .collect();

                if !app_ids.is_empty() {
                    info!(
                        "Goldberg detection active for game {} with AppIDs: {:?}",
                        game_id, app_ids
                    );
                }

                (unlocked_ids, unlocked_ext_ids, app_ids)
            }
            Err(e) => {
                warn!(
                    "Failed to fetch initial achievement config for {}: {}",
                    game_id, e
                );
                (HashSet::new(), HashSet::new(), Vec::new())
            }
        };

    info!(
        "Achievement polling started for game {}. {} already unlocked.",
        game_id,
        known_unlocked.len()
    );

    loop {
        // Wait 15 seconds or until cancelled
        tokio::select! {
            _ = cancel.notified() => {
                // On session end, do one final Goldberg check
                if !goldberg_app_ids.is_empty() {
                    let final_reports = check_and_report_goldberg(
                        &game_id,
                        &goldberg_app_ids,
                        &known_unlocked_external_ids,
                    ).await;
                    if !final_reports.is_empty() {
                        info!("Final Goldberg sync: reporting {} achievements for {}", final_reports.len(), game_id);
                        if let Err(e) = report_achievements(&game_id, final_reports).await {
                            warn!("Failed to report final Goldberg achievements: {}", e);
                        }
                    }
                }
                info!("Achievement polling stopped for game {}", game_id);
                return;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(15)) => {}
        }

        // Check local Goldberg files first (fast, no network)
        if !goldberg_app_ids.is_empty() {
            let new_reports = check_and_report_goldberg(
                &game_id,
                &goldberg_app_ids,
                &known_unlocked_external_ids,
            )
            .await;

            if !new_reports.is_empty() {
                // Track these as known before reporting
                for r in &new_reports {
                    known_unlocked_external_ids.insert(r.external_id.clone());
                }

                info!(
                    "Reporting {} new Goldberg achievements for {}",
                    new_reports.len(),
                    game_id
                );
                if let Err(e) = report_achievements(&game_id, new_reports).await {
                    warn!("Failed to report Goldberg achievements: {}", e);
                }
            }
        }

        // Poll server for all achievement state changes (covers Steam, RA, and
        // Goldberg unlocks that were just reported above)
        match fetch_achievement_config(&game_id).await {
            Ok(data) => {
                for achievement in &data.achievements {
                    if achievement.unlocked && !known_unlocked.contains(&achievement.id) {
                        info!(
                            "New achievement unlocked: {} - {}",
                            achievement.title, achievement.description
                        );
                        known_unlocked.insert(achievement.id.clone());
                        known_unlocked_external_ids.insert(achievement.external_id.clone());
                        on_new_achievement(achievement.clone());
                    }
                }
            }
            Err(e) => {
                debug!(
                    "Achievement poll failed for {}: {} (will retry)",
                    game_id, e
                );
            }
        }
    }
}
