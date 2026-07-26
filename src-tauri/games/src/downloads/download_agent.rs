use async_trait::async_trait;
use database::models::data::UserConfiguration;
use database::{
    ApplicationTransientStatus, DownloadableMetadata, borrow_db_checked, borrow_db_mut_checked,
};
use download_manager::depot_manager::DepotManager;
use download_manager::download_manager_frontend::{DownloadManagerSignal, DownloadStatus};
use download_manager::downloadable::Downloadable;
use download_manager::error::ApplicationDownloadError;
use download_manager::util::download_thread_control_flag::{
    DownloadThreadControl, DownloadThreadControlFlag,
};
use download_manager::util::progress_object::{ProgressHandle, ProgressObject, ProgressType};
use droplet_rs::manifest::{ChunkData, Manifest};
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use log::{debug, error, info, warn};
use remote::auth::generate_authorization_header;
use remote::cache::get_cached_object;
use remote::error::RemoteAccessError;
use remote::requests::generate_url;
use remote::utils::DROP_CLIENT_ASYNC;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{create_dir_all, remove_file};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::mpsc::Sender;
use utils::{app_emit, lock, send};

use crate::downloads::utils::get_disk_available;
use crate::library::{Game, on_game_complete, push_game_update, set_partially_installed};
use crate::state::GameStatusManager;

use super::download_logic::download_game_chunk;
use super::drop_data::DropData;

pub(crate) static RETRY_COUNT: usize = 3;

/// Make a version identifier safe to use as a single path segment (fresh
/// installs live under `<base>/<library_path>/<version>`). Keeps alphanumerics
/// plus `-_.`, replaces anything else with `_`, and never yields an empty or
/// dot-only segment.
fn sanitize_version_segment(version: &str) -> String {
    let cleaned: String = version
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() {
        "version".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Whether a download error is a full disk — which no amount of retrying fixes.
/// Matches ENOSPC (28) on Unix and ERROR_DISK_FULL (112) / ERROR_HANDLE_DISK_FULL
/// (39) on Windows, since `io::ErrorKind::StorageFull` isn't stable.
pub(crate) fn is_disk_full(e: &ApplicationDownloadError) -> bool {
    if let ApplicationDownloadError::IoError(io) = e {
        return matches!(io.raw_os_error(), Some(28) | Some(112) | Some(39));
    }
    false
}

/// Top-level directories inside an install dir that hold USER data created at
/// runtime (saves, NAND, configs) rather than files shipped in the server
/// manifest. The reconcile sweep in `run()` deletes anything not in the
/// manifest to clear stale files left by a previous version; without this
/// guard it also deletes these, wiping the player's saves on every
/// re-download. Standalone emulators are the acute case: Eden/Yuzu/Ryujinx
/// keep per-title saves under `user/` (portable mode) and Cemu under `mlc01/`;
/// RetroArch saves live in `drop-saves/`. `remove_file` is a hard unlink (no
/// Recycle Bin), so a wrong delete here is irreversible.
const PROTECTED_DATA_DIRS: &[&str] = &[
    "user",       // Eden / Yuzu / Ryujinx / Citron / Suyu / Sudachi portable data
    "mlc01",      // Cemu NAND (saves + updates + DLC)
    "drop-saves",    // RetroArch per-game saves/states (Drop-managed)
    "drop-goldberg", // Goldberg/GBE per-AppID earned achievements + saves
    "steam_settings", // GBE config the CLIENT writes at launch (configs.user.ini
                      // absolute save path + custom_broadcasts.txt co-op peers) —
                      // not in the manifest, so the sweep would otherwise unlink it
    "saves",
    "states",
    "nand",
    "sdmc",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInformation {
    pub file_list: HashMap<String, String>,
    pub manifests: HashMap<String, Manifest>,
    pub install_size: u64,
    pub download_size: u64,
}

pub struct GameDownloadAgent {
    pub metadata: DownloadableMetadata,
    pub configuration: UserConfiguration,
    pub control_flag: DownloadThreadControl,
    pub dl_info: Mutex<Option<DownloadInformation>>,
    pub download_progress: Arc<ProgressObject>,
    pub disk_progress: Arc<ProgressObject>,
    depot_manager: Arc<DepotManager>,
    sender: Sender<DownloadManagerSignal>,
    pub dropdata: DropData,
    status: Mutex<DownloadStatus>,
}

impl Debug for GameDownloadAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameDownloadAgent").finish()
    }
}

impl GameDownloadAgent {
    pub async fn new(
        metadata: DownloadableMetadata,
        base_dir: PathBuf,
        sender: Sender<DownloadManagerSignal>,
        depot_manager: Arc<DepotManager>,
        configuration: UserConfiguration,
    ) -> Result<Self, ApplicationDownloadError> {
        // Don't run by default
        let control_flag = DownloadThreadControl::new(DownloadThreadControlFlag::Stop);

        let game_name = get_cached_object::<Game>(&format!("game/{}", metadata.id))
            .map(|v| v.library_path)
            .unwrap_or(metadata.id.clone());

        info!("base dir {}", Path::new(&base_dir).display());
        // Multi-version install: an existing install of this exact (game,
        // version) keeps its recorded directory, so updates/resumes land in the
        // same place and pre-multi-version single-version installs aren't
        // orphaned. A fresh install goes under a per-version subfolder, so two
        // versions of one game never collide on disk and overwrite each other.
        let data_base_dir_path = borrow_db_checked()
            .applications
            .get_install(&metadata.id, &metadata.version)
            .map(|r| PathBuf::from(&r.install_dir))
            .unwrap_or_else(|| {
                Path::new(&base_dir)
                    .join(&game_name)
                    .join(sanitize_version_segment(&metadata.version))
            });
        info!("data dir path {}", data_base_dir_path.display());

        create_dir_all(data_base_dir_path.clone())?;

        let stored_manifest = DropData::generate(
            metadata.id.clone(),
            metadata.version.clone(),
            metadata.target_platform,
            data_base_dir_path.clone(),
        );

        let result = Self {
            metadata,
            control_flag,
            dl_info: Mutex::new(None),
            download_progress: Arc::new(ProgressObject::new(
                0,
                0,
                sender.clone(),
                ProgressType::Download,
            )),
            disk_progress: Arc::new(ProgressObject::new(
                0,
                0,
                sender.clone(),
                ProgressType::Disk,
            )),
            sender,
            dropdata: stored_manifest,
            status: Mutex::new(DownloadStatus::Queued),
            depot_manager,
            configuration,
        };

        result.ensure_manifest_exists().await?;

        let required_space = lock!(result.dl_info).as_ref().unwrap().install_size;

        let available_space = get_disk_available(data_base_dir_path)? as u64;

        if required_space > available_space {
            return Err(ApplicationDownloadError::DiskFull(
                required_space,
                available_space,
            ));
        }

        Ok(result)
    }

    fn scan_filetree(&self, path: &Path) -> Result<Vec<PathBuf>, io::Error> {
        if !path.is_dir() {
            return Ok(vec![path.into()]);
        };

        let subdirs = path.read_dir()?;
        let mut results = Vec::new();
        for subdir in subdirs {
            let subdir = subdir?;
            let subfiles = self.scan_filetree(&subdir.path())?;
            results.extend(subfiles);
        }
        Ok(results)
    }

    // Blocking
    pub fn setup_download(&self, app_handle: &AppHandle) -> Result<(), ApplicationDownloadError> {
        let mut db_lock = borrow_db_mut_checked();
        let status = ApplicationTransientStatus::Downloading {
            version_id: self.metadata.version.clone(),
        };
        db_lock
            .applications
            .transient_statuses
            .insert(self.metadata(), status.clone());
        // Don't use GameStatusManager because this game isn't installed
        push_game_update(app_handle, &self.metadata().id, None, (None, Some(status)));

        if !self.check_manifest_exists() {
            return Err(ApplicationDownloadError::NotInitialized);
        }

        // The download manager sets the flag to Go before spawning the
        // thread that calls us. Setting it again here would clobber any
        // pause the user issued in the brief window between spawn and the
        // first chunk check.
        Ok(())
    }

    // Blocking
    pub async fn download(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        self.setup_download(app_handle)?;
        let timer = Instant::now();

        info!("beginning download for {}...", self.metadata().id);

        let res = self.run().await;

        debug!(
            "{} took {}ms to download",
            self.metadata.id,
            timer.elapsed().as_millis()
        );
        res
    }

    pub fn check_manifest_exists(&self) -> bool {
        lock!(self.dl_info).is_some()
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
                    self.dropdata
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

    // Sets up progress for download writes
    fn setup_progress(&self) {
        let dl_info = lock!(self.dl_info);
        let dl_info = dl_info.as_ref().unwrap();

        let total_chunks = dl_info
            .manifests
            .iter()
            .map(|v| v.1.chunks.len())
            .sum::<usize>();

        self.download_progress
            .set_max(dl_info.download_size.try_into().unwrap());
        self.download_progress.set_size(total_chunks);
        self.download_progress.reset();

        self.disk_progress
            .set_max(dl_info.install_size.try_into().unwrap());
        self.disk_progress.set_size(total_chunks);
        self.disk_progress.reset();
    }

    async fn run(&self) -> Result<bool, ApplicationDownloadError> {
        self.depot_manager.sync_depots().await?;
        info!("synced depots");
        self.setup_progress();
        info!("setup progress objects");
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
            let completed_chunks = lock!(self.dropdata.contexts);
            completed_chunks.clone()
        };
        info!("started with {} existing chunks", completed_chunks.len());
        let chunk_len = manifests_chunks.iter().map(|v| v.1.len()).sum::<usize>();
        let mut max_download_threads = borrow_db_checked().settings.max_download_threads;
        if max_download_threads == 0 {
            max_download_threads = 1;
        }

        let file_list = &file_list;
        let base_path = &self.dropdata.base_path;
        let current_file_tree = self.scan_filetree(base_path)?;

        for file in current_file_tree {
            let relative = file.strip_prefix(base_path)?;
            // The server's file_list keys are POSIX paths (forward slashes).
            // On Windows relative.to_string_lossy() yields BACKSLASH separators,
            // so a subdirectory file ("game\asset\x") never matches the manifest
            // key ("game/asset/x") — and the sweep below would then DELETE every
            // file in a subdirectory on any re-run of run() (a validation repair
            // pass or a resumed download), destroying a working install. Join the
            // components with "/" so the key matches on every OS. Literal
            // backslashes inside a single Unix filename are preserved (they are
            // one Normal component, not a separator).
            let filename = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let needed = file_list.contains_key(&filename) || filename == ".dropdata";
            if needed {
                continue;
            }

            // Never delete runtime/user data that lives inside the install dir
            // but isn't part of any manifest (emulator saves/NAND/configs).
            // The sweep exists to remove files left over from a *previous Drop
            // install*, not data the user or a standalone emulator wrote at
            // runtime. See PROTECTED_DATA_DIRS — installing a second game that
            // shares an emulator re-runs this agent over the shared install
            // dir, and without the guard it wipes every title's saves.
            let top_component = relative
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str());
            let in_protected_dir = match top_component {
                Some(top) => PROTECTED_DATA_DIRS
                    .iter()
                    .any(|dir| dir.eq_ignore_ascii_case(top)),
                None => false,
            };
            if in_protected_dir {
                debug!("preserving user data (not in manifest): {}", file.display());
                continue;
            }

            debug!("deleted {}", file.display());
            remove_file(file)?;
        }

        let local_completed_chunks = completed_chunks.clone();

        let mut chunk_completions = FuturesUnordered::new();

        let mut outputs = Vec::new();

        // Persist each successful chunk to .dropdata immediately. The old
        // code only wrote dropdata after every chunk finished, so a crash /
        // force-quit / power loss with 9.5/10 GB downloaded re-downloaded
        // the entire 9.5 GB on resume. Each write is small (a few KB of
        // bincode) and sits well below the per-chunk download cost, so the
        // I/O is negligible compared to the bandwidth saved on resume.
        let dropdata = &self.dropdata;
        let mut handle_output =
            |value: Result<Option<String>, ApplicationDownloadError>| match value {
                Ok(value) => {
                    if let Some(chunk_id) = value {
                        dropdata.set_context(chunk_id.clone(), true);
                        dropdata.write();
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
                    Err(err) => {
                        return Err(err.into());
                    }
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

                                // Retry transient failures. A full disk is the
                                // exception — it won't clear itself, so fail
                                // fast rather than burning the backoff retries
                                // on a write that can't succeed.
                                let retry = !is_disk_full(&e);

                                if i == RETRY_COUNT - 1 || !retry {
                                    warn!("retry logic failed after {} attempts, not re-attempting.", i + 1);
                                    return Err(e);
                                }

                                // Exponential backoff: 1s, 2s, 4s, ...
                                let backoff = Duration::from_secs(1 << i);
                                warn!("retrying chunk {} in {:?} (attempt {}/{})", chunk_id, backoff, i + 2, RETRY_COUNT);
                                tokio::time::sleep(backoff).await;
                            }
                        }
                    }
                    Ok(None)
                });
            }
        }

        // Collect failures without bailing early. The old code did
        // `handle_output(value)?` which aborted on the first chunk that
        // exhausted its retries — cancelling every other in-flight chunk
        // in FuturesUnordered. With incremental persistence (above), the
        // completed chunks survive, but cancelling the in-flight ones
        // throws away minutes of bandwidth that would have succeeded.
        // Let everything drain, then surface the combined error so the
        // user retries against a much smaller remaining set.
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
            // Pause was a legitimate exit (chunks return Ok(false), not Err),
            // so any errors here are real failures. Surface the first — the
            // outer manager logs and removes the agent; the user retries from
            // the queue UI, and incremental persistence means we restart
            // from `completed_chunks.len()` chunks ahead of where we were.
            return Err(first);
        }

        let drop_data_chunks = completed_chunks
            .iter()
            .map(|v| (v.0.to_string(), *v.1))
            .collect::<Vec<(String, bool)>>();

        self.dropdata.set_contexts(&drop_data_chunks);
        self.dropdata.write();

        info!("completed {} chunks", drop_data_chunks.len());

        // If there are any contexts left which are false
        if completed_chunks.len() != chunk_len {
            info!(
                "download agent for {} exited without completing ({}/{})",
                self.metadata.id.clone(),
                completed_chunks.len(),
                chunk_len,
            );
            return Ok(false);
        }
        Ok(true)
    }

    /// Mark the game as `Validating` in the DB and notify the frontend, so
    /// the user sees the post-download verification phase rather than a
    /// silent gap before the install is confirmed.
    fn setup_validate(&self, app_handle: &AppHandle) {
        let status = ApplicationTransientStatus::Validating {
            version_id: self.metadata.version.clone(),
        };

        let mut db_lock = borrow_db_mut_checked();
        db_lock
            .applications
            .transient_statuses
            .insert(self.metadata(), status.clone());
        drop(db_lock);
        push_game_update(app_handle, &self.metadata().id, None, (None, Some(status)));
    }

    /// Post-install validation — the gate the LWIW incident proved was
    /// missing. Re-derives ground truth from disk (file presence, file
    /// sizes, per-chunk SHA-256) and compares it to the server manifest.
    ///
    /// Returns:
    ///   - `Ok(true)`  — every manifest file is present at the right size
    ///     and every chunk hashes correctly. The caller may
    ///     transition the game to `Installed`.
    ///   - `Err(ValidationFailed)` — the install does not match the
    ///     manifest. A `bool` of `false` is deliberately NOT
    ///     returned here: in the download manager loop `false`
    ///     means "re-run download", which for a genuinely
    ///     incomplete upstream (the LWIW case) would loop
    ///     forever. An error surfaces a clear message to the
    ///     user and aborts the install instead.
    pub fn validate(&self, app_handle: &AppHandle) -> Result<bool, ApplicationDownloadError> {
        self.setup_validate(app_handle);

        let install_dir = self.dropdata.base_path.clone();
        info!(
            "running post-install validation for {} at {}",
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
                info!("validation succeeded for {}", self.metadata.id);
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

                // Invalidate exactly the chunks the validation implicated so the
                // next download pass re-fetches ONLY them and skips the
                // (verified-good) remainder. Without this the bad chunk stays
                // marked complete in .dropdata, so any resume skips it and
                // re-fails on the same chunk forever — the user's only escape
                // was wiping .dropdata and re-downloading the entire game.
                let total_chunks = {
                    let dl_info = lock!(self.dl_info);
                    dl_info
                        .as_ref()
                        .map(|d| d.manifests.values().map(|m| m.chunks.len()).sum())
                        .unwrap_or(0usize)
                };
                let invalidated = self.invalidate_failed_chunks(&missing, &mismatched);

                // Demote to PartiallyInstalled and persist the cleared contexts,
                // so a manual resume also re-fetches only the bad chunks rather
                // than the whole game.
                set_partially_installed(
                    &self.metadata(),
                    self.dropdata.base_path.display().to_string(),
                    Some(app_handle),
                    self.configuration.clone(),
                );
                self.dropdata.write();

                // A handful of bad chunks is a transient corruption: return
                // Ok(false) to drive the download manager's bounded repair loop
                // (MAX_DOWNLOAD_PASSES), which re-downloads just the invalidated
                // chunks and re-validates. If a large fraction failed the cause
                // is systemic (stale server manifest, wrong depot, truncated
                // upstream) and re-fetching cannot fix it — fail loudly instead
                // of looping (the LWIW lesson). Small games get an absolute floor
                // so one bad chunk out of three still counts as repairable.
                const REPAIRABLE_CHUNK_FLOOR: usize = 64;
                let repairable = invalidated > 0
                    && (invalidated <= REPAIRABLE_CHUNK_FLOOR
                        || invalidated.saturating_mul(4) <= total_chunks);
                if repairable {
                    warn!(
                        "validation failed for {}: {} missing, {} mismatched — invalidated {}/{} chunk(s); requesting targeted re-download",
                        self.metadata.id,
                        missing.len(),
                        mismatched.len(),
                        invalidated,
                        total_chunks
                    );
                    Ok(false)
                } else {
                    error!(
                        "validation failed for {}: {} missing, {} mismatched — {}/{} chunk(s) bad, too broad to repair by re-download; aborting",
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

    /// Mark every chunk implicated by a failed validation as incomplete in
    /// `.dropdata`, so the next download pass re-fetches exactly those chunks
    /// and skips the verified-good remainder. Mismatched chunks are cleared by
    /// id; missing/short files carry no chunk id, so they are mapped back to
    /// every chunk that writes any byte into them. Returns the count of
    /// distinct chunks invalidated.
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
            self.dropdata.set_context(chunk_id.clone(), false);
        }

        to_clear.len()
    }

    pub fn cancel(&self, app_handle: &AppHandle) {
        // See docs on usage
        set_partially_installed(
            &self.metadata(),
            self.dropdata.base_path.display().to_string(),
            Some(app_handle),
            self.configuration.clone(),
        );

        self.dropdata.write();
    }
}

#[async_trait]
impl Downloadable for GameDownloadAgent {
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

    fn on_queued(&self, app_handle: &tauri::AppHandle) {
        *self.status.lock().unwrap() = DownloadStatus::Queued;
        let mut db_lock = borrow_db_mut_checked();
        let status = ApplicationTransientStatus::Queued {
            version_id: self.metadata.version.clone(),
        };
        db_lock
            .applications
            .transient_statuses
            .insert(self.metadata(), status.clone());
        push_game_update(app_handle, &self.metadata.id, None, (None, Some(status)));
    }

    fn on_error(&self, app_handle: &tauri::AppHandle, error: &ApplicationDownloadError) {
        *lock!(self.status) = DownloadStatus::Error;
        app_emit!(app_handle, "download_error", error.to_string());

        error!("error while managing download: {error:?}");

        let mut handle = borrow_db_mut_checked();
        handle
            .applications
            .transient_statuses
            .remove(&self.metadata());

        // Pass the installed version. push_game_update SKIPS refreshes for an
        // Installed game when version is None (see its guard) — so a failed
        // re-download/update of an already-installed game left the UI frozen on
        // "Downloading" with no way to retry. With the version present the guard
        // passes and the game resets to its real (playable, retryable) status.
        // A never-installed game has no entry here → None, and its post-error
        // state is Remote, which the guard doesn't skip anyway.
        let version = handle
            .applications
            .installed_game_version
            .get(&self.metadata.id)
            .and_then(|m| handle.applications.game_versions.get(&m.version).cloned());

        push_game_update(
            app_handle,
            &self.metadata.id,
            version,
            GameStatusManager::fetch_state(&self.metadata.id, &handle),
        );
    }

    async fn on_complete(&self, app_handle: &tauri::AppHandle) {
        match on_game_complete(
            &self.metadata(),
            self.configuration.clone(),
            self.dropdata.base_path.to_string_lossy().to_string(),
            app_handle,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("could not mark game as complete: {e}");
                send!(
                    self.sender,
                    DownloadManagerSignal::Error(ApplicationDownloadError::DownloadError(e))
                );
            }
        }
    }

    fn on_cancelled(&self, app_handle: &tauri::AppHandle) {
        self.cancel(app_handle);
    }

    fn status(&self) -> DownloadStatus {
        lock!(self.status).clone()
    }
}
