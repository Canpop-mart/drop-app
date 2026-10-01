use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use bitcode::{Encode, Decode};

use crate::{
    error::RemoteAccessError,
    goldberg::{self, EmulatorInfo},
    requests::{generate_url, remote_request, remote_request_ok, RemoteRequest},
    cache::{cache_object, get_cached_object},
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
fn get_current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Last good achievement config for a game, kept on disk so a launch with the
/// server unreachable still knows which provider/AppIDs to watch.
#[derive(Encode, Decode, Clone)]
struct CachedAchievementConfig {
    data: AchievementConfigResponse,
    /// Unix seconds when this copy was fetched. Only used for logging.
    fetched_at: u64,
}

fn config_cache_key(game_id: &str) -> String {
    format!("achievement-config/{game_id}")
}

/// Fetch the achievement config for a game from the server.
///
/// Always asks the server first. The unlock state in it must be current: the
/// poller seeds its "already unlocked" set from it, and a stale copy (the old
/// five-minute cache) made a relaunch re-announce the previous session's
/// unlocks. The on-disk copy is only a fallback for when the server can't be
/// reached, so an offline launch still knows the game's provider and AppIDs.
pub async fn fetch_achievement_config(
    game_id: &str,
) -> Result<AchievementConfigResponse, RemoteAccessError> {
    let cache_key = config_cache_key(game_id);

    debug!("{TAG} Fetching achievement config for game {game_id}");
    let url = generate_url(
        &[&format!(
            "/api/v1/client/game/{}/achievement-config",
            game_id
        )],
        &[],
    )?;
    let data: AchievementConfigResponse = match remote_request(RemoteRequest::get(url)).await {
        Ok(data) => data,
        Err(e) => {
            if let Ok(cached) = get_cached_object::<CachedAchievementConfig>(&cache_key) {
                let age = get_current_time_secs().saturating_sub(cached.fetched_at);
                warn!(
                    "{TAG} Config fetch for {game_id} failed ({e}); using the copy from {age}s ago"
                );
                return Ok(cached.data);
            }
            return Err(e);
        }
    };
    debug!(
        "{TAG} Config for {game_id}: {} achievements, {} external links, {} already unlocked",
        data.achievements.len(),
        data.external_links.len(),
        data.achievements.iter().filter(|a| a.unlocked).count()
    );

    let cached = CachedAchievementConfig {
        data: data.clone(),
        fetched_at: get_current_time_secs(),
    };
    if let Err(e) = cache_object(&cache_key, &cached) {
        // Only the offline fallback is lost; the fresh data is still returned.
        debug!("{TAG} Failed to cache achievement config for {game_id}: {e}");
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
    let body = AchievementReportBody {
        client_now: chrono::Utc::now().to_rfc3339(),
        achievements,
    };
    let data: AchievementReportResponse =
        remote_request(RemoteRequest::post(url, &body)).await?;
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

    match remote_request::<RAPollResponse, _>(RemoteRequest::post(url, &Empty {})).await {
        Ok(data) => {
            if let Some(reason) = &data.skipped {
                // Once per poll while it lasts, so debug; the launch-time
                // warning below says it once at info/warn level.
                debug!("{TAG} RA poll for {game_id} read nothing: {reason}");
            }
            if !data.newly_unlocked.is_empty() {
                info!(
                    "{TAG} RA poll found {} new unlocks for {game_id}",
                    data.newly_unlocked.len()
                );
            }
            data.newly_unlocked
        }
        Err(e) => {
            warn!("{TAG} RA poll request failed for {game_id}: {e}");
            Vec::new()
        }
    }
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

/// Checks local emulator save files for newly earned achievements and
/// returns the ones not yet known to be unlocked, ready to report. Supports
/// both Goldberg and SSE, plus the cracker locations, on the host and (Linux)
/// inside the game's Wine prefix.
async fn check_and_report_local(
    game_id: &str,
    goldberg_app_ids: &[String],
    known_unlocked_external_ids: &HashSet<String>,
    emulator_info: Option<&EmulatorInfo>,
    wine_prefix: Option<&std::path::Path>,
) -> Vec<AchievementReportEntry> {
    let mut new_reports = Vec::new();

    for app_id in goldberg_app_ids {
        // Both of these fire on every poll of a running game, i.e. four times a
        // minute per launch, and neither says anything until the count changes.
        debug!("{TAG} Checking local files for AppID {app_id} (game {game_id})");

        // Use the unified reader that auto-selects based on emulator type
        let earned = goldberg::read_earned(app_id, emulator_info, wine_prefix);
        debug!(
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
                // Clamp timestamps in the future (clock skew / bad save) to now
                // so unlock ordering on the server/UI stays sane.
                let now = get_current_time_secs();
                let ts = if ach.earned_time > now { now } else { ach.earned_time };
                chrono::DateTime::from_timestamp(ts as i64, 0)
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
        debug!("{TAG} No new local achievements for game {game_id}");
    }
    new_reports
}

/// Reports locally-found unlocks and toasts exactly the ones the server says
/// it recorded for the first time. Used by every poll tick and by the final
/// check when the game exits.
///
/// Every reported name is marked known afterwards, whatever the server said
/// about it: a name the server couldn't match (or ignored as earned before a
/// reset) would otherwise be re-sent every 15 s. The save file persists, so a
/// report that genuinely failed is retried next launch (the known set is
/// reseeded from the server). On a network error nothing is marked, so the
/// next tick retries.
async fn report_and_announce(
    game_id: &str,
    reports: Vec<AchievementReportEntry>,
    known_unlocked: &mut HashSet<String>,
    known_unlocked_external_ids: &mut HashSet<String>,
    on_new_achievement: &(impl Fn(AchievementItem) + Send),
) {
    if reports.is_empty() {
        return;
    }
    info!(
        "{TAG} Reporting {} new local achievements for {}",
        reports.len(),
        game_id
    );
    let resp = match report_achievements(game_id, reports.clone()).await {
        Ok(resp) => resp,
        Err(e) => {
            warn!("{TAG} Failed to report achievements for {game_id}: {e} (will retry)");
            return;
        }
    };
    if resp.skipped > 0 {
        warn!(
            "{TAG} {} of {} reported achievements for {} had NO matching \
             server definition (externalId/definition mismatch) and were dropped",
            resp.skipped,
            reports.len(),
            game_id
        );
    }
    for r in &reports {
        known_unlocked_external_ids.insert(r.external_id.clone());
    }

    match resp.unlocks {
        Some(unlocks) => {
            for u in unlocks {
                if !known_unlocked.insert(u.id.clone()) {
                    continue;
                }
                known_unlocked_external_ids.insert(u.external_id.clone());
                info!("{TAG} New achievement unlocked: {} - {}", u.title, u.description);
                on_new_achievement(AchievementItem {
                    id: u.id,
                    external_id: u.external_id,
                    provider: "Goldberg".to_string(),
                    title: u.title,
                    description: u.description,
                    icon_url: u.icon_url,
                    unlocked: true,
                });
            }
        }
        // A server from before `unlocks` existed: fall back to a fresh config
        // diff, only when it says it created something.
        None if resp.newly_unlocked > 0 => match fetch_achievement_config(game_id).await {
            Ok(data) => {
                for a in data.achievements {
                    if a.unlocked && known_unlocked.insert(a.id.clone()) {
                        known_unlocked_external_ids.insert(a.external_id.clone());
                        info!("{TAG} New achievement unlocked: {} - {}", a.title, a.description);
                        on_new_achievement(a);
                    }
                }
            }
            Err(e) => warn!(
                "{TAG} {} new unlock(s) recorded for {game_id} but the config refetch \
                 for the toast failed: {e}",
                resp.newly_unlocked
            ),
        },
        None => {}
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

                // AppIDs to scan locally: the "Goldberg" links. The server
                // files every Steam-style game under Goldberg and never
                // creates a "Steam"-provider link today, so the "Steam" arm
                // below matches nothing; it is kept so a server that does add
                // them later gets scanned without a client change. Games with
                // no server link at all are covered by the local AppID fold-in
                // just below.
                let mut goldberg_app_ids: Vec<String> = data
                    .external_links
                    .iter()
                    .filter(|l| l.provider == "Goldberg" || l.provider == "Steam")
                    .map(|l| l.external_game_id.clone())
                    .collect();
                goldberg_app_ids.sort();
                goldberg_app_ids.dedup();

                // Fold in the game's own on-disk Steam AppID when a Goldberg-
                // family emulator was detected locally. This (a) lets a game with
                // no server-side Goldberg link still be tracked, and (b) catches a
                // server AppID that doesn't match the folder GBE actually writes
                // to — without it those unlocks are read by nobody. RA still wins
                // below if the game is RA-linked.
                if !ra_linked
                    && let Some(info) = &emulator_info
                    && matches!(
                        info.emulator,
                        goldberg::SteamEmulator::Goldberg { .. }
                            | goldberg::SteamEmulator::Unknown { .. }
                    )
                    && let Some(local_id) = goldberg::read_local_steam_appid(info.dll_dir())
                    && !goldberg_app_ids.contains(&local_id)
                {
                    info!("{TAG} Using locally-detected Goldberg AppID {local_id} for {game_id}");
                    goldberg_app_ids.push(local_id);
                }

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
                if let Some(reason) = &data.reason {
                    warn!("{TAG} Server has no achievements for {game_id}: {reason}");
                }
                if ra_linked && data.ra_account_missing {
                    warn!(
                        "{TAG} {game_id} tracks through RetroAchievements but this account \
                         has no RetroAchievements account linked on the server: unlocks \
                         will NOT be recorded"
                    );
                }

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
    // The game's Proton prefix on Linux, where a Windows build writes its
    // AppData / Documents saves. Looked up once; `None` elsewhere.
    let wine_prefix = goldberg::wine_prefix_for_game(&game_id);
    if let Some(p) = &wine_prefix {
        info!("{TAG} Also searching the Wine prefix {} for {game_id}", p.display());
    }

    // A fixed-cadence interval rather than `sleep(15s)` at the end of each
    // cycle: `sleep` would make the real period `15s + poll_duration`, so the
    // interval would drift slower the slower the server is. `interval` ticks
    // on a fixed 15s grid. `MissedTickBehavior::Skip` means that if one poll
    // cycle ever overruns 15s, the missed ticks are dropped — the next poll
    // waits for the next grid point instead of firing back-to-back, so polls
    // can never stack up.
    let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(15));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // The first `tick()` returns immediately; consume it so the loop's first
    // real poll still happens ~15s in, matching the previous behaviour.
    ticker.tick().await;

    loop {
        // Wait for the next 15s tick or until cancelled
        tokio::select! {
            _ = cancel.notified() => {
                // On session end, do one final check for Goldberg mode. An
                // unlock earned in the last seconds before quitting is only
                // seen here, and it toasts like any other.
                if let AchievementMode::Goldberg { app_ids } = &mode {
                    let final_reports = check_and_report_local(
                        &game_id,
                        app_ids,
                        &known_unlocked_external_ids,
                        emulator_info.as_ref(),
                        wine_prefix.as_deref(),
                    ).await;
                    if !final_reports.is_empty() {
                        info!("{TAG} Final sync: {} unreported achievements for {}", final_reports.len(), game_id);
                    }
                    report_and_announce(
                        &game_id,
                        final_reports,
                        &mut known_unlocked,
                        &mut known_unlocked_external_ids,
                        &on_new_achievement,
                    )
                    .await;
                }
                // For RA mode, do a few *delayed* final polls. RetroArch reports
                // unlocks to RA's servers asynchronously, so an achievement
                // earned right before quitting usually isn't visible to the RA
                // API yet — a single immediate poll misses it. Retry a handful
                // of times with a short delay so the last unlocks still reach
                // the UI instead of only being reconciled silently server-side.
                if matches!(mode, AchievementMode::RetroAchievements) {
                    for attempt in 0..3 {
                        if attempt > 0 {
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        }
                        for unlock in &poll_ra(&game_id).await {
                            if !known_unlocked_external_ids.contains(&unlock.external_id) {
                                info!("{TAG} Final RA unlock: {} - {}", unlock.title, unlock.description);
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
                }
                info!("{TAG} Achievement polling stopped for game {game_id}");
                return;
            }
            _ = ticker.tick() => {}
        }

        match &mode {
            AchievementMode::Goldberg { app_ids } => {
                // On first poll, run GBE diagnostics. If the emulator isn't
                // actually writing anything, achievements will never be recorded
                // for this game — surface that to the UI instead of leaving the
                // user staring at a silently-stuck count.
                if first_poll {
                    first_poll = false;
                    if let Some(info) = &emulator_info
                        && !goldberg::check_gbe_activity(info.dll_dir())
                    {
                        #[derive(Serialize, Clone)]
                        #[serde(rename_all = "camelCase")]
                        struct TrackingInactive {
                            game_id: String,
                        }
                        let lock = crate::utils::DROP_APP_HANDLE.lock().await;
                        if let Some(handle) = &*lock {
                            use tauri::Emitter;
                            let _ = handle.emit(
                                "achievement_tracking_inactive",
                                TrackingInactive { game_id: game_id.clone() },
                            );
                        }
                    }
                }

                // Check local emulator files (fast, no network), report what's
                // new, and toast what the server says it recorded.
                let new_reports = check_and_report_local(
                    &game_id,
                    app_ids,
                    &known_unlocked_external_ids,
                    emulator_info.as_ref(),
                    wine_prefix.as_deref(),
                )
                .await;
                report_and_announce(
                    &game_id,
                    new_reports,
                    &mut known_unlocked,
                    &mut known_unlocked_external_ids,
                    &on_new_achievement,
                )
                .await;
            }

            AchievementMode::RetroAchievements => {
                first_poll = false;

                debug!("{TAG} Polling RA for game {game_id} (known unlocked: {})", known_unlocked.len());
                // Poll the server which checks RA API for new unlocks
                let new_unlocks = poll_ra(&game_id).await;

                for unlock in &new_unlocks {
                    // Dedup on the RA achievement id (`external_id`), which is a
                    // stable identifier present in both the config and the RA
                    // poll — unlike `id`, whose id-space isn't guaranteed to
                    // match between the two endpoints.
                    if !known_unlocked_external_ids.contains(&unlock.external_id) {
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

            AchievementMode::None => {
                // Should be unreachable (None returns before the loop), but a
                // `unreachable!()` here would panic the whole poller task on any
                // future regression. Degrade gracefully instead.
                warn!("{TAG} poll loop entered None mode for {game_id}; stopping poller");
                return;
            }
        }
    }
}
