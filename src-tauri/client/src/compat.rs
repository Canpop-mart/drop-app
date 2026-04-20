use std::{
    ffi::OsStr,
    path::PathBuf,
    process::{Command, Stdio},
    sync::LazyLock,
};

use log::info;

pub static COMPAT_INFO: LazyLock<Option<CompatInfo>> = LazyLock::new(create_new_compat_info);

pub static UMU_LAUNCHER_EXECUTABLE: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    let x = get_umu_executable();
    info!("{:?}", &x);
    x
});

#[derive(Clone)]
pub struct CompatInfo {
    pub umu_installed: bool,
}

fn create_new_compat_info() -> Option<CompatInfo> {
    #[cfg(target_os = "windows")]
    return None;

    let has_umu_installed = UMU_LAUNCHER_EXECUTABLE.is_some();
    Some(CompatInfo {
        umu_installed: has_umu_installed,
    })
}

const UMU_BASE_LAUNCHER_EXECUTABLE: &str = "umu-run";
const UMU_INSTALL_DIRS: [&str; 7] = [
    "/usr/bin",
    "/usr/local/bin",
    "/app/share",
    "/usr/local/share",
    "/usr/share",
    "/opt",
    "/home/deck/.local/bin",
];

/// Returns true if the path points to a valid, non-empty, executable file.
/// Follows symlinks to check the real file. This catches broken installs
/// where `~/.local/bin/umu-run` is a 0-byte placeholder or dangling symlink.
fn is_valid_executable(path: &PathBuf) -> bool {
    // Resolve symlinks to find the real file
    let resolved = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            info!(
                "[UMU] Cannot resolve {:?}: {} (dangling symlink?)",
                path, e
            );
            return false;
        }
    };

    // Check the real file exists and is non-empty
    match std::fs::metadata(&resolved) {
        Ok(meta) => {
            if meta.len() == 0 {
                info!(
                    "[UMU] {:?} (resolved to {:?}) is 0 bytes — broken install",
                    path, resolved
                );
                return false;
            }
            if !meta.is_file() {
                info!("[UMU] {:?} is not a regular file", resolved);
                return false;
            }
            // On Unix, check execute permission
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = meta.permissions().mode();
                if mode & 0o111 == 0 {
                    info!("[UMU] {:?} is not executable (mode {:o})", resolved, mode);
                    return false;
                }
            }
            true
        }
        Err(e) => {
            info!("[UMU] Cannot stat {:?}: {}", resolved, e);
            false
        }
    }
}

/// Re-run umu-run detection without the LazyLock cache.
/// Used after installing umu-launcher at runtime.
pub fn get_umu_executable_fresh() -> Option<PathBuf> {
    get_umu_executable()
}

fn get_umu_executable() -> Option<PathBuf> {
    // Build a list of candidate paths from multiple sources, then validate
    // each one. This handles the common Steam Deck case where
    // ~/.local/bin/umu-run is a 0-byte broken file (from a failed pipx
    // install or SteamOS update) but the real executable lives in the
    // pipx venv at ~/.local/share/pipx/venvs/umu-launcher/bin/umu-run.

    let mut candidates: Vec<PathBuf> = Vec::new();

    // 0. Bundled copy inside AppImage — highest priority.
    //    The AppImage runtime sets APPDIR to the extracted mount point.
    //    Our CI bundles umu-run at $APPDIR/usr/bin/umu-run.
    if let Some(appdir) = std::env::var_os("APPDIR") {
        candidates.push(PathBuf::from(&appdir).join("usr/bin/umu-run"));
    }

    // 1. pipx venv paths FIRST — these are most likely to have the real
    //    executable on Steam Deck, even when the ~/.local/bin shim is broken.
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(
            home.join(".local/share/pipx/venvs/umu-launcher/bin/umu-run"),
        );
        candidates.push(
            home.join(".local/share/pipx/venvs/umu-run/bin/umu-run"),
        );
    }

    // 2. Check `which` output (may return a broken shim, validated below)
    if let Ok(output) = Command::new("which")
        .arg(UMU_BASE_LAUNCHER_EXECUTABLE)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                candidates.push(PathBuf::from(path));
            }
        }
    }

    // 3. Known system directories
    for dir in UMU_INSTALL_DIRS {
        candidates.push(PathBuf::from(dir).join(UMU_BASE_LAUNCHER_EXECUTABLE));
    }

    // 4. Flatpak paths
    candidates.push(PathBuf::from("/app/bin/umu-run"));

    // Validate each candidate and return the first valid one
    for candidate in &candidates {
        if candidate.exists() && is_valid_executable(candidate) {
            // Resolve to the real path to avoid broken shims at runtime
            let resolved = candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone());
            info!(
                "[UMU] Found valid umu-run at: {} (resolved: {})",
                candidate.display(),
                resolved.display()
            );
            return Some(resolved);
        }
    }

    // Log all candidates we checked for debugging
    info!(
        "[UMU] No valid umu-run found. Checked {} candidates:",
        candidates.len()
    );
    for c in &candidates {
        let exists = c.exists();
        let resolved = c.canonicalize().ok();
        let size = c.metadata().ok().map(|m| m.len());
        info!(
            "[UMU]   {} — exists={}, size={:?}, resolved={:?}",
            c.display(),
            exists,
            size,
            resolved
        );
    }

    None
}
