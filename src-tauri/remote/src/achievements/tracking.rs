//! Local capture never awaits the network. One independent worker drains durable observations.
use super::journal::{Journal, Observation, Record, Scope};
use super::{AchievementConfigResponse, AchievementItem};
use crate::goldberg::{
    self, EmulatorInfo,
    tracking::{Sample, SourceWatch},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tauri::Emitter;
use tokio::sync::Notify;

static WAKE: Notify = Notify::const_new();
static SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static STARTED: OnceLock<()> = OnceLock::new();
static STATUS: Mutex<Vec<TrackingStatus>> = Mutex::new(Vec::new());
static CONFIG_LAST: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingStatus {
    pub game_id: String,
    pub active: bool,
    pub sources: Vec<String>,
    pub last_read_at: Option<String>,
    pub last_sync_at: Option<String>,
    pub error: Option<String>,
    pub sync_error: Option<String>,
    pub popup_error: Option<String>,
    #[serde(skip)]
    scope: Scope,
}

fn status_update(scope: &Scope, game_id: &str, update: impl FnOnce(&mut TrackingStatus)) {
    if let Ok(mut statuses) = STATUS.lock() {
        let i = if let Some(i) = statuses
            .iter()
            .position(|s| s.game_id == game_id && s.scope == *scope)
        {
            i
        } else {
            statuses.push(TrackingStatus {
                game_id: game_id.into(),
                active: false,
                sources: vec![],
                last_read_at: None,
                last_sync_at: None,
                error: None,
                sync_error: None,
                popup_error: None,
                scope: scope.clone(),
            });
            statuses.len() - 1
        };
        update(&mut statuses[i]);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingView {
    pub status: Option<TrackingStatus>,
    pub records: Vec<Record>,
    pub pending: usize,
    pub reset_epoch: Option<String>,
    pub remote_states: Vec<super::TrackingState>,
    pub popup_supported: bool,
    pub popup_enabled: bool,
    pub journal_errors: Vec<String>,
}
pub fn view(game_id: &str) -> Result<TrackingView, String> {
    let scope = Scope::current().ok_or("Sign in to view achievement tracking.")?;
    let journal = Journal::open(scope.clone());
    let config = journal
        .read_config::<AchievementConfigResponse>(game_id)
        .ok();
    let epoch = config.as_ref().map(|c| c.reset_epoch.clone());
    let mut records = journal.records(Some(game_id)).map_err(|e| e.to_string())?;
    records.retain(|r| {
        !matches!(r.disposition.as_str(), "before_reset" | "superseded")
            && epoch
                .as_ref()
                .is_none_or(|e| r.event.reset_epoch == *e || r.event.reset_epoch == "unresolved")
    });
    let pending = records
        .iter()
        .filter(|r| matches!(r.disposition.as_str(), "pending" | "awaiting_definitions"))
        .count();
    let status = STATUS
        .lock()
        .map_err(|_| "Achievement status is unavailable.")?
        .iter()
        .find(|s| s.game_id == game_id && s.scope == scope)
        .cloned();
    Ok(TrackingView {
        status,
        records,
        pending,
        reset_epoch: epoch,
        remote_states: config.map(|c| c.tracking_states).unwrap_or_default(),
        popup_supported: cfg!(windows),
        popup_enabled: database::borrow_db_checked()
            .settings
            .achievement_game_popups,
        journal_errors: journal.problems().map_err(|e| e.to_string())?,
    })
}
pub fn retry() {
    if let Some(last) = CONFIG_LAST.get() {
        if let Ok(mut last) = last.lock() {
            last.clear();
        }
    }
    WAKE.notify_one();
}
pub(super) fn notification_error(scope: &Scope, game_id: &str, error: String) {
    status_update(scope, game_id, |s| s.popup_error = Some(error));
}

pub fn mark_reset(game_id: &str) -> Result<(), String> {
    let scope = Scope::current().ok_or("Sign in to reset achievement tracking.")?;
    let journal = Journal::open(scope);
    for record in journal.records(Some(game_id)).map_err(|e| e.to_string())? {
        journal
            .acknowledge(&record.event.event_id, "before_reset", None, None)
            .map_err(|e| e.to_string())?;
    }
    journal
        .invalidate_config(game_id)
        .map_err(|e| e.to_string())?;
    retry();
    Ok(())
}

pub struct PreparedTracking {
    game_id: String,
    journal: Journal,
    app_ids: Vec<String>,
    baseline: HashSet<String>,
    native_account: Option<String>,
    watch: SourceWatch,
}
impl Drop for PreparedTracking {
    fn drop(&mut self) {
        status_update(&self.journal.scope, &self.game_id, |s| s.active = false);
    }
}

fn app_ids(config: Option<&AchievementConfigResponse>, info: Option<&EmulatorInfo>) -> Vec<String> {
    let mut ids: Vec<_> = config
        .into_iter()
        .flat_map(|c| c.external_links.iter())
        .filter(|l| l.provider == "Goldberg" || l.provider == "Steam")
        .map(|l| l.external_game_id.clone())
        .collect();
    if let Some(info) = info {
        if let goldberg::SteamEmulator::SmartSteamEmu { app_id, .. } = &info.emulator {
            ids.push(app_id.clone());
        }
        if let Some(id) = goldberg::read_local_steam_appid(info.dll_dir()) {
            ids.push(id);
        }
    }
    ids.retain(|id| id.parse::<u32>().is_ok());
    ids.sort();
    ids.dedup();
    ids
}

/// Called BEFORE spawning the game. Watchers and baseline are ready for the first runtime write.
pub fn prepare(game_id: &str, info: Option<&EmulatorInfo>) -> Option<PreparedTracking> {
    let scope = Scope::current()?;
    let journal = Journal::open(scope.clone());
    let config = journal
        .read_config::<AchievementConfigResponse>(game_id)
        .ok();
    let ids = app_ids(config.as_ref(), info);
    let prefix = goldberg::wine_prefix_for_game(game_id);
    let snapshot = goldberg::tracking::snapshot(&ids, info, prefix.as_deref());
    let baseline = snapshot
        .samples
        .iter()
        .filter(|s| s.earned)
        .map(|s| s.key())
        .collect();
    let native_account = snapshot
        .samples
        .iter()
        .find(|s| s.source == "steam-native")
        .map(|s| s.account.clone());
    let watch = SourceWatch::new(&snapshot.paths);
    status_update(&scope, game_id, |s| {
        s.active = true;
        s.error = snapshot.errors.first().cloned();
    });
    WAKE.notify_one();
    Some(PreparedTracking {
        game_id: game_id.into(),
        journal,
        app_ids: ids,
        baseline,
        native_account,
        watch,
    })
}

fn occurred_at(seconds: u64) -> Option<String> {
    if !(946_684_800..=4_102_444_800).contains(&seconds)
        || seconds > chrono::Utc::now().timestamp().max(0) as u64
    {
        return None;
    }
    chrono::DateTime::from_timestamp(seconds as i64, 0).map(|d| d.to_rfc3339())
}

fn record_sample(
    journal: &Journal,
    game_id: &str,
    sample: &Sample,
    config: Option<&AchievementConfigResponse>,
    historical: bool,
    kind: &str,
) -> Result<Option<Record>, String> {
    let definition = config.and_then(|c| {
        c.achievements
            .iter()
            .find(|a| a.provider != "RetroAchievements" && a.external_id == sample.name)
    });
    let mut event = Observation {
        event_id: String::new(),
        external_id: sample.name.clone(),
        provider: "Goldberg".into(),
        provider_game_id: sample.app_id.clone(),
        provider_account: sample.account.clone(),
        source: sample.source.clone(),
        mode: "standard".into(),
        kind: kind.into(),
        reset_epoch: config
            .map(|c| c.reset_epoch.clone())
            .unwrap_or_else(|| "unresolved".into()),
        observed_at: chrono::Utc::now().to_rfc3339(),
        occurred_at: occurred_at(sample.earned_time),
        historical,
        current: sample.current,
        target: sample.target,
    };
    event.identify(&journal.scope, game_id);
    let record = Record {
        version: 1,
        scope: journal.scope.clone(),
        game_id: game_id.into(),
        event,
        title: definition
            .map(|a| a.title.clone())
            .unwrap_or_else(|| "Achievement unlocked".into()),
        description: definition
            .map(|a| a.description.clone())
            .unwrap_or_default(),
        icon_url: definition.map(|a| a.icon_url.clone()).unwrap_or_default(),
        achievement_id: definition.map(|a| a.id.clone()),
        disposition: "pending".into(),
        reason: None,
        announced: kind != "unlock",
    };
    if journal.capture(&record).map_err(|e| e.to_string())? {
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

pub async fn run(
    game_id: String,
    info: Option<EmulatorInfo>,
    prepared: Option<PreparedTracking>,
    cancel: Arc<Notify>,
    on_new: impl Fn(AchievementItem) + Send + 'static,
) {
    let Some(mut prepared) = prepared.or_else(|| prepare(&game_id, info.as_ref())) else {
        return;
    };
    let journal = &prepared.journal;
    let scope = journal.scope.clone();
    let prefix = goldberg::wine_prefix_for_game(&game_id);
    let mut known = HashSet::new();
    let mut progress = HashMap::new();
    let mut epoch = String::new();
    let mut first = true;
    let mut exit_deadline = None;
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        if !scope.is_current() {
            break;
        }
        let config = journal
            .read_config::<AchievementConfigResponse>(&game_id)
            .ok();
        let next_epoch = config
            .as_ref()
            .map(|c| c.reset_epoch.as_str())
            .unwrap_or("unresolved");
        if epoch != next_epoch {
            // A new reset baseline must not reimport an old timestamp-less save as a fresh unlock.
            if !first && !epoch.is_empty() && epoch != "unresolved" {
                prepared.baseline.extend(known.iter().cloned());
            }
            epoch = next_epoch.into();
            known.clear();
            progress.clear();
        }
        let ids = app_ids(config.as_ref(), info.as_ref());
        let mut late_ids = HashSet::new();
        for id in ids {
            if !prepared.app_ids.contains(&id) {
                late_ids.insert(id.clone());
                prepared.app_ids.push(id);
            }
        }
        let snapshot =
            goldberg::tracking::snapshot(&prepared.app_ids, info.as_ref(), prefix.as_deref());
        prepared.baseline.extend(
            snapshot
                .samples
                .iter()
                .filter(|s| s.earned && late_ids.contains(&s.app_id))
                .map(|s| s.key()),
        );
        prepared.watch.rearm(&snapshot.paths);
        let mut sources: Vec<_> = snapshot.samples.iter().map(|s| s.source.clone()).collect();
        sources.sort();
        sources.dedup();
        if config.as_ref().is_some_and(|c| {
            c.external_links
                .iter()
                .any(|l| l.provider == "RetroAchievements")
        }) {
            sources.push("retroachievements-api".into());
        }
        let mut error = snapshot
            .errors
            .first()
            .cloned()
            .or_else(|| prepared.watch.error.clone());
        for sample in &snapshot.samples {
            if sample.source == "steam-native" {
                if prepared
                    .native_account
                    .as_ref()
                    .is_some_and(|account| *account != sample.account)
                {
                    error = Some(
                        "Steam account changed. Restart this game to select the new account."
                            .into(),
                    );
                    continue;
                }
                if prepared.native_account.is_none() {
                    prepared.native_account = Some(sample.account.clone());
                    prepared.baseline.extend(
                        snapshot
                            .samples
                            .iter()
                            .filter(|s| s.source == "steam-native" && s.earned)
                            .map(|s| s.key()),
                    );
                }
            }
            let key = sample.key();
            let historical = prepared.baseline.contains(&key)
                || (first && !prepared.baseline.contains(&key) && info.is_none());
            let server_known = config.as_ref().is_some_and(|c| {
                c.achievements.iter().any(|a| {
                    a.provider != "RetroAchievements" && a.external_id == sample.name && a.unlocked
                })
            });
            if sample.earned && !known.contains(&key) {
                match record_sample(
                    journal,
                    &game_id,
                    sample,
                    config.as_ref(),
                    historical || server_known,
                    "unlock",
                ) {
                    Ok(Some(record)) => {
                        if journal
                            .announce_once(&record.event.event_id)
                            .unwrap_or(false)
                        {
                            super::notification::enqueue(&record);
                            on_new(AchievementItem {
                                id: record
                                    .achievement_id
                                    .clone()
                                    .unwrap_or_else(|| record.event.event_id.clone()),
                                external_id: record.event.external_id.clone(),
                                provider: "Goldberg".into(),
                                title: record.title,
                                description: record.description,
                                icon_url: record.icon_url,
                                unlocked: true,
                            });
                        }
                        WAKE.notify_one();
                        known.insert(key.clone());
                    }
                    Ok(None) => {
                        known.insert(key.clone());
                    }
                    Err(e) => error = Some(e),
                }
            }
            if !sample.earned {
                known.remove(&key);
            }
            if goldberg::tracking::progress_changed(&progress, sample) {
                match record_sample(journal, &game_id, sample, config.as_ref(), true, "progress") {
                    Ok(_) => {
                        progress.insert(key, (sample.current, sample.target));
                        WAKE.notify_one();
                    }
                    Err(e) => error = Some(e),
                }
            }
        }
        status_update(&scope, &game_id, |s| {
            s.last_read_at = Some(chrono::Utc::now().to_rfc3339());
            s.sources = sources;
            s.error = error;
        });
        first = false;
        if exit_deadline.is_some_and(|at| Instant::now() >= at) {
            break;
        }
        tokio::select! {
            _ = cancel.notified(), if exit_deadline.is_none() => { exit_deadline = Some(Instant::now()+Duration::from_secs(3)); },
            _ = ticker.tick() => {},
            _ = prepared.watch.signal.notified() => { tokio::time::sleep(Duration::from_millis(60)).await; },
        }
    }
    status_update(&scope, &game_id, |s| s.active = false);
    WAKE.notify_one();
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ack {
    event_id: String,
    status: String,
    reason: Option<String>,
    achievement_id: Option<String>,
}
#[derive(Deserialize)]
struct AckBatch {
    acknowledgements: Vec<Ack>,
}

async fn sync_game(journal: &Journal, game_id: &str) -> Result<bool, String> {
    if !journal.scope.is_current() {
        return Err("Achievement account changed.".into());
    }
    let cache_key = format!(
        "{}:{}:{}",
        journal.scope.server, journal.scope.client_id, game_id
    );
    let fresh = CONFIG_LAST
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map(|last| {
            last.get(&cache_key)
                .is_some_and(|at| at.elapsed() < Duration::from_secs(30))
        })
        .unwrap_or(false);
    let cached = if fresh {
        journal
            .read_config::<AchievementConfigResponse>(game_id)
            .ok()
    } else {
        None
    };
    let fetched = cached.is_none();
    let config = match cached {
        Some(config) => config,
        None => {
            let config = super::fetch_config_scoped(&journal.scope, game_id)
                .await
                .map_err(|e| e.to_string())?;
            journal
                .cache_config(game_id, &config)
                .map_err(|e| e.to_string())?;
            if let Ok(mut last) = CONFIG_LAST
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
            {
                last.insert(cache_key, Instant::now());
            }
            config
        }
    };
    let mut pending: Vec<_> = journal
        .records(Some(game_id))
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|r| matches!(r.disposition.as_str(), "pending" | "awaiting_definitions"))
        .collect();
    for record in &mut pending {
        if record.event.reset_epoch == "unresolved" {
            let mut rebound = record.clone();
            rebound.event.reset_epoch = config.reset_epoch.clone();
            rebound.event.identify(&journal.scope, game_id);
            journal.capture(&rebound).map_err(|e| e.to_string())?;
            journal
                .acknowledge(&record.event.event_id, "superseded", None, None)
                .map_err(|e| e.to_string())?;
            *record = rebound;
        }
    }
    for chunk in pending.chunks(100) {
        if !journal.scope.is_current() {
            return Err("Achievement account changed.".into());
        }
        if config.tracking_version >= 2 {
            let url = journal
                .scope
                .url(&format!("/api/v1/client/game/{game_id}/achievement-events"))
                .map_err(|e| e.to_string())?;
            #[derive(Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Batch<'a> {
                protocol_version: u8,
                events: Vec<&'a Observation>,
            }
            let body = Batch {
                protocol_version: 2,
                events: chunk.iter().map(|r| &r.event).collect(),
            };
            let request = crate::requests::RemoteRequest::post(url, &body)
                .for_client(&journal.scope.client_id)
                .map_err(|e| e.to_string())?;
            let response: AckBatch = crate::requests::remote_request(request)
                .await
                .map_err(|e| e.to_string())?;
            for ack in response.acknowledgements {
                if chunk.iter().any(|r| r.event.event_id == ack.event_id)
                    && matches!(
                        ack.status.as_str(),
                        "accepted"
                            | "duplicate"
                            | "awaiting_definitions"
                            | "before_reset"
                            | "invalid"
                    )
                {
                    journal
                        .acknowledge(&ack.event_id, &ack.status, ack.reason, ack.achievement_id)
                        .map_err(|e| e.to_string())?;
                }
            }
        } else {
            // Legacy servers cannot acknowledge individual missing IDs. Keep ambiguous reports queued.
            let reports: Vec<_> = chunk
                .iter()
                .filter(|r| r.event.kind == "unlock")
                .map(|r| super::AchievementReportEntry {
                    external_id: r.event.external_id.clone(),
                    provider: r.event.provider.clone(),
                    unlocked_at: r
                        .event
                        .occurred_at
                        .clone()
                        .unwrap_or_else(|| r.event.observed_at.clone()),
                })
                .collect();
            if reports.is_empty() {
                continue;
            }
            let response = super::report_scoped(&journal.scope, game_id, reports)
                .await
                .map_err(|e| e.to_string())?;
            if response.skipped == 0 && response.ignored_before_reset == 0 {
                for record in chunk.iter().filter(|r| r.event.kind == "unlock") {
                    journal
                        .acknowledge(&record.event.event_id, "accepted", None, None)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(fetched || !pending.is_empty())
}

pub fn start_worker() {
    if STARTED.set(()).is_err() {
        return;
    }
    tauri::async_runtime::spawn(async {
        let mut failures = 0u32;
        loop {
            let started = Instant::now();
            let result = drain().await;
            failures = if result { 0 } else { (failures + 1).min(5) };
            let delay = 5u64.saturating_mul(1u64 << failures).min(120);
            tokio::select! { _ = tokio::time::sleep(Duration::from_secs(delay)) => {}, _ = WAKE.notified() => {} }
            // File events and Retry cannot defeat the provider's rate/backoff budget.
            let minimum = Duration::from_secs(if failures == 0 { 5 } else { delay });
            if let Some(remaining) = minimum.checked_sub(started.elapsed()) {
                tokio::time::sleep(remaining).await;
            }
        }
    });
}

async fn drain() -> bool {
    let Ok(_guard) = SYNC_LOCK.try_lock() else {
        return true;
    };
    let Some(scope) = Scope::current() else {
        return true;
    };
    let journal = Journal::open(scope.clone());
    let records = match journal.records(None) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[ACH] Journal read failed: {e}");
            return false;
        }
    };
    let mut games: HashSet<_> = records
        .iter()
        .filter(|r| matches!(r.disposition.as_str(), "pending" | "awaiting_definitions"))
        .map(|r| r.game_id.clone())
        .collect();
    let active: Vec<_> = STATUS
        .lock()
        .map(|s| {
            s.iter()
                .filter(|s| s.active && s.scope == scope)
                .map(|s| s.game_id.clone())
                .collect()
        })
        .unwrap_or_default();
    games.extend(active.iter().cloned());
    let mut ok = true;
    for game_id in games {
        if !scope.is_current() {
            break;
        }
        let result = sync_game(&journal, &game_id).await;
        status_update(&scope, &game_id, |s| match &result {
            Ok(true) => {
                s.last_sync_at = Some(chrono::Utc::now().to_rfc3339());
                s.sync_error = None;
            }
            Ok(false) => {}
            Err(e) => {
                s.sync_error = Some(e.clone());
            }
        });
        if result.is_err() {
            ok = false;
        }
        if active.contains(&game_id) {
            if let Ok(config) = journal.read_config::<AchievementConfigResponse>(&game_id) {
                static RA_LAST: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
                let ra_key = format!("{}:{}:{}", scope.server, scope.user_id, game_id);
                let should_poll = RA_LAST
                    .get_or_init(|| Mutex::new(HashMap::new()))
                    .lock()
                    .map(|mut last| {
                        if last
                            .get(&ra_key)
                            .is_some_and(|t| t.elapsed() < Duration::from_secs(15))
                        {
                            false
                        } else {
                            last.insert(ra_key, Instant::now());
                            true
                        }
                    })
                    .unwrap_or(false);
                if should_poll
                    && config
                        .external_links
                        .iter()
                        .any(|l| l.provider == "RetroAchievements")
                {
                    match super::poll_ra(&scope, &game_id).await {
                        Ok(unlocks) => {
                            for unlock in unlocks {
                                emit_remote_unlock(&game_id, &unlock).await;
                            }
                        }
                        Err(e) => status_update(&scope, &game_id, |s| s.sync_error = Some(e)),
                    }
                }
            }
        }
    }
    ok
}

async fn emit_remote_unlock(game_id: &str, unlock: &super::RAPollUnlock) {
    let handle = crate::utils::DROP_APP_HANDLE.lock().await;
    if let Some(handle) = &*handle {
        let _ = handle.emit("achievement_unlocked", serde_json::json!({ "id":unlock.id,"title":unlock.title,
            "description":unlock.description,"iconUrl":unlock.icon_url,"gameId":game_id,"externalId":unlock.external_id }));
    }
}
