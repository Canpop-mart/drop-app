//! Download agent for a mod.
//!
//! A mod is a `Game` with `type = Mod` and a `parentGameId`. Its files overlay
//! directly into the parent game's install directory. This agent mirrors
//! `GameDownloadAgent` (same manifest fetch, same chunk crypto via
//! `download_game_chunk`, same validation) with exactly three differences:
//!
//!  1. `new()` receives the parent's already-final install dir and writes its
//!     resume ledger to `<install dir>/.mods/<mod id>.moddata` (not the
//!     parent's `.dropdata`, which belongs to the base game).
//!  2. `run()` has no stale-file sweep. The game agent's removal of files
//!     dropped since the previous version works from the BASE game's
//!     manifests, which say nothing about a mod; a mod update instead lets go
//!     of its own old files on completion, from its ledger (point 3).
//!  3. When it starts (not when it is queued) it re-reads the ledger from
//!     disk, then records the manifest's file list in `.moddata`, drops a
//!     `.pending` marker, and moves aside every base-game file it is about to
//!     overwrite (`mod_data::back_up_originals`). So a cancelled or failed
//!     install can be removed as cleanly as a finished one, and uninstall puts
//!     the base game's own files back. Once every chunk is on disk it cuts
//!     rewritten files back to their manifest size. On completion it lets go
//!     of files the previous version had but this one doesn't (an update),
//!     and removes the marker: only then is the mod installed.

use async_trait::async_trait;
use database::models::data::{InstalledGameType, UserConfiguration};
use database::{
    ApplicationTransientStatus, DownloadableMetadata, GameDownloadStatus, borrow_db_mut_checked,
};
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
use log::{debug, error, info, warn};
use remote::auth::generate_authorization_header;
use remote::error::RemoteAccessError;
use remote::requests::generate_url;
use remote::utils::DROP_CLIENT_ASYNC;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::mpsc::Sender;
use utils::{app_emit, lock, send};

use crate::downloads::download_agent::{DownloadInformation, RETRY_COUNT, is_disk_full};
use crate::downloads::utils::get_disk_available;
use crate::library::{on_game_complete, push_game_update};
use crate::state::GameStatusManager;

use super::download_logic::{
    download_game_chunk, files_written_by, manifest_file_sizes, trim_stale_tails,
};
use super::drop_data::DropData;
use super::mod_data::{
    ModData, MODS_DIR, back_up_originals, moddata_path, overlay_rel, pending_marker_path,
    release_recorded, reload_for_run, settle_unlisted_originals, undo_backups,
};

/// Emitted with the parent game's id whenever a mod's on-disk state under it
/// changes (download started, finished, failed, cancelled, uninstalled). The
/// game page's Mods tab re-reads the ledgers on it. Deliberately not
/// `update_library`, which re-fetches every game in the library.
pub fn mods_changed_event(parent_game_id: &str) -> String {
    format!("update_mods/{parent_game_id}")
}

pub struct ModDownloadAgent {
    pub metadata: DownloadableMetadata,
    pub parent_game_id: String,
    pub configuration: UserConfiguration,
    pub control_flag: DownloadThreadControl,
    pub dl_info: Mutex<Option<DownloadInformation>>,
    pub download_progress: Arc<ProgressObject>,
    pub disk_progress: Arc<ProgressObject>,
    depot_manager: Arc<DepotManager>,
    sender: Sender<DownloadManagerSignal>,
    pub moddata: ModData,
    /// The parent game's install dir (where `.mods/` lives). The overlay
    /// folder `moddata.base_path` may be a subfolder of it.
    install_dir: PathBuf,
    status: Mutex<DownloadStatus>,
}

impl Debug for ModDownloadAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModDownloadAgent").finish()
    }
}

impl ModDownloadAgent {
    /// `install_dir` is the parent game's FINAL install directory. Files overlay
    /// into `install_dir/mod_install_dir` (the mod version's declared location;
    /// empty = the install root). The ledger lives at `install_dir/.mods/` (top
    /// level) so listing/uninstall find it with only the install dir.
    /// `launch_override`, if set, is recorded so the launcher can swap the
    /// game's exe while this mod is installed.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        metadata: DownloadableMetadata,
        parent_game_id: String,
        install_dir: PathBuf,
        mod_install_dir: String,
        launch_override: Option<String>,
        sender: Sender<DownloadManagerSignal>,
        depot_manager: Arc<DepotManager>,
        configuration: UserConfiguration,
    ) -> Result<Self, ApplicationDownloadError> {
        // Don't run by default
        let control_flag = DownloadThreadControl::new(DownloadThreadControlFlag::Stop);

        // Files overlay into install_dir/mod_install_dir; the ledger stays at
        // install_dir/.mods/ regardless.
        let overlay_dir = install_dir.join(&mod_install_dir);
        info!(
            "mod {} overlaying into {} (install dir {})",
            metadata.id,
            overlay_dir.display(),
            install_dir.display()
        );

        create_dir_all(install_dir.join(MODS_DIR))?;
        create_dir_all(&overlay_dir)?;

        let meta_path = moddata_path(&install_dir, &metadata.id);

        let moddata = ModData::generate(
            metadata.id.clone(),
            metadata.version.clone(),
            metadata.target_platform,
            parent_game_id.clone(),
            launch_override,
            overlay_dir,
            meta_path,
        );
        let ledger_install_dir = install_dir.clone();

        let result = Self {
            metadata,
            parent_game_id,
            control_flag,
            dl_info: Mutex::new(None),
            download_progress: Arc::new(ProgressObject::new(
                0,
                0,
                sender.clone(),
                ProgressType::Download,
            )),
            disk_progress: Arc::new(ProgressObject::new(0, 0, sender.clone(), ProgressType::Disk)),
            sender,
            moddata,
            install_dir: ledger_install_dir,
            status: Mutex::new(DownloadStatus::Queued),
            depot_manager,
            configuration,
        };

        result.ensure_manifest_exists().await?;

        let required_space = lock!(result.dl_info).as_ref().unwrap().install_size;
        let available_space = get_disk_available(install_dir)? as u64;
        if required_space > available_space {
            return Err(ApplicationDownloadError::DiskFull(
                required_space,
                available_space,
            ));
        }

        Ok(result)
    }

    pub async fn download(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        self.setup_download(app_handle)?;
        let timer = Instant::now();
        info!("beginning mod download for {}...", self.metadata.id);
        let res = self.run().await;
        debug!(
            "{} took {}ms to download",
            self.metadata.id,
            timer.elapsed().as_millis()
        );
        res
    }

    fn setup_download(&self, app_handle: &AppHandle) -> Result<(), ApplicationDownloadError> {
        let mut db_lock = borrow_db_mut_checked();
        let status = ApplicationTransientStatus::Downloading {
            version_id: self.metadata.version.clone(),
        };
        db_lock
            .applications
            .transient_statuses
            .insert(self.metadata.clone(), status.clone());
        drop(db_lock);
        push_game_update(app_handle, &self.metadata.id, None, (None, Some(status)));
        app_emit!(app_handle, &mods_changed_event(&self.parent_game_id), ());

        if lock!(self.dl_info).is_none() {
            return Err(ApplicationDownloadError::NotInitialized);
        }
        Ok(())
    }

    pub async fn ensure_manifest_exists(&self) -> Result<(), ApplicationDownloadError> {
        if lock!(self.dl_info).is_some() {
            return Ok(());
        }
        self.download_manifest().await
    }

    async fn download_manifest(&self) -> Result<(), ApplicationDownloadError> {
        let manifest_download = self
            .fetch_manifest(self.moddata.previously_installed_version.as_deref())
            .await?;

        if let Ok(mut manifest) = self.dl_info.lock() {
            *manifest = Some(manifest_download);
            return Ok(());
        }
        Err(ApplicationDownloadError::Lock)
    }

    /// The server's download manifest for this version. With `previous`,
    /// chunks already present from that version are left out.
    async fn fetch_manifest(
        &self,
        previous: Option<&str>,
    ) -> Result<DownloadInformation, ApplicationDownloadError> {
        let client = DROP_CLIENT_ASYNC.clone();
        let url = generate_url(
            &["/api/v1/client/game/manifest"],
            &[
                ("id", &self.metadata.id),
                ("version", &self.metadata.version),
                ("previous", previous.unwrap_or("")),
            ],
        )
        .map_err(ApplicationDownloadError::Communication)?;

        let response = client
            .get(url)
            .header("Authorization", generate_authorization_header()?)
            .send()
            .await
            .map_err(|e| ApplicationDownloadError::Communication(e.into()))?;

        if response.status() != 200 {
            return Err(ApplicationDownloadError::Communication(
                RemoteAccessError::ManifestDownloadFailed(
                    response.status(),
                    response
                        .text()
                        .await
                        .unwrap_or_else(|e| format!("<failed to read error body: {e}>")),
                ),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| ApplicationDownloadError::Communication(e.into()))
    }

    /// Cut every file this download rewrote back to its manifest size (see
    /// `trim_stale_tails`). The sizes need the full manifest: the delta one
    /// used for an update leaves out chunks of unchanged files.
    async fn trim_rewritten_files(&self) -> Result<(), ApplicationDownloadError> {
        let (written, delta_sizes) = {
            let dl_info = lock!(self.dl_info);
            let info = dl_info.as_ref().ok_or(ApplicationDownloadError::NotInitialized)?;
            (files_written_by(info), manifest_file_sizes(info))
        };
        let sizes = if self.moddata.previously_installed_version.is_some() {
            manifest_file_sizes(&self.fetch_manifest(None).await?)
        } else {
            delta_sizes
        };
        let trimmed = trim_stale_tails(&self.moddata.base_path, &written, &sizes)
            .map_err(|e| ApplicationDownloadError::IoError(Arc::new(e)))?;
        if trimmed > 0 {
            info!("mod {}: cut {trimmed} rewritten file(s) back to size", self.metadata.id);
        }
        Ok(())
    }

    fn setup_progress(&self) {
        let dl_info = lock!(self.dl_info);
        let dl_info = dl_info.as_ref().unwrap();

        let total_chunks = dl_info.manifests.iter().map(|v| v.1.chunks.len()).sum::<usize>();

        self.download_progress
            .set_max(dl_info.download_size.try_into().unwrap());
        self.download_progress.set_size(total_chunks);
        self.download_progress.reset();

        self.disk_progress
            .set_max(dl_info.install_size.try_into().unwrap());
        self.disk_progress.set_size(total_chunks);
        self.disk_progress.reset();
    }

    /// Same download loop as `GameDownloadAgent::run`, MINUS the reconcile
    /// sweep. A mod only ever adds files to the parent's dir; the sweep would
    /// delete every base-game file (none of which are in the mod's manifest).
    async fn run(&self) -> Result<bool, ApplicationDownloadError> {
        self.depot_manager.sync_depots().await?;
        self.setup_progress();
        self.prepare_install_dir()?;

        let manifests_chunks: Vec<(String, HashMap<String, ChunkData>, [u8; 16])> = {
            let dl_info = lock!(self.dl_info);
            dl_info
                .as_ref()
                .unwrap()
                .manifests
                .iter()
                .map(|v| (v.0.clone(), v.1.chunks.clone(), v.1.key))
                .collect()
        };
        let file_list = {
            let dl_info = lock!(self.dl_info);
            dl_info.as_ref().unwrap().file_list.clone()
        };
        let mut completed_chunks = {
            let completed_chunks = lock!(self.moddata.contexts);
            completed_chunks.clone()
        };
        info!("mod started with {} existing chunks", completed_chunks.len());
        let chunk_len = manifests_chunks.iter().map(|v| v.1.len()).sum::<usize>();
        let mut max_download_threads =
            database::borrow_db_checked().settings.max_download_threads;
        if max_download_threads == 0 {
            max_download_threads = 1;
        }

        let file_list = &file_list;
        let base_path = &self.moddata.base_path;

        let local_completed_chunks = completed_chunks.clone();
        let mut chunk_completions = FuturesUnordered::new();
        let mut outputs = Vec::new();

        let moddata = &self.moddata;
        let mut handle_output =
            |value: Result<Option<String>, ApplicationDownloadError>| match value {
                Ok(value) => {
                    if let Some(chunk_id) = value {
                        moddata.set_context(chunk_id.clone(), true);
                        moddata.write();
                        outputs.push(chunk_id);
                    }
                    Ok(())
                }
                Err(err) => Err(err),
            };

        let mut index = 0;
        for (version_id, chunks, key) in manifests_chunks.into_iter() {
            let version_id = &version_id;
            for (chunk_id, chunk_data) in chunks.into_iter() {
                let download_progress_handle = ProgressHandle::new(
                    self.download_progress.get(index),
                    self.download_progress.clone(),
                );
                let disk_progress_handle =
                    ProgressHandle::new(self.disk_progress.get(index), self.disk_progress.clone());
                index += 1;

                let chunk_length = chunk_data.files.iter().map(|v| v.length).sum();

                if *local_completed_chunks.get(&chunk_id).unwrap_or(&false) {
                    download_progress_handle.skip(chunk_length);
                    continue;
                }

                let (depot, permit) = match self
                    .depot_manager
                    .next_depot(&self.metadata.id, &self.metadata.version)
                {
                    Ok(v) => v,
                    Err(err) => return Err(err.into()),
                };

                let local_version_id = version_id.clone();
                while chunk_completions.len() >= max_download_threads {
                    handle_output(
                        chunk_completions
                            .next()
                            .await
                            .expect("max download threads is zero?"),
                    )?;
                }
                chunk_completions.push(async move {
                    for i in 0..RETRY_COUNT {
                        match download_game_chunk(
                            &self.metadata.id,
                            &local_version_id,
                            &chunk_id,
                            &depot,
                            &key,
                            &chunk_data,
                            file_list,
                            base_path,
                            &self.control_flag,
                            &download_progress_handle,
                            &disk_progress_handle,
                        )
                        .await
                        {
                            Ok(true) => {
                                drop(permit);
                                return Ok(Some(chunk_id.clone()));
                            }
                            Ok(false) => return Ok(None),
                            Err(e) => {
                                warn!("got error for chunk id {}: {e:?}", chunk_id);
                                let retry = !is_disk_full(&e);
                                if i == RETRY_COUNT - 1 || !retry {
                                    warn!(
                                        "retry logic failed after {} attempts, not re-attempting.",
                                        i + 1
                                    );
                                    return Err(e);
                                }
                                let backoff = Duration::from_secs(1 << i);
                                warn!(
                                    "retrying chunk {} in {:?} (attempt {}/{})",
                                    chunk_id,
                                    backoff,
                                    i + 2,
                                    RETRY_COUNT
                                );
                                tokio::time::sleep(backoff).await;
                            }
                        }
                    }
                    Ok(None)
                });
            }
        }

        let mut errors: Vec<ApplicationDownloadError> = Vec::new();
        while let Some(value) = chunk_completions.next().await {
            if let Err(e) = handle_output(value) {
                errors.push(e);
            }
        }

        for completed_chunk in outputs {
            completed_chunks.insert(completed_chunk, true);
        }

        if let Some(first) = errors.into_iter().next() {
            return Err(first);
        }

        let drop_data_chunks = completed_chunks
            .iter()
            .map(|v| (v.0.to_string(), *v.1))
            .collect::<Vec<(String, bool)>>();

        self.moddata.set_contexts(&drop_data_chunks);
        self.moddata.write();

        info!("mod completed {} chunks", drop_data_chunks.len());

        if completed_chunks.len() != chunk_len {
            info!(
                "mod download agent for {} exited without completing ({}/{})",
                self.metadata.id,
                completed_chunks.len(),
                chunk_len,
            );
            return Ok(false);
        }
        self.trim_rewritten_files().await?;
        Ok(true)
    }

    fn setup_validate(&self, app_handle: &AppHandle) {
        let status = ApplicationTransientStatus::Validating {
            version_id: self.metadata.version.clone(),
        };
        let mut db_lock = borrow_db_mut_checked();
        db_lock
            .applications
            .transient_statuses
            .insert(self.metadata.clone(), status.clone());
        drop(db_lock);
        push_game_update(app_handle, &self.metadata.id, None, (None, Some(status)));
    }

    /// Presence/size/SHA-256 validation of the mod's files against its manifest.
    /// Reuses the shared `validate_install`, which NEVER deletes files. On a
    /// handful of bad chunks it invalidates just those (targeted repair) and
    /// returns Ok(false) to drive the manager's repair loop; a systemic failure
    /// aborts. Unlike the game agent it does NOT set PartiallyInstalled — a mod
    /// must never take a game-shaped status, since that would let the generic
    /// resume/uninstall paths operate on the parent's install dir.
    pub fn validate(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        self.setup_validate(app_handle);

        let install_dir = self.moddata.base_path.clone();
        info!(
            "running post-install validation for mod {} at {}",
            self.metadata.id,
            install_dir.display()
        );

        let result = {
            let dl_info = lock!(self.dl_info);
            let dl_info = dl_info
                .as_ref()
                .ok_or(ApplicationDownloadError::NotInitialized)?;
            crate::downloads::validate::validate_install(dl_info, &install_dir)
        };

        match result {
            crate::downloads::validate::ValidationResult::Valid => {
                info!("mod validation succeeded for {}", self.metadata.id);
                Ok(true)
            }
            crate::downloads::validate::ValidationResult::Incomplete {
                missing,
                mismatched,
            } => {
                let summary = crate::downloads::validate::ValidationResult::Incomplete {
                    missing: missing.clone(),
                    mismatched: mismatched.clone(),
                }
                .describe();

                let total_chunks = {
                    let dl_info = lock!(self.dl_info);
                    dl_info
                        .as_ref()
                        .map(|d| d.manifests.values().map(|m| m.chunks.len()).sum())
                        .unwrap_or(0usize)
                };
                let invalidated = self.invalidate_failed_chunks(&missing, &mismatched);
                self.moddata.write();

                const REPAIRABLE_CHUNK_FLOOR: usize = 64;
                let repairable = invalidated > 0
                    && (invalidated <= REPAIRABLE_CHUNK_FLOOR
                        || invalidated.saturating_mul(4) <= total_chunks);
                if repairable {
                    warn!(
                        "mod validation failed for {}: {} missing, {} mismatched — invalidated {}/{} chunk(s); requesting targeted re-download",
                        self.metadata.id,
                        missing.len(),
                        mismatched.len(),
                        invalidated,
                        total_chunks
                    );
                    Ok(false)
                } else {
                    error!(
                        "mod validation failed for {}: {} missing, {} mismatched — {}/{} chunk(s) bad, too broad to repair; aborting",
                        self.metadata.id,
                        missing.len(),
                        mismatched.len(),
                        invalidated,
                        total_chunks
                    );
                    Err(ApplicationDownloadError::ValidationFailed(summary))
                }
            }
        }
    }

    fn invalidate_failed_chunks(
        &self,
        missing: &[crate::downloads::validate::MissingFile],
        mismatched: &[crate::downloads::validate::MismatchedChunk],
    ) -> usize {
        use std::collections::HashSet;
        let mut to_clear: HashSet<String> = HashSet::new();

        for chunk in mismatched {
            to_clear.insert(chunk.chunk_id.clone());
        }

        if !missing.is_empty() {
            let missing_files: HashSet<&str> =
                missing.iter().map(|m| m.filename.as_str()).collect();
            let dl_info = lock!(self.dl_info);
            if let Some(dl_info) = dl_info.as_ref() {
                for manifest in dl_info.manifests.values() {
                    for (chunk_id, chunk_data) in &manifest.chunks {
                        if chunk_data
                            .files
                            .iter()
                            .any(|f| missing_files.contains(f.filename.as_str()))
                        {
                            to_clear.insert(chunk_id.clone());
                        }
                    }
                }
            }
        }

        for chunk_id in &to_clear {
            self.moddata.set_context(chunk_id.clone(), false);
        }
        to_clear.len()
    }

    /// The files this run's chunks write (POSIX paths relative to the overlay
    /// folder). For an update this is only what changed since the installed
    /// version (the delta manifest); files the mod ships unchanged are not
    /// downloaded, so they must not be moved aside or claimed either.
    fn files_this_run_writes(&self) -> Result<Vec<String>, ApplicationDownloadError> {
        let dl_info = lock!(self.dl_info);
        let info = dl_info.as_ref().ok_or(ApplicationDownloadError::NotInitialized)?;
        Ok(run_writes(info))
    }

    /// The manifest's file list (POSIX paths relative to the overlay folder).
    fn manifest_files(&self) -> Result<Vec<String>, ApplicationDownloadError> {
        let dl_info = lock!(self.dl_info);
        let info = dl_info.as_ref().ok_or(ApplicationDownloadError::NotInitialized)?;
        let mut files: Vec<String> = info.file_list.keys().cloned().collect();
        files.sort();
        Ok(files)
    }

    /// The parent game must still be installed where it was when this
    /// download was queued. A base-game download or uninstall can run while
    /// a mod waits in the queue, and the mod must not write into a folder the
    /// game has left or is half-way through rewriting.
    ///
    /// A failed or paused in-place update leaves the game's status Installed
    /// at the old version, so the game's own `.dropdata` is checked too: it
    /// names the version the folder is being brought to.
    fn check_parent_unchanged(&self) -> Result<(), String> {
        let installed_version = {
            let db = database::borrow_db_checked();
            match db.applications.game_statuses.get(&self.parent_game_id) {
                Some(GameDownloadStatus::Installed {
                    install_dir,
                    install_type,
                    version_id,
                    ..
                }) if !matches!(install_type, InstalledGameType::PartiallyInstalled { .. })
                    && std::path::Path::new(install_dir) == self.install_dir.as_path() =>
                {
                    version_id.clone()
                }
                _ => {
                    return Err(format!(
                        "the game this mod belongs to is no longer fully installed at {}",
                        self.install_dir.display()
                    ));
                }
            }
        };
        parent_folder_settled(&self.install_dir, &installed_version)
    }

    /// Everything that has to be on disk before the first chunk is written:
    ///  0. the parent still installed in place, `moddata` re-read from disk,
    ///     and any base-game originals left over from a crashed earlier
    ///     attempt put back (`settle_unlisted_originals`);
    ///  1. the `.pending` marker, so this is never mistaken for an installed mod;
    ///  2. base-game files this run will overwrite moved to `originals/`;
    ///  3. the ledger, listing the files this run writes on top of any it
    ///     already had.
    ///
    /// The backups come before the ledger on purpose: a file the ledger lists
    /// counts as the mod's own and is never backed up, so listing first and
    /// crashing mid-backup would lose the originals not yet moved. The cost is
    /// that a crash between 2 and 3 leaves originals the ledger doesn't list;
    /// step 0 of the next attempt (or `remove_mod`) puts them back.
    ///
    /// Any failure aborts the download before a single file is overwritten,
    /// and puts back what step 2 moved.
    fn prepare_install_dir(&self) -> Result<(), ApplicationDownloadError> {
        let io_err = |what: String| ApplicationDownloadError::IoError(Arc::new(std::io::Error::other(what)));
        let id = &self.metadata.id;
        let incoming = self.files_this_run_writes()?;

        self.check_parent_unchanged()
            .and_then(|()| reload_for_run(&self.install_dir, &self.moddata))
            .map_err(|why| io_err(format!("cannot install mod {id}: {why}")))?;

        let prefix = overlay_rel(&self.install_dir, &self.moddata)
            .map_err(|why| io_err(format!("cannot install mod {id}: {why}")))?;
        let listed: std::collections::HashSet<String> = self
            .moddata
            .get_installed_files()
            .iter()
            .map(|f| if prefix.is_empty() { f.to_lowercase() } else { format!("{prefix}/{f}").to_lowercase() })
            .collect();
        let leftovers = settle_unlisted_originals(&self.install_dir, id, &listed);
        if let Some((file, why)) = leftovers.failed_originals.first() {
            return Err(io_err(format!(
                "cannot install mod {id}: a game file it set aside earlier ({file}) could not be put back: {why}"
            )));
        }
        if leftovers.restored + leftovers.kept_for_other_mods > 0 {
            info!(
                "mod {id}: put back {} game file(s) left over from an earlier attempt",
                leftovers.restored + leftovers.kept_for_other_mods
            );
        }

        let marker = pending_marker_path(&self.install_dir, id);
        std::fs::write(&marker, self.metadata.version.as_bytes()).map_err(|e| {
            io_err(format!("could not mark mod {id} as installing ({}): {e}", marker.display()))
        })?;

        let moved = back_up_originals(&self.install_dir, &self.moddata, &incoming).map_err(|why| {
            io_err(format!("could not set aside the game files mod {id} replaces: {why}"))
        })?;

        let previous = self.moddata.get_installed_files();
        let known: std::collections::HashSet<String> = previous.iter().cloned().collect();
        let mut files = previous.clone();
        files.extend(incoming.into_iter().filter(|f| !known.contains(f)));
        self.moddata.set_installed_files(files);
        if let Err(e) = self.moddata.try_write() {
            // Nothing has been overwritten yet: put the game's files back and
            // forget this run, so no game file sits in originals/ unlisted.
            self.moddata.set_installed_files(previous);
            for (file, why) in undo_backups(&self.install_dir, id, &moved) {
                error!("mod {id}: could not put back game file {file}: {why}");
            }
            return Err(io_err(format!("could not record the files of mod {id}: {e}")));
        }
        Ok(())
    }

    /// Make the finished download the installed mod: hand back files the
    /// previous version had but this one doesn't (restoring any base-game
    /// original, see `release_recorded`), shrink the ledger to this version's
    /// files, then remove the `.pending` marker. A file that could not be
    /// handed back stays listed, so uninstalling later still covers it. An
    /// original that could not be put back fails the install.
    fn finish_install(&self) -> Result<(), String> {
        let current = self.manifest_files().map_err(|e| e.to_string())?;
        let current_set: std::collections::HashSet<&String> = current.iter().collect();
        let stale: Vec<String> = self
            .moddata
            .get_installed_files()
            .into_iter()
            .filter(|f| !current_set.contains(f))
            .collect();

        let outcome = release_recorded(&self.install_dir, &self.moddata, &stale, false)
            .map_err(|why| format!("could not remove the old version of mod {}: {why}", self.metadata.id))?;
        if !stale.is_empty() {
            info!(
                "mod {} update: {} old file(s) removed, {} original(s) restored, {} left to other mods",
                self.metadata.id, outcome.removed, outcome.restored, outcome.kept_for_other_mods
            );
        }
        for (file, why) in &outcome.failed {
            warn!("mod {}: could not remove old file {file}: {why}", self.metadata.id);
        }
        // A game file the old version had replaced and that could not be put
        // back: the install stays unfinished (Resume or Remove), because
        // another mod installed meanwhile would take the mod's copy now in
        // its place for the game's original.
        if let Some((file, why)) = outcome.failed_originals.first() {
            return Err(format!(
                "could not put back the game's file {file} that the old version of mod {} replaced: {why}",
                self.metadata.id
            ));
        }

        let marker = pending_marker_path(&self.install_dir, &self.metadata.id);
        match std::fs::remove_file(&marker) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!(
                "could not mark mod {} as installed ({}): {e}",
                self.metadata.id,
                marker.display()
            )),
        }
    }
}

#[async_trait]
impl Downloadable for ModDownloadAgent {
    async fn download(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        *lock!(self.status) = DownloadStatus::Downloading;
        self.download(app_handle).await
    }

    fn validate(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        *lock!(self.status) = DownloadStatus::Validating;
        self.validate(app_handle)
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

    fn metadata(&self) -> DownloadableMetadata {
        self.metadata.clone()
    }

    fn on_queued(&self, app_handle: &AppHandle) {
        *lock!(self.status) = DownloadStatus::Queued;
        let mut db_lock = borrow_db_mut_checked();
        let status = ApplicationTransientStatus::Queued {
            version_id: self.metadata.version.clone(),
        };
        db_lock
            .applications
            .transient_statuses
            .insert(self.metadata.clone(), status.clone());
        drop(db_lock);
        push_game_update(app_handle, &self.metadata.id, None, (None, Some(status)));
    }

    fn on_error(&self, app_handle: &AppHandle, error: &ApplicationDownloadError) {
        *lock!(self.status) = DownloadStatus::Error;
        app_emit!(app_handle, "download_error", error.to_string());
        error!("error while managing mod download: {error:?}");

        let mut handle = borrow_db_mut_checked();
        handle
            .applications
            .transient_statuses
            .remove(&self.metadata);
        push_game_update(
            app_handle,
            &self.metadata.id,
            None,
            GameStatusManager::fetch_state(&self.metadata.id, &handle),
        );
        drop(handle);
        app_emit!(app_handle, &mods_changed_event(&self.parent_game_id), ());
    }

    async fn on_complete(&self, app_handle: &AppHandle) {
        // Settle the files on disk BEFORE recording the mod as installed. Until
        // the marker is gone the Mods tab offers Resume / Remove, never a
        // half-installed "Installed".
        if let Err(why) = self.finish_install() {
            error!("could not finish installing mod {}: {why}", self.metadata.id);
            send!(
                self.sender,
                DownloadManagerSignal::Error(ApplicationDownloadError::IoError(Arc::new(
                    std::io::Error::other(why)
                )))
            );
            return;
        }

        // Reuse the shared completion path: it fetches the mod's version,
        // records installed_game_version + game_statuses (keyed by the mod's own
        // id, download_type=Mod), clears the transient status, and emits
        // update_game/<modid> + update_library. The install_dir it stores is the
        // parent dir; the generic uninstall/resume commands are guarded on
        // download_type==Mod so they never act on it.
        match on_game_complete(
            &self.metadata,
            self.configuration.clone(),
            self.moddata.base_path.to_string_lossy().to_string(),
            app_handle,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("could not mark mod as complete: {e}");
                send!(
                    self.sender,
                    DownloadManagerSignal::Error(ApplicationDownloadError::DownloadError(e))
                );
            }
        }
        app_emit!(app_handle, &mods_changed_event(&self.parent_game_id), ());
    }

    fn on_cancelled(&self, app_handle: &AppHandle) {
        // Mod-safe cancel: the ledger is already written incrementally, so just
        // clear the transient status and refresh the UI. Deliberately does NOT
        // call set_partially_installed — a mod must never hold a game-shaped
        // "PartiallyInstalled" status, or the generic resume path would rebuild
        // a GameDownloadAgent over the parent dir and sweep the base game. The
        // Mods tab offers Resume (download_mod again, which picks up the
        // persisted `.moddata` ledger) or Remove (uninstall_mod).
        //
        // Nothing is written here. Every finished chunk is already on disk
        // (run() writes the ledger per chunk), and `moddata` may be out of
        // date: it is what the ledger said when this download was queued, or
        // when it last started, and a base-game download may have taken
        // files back from the mod since (for instance after the queue was
        // rearranged). Writing it back would list the game's file as the
        // mod's again.
        let mut handle = borrow_db_mut_checked();
        handle
            .applications
            .transient_statuses
            .remove(&self.metadata);
        push_game_update(
            app_handle,
            &self.metadata.id,
            None,
            GameStatusManager::fetch_state(&self.metadata.id, &handle),
        );
        drop(handle);
        app_emit!(app_handle, &mods_changed_event(&self.parent_game_id), ());
    }

    fn status(&self) -> DownloadStatus {
        lock!(self.status).clone()
    }
}

/// Sorted list of the files a download of `info` writes (see
/// `files_written_by`).
fn run_writes(info: &DownloadInformation) -> Vec<String> {
    let mut files: Vec<String> = files_written_by(info).into_iter().collect();
    files.sort();
    files
}

/// Refuse when the game's folder is part-way through a download of another
/// version: its `.dropdata` names a version other than the installed one (a
/// failed, paused or cancelled in-place update). Mods must not set aside or
/// overwrite files that the game's own download is about to rewrite. No
/// `.dropdata` (an older install) says nothing either way. Only finishing
/// that download clears this: cancelling it rewrites `.dropdata` with the
/// same version and leaves the game partially installed.
fn parent_folder_settled(install_dir: &std::path::Path, installed_version: &str) -> Result<(), String> {
    match DropData::read(install_dir) {
        Ok(d) if d.game_version == installed_version => Ok(()),
        Ok(d) => Err(format!(
            "the game has an unfinished download of another version ({}); let that download finish first",
            d.game_version
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("the game's download record cannot be read: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::mod_data::{originals_dir, remove_mod};
    use database::platform::Platform;
    use droplet_rs::manifest::{FileEntry, Manifest};
    use std::path::Path;

    fn chunk(files: &[&str]) -> ChunkData {
        ChunkData {
            files: files
                .iter()
                .map(|f| FileEntry {
                    filename: f.to_string(),
                    start: 0,
                    length: 1,
                    permissions: 0,
                })
                .collect(),
            checksum: String::new(),
            iv: [0; 16],
        }
    }

    fn put(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    /// Mod v1 ships G and H. A base-game update rewrites G and takes it back.
    /// Mod v2 ships G unchanged and a new H, so the delta manifest only has
    /// H's chunk. The run must not move the game's G aside: nothing would
    /// write it back.
    #[test]
    fn a_delta_update_only_sets_aside_files_it_downloads() {
        let dir = std::env::temp_dir().join(format!("drop-mod-agent-delta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(MODS_DIR)).unwrap();
        let v1 = ModData::new(
            "m".to_string(),
            "v1".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            dir.clone(),
            moddata_path(&dir, "m"),
            None,
        );
        v1.set_installed_files(vec!["G.dll".to_string(), "H.dll".to_string()]);
        v1.write();
        put(&dir.join("G.dll"), "game v2");
        put(&dir.join("H.dll"), "mod v1");
        crate::downloads::mod_data::hand_files_to_base_game(&dir, &["G.dll".to_string()]).unwrap();

        // Delta from v1: G still maps to v1 and its chunk is left out.
        let mut file_list = HashMap::new();
        file_list.insert("G.dll".to_string(), "v1".to_string());
        file_list.insert("H.dll".to_string(), "v2".to_string());
        let mut chunks = HashMap::new();
        chunks.insert("h".to_string(), chunk(&["H.dll"]));
        let mut manifests = HashMap::new();
        manifests.insert(
            "v2".to_string(),
            Manifest { version: "v2".to_string(), chunks, size: 0, key: [0; 16] },
        );
        let delta = DownloadInformation { file_list, manifests, install_size: 0, download_size: 0, revision: None };
        let incoming = run_writes(&delta);
        assert_eq!(incoming, vec!["H.dll".to_string()]);

        let v2 = ModData::generate(
            "m".to_string(),
            "v2".to_string(),
            Platform::Windows,
            "parent".to_string(),
            None,
            dir.clone(),
            moddata_path(&dir, "m"),
        );
        reload_for_run(&dir, &v2).unwrap();
        assert!(back_up_originals(&dir, &v2, &incoming).unwrap().is_empty());
        assert_eq!(std::fs::read_to_string(dir.join("G.dll")).unwrap(), "game v2");
        assert!(!originals_dir(&dir, "m").join("G.dll").exists());
        // And uninstalling leaves the game's G alone.
        v2.set_installed_files(vec!["H.dll".to_string()]);
        v2.write();
        remove_mod(&dir, "m").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("G.dll")).unwrap(), "game v2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_half_updated_game_folder_is_refused() {
        let dir = std::env::temp_dir().join(format!("drop-mod-agent-settled-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(parent_folder_settled(&dir, "v1").is_ok(), "no .dropdata");
        let d = DropData::new(
            "game".to_string(),
            "v2".to_string(),
            Platform::Windows,
            dir.clone(),
            None,
        );
        d.write();
        assert!(parent_folder_settled(&dir, "v1").unwrap_err().contains("v2"));
        assert!(parent_folder_settled(&dir, "v2").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
