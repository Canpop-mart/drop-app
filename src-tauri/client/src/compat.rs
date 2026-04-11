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

fn get_umu_executable() -> Option<PathBuf> {
    // First check if umu-run is on PATH using `which`
    if let Ok(output) = Command::new("which")
        .arg(UMU_BASE_LAUNCHER_EXECUTABLE)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                info!("Found umu-run on PATH: {}", path);
                return Some(PathBuf::from(path));
            }
        }
    }

    // Fallback: check known installation directories
    for dir in UMU_INSTALL_DIRS {
        let p = PathBuf::from(dir).join(UMU_BASE_LAUNCHER_EXECUTABLE);
        if p.exists() && p.is_file() {
            info!("Found umu-run at: {}", p.display());
            return Some(p);
        }
    }
    None
}
