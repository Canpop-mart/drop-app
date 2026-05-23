//! RetroArch config-file primitives.
//!
//! RetroArch's config format is line-oriented `key = "value"`. Drop never
//! rewrites a config wholesale — it *patches* it: existing keys it cares
//! about are replaced, stale keys are deleted, everything else is left
//! untouched. This keeps any user-made changes that Drop doesn't manage.
//!
//! Patching is idempotent: running it twice produces the same file.

use log::{debug, warn};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Converts a path to RetroArch config format: forward slashes (even on
/// Windows) wrapped in double quotes.
pub fn path_to_cfg(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
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
            if delete_keys.iter().any(|dk| *dk == key) {
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
        if !was_found {
            if let Some(value) = overrides.get(key) {
                lines.push(format!("{key} = {value}"));
            }
        }
    }

    let content = lines.join("\n") + "\n";

    fs::write(cfg_path, &content)?;
    debug!("[RETROARCH] Wrote config to {}", cfg_path.display());
    Ok(())
}
