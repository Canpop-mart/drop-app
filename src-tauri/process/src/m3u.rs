use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ProcessError;

/// Prefix for temporary .m3u files so they can be identified and cleaned up.
const M3U_PREFIX: &str = ".drop-";

/// Generate a temporary .m3u playlist file for multi-disc games.
///
/// The file is written to `game_install_dir/.drop-{game_id}.m3u` and contains
/// one absolute path per line, pointing to each disc image in order.
///
/// Returns the absolute path to the generated .m3u file.
pub fn generate_m3u(
    game_install_dir: &Path,
    game_id: &str,
    disc_paths: &[String],
) -> Result<PathBuf, ProcessError> {
    let m3u_filename = format!("{}{}.m3u", M3U_PREFIX, game_id);
    let m3u_path = game_install_dir.join(&m3u_filename);

    let contents: Vec<String> = disc_paths
        .iter()
        .map(|p| {
            let full = game_install_dir.join(p);
            full.to_string_lossy().to_string()
        })
        .collect();

    fs::write(&m3u_path, contents.join("\n")).map_err(|e| {
        ProcessError::FormatError(format!(
            "Failed to write .m3u playlist at {}: {}",
            m3u_path.display(),
            e
        ))
    })?;

    log::info!(
        "Generated .m3u playlist at {} with {} disc(s)",
        m3u_path.display(),
        disc_paths.len()
    );

    Ok(m3u_path)
}

/// Clean up any temporary .m3u files from previous launches in the given directory.
pub fn cleanup_m3u(game_install_dir: &Path) {
    if let Ok(entries) = fs::read_dir(game_install_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(M3U_PREFIX) && name_str.ends_with(".m3u") {
                if let Err(e) = fs::remove_file(entry.path()) {
                    log::warn!("Failed to clean up {}: {}", entry.path().display(), e);
                }
            }
        }
    }
}
