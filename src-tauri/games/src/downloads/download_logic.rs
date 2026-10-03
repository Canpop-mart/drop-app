use std::collections::{HashMap, HashSet};
#[cfg(unix)]
use std::fs::{Permissions, set_permissions};
use std::io::SeekFrom;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
#[cfg(unix)]
use std::sync::Arc;
use std::time::Instant;

use aes::cipher::{KeyIvInit, StreamCipher};
use download_manager::error::ApplicationDownloadError;
use download_manager::util::download_thread_control_flag::{
    DownloadThreadControl, DownloadThreadControlFlag,
};
use download_manager::util::progress_object::ProgressHandle;
use droplet_rs::manifest::ChunkData;
use futures_util::StreamExt as _;
use log::{debug, info};
use remote::auth::generate_authorization_header;
use remote::error::{DropServerError, RemoteAccessError};
use remote::utils::DROP_CLIENT_DOWNLOAD;
use sha2::Digest;
use tauri::Url;
use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _, AsyncWriteExt as _};
use tokio_util::io::StreamReader;
use utils::path_guard;

use super::download_agent::DownloadInformation;

const READ_BUF_LEN: usize = 1024 * 1024;

type Aes128Ctr64LE = ctr::Ctr64LE<aes::Aes128>;

#[allow(clippy::too_many_arguments)]
pub async fn download_game_chunk(
    game_id: &str,
    version_id: &str,
    chunk_id: &str,
    depot: &str,
    key: &[u8; 16],
    chunk_data: &ChunkData,
    file_list: &HashMap<String, String>,
    base_path: &Path,
    control_flag: &DownloadThreadControl,
    // How much we're downloading
    download_progress: &ProgressHandle,
    // How much we're writing to disk
    disk_progress: &ProgressHandle,
) -> Result<bool, ApplicationDownloadError> {
    // Reset the per-chunk progress counters at the start of every call.
    // The outer caller retries this function on failure, and each retry
    // re-downloads the chunk from byte 0 — without this reset, bytes
    // from the previous attempt remain in the counter and subsequent
    // .add() calls stack on top, producing "27/22 GB" style overshoots.
    download_progress.set(0);
    disk_progress.set(0);

    // If we're paused
    if control_flag.get() == DownloadThreadControlFlag::Stop {
        return Ok(false);
    }

    let start = Instant::now();

    let header = generate_authorization_header()?;

    let url = Url::parse(depot)
        .map_err(|v| ApplicationDownloadError::DownloadError(v.into()))?
        .join(&format!("content/{}/{}/{}", game_id, version_id, chunk_id))
        .map_err(|v| ApplicationDownloadError::DownloadError(v.into()))?;

    let response = DROP_CLIENT_DOWNLOAD
        .get(url)
        .header("Authorization", header)
        .send()
        .await
        .map_err(|e| ApplicationDownloadError::Communication(e.into()))?;

    if response.status() != 200 {
        info!("chunk request got status code: {}", response.status());
        let raw_res = response.text().await.map_err(|e| {
            ApplicationDownloadError::Communication(RemoteAccessError::FetchErrorLegacy(e.into()))
        })?;
        info!("{raw_res}");
        if let Ok(err) = serde_json::from_str::<DropServerError>(&raw_res) {
            return Err(ApplicationDownloadError::Communication(
                RemoteAccessError::InvalidResponse(err),
            ));
        }
        return Err(ApplicationDownloadError::Communication(
            RemoteAccessError::UnparseableResponse(raw_res),
        ));
    }

    if control_flag.get() == DownloadThreadControlFlag::Stop {
        download_progress.set(0);
        disk_progress.set(0);
        return Ok(false);
    }

    let timestep = start.elapsed().as_millis();

    debug!("took {}ms to start downloading", timestep);

    let stream = response
        .bytes_stream()
        .map(|v| v.map_err(std::io::Error::other));
    let mut stream_reader = StreamReader::new(stream);
    //let mut stream_reader = response;

    let mut hasher = sha2::Sha256::new();
    let mut cipher = Aes128Ctr64LE::new(key.into(), &chunk_data.iv.into());
    let mut read_buf = vec![0u8; READ_BUF_LEN];
    for file in &chunk_data.files {
        let should_write = file_list
            .get(&file.filename)
            .map(|v| v == version_id)
            .unwrap_or(false);
        let path = path_guard::join_within(base_path, Path::new(&file.filename)).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("server chunk contains unsafe filename {:?}: {e}", file.filename),
            )
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file_handle = if should_write {
            let mut file_handle = tokio::fs::OpenOptions::new()
                .truncate(false)
                .write(true)
                .append(false)
                .create(true)
                .open(&path)
                .await?;
            file_handle.seek(SeekFrom::Start(file.start.try_into().unwrap())).await?;
            Some(file_handle)
        } else {
            None
        };

        let mut remaining = file.length;
        while remaining > 0 {
            // Check the pause flag on every read buffer (1 MB) rather than
            // only at file boundaries. Without this, pausing a chunk that
            // contains one large file (the common case) takes until the
            // entire file finishes before the chunk actually stops — which
            // for a 200 MB file on a 10 MB/s connection is a 20-second
            // pause delay. The chunk's partial bytes already on disk are
            // harmless: it isn't marked complete in .dropdata, so on
            // resume it re-downloads from byte 0 and overwrites them.
            if control_flag.get() == DownloadThreadControlFlag::Stop {
                download_progress.set(0);
                disk_progress.set(0);
                return Ok(false);
            }
            let amount = stream_reader.read(&mut read_buf[0..remaining.min(READ_BUF_LEN)]).await?;
            if amount == 0 {
                // Stream closed before delivering the chunk's bytes. Without this
                // guard `remaining -= 0` would spin forever; surface a retryable IO
                // error so the chunk gets re-fetched (mirrors validate.rs).
                return Err(ApplicationDownloadError::IoError(std::sync::Arc::new(
                    std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "download stream ended before the chunk was fully received",
                    ),
                )));
            }
            download_progress.add(amount);
            remaining -= amount;

            cipher.apply_keystream(&mut read_buf[0..amount]);
            hasher.update(&read_buf[0..amount]);
            if let Some(file_handle) = &mut file_handle {
                file_handle.write_all(&read_buf[0..amount]).await?;
                disk_progress.add(amount);
            }
        }

        // Only for a file this call wrote: one it skipped may not exist here
        // (the in-place updater writes a chunk's selected files into an
        // empty staging folder) and belongs to another version otherwise.
        #[cfg(unix)]
        if should_write {
            drop(file_handle);
            let permissions = if file.permissions == 0 {
                0o744
            } else {
                file.permissions
            };
            let permissions = Permissions::from_mode(permissions);
            set_permissions(path, permissions)
                .map_err(|e| ApplicationDownloadError::IoError(Arc::new(e)))?;
        }

        if control_flag.get() == DownloadThreadControlFlag::Stop {
            download_progress.set(0);
            return Ok(false);
        }
    }

    let digest = hex::encode(hasher.finalize());
    if digest != chunk_data.checksum {
        return Err(ApplicationDownloadError::Checksum);
    }

    Ok(true)
}

/// The size each file in `info` ends up, from the end of its last chunk entry
/// in the manifest of the version that ships it (`file_list`).
///
/// Only complete for a manifest fetched WITHOUT `previous`: a delta leaves out
/// chunks that hold only files unchanged since that version, so a large
/// unchanged file can look shorter than it is.
pub fn manifest_file_sizes(info: &DownloadInformation) -> HashMap<String, u64> {
    let mut sizes: HashMap<String, u64> = HashMap::new();
    for (version_id, manifest) in &info.manifests {
        for chunk in manifest.chunks.values() {
            for f in &chunk.files {
                if info.file_list.get(&f.filename) != Some(version_id) {
                    continue;
                }
                let end = (f.start as u64).saturating_add(f.length as u64);
                let size = sizes.entry(f.filename.clone()).or_insert(0);
                *size = (*size).max(end);
            }
        }
    }
    sizes
}

/// The files a download of `info` writes: every chunk entry whose file this
/// version ships (`download_game_chunk` skips the rest).
pub fn files_written_by(info: &DownloadInformation) -> HashSet<String> {
    info.manifests
        .iter()
        .flat_map(|(version_id, manifest)| {
            manifest.chunks.values().flat_map(move |chunk| {
                chunk
                    .files
                    .iter()
                    .filter(move |f| info.file_list.get(&f.filename) == Some(version_id))
                    .map(|f| f.filename.clone())
            })
        })
        .collect()
}

/// Cut every file in `written` back to its manifest size (`sizes`, from
/// `manifest_file_sizes` over a FULL manifest) when it is longer.
///
/// `download_game_chunk` writes into an existing file in place, without
/// truncating it, because one file can span several chunks written in any
/// order. So when an update rewrites a file that got smaller (a mod's own
/// file from its previous version, another mod's file it overwrites, or a
/// base-game file updated in place), the old file's tail is left after the
/// new content. Validation only checks that a file is long enough, so this is
/// the step that removes it. Run once every chunk is on disk.
///
/// A file with no known size, or whose folder leads out of `base_path`
/// through a symlink (an SD-card link on the Deck, say), is left alone and
/// reported in the log. Only an error opening or cutting a file that needs it
/// fails. Returns how many files were cut.
pub fn trim_stale_tails(
    base_path: &Path,
    written: &HashSet<String>,
    sizes: &HashMap<String, u64>,
) -> Result<usize, std::io::Error> {
    // Resolved only once a file actually needs cutting.
    let mut base_real: Option<std::path::PathBuf> = None;
    let mut trimmed = 0;
    for name in written {
        let Some(&size) = sizes.get(name) else {
            log::warn!("no size for {name} in the full manifest; not checking it for a stale tail");
            continue;
        };
        let Ok(path) = path_guard::join_within(base_path, Path::new(name)) else {
            // download_game_chunk refuses this name, so it was never written.
            continue;
        };
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                // Missing: validation reports it. Anything else: it can't be
                // looked at, so it isn't touched.
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("not checking {} for a stale tail: {e}", path.display());
                }
                continue;
            }
        };
        if !meta.is_file() || meta.len() <= size {
            continue;
        }
        let root = match &base_real {
            Some(r) => r,
            None => base_real.insert(base_path.canonicalize()?),
        };
        if path_guard::ensure_parent_within(root, &path).is_err() {
            log::warn!(
                "{} is {} bytes, more than the {size} it should be, but its folder leads outside {}; not cutting it",
                path.display(),
                meta.len(),
                base_path.display()
            );
            continue;
        }
        std::fs::OpenOptions::new().write(true).open(&path)?.set_len(size)?;
        debug!("cut stale tail from {} ({} -> {size} bytes)", path.display(), meta.len());
        trimmed += 1;
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use droplet_rs::manifest::{FileEntry, Manifest};
    use std::io::{Seek, Write};

    fn entry(name: &str, start: usize, length: usize) -> FileEntry {
        FileEntry {
            filename: name.to_string(),
            start,
            length,
            permissions: 0,
        }
    }

    fn info(version: &str, chunks: Vec<(&str, Vec<FileEntry>)>) -> DownloadInformation {
        let mut file_list = HashMap::new();
        let chunks: HashMap<String, ChunkData> = chunks
            .into_iter()
            .map(|(id, files)| {
                for f in &files {
                    file_list.insert(f.filename.clone(), version.to_string());
                }
                (
                    id.to_string(),
                    ChunkData {
                        files,
                        checksum: String::new(),
                        iv: [0; 16],
                    },
                )
            })
            .collect();
        let mut manifests = HashMap::new();
        manifests.insert(
            version.to_string(),
            Manifest {
                version: version.to_string(),
                chunks,
                size: 0,
                key: [0; 16],
            },
        );
        DownloadInformation {
            file_list,
            manifests,
            install_size: 0,
            download_size: 0,
            revision: None,
        }
    }

    /// Exactly how `download_game_chunk` writes a file region: open without
    /// truncating, seek to the entry's start, write.
    fn write_in_place(path: &Path, start: u64, bytes: &[u8]) {
        let mut f = std::fs::OpenOptions::new()
            .truncate(false)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        f.seek(std::io::SeekFrom::Start(start)).unwrap();
        f.write_all(bytes).unwrap();
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("drop-trim-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_shorter_rewritten_file_has_no_stale_tail() {
        let dir = scratch("shorter");
        let file = dir.join("config.json");
        // Version 1 of the mod wrote a long file.
        write_in_place(&file, 0, b"{\"old\": \"a much longer first version\"}");
        // Version 2 rewrites it in place with shorter content, across two
        // chunks written out of order.
        let v2 = b"{\"new\": 1}";
        write_in_place(&file, 5, &v2[5..]);
        write_in_place(&file, 0, &v2[..5]);
        let tail = std::fs::read(&file).unwrap();
        assert!(tail.len() > v2.len(), "the in-place write left the old tail");

        let manifest = info(
            "v2",
            vec![
                ("c2", vec![entry("config.json", 5, v2.len() - 5)]),
                ("c1", vec![entry("config.json", 0, 5)]),
            ],
        );
        let sizes = manifest_file_sizes(&manifest);
        assert_eq!(sizes.get("config.json"), Some(&(v2.len() as u64)));
        let written = files_written_by(&manifest);
        assert_eq!(trim_stale_tails(&dir, &written, &sizes).unwrap(), 1);
        assert_eq!(std::fs::read(&file).unwrap(), v2);
        // Running it again changes nothing.
        assert_eq!(trim_stale_tails(&dir, &written, &sizes).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn files_this_download_does_not_write_are_left_alone() {
        let dir = scratch("untouched");
        std::fs::write(dir.join("other.dll"), b"another mod's longer file").unwrap();
        std::fs::write(dir.join("mine.dll"), b"exact").unwrap();
        let mut manifest = info("v2", vec![("c1", vec![entry("mine.dll", 0, 5), entry("other.dll", 0, 3)])]);
        // other.dll ships in an older version: this download skips it.
        manifest.file_list.insert("other.dll".to_string(), "v1".to_string());
        let sizes = manifest_file_sizes(&manifest);
        let written = files_written_by(&manifest);
        assert!(!written.contains("other.dll"));
        assert_eq!(trim_stale_tails(&dir, &written, &sizes).unwrap(), 0);
        assert_eq!(std::fs::read(dir.join("other.dll")).unwrap(), b"another mod's longer file");
        assert_eq!(std::fs::read(dir.join("mine.dll")).unwrap(), b"exact");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_without_a_known_size_is_not_cut() {
        let dir = scratch("unknown");
        std::fs::write(dir.join("big.pak"), b"0123456789").unwrap();
        let written: HashSet<String> = ["big.pak".to_string()].into_iter().collect();
        assert_eq!(trim_stale_tails(&dir, &written, &HashMap::new()).unwrap(), 0);
        assert_eq!(std::fs::read(dir.join("big.pak")).unwrap().len(), 10);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn never_cuts_a_file_reached_through_a_symlink_out_of_the_folder() {
        let dir = scratch("symlink");
        let outside = scratch("symlink-outside");
        std::fs::write(outside.join("save.dat"), b"precious save data").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("Saves")).unwrap();
        let written: HashSet<String> = ["Saves/save.dat".to_string()].into_iter().collect();
        let sizes: HashMap<String, u64> = [("Saves/save.dat".to_string(), 2)].into_iter().collect();
        assert_eq!(trim_stale_tails(&dir, &written, &sizes).unwrap(), 0);
        assert_eq!(std::fs::read(outside.join("save.dat")).unwrap(), b"precious save data");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
