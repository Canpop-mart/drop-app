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
//!  2. `run()` omits the reconcile sweep — a mod is purely additive, so a sweep
//!     over the shared parent dir would delete the entire base game.
//!  3. On completion it records the exact files it wrote into `.moddata` so an
//!     uninstall removes precisely those files and nothing of the base game.

use async_trait::async_trait;
use database::models::data::UserConfiguration;
use database::{
    ApplicationTransientStatus, DownloadableMetadata, borrow_db_mut_checked,
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

use super::download_logic::download_game_chunk;
use super::mod_data::{ModData, MODS_DIR, moddata_path};

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
        let client = DROP_CLIENT_ASYNC.clone();
        let url = generate_url(
            &["/api/v1/client/game/manifest"],
            &[
                ("id", &self.metadata.id),
                ("version", &self.metadata.version),
                (
                    "previous",
                    self.moddata
                        .previously_installed_version
                        .as_ref()
                        .map_or("", |v| v),
                ),
            ],
        )
        .map_err(ApplicationDownloadError::Communication)?;

        let response = client
            .get(url)
            .header("Authorization", generate_authorization_header())
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

        let manifest_download: DownloadInformation = response
            .json()
            .await
            .map_err(|e| ApplicationDownloadError::Communication(e.into()))?;

        if let Ok(mut manifest) = self.dl_info.lock() {
            *manifest = Some(manifest_download);
            return Ok(());
        }
        Err(ApplicationDownloadError::Lock)
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

    /// Record the exact set of files this mod wrote (the manifest's file list,
    /// POSIX-relative) so uninstall removes precisely these.
    fn record_installed_files(&self) {
        let files: Vec<String> = {
            let dl_info = lock!(self.dl_info);
            match dl_info.as_ref() {
                Some(info) => info.file_list.keys().cloned().collect(),
                None => Vec::new(),
            }
        };
        self.moddata.set_installed_files(files);
        self.moddata.write();
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
    }

    async fn on_complete(&self, app_handle: &AppHandle) {
        // Record which files we wrote BEFORE marking installed, so a crash in
        // the window can't leave an "installed" mod with an empty file list
        // (which would make uninstall a silent no-op that leaks the files).
        self.record_installed_files();

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
    }

    fn on_cancelled(&self, app_handle: &AppHandle) {
        // Mod-safe cancel: the ledger is already written incrementally, so just
        // clear the transient status and refresh the UI. Deliberately does NOT
        // call set_partially_installed — a mod must never hold a game-shaped
        // "PartiallyInstalled" status, or the generic resume path would rebuild
        // a GameDownloadAgent over the parent dir and sweep the base game. A
        // re-install resumes from the persisted `.moddata` ledger instead.
        self.moddata.write();
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
    }

    fn status(&self) -> DownloadStatus {
        lock!(self.status).clone()
    }
}
