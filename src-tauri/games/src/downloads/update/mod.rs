//! In-place game updates.
//!
//! An update changes an existing install folder into another (version,
//! revision) by applying only the file differences, instead of installing a
//! fresh copy next to it. The pieces:
//!
//! - [`plan`]: the pure three-way planner (baseline, target, disk);
//! - [`baseline`]: the install-local `.drop-baseline.json` and hashing;
//! - [`commit`]: the crash-safe swap of staged files into the install;
//! - [`agent`]: the download-queue agent that stages, verifies and commits;
//! - this module: the server API, planning for an install, moving the install
//!   record, and recovering an interrupted commit.

pub mod agent;
pub mod baseline;
pub mod commit;
pub mod plan;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock};

use database::{
    ApplicationTransientStatus, Database, DownloadType, DownloadableMetadata, GameDownloadStatus,
    GameVersion, borrow_db_checked, borrow_db_mut_checked,
    models::data::{InstallRecord, InstalledGameType},
    platform::Platform,
};
use download_manager::DOWNLOAD_MANAGER;
use download_manager::downloadable::Downloadable;
use download_manager::error::ApplicationDownloadError;
use log::{error, info, warn};
use remote::error::RemoteAccessError;
use remote::requests::{RemoteRequest, generate_url, remote_request};
use serde::{Deserialize, Serialize};
use serde_with::SerializeDisplay;
use tauri::AppHandle;

use crate::downloads::download_agent::{DownloadInformation, fetch_download_info, is_protected_user_data};
use crate::downloads::mod_data::mod_owned_files_spelled;
use crate::library::push_game_update;
use crate::state::GameStatusManager;
use crate::status::{StatusKind, transition_from_db};

use plan::{BaselineFile, Conflict, Plan, PlanInput, RemoteFile, Resolution};

/// Staging folder inside the install (hidden, same filesystem).
pub const UPDATE_DIR: &str = ".drop-update";
/// What the client last installed here (see `baseline`).
pub const BASELINE_FILE: &str = ".drop-baseline.json";

// ---------------------------------------------------------------------------
// Errors

/// Every `plan_game_update` / `apply_game_update` error that means "an
/// earlier update needs recovering first" starts with exactly this, so the
/// UI can recognise it, strip it and offer `recover_game_update`.
pub const NEEDS_RECOVERY_MARKER: &str = "[needs-recovery] ";

#[derive(Debug, SerializeDisplay)]
pub enum UpdateError {
    NotInstalled,
    PartiallyInstalled,
    IsMod,
    Busy,
    ModDownloadActive,
    GameRunning,
    TargetAlreadyInstalled,
    DeltaVersion,
    NoVersionForPlatform,
    NoFileList,
    ServerUnsupported,
    RevisionChanged { planned: u32, current: u32 },
    Inconsistent(String),
    Unresolved(Vec<String>),
    ChangedSinceReview(String),
    /// An earlier update of this install was interrupted and could be
    /// neither finished nor undone automatically. Its message starts with
    /// [`NEEDS_RECOVERY_MARKER`] so the UI can offer `recover_game_update`.
    NeedsRecovery(String),
    DiskFull { needed: u64, available: u64 },
    Plan(String),
    Remote(RemoteAccessError),
    Io(String),
    Queue(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Player-facing placeholder copy (see the U2 report for the list).
        match self {
            UpdateError::NotInstalled => write!(f, "This version is not installed."),
            UpdateError::PartiallyInstalled => {
                write!(f, "This install is not finished. Resume or repair it before updating.")
            }
            UpdateError::IsMod => write!(f, "Mods are updated from the game's Mods tab."),
            UpdateError::Busy => write!(f, "This game is already downloading or updating."),
            UpdateError::ModDownloadActive => {
                write!(f, "A mod is downloading. Wait for it to finish, then update.")
            }
            UpdateError::GameRunning => write!(
                f,
                "Close the game (or any game that runs through it) before updating it."
            ),
            UpdateError::TargetAlreadyInstalled => {
                write!(f, "That version is already installed in another folder.")
            }
            UpdateError::DeltaVersion => write!(
                f,
                "This version is a patch on top of another version and can't be applied in place."
            ),
            UpdateError::NoVersionForPlatform => {
                write!(f, "The server has no version of this game for this platform.")
            }
            UpdateError::NoFileList => write!(
                f,
                "The server has no file list for this version yet. Try again later."
            ),
            UpdateError::ServerUnsupported => {
                write!(f, "The Drop server needs updating before games can be updated in place.")
            }
            UpdateError::RevisionChanged { .. } => write!(
                f,
                "The update changed on the server while you were reviewing it. Review it again."
            ),
            UpdateError::Inconsistent(why) => {
                write!(f, "The server sent inconsistent update data ({why}). Try again later.")
            }
            UpdateError::Unresolved(paths) => write!(
                f,
                "Choose what to do with every changed file first ({} left).",
                paths.len()
            ),
            UpdateError::ChangedSinceReview(path) => write!(
                f,
                "{path} changed since the update was reviewed. Review the update again."
            ),
            UpdateError::NeedsRecovery(why) => write!(
                f,
                "{NEEDS_RECOVERY_MARKER}The last update of this game did not finish and has to be \
                 repaired before it can be updated again ({why})."
            ),
            UpdateError::DiskFull { .. } => write!(f, "Not enough free space for this update."),
            UpdateError::Plan(why) => write!(f, "Could not work out the update: {why}"),
            UpdateError::Remote(e) => write!(f, "{e}"),
            UpdateError::Io(why) => write!(f, "{why}"),
            UpdateError::Queue(why) => write!(f, "Could not queue the update: {why}"),
        }
    }
}

impl From<RemoteAccessError> for UpdateError {
    fn from(e: RemoteAccessError) -> Self {
        UpdateError::Remote(e)
    }
}

impl From<ApplicationDownloadError> for UpdateError {
    fn from(e: ApplicationDownloadError) -> Self {
        match e {
            ApplicationDownloadError::Communication(r) => UpdateError::Remote(r),
            other => UpdateError::Io(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// One update per game at a time

static ACTIVE: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Whether an update of this game is queued, running or being recovered.
/// Mod downloads into the game's folder are refused meanwhile.
pub fn update_active(game_id: &str) -> bool {
    ACTIVE.lock().map(|s| s.contains(game_id)).unwrap_or(true)
}

fn try_claim(game_id: &str) -> bool {
    ACTIVE
        .lock()
        .map(|mut s| s.insert(game_id.to_string()))
        .unwrap_or(false)
}

fn release(game_id: &str) {
    if let Ok(mut s) = ACTIVE.lock() {
        s.remove(game_id);
    }
}

// ---------------------------------------------------------------------------
// Launch gate: the commit must not race a launch

/// Runs the closure with launches blocked, passing whether any of the given
/// games is running (or part-way through launching). Set by the app, which
/// owns the process manager; this crate can't depend on it.
pub type LaunchGate = Box<dyn Fn(&[String], &mut dyn FnMut(bool)) + Send + Sync>;

static LAUNCH_GATE: OnceLock<LaunchGate> = OnceLock::new();

pub fn set_launch_gate(gate: LaunchGate) {
    if LAUNCH_GATE.set(gate).is_err() {
        warn!("update launch gate was already set");
    }
}

/// The game itself, plus every game whose installed version launches
/// through it. An emulator is a game too: updating it while a ROM that runs
/// on it is playing would swap the emulator's files under the running
/// process, and the process is tracked under the ROM's id, not the
/// emulator's.
pub fn games_using(db: &Database, game_id: &str) -> Vec<String> {
    let mut ids: Vec<String> = vec![game_id.to_string()];
    for v in db.applications.game_versions.values() {
        let through_it = v
            .launches
            .iter()
            .any(|l| l.emulator.as_ref().is_some_and(|e| e.game_id == game_id));
        if through_it && !ids.contains(&v.game_id) {
            ids.push(v.game_id.clone());
        }
    }
    ids
}

fn with_launches_blocked<R>(game_id: &str, f: impl FnOnce(bool) -> R) -> R {
    let (ids, running_in_db) = {
        let db = borrow_db_checked();
        let ids = games_using(&db, game_id);
        let running = db
            .applications
            .transient_statuses
            .iter()
            .any(|(m, s)| ids.contains(&m.id) && matches!(s, ApplicationTransientStatus::Running {}));
        (ids, running)
    };
    let mut f = Some(f);
    let mut out: Option<R> = None;
    if let Some(gate) = LAUNCH_GATE.get() {
        gate(&ids, &mut |running| {
            if let Some(f) = f.take() {
                out = Some(f(running || running_in_db));
            }
        });
    }
    if let Some(r) = out {
        return r;
    }
    // No gate (tests), or a gate that never called back: in the second case
    // treat the game as running so nothing is committed under an unknown state.
    let running = LAUNCH_GATE.get().is_some() || running_in_db;
    match f.take() {
        Some(f) => f(running),
        None => unreachable!("the closure ran but produced no result"),
    }
}

// ---------------------------------------------------------------------------
// Server API

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionFiles {
    #[serde(default)]
    pub version_id: String,
    pub revision: u32,
    pub files: Vec<RemoteFile>,
}

pub enum RevisionQuery {
    Current,
    Earliest,
    Exact(u32),
}

/// A file list can run to tens of thousands of entries.
const FILE_LIST_JSON_CAP: u64 = 256 * 1024 * 1024;

/// `GET /api/v1/client/game/version/files`. `Ok(None)` on 404: that version
/// or revision has no snapshot (no fingerprints recorded for it yet, or an
/// older server).
pub async fn fetch_revision_files(
    version_id: &str,
    query: RevisionQuery,
) -> Result<Option<RevisionFiles>, RemoteAccessError> {
    let revision = match query {
        RevisionQuery::Current => None,
        RevisionQuery::Earliest => Some("earliest".to_string()),
        RevisionQuery::Exact(n) => Some(n.to_string()),
    };
    let mut params: Vec<(&str, &str)> = vec![("version", version_id)];
    if let Some(r) = revision.as_deref() {
        params.push(("revision", r));
    }
    let url = generate_url(&["/api/v1/client/game/version/files"], &params)?;
    match remote_request::<RevisionFiles, _>(RemoteRequest::get(url).with_json_cap(FILE_LIST_JSON_CAP)).await {
        Ok(v) => Ok(Some(v)),
        Err(RemoteAccessError::ServerError { status: 404, .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn fetch_game_version(game_id: &str, version_id: &str) -> Result<GameVersion, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/game", game_id, "version", version_id], &[])?;
    remote_request(RemoteRequest::get(url)).await
}

// ---------------------------------------------------------------------------
// Planning

#[derive(Debug, Clone)]
pub struct InstallRef {
    pub game_id: String,
    pub version_id: String,
    pub platform: Platform,
    pub install_dir: PathBuf,
}

fn find_install(db: &Database, game_id: &str, version_id: &str) -> Result<InstallRef, UpdateError> {
    if db
        .applications
        .installed_game_version
        .get(game_id)
        .is_some_and(|m| m.download_type == DownloadType::Mod)
    {
        return Err(UpdateError::IsMod);
    }
    let rec = db
        .applications
        .get_install(game_id, version_id)
        .ok_or(UpdateError::NotInstalled)?;
    if matches!(rec.install_type, InstalledGameType::PartiallyInstalled { .. }) {
        return Err(UpdateError::PartiallyInstalled);
    }
    Ok(InstallRef {
        game_id: game_id.to_string(),
        version_id: version_id.to_string(),
        platform: rec.target_platform,
        install_dir: PathBuf::from(&rec.install_dir),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BaselineSource {
    Local,
    Server,
    None,
}

/// The review the UI shows (the `plan_game_update` result).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlan {
    pub game_id: String,
    pub from_version_id: String,
    pub from_revision: Option<u32>,
    pub to_version_id: String,
    pub to_revision: u32,
    pub add_count: usize,
    pub update_count: usize,
    pub remove_count: usize,
    pub download_bytes: u64,
    pub conflicts: Vec<Conflict>,
    /// Files replaced or removed with the old copy kept as `<file>.bak`
    /// without asking, because Drop can't tell whether the player changed
    /// them (see `Plan::backup_paths`). Never also in `conflicts`; counted in
    /// `updateCount` / `removeCount`.
    pub backup_paths: Vec<String>,
    pub baseline_source: BaselineSource,
}

pub struct Prepared {
    pub install: InstallRef,
    pub to_version_id: String,
    pub to_revision: u32,
    pub from_revision: Option<u32>,
    pub baseline_source: BaselineSource,
    pub plan: Plan,
    pub manifest: DownloadInformation,
    /// The target version, as the client will record it.
    pub game_version: GameVersion,
}

/// The baseline for an install: its sidecar when that matches the install,
/// else the server's earliest snapshot of the installed version, else empty.
/// Also the files the player chose "keep mine" for (only a local sidecar has
/// any).
async fn load_baseline(
    install: &InstallRef,
) -> Result<(Vec<BaselineFile>, BaselineSource, Option<u32>, HashSet<String>), UpdateError> {
    match baseline::read_sidecar(&install.install_dir) {
        Ok(Some(s)) if s.game_id == install.game_id && s.version_id == install.version_id => {
            let kept_mine = s.kept_mine.into_iter().collect();
            return Ok((s.files, BaselineSource::Local, Some(s.revision), kept_mine));
        }
        Ok(Some(s)) => warn!(
            "{}: ignoring baseline for {}/{} in {}; the install is {}",
            install.game_id,
            s.game_id,
            s.version_id,
            install.install_dir.display(),
            install.version_id
        ),
        Ok(None) => {}
        Err(e) => warn!(
            "{}: baseline in {} is unreadable ({e}); using the server's file list instead",
            install.game_id,
            install.install_dir.display()
        ),
    }
    match fetch_revision_files(&install.version_id, RevisionQuery::Earliest).await? {
        Some(r) => {
            let files = r.files.iter().map(|f| BaselineFile::from_remote(f, None)).collect();
            Ok((files, BaselineSource::Server, Some(r.revision), HashSet::new()))
        }
        None => {
            warn!(
                "{}: no baseline for version {}; every difference will be a conflict and nothing is removed",
                install.game_id, install.version_id
            );
            Ok((Vec::new(), BaselineSource::None, None, HashSet::new()))
        }
    }
}

/// The chunks a set of files needs, and the bytes they download.
pub(crate) fn chunks_for<'a>(
    manifest: &'a DownloadInformation,
    files: &HashSet<&str>,
) -> Vec<(&'a str, &'a str, &'a droplet_rs::manifest::ChunkData, [u8; 16])> {
    let mut out = Vec::new();
    for (version_key, m) in &manifest.manifests {
        for (chunk_id, chunk) in &m.chunks {
            let needed = chunk.files.iter().any(|f| {
                files.contains(f.filename.as_str())
                    && manifest.file_list.get(&f.filename) == Some(version_key)
            });
            if needed {
                out.push((version_key.as_str(), chunk_id.as_str(), chunk, m.key));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(b.1));
    out
}

pub(crate) fn chunk_bytes(chunk: &droplet_rs::manifest::ChunkData) -> u64 {
    chunk.files.iter().map(|f| f.length as u64).sum()
}

/// Work out the update for one install. Reads the server's file lists and
/// manifest, and hashes files on disk where the plan needs it.
pub async fn prepare(game_id: &str, install_version_id: &str, to_version_id: &str) -> Result<Prepared, UpdateError> {
    if update_active(game_id) {
        return Err(UpdateError::Busy);
    }
    let install = {
        let db = borrow_db_checked();
        find_install(&db, game_id, install_version_id)?
    };
    recover_install(&install.install_dir, None).map_err(UpdateError::NeedsRecovery)?;

    // A delta version's file list and manifest cover only the files it
    // changes over its base, not the whole install. Planned as a full file
    // list, every other file would read as removed. Composing the delta chain
    // isn't supported, so in-place updates to or from one are refused.
    let game_version = fetch_game_version(game_id, to_version_id).await?;
    if game_version.delta || installed_is_delta(game_id, &install.version_id).await? {
        return Err(UpdateError::DeltaVersion);
    }

    // The manifest first: a server from before in-place updates has no
    // `revision` in it, and its missing file-list route would otherwise read
    // as "no file list yet, try again later".
    let manifest = fetch_download_info(game_id, to_version_id, None).await?;
    if manifest.revision.is_none() {
        return Err(UpdateError::ServerUnsupported);
    }
    let target = fetch_revision_files(to_version_id, RevisionQuery::Current)
        .await?
        .ok_or(UpdateError::NoFileList)?;
    let (baseline_files, baseline_source, from_revision, kept_mine) = load_baseline(&install).await?;
    match manifest.revision {
        None => return Err(UpdateError::ServerUnsupported),
        Some(r) if r != target.revision => {
            return Err(UpdateError::RevisionChanged {
                planned: target.revision,
                current: r,
            });
        }
        Some(_) => {}
    }

    let install_dir = install.install_dir.clone();
    let target_files = target.files.clone();
    let plan = tauri::async_runtime::spawn_blocking(move || -> Result<Plan, UpdateError> {
        let mod_owned = match mod_owned_files_spelled(&install_dir) {
            Ok(set) => set,
            Err(why) => {
                // Without ownership, mod files read as the player's: they
                // become conflicts the player decides, never silent writes.
                warn!("update plan for {}: {why}; treating mod files as the player's", install_dir.display());
                HashSet::new()
            }
        };
        let disk = baseline::FsDisk {
            root: install_dir.clone(),
        };
        plan::plan(PlanInput {
            baseline: &baseline_files,
            target: &target_files,
            disk: &disk,
            is_protected: &is_protected_user_data,
            mod_owned: &mod_owned,
            case_insensitive: cfg!(windows),
            kept_mine: &kept_mine,
        })
        .map_err(|e| UpdateError::Plan(e.to_string()))
    })
    .await
    .map_err(|e| UpdateError::Io(format!("planning stopped: {e}")))??;

    for w in &plan.writes {
        if !manifest.file_list.contains_key(&w.file.path) {
            return Err(UpdateError::Inconsistent(format!("{} is not in the manifest", w.file.path)));
        }
    }

    Ok(Prepared {
        install,
        to_version_id: to_version_id.to_string(),
        to_revision: target.revision,
        from_revision,
        baseline_source,
        plan,
        manifest,
        game_version,
    })
}

/// Whether the installed version is a delta version: from the local copy of
/// its version info, or the server's. A version the server no longer has
/// counts as not delta (its file list is gone too, so the baseline is empty
/// and nothing would be removed anyway).
async fn installed_is_delta(game_id: &str, version_id: &str) -> Result<bool, UpdateError> {
    let local = borrow_db_checked()
        .applications
        .game_versions
        .get(version_id)
        .map(|v| v.delta);
    if let Some(delta) = local {
        return Ok(delta);
    }
    match fetch_game_version(game_id, version_id).await {
        Ok(v) => Ok(v.delta),
        Err(RemoteAccessError::ServerError { status: 404, .. }) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

impl Prepared {
    /// Bytes to download if every conflict takes the update (the most it can
    /// be; keeping files only lowers it).
    pub fn download_bytes(&self) -> u64 {
        let files: HashSet<&str> = self.plan.writes.iter().map(|w| w.file.path.as_str()).collect();
        chunks_for(&self.manifest, &files)
            .into_iter()
            .map(|(_, _, c, _)| chunk_bytes(c))
            .sum()
    }

    pub fn summary(&self) -> UpdatePlan {
        let (add_count, update_count, remove_count) = self.plan.counts();
        UpdatePlan {
            game_id: self.install.game_id.clone(),
            from_version_id: self.install.version_id.clone(),
            from_revision: self.from_revision,
            to_version_id: self.to_version_id.clone(),
            to_revision: self.to_revision,
            add_count,
            update_count,
            remove_count,
            download_bytes: self.download_bytes(),
            conflicts: self.plan.conflicts(),
            backup_paths: self.plan.backup_paths(),
            baseline_source: self.baseline_source,
        }
    }
}

/// Whether this install may be updated now (checked before queueing).
fn check_can_update(db: &Database, install: &InstallRef, to_version_id: &str) -> Result<(), UpdateError> {
    let game_busy = db
        .applications
        .transient_statuses
        .keys()
        .any(|m| m.id == install.game_id && m.download_type == DownloadType::Game);
    if game_busy {
        return Err(UpdateError::Busy);
    }
    if db
        .applications
        .transient_statuses
        .keys()
        .any(|m| m.download_type == DownloadType::Mod)
    {
        return Err(UpdateError::ModDownloadActive);
    }
    if to_version_id != install.version_id
        && let Some(other) = db.applications.get_install(&install.game_id, to_version_id)
        && Path::new(&other.install_dir) != install.install_dir
    {
        return Err(UpdateError::TargetAlreadyInstalled);
    }
    Ok(())
}

/// Re-plan, check the player decided every conflict, and queue the update.
pub async fn apply(
    game_id: &str,
    install_version_id: &str,
    to_version_id: &str,
    to_revision: u32,
    resolutions: HashMap<String, Resolution>,
) -> Result<(), UpdateError> {
    if update_active(game_id) {
        return Err(UpdateError::Busy);
    }
    let prepared = prepare(game_id, install_version_id, to_version_id).await?;
    if prepared.to_revision != to_revision {
        return Err(UpdateError::RevisionChanged {
            planned: to_revision,
            current: prepared.to_revision,
        });
    }
    {
        let db = borrow_db_checked();
        check_can_update(&db, &prepared.install, to_version_id)?;
    }
    if with_launches_blocked(game_id, |running| running) {
        return Err(UpdateError::GameRunning);
    }
    let resolved = plan::resolve(prepared.plan.clone(), &resolutions).map_err(UpdateError::Unresolved)?;

    let needed: u64 = resolved.writes.iter().map(|w| w.file.size).sum();
    let available = crate::downloads::utils::get_disk_available(prepared.install.install_dir.clone())?;
    if needed > available {
        return Err(UpdateError::DiskFull { needed, available });
    }

    let game_version = prepared.game_version.clone();

    if !try_claim(game_id) {
        return Err(UpdateError::Busy);
    }
    let agent = match agent::UpdateAgent::new(
        prepared,
        resolved,
        game_version,
        DOWNLOAD_MANAGER.get_sender(),
        DOWNLOAD_MANAGER.clone_depot_manager(),
    ) {
        Ok(a) => a,
        Err(e) => {
            release(game_id);
            return Err(e);
        }
    };
    let agent = std::sync::Arc::new(Box::new(agent) as Box<dyn Downloadable + Send + Sync>);
    if let Err(e) = DOWNLOAD_MANAGER.queue_download(agent).await {
        release(game_id);
        return Err(UpdateError::Queue(e.to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Moving the install record

/// Point the install at `to` (same folder). Idempotent: a roll-forward after a
/// crash may run it again. Keeps the install's type and the player's settings
/// for the game (`user_configuration`).
pub fn move_install_record(
    db: &mut Database,
    game_id: &str,
    from: &str,
    to: &str,
    install_dir: &Path,
    target_version: Option<GameVersion>,
) {
    let same_dir = |d: &str| Path::new(d) == install_dir;
    let record = match (
        db.applications.get_install(game_id, from).cloned(),
        db.applications.get_install(game_id, to).cloned(),
    ) {
        (Some(r), _) if same_dir(&r.install_dir) => r,
        (_, Some(r)) if same_dir(&r.install_dir) => r,
        _ => {
            error!(
                "update of {game_id}: no install record for {from} or {to} in {}; the files are \
                 updated but the library entry was not moved",
                install_dir.display()
            );
            return;
        }
    };

    let configuration = db
        .applications
        .game_versions
        .get(from)
        .or_else(|| db.applications.game_versions.get(to))
        .map(|v| v.user_configuration.clone());
    let version = match target_version {
        Some(v) => Some(v),
        // No target version to hand (a roll-forward without one): reuse the
        // old entry so launches keep working until the next library sync.
        None => db.applications.game_versions.get(from).cloned().map(|mut v| {
            v.version_id = to.to_string();
            v
        }),
    };
    if let Some(mut v) = version {
        if let Some(c) = configuration {
            v.user_configuration = c;
        }
        db.applications.game_versions.insert(to.to_string(), v);
    }

    if from != to {
        db.applications.remove_install(game_id, from);
    }
    db.applications.upsert_install(InstallRecord {
        version_id: to.to_string(),
        update_available: false,
        ..record
    });

    if let Some(meta) = db.applications.installed_game_version.get_mut(game_id)
        && meta.version == from
        && meta.download_type == DownloadType::Game
    {
        meta.version = to.to_string();
    }
    let points_here = matches!(
        db.applications.game_statuses.get(game_id),
        Some(GameDownloadStatus::Installed { version_id, install_dir: d, .. })
            if (version_id == from || version_id == to) && same_dir(d)
    );
    if points_here {
        if let Some(status @ GameDownloadStatus::Installed { .. }) = db.applications.game_statuses.get(game_id) {
            transition_from_db(db, game_id, StatusKind::from_persistent(status));
        }
        if let Some(GameDownloadStatus::Installed {
            version_id,
            update_available,
            ..
        }) = db.applications.game_statuses.get_mut(game_id)
        {
            *version_id = to.to_string();
            *update_available = false;
        }
    }
    info!("install of {game_id} in {} moved from {from} to {to}", install_dir.display());
}

/// Push the game's current state to the UI (`update_game/<id>`).
pub fn push_state(app: &AppHandle, game_id: &str) {
    let db = borrow_db_checked();
    let version = db
        .applications
        .installed_game_version
        .get(game_id)
        .and_then(|m| db.applications.game_versions.get(&m.version).cloned());
    push_game_update(app, &game_id.to_string(), version, GameStatusManager::fetch_state(&game_id.to_string(), &db));
}

// ---------------------------------------------------------------------------
// Recovery of an interrupted commit

#[derive(Debug, PartialEq, Eq)]
pub enum Recovery {
    Nothing,
    RolledBack,
    RolledForward,
    /// An update of this game is running right now; it owns the journal.
    Skipped,
}

/// Finish or undo a commit a crash interrupted in `install_dir`. Called at
/// startup, before a launch and before planning an update.
pub fn recover_install(install_dir: &Path, app: Option<&AppHandle>) -> Result<Recovery, String> {
    let journal = match commit::read_journal(install_dir) {
        Ok(Some(j)) => j,
        Ok(None) => return Ok(Recovery::Nothing),
        Err(e) => {
            return Err(format!(
                "an interrupted update in {} could not be read ({e}); the game can't be played \
                 until it is set aside",
                install_dir.display()
            ));
        }
    };
    if !try_claim(&journal.game_id) {
        return Ok(Recovery::Skipped);
    }
    let result = {
        let _staging = commit::lock_staging();
        match journal.phase {
            commit::Phase::Moving => commit::rollback(install_dir, &journal).map(|()| Recovery::RolledBack),
            commit::Phase::Committed => roll_forward(install_dir, &journal),
        }
    };
    release(&journal.game_id);
    match &result {
        Ok(r) => info!("update recovery in {}: {r:?}", install_dir.display()),
        Err(e) => error!("update recovery in {} failed: {e}", install_dir.display()),
    }
    if let (Some(app), Ok(Recovery::RolledForward | Recovery::RolledBack)) = (app, &result) {
        push_state(app, &journal.game_id);
    }
    result
}

/// Finish a committed update: the install record, the mod hand-over, the
/// clean-up. Each step is safe to repeat.
fn roll_forward(install_dir: &Path, journal: &commit::Journal) -> Result<Recovery, String> {
    let version = journal
        .game_version
        .clone()
        .and_then(|v| serde_json::from_value::<GameVersion>(v).ok());
    {
        let mut db = borrow_db_mut_checked();
        move_install_record(
            &mut db,
            &journal.game_id,
            &journal.from_version_id,
            &journal.to_version_id,
            install_dir,
            version,
        );
        db.applications.transient_statuses.retain(|m, _| {
            !(m.id == journal.game_id
                && m.download_type == DownloadType::Game
                && m.version == journal.to_version_id)
        });
    }
    apply_handover(install_dir, journal)?;
    let baks = commit::finish(install_dir, journal)?;
    for b in baks {
        info!("kept the player's copy as {}", b.display());
    }
    Ok(Recovery::RolledForward)
}

/// The mod hand-over for a committed update, as a full download does it:
/// the game takes back every file it wrote that a mod had replaced, and no
/// mod may restore an old copy of a file this version rewrote or dropped.
/// Runs after the commit point (so it is never undone half-way by a
/// rollback) and again on a roll-forward; both steps are idempotent.
pub(crate) fn apply_handover(install_dir: &Path, journal: &commit::Journal) -> Result<(), String> {
    crate::downloads::mod_data::hand_files_to_base_game(install_dir, &journal.handover)
        .map_err(|e| format!("could not take back files a mod had replaced: {e}"))?;
    crate::downloads::mod_data::discard_originals(install_dir, &journal.dropped)
        .map_err(|e| format!("could not stop mods from putting back removed files: {e}"))
}

/// What `recover_update` did (the `recover_game_update` command's result).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum RecoverOutcome {
    /// No unfinished update here.
    Nothing,
    /// The interrupted update was undone; the game is as it was before it.
    RolledBack,
    /// The interrupted update had already been applied; it was finished.
    RolledForward,
    /// It could be neither undone nor finished. The update's folder (with
    /// the player's files from before the update in `old/`) was renamed to
    /// `folder`, inside the game folder, so the game can be launched again.
    /// The install may be part-updated: a repair, or applying the update
    /// again, brings it back in line.
    SetAside { folder: String },
}

/// The way out of an update that recovery can't finish or undo: try once
/// more, and if that fails, set the update's folder aside, untouched, so
/// launching works again. Nothing is deleted.
pub fn recover_update(game_id: &str, version_id: &str, app: Option<&AppHandle>) -> Result<RecoverOutcome, UpdateError> {
    if update_active(game_id) {
        return Err(UpdateError::Busy);
    }
    let install_dir = borrow_db_checked()
        .applications
        .get_install(game_id, version_id)
        .map(|r| PathBuf::from(&r.install_dir))
        .ok_or(UpdateError::NotInstalled)?;
    match recover_install(&install_dir, app) {
        Ok(Recovery::Nothing) => Ok(RecoverOutcome::Nothing),
        Ok(Recovery::RolledBack) => Ok(RecoverOutcome::RolledBack),
        Ok(Recovery::RolledForward) => Ok(RecoverOutcome::RolledForward),
        Ok(Recovery::Skipped) => Err(UpdateError::Busy),
        Err(why) => {
            if !try_claim(game_id) {
                return Err(UpdateError::Busy);
            }
            let set_aside = {
                let _staging = commit::lock_staging();
                set_aside_update_dir(&install_dir)
            };
            release(game_id);
            let folder = set_aside.map_err(|e| UpdateError::Io(format!("{why}; setting it aside also failed: {e}")))?;
            warn!(
                "{game_id}: an unfinished update could not be recovered ({why}); set aside as {}",
                folder.display()
            );
            if let Some(app) = app {
                push_state(app, game_id);
            }
            Ok(RecoverOutcome::SetAside {
                folder: folder.to_string_lossy().to_string(),
            })
        }
    }
}

/// Rename `.drop-update` to `.drop-update-set-aside-<unix seconds>` in the
/// same folder. Hold [`commit::lock_staging`].
fn set_aside_update_dir(install_dir: &Path) -> std::io::Result<PathBuf> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let to = install_dir.join(format!("{UPDATE_DIR}-set-aside-{secs}"));
    std::fs::rename(commit::update_dir(install_dir), &to)?;
    baseline::sync_dir(Some(install_dir));
    Ok(to)
}

/// Before a repair rewrites files the player chose to keep in an update
/// (`kept_mine` in the baseline), copy each one that still differs from the
/// game's copy to `<file>.bak`. `about_to_write` holds the files the repair
/// writes, as the manifest spells them. Returns the copies made; an error
/// means a kept file could not be backed up, and the repair must not run.
pub(crate) fn back_up_kept_files(
    install_dir: &Path,
    version_id: &str,
    about_to_write: &HashSet<String>,
) -> Result<Vec<PathBuf>, String> {
    let sidecar = match baseline::read_sidecar(install_dir) {
        Ok(Some(s)) if s.version_id == version_id => s,
        Ok(_) => return Ok(Vec::new()),
        Err(e) => {
            warn!("could not read the update baseline in {}: {e}", install_dir.display());
            return Ok(Vec::new());
        }
    };
    let mut copies = Vec::new();
    for rel in sidecar.kept_mine.iter().filter(|p| about_to_write.contains(*p)) {
        let Ok(path) = utils::path_guard::join_within(install_dir, Path::new(rel)) else {
            continue;
        };
        if !std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) {
            continue;
        }
        let game_hash = sidecar.files.iter().find(|f| f.path == *rel).map(|f| f.sha256.as_str());
        let same = match game_hash {
            Some(h) => baseline::sha256_file(&path).is_ok_and(|disk| plan::same_hash(&disk, h)),
            None => false,
        };
        if same {
            continue;
        }
        let bak = commit::free_bak_path(&path);
        std::fs::copy(&path, &bak).map_err(|e| format!("could not keep your copy of {rel}: {e}"))?;
        info!("kept the player's copy of {rel} as {}", bak.display());
        copies.push(bak);
    }
    Ok(copies)
}

/// Recover every install at startup. Runs before the state reconcile, which
/// would otherwise see a `.dropdata` moved aside mid-commit and demote the
/// game to partially installed.
pub fn recover_all_at_startup() {
    let dirs: Vec<PathBuf> = {
        let db = borrow_db_checked();
        let mut dirs: Vec<PathBuf> = db
            .applications
            .installs
            .values()
            .map(|r| PathBuf::from(&r.install_dir))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    };
    for dir in dirs {
        if commit::journal_present(&dir)
            && let Err(e) = recover_install(&dir, None)
        {
            error!("{e}");
        }
    }
}

/// Recover the install a launch is about to use. `version` None means the
/// game's current install.
pub fn recover_before_launch(game_id: &str, version: Option<&str>) {
    let dir = {
        let db = borrow_db_checked();
        let version = version
            .map(str::to_string)
            .or_else(|| db.applications.installed_game_version.get(game_id).map(|m| m.version.clone()));
        version.and_then(|v| db.applications.get_install(game_id, &v).map(|r| PathBuf::from(&r.install_dir)))
    };
    if let Some(dir) = dir
        && commit::journal_present(&dir)
        && let Err(e) = recover_install(&dir, None)
    {
        error!("{e}");
    }
}

/// Whether a launch from `install_dir` must wait: an update commit there was
/// interrupted before its commit point and could not be undone yet, or its
/// journal can't be read. A committed update only has clean-up left and its
/// files are final, so it does not block.
pub fn launch_blocked(install_dir: &Path) -> bool {
    match commit::read_journal(install_dir) {
        Ok(None) => false,
        Ok(Some(j)) => j.phase == commit::Phase::Moving,
        Err(_) => true,
    }
}

// ---------------------------------------------------------------------------
// Baseline after a fresh install

/// Write the sidecar for a folder the download agent just installed or
/// repaired, from the server's list for the revision the manifest belonged
/// to. Best effort: without it the next update uses the server's earliest
/// list instead, which only means more conflicts, never lost files.
pub async fn record_fresh_baseline(game_id: &str, version_id: &str, install_dir: &Path, revision: Option<u32>) {
    // A delta version's list covers only its own files (see `prepare`).
    let delta = borrow_db_checked()
        .applications
        .game_versions
        .get(version_id)
        .is_some_and(|v| v.delta);
    if delta {
        info!("{game_id}: {version_id} is a delta version; not recording an update baseline");
        return;
    }
    let Some(revision) = revision else {
        info!("{game_id}: server sent no revision; not recording an update baseline");
        return;
    };
    let files = match fetch_revision_files(version_id, RevisionQuery::Exact(revision)).await {
        Ok(Some(r)) => r.files,
        Ok(None) => {
            warn!("{game_id}: server has no file list for {version_id} rev {revision}; no update baseline recorded");
            return;
        }
        Err(e) => {
            warn!("{game_id}: could not fetch the file list for {version_id} rev {revision} ({e}); no update baseline recorded");
            return;
        }
    };
    let dir = install_dir.to_path_buf();
    let (g, v) = (game_id.to_string(), version_id.to_string());
    let written = tauri::async_runtime::spawn_blocking(move || {
        let sidecar = baseline::fresh_sidecar(&dir, &g, &v, revision, &files);
        baseline::write_sidecar(&dir, &sidecar)
    })
    .await;
    match written {
        Ok(Ok(())) => info!("{game_id}: recorded update baseline for {version_id} rev {revision}"),
        Ok(Err(e)) => warn!("{game_id}: could not write the update baseline: {e}"),
        Err(e) => warn!("{game_id}: writing the update baseline stopped: {e}"),
    }
}

/// The metadata an update of this install queues under.
pub fn update_meta(game_id: &str, to_version_id: &str, platform: Platform) -> DownloadableMetadata {
    DownloadableMetadata::new(game_id.to_string(), to_version_id.to_string(), platform, DownloadType::Game)
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::models::data::UserConfiguration;

    fn version(game: &str, version: &str, emulator_game: Option<&str>) -> GameVersion {
        let emulator = emulator_game.map(|g| {
            serde_json::json!({ "launchId": "l", "gameId": g, "versionId": "ev" })
        });
        serde_json::from_value(serde_json::json!({
            "gameId": game,
            "versionId": version,
            "displayName": null,
            "versionPath": version,
            "onlySetup": false,
            "versionIndex": 0,
            "delta": false,
            "launches": [{
                "launchId": "l1",
                "name": "Play",
                "command": "game.bin",
                "platform": "Linux",
                "umuIdOverride": null,
                "emulator": emulator,
            }],
            "setups": [],
        }))
        .expect("GameVersion json")
    }

    fn installed(db: &mut Database, game: &str, version: &str, dir: &str) {
        db.applications.upsert_install(InstallRecord {
            game_id: game.into(),
            version_id: version.into(),
            target_platform: Platform::Linux,
            install_dir: dir.into(),
            install_type: InstalledGameType::Installed,
            update_available: true,
        });
        db.applications.installed_game_version.insert(
            game.into(),
            DownloadableMetadata::new(game.into(), version.into(), Platform::Linux, DownloadType::Game),
        );
        db.applications.game_statuses.insert(
            game.into(),
            GameDownloadStatus::Installed {
                install_type: InstalledGameType::Installed,
                version_id: version.into(),
                install_dir: dir.into(),
                update_available: true,
            },
        );
    }

    #[test]
    fn roms_that_launch_through_an_emulator_block_its_update() {
        let mut db = Database::default();
        db.applications
            .game_versions
            .insert("rv".into(), version("rom", "rv", Some("retroarch")));
        db.applications
            .game_versions
            .insert("ov".into(), version("other", "ov", None));
        assert_eq!(games_using(&db, "retroarch"), vec!["retroarch".to_string(), "rom".to_string()]);
        assert_eq!(games_using(&db, "other"), vec!["other".to_string()]);
    }

    #[test]
    fn the_install_record_moves_in_place_and_keeps_the_players_settings() {
        let mut db = Database::default();
        installed(&mut db, "g", "v1", "/games/G");
        let mut old = version("g", "v1", None);
        old.user_configuration = UserConfiguration {
            launch_template: "gamemoderun {}".into(),
            enable_updates: true,
            ..Default::default()
        };
        db.applications.game_versions.insert("v1".into(), old);

        let target = version("g", "v2", None);
        move_install_record(&mut db, "g", "v1", "v2", Path::new("/games/G"), Some(target.clone()));
        // Running it again (a roll-forward after a crash) changes nothing.
        move_install_record(&mut db, "g", "v1", "v2", Path::new("/games/G"), Some(target));

        assert!(db.applications.get_install("g", "v1").is_none());
        let rec = db.applications.get_install("g", "v2").expect("moved");
        assert_eq!(rec.install_dir, "/games/G");
        assert!(!rec.update_available);
        assert_eq!(db.applications.installed_game_version["g"].version, "v2");
        assert!(matches!(
            &db.applications.game_statuses["g"],
            GameDownloadStatus::Installed { version_id, update_available: false, .. } if version_id == "v2"
        ));
        let gv = &db.applications.game_versions["v2"];
        assert_eq!(gv.user_configuration.launch_template, "gamemoderun {}");
        assert!(gv.user_configuration.enable_updates);
    }

    #[test]
    fn a_record_for_another_folder_is_never_moved() {
        let mut db = Database::default();
        installed(&mut db, "g", "v1", "/games/G");
        move_install_record(&mut db, "g", "v1", "v2", Path::new("/elsewhere"), None);
        assert!(db.applications.get_install("g", "v1").is_some());
        assert!(db.applications.get_install("g", "v2").is_none());
    }

    #[test]
    fn only_an_unfinished_or_unreadable_journal_blocks_a_launch() {
        let dir = std::env::temp_dir().join(format!("drop-launch-blocked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(commit::update_dir(&dir)).unwrap();
        assert!(!launch_blocked(&dir));
        let mut journal = commit::Journal {
            game_id: "g".into(),
            from_version_id: "v1".into(),
            to_version_id: "v2".into(),
            to_revision: 2,
            phase: commit::Phase::Moving,
            ops: vec![],
            created_dirs: vec![],
            game_version: None,
            handover: vec![],
            dropped: vec![],
        };
        std::fs::write(commit::journal_path(&dir), serde_json::to_vec(&journal).unwrap()).unwrap();
        assert!(launch_blocked(&dir));
        journal.phase = commit::Phase::Committed;
        std::fs::write(commit::journal_path(&dir), serde_json::to_vec(&journal).unwrap()).unwrap();
        assert!(!launch_blocked(&dir));
        std::fs::write(commit::journal_path(&dir), b"{garbage").unwrap();
        assert!(launch_blocked(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop-update-mod-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_repair_keeps_a_bak_of_files_the_player_chose_to_keep() {
        let dir = scratch("kept");
        std::fs::write(dir.join("pack.toml"), b"player version").unwrap();
        std::fs::write(dir.join("same.toml"), b"abc").unwrap();
        std::fs::write(dir.join("untouched.toml"), b"player").unwrap();
        let entry = |path: &str, sha: &str| BaselineFile {
            path: path.into(),
            size: 3,
            sha256: sha.into(),
            mtime: None,
        };
        let sidecar = baseline::Sidecar {
            game_id: "g".into(),
            version_id: "v2".into(),
            revision: 2,
            files: vec![
                entry("pack.toml", "00"),
                // sha256("abc"): already the game's copy, nothing to keep.
                entry("same.toml", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
                entry("untouched.toml", "00"),
            ],
            kept_mine: vec!["pack.toml".into(), "same.toml".into(), "untouched.toml".into()],
        };
        baseline::write_sidecar(&dir, &sidecar).unwrap();
        let writing: HashSet<String> = ["pack.toml".to_string(), "same.toml".to_string()].into();
        let copies = back_up_kept_files(&dir, "v2", &writing).unwrap();
        assert_eq!(copies, vec![dir.join("pack.toml.bak")]);
        assert_eq!(std::fs::read(dir.join("pack.toml.bak")).unwrap(), b"player version");
        // A sidecar for another version says nothing about this repair.
        assert!(back_up_kept_files(&dir, "v3", &writing).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unrecoverable_update_can_be_set_aside_without_deleting_anything() {
        let dir = scratch("set-aside");
        std::fs::create_dir_all(commit::old_dir(&dir)).unwrap();
        std::fs::write(commit::old_dir(&dir).join("1"), b"player original").unwrap();
        std::fs::write(commit::journal_path(&dir), b"{garbage").unwrap();
        assert!(launch_blocked(&dir));
        let aside = set_aside_update_dir(&dir).unwrap();
        assert!(!launch_blocked(&dir));
        assert_eq!(std::fs::read(aside.join("old").join("1")).unwrap(), b"player original");
        assert!(aside.join("journal.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stuck_earlier_update_is_a_recognisable_error() {
        let e = UpdateError::NeedsRecovery("could not remove the old a.jar".into());
        let shown = serde_json::to_value(&e).unwrap();
        assert!(shown.as_str().unwrap().starts_with(NEEDS_RECOVERY_MARKER), "{shown}");
        assert!(!UpdateError::Busy.to_string().starts_with(NEEDS_RECOVERY_MARKER));
    }
}
