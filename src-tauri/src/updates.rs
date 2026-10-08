//! Update detection (every 30 minutes, or on demand) and the Tauri commands
//! for in-place game updates. The update engine itself lives in
//! `games::downloads::update`.

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;
use std::path::PathBuf;
use std::sync::nonpoison::Mutex;

use async_trait::async_trait;
use client::{app_state::AppState, app_status::AppStatus};
use database::{
    DownloadType, GameDownloadStatus, borrow_db_checked, borrow_db_mut_checked,
    models::data::InstalledGameType, platform::Platform,
};
use games::downloads::update::{
    self as engine, RecoverOutcome, UpdateError, UpdatePlan, baseline::read_sidecar_header,
    plan::Resolution,
};
use log::{info, warn};
use remote::utils::DROP_APP_HANDLE;
use tauri::{AppHandle, Manager};

use crate::{
    games::{VersionDownloadOption, fetch_game_version_options_logic},
    scheduler::ScheduleTask,
};

pub struct GameUpdater {
    no_internet: bool,
}

impl GameUpdater {
    pub fn new() -> Self {
        GameUpdater { no_internet: false }
    }
}

#[async_trait]
impl ScheduleTask for GameUpdater {
    fn timeframe(&mut self) -> usize {
        if self.no_internet { 5 } else { 30 }
    }

    async fn call(&mut self) -> Result<(), anyhow::Error> {
        let app_handle = {
            let guard = DROP_APP_HANDLE.lock().await;
            guard
                .as_ref()
                .ok_or(anyhow::anyhow!("game update task ran before setup"))?
                .clone()
        };
        let state = app_handle.state::<Mutex<AppState>>();
        if state.lock().status == AppStatus::Offline {
            self.no_internet = true;
            return Ok(());
        }
        self.no_internet = false;
        run_update_check(&app_handle, state, None)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}

/// Whether an install has an update, from the server's versions for its
/// platform (latest first, as the versions route orders them by
/// `versionIndex`) with each one's current revision.
///
/// - A version listed before the installed one is newer. It counts only when
///   updates are enabled for the install. If the installed version is no
///   longer listed at all, the latest one is the way forward.
/// - The installed version's current revision being above the one installed
///   (a republish of the same version) always counts. `installed_revision`
///   is `None` when that can't be known or applied (a delta version: those
///   are never updated in place), and then only newer versions count.
///
/// Delta versions are never updated in place (the engine refuses them), so
/// nothing is offered when the installed version is one, and a newer version
/// is not offered when the latest is one. Installing a delta version side by
/// side, from the version list, is unaffected.
pub fn update_available(
    versions_latest_first: &[(&str, Option<u32>)],
    installed_version: &str,
    enable_updates: bool,
    installed_revision: Option<u32>,
    installed_is_delta: bool,
    latest_is_delta: bool,
) -> bool {
    if installed_is_delta {
        return false;
    }
    let position = versions_latest_first
        .iter()
        .position(|(v, _)| *v == installed_version);
    let newer = enable_updates
        && !latest_is_delta
        && match position {
            Some(p) => p > 0,
            None => !versions_latest_first.is_empty(),
        };
    let republished = match (position.and_then(|p| versions_latest_first[p].1), installed_revision) {
        (Some(current), Some(installed)) => current > installed,
        _ => false,
    };
    newer || republished
}

/// The version an update of this install goes to when none is named: the
/// latest, when updates are enabled for the install and it is newer (or the
/// installed version is gone from the server); otherwise the installed
/// version itself, so a republish of a pinned version never moves the player
/// to a different version, and neither does one to a delta version.
pub fn default_target<'a>(
    versions_latest_first: &[(&'a str, Option<u32>)],
    installed_version: &str,
    enable_updates: bool,
    latest_is_delta: bool,
) -> Option<&'a str> {
    match versions_latest_first
        .iter()
        .position(|(v, _)| *v == installed_version)
    {
        // A delta latest version can't be applied in place: stay on (a
        // republish of) the installed version instead.
        Some(p) if p > 0 && enable_updates && !latest_is_delta => Some(versions_latest_first[0].0),
        Some(p) => Some(versions_latest_first[p].0),
        None => versions_latest_first.first().map(|v| v.0),
    }
}

/// Whether a version is a delta version, from the local copy of its version
/// info or the server's (remembered: a version's kind never changes). `None`
/// when it can't be found out.
async fn version_is_delta(game_id: &str, version_id: &str) -> Option<bool> {
    static KNOWN: LazyLock<std::sync::Mutex<HashMap<String, bool>>> =
        LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
    if let Some(d) = borrow_db_checked()
        .applications
        .game_versions
        .get(version_id)
        .map(|v| v.delta)
    {
        return Some(d);
    }
    if let Some(d) = KNOWN.lock().ok().and_then(|k| k.get(version_id).copied()) {
        return Some(d);
    }
    match engine::fetch_game_version(game_id, version_id).await {
        Ok(v) => {
            if let Ok(mut k) = KNOWN.lock() {
                k.insert(version_id.to_string(), v.delta);
            }
            Some(v.delta)
        }
        Err(e) => {
            warn!("could not tell whether {game_id} {version_id} is a delta version: {e}");
            None
        }
    }
}

struct Candidate {
    version_id: String,
    platform: Platform,
    install_dir: PathBuf,
    enable_updates: bool,
    delta: bool,
}

/// The revision an install has: its baseline's, or 1 for an install from
/// before revisions existed (revision 1 is what the server's import, or an
/// admin's "Record fingerprints", records first).
fn installed_revision(game_id: &str, c: &Candidate) -> u32 {
    match read_sidecar_header(&c.install_dir) {
        Ok(Some(h)) if h.game_id == game_id && h.version_id == c.version_id => h.revision,
        Ok(_) => 1,
        Err(e) => {
            warn!("{game_id}: unreadable update baseline in {} ({e})", c.install_dir.display());
            1
        }
    }
}

/// Check installs for updates and record the result on each install and on
/// the game's status. Emits `update_game/<gameId>` for every game whose flag
/// changed. `only` limits the check to one game. Errors name the games whose
/// versions could not be fetched; the others are still checked.
pub async fn run_update_check(
    app: &AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    only: Option<&str>,
) -> Result<(), String> {
    let groups: BTreeMap<String, Vec<Candidate>> = {
        let db = borrow_db_checked();
        let mut groups: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
        for rec in db.applications.installs.values() {
            if only.is_some_and(|g| g != rec.game_id) {
                continue;
            }
            if matches!(rec.install_type, InstalledGameType::PartiallyInstalled { .. }) {
                continue;
            }
            let is_mod = db
                .applications
                .installed_game_version
                .get(&rec.game_id)
                .is_some_and(|m| m.download_type == DownloadType::Mod);
            if is_mod {
                continue;
            }
            let version = db.applications.game_versions.get(&rec.version_id);
            let enable_updates = version.is_some_and(|v| v.user_configuration.enable_updates);
            let delta = version.is_some_and(|v| v.delta);
            groups.entry(rec.game_id.clone()).or_default().push(Candidate {
                version_id: rec.version_id.clone(),
                platform: rec.target_platform,
                install_dir: PathBuf::from(&rec.install_dir),
                enable_updates,
                delta,
            });
        }
        groups
    };

    let mut failed: Vec<String> = Vec::new();
    for (game_id, installs) in groups {
        let options: Vec<VersionDownloadOption> =
            match fetch_game_version_options_logic(game_id.clone(), state.clone()).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("could not check {game_id} for updates: {e}");
                    failed.push(game_id);
                    continue;
                }
            };
        let mut decisions: Vec<(String, bool)> = Vec::with_capacity(installs.len());
        for c in &installs {
            let versions: Vec<(&str, Option<u32>)> = options
                .iter()
                .filter(|o| o.platform == c.platform)
                .map(|o| (o.version_id.as_str(), o.revision))
                .collect();
            // An in-place update to or from a delta version is refused, so
            // it must never be offered. Only asked about when it matters.
            let latest_is_delta = match versions.first() {
                Some((latest, _)) if c.enable_updates && *latest != c.version_id && !c.delta => {
                    // Unknown: don't offer what might be refused.
                    version_is_delta(&game_id, latest).await.unwrap_or(true)
                }
                _ => false,
            };
            let flag = update_available(
                &versions,
                &c.version_id,
                c.enable_updates,
                // A delta install has no baseline; its revision means nothing.
                (!c.delta).then(|| installed_revision(&game_id, c)),
                c.delta,
                latest_is_delta,
            );
            decisions.push((c.version_id.clone(), flag));
        }

        let needs_write = {
            let db = borrow_db_checked();
            decisions.iter().any(|(v, flag)| {
                let rec_differs = db
                    .applications
                    .get_install(&game_id, v)
                    .is_some_and(|r| r.update_available != *flag);
                let status_differs = matches!(
                    db.applications.game_statuses.get(&game_id),
                    Some(GameDownloadStatus::Installed { version_id, update_available, .. })
                        if version_id == v && update_available != flag
                );
                rec_differs || status_differs
            })
        };
        if !needs_write {
            continue;
        }
        {
            let mut db = borrow_db_mut_checked();
            for (v, flag) in &decisions {
                if let Some(rec) = db.applications.get_install_mut(&game_id, v) {
                    rec.update_available = *flag;
                }
                if let Some(GameDownloadStatus::Installed {
                    version_id,
                    update_available,
                    ..
                }) = db.applications.game_statuses.get_mut(&game_id)
                    && version_id == v
                {
                    *update_available = *flag;
                }
            }
        }
        info!("{game_id}: update flags now {decisions:?}");
        engine::push_state(app, &game_id);
    }

    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Could not check {} game(s) for updates. Check your connection and try again.",
            failed.len()
        ))
    }
}

/// Check now, for one game or all.
#[tauri::command]
pub async fn check_for_updates(
    game_id: Option<String>,
    app_handle: AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    if state.lock().status == AppStatus::Offline {
        return Err("Drop is offline. Connect to your server to check for updates.".to_string());
    }
    run_update_check(&app_handle, state, game_id.as_deref()).await
}

/// The version to update this install to when the caller names none (see
/// [`default_target`]).
async fn default_target_for_install(
    game_id: &str,
    install_version_id: &str,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<String, UpdateError> {
    let (platform, enable_updates) = {
        let db = borrow_db_checked();
        let platform = db
            .applications
            .get_install(game_id, install_version_id)
            .map(|r| r.target_platform)
            .ok_or(UpdateError::NotInstalled)?;
        let enable_updates = db
            .applications
            .game_versions
            .get(install_version_id)
            .is_some_and(|v| v.user_configuration.enable_updates);
        (platform, enable_updates)
    };
    let options = fetch_game_version_options_logic(game_id.to_string(), state).await?;
    let versions: Vec<(&str, Option<u32>)> = options
        .iter()
        .filter(|o| o.platform == platform)
        .map(|o| (o.version_id.as_str(), o.revision))
        .collect();
    let latest_is_delta = match versions.first() {
        Some((latest, _)) if *latest != install_version_id => {
            version_is_delta(game_id, latest).await.unwrap_or(true)
        }
        _ => false,
    };
    default_target(&versions, install_version_id, enable_updates, latest_is_delta)
        .map(str::to_string)
        .ok_or(UpdateError::NoVersionForPlatform)
}

/// Work out what updating this install would change. Nothing is modified.
#[tauri::command]
pub async fn plan_game_update(
    game_id: String,
    install_version_id: String,
    target_version_id: Option<String>,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<UpdatePlan, UpdateError> {
    let target = match target_version_id {
        Some(v) => v,
        None => default_target_for_install(&game_id, &install_version_id, state).await?,
    };
    let prepared = engine::prepare(&game_id, &install_version_id, &target).await?;
    let plan = prepared.summary();
    info!(
        "update plan for {game_id} {install_version_id} -> {} rev {}: +{} ~{} -{} ({} conflict(s), {} bytes, baseline {:?})",
        plan.to_version_id,
        plan.to_revision,
        plan.add_count,
        plan.update_count,
        plan.remove_count,
        plan.conflicts.len(),
        plan.download_bytes,
        plan.baseline_source
    );
    Ok(plan)
}

/// Queue the update. Plans again first: a conflict without a decision, or a
/// revision different from the reviewed one, is an error and nothing is
/// queued. Progress uses the download queue's events.
#[tauri::command]
pub async fn apply_game_update(
    game_id: String,
    install_version_id: String,
    to_version_id: String,
    to_revision: u32,
    mirror_folders: Vec<String>,
    resolutions: HashMap<String, Resolution>,
) -> Result<(), UpdateError> {
    engine::apply(
        &game_id,
        &install_version_id,
        &to_version_id,
        to_revision,
        &mirror_folders,
        resolutions,
    )
    .await
}

/// The way out of an update that was interrupted and could not be finished
/// or undone (launching says `ProcessError::UpdateInProgress`). Tries once
/// more; failing that, renames the update's folder aside, untouched (the
/// player's pre-update files stay in it), so the game can be launched.
/// Nothing is deleted.
#[tauri::command]
pub fn recover_game_update(
    game_id: String,
    install_version_id: String,
    app_handle: AppHandle,
) -> Result<RecoverOutcome, UpdateError> {
    engine::recover_update(&game_id, &install_version_id, Some(&app_handle))
}

#[cfg(test)]
mod tests {
    use super::{default_target, update_available};

    #[test]
    fn a_newer_version_counts_only_with_updates_enabled() {
        let v = [("v3", Some(1)), ("v2", Some(1)), ("v1", Some(1))];
        assert!(update_available(&v, "v2", true, Some(1), false, false));
        assert!(!update_available(&v, "v2", false, Some(1), false, false));
        assert!(!update_available(&v, "v3", true, Some(1), false, false));
    }

    #[test]
    fn an_older_version_is_never_an_update() {
        // The old code compared ids with `!=`, so being on the latest of
        // three and seeing an older id first would have flagged it.
        let v = [("v3", Some(1)), ("v2", Some(1))];
        assert!(!update_available(&v, "v3", true, Some(1), false, false));
    }

    #[test]
    fn a_republished_version_counts_even_with_updates_off() {
        let v = [("v1", Some(3))];
        assert!(update_available(&v, "v1", false, Some(2), false, false));
        assert!(!update_available(&v, "v1", false, Some(3), false, false));
    }

    #[test]
    fn an_old_server_without_revisions_never_flags_a_republish() {
        let v = [("v1", None)];
        assert!(!update_available(&v, "v1", true, Some(1), false, false));
    }

    #[test]
    fn a_version_removed_from_the_server_points_at_the_latest() {
        let v = [("v3", Some(1))];
        assert!(update_available(&v, "gone", true, Some(1), false, false));
        assert!(!update_available(&v, "gone", false, Some(1), false, false));
        assert!(!update_available(&[], "gone", true, Some(1), false, false));
    }

    #[test]
    fn a_delta_install_never_flags_a_republish() {
        let v = [("d1", Some(5))];
        assert!(!update_available(&v, "d1", true, None, true, false));
        // Nor a newer version: the engine refuses delta installs.
        let v = [("v2", Some(1)), ("d1", Some(5))];
        assert!(!update_available(&v, "d1", true, None, true, false));
    }

    #[test]
    fn a_pinned_version_is_updated_to_itself_not_to_a_newer_one() {
        let v = [("v3", Some(1)), ("v2", Some(4))];
        assert_eq!(default_target(&v, "v2", false, false), Some("v2"));
        assert_eq!(default_target(&v, "v2", true, false), Some("v3"));
        assert_eq!(default_target(&v, "v3", true, false), Some("v3"));
        assert_eq!(default_target(&v, "gone", false, false), Some("v3"));
        assert_eq!(default_target(&[], "gone", true, false), None);
        // A delta latest version: stay on the installed one.
        assert_eq!(default_target(&v, "v2", true, true), Some("v2"));
    }


    #[test]
    fn nothing_that_can_only_be_refused_is_offered_for_delta_versions() {
        // Installed version is a delta: no in-place update at all.
        let v = [("v2", Some(1)), ("d1", Some(3))];
        assert!(!update_available(&v, "d1", true, None, true, false));
        // Latest version is a delta: a newer version is not offered...
        let v = [("d2", Some(1)), ("v1", Some(2))];
        assert!(!update_available(&v, "v1", true, Some(2), false, true));
        // ...but a republish of the installed version still is.
        assert!(update_available(&v, "v1", true, Some(1), false, true));
    }

}
