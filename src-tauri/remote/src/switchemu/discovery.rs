//! Switch-emulator install discovery and portable-mode setup.
//!
//! Mirrors [`crate::retroarch::discovery`]: decide from the install directory
//! alone what emulator lives there, and resolve the config path Drop is
//! allowed to write.
//!
//! Only the **yuzu family** (Eden, yuzu, Citron, Sudachi, Suyu — all forks of
//! the same codebase, all reading `qt-config.ini` via the shared
//! `frontend_common` config writer) is supported. Ryujinx is a completely
//! different code base with a completely different config format, so it is
//! detected as its own variant and deliberately left alone; writing a yuzu
//! config into a Ryujinx install would do nothing except look like it worked.

use log::{debug, info};
use std::path::{Path, PathBuf};

/// Executable stems that identify a yuzu-family Switch emulator.
///
/// Matched case-insensitively against the file stem, so `Eden.exe`, `eden`,
/// `yuzu.exe` and `Citron` all hit.
const YUZU_FAMILY_STEMS: &[&str] = &["eden", "yuzu", "citron", "sudachi", "suyu"];

/// Executable stems that identify Ryujinx. `Ryujinx.Ava` / `Ryujinx.Headless`
/// ship alongside the main binary in some builds, hence the prefix match.
const RYUJINX_STEMS: &[&str] = &["ryujinx"];

/// The portable-mode directory name. Its mere *existence* is what puts a
/// yuzu-family emulator into portable mode — there is no CLI flag.
/// (`PORTABLE_DIR` — eden `src/common/fs/fs_paths.h:11-23`.)
pub const PORTABLE_DIR: &str = "user";

/// Which Switch emulator an install directory holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchEmuFamily {
    /// Eden / yuzu / Citron / Sudachi / Suyu — one config format, supported.
    YuzuLike {
        /// Lower-cased executable stem that matched, e.g. `"eden"`.
        flavour: String,
        /// The executable that matched.
        executable: PathBuf,
    },
    /// Ryujinx — detected so the logs are honest, but **not** written to.
    Ryujinx { executable: PathBuf },
}

impl SwitchEmuFamily {
    /// Short label for logs and launch traces.
    pub fn label(&self) -> &str {
        match self {
            SwitchEmuFamily::YuzuLike { flavour, .. } => flavour,
            SwitchEmuFamily::Ryujinx { .. } => "ryujinx",
        }
    }
}

/// Returns the Switch emulator installed in `emu_root`, if any.
///
/// Scans the top level of the directory only — every one of these emulators
/// ships its launcher next to its data, and a recursive scan would risk
/// matching a bundled tool. Recognises Windows `.exe`, extension-less Linux
/// binaries and Linux `.AppImage` builds.
pub fn detect_switch_emulator(emu_root: &Path) -> Option<SwitchEmuFamily> {
    let entries = std::fs::read_dir(emu_root).ok()?;
    let mut ryujinx: Option<PathBuf> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let stem = Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = Path::new(&name)
            .extension()
            .map(|s| s.to_string_lossy().to_string());

        let launcher_like = matches!(ext.as_deref(), None | Some("exe") | Some("appimage"));
        if !launcher_like {
            continue;
        }

        if let Some(flavour) = YUZU_FAMILY_STEMS.iter().find(|f| {
            // `.AppImage` builds carry a decorated name ("Eden-Linux-x86_64"),
            // so those match on prefix; plain binaries must match exactly or
            // an unrelated "edenutils.exe" would be treated as the emulator.
            if ext.as_deref() == Some("appimage") {
                stem.starts_with(*f)
            } else {
                stem == **f
            }
        }) {
            info!(
                "[SWITCHEMU] Detected {flavour} at {} (yuzu family)",
                path.display()
            );
            return Some(SwitchEmuFamily::YuzuLike {
                flavour: (*flavour).to_string(),
                executable: path,
            });
        }

        // Keep looking for a yuzu-family binary before settling on Ryujinx:
        // some multi-emulator install dirs hold both.
        if ryujinx.is_none() && RYUJINX_STEMS.iter().any(|r| stem.starts_with(*r)) {
            ryujinx = Some(path);
        }
    }

    if let Some(executable) = ryujinx {
        info!("[SWITCHEMU] Detected Ryujinx at {}", executable.display());
        return Some(SwitchEmuFamily::Ryujinx { executable });
    }

    if let Ok(entries) = std::fs::read_dir(emu_root) {
        let files: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        debug!("[SWITCHEMU] No Switch emulator in {emu_root:?}, contents: {files:?}");
    }
    None
}

/// Creates `<emu_root>/user/` if it does not exist, putting the emulator into
/// portable mode so its config and NAND live inside the Drop-managed install
/// instead of `%APPDATA%` / `~/.local/share` — which on Linux would land in a
/// disposable Proton prefix.
///
/// An empty directory is enough: the emulator creates `config/` itself
/// (`Config::Initialize` → `CreateParentDir`, eden
/// `src/frontend_common/config.cpp:30-40`).
///
/// **Linux caveat**: portable mode there is resolved from the process working
/// directory, not the executable directory (eden
/// `src/common/fs/path_util.cpp:134`). The launcher must therefore spawn the
/// emulator with its cwd set to `emu_root`, or this directory is ignored.
pub fn ensure_portable_dir(emu_root: &Path) -> std::io::Result<PathBuf> {
    let dir = emu_root.join(PORTABLE_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Path of the portable `qt-config.ini`: `<emu_root>/user/config/qt-config.ini`.
///
/// Derivation: `QtConfig`'s default `config_name` is `"qt-config"`
/// (eden `src/qt_common/config/qt_config.h:15`) and `Config::Initialize`
/// resolves it to `GetEdenPath(ConfigDir) / "qt-config.ini"`
/// (`src/frontend_common/config.cpp:30-40`), where `ConfigDir` is
/// `<portable root>/config` (`src/common/fs/path_util.cpp:120,168`).
pub fn qt_config_path(emu_root: &Path) -> PathBuf {
    emu_root
        .join(PORTABLE_DIR)
        .join("config")
        .join("qt-config.ini")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"").unwrap();
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "drop-switchemu-disc-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detects_eden_exe_case_insensitively() {
        let dir = tmpdir("eden");
        touch(&dir, "Eden.exe");
        match detect_switch_emulator(&dir) {
            Some(SwitchEmuFamily::YuzuLike { flavour, .. }) => assert_eq!(flavour, "eden"),
            other => panic!("expected Eden, got {other:?}"),
        }
    }

    #[test]
    fn detects_extensionless_linux_binary() {
        let dir = tmpdir("citron");
        touch(&dir, "citron");
        assert!(matches!(
            detect_switch_emulator(&dir),
            Some(SwitchEmuFamily::YuzuLike { .. })
        ));
    }

    #[test]
    fn detects_ryujinx_as_its_own_variant() {
        let dir = tmpdir("ryu");
        touch(&dir, "Ryujinx.exe");
        assert!(matches!(
            detect_switch_emulator(&dir),
            Some(SwitchEmuFamily::Ryujinx { .. })
        ));
    }

    #[test]
    fn ignores_unrelated_executables() {
        let dir = tmpdir("none");
        touch(&dir, "edenutils.exe");
        touch(&dir, "readme.txt");
        assert_eq!(detect_switch_emulator(&dir), None);
    }

    #[test]
    fn portable_dir_is_created_once() {
        let dir = tmpdir("portable");
        let user = ensure_portable_dir(&dir).unwrap();
        assert!(user.is_dir());
        // Second call must not fail on the existing directory.
        assert!(ensure_portable_dir(&dir).is_ok());
        assert_eq!(qt_config_path(&dir), user.join("config").join("qt-config.ini"));
    }
}
