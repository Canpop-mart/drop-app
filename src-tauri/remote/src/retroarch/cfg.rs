//! RetroArch config-file primitives.
//!
//! RetroArch's config format is line-oriented `key = "value"`. Drop never
//! rewrites a config wholesale — it *patches* it: existing keys it cares
//! about are replaced, stale keys are deleted, everything else is left
//! untouched. This keeps any user-made changes that Drop doesn't manage.
//!
//! Patching is idempotent: running it twice produces the same file.

use log::debug;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// How paths written into a config file must be spelled for the RetroArch
/// build that will read them.
///
/// Drop's emulator installs are frequently the **Windows** `retroarch.exe`
/// running under Proton/umu on Linux. That build resolves every path through
/// Wine, where a Unix-absolute path is not absolute at all: a leading `/` means
/// "root of the *current* drive". On the Steam Deck the prefix maps
/// `x: -> /home/deck`, and the current drive resolves to X:, so
/// `/home/deck/foo` becomes `X:\home\deck\foo` = `/home/deck/home/deck/foo` —
/// a doubled path that cannot exist. That is why the CRT filter, the system
/// directory and the shader presets all silently failed on the Deck while the
/// identical config worked on native Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PathStyle {
    /// The RetroArch build shares this machine's filesystem view — native
    /// Windows reading Windows paths, native Linux reading Unix paths.
    #[default]
    Native,
    /// Windows RetroArch under Proton/Wine: Unix-absolute paths are rewritten
    /// through the prefix's `Z:` drive.
    Wine,
}

/// Rewrites a host path so Wine resolves it to the same file.
///
/// `Z:` is used rather than any of the other drives a prefix happens to
/// define. Every Wine prefix maps `z: -> /`, so `/home/deck/foo` is always
/// `Z:\home\deck\foo`; picking a narrower drive (the Deck's `x: -> /home/deck`)
/// would depend on drive letters that differ per prefix and per distro.
/// Confirmed against the Deck's own `dosdevices` listing, which has
/// `c:`, `s: -> /home`, `x: -> /home/deck` and `z: -> /` — only `z:` is
/// guaranteed to be there and to cover every absolute path.
///
/// Anything that is not Unix-absolute is returned unchanged:
/// * an already-Windows path (`C:\games`, `Z:\home\deck`) is already correct,
/// * a relative path resolves against RetroArch's own directory, which is what
///   the caller wanted (shader presets rely on this).
pub fn to_wine_path(path: &str) -> String {
    if !path.starts_with('/') {
        return path.to_owned();
    }
    format!("Z:{}", path.replace('/', "\\"))
}

/// Converts a path to a RetroArch config value: wrapped in double quotes and
/// spelled for the build that will read it.
///
/// [`PathStyle::Native`] keeps forward slashes, which RetroArch accepts on
/// every platform. [`PathStyle::Wine`] keeps backslashes after the drive letter
/// because that is the spelling Wine itself emits, and RetroArch's
/// `path_is_absolute` recognises both `":/"` and `":\\"`, so nothing downstream
/// re-resolves it.
pub fn path_to_cfg(path: &Path, style: PathStyle) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = match style {
        PathStyle::Native => s,
        PathStyle::Wine => to_wine_path(&s),
    };
    format!("\"{s}\"")
}

/// Extracts the key from a config line (`key = "value"` or `key = value`).
/// Returns `None` for comments, blank lines, or malformed lines.
pub fn extract_cfg_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    trimmed
        .split('=')
        .next()
        .map(str::trim)
        .filter(|k| !k.is_empty())
}

/// Reads an existing `retroarch.cfg`, applies `overrides`, and writes it back.
/// Creates the file if it does not exist. Only keys present in `overrides`
/// are touched; everything else is preserved verbatim.
pub fn patch_retroarch_cfg(
    cfg_path: &Path,
    overrides: &HashMap<&str, String>,
) -> std::io::Result<()> {
    patch_retroarch_cfg_with_deletions(cfg_path, overrides, &[])
}

/// Like [`patch_retroarch_cfg`] but also removes any line whose key appears
/// in `delete_keys`. Used to clean up stale settings from older Drop versions
/// (e.g. an empty `joypad_autoconfig_dir` that triggers fallback warnings).
///
/// Returns the underlying `fs::write` error if the rewrite fails. Callers
/// are expected to treat that as a hard failure — previously a write error
/// here was silently warn-logged and the launch continued against stale or
/// half-written config, which produced "RA launches and then mysteriously
/// freezes" reports. The orchestrator now aborts RA configuration when this
/// returns Err, so the game either launches with the patches we intended or
/// the user sees a visible failure rather than a degraded random one.
pub fn patch_retroarch_cfg_with_deletions(
    cfg_path: &Path,
    overrides: &HashMap<&str, String>,
    delete_keys: &[&str],
) -> std::io::Result<()> {
    let existing = fs::read_to_string(cfg_path).unwrap_or_default();

    let mut found_keys: HashMap<&str, bool> = overrides.keys().map(|k| (*k, false)).collect();
    let mut lines: Vec<String> = Vec::new();

    for line in existing.lines() {
        let trimmed = line.trim();

        if let Some(key) = extract_cfg_key(trimmed) {
            if delete_keys.contains(&key) {
                debug!("[RETROARCH] Removing stale config key: {key}");
                continue;
            }
            if let Some(value) = overrides.get(key) {
                lines.push(format!("{key} = {value}"));
                found_keys.insert(key, true);
                continue;
            }
        }

        lines.push(line.to_string());
    }

    // Append override keys that weren't already in the file.
    for (key, was_found) in &found_keys {
        if !was_found
            && let Some(value) = overrides.get(key) {
                lines.push(format!("{key} = {value}"));
            }
    }

    let content = lines.join("\n") + "\n";

    fs::write(cfg_path, &content)?;
    debug!("[RETROARCH] Wrote config to {}", cfg_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{path_to_cfg, to_wine_path, PathStyle};
    use std::path::Path;

    /// The exact case from the Deck: a Unix-absolute path handed to a Windows
    /// RetroArch is drive-relative, so it must go through `Z:`.
    #[test]
    fn unix_absolute_becomes_a_z_drive_path() {
        assert_eq!(
            to_wine_path("/home/deck/.local/share/drop/games/emu/1/system"),
            r"Z:\home\deck\.local\share\drop\games\emu\1\system"
        );
    }

    #[test]
    fn windows_paths_are_left_alone() {
        assert_eq!(to_wine_path(r"C:\games\emu\system"), r"C:\games\emu\system");
        assert_eq!(to_wine_path("C:/games/emu/system"), "C:/games/emu/system");
        assert_eq!(to_wine_path(r"Z:\home\deck\emu"), r"Z:\home\deck\emu");
    }

    /// Preset files reference their stages relatively; converting those would
    /// break the one thing that already worked.
    #[test]
    fn relative_paths_are_left_alone() {
        assert_eq!(
            to_wine_path("../shaders_slang/crt/crt-lottes.slangp"),
            "../shaders_slang/crt/crt-lottes.slangp"
        );
        assert_eq!(to_wine_path("shaders/crt.slang"), "shaders/crt.slang");
    }

    #[test]
    fn spaces_and_non_ascii_survive() {
        assert_eq!(
            to_wine_path("/home/deck/My Games/Pokémon Ranger/system"),
            r"Z:\home\deck\My Games\Pokémon Ranger\system"
        );
    }

    #[test]
    fn empty_input_is_not_turned_into_a_drive_root() {
        assert_eq!(to_wine_path(""), "");
    }

    /// Native styling must be byte-for-byte what Drop wrote before the Wine
    /// path existed, so the working Windows and native-Linux cases cannot move.
    #[test]
    fn native_style_is_unchanged() {
        assert_eq!(
            path_to_cfg(Path::new("/home/deck/emu/system"), PathStyle::Native),
            "\"/home/deck/emu/system\""
        );
        assert_eq!(
            path_to_cfg(Path::new(r"C:\games\emu\system"), PathStyle::Native),
            "\"C:/games/emu/system\""
        );
    }

    #[test]
    fn wine_style_quotes_the_converted_path() {
        assert_eq!(
            path_to_cfg(Path::new("/home/deck/emu/system"), PathStyle::Wine),
            "\"Z:\\home\\deck\\emu\\system\""
        );
    }
}
