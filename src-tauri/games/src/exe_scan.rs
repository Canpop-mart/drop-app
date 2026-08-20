//! Find the executables inside a game's install directory, and resolve the
//! user's per-device pick back to a real path at launch time.
//!
//! The server auto-detects a launch command at import, and it is often the
//! wrong binary: the uninstaller, a bundled VC++ redistributable, Unity's
//! crash handler, or a launcher stub that the user would rather skip. Drop's
//! only escape hatch for that used to be typing a literal path into the
//! launch template, which has no validation at all.
//!
//! ## Which files count as executables
//!
//! Keyed off the version's **target platform**, not the host. A Windows game
//! sitting on a Linux filesystem and run through Proton is still a `.exe`, so
//! the host's own conventions are the wrong thing to ask:
//!
//!   - `Platform::Windows` → files ending in `.exe`.
//!   - `Platform::Linux` → the Unix executable bit, plus the extensionless and
//!     `.sh` / `.x86_64` / `.x86` / `.AppImage` / `.run` names that Linux game
//!     builds use. Shared objects and data files are rejected by extension
//!     even when they carry the exec bit (Unity ships plenty of both).
//!     Listing a Linux install *from* Windows cannot read the exec bit, so the
//!     extension rules carry it there.
//!   - `Platform::macOS` → `.app` bundles and exec-bit files.
//!
//! ## Ranking
//!
//! Noise is pushed to the bottom, never removed: a hard filter that guesses
//! wrong on an unusual install would leave the user with an empty list and no
//! way to fix it. Everything else sorts largest-first, because the real game
//! binary is almost always the biggest thing in the directory.

use std::fs;
use std::path::{Path, PathBuf};

use database::platform::Platform;
use serde::Serialize;
use utils::path_guard;

/// How far below the install root to look. Deep enough for the usual
/// `bin/x64/Game.exe`, shallow enough that an install with a huge asset tree
/// does not turn a click into a stall.
const MAX_DEPTH: usize = 4;

/// Stop walking after this many candidate executables. A pathological install
/// (a bundled toolchain, a Wine prefix committed into the game folder) can
/// hold thousands.
const SCAN_BUDGET: usize = 512;

/// How many entries the UI is handed after ranking.
const MAX_RESULTS: usize = 60;

/// One executable found inside a game's install directory.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableCandidate {
    /// Path relative to the install directory, forward-slash separated. This
    /// is the value stored in `UserConfiguration::executable_override`.
    pub relative_path: String,
    pub file_name: String,
    pub size: u64,
    /// True for the entry the game currently launches.
    pub is_current: bool,
    /// True for uninstallers, redistributables and crash handlers. Shown, but
    /// sorted last, so the list is still complete when the guess is wrong.
    pub likely_noise: bool,
}

/// File-name stems that are never the game.
const NOISE_PREFIXES: &[&str] = &[
    "unins",
    "uninstall",
    "vcredist",
    "vc_redist",
    "dxsetup",
    "dxwebsetup",
    "directx",
    "dotnet",
    "ndp4",
    "oalinst",
    "openal",
    "xnafx",
];

/// Substrings anywhere in the file name that mark it as support tooling.
const NOISE_FRAGMENTS: &[&str] = &[
    "crashhandler",
    "crashreport",
    "crashpad",
    "bugreport",
    "errorreport",
    "unitycrash",
];

/// Directory names whose whole subtree is support tooling.
const NOISE_DIRS: &[&str] = &[
    "redist",
    "_commonredist",
    "commonredist",
    "directx",
    "dotnet",
    "vcredist",
    "__installer",
    "__redist",
    "prerequisites",
];

/// Extensions that are not a program even when the exec bit is set.
const NON_PROGRAM_EXTENSIONS: &[&str] = &[
    "so", "dll", "dylib", "pdb", "dat", "pak", "bin", "cfg", "ini", "json", "xml", "txt", "md",
    "log", "sav", "zip", "tar", "gz", "png", "jpg", "jpeg", "ogg", "wav", "mp3", "ttf", "asset",
    "assets", "resource", "resources", "unity3d", "bundle", "lua", "py", "db",
];

/// Extensions a Linux game build uses for its entry point.
const LINUX_PROGRAM_EXTENSIONS: &[&str] = &["sh", "x86_64", "x86", "appimage", "run"];

/// True when a candidate looks like an uninstaller, a redistributable, or a
/// crash reporter rather than the game.
///
/// Takes the forward-slash relative path so a whole `__Installer/` subtree is
/// caught even when the file inside it has an innocent name.
pub fn is_likely_noise(relative_path: &str) -> bool {
    let lower = relative_path.to_ascii_lowercase();
    let mut parts = lower.rsplit('/');
    let file = parts.next().unwrap_or("");
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);

    if NOISE_PREFIXES.iter().any(|p| stem.starts_with(p)) {
        return true;
    }
    if NOISE_FRAGMENTS.iter().any(|f| stem.contains(f)) {
        return true;
    }
    parts.any(|dir| NOISE_DIRS.contains(&dir))
}

/// True when `name` (already lowercased) has one of `exts` as its extension.
fn has_extension(name: &str, exts: &[&str]) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| exts.contains(&ext))
}

/// A `.so.1.2` style versioned shared object does not end in `.so`, so the
/// plain extension check misses it.
fn is_versioned_shared_object(name: &str) -> bool {
    name.contains(".so.")
}

#[cfg(unix)]
fn has_exec_bit(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_exec_bit(_meta: &fs::Metadata) -> bool {
    false
}

/// Whether a regular file is a launch candidate for `platform`.
fn is_program(platform: Platform, name_lower: &str, meta: &fs::Metadata) -> bool {
    match platform {
        Platform::Windows => name_lower.ends_with(".exe"),
        Platform::Linux | Platform::macOS => {
            if is_versioned_shared_object(name_lower)
                || has_extension(name_lower, NON_PROGRAM_EXTENSIONS)
            {
                return false;
            }
            has_exec_bit(meta)
                || has_extension(name_lower, LINUX_PROGRAM_EXTENSIONS)
                || !name_lower.contains('.')
        }
    }
}

/// Sort the candidates the way the picker shows them and cut the list to a
/// length a controller can walk: the entry in use first, then real binaries
/// largest-first, then the support tooling.
pub fn rank_candidates(mut candidates: Vec<ExecutableCandidate>) -> Vec<ExecutableCandidate> {
    candidates.sort_by(|a, b| {
        b.is_current
            .cmp(&a.is_current)
            .then(a.likely_noise.cmp(&b.likely_noise))
            .then(b.size.cmp(&a.size))
            .then(a.relative_path.cmp(&b.relative_path))
    });
    candidates.truncate(MAX_RESULTS);
    candidates
}

/// Walk `install_dir` and return every launch candidate for `platform`,
/// ranked. `current` is the relative path the game launches today, used to
/// mark one entry; pass `None` when it could not be worked out.
pub fn scan_executables(
    install_dir: &Path,
    platform: Platform,
    current: Option<&str>,
) -> Vec<ExecutableCandidate> {
    let current = current.map(normalise_separators);
    let mut out = Vec::new();
    walk(install_dir, platform, "", 0, &mut out);
    for candidate in &mut out {
        candidate.is_current = current
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case(&candidate.relative_path));
    }
    rank_candidates(out)
}

fn walk(
    dir: &Path,
    platform: Platform,
    prefix: &str,
    depth: usize,
    out: &mut Vec<ExecutableCandidate>,
) {
    if depth > MAX_DEPTH || out.len() >= SCAN_BUDGET {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= SCAN_BUDGET {
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let name_lower = name.to_ascii_lowercase();

        if meta.is_dir() {
            // A macOS .app is a directory the user launches, not one to walk.
            if platform == Platform::macOS && name_lower.ends_with(".app") {
                out.push(ExecutableCandidate {
                    likely_noise: is_likely_noise(&relative),
                    relative_path: relative,
                    file_name: name,
                    size: 0,
                    is_current: false,
                });
                continue;
            }
            walk(&entry.path(), platform, &relative, depth + 1, out);
            continue;
        }

        if !meta.is_file() || !is_program(platform, &name_lower, &meta) {
            continue;
        }
        out.push(ExecutableCandidate {
            likely_noise: is_likely_noise(&relative),
            relative_path: relative,
            file_name: name,
            size: meta.len(),
            is_current: false,
        });
    }
}

/// Why an executable override could not be used.
#[derive(Debug, PartialEq, Eq)]
pub enum OverrideError {
    /// The stored path tried to leave the install directory.
    Escapes,
    /// Nothing at that path any more (game moved, verified, or repaired).
    Missing,
}

impl std::fmt::Display for OverrideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Escapes => write!(f, "path escapes the install directory"),
            Self::Missing => write!(f, "file no longer exists"),
        }
    }
}

/// Turn a stored override into an absolute path inside `install_dir`.
///
/// The stored value is untrusted input as far as this function is concerned —
/// it comes from a local database that another process could have edited — so
/// it goes through `path_guard::join_within`, which rejects `..`, absolute
/// paths and drive prefixes outright.
///
/// `join_within` is purely lexical, so a symlink or junction sitting inside
/// the install directory would still pass it and then resolve to a binary
/// somewhere else entirely. The second check closes that: canonicalise both
/// sides and require the real target to stay inside the install directory.
///
/// The canonical path is used for the *check only*. On Windows canonicalising
/// yields a `\\?\C:\...` verbatim path, and this value goes on to be spliced
/// into a launch string, split by `shell_words`, and in the Proton case handed
/// to umu-run — none of which handle that prefix well. So the plain joined
/// path is what comes back.
///
/// Separators are normalised first: the scanner emits forward slashes, but a
/// value hand-edited on Windows may carry backslashes, and on Linux those
/// would otherwise parse as one long filename.
pub fn resolve_override(install_dir: &Path, stored: &str) -> Result<PathBuf, OverrideError> {
    let normalised = normalise_separators(stored);
    if normalised.is_empty() {
        return Err(OverrideError::Missing);
    }
    let joined = path_guard::join_within(install_dir, Path::new(&normalised))
        .map_err(|_| OverrideError::Escapes)?;
    // `exists`, not `is_file`, so a macOS .app bundle still resolves.
    if !joined.exists() {
        return Err(OverrideError::Missing);
    }
    path_guard::ensure_within(install_dir, &joined).map_err(|_| OverrideError::Escapes)?;
    Ok(joined)
}

/// Forward slashes, no leading `./`. A launch command can write the same path
/// three ways (`bin\Game.exe`, `./Game.exe`, `Game.exe`) and all of them have
/// to compare equal to a scanned candidate.
fn normalise_separators(path: &str) -> String {
    let slashed = path.trim().replace('\\', "/");
    slashed.trim_start_matches("./").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(rel: &str, size: u64) -> ExecutableCandidate {
        ExecutableCandidate {
            file_name: rel.rsplit('/').next().unwrap().to_string(),
            likely_noise: is_likely_noise(rel),
            relative_path: rel.to_string(),
            size,
            is_current: false,
        }
    }

    #[test]
    fn flags_uninstallers_and_redistributables() {
        assert!(is_likely_noise("unins000.exe"));
        assert!(is_likely_noise("Uninstall.exe"));
        assert!(is_likely_noise("vcredist_x64.exe"));
        assert!(is_likely_noise("DXSETUP.exe"));
        assert!(is_likely_noise("UnityCrashHandler64.exe"));
        assert!(is_likely_noise("Game_CrashReporter.exe"));
    }

    #[test]
    fn flags_by_containing_directory() {
        assert!(is_likely_noise("_CommonRedist/vcredist/2019/install.exe"));
        assert!(is_likely_noise("__Installer/setup.exe"));
        assert!(!is_likely_noise("bin/x64/Game.exe"));
    }

    #[test]
    fn leaves_ordinary_binaries_alone() {
        assert!(!is_likely_noise("Game.exe"));
        assert!(!is_likely_noise("bin/Launcher64.exe"));
        // "uninstall" as a prefix, not a substring: a game whose own name
        // happens to contain the word must not be buried.
        assert!(!is_likely_noise("TheUninstaller.exe"));
    }

    #[test]
    fn ranks_current_first_then_size_then_noise() {
        let mut small = candidate("Tool.exe", 200);
        let big = candidate("Game.exe", 900_000);
        let noise = candidate("unins000.exe", 5_000_000);
        small.is_current = true;

        let ranked = rank_candidates(vec![noise, big, small]);
        let order: Vec<&str> = ranked.iter().map(|c| c.relative_path.as_str()).collect();
        // The current pick wins outright, the noisy 5 MB uninstaller sinks
        // below the 900 KB game despite being the largest file present.
        assert_eq!(order, vec!["Tool.exe", "Game.exe", "unins000.exe"]);
    }

    #[test]
    fn ranking_keeps_noise_when_it_is_all_there_is() {
        let ranked = rank_candidates(vec![candidate("unins000.exe", 10)]);
        assert_eq!(ranked.len(), 1);
        assert!(ranked[0].likely_noise);
    }

    #[test]
    fn windows_rule_matches_exe_only() {
        let meta = fs::metadata(std::env::current_dir().unwrap()).unwrap();
        assert!(is_program(Platform::Windows, "game.exe", &meta));
        assert!(!is_program(Platform::Windows, "game.sh", &meta));
        assert!(!is_program(Platform::Windows, "libsteam_api.dll", &meta));
    }

    #[test]
    fn linux_rule_rejects_shared_objects() {
        let meta = fs::metadata(std::env::current_dir().unwrap()).unwrap();
        assert!(!is_program(Platform::Linux, "libfoo.so", &meta));
        assert!(!is_program(Platform::Linux, "libfoo.so.1.2", &meta));
        assert!(!is_program(Platform::Linux, "data.pak", &meta));
        assert!(is_program(Platform::Linux, "start.sh", &meta));
        assert!(is_program(Platform::Linux, "Game.x86_64", &meta));
        assert!(is_program(Platform::Linux, "gamebinary", &meta));
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("drop-exe-scan-{tag}-{nanos}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn resolves_a_nested_override() {
        let dir = TempDir::new("resolve");
        fs::create_dir_all(dir.0.join("bin")).unwrap();
        fs::write(dir.0.join("bin/Game.exe"), b"x").unwrap();

        let resolved = resolve_override(&dir.0, "bin/Game.exe").unwrap();
        assert_eq!(resolved, dir.0.join("bin").join("Game.exe"));
        // Backslashes survive the round trip, so a value typed on Windows and
        // synced to the Deck still resolves.
        assert_eq!(resolve_override(&dir.0, r"bin\Game.exe").unwrap(), resolved);
        assert_eq!(resolve_override(&dir.0, "./bin/Game.exe").unwrap(), resolved);
    }

    #[test]
    fn rejects_an_escape_attempt() {
        let dir = TempDir::new("escape");
        assert_eq!(
            resolve_override(&dir.0, "../../windows/system32/cmd.exe"),
            Err(OverrideError::Escapes)
        );
        assert_eq!(
            resolve_override(&dir.0, "bin/../../outside.exe"),
            Err(OverrideError::Escapes)
        );
    }

    /// Best-effort directory link. A Windows junction needs no privilege, but
    /// a Unix symlink on an odd filesystem can still be refused, so the test
    /// using this skips itself when it returns false.
    fn link_dir(target: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|o| o.status.success())
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (target, link);
            false
        }
    }

    #[test]
    fn rejects_an_override_that_leaves_through_a_link() {
        let dir = TempDir::new("link");
        let outside = TempDir::new("link-outside");
        fs::write(outside.0.join("payload.exe"), b"x").unwrap();
        if !link_dir(&outside.0, &dir.0.join("link")) {
            return;
        }
        // Lexically this sits inside the install directory, so the component
        // rules pass it. Only resolving the reparse point shows that it does
        // not, which is why the check has to canonicalise.
        assert_eq!(
            resolve_override(&dir.0, "link/payload.exe"),
            Err(OverrideError::Escapes)
        );
    }

    #[test]
    fn reports_a_deleted_target_as_missing() {
        let dir = TempDir::new("missing");
        assert_eq!(
            resolve_override(&dir.0, "bin/Gone.exe"),
            Err(OverrideError::Missing)
        );
        assert_eq!(resolve_override(&dir.0, "   "), Err(OverrideError::Missing));
    }

    #[test]
    fn scan_finds_and_marks_the_current_entry() {
        let dir = TempDir::new("scan");
        fs::create_dir_all(dir.0.join("bin")).unwrap();
        fs::write(dir.0.join("bin/Game.exe"), vec![0u8; 4096]).unwrap();
        fs::write(dir.0.join("unins000.exe"), vec![0u8; 8192]).unwrap();
        fs::write(dir.0.join("readme.txt"), b"hi").unwrap();

        let found = scan_executables(&dir.0, Platform::Windows, Some(r"bin\Game.exe"));
        let paths: Vec<&str> = found.iter().map(|c| c.relative_path.as_str()).collect();
        assert_eq!(paths, vec!["bin/Game.exe", "unins000.exe"]);
        assert!(found[0].is_current);
        assert_eq!(found[0].size, 4096);
    }
}
