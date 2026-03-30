use log::{debug, info, warn};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    error::RemoteAccessError,
    requests::{generate_url, make_authenticated_get},
};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AchievementItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon_url: String,
    pub unlocked: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AchievementsResponse {
    pub achievements: Vec<AchievementItem>,
    pub total_count: u32,
    pub unlocked_count: u32,
}

/// Fetch achievements for a game from the server
pub async fn fetch_achievements(game_id: &str) -> Result<AchievementsResponse, RemoteAccessError> {
    let url = generate_url(&[&format!("/api/v1/games/{}/achievements", game_id)], &[])?;
    let response = make_authenticated_get(url).await?;

    if response.status() != 200 {
        return Err(RemoteAccessError::UnparseableResponse(
            format!("Failed to fetch achievements: {}", response.status()),
        ));
    }

    let data: AchievementsResponse = response.json().await?;
    Ok(data)
}

/// Polls for new achievement unlocks while a game is running.
/// Returns newly unlocked achievements via the callback.
pub async fn poll_achievements(
    game_id: String,
    cancel: Arc<tokio::sync::Notify>,
    on_new_achievement: impl Fn(AchievementItem) + Send + 'static,
) {
    // Fetch initial state
    let mut known_unlocked: HashSet<String> = match fetch_achievements(&game_id).await {
        Ok(data) => data
            .achievements
            .iter()
            .filter(|a| a.unlocked)
            .map(|a| a.id.clone())
            .collect(),
        Err(e) => {
            warn!("Failed to fetch initial achievements for {}: {}", game_id, e);
            HashSet::new()
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
                info!("Achievement polling stopped for game {}", game_id);
                return;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(15)) => {}
        }

        match fetch_achievements(&game_id).await {
            Ok(data) => {
                for achievement in &data.achievements {
                    if achievement.unlocked && !known_unlocked.contains(&achievement.id) {
                        info!("New achievement unlocked: {} - {}", achievement.title, achievement.description);
                        known_unlocked.insert(achievement.id.clone());
                        on_new_achievement(achievement.clone());
                    }
                }
            }
            Err(e) => {
                debug!("Achievement poll failed for {}: {} (will retry)", game_id, e);
            }
        }
    }
}
