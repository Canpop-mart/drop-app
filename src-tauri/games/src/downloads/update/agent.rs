//! The download-queue agent for an in-place update.
//!
//! `download()` fetches only the chunks that hold a file the update writes,
//! and writes only those files, into `.drop-update/new/`. `validate()` checks
//! every staged file against the target's SHA-256, then commits (see
//! `commit`): originals move to `.drop-update/old/`, staged files move in,
//! the install record moves to the new version. A failure or cancel before
//! the commit point leaves the install exactly as it was.
//!
//! Pause/resume: completed chunks are recorded in `.drop-update/state.json`
//! with the update's identity (target, revision, files). Queuing the same
//! update again, even after a restart, picks up where it stopped; a
//! different update throws the staging folder away first.

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use database::models::data::GameVersion;
use database::platform::Platform;
use database::{ApplicationTransientStatus, DownloadableMetadata, borrow_db_checked, borrow_db_mut_checked};
use download_manager::depot_manager::DepotManager;
use download_manager::download_manager_frontend::{DownloadManagerSignal, DownloadStatus};
use download_manager::downloadable::Downloadable;
use download_manager::error::ApplicationDownloadError;
use download_manager::util::download_thread_control_flag::{
    DownloadThreadControl, DownloadThreadControlFlag,
};
use download_manager::util::progress_object::{ProgressHandle, ProgressObject, ProgressType};
use droplet_rs::manifest::ChunkData;
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tokio::sync::mpsc::Sender;
use utils::{app_emit, path_guard};

use super::super::download_agent::{RETRY_COUNT, is_disk_full};
use super::super::download_logic::download_game_chunk;
use super::super::drop_data::{DROPDATA_PATH, DropData};
use super::baseline::{self, Sidecar, sha256_file, stat_path};
use super::commit::{self, Journal, JournalOp, Original, Phase};
use super::plan::{BaselineFile, DiskStat, DiskView, Existing, Expect, NextMtime, Resolved, same_hash};
use super::{
    BASELINE_FILE, Prepared, UpdateError, chunk_bytes, chunks_for, move_install_record, push_state,
    release, update_meta, with_launches_blocked,
};

const STATE_FILE: &str = "state.json";

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StageState {
    identity: String,
    completed_chunks: Vec<String>,
}

struct StageChunk {
    version_key: String,
    chunk_id: String,
    data: ChunkData,
    key: [u8; 16],
}

#[derive(Default)]
struct Life {
    /// `download()` is staging chunks.
    running: bool,
    /// `validate()` is verifying or committing.
    committing: bool,
    cancelled: bool,
    done: bool,
}

/// What a cancel may do right now.
#[derive(Debug, PartialEq, Eq)]
enum OnCancel {
    /// The update was already applied; nothing to cancel.
    AlreadyDone,
    /// A download or a commit is running on the queue's thread. It sees the
    /// flag and finishes the cancel itself when it stops; touching the
    /// staging folder or releasing the game from here could pull files out
    /// from under a commit.
    Defer,
    /// Nothing is running: discard the staging folder and release now.
    DiscardNow,
}

impl Life {
    fn cancel(&mut self) -> OnCancel {
        if self.done {
            return OnCancel::AlreadyDone;
        }
        self.cancelled = true;
        if self.running || self.committing {
            OnCancel::Defer
        } else {
            OnCancel::DiscardNow
        }
    }
}

pub struct UpdateAgent {
    meta: DownloadableMetadata,
    from_version: String,
    install_dir: PathBuf,
    to_revision: u32,
    resolved: Resolved,
    chunks: Vec<StageChunk>,
    /// Every chunk of the target manifest, for the new `.dropdata`.
    all_chunk_ids: Vec<String>,
    /// The files to write, keyed as the manifest spells them, mapped to the
    /// manifest version that ships them (what `download_game_chunk` expects).
    write_list: HashMap<String, String>,
    game_version: GameVersion,
    identity: String,
    control_flag: DownloadThreadControl,
    download_progress: Arc<ProgressObject>,
    disk_progress: Arc<ProgressObject>,
    depot_manager: Arc<DepotManager>,
    status: Mutex<DownloadStatus>,
    completed: Mutex<HashSet<String>>,
    life: Mutex<Life>,
}

impl Debug for UpdateAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateAgent").field("meta", &self.meta).finish()
    }
}

fn update_err(e: impl std::fmt::Display) -> ApplicationDownloadError {
    ApplicationDownloadError::UpdateFailed(e.to_string())
}

fn identity_of(game_id: &str, to_version: &str, revision: u32, resolved: &Resolved) -> String {
    let mut lines: Vec<String> = resolved
        .writes
        .iter()
        .map(|w| format!("{}\n{}", w.file.path, w.file.sha256))
        .collect();
    lines.sort();
    let mut h = Sha256::new();
    h.update(format!("{game_id}\n{to_version}\n{revision}\n").as_bytes());
    for l in lines {
        h.update(l.as_bytes());
        h.update(b"\n");
    }
    hex::encode(h.finalize())
}

impl UpdateAgent {
    pub fn new(
        prepared: Prepared,
        resolved: Resolved,
        game_version: GameVersion,
        sender: Sender<DownloadManagerSignal>,
        depot_manager: Arc<DepotManager>,
    ) -> Result<Self, UpdateError> {
        let Prepared {
            install,
            to_version_id,
            to_revision,
            manifest,
            ..
        } = prepared;
        let meta = update_meta(&install.game_id, &to_version_id, install.platform);

        let paths: HashSet<&str> = resolved.writes.iter().map(|w| w.file.path.as_str()).collect();
        let chunks: Vec<StageChunk> = chunks_for(&manifest, &paths)
            .into_iter()
            .map(|(v, id, data, key)| StageChunk {
                version_key: v.to_string(),
                chunk_id: id.to_string(),
                data: data.clone(),
                key,
            })
            .collect();
        let write_list: HashMap<String, String> = resolved
            .writes
            .iter()
            .filter_map(|w| {
                manifest
                    .file_list
                    .get(&w.file.path)
                    .map(|v| (w.file.path.clone(), v.clone()))
            })
            .collect();
        let mut all_chunk_ids: Vec<String> = manifest
            .manifests
            .values()
            .flat_map(|m| m.chunks.keys().cloned())
            .collect();
        all_chunk_ids.sort();

        let identity = identity_of(&install.game_id, &to_version_id, to_revision, &resolved);
        let state_path = commit::update_dir(&install.install_dir).join(STATE_FILE);
        let previous: Option<StageState> = std::fs::read(&state_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let completed: HashSet<String> = match previous {
            Some(s) if s.identity == identity => {
                info!(
                    "update of {}: resuming with {} staged chunk(s)",
                    install.game_id,
                    s.completed_chunks.len()
                );
                s.completed_chunks.into_iter().collect()
            }
            _ => {
                commit::discard_staging_locked(&install.install_dir).map_err(|e| {
                    UpdateError::Io(format!("could not clear an earlier unfinished update: {e}"))
                })?;
                HashSet::new()
            }
        };

        let total_download: u64 = chunks.iter().map(|c| chunk_bytes(&c.data)).sum();
        let total_disk: u64 = resolved.writes.iter().map(|w| w.file.size).sum();
        let download_progress = Arc::new(ProgressObject::new(
            usize::try_from(total_download).unwrap_or(usize::MAX),
            chunks.len(),
            sender.clone(),
            ProgressType::Download,
        ));
        let disk_progress = Arc::new(ProgressObject::new(
            usize::try_from(total_disk).unwrap_or(usize::MAX),
            chunks.len(),
            sender,
            ProgressType::Disk,
        ));

        info!(
            "update of {} in {}: {} -> {} rev {}, {} file(s) to write, {} to delete, {} chunk(s)",
            install.game_id,
            install.install_dir.display(),
            install.version_id,
            to_version_id,
            to_revision,
            resolved.writes.len(),
            resolved.deletes.len(),
            chunks.len()
        );

        Ok(Self {
            meta,
            from_version: install.version_id,
            install_dir: install.install_dir,
            to_revision,
            resolved,
            chunks,
            all_chunk_ids,
            write_list,
            game_version,
            identity,
            control_flag: DownloadThreadControl::new(DownloadThreadControlFlag::Stop),
            download_progress,
            disk_progress,
            depot_manager,
            status: Mutex::new(DownloadStatus::Queued),
            completed: Mutex::new(completed),
            life: Mutex::new(Life::default()),
        })
    }

    fn new_dir(&self) -> PathBuf {
        commit::new_dir(&self.install_dir)
    }

    fn save_state(&self) {
        let completed: Vec<String> = match self.completed.lock() {
            Ok(c) => {
                let mut v: Vec<String> = c.iter().cloned().collect();
                v.sort();
                v
            }
            Err(_) => return,
        };
        let state = StageState {
            identity: self.identity.clone(),
            completed_chunks: completed,
        };
        let path = commit::update_dir(&self.install_dir).join(STATE_FILE);
        let written = serde_json::to_vec(&state)
            .map_err(std::io::Error::other)
            .and_then(|b| baseline::write_file_atomic(&path, &b));
        if let Err(e) = written {
            // Only costs re-downloading these chunks after a restart.
            warn!("update of {}: could not record staged chunks: {e}", self.meta.id);
        }
    }

    fn set_transient(&self, app: &AppHandle, status: ApplicationTransientStatus) {
        {
            let mut db = borrow_db_mut_checked();
            db.applications.transient_statuses.insert(self.meta.clone(), status);
        }
        push_state(app, &self.meta.id);
    }

    fn clear_transient(&self) {
        let mut db = borrow_db_mut_checked();
        db.applications.transient_statuses.remove(&self.meta);
    }

    /// Throw the staged download away (cancel). Never while a journal says a
    /// commit holds the player's files in the staging folder.
    fn discard(&self) {
        if let Err(e) = commit::discard_staging_locked(&self.install_dir) {
            warn!("update of {}: staging folder kept: {e}", self.meta.id);
        }
    }

    async fn run(&self) -> Result<bool, ApplicationDownloadError> {
        self.depot_manager.sync_depots().await?;
        std::fs::create_dir_all(self.new_dir())?;

        self.download_progress.reset();
        self.disk_progress.reset();
        let completed: HashSet<String> = self.completed.lock().map(|c| c.clone()).unwrap_or_default();

        let mut max_threads = borrow_db_checked().settings.max_download_threads;
        if max_threads == 0 {
            max_threads = 1;
        }
        let new_dir = self.new_dir();
        let new_dir = &new_dir;
        let write_list = &self.write_list;
        let mut errors: Vec<ApplicationDownloadError> = Vec::new();
        {
            let mut in_flight = FuturesUnordered::new();
            let mut handle = |r: Result<Option<String>, ApplicationDownloadError>| match r {
                Ok(Some(chunk_id)) => {
                    if let Ok(mut c) = self.completed.lock() {
                        c.insert(chunk_id);
                    }
                    self.save_state();
                }
                Ok(None) => {}
                Err(e) => errors.push(e),
            };

            for (index, chunk) in self.chunks.iter().enumerate() {
                let dl = ProgressHandle::new(self.download_progress.get(index), self.download_progress.clone());
                let disk = ProgressHandle::new(self.disk_progress.get(index), self.disk_progress.clone());
                if completed.contains(&chunk.chunk_id) {
                    dl.skip(chunk_bytes(&chunk.data) as usize);
                    continue;
                }
                if self.control_flag.get() == DownloadThreadControlFlag::Stop {
                    break;
                }
                let (depot, permit) = self.depot_manager.next_depot(&self.meta.id, &self.meta.version)?;
                while in_flight.len() >= max_threads {
                    if let Some(r) = in_flight.next().await {
                        handle(r);
                    }
                }
                in_flight.push(async move {
                    for attempt in 0..RETRY_COUNT {
                        match download_game_chunk(
                            &self.meta.id,
                            &chunk.version_key,
                            &chunk.chunk_id,
                            &depot,
                            &chunk.key,
                            &chunk.data,
                            write_list,
                            new_dir,
                            &self.control_flag,
                            &dl,
                            &disk,
                        )
                        .await
                        {
                            Ok(true) => {
                                drop(permit);
                                return Ok(Some(chunk.chunk_id.clone()));
                            }
                            Ok(false) => return Ok(None),
                            Err(e) => {
                                warn!("update chunk {} failed: {e:?}", chunk.chunk_id);
                                if attempt == RETRY_COUNT - 1 || is_disk_full(&e) {
                                    return Err(e);
                                }
                                tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                            }
                        }
                    }
                    Ok(None)
                });
            }
            while let Some(r) = in_flight.next().await {
                handle(r);
            }
        }
        if let Some(first) = errors.into_iter().next() {
            return Err(first);
        }

        let done = self.completed.lock().map(|c| c.len()).unwrap_or(0);
        let all_done = self
            .chunks
            .iter()
            .all(|c| self.completed.lock().map(|s| s.contains(&c.chunk_id)).unwrap_or(false));
        if !all_done {
            info!(
                "update of {} stopped with {done}/{} chunk(s) staged",
                self.meta.id,
                self.chunks.len()
            );
            return Ok(false);
        }

        // An empty file may not be in any chunk; make sure it exists.
        for w in &self.resolved.writes {
            if w.file.size == 0 {
                let p = path_guard::join_within(new_dir, Path::new(&w.file.path)).map_err(update_err)?;
                if stat_path(&p) == DiskStat::Missing {
                    if let Some(parent) = p.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::File::create(&p)?;
                }
            }
        }
        Ok(true)
    }

    /// Check every staged file against the target's hash. Bad ones have
    /// their chunks marked undone; returns whether all were good.
    fn verify_staged(&self) -> Result<bool, ApplicationDownloadError> {
        let new_dir = self.new_dir();
        let mut bad: Vec<&str> = Vec::new();
        for w in &self.resolved.writes {
            let p = path_guard::join_within(&new_dir, Path::new(&w.file.path)).map_err(update_err)?;
            let ok = match stat_path(&p) {
                // Unknown target hash: the chunk checksums verified the bytes
                // as they were written; only the size can be checked here.
                DiskStat::File { size, .. } if size == w.file.size && w.file.sha256.is_empty() => true,
                DiskStat::File { size, .. } if size == w.file.size => {
                    sha256_file(&p).is_ok_and(|h| same_hash(&h, &w.file.sha256))
                }
                _ => false,
            };
            if !ok {
                warn!("update of {}: staged {} does not match the server's file list", self.meta.id, w.file.path);
                bad.push(&w.file.path);
            }
        }
        if bad.is_empty() {
            return Ok(true);
        }
        let bad: HashSet<&str> = bad.into_iter().collect();
        let mut cleared = 0;
        if let Ok(mut c) = self.completed.lock() {
            for chunk in &self.chunks {
                if chunk.data.files.iter().any(|f| bad.contains(f.filename.as_str())) && c.remove(&chunk.chunk_id) {
                    cleared += 1;
                }
            }
        }
        self.save_state();
        if cleared == 0 {
            // Nothing to re-download would change it: the manifest and the
            // file list disagree.
            return Err(update_err(UpdateError::Inconsistent(format!(
                "{} file(s) do not match after download",
                bad.len()
            ))));
        }
        Ok(false)
    }

    /// Stage the new baseline and `.dropdata`, then swap everything in.
    fn commit_update(&self, app: &AppHandle) -> Result<Vec<PathBuf>, UpdateError> {
        let game_id = self.meta.id.clone();
        let to = self.meta.version.clone();

        if to != self.from_version {
            let db = borrow_db_checked();
            if let Some(other) = db.applications.get_install(&game_id, &to)
                && Path::new(&other.install_dir) != self.install_dir
            {
                return Err(UpdateError::TargetAlreadyInstalled);
            }
        }

        let input = CommitInput {
            install: &self.install_dir,
            game_id: &game_id,
            from_version: &self.from_version,
            to_version: &to,
            to_revision: self.to_revision,
            platform: self.meta.target_platform,
            resolved: &self.resolved,
            all_chunk_ids: &self.all_chunk_ids,
            game_version: serde_json::to_value(&self.game_version).ok(),
        };
        // Nothing else moves or deletes in a staging folder while this runs
        // (a cancel's discard, a recovery's rollback).
        let _staging = commit::lock_staging();
        let journal = stage_meta(&input)?;
        let journal = with_launches_blocked(&game_id, |running| {
            if running {
                return Err(UpdateError::GameRunning);
            }
            commit_staged(&input, journal)
        })?;

        // After the commit point: the mod hand-over (as a full download does
        // it), then the install record. A hand-over failure leaves the
        // journal in place, so the next launch or startup retries it before
        // cleaning up.
        let handed_over = super::apply_handover(&self.install_dir, &journal);
        {
            let mut db = borrow_db_mut_checked();
            move_install_record(
                &mut db,
                &game_id,
                &self.from_version,
                &to,
                &self.install_dir,
                Some(self.game_version.clone()),
            );
            db.applications.transient_statuses.remove(&self.meta);
        }
        push_state(app, &game_id);
        if let Err(e) = handed_over {
            error!("update of {game_id} applied, but the mod hand-over failed and will be retried: {e}");
            return Ok(Vec::new());
        }

        match commit::finish(&self.install_dir, &journal) {
            Ok(baks) => Ok(baks),
            Err(e) => {
                // Applied; only the clean-up is left. The journal stays and
                // the next startup or launch finishes it.
                error!("update of {game_id} applied, but clean-up failed: {e}");
                Ok(Vec::new())
            }
        }
    }
}

/// What committing a staged update needs, apart from the queue and the
/// database. Split out of the agent so the whole file side is testable.
pub(crate) struct CommitInput<'a> {
    pub install: &'a Path,
    pub game_id: &'a str,
    pub from_version: &'a str,
    pub to_version: &'a str,
    pub to_revision: u32,
    pub platform: Platform,
    pub resolved: &'a Resolved,
    /// Every chunk of the target manifest, for the new `.dropdata`.
    pub all_chunk_ids: &'a [String],
    pub game_version: Option<serde_json::Value>,
}

/// The files the player has chosen to keep their own copy of, after this
/// update: this update's "keep mine" decisions, plus earlier ones for files
/// this update leaves alone (still in the baseline, not written). A file the
/// update writes (the player took the update this time) or no longer ships
/// drops out.
fn kept_mine_after(input: &CommitInput<'_>) -> Vec<String> {
    let mut kept: Vec<String> = input.resolved.kept_mine.clone();
    let earlier = match baseline::read_sidecar(input.install) {
        Ok(Some(s)) if s.game_id == input.game_id => s.kept_mine,
        Ok(_) => Vec::new(),
        Err(e) => {
            warn!("could not read the earlier baseline in {}: {e}", input.install.display());
            Vec::new()
        }
    };
    let written: HashSet<&str> = input.resolved.writes.iter().map(|w| w.file.path.as_str()).collect();
    let in_baseline: HashSet<&str> = input.resolved.next_baseline.iter().map(|(f, _)| f.path.as_str()).collect();
    for p in earlier {
        if in_baseline.contains(p.as_str()) && !written.contains(p.as_str()) && !kept.contains(&p) {
            kept.push(p);
        }
    }
    kept.sort();
    kept
}

/// Write the next baseline and the new version's `.dropdata` into the staging
/// folder, and build the journal that swaps everything in.
pub(crate) fn stage_meta(input: &CommitInput<'_>) -> Result<Journal, UpdateError> {
    let new_dir = commit::new_dir(input.install);

    // The next baseline. A rename keeps a file's mtime, so the staged copy's
    // mtime is what the installed file will have.
    //
    // A target file with an unknown hash (`""` from the server) that this
    // update writes gets the hash of the bytes actually written. One it does
    // not write keeps `""` with no mtime: "unknown, never equal", never a
    // claim about what is on disk.
    let mut files: Vec<BaselineFile> = Vec::with_capacity(input.resolved.next_baseline.len());
    for (file, next) in &input.resolved.next_baseline {
        let entry = match next {
            NextMtime::Known(_) if file.sha256.is_empty() => BaselineFile::from_remote(file, None),
            NextMtime::Known(m) => BaselineFile::from_remote(file, *m),
            NextMtime::AfterWrite => {
                let staged = path_guard::join_within(&new_dir, Path::new(&file.path))
                    .map_err(|e| UpdateError::Io(format!("unsafe path {}: {e}", file.path)))?;
                let mtime = match stat_path(&staged) {
                    DiskStat::File { mtime, .. } => mtime,
                    _ => None,
                };
                let mut entry = BaselineFile::from_remote(file, mtime);
                if entry.sha256.is_empty() {
                    entry.sha256 = sha256_file(&staged).map_err(|e| {
                        UpdateError::Io(format!("could not read the downloaded {}: {e}", file.path))
                    })?;
                }
                entry
            }
        };
        files.push(entry);
    }
    let sidecar = Sidecar {
        game_id: input.game_id.to_string(),
        version_id: input.to_version.to_string(),
        revision: input.to_revision,
        files,
        kept_mine: kept_mine_after(input),
    };
    std::fs::create_dir_all(&new_dir).map_err(|e| UpdateError::Io(format!("could not create the staging folder: {e}")))?;
    baseline::write_sidecar(&new_dir, &sidecar)
        .map_err(|e| UpdateError::Io(format!("could not stage the new baseline: {e}")))?;

    // A fresh ledger for the new version with every chunk done, so a later
    // repair validates against the new manifest, and no "previous version"
    // is recorded (that would make a repair run the Phase 0 sweep against
    // the old version and delete files the player chose to keep).
    let dropdata = DropData::new(
        input.game_id.to_string(),
        input.to_version.to_string(),
        input.platform,
        input.install.to_path_buf(),
        None,
    );
    dropdata.set_contexts(&input.all_chunk_ids.iter().map(|c| (c.clone(), true)).collect::<Vec<_>>());
    dropdata
        .try_write_to(&new_dir)
        .map_err(|e| UpdateError::Io(format!("could not stage the new install record: {e}")))?;

    let mut slot = 0usize;
    let mut next_slot = || {
        slot += 1;
        slot.to_string()
    };
    // Deletes first: a removed file may stand where a written file's folder
    // goes.
    let mut ops: Vec<JournalOp> = Vec::new();
    for d in &input.resolved.deletes {
        ops.push(JournalOp {
            target: d.existing.disk_path.clone(),
            staged: false,
            original: Some(Original {
                disk_path: d.existing.disk_path.clone(),
                slot: next_slot(),
                keep_as_bak: d.keep_bak,
            }),
            ..Default::default()
        });
    }
    for w in &input.resolved.writes {
        ops.push(JournalOp {
            target: w.file.path.clone(),
            staged: true,
            original: w.existing.as_ref().map(|e| Original {
                disk_path: e.disk_path.clone(),
                slot: next_slot(),
                keep_as_bak: w.keep_bak,
            }),
            ..Default::default()
        });
    }
    for name in [BASELINE_FILE, DROPDATA_PATH] {
        let exists = std::fs::symlink_metadata(input.install.join(name)).is_ok();
        ops.push(JournalOp {
            target: name.to_string(),
            staged: true,
            original: exists.then(|| Original {
                disk_path: name.to_string(),
                slot: next_slot(),
                keep_as_bak: false,
            }),
            ..Default::default()
        });
    }
    Ok(Journal {
        game_id: input.game_id.to_string(),
        from_version_id: input.from_version.to_string(),
        to_version_id: input.to_version.to_string(),
        to_revision: input.to_revision,
        phase: Phase::Moving,
        ops,
        created_dirs: Vec::new(),
        game_version: input.game_version.clone(),
        handover: input.resolved.writes.iter().map(|w| w.file.path.clone()).collect(),
        dropped: input.resolved.dropped.clone(),
    })
}

/// The file the plan relied on is still what it saw.
fn still_as_planned(install: &Path, existing: Option<&Existing>, target: &str) -> Result<(), UpdateError> {
    let (path, expect) = match existing {
        Some(e) => (e.disk_path.as_str(), &e.expect),
        None => (target, &Expect::Missing),
    };
    let full = path_guard::join_within(install, Path::new(path))
        .map_err(|e| UpdateError::Io(format!("unsafe path {path}: {e}")))?;
    let stat = stat_path(&full);
    let ok = match (expect, stat) {
        (Expect::Missing, DiskStat::Missing) => true,
        (Expect::Missing, _) => false,
        (Expect::Present, s) => s != DiskStat::Missing,
        // Only what the review saw (and the update removes) may be in it.
        (Expect::Folder { files }, DiskStat::Dir) => baseline::FsDisk {
            root: install.to_path_buf(),
        }
        .files_under(path)
        .is_ok_and(|now| now.iter().all(|f| files.contains(f))),
        (Expect::Folder { .. }, _) => false,
        (
            Expect::Content { sha256, size, mtime },
            DiskStat::File {
                size: now_size,
                mtime: now_mtime,
            },
        ) => {
            now_size == *size
                && (now_mtime == *mtime || sha256_file(&full).is_ok_and(|h| same_hash(&h, sha256)))
        }
        (Expect::Content { .. }, _) => false,
    };
    if ok {
        Ok(())
    } else {
        Err(UpdateError::ChangedSinceReview(path.to_string()))
    }
}

/// Check nothing changed since the plan, then swap the staged files in. Run
/// with launches of the game blocked and [`commit::lock_staging`] held. The
/// mod hand-over is the caller's, after the commit point.
pub(crate) fn commit_staged(input: &CommitInput<'_>, journal: Journal) -> Result<Journal, UpdateError> {
    for w in &input.resolved.writes {
        still_as_planned(input.install, w.existing.as_ref(), &w.file.path)?;
    }
    for d in &input.resolved.deletes {
        still_as_planned(input.install, Some(&d.existing), &d.existing.disk_path)?;
    }
    commit::commit(input.install, journal)
        .map_err(|e| UpdateError::Io(format!("the update could not be applied: {e}")))
}

impl UpdateAgent {
    /// Finish a cancel that had to wait for the download or commit: once
    /// neither runs, discard the staging folder and release the game.
    fn finish_cancel_if_idle(&self) {
        let idle_cancelled = match self.life.lock() {
            Ok(life) => life.cancelled && !life.done && !life.running && !life.committing,
            Err(_) => false,
        };
        if idle_cancelled {
            self.discard();
            release(&self.meta.id);
        }
    }
}

/// Resets `running` when the download future ends, including when the
/// manager aborts it after a cancel, and finishes a cancel that arrived
/// while it ran.
struct RunGuard<'a>(&'a UpdateAgent);

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut life) = self.0.life.lock() {
            life.running = false;
        }
        self.0.finish_cancel_if_idle();
    }
}

/// The same for `validate()`: verifying and committing.
struct CommitGuard<'a>(&'a UpdateAgent);

impl Drop for CommitGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut life) = self.0.life.lock() {
            life.committing = false;
        }
        self.0.finish_cancel_if_idle();
    }
}

#[async_trait]
impl Downloadable for UpdateAgent {
    async fn download(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        {
            let mut life = self.life.lock().map_err(|_| ApplicationDownloadError::Lock)?;
            // Already applied (a pause landed after the commit): report
            // success so the queue completes it instead of holding it forever.
            if life.done {
                return Ok(true);
            }
            if life.cancelled {
                return Ok(false);
            }
            life.running = true;
        }
        let _guard = RunGuard(self);
        if let Ok(mut s) = self.status.lock() {
            *s = DownloadStatus::Downloading;
        }
        self.set_transient(
            app_handle,
            ApplicationTransientStatus::Updating {
                version_id: self.meta.version.clone(),
            },
        );
        self.run().await
    }

    fn validate(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        {
            let mut life = self.life.lock().map_err(|_| ApplicationDownloadError::Lock)?;
            if life.done {
                return Ok(true);
            }
            if life.cancelled {
                return Ok(false);
            }
            // From here a cancel only sets the flag (see `Life::cancel`).
            life.committing = true;
        }
        let _guard = CommitGuard(self);
        if let Ok(mut s) = self.status.lock() {
            *s = DownloadStatus::Validating;
        }
        self.set_transient(
            app_handle,
            ApplicationTransientStatus::Validating {
                version_id: self.meta.version.clone(),
            },
        );
        if !self.verify_staged()? {
            return Ok(false);
        }
        // Last point at which a cancel still stops the update.
        if self.life.lock().map(|l| l.cancelled).unwrap_or(true) {
            return Ok(false);
        }
        let baks = self.commit_update(app_handle).map_err(update_err)?;
        for b in &baks {
            info!("update of {}: kept the player's copy as {}", self.meta.id, b.display());
        }
        if let Ok(mut life) = self.life.lock() {
            life.done = true;
        }
        release(&self.meta.id);
        info!(
            "update of {} to {} rev {} applied in {}",
            self.meta.id,
            self.meta.version,
            self.to_revision,
            self.install_dir.display()
        );
        Ok(true)
    }

    fn dl_progress(&self) -> &Arc<ProgressObject> {
        &self.download_progress
    }

    fn disk_progress(&self) -> &Arc<ProgressObject> {
        &self.disk_progress
    }

    fn control_flag(&self) -> DownloadThreadControl {
        self.control_flag.clone()
    }

    fn status(&self) -> DownloadStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or(DownloadStatus::Error)
    }

    fn metadata(&self) -> DownloadableMetadata {
        self.meta.clone()
    }

    fn on_queued(&self, app_handle: &AppHandle) {
        let skip = self.life.lock().map(|l| l.cancelled || l.done).unwrap_or(true);
        if skip {
            return;
        }
        if let Ok(mut s) = self.status.lock() {
            *s = DownloadStatus::Queued;
        }
        self.set_transient(
            app_handle,
            ApplicationTransientStatus::Queued {
                version_id: self.meta.version.clone(),
            },
        );
    }

    fn on_error(&self, app_handle: &AppHandle, error: &ApplicationDownloadError) {
        if let Ok(mut s) = self.status.lock() {
            *s = DownloadStatus::Error;
        }
        error!("update of {} failed: {error}", self.meta.id);
        app_emit!(app_handle, "download_error", error.to_string());
        self.clear_transient();
        push_state(app_handle, &self.meta.id);
        // The staged chunks are kept: applying the same update again resumes.
        release(&self.meta.id);
    }

    async fn on_complete(&self, app_handle: &AppHandle) {
        app_emit!(app_handle, "update_library", ());
    }

    fn on_cancelled(&self, app_handle: &AppHandle) {
        let action = match self.life.lock() {
            Ok(mut life) => life.cancel(),
            Err(_) => OnCancel::Defer,
        };
        if action == OnCancel::AlreadyDone {
            return;
        }
        self.clear_transient();
        push_state(app_handle, &self.meta.id);
        if action == OnCancel::DiscardNow {
            self.discard();
            release(&self.meta.id);
            info!("update of {} cancelled; the install is unchanged", self.meta.id);
        } else {
            info!(
                "update of {} cancelled; it stops at the next safe point (a commit already under way completes)",
                self.meta.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    //! The file side of an update end to end, on a real temp folder: plan
    //! against the disk, resolve, "download" into staging, commit, finish.
    use super::*;
    use crate::downloads::update::baseline::{FsDisk, read_sidecar};
    use crate::downloads::update::plan::{self, PlanInput, RemoteFile, Resolution};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop-update-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(base: &Path, rel: &str, content: &str) {
        let p = base.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn read(base: &Path, rel: &str) -> Option<String> {
        std::fs::read_to_string(base.join(rel)).ok()
    }

    fn remote(path: &str, content: &str) -> RemoteFile {
        RemoteFile {
            path: path.into(),
            size: content.len() as u64,
            sha256: hex::encode(Sha256::digest(content.as_bytes())),
        }
    }

    /// A modpack at revision 1, installed with a baseline: the player then
    /// edited a pack config, added their own mod and has a save.
    fn installed_pack(name: &str) -> (PathBuf, Vec<RemoteFile>) {
        let install = scratch(name);
        let rev1 = vec![
            remote("mods/a.jar", "a1"),
            remote("mods/b.jar", "b1"),
            remote("config/pack.toml", "pack1"),
            remote("Game.exe", "exe"),
        ];
        for (f, c) in rev1.iter().zip(["a1", "b1", "pack1", "exe"]) {
            put(&install, &f.path, c);
        }
        let sidecar = baseline::fresh_sidecar(&install, "g", "v1", 1, &rev1);
        baseline::write_sidecar(&install, &sidecar).unwrap();
        put(&install, "config/pack.toml", "player edit");
        put(&install, "mods/mine.jar", "player mod");
        put(&install, "saves/world.dat", "progress");
        (install, rev1)
    }

    fn rev2() -> Vec<RemoteFile> {
        vec![
            remote("mods/a.jar", "a2 changed"),
            remote("mods/c.jar", "c new"),
            remote("config/pack.toml", "pack2"),
            remote("Game.exe", "exe"),
        ]
    }

    fn plan_for(install: &Path, target: &[RemoteFile]) -> plan::Plan {
        let base = read_sidecar(install).unwrap().unwrap().files;
        plan::plan(PlanInput {
            baseline: &base,
            target,
            disk: &FsDisk {
                root: install.to_path_buf(),
            },
            is_protected: &crate::downloads::download_agent::is_protected_user_data,
            mod_owned: &HashSet::new(),
            case_insensitive: false,
        })
        .unwrap()
    }

    /// What the downloader would leave in staging.
    fn stage_downloads(install: &Path, resolved: &Resolved, target: &[RemoteFile]) {
        let contents: HashMap<&str, &str> = [
            ("mods/a.jar", "a2 changed"),
            ("mods/c.jar", "c new"),
            ("config/pack.toml", "pack2"),
        ]
        .into_iter()
        .collect();
        for w in &resolved.writes {
            assert!(target.iter().any(|t| t.path == w.file.path));
            put(&commit::new_dir(install), &w.file.path, contents[w.file.path.as_str()]);
        }
    }

    fn input<'a>(install: &'a Path, resolved: &'a Resolved, chunks: &'a [String]) -> CommitInput<'a> {
        CommitInput {
            install,
            game_id: "g",
            from_version: "v1",
            to_version: "v2",
            to_revision: 1,
            platform: Platform::Windows,
            resolved,
            all_chunk_ids: chunks,
            game_version: None,
        }
    }

    #[test]
    fn an_update_applies_only_the_differences_and_keeps_player_files() {
        let (install, _) = installed_pack("apply");
        let target = rev2();
        let p = plan_for(&install, &target);
        assert_eq!(p.counts(), (1, 1, 1), "{p:#?}");
        assert_eq!(p.conflicts().len(), 1);
        let resolutions: HashMap<String, Resolution> =
            [("config/pack.toml".to_string(), Resolution::TakeUpdate)].into();
        let resolved = plan::resolve(p, &resolutions).unwrap();
        stage_downloads(&install, &resolved, &target);

        let chunks = vec!["c1".to_string()];
        let inp = input(&install, &resolved, &chunks);
        let journal = stage_meta(&inp).unwrap();
        let journal = commit_staged(&inp, journal).unwrap();
        commit::finish(&install, &journal).unwrap();

        assert_eq!(read(&install, "mods/a.jar").as_deref(), Some("a2 changed"));
        assert_eq!(read(&install, "mods/c.jar").as_deref(), Some("c new"));
        assert!(!install.join("mods/b.jar").exists());
        assert_eq!(read(&install, "config/pack.toml").as_deref(), Some("pack2"));
        assert_eq!(read(&install, "config/pack.toml.bak").as_deref(), Some("player edit"));
        assert_eq!(read(&install, "mods/mine.jar").as_deref(), Some("player mod"));
        assert_eq!(read(&install, "saves/world.dat").as_deref(), Some("progress"));
        assert_eq!(read(&install, "Game.exe").as_deref(), Some("exe"));
        assert!(!commit::update_dir(&install).exists());

        // The new baseline is the target, and the install now reads as fully
        // up to date: planning the same target again changes nothing.
        let side = read_sidecar(&install).unwrap().unwrap();
        assert_eq!(side.version_id, "v2");
        assert!(side.files.iter().all(|f| f.mtime.is_some()), "{side:#?}");
        let again = plan_for(&install, &target);
        assert!(again.writes.is_empty() && again.deletes.is_empty(), "{again:#?}");

        // And the ledger moved to the new version.
        let ledger = DropData::read(&install).unwrap();
        assert_eq!(ledger.game_version, "v2");
        assert_eq!(ledger.previously_installed_version, None);
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn keep_mine_stays_the_players_change_through_the_next_update() {
        let (install, _) = installed_pack("keep");
        let target = rev2();
        let resolutions: HashMap<String, Resolution> =
            [("config/pack.toml".to_string(), Resolution::KeepMine)].into();
        let resolved = plan::resolve(plan_for(&install, &target), &resolutions).unwrap();
        stage_downloads(&install, &resolved, &target);
        let chunks = vec![];
        let inp = input(&install, &resolved, &chunks);
        let journal = commit_staged(&inp, stage_meta(&inp).unwrap()).unwrap();
        commit::finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "config/pack.toml").as_deref(), Some("player edit"));

        assert_eq!(read_sidecar(&install).unwrap().unwrap().kept_mine, vec!["config/pack.toml".to_string()]);

        // Revision 3 changes only a jar. The kept config is untouched, and it
        // must still be recorded as kept, or a later repair would overwrite
        // the player's copy without a .bak.
        let mut rev3 = rev2();
        rev3[0] = remote("mods/a.jar", "a3");
        let resolved = plan::resolve(plan_for(&install, &rev3), &HashMap::new()).unwrap();
        put(&commit::new_dir(&install), "mods/a.jar", "a3");
        let chunks = vec![];
        let mut inp = input(&install, &resolved, &chunks);
        inp.from_version = "v2";
        inp.to_version = "v3";
        let journal = commit_staged(&inp, stage_meta(&inp).unwrap()).unwrap();
        commit::finish(&install, &journal).unwrap();
        assert_eq!(read(&install, "config/pack.toml").as_deref(), Some("player edit"));
        assert_eq!(read_sidecar(&install).unwrap().unwrap().kept_mine, vec!["config/pack.toml".to_string()]);

        // Revision 4 changes the config again: still the player's, so a
        // conflict, not a silent overwrite.
        let mut rev4 = rev3.clone();
        rev4[2] = remote("config/pack.toml", "pack4");
        let p = plan_for(&install, &rev4);
        assert_eq!(p.conflicts().len(), 1, "{p:#?}");
        // Taking the update this time ends the "kept" record.
        let resolutions: HashMap<String, Resolution> =
            [("config/pack.toml".to_string(), Resolution::TakeUpdate)].into();
        let resolved = plan::resolve(p, &resolutions).unwrap();
        put(&commit::new_dir(&install), "config/pack.toml", "pack4");
        let mut inp = input(&install, &resolved, &chunks);
        inp.from_version = "v3";
        inp.to_version = "v4";
        let journal = commit_staged(&inp, stage_meta(&inp).unwrap()).unwrap();
        commit::finish(&install, &journal).unwrap();
        assert!(read_sidecar(&install).unwrap().unwrap().kept_mine.is_empty());
        assert_eq!(read(&install, "config/pack.toml.bak").as_deref(), Some("player edit"));
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn a_file_changed_after_the_review_stops_the_commit_untouched() {
        let (install, _) = installed_pack("changed");
        let target = rev2();
        let resolutions: HashMap<String, Resolution> =
            [("config/pack.toml".to_string(), Resolution::TakeUpdate)].into();
        let resolved = plan::resolve(plan_for(&install, &target), &resolutions).unwrap();
        stage_downloads(&install, &resolved, &target);
        // The player edits a.jar while the update downloads.
        put(&install, "mods/a.jar", "edited meanwhile");
        let chunks = vec![];
        let inp = input(&install, &resolved, &chunks);
        let err = commit_staged(&inp, stage_meta(&inp).unwrap()).unwrap_err();
        assert!(matches!(err, UpdateError::ChangedSinceReview(ref p) if p == "mods/a.jar"), "{err}");
        assert_eq!(read(&install, "mods/a.jar").as_deref(), Some("edited meanwhile"));
        assert!(install.join("mods/b.jar").exists());
        assert!(!commit::journal_present(&install));
        assert_eq!(read_sidecar(&install).unwrap().unwrap().version_id, "v1");
        let _ = std::fs::remove_dir_all(&install);
    }

    #[test]
    fn unknown_target_hashes_become_real_hashes_or_stay_unknown_without_mtime() {
        let install = scratch("unknown-target");
        let written = RemoteFile {
            path: "data/new.pak".into(),
            size: 3,
            sha256: String::new(),
        };
        let kept = RemoteFile {
            path: "saves/default.sav".into(),
            size: 4,
            sha256: String::new(),
        };
        put(&commit::new_dir(&install), "data/new.pak", "abc");
        let resolved = Resolved {
            next_baseline: vec![
                (written.clone(), NextMtime::AfterWrite),
                (kept.clone(), NextMtime::Known(Some(42))),
            ],
            ..Default::default()
        };
        let chunks = vec![];
        let inp = input(&install, &resolved, &chunks);
        stage_meta(&inp).unwrap();
        let staged = read_sidecar(&commit::new_dir(&install)).unwrap().unwrap();
        let by_path: HashMap<&str, &BaselineFile> = staged.files.iter().map(|f| (f.path.as_str(), f)).collect();
        assert_eq!(
            by_path["data/new.pak"].sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(by_path["data/new.pak"].mtime.is_some());
        assert_eq!(by_path["saves/default.sav"].sha256, "");
        assert!(by_path["saves/default.sav"].mtime.is_none());
        let _ = std::fs::remove_dir_all(&install);
    }


    #[test]
    fn a_cancel_during_a_download_or_commit_is_deferred_to_it() {
        let mut life = Life {
            committing: true,
            ..Default::default()
        };
        assert_eq!(life.cancel(), OnCancel::Defer);
        assert!(life.cancelled);
        let mut life = Life {
            running: true,
            ..Default::default()
        };
        assert_eq!(life.cancel(), OnCancel::Defer);
        let mut life = Life::default();
        assert_eq!(life.cancel(), OnCancel::DiscardNow);
        let mut life = Life {
            done: true,
            committing: true,
            ..Default::default()
        };
        assert_eq!(life.cancel(), OnCancel::AlreadyDone);
        assert!(!life.cancelled);
    }

    #[test]
    fn a_cancelled_discard_cannot_run_inside_a_commit() {
        // What the agent's discard does on cancel, racing a commit that holds
        // the staging lock: it waits, and by then the journal is gone (the
        // commit finished), so nothing of the player's is in staging.
        let (install, _) = installed_pack("race");
        let target = rev2();
        let resolutions: HashMap<String, Resolution> =
            [("config/pack.toml".to_string(), Resolution::TakeUpdate)].into();
        let resolved = plan::resolve(plan_for(&install, &target), &resolutions).unwrap();
        stage_downloads(&install, &resolved, &target);
        let chunks = vec![];
        let inp = input(&install, &resolved, &chunks);
        let guard = commit::lock_staging();
        let dir = install.clone();
        let racer = std::thread::spawn(move || commit::discard_staging_locked(&dir));
        let journal = commit_staged(&inp, stage_meta(&inp).unwrap()).unwrap();
        commit::finish(&install, &journal).unwrap();
        drop(guard);
        racer.join().unwrap().unwrap();
        assert_eq!(read(&install, "config/pack.toml.bak").as_deref(), Some("player edit"));
        assert_eq!(read(&install, "mods/a.jar").as_deref(), Some("a2 changed"));
        let _ = std::fs::remove_dir_all(&install);
    }


    #[test]
    fn a_player_file_added_to_a_folder_the_update_turns_into_a_file_stops_the_commit() {
        let install = scratch("d2f-review");
        put(&install, "data/x", "x1");
        let rev1 = vec![remote("data/x", "x1")];
        baseline::write_sidecar(&install, &baseline::fresh_sidecar(&install, "g", "v1", 1, &rev1)).unwrap();
        let target = vec![remote("data", "now a file")];
        let resolved = plan::resolve(plan_for(&install, &target), &HashMap::new()).unwrap();
        put(&commit::new_dir(&install), "data", "now a file");
        // After the review, the player saves something into the folder.
        put(&install, "data/mine.sav", "progress");
        let chunks = vec![];
        let inp = input(&install, &resolved, &chunks);
        let err = commit_staged(&inp, stage_meta(&inp).unwrap()).unwrap_err();
        assert!(matches!(err, UpdateError::ChangedSinceReview(ref p) if p == "data"), "{err}");
        assert_eq!(read(&install, "data/mine.sav").as_deref(), Some("progress"));
        assert_eq!(read(&install, "data/x").as_deref(), Some("x1"));
        let _ = std::fs::remove_dir_all(&install);
    }
}
