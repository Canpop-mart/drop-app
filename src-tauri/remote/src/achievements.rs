use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use bitcode::{Encode, Decode};

use crate::{
    error::RemoteAccessError,
    goldberg::{self, EmulatorInfo},
    requests::{generate_url, make_authenticated_get, make_authenticated_post},
    cache::{cache_object, get_cached_object},
    utils::{bounded_json, DEFAULT_JSON_CAP_BYTES},
};

/// Prefix for all achievement debug logs — makes grep/filter easy.
const TAG: &str = "[ACH]";

/// Which achievement provider this game uses — mutually exclusive.
#[derive(Debug, Clone)]
enum AchievementMode {
    /// Goldberg: poll local save files for unlock state
    Goldberg { app_ids: Vec<String> },
    /// RetroAchievements: poll server which checks RA API
    RetroAchievements,
    /// No provider linked — achievements won't be tracked
    None,
}

#[derive(Deserialize, Clone, Debug, Encode, Decode)]
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

#[derive(Deserialize, Clone, Debug, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLink {
    pub provider: String,
    pub external_game_id: String,
}

#[derive(Deserialize, Debug, Clone, Encode, Decode)]
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

/// Helper to get current time in seconds
fn get_current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Cache wrapper for achievement config with expiry info
#[derive(Encode, Decode, Clone)]
struct CachedAchievementConfig {
    data: AchievementConfigResponse,
    expiry: u64,
}

const ACHIEVEMENT_CONFIG_CACHE_TTL: u64 = 5 * 60; // 5 minutes in seconds

/// Fetch achievement config for a game from the server with 5-minute caching
pub async fn fetch_achievement_config(
    game_id: &str,
) -> Result<AchievementConfigResponse, RemoteAccessError> {
    let cache_key = format!("achievement-config/{}", game_id);

    // Try to get from cache first
    if let Ok(cached) = get_cached_object::<CachedAchievementConfig>(&cache_key) {
        let now = get_current_time_secs();
        if cached.expiry > now {
            debug!("{TAG} Using cached achievement config for game {game_id}");
            return Ok(cached.data);
        }
    }

    debug!("{TAG} Fetching achievement config for game {game_id}");
    let url = generate_url(
        &[&format!(
            "/api/v1/client/game/{}/achievement-config",
            game_id
        )],
        &[],
    )?;
    let response = make_authenticated_get(url).await?;

    if response.status() != 200 {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("{TAG} Config fetch failed for {game_id}: {status} — {text}");
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to fetch achievement config: {status}"
        )));
    }

    let data: AchievementConfigResponse = bounded_json(response, DEFAULT_JSON_CAP_BYTES).await?;
    debug!(
        "{TAG} Config for {game_id}: {} achievements, {} external links, {} already unlocked",
        data.achievements.len(),
        data.external_links.len(),
        data.achievements.iter().filter(|a| a.unlocked).count()
    );

    // Cache the config with 5-minute expiry
    let cached = CachedAchievementConfig {
        data: data.clone(),
        expiry: get_current_time_secs() + ACHIEVEMENT_CONFIG_CACHE_TTL,
    };
    if let Err(e) = cache_object(&cache_key, &cached) {
        debug!("{TAG} Failed to cache achievement config for {game_id}: {e}");
        // Don't fail the request if caching fails, just log it
    }

    Ok(data)
}

/// Report achievement unlocks to the server.
/// The server records them and pushes real-time notifications.
pub async fn report_achievements(
    game_id: &str,
    achievements: Vec<AchievementReportEntry>,
) -> Result<AchievementReportResponse, RemoteAccessError> {
    info!(
        "{TAG} Reporting {} achievements for game {game_id}: {:?}",
        achievements.len(),
        achievements.iter().map(|a| &a.external_id).collect::<Vec<_>>()
    );
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
        warn!("{TAG} Report failed for {game_id}: {status} — {text}");
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "Failed to report achievements: {status} - {text}"
        )));
    }

    let data: AchievementReportResponse = bounded_json(response, DEFAULT_JSON_CAP_BYTES).await?;
    info!("{TAG} Server recorded {} achievements for {game_id}", data.recorded);
    Ok(data)
}

/// Response from the server-side RA poll endpoint
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct RAPollResponse {
    newly_unlocked: Vec<RAPollUnlock>,
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
async fn poll_ra(game_id: &str) -> Vec<RAPollUnlock> {
    let url = match generate_url(
        &[&format!("/api/v1/client/game/{}/ra-poll", game_id)],
        &[],
    ) {
        Ok(u) => u,
        Err(e) => {
            warn!("{TAG} Failed to generate RA poll URL: {e}");
            return Vec::new();
        }
    };

    #[derive(Serialize)]
    struct Empty {}

    match make_authenticated_post(url, &Empty {}).await {
        Ok(response) => {
            let status = response.status();
            if status != 200 {
                warn!("{TAG} RA poll returned status {status} for game {game_id}");
                if let Ok(text) = response.text().await {
                    warn!("{TAG} RA poll error body: {text}");
                }
                return Vec::new();
            }
            match response.json::<RAPollResponse>().await {
                Ok(data) => {
                    if !data.newly_unlocked.is_empty() {
                        info!(
                            "{TAG} RA poll found {} new unlocks for {game_id}",
                            data.newly_unlocked.len()
                        );
                    }
                    data.newly_unlocked
                }
                Err(e) => {
                    warn!("{TAG} Failed to parse RA poll response: {e}");
                    Vec::new()
                }
            }
        }
        Err(e) => {
            warn!("{TAG} RA poll request failed for {game_id}: {e}");
            Vec::new()
        }
    }
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

/// Checks local emulator save files for newly earned achievements and
/// reports them to the server. Supports both Goldberg and SSE.
async fn check_and_report_local(
    game_id: &str,
    goldberg_app_ids: &[String],
    known_unlocked_external_ids: &HashSet<String>,
    emulator_info: Option<&EmulatorInfo>,
) -> Vec<AchievementReportEntry> {
    let mut new_reports = Vec::new();

    for app_id in goldberg_app_ids {
        info!("{TAG} Checking local files for AppID {app_id} (game {game_id})");

        // Use the unified reader that auto-selects based on emulator type
        let earned = goldberg::read_earned(app_id, emulator_info);
        info!(
            "{TAG} AppID {app_id}: {} earned achievements on disk, {} already known",
            earned.len(),
            known_unlocked_external_ids.len()
        );

        for ach in earned {
            if known_unlocked_external_ids.contains(&ach.name) {
                continue;
            }

            debug!(
                "{TAG} NEW achievement: '{}' earned_time={} (AppID {app_id})",
                ach.name, ach.earned_time
            );

            // Convert unix timestamp to ISO 8601.
            // Validate range: must be between 2000-01-01 and 2100-01-01.
            // Corrupted save files can produce bogus timestamps.
            const MIN_TS: u64 = 946_684_800;  // 2000-01-01
            const MAX_TS: u64 = 4_102_444_800; // 2100-01-01
            let unlocked_at = if ach.earned_time >= MIN_TS && ach.earned_time <= MAX_TS {
                chrono::DateTime::from_timestamp(ach.earned_time as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
            } else {
                if ach.earned_time > 0 {
                    debug!(
                        "{TAG} Suspicious timestamp {} for '{}', using current time",
                        ach.earned_time, ach.name
                    );
                }
                chrono::Utc::now().to_rfc3339()
            };

            new_reports.push(AchievementReportEntry {
                external_id: ach.name.clone(),
                provider: "Goldberg".to_string(),
                unlocked_at,
            });
        }
    }

    if new_reports.is_empty() {
        info!("{TAG} No new local achievements for game {game_id}");
    }
    new_reports
}

/// Cache entry for achievement config with timestamp
struct AchievementConfigCache {
    data: AchievementConfigResponse,
    cached_at: SystemTime,
}

impl AchievementConfigCache {
    fn is_stale(&self, cache_duration_secs: u64) -> bool {
        match SystemTime::now().duration_since(self.cached_at) {
            Ok(elapsed) => elapsed.as_secs() >= cache_duration_secs,
            Err(_) => true, // If time went backwards, consider it stale
        }
    }
}

/// Polls for new achievement unlocks while a game is running.
/// Detects provider mode (Goldberg OR RetroAchievements) from external links
/// and polls accordingly. Never runs both simultaneously.
///
/// `emulator_info` describes which emulator the game uses and where
/// to find its save files (only used in Goldberg mode).
pub async fn poll_achievements(
    game_id: String,
    emulator_info: Option<EmulatorInfo>,
    cancel: Arc<tokio::sync::Notify>,
    on_new_achievement: impl Fn(AchievementItem) + Send + 'static,
) {
    info!("{TAG} Starting achievement polling for game {game_id}");

    if let Some(info) = &emulator_info {
        info!("{TAG} Emulator: {:?}", info.emulator);
    }

    // Fetch initial state from server
    let (mut known_unlocked, mut known_unlocked_external_ids, mode) =
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

                // Determine provider mode — RA takes priority if linked,
                // otherwise fall back to Goldberg
                let ra_linked = data
                    .external_links
                    .iter()
                    .any(|l| l.provider == "RetroAchievements");

                let goldberg_app_ids: Vec<String> = data
                    .external_links
                    .iter()
                    .filter(|l| l.provider == "Goldberg")
                    .map(|l| l.external_game_id.clone())
                    .collect();

                let mode = if ra_linked {
                    info!("{TAG} Mode: RetroAchievements (game {game_id})");
                    AchievementMode::RetroAchievements
                } else if !goldberg_app_ids.is_empty() {
                    info!(
                        "{TAG} Mode: Goldberg (game {game_id}, AppIDs: {:?})",
                        goldberg_app_ids
                    );
                    AchievementMode::Goldberg {
                        app_ids: goldberg_app_ids,
                    }
                } else {
                    warn!(
                        "{TAG} No external links found for game {game_id} — \
                         achievements will not be tracked."
                    );
                    AchievementMode::None
                };

                info!(
                    "{TAG} Initial state for {game_id}: {} total achievements, {} unlocked",
                    data.achievements.len(),
                    unlocked_ids.len(),
                );

                (unlocked_ids, unlocked_ext_ids, mode)
            }
            Err(e) => {
                warn!(
                    "{TAG} FAILED to fetch initial config for {game_id}: {e} — \
                     achievements will not be tracked this session!"
                );
                (HashSet::new(), HashSet::new(), AchievementMode::None)
            }
        };

    // If no provider, just wait for cancellation
    if matches!(mode, AchievementMode::None) {
        cancel.notified().await;
        info!("{TAG} Achievement polling stopped for game {game_id}");
        return;
    }

    let mut first_poll = true;
    let mut cached_config: Option<AchievementConfigCache> = None;
    // Only re-fetch config every 3 cycles (45 seconds), reuse cache otherwise
    const CONFIG_CACHE_DURATION_SECS: u64 = 45;

    loop {
        // Wait 15 seconds or until cancelled
        tokio::select! {
            _ = cancel.notified() => {
                // On session end, do one final check for Goldberg mode
                if let AchievementMode::Goldberg { app_ids } = &mode {
                    let final_reports = check_and_report_local(
                        &game_id,
                        app_ids,
                        &known_unlocked_external_ids,
                        emulator_info.as_ref(),
                    ).await;
                    if !final_reports.is_empty() {
                        info!("{TAG} Final sync: reporting {} achievements for {}", final_reports.len(), game_id);
                        if let Err(e) = report_achievements(&game_id, final_reports).await {
                            warn!("{TAG} Failed to report final achievements: {}", e);
                        }
                    }
                }
                // For RA mode, do one final poll
                if matches!(mode, AchievementMode::RetroAchievements) {
                    let new_unlocks = poll_ra(&game_id).await;
                    for unlock in &new_unlocks {
                        if !known_unlocked.contains(&unlock.id) {
                            info!("{TAG} Final RA unlock: {} - {}", unlock.title, unlock.description);
                            known_unlocked.insert(unlock.id.clone());
                            on_new_achievement(AchievementItem {
                                id: unlock.id.clone(),
                                external_id: unlock.external_id.clone(),
                                provider: "RetroAchievements".to_string(),
                                title: unlock.title.clone(),
                                description: unlock.description.clone(),
                                icon_url: unlock.icon_url.clone(),
                                unlocked: true,
                            });
                        }
                    }
                }
                info!("{TAG} Achievement polling stopped for game {game_id}");
                return;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(15)) => {}
        }

        match &mode {
            AchievementMode::Goldberg { app_ids } => {
                // On first poll, run GBE diagnostics
                if first_poll {
                    first_poll = false;
                    if let Some(info) = &emulator_info {
                        goldberg::check_gbe_activity(info.dll_dir());
                    }
                }

                // Check local emulator files (fast, no network)
                let new_reports = check_and_report_local(
                    &game_id,
                    app_ids,
                    &known_unlocked_external_ids,
                    emulator_info.as_ref(),
                )
                .await;

                if !new_reports.is_empty() {
                    info!(
                        "{TAG} Reporting {} new Goldberg achievements for {}",
                        new_reports.len(),
                        game_id
                    );
                    match report_achievements(&game_id, new_reports.clone()).await {
                        Ok(_) => {
                            for r in &new_reports {
                                known_unlocked_external_ids.insert(r.external_id.clone());
                            }
                        }
                        Err(e) => {
                            warn!("{TAG} Failed to report achievements: {}", e);
                        }
                    }
                }

                // Poll server for state changes only if cache is stale (every 45 secs)
                let should_fetch = cached_config
                    .as_ref()
                    .map(|c| c.is_stale(CONFIG_CACHE_DURATION_SECS))
                    .unwrap_or(true);

                if should_fetch {
                    match fetch_achievement_config(&game_id).await {
                        Ok(data) => {
                            cached_config = Some(AchievementConfigCache {
                                data: data.clone(),
                                cached_at: SystemTime::now(),
                            });

                            for achievement in &data.achievements {
                                if achievement.unlocked && !known_unlocked.contains(&achievement.id) {
                                    info!(
                                        "{TAG} New achievement unlocked: {} - {}",
                                        achievement.title, achievement.description
                                    );
                                    known_unlocked.insert(achievement.id.clone());
                                    known_unlocked_external_ids
                                        .insert(achievement.external_id.clone());
                                    on_new_achievement(achievement.clone());
                                }
                            }
                        }
                        Err(e) => {
                            debug!(
                                "{TAG} Achievement poll failed for {}: {} (will retry)",
                                game_id, e
                            );
                        }
                    }
                } else if let Some(cache) = &cached_config {
                    // Use cached config to check for new unlocks without fetching
                    for achievement in &cache.data.achievements {
                        if achievement.unlocked && !known_unlocked.contains(&achievement.id) {
                            info!(
                                "{TAG} New achievement unlocked (from cache): {} - {}",
                                achievement.title, achievement.description
                            );
                            known_unlocked.insert(achievement.id.clone());
                            known_unlocked_external_ids
                                .insert(achievement.external_id.clone());
                            on_new_achievement(achievement.clone());
                        }
                    }
                }
            }

            AchievementMode::RetroAchievements => {
                first_poll = false;

                info!("{TAG} Polling RA for game {game_id} (known unlocked: {})", known_unlocked.len());
                // Poll the server which checks RA API for new unlocks
                let new_unlocks = poll_ra(&game_id).await;

                for unlock in &new_unlocks {
                    if !known_unlocked.contains(&unlock.id) {
                        info!(
                            "{TAG} RA achievement unlocked: {} - {}",
                            unlock.title, unlock.description
                        );
                        known_unlocked.insert(unlock.id.clone());
                        known_unlocked_external_ids.insert(unlock.external_id.clone());
                        on_new_achievement(AchievementItem {
                            id: unlock.id.clone(),
                            external_id: unlock.external_id.clone(),
                            provider: "RetroAchievements".to_string(),
                            title: unlock.title.clone(),
                            description: unlock.description.clone(),
                            icon_url: unlock.icon_url.clone(),
                            unlocked: true,
                        });
                    }
                }
            }

            AchievementMode::None => unreachable!(),
        }
    }
}
