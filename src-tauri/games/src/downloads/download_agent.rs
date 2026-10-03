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
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::fs::{create_dir_all, remove_file};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::mpsc::Sender;
use utils::{app_emit, lock, send};

use crate::downloads::utils::get_disk_available;
use crate::library::{Game, on_game_complete, push_game_update, set_partially_installed};
use crate::state::GameStatusManager;

use super::download_logic::{
    download_game_chunk, files_written_by, manifest_file_sizes, trim_stale_tails,
};
use super::drop_data::DropData;
use super::mod_data::{discard_originals, hand_files_to_base_game, mod_owned_files};

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
/// manifest. `run()` removes files an earlier version of the game shipped and
/// this one no longer does; a path under one of these is never removed even
/// then. The sweep used to delete EVERY file not in the manifest, and this
/// list was all that stood between it and the player's saves. Standalone emulators are the acute case: Eden/Yuzu/Ryujinx
/// keep per-title saves under `user/` (portable mode) and Cemu under `mlc01/`;
/// RetroArch saves live in `drop-saves/`. `remove_file` is a hard unlink (no
/// Recycle Bin), so a wrong delete here is irreversible.
const PROTECTED_DATA_DIRS: &[&str] = &[
    "user",       // Eden / Yuzu / Citron / Suyu / Sudachi portable data
    "portable",   // Ryujinx portable mode — Config.json (the controller bindings)
                  // plus bis/user/save. Ryujinx does NOT use `user/`; missing this
                  // meant every re-download, validation repair, resumed download or
                  // second ROM installed against the same emulator hard-unlinked
                  // the player's Switch saves and reset their controller layout.
    "bis",        // Ryujinx internal storage when not in portable mode
    "keys",       // Switch prod.keys/title.keys — user-supplied, never in a manifest
    "system",     // RetroArch BIOS/firmware. Two ways in and neither is in a
                  // manifest: the user drops a BIOS straight into system/ (which
                  // is what bios.rs is written around), and Drop itself copies
                  // that BIOS into the subdirectory the core reads —
                  // system/pcsx2/bios/ for PCSX2, system/dc/ for Flycast. Both
                  // were being hard-unlinked on every re-download, resumed
                  // download, validation repair, or second ROM installed against
                  // the same emulator, which reads to the player as "PS2 games
                  // stopped working again".
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
    ".mods", // mod_data::MODS_DIR: one ledger per installed mod. Losing these
             // makes every installed mod read as uninstalled and drops its
             // launch override, even when the mod's files survive.
];

/// Directories that hold user data WHEREVER they sit in the install tree, not
/// only at the top. Drop creates these next to a binary at runtime, and that
/// binary is often in a subfolder: an Unreal game keeps its Steam DLL in
/// `Binaries/Win64/`, so GBE's `drop-goldberg/` (achievements AND game saves)
/// and the `steam_settings/` Drop writes at launch live there, and a
/// top-level-only check let the sweep unlink them on every update or repair.
/// Kept separate from `PROTECTED_DATA_DIRS` because generic names like `user`
/// or `system` do appear deep inside shipped game data, and protecting those
/// at any depth would stop stale game files from ever being cleaned up.
const PROTECTED_DATA_DIRS_ANY_DEPTH: &[&str] = &[
    "drop-goldberg",
    "drop-saves",
    "steam_settings",
    // GBE fork and original Goldberg defaults, used next to the DLL by builds
    // that ignore Drop's `local_save_path` redirect (see goldberg/mod.rs
    // APPDATA_FALLBACK_DIRS, which Drop's own achievement reader scans).
    "GSE Saves",
    "Goldberg SteamEmu Saves",
];

/// Whether a path (POSIX-relative to the install dir) is runtime user data the
/// stale-file sweep must never delete. See the two lists above.
pub(crate) fn is_protected_user_data(relative: &str) -> bool {
    // Manifest keys are meant to be POSIX, but the path is joined with the OS
    // separator rules, so a `\\` or a `./` in a key would still name a real
    // nested directory on Windows. Judge the components the OS will see.
    let parts: Vec<&str> = relative
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let top_level = parts
        .first()
        .is_some_and(|top| PROTECTED_DATA_DIRS.iter().any(|d| d.eq_ignore_ascii_case(top)));
    // Directory components only: the last part is the file itself.
    let dirs = &parts[..parts.len().saturating_sub(1)];
    let any_depth = dirs.iter().any(|component| {
        PROTECTED_DATA_DIRS_ANY_DEPTH
            .iter()
            .any(|d| d.eq_ignore_ascii_case(component))
    });
    top_level || any_depth
}

/// Whether the stale-file sweep may delete this path (POSIX-relative to the
/// install dir): it is not in the current manifest, not Drop's own resume
/// ledger, not user data, and no installed mod wrote it. `mod_files` holds
/// lower-cased paths (see `mod_owned_files`).
fn should_sweep(
    relative: &str,
    file_list: &HashMap<String, String>,
    mod_files: &HashSet<String>,
) -> bool {
    if file_list.contains_key(relative) || relative == ".dropdata" {
        return false;
    }
    if is_protected_user_data(relative) {
        return false;
    }
    !mod_files.contains(&relative.to_lowercase())
}

/// Files the previous version of this game shipped that the version being
/// installed does not. These are the only files the sweep may consider: a file
/// Drop never installed is the player's or the game's, whatever its name.
fn stale_paths<'a>(
    previous: &'a HashMap<String, String>,
    current: &HashMap<String, String>,
) -> Vec<&'a str> {
    let mut stale: Vec<&str> = previous
        .keys()
        .filter(|path| !current.contains_key(*path))
        .map(String::as_str)
        .collect();
    stale.sort_unstable();
    stale
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInformation {
    pub file_list: HashMap<String, String>,
    pub manifests: HashMap<String, Manifest>,
    pub install_size: u64,
    pub download_size: u64,
    /// The version revision this manifest belongs to. Absent on servers from
    /// before in-place updates.
    #[serde(default)]
    pub revision: Option<u32>,
}

/// The server's download manifest for `version`. With `previous`, chunks
/// already present from that version are left out of the download.
pub(crate) async fn fetch_download_info(
    game_id: &str,
    version: &str,
    previous: Option<&str>,
) -> Result<DownloadInformation, ApplicationDownloadError> {
    let client = DROP_CLIENT_ASYNC.clone();
    let url = generate_url(
        &["/api/v1/client/game/manifest"],
        &[
            ("id", game_id),
            ("version", version),
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
        )?;

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
        let manifest_download = self
            .fetch_manifest(
                &self.metadata.version,
                self.dropdata.previously_installed_version.as_deref(),
            )
            .await?;

        if let Ok(mut manifest) = self.dl_info.lock() {
            *manifest = Some(manifest_download);
            return Ok(());
        }

        Err(ApplicationDownloadError::Lock)
    }

    /// The server's download manifest for `version`. With `previous`, chunks
    /// already present from that version are left out of the download.
    async fn fetch_manifest(
        &self,
        version: &str,
        previous: Option<&str>,
    ) -> Result<DownloadInformation, ApplicationDownloadError> {
        fetch_download_info(&self.metadata.id, version, previous).await
    }

    /// Remove files an earlier version of this game installed into this same
    /// directory that the version being installed no longer ships.
    ///
    /// Only an in-place version change has anything to remove. New versions
    /// normally get their own folder, and a resume, repair or reinstall of the
    /// same version wrote nothing that isn't in its manifest, so any other
    /// file there belongs to the player or the game: saves (RPG Maker's
    /// `save/`, Ren'Py's `game/saves/`), configs, GBE data, mods. The old
    /// sweep deleted everything not in the manifest and took those with it.
    ///
    /// `previously_installed_version` stays in the ledger after the update
    /// completes, so later resumes and repairs of this install run this
    /// again against the same old list. That only ever removes files the old
    /// version shipped and the new one doesn't.
    ///
    /// A file we can't identify or can't delete is left where it is and
    /// logged. The one failure that stops the download is not being able to
    /// stop mods from putting back their copies of files this version
    /// dropped (`discard_originals`), since that would roll the game back
    /// later; the next attempt tries again.
    ///
    /// If the previous version's file list can't be fetched, nothing is
    /// removed and no mod backup is discarded either. The folder then keeps
    /// the dropped files whether or not a mod is installed, so a mod putting
    /// its copy back on uninstall leaves it as it would be without mods. The
    /// sweep runs again on every later run of this install.
    async fn remove_files_dropped_since_previous_version(
        &self,
        file_list: &HashMap<String, String>,
    ) -> Result<(), ApplicationDownloadError> {
        let Some(previous) = self.dropdata.previously_installed_version.as_deref() else {
            return Ok(());
        };
        if previous == self.metadata.version {
            return Ok(());
        }
        let base_path = &self.dropdata.base_path;

        let previous_files = match self.fetch_manifest(previous, None).await {
            Ok(info) => info.file_list,
            Err(e) => {
                warn!(
                    "not removing files from version {previous} in {}: could not fetch its file list ({e})",
                    base_path.display()
                );
                return Ok(());
            }
        };

        // A mod may hold a backup of a file this version no longer ships.
        // Uninstalling the mod must not bring that file back, so the backups
        // go now, whether or not the file itself is swept below.
        let dropped: Vec<String> = stale_paths(&previous_files, file_list)
            .into_iter()
            .filter(|p| !is_protected_user_data(p))
            .map(str::to_string)
            .collect();
        discard_originals(base_path, &dropped).map_err(|why| {
            ApplicationDownloadError::IoError(Arc::new(std::io::Error::other(format!(
                "could not stop mods from putting back files version {previous} dropped: {why}"
            ))))
        })?;

        // Mods overlay into this directory and can overwrite a file the old
        // version shipped. If we can't tell which files are theirs, remove
        // nothing: a leftover file is recoverable, a deleted mod file is not.
        let mod_files = match mod_owned_files(base_path) {
            Ok(files) => files,
            Err(why) => {
                warn!(
                    "not removing files from version {previous} in {}: {why}",
                    base_path.display()
                );
                return Ok(());
            }
        };

        let Ok(base_real) = base_path.canonicalize() else {
            warn!("not removing old files: cannot resolve {}", base_path.display());
            return Ok(());
        };

        for relative in stale_paths(&previous_files, file_list) {
            if !should_sweep(relative, file_list, &mod_files) {
                continue;
            }
            let path = base_path.join(relative);
            // Only a regular file, reached without leaving the install dir
            // through a symlink, a junction or a `..` in the server's path.
            let is_file = std::fs::symlink_metadata(&path)
                .map(|m| m.is_file())
                .unwrap_or(false);
            if !is_file {
                continue;
            }
            let inside = path
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .is_some_and(|parent| parent.starts_with(&base_real));
            if !inside {
                warn!("not removing {}: it is outside the install dir", path.display());
                continue;
            }
            match remove_file(&path) {
                Ok(()) => debug!("removed {} (shipped by {previous}, not by this version)", path.display()),
                Err(e) => warn!("could not remove old file {}: {e}", path.display()),
            }
        }
        Ok(())
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
        self.remove_files_dropped_since_previous_version(file_list)
            .await?;

        // A repair after an in-place update: files the player chose to keep
        // in that update get a .bak before the game's copy is written back.
        let about_to_write: HashSet<String> = manifests_chunks
            .iter()
            .flat_map(|(version_id, chunks, _)| {
                chunks
                    .iter()
                    .filter(|(id, _)| !completed_chunks.get(*id).copied().unwrap_or(false))
                    .flat_map(move |(_, chunk)| {
                        chunk
                            .files
                            .iter()
                            .filter(move |f| file_list.get(&f.filename) == Some(version_id))
                            .map(|f| f.filename.clone())
                    })
            })
            .collect();
        crate::downloads::update::back_up_kept_files(base_path, &self.metadata.version, &about_to_write)
            .map_err(|e| ApplicationDownloadError::IoError(Arc::new(std::io::Error::other(e))))?;

        let local_completed_chunks = completed_chunks.clone();

        // Which files each chunk writes (relative to the install dir, as the
        // manifest spells them). A mod may have overwritten some of them; once
        // the base game writes them again they are the game's, not the mod's,
        // and the mod must neither delete them nor restore its stale backup on
        // uninstall. Handed over per chunk, before the chunk is recorded as
        // done: a crash in between re-downloads the chunk and hands over
        // again, where a hand-over at the end of the run would be lost.
        let chunk_files: HashMap<String, Vec<String>> = manifests_chunks
            .iter()
            .flat_map(|(version_id, chunks, _)| {
                chunks.iter().map(move |(chunk_id, chunk)| {
                    let written = chunk
                        .files
                        .iter()
                        .filter(|f| file_list.get(&f.filename) == Some(version_id))
                        .map(|f| f.filename.clone())
                        .collect();
                    (chunk_id.clone(), written)
                })
            })
            .collect();

        let mut chunk_completions = FuturesUnordered::new();

        let mut outputs = Vec::new();

        // Persist each successful chunk to .dropdata immediately. The old
        // code only wrote dropdata after every chunk finished, so a crash /
        // force-quit / power loss with 9.5/10 GB downloaded re-downloaded
        // the entire 9.5 GB on resume. Each write is small (a few KB of
        // bincode) and sits well below the per-chunk download cost, so the
        // I/O is negligible compared to the bandwidth saved on resume.
        let dropdata = &self.dropdata;
        let chunk_files = &chunk_files;
        let mut handle_output =
            |value: Result<Option<String>, ApplicationDownloadError>| match value {
                Ok(value) => {
                    if let Some(chunk_id) = value {
                        if let Some(written) = chunk_files.get(&chunk_id) {
                            // Not recorded as done if this fails, so the chunk
                            // (and the hand-over) is retried.
                            hand_files_to_base_game(base_path, written).map_err(|why| {
                                ApplicationDownloadError::IoError(Arc::new(std::io::Error::other(
                                    format!("could not take back files a mod had replaced: {why}"),
                                )))
                            })?;
                        }
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
        self.trim_rewritten_files().await?;
        Ok(true)
    }

    /// Cut every file this download rewrote back to its manifest size (see
    /// `trim_stale_tails`): an in-place update leaves the old tail on a file
    /// that got smaller. The sizes need the full manifest, since the delta
    /// one leaves out chunks of unchanged files.
    async fn trim_rewritten_files(&self) -> Result<(), ApplicationDownloadError> {
        let (written, delta_sizes) = {
            let dl_info = lock!(self.dl_info);
            let info = dl_info.as_ref().ok_or(ApplicationDownloadError::NotInitialized)?;
            (files_written_by(info), manifest_file_sizes(info))
        };
        let sizes = if self.dropdata.previously_installed_version.is_some() {
            manifest_file_sizes(&self.fetch_manifest(&self.metadata.version, None).await?)
        } else {
            delta_sizes
        };
        let trimmed = trim_stale_tails(&self.dropdata.base_path, &written, &sizes)
            .map_err(|e| ApplicationDownloadError::IoError(Arc::new(e)))?;
        if trimmed > 0 {
            info!("{}: cut {trimmed} rewritten file(s) back to size", self.metadata.id);
        }
        Ok(())
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
            Ok(_) => {
                // What was installed, for the next in-place update to compare
                // against. Best effort: it never fails the install.
                let revision = lock!(self.dl_info).as_ref().and_then(|i| i.revision);
                crate::downloads::update::record_fresh_baseline(
                    &self.metadata.id,
                    &self.metadata.version,
                    &self.dropdata.base_path,
                    revision,
                )
                .await;
            }
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

#[cfg(test)]
mod sweep_tests {
    use super::*;

    fn manifest(paths: &[&str]) -> HashMap<String, String> {
        paths.iter().map(|p| (p.to_string(), String::new())).collect()
    }

    #[test]
    fn protected_names_at_top_level() {
        assert!(is_protected_user_data("user/nand/save.bin"));
        assert!(is_protected_user_data("System/scph1001.bin"));
        assert!(is_protected_user_data(".mods/smapi.moddata"));
        // Generic names are only protected at the top: deep inside game data
        // they are ordinary shipped files that may go stale.
        assert!(!is_protected_user_data("Content/system/old.pak"));
        assert!(!is_protected_user_data("data/user/stale.dat"));
    }

    #[test]
    fn drop_runtime_dirs_protected_at_any_depth() {
        // Unreal layout: the Steam DLL, and so GBE's folders, sit two deep.
        assert!(is_protected_user_data(
            "Binaries/Win64/drop-goldberg/480/achievements.json"
        ));
        assert!(is_protected_user_data(
            "Binaries/Win64/drop-goldberg/480/remote/save1.sav"
        ));
        assert!(is_protected_user_data(
            "Binaries/Win64/steam_settings/configs.user.ini"
        ));
        assert!(is_protected_user_data("RetroArch-Win64/drop-saves/u1/game.srm"));
        assert!(is_protected_user_data("A/B/C/DROP-GOLDBERG/x"));
    }

    #[test]
    fn protection_sees_through_backslashes_and_dot_segments() {
        assert!(is_protected_user_data("saves\\slot1.sav"));
        assert!(is_protected_user_data("./saves/slot1.sav"));
        assert!(is_protected_user_data("Binaries\\Win64\\drop-goldberg\\480\\a.json"));
    }

    #[test]
    fn a_file_merely_named_like_a_dir_is_not_protected() {
        assert!(!is_protected_user_data("bin/steam_settings"));
        assert!(!is_protected_user_data("readme-drop-goldberg.txt"));
    }

    #[test]
    fn mods_dir_constant_is_protected() {
        assert!(PROTECTED_DATA_DIRS.contains(&super::super::mod_data::MODS_DIR));
    }

    #[test]
    fn gbe_fork_default_dirs_protected_at_any_depth() {
        assert!(is_protected_user_data("Binaries/Win64/GSE Saves/480/achievements.json"));
        assert!(is_protected_user_data("bin/Goldberg SteamEmu Saves/480/remote/a.sav"));
    }

    #[test]
    fn only_files_the_previous_version_shipped_are_stale() {
        let previous = manifest(&["Game.exe", "old.dll", "data/removed.pak", "data/kept.pak"]);
        let current = manifest(&["Game.exe", "data/kept.pak", "data/new.pak"]);
        assert_eq!(stale_paths(&previous, &current), vec!["data/removed.pak", "old.dll"]);
        // A resume or repair of the same version has nothing to remove.
        assert!(stale_paths(&current, &current).is_empty());
    }

    #[test]
    fn stale_file_under_user_data_or_owned_by_a_mod_is_kept() {
        let current = manifest(&["Game.exe"]);
        let mods: HashSet<String> = ["mods/loader.dll".to_string()].into();
        // The old version shipped a default save, which the player now owns.
        assert!(!should_sweep("saves/slot1.sav", &current, &mods));
        // A mod replaced a file the old version shipped.
        assert!(!should_sweep("Mods/Loader.dll", &current, &mods));
        assert!(should_sweep("old.dll", &current, &mods));
    }

    #[test]
    fn sweep_keeps_manifest_ledger_userdata_and_mod_files() {
        let list = manifest(&["Game.exe", "Content/Paks/base.pak"]);
        let mods: HashSet<String> = ["mods/contentpatcher/contentpatcher.dll".to_string()].into();

        assert!(!should_sweep("Game.exe", &list, &mods));
        assert!(!should_sweep(".dropdata", &list, &mods));
        assert!(!should_sweep("Binaries/Win64/drop-goldberg/480/x.sav", &list, &mods));
        // Mod ledger paths are lower-cased; on-disk casing may differ.
        assert!(!should_sweep("Mods/ContentPatcher/ContentPatcher.dll", &list, &mods));

        assert!(should_sweep("Content/Paks/old_v1.pak", &list, &mods));
        assert!(should_sweep("leftover.txt", &list, &mods));
    }
}

