//! Section-aware INI patching for `qt-config.ini`.
//!
//! [`crate::retroarch::cfg::patch_retroarch_cfg`] cannot be reused here.
//! `retroarch.cfg` is a **flat** `key = "value"` file, so that patcher has no
//! notion of sections: keys it doesn't find get appended to the end of the
//! file. In an INI that puts them under whatever section happens to be last —
//! `player_0_button_a` would land in `[Renderer]` and the emulator would never
//! see it. It also normalises spacing to `key = value`, whereas the yuzu-family
//! writer uses `SetSpaces(false)`, i.e. `key=value`
//! (eden `src/frontend_common/config.cpp:98-101`).
//!
//! So this is a small patcher with the same contract as the RetroArch one:
//! replace the keys Drop owns *in place*, append the missing ones at the end of
//! the right section, and leave every other byte alone. Running it twice
//! produces an identical file.

use log::debug;
use std::fs;
use std::path::Path;

/// Splits an INI line into its key, if it is a `key=value` assignment.
/// Returns `None` for blank lines, comments and section headers.
///
/// Note that keys here legitimately contain a backslash — the emulator's
/// "use the built-in default" flag is a sibling key literally named
/// `<key>\default` (`src/frontend_common/config.cpp:847-867`).
fn ini_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('[')
        || trimmed.starts_with(';')
        || trimmed.starts_with('#')
    {
        return None;
    }
    let key = trimmed.split('=').next()?.trim();
    if key.is_empty() { None } else { Some(key) }
}

/// Returns the section name if `line` is a `[Section]` header.
fn section_header(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    Some(inner.trim())
}

/// Reads `path`, sets every `(key, value)` in `entries` inside `[section]`,
/// and writes the file back. Creates the file and/or the section if missing.
///
/// Everything Drop does not own — other sections, other keys, comments,
/// ordering — survives verbatim. `entries` order is preserved for keys that
/// have to be appended, which is what makes a re-run byte-identical.
pub fn patch_ini_section(
    path: &Path,
    section: &str,
    entries: &[(String, String)],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(path).unwrap_or_default();

    let mut out: Vec<String> = Vec::new();
    let mut written: Vec<bool> = vec![false; entries.len()];
    let mut in_section = false;
    let mut section_seen = false;
    // Index in `out` just past the last non-blank line of the target section,
    // i.e. where appended keys belong. Keeps trailing blank separators between
    // sections intact instead of pushing new keys after them (which would move
    // them into the next section on a later parse).
    let mut append_at: Option<usize> = None;

    for line in existing.lines() {
        if let Some(name) = section_header(line) {
            if in_section {
                in_section = false;
            }
            if name.eq_ignore_ascii_case(section) {
                in_section = true;
                section_seen = true;
            }
            out.push(line.to_string());
            if in_section {
                append_at = Some(out.len());
            }
            continue;
        }

        if in_section {
            if let Some(key) = ini_key(line)
                && let Some(idx) = entries.iter().position(|(k, _)| k == key)
            {
                if written[idx] {
                    // A duplicate of a key we own: the emulator's own reader
                    // keeps the last one, so dropping the earlier copy is what
                    // makes a second run byte-identical.
                    continue;
                }
                written[idx] = true;
                out.push(format!("{}={}", entries[idx].0, entries[idx].1));
                append_at = Some(out.len());
                continue;
            }
            out.push(line.to_string());
            if !line.trim().is_empty() {
                append_at = Some(out.len());
            }
            continue;
        }

        out.push(line.to_string());
    }

    if !section_seen {
        if !out.is_empty() && !out.last().is_some_and(|l| l.trim().is_empty()) {
            out.push(String::new());
        }
        out.push(format!("[{section}]"));
        append_at = Some(out.len());
    }

    let insert_at = append_at.unwrap_or(out.len());
    let pending: Vec<String> = entries
        .iter()
        .enumerate()
        .filter(|(i, _)| !written[*i])
        .map(|(_, (k, v))| format!("{k}={v}"))
        .collect();
    if !pending.is_empty() {
        out.splice(insert_at..insert_at, pending);
    }

    let content = out.join("\n") + "\n";
    fs::write(path, &content)?;
    debug!(
        "[SWITCHEMU] Patched {} key(s) into [{section}] of {}",
        entries.len(),
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpfile(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop-switchemu-ini-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{tag}.ini"));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn creates_file_and_section() {
        let path = tmpfile("create");
        patch_ini_section(&path, "Controls", &entries(&[("player_0_type", "0")])).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert_eq!(out, "[Controls]\nplayer_0_type=0\n");
    }

    #[test]
    fn rewrite_is_idempotent() {
        let path = tmpfile("idempotent");
        let e = entries(&[
            ("player_0_button_a", "engine:sdl,button:1"),
            ("player_0_button_a\\default", "false"),
        ]);
        patch_ini_section(&path, "Controls", &e).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        patch_ini_section(&path, "Controls", &e).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn only_owned_keys_are_touched() {
        let path = tmpfile("preserve");
        std::fs::write(
            &path,
            "[Controls]\nplayer_0_button_a=engine:keyboard,code:65\nvibration_enabled=true\n\n\
             [Renderer]\nbackend=1\n",
        )
        .unwrap();
        patch_ini_section(
            &path,
            "Controls",
            &entries(&[("player_0_button_a", "engine:sdl,button:1")]),
        )
        .unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains("player_0_button_a=engine:sdl,button:1"));
        assert!(!out.contains("engine:keyboard"));
        // Untouched neighbours survive, in place, in their own sections.
        assert!(out.contains("vibration_enabled=true"));
        assert!(out.contains("[Renderer]\nbackend=1"));
    }

    #[test]
    fn appends_inside_target_section_not_at_eof() {
        let path = tmpfile("append");
        std::fs::write(&path, "[Controls]\nvibration_enabled=true\n\n[Renderer]\nbackend=1\n")
            .unwrap();
        patch_ini_section(&path, "Controls", &entries(&[("player_0_type", "0")])).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        let controls = out.find("player_0_type=0").unwrap();
        let renderer = out.find("[Renderer]").unwrap();
        assert!(controls < renderer, "new key landed outside [Controls]:\n{out}");
    }

    #[test]
    fn collapses_duplicate_owned_keys() {
        let path = tmpfile("dupes");
        std::fs::write(&path, "[Controls]\nplayer_0_type=4\nplayer_0_type=2\n").unwrap();
        patch_ini_section(&path, "Controls", &entries(&[("player_0_type", "0")])).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert_eq!(out.matches("player_0_type=").count(), 1);
        assert!(out.contains("player_0_type=0"));
    }
}
