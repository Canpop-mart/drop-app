use std::{
    fs::{self, File, create_dir_all},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use crate::error::BackupError;

use super::{backup_manager::BackupHandler, placeholder::*};
use database::GameVersion;
use log::{debug, warn};
use rustix::path::Arg;
use tempfile::tempfile;

use super::{backup_manager::BackupManager, metadata::CloudSaveMetadata, normalise::normalize};

pub fn resolve(meta: &mut CloudSaveMetadata) -> Result<File, BackupError> {
    let f = File::create_new("save").map_err(|e| BackupError::IoError(format!("Failed to create save file: {e}")))?;
    let compressor = zstd::Encoder::new(f, 22).map_err(|e| BackupError::IoError(format!("Failed to create zstd encoder: {e}")))?;
    let mut tarball = tar::Builder::new(compressor);
    let manager = BackupManager::new();
    for file in meta.files.iter_mut() {
        let id = uuid::Uuid::new_v4().to_string();
        let os = match file
            .conditions
            .iter()
            .find_map(|p| match p {
                super::conditions::Condition::Os(os) => Some(os),
                _ => None
            })
            .cloned()
        {
            Some(os) => os,
            None => {
                warn!(
                    "File {:?} could not be backed up because it did not provide an OS",
                    &file
                );
                continue;
            }
        };
        let handler = match manager.sources.get(&(manager.current_platform, os)) {
            Some(h) => *h,
            None => continue,
        };
        let t_path = PathBuf::from(normalize(&file.path, os));
        debug!("resolve: normalized path: {:?}", &t_path);
        let path = match parse_path(t_path, handler, &meta.game_version) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to parse path for file {:?}: {}", &file, e);
                continue;
            }
        };
        let f = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to read metadata for {:?}: {}", &path, e);
                continue;
            }
        };
        if f.is_dir() {
            if let Err(e) = tarball.append_dir_all(&id, &path) {
                warn!("Failed to append directory {:?} to tarball: {}", &path, e);
                continue;
            }
        } else if f.is_file() {
            match File::open(&path) {
                Ok(mut opened) => {
                    if let Err(e) = tarball.append_file(&id, &mut opened) {
                        warn!("Failed to append file {:?} to tarball: {}", &path, e);
                        continue;
                    }
                }
                Err(e) => {
                    warn!("Failed to open file {:?}: {}", &path, e);
                    continue;
                }
            }
        }
        file.id = Some(id);
    }
    let serialized = serde_json::to_vec(meta)?;
    let mut file = tempfile().map_err(|e| BackupError::IoError(format!("Failed to create temp file: {e}")))?;
    file.write_all(&serialized)?;
    tarball.append_file("metadata", &mut file)?;
    let encoder = tarball.into_inner().map_err(|e| BackupError::IoError(format!("Failed to finish tarball: {e}")))?;
    let finished = encoder.finish().map_err(|e| BackupError::IoError(format!("Failed to finish zstd encoder: {e}")))?;
    Ok(finished)
}

pub fn extract(file: PathBuf) -> Result<(), BackupError> {
    let tmpdir = tempfile::tempdir().map_err(|e| BackupError::IoError(format!("Failed to create temp dir: {e}")))?;

    // Reopen the file for reading
    let file = File::open(file)?;

    let decompressor = zstd::Decoder::new(file).map_err(|e| BackupError::IoError(format!("Failed to create zstd decoder: {e}")))?;
    let mut f = tar::Archive::new(decompressor);
    f.unpack(tmpdir.path())?;

    let path = tmpdir.path();

    let mut manifest = File::open(path.join("metadata"))?;

    let mut manifest_slice = Vec::new();
    manifest.read_to_end(&mut manifest_slice)?;

    let manifest: CloudSaveMetadata = serde_json::from_slice(&manifest_slice)?;

    for file in manifest.files {
        let file_id = match file.id.as_ref() {
            Some(id) => id,
            None => {
                warn!("Skipping file with no ID in manifest: {:?}", &file);
                continue;
            }
        };
        let current_path = path.join(file_id);

        let manager = BackupManager::new();
        let os = match file
            .conditions
            .iter()
            .find_map(|p| match p {
                super::conditions::Condition::Os(os) => Some(os),
                _ => None
            })
            .cloned()
        {
            Some(os) => os,
            None => {
                warn!(
                    "File {:?} could not be replaced up because it did not provide an OS",
                    &file
                );
                continue;
            }
        };
        let handler = match manager.sources.get(&(manager.current_platform, os)) {
            Some(h) => *h,
            None => continue,
        };

        let new_path = parse_path(file.path.into(), handler, &manifest.game_version)?;
        if let Some(parent) = new_path.parent() {
            create_dir_all(parent)?;
        }

        debug!(
            "Current path {:?} copying to {:?}",
            &current_path, &new_path
        );

        copy_item(current_path, new_path)?;
    }

    Ok(())
}

pub fn copy_item<P: AsRef<Path>>(src: P, dest: P) -> io::Result<()> {
    let src_path = src.as_ref();
    let dest_path = dest.as_ref();

    let metadata = fs::metadata(src_path)?;

    if metadata.is_file() {
        // Ensure the parent directory of the destination exists for a file copy
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src_path, dest_path)?;
    } else if metadata.is_dir() {
        // For directories, we call the recursive helper function.
        // The destination for the recursive copy is the `dest_path` itself.
        copy_dir_recursive(src_path, dest_path)?;
    } else {
        // Handle other file types like symlinks if necessary,
        // for now, return an error or skip.
        return Err(io::Error::other(
            format!("Source {:?} is neither a file nor a directory", src_path),
        ));
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let entry_file_name = match entry_path.file_name() {
            Some(name) => name,
            None => continue, // Skip if somehow there's no file name
        };
        let dest_entry_path = dest.join(entry_file_name);
        let metadata = entry.metadata()?;

        if metadata.is_file() {
            debug!(
                "Writing file {} to {}",
                entry_path.display(),
                dest_entry_path.display()
            );
            fs::copy(&entry_path, &dest_entry_path)?;
        } else if metadata.is_dir() {
            copy_dir_recursive(&entry_path, &dest_entry_path)?;
        }
        // Ignore other types like symlinks for this basic implementation
    }

    Ok(())
}

pub fn parse_path(
    path: PathBuf,
    backup_handler: &dyn BackupHandler,
    game: &GameVersion,
) -> Result<PathBuf, BackupError> {
    debug!("Parsing: {:?}", &path);
    let mut s = PathBuf::new();
    for component in path.components() {
        let component_str = component.as_str().unwrap_or_else(|| {
            component.as_os_str().to_str().unwrap_or_default()
        });
        match component_str {
            ROOT => s.push(backup_handler.root_translate(&path, game)?),
            GAME => s.push(backup_handler.game_translate(&path, game)?),
            BASE => s.push(backup_handler.base_translate(&path, game)?),
            HOME => s.push(backup_handler.home_translate(&path, game)?),
            STORE_USER_ID => s.push(backup_handler.store_user_id_translate(&path, game)?),
            OS_USER_NAME => s.push(backup_handler.os_user_name_translate(&path, game)?),
            WIN_APP_DATA => s.push(backup_handler.win_app_data_translate(&path, game)?),
            WIN_LOCAL_APP_DATA => s.push(backup_handler.win_local_app_data_translate(&path, game)?),
            WIN_LOCAL_APP_DATA_LOW => {
                s.push(backup_handler.win_local_app_data_low_translate(&path, game)?)
            }
            WIN_DOCUMENTS => s.push(backup_handler.win_documents_translate(&path, game)?),
            WIN_PUBLIC => s.push(backup_handler.win_public_translate(&path, game)?),
            WIN_PROGRAM_DATA => s.push(backup_handler.win_program_data_translate(&path, game)?),
            WIN_DIR => s.push(backup_handler.win_dir_translate(&path, game)?),
            XDG_DATA => s.push(backup_handler.xdg_data_translate(&path, game)?),
            XDG_CONFIG => s.push(backup_handler.xdg_config_translate(&path, game)?),
            SKIP => s.push(backup_handler.skip_translate(&path, game)?),
            _ => s.push(PathBuf::from(component.as_os_str())),
        }
    }

    debug!("Final line: {:?}", &s);
    Ok(s)
}
