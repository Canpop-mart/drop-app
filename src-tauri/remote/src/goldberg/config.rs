//! Goldberg pre-launch configuration and runtime diagnostics.
//!
//! Drop patches Goldberg's `steam_settings/configs.user.ini` so that:
//!
//! * saves go to `./drop-goldberg` (relative to the game DLL) — a known
//!   location the cloud-save and achievement code can find,
//! * the player's display name is set as `account_name`.
//!
//! INI editing is line-oriented and idempotent: only the keys Drop manages are
//! touched, and a stale legacy key (`saves_folder_name`) is stripped.

use log::{debug, info, warn};
use std::path::{Path, PathBuf};

use super::DROP_GSE_FOLDER;

/// `[user::saves]` `local_save_path` — controls where Goldberg writes saves.
const INI_SECTION: &str = "[user::saves]";
const INI_KEY: &str = "local_save_path";

/// `[user::general]` `account_name` — the player's display name.
const INI_GENERAL_SECTION: &str = "[user::general]";
const INI_ACCOUNT_NAME_KEY: &str = "account_name";

/// Legacy key Drop used to write. If found inside `[user::saves]` it is
/// removed so Goldberg doesn't fall back to the AppData path.
const LEGACY_INI_KEY: &str = "saves_folder_name";

/// Writes Goldberg's save-path and account-name config into
/// `steam_settings/configs.user.ini`, creating the directory if needed.
///
/// Skips the write entirely when both settings are already correct.
pub fn configure_goldberg(dll_dir: &Path, display_name: Option<&str>) {
    let steam_settings = dll_dir.join("steam_settings");
    if !steam_settings.is_dir() {
        if let Err(e) = std::fs::create_dir_all(&steam_settings) {
            warn!("[EMU] Could not create steam_settings at {}: {e}", steam_settings.display());
            return;
        }
        info!("[EMU] Created {} for Goldberg config", steam_settings.display());
    }

    // Absolute, DLL-anchored save path. Goldberg resolves a relative
    // local_save_path against the process CWD, so a launch whose CWD isn't the
    // DLL dir would spawn a stray drop-goldberg next to the CWD. Anchoring it at
    // the DLL dir makes the location CWD-independent. Forward slashes keep the
    // value valid under both Windows and Proton/wine; the path is written
    // unquoted (Goldberg reads the rest of the line verbatim, spaces included).
    let save_dir = dll_dir.join(DROP_GSE_FOLDER);
    let _ = std::fs::create_dir_all(&save_dir);
    let save_value = save_dir.to_string_lossy().replace('\\', "/");

    let ini_path = steam_settings.join("configs.user.ini");
    let desired_save_line = format!("{INI_KEY}={save_value}");
    let existing = std::fs::read_to_string(&ini_path).unwrap_or_default();

    let saves_ok = has_correct_setting(&existing, &save_value);
    let name_ok = display_name
        .map(|n| has_correct_account_name(&existing, n))
        .unwrap_or(true);

    if saves_ok && name_ok {
        debug!("[EMU] Goldberg config already up to date for {}", dll_dir.display());
        return;
    }

    let mut updated = if saves_ok {
        existing.clone()
    } else {
        update_ini_content(&existing, &desired_save_line)
    };
    if let Some(name) = display_name
        && !name_ok {
            updated = update_ini_account_name(&updated, name);
        }

    match std::fs::write(&ini_path, &updated) {
        Ok(_) => info!(
            "[EMU] Configured Goldberg for {} (saves -> {save_value}, name -> {:?})",
            dll_dir.display(),
            display_name.unwrap_or("<unchanged>")
        ),
        Err(e) => warn!("[EMU] Could not write configs.user.ini at {}: {e}", ini_path.display()),
    }
}

/// `true` if `[user::saves]` already carries `local_save_path=<expected>`.
/// Parses the line as key=value so paths containing spaces compare correctly
/// (a blanket space-strip would corrupt a value like `C:/My Games/...`).
fn has_correct_setting(content: &str, expected: &str) -> bool {
    section_has(content, INI_SECTION, |trimmed| {
        match trimmed.split_once('=') {
            Some((k, v)) => k.trim() == INI_KEY && v.trim() == expected,
            None => false,
        }
    })
}

/// Seeds (or clears) `steam_settings/custom_broadcasts.txt` with a co-op room's
/// peer IPs. gbe_fork unicasts its LAN announce to each listed IP, which is how
/// Goldberg discovery works over a ZeroTier overlay (broadcast is dropped on
/// L3). An empty list removes the file so a stale peer set from a previous room
/// never leaks into a later solo launch. Best-effort: failures are logged, never
/// propagated — co-op seeding must not block a launch.
pub fn write_custom_broadcasts(dll_dir: &Path, peer_ips: &[String]) {
    let steam_settings = dll_dir.join("steam_settings");
    let path = steam_settings.join("custom_broadcasts.txt");

    if peer_ips.is_empty() {
        if path.exists() {
            match std::fs::remove_file(&path) {
                Ok(_) => info!("[COOP] cleared {}", path.display()),
                Err(e) => warn!("[COOP] could not remove {}: {e}", path.display()),
            }
        }
        return;
    }

    if !steam_settings.is_dir()
        && let Err(e) = std::fs::create_dir_all(&steam_settings)
    {
        warn!("[COOP] could not create {}: {e}", steam_settings.display());
        return;
    }

    let body = format!("{}\n", peer_ips.join("\n"));
    match std::fs::write(&path, body) {
        Ok(_) => info!(
            "[COOP] seeded {} peer IP(s) -> {}",
            peer_ips.len(),
            path.display()
        ),
        Err(e) => warn!("[COOP] could not write {}: {e}", path.display()),
    }
}

/// `true` if `[user::general]` already carries the correct `account_name`.
fn has_correct_account_name(content: &str, expected_name: &str) -> bool {
    let target = format!("{INI_ACCOUNT_NAME_KEY}={expected_name}");
    section_has(content, INI_GENERAL_SECTION, |trimmed| {
        trimmed.replace(' ', "") == target
    })
}

/// Returns `true` if any line within `[section]` satisfies `pred`.
fn section_has(content: &str, section: &str, pred: impl Fn(&str) -> bool) -> bool {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(section) {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_section = false;
            continue;
        }
        if in_section && pred(trimmed) {
            return true;
        }
    }
    false
}

/// Updates (or appends) `[user::saves]` with `desired_line`, preserving all
/// other content and stripping the legacy `saves_folder_name` key.
fn update_ini_content(existing: &str, desired_line: &str) -> String {
    if existing.is_empty() {
        return format!("{INI_SECTION}\n{desired_line}\n");
    }
    upsert_section_key(existing, INI_SECTION, INI_KEY, desired_line, Some(LEGACY_INI_KEY))
}

/// Updates (or appends) `[user::general]` with `account_name`.
fn update_ini_account_name(existing: &str, name: &str) -> String {
    let desired_line = format!("{INI_ACCOUNT_NAME_KEY}={name}");
    upsert_section_key(existing, INI_GENERAL_SECTION, INI_ACCOUNT_NAME_KEY, &desired_line, None)
}

/// Line-oriented INI upsert: ensures `[section]` contains exactly one line
/// `desired_line` for `key`, dropping any line whose key is `drop_key`, and
/// preserving every other line. Appends the section if it is absent.
fn upsert_section_key(
    existing: &str,
    section: &str,
    key: &str,
    desired_line: &str,
    drop_key: Option<&str>,
) -> String {
    let mut result = String::with_capacity(existing.len() + 80);
    let mut found_section = false;
    let mut replaced_key = false;
    let mut in_section = false;

    for line in existing.lines() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case(section) {
            found_section = true;
            in_section = true;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        // Leaving the section without having written our key — append it.
        if trimmed.starts_with('[') {
            if in_section && !replaced_key {
                result.push_str(desired_line);
                result.push('\n');
                replaced_key = true;
            }
            in_section = false;
        }

        if in_section {
            let line_key = trimmed.split(['=', ' ']).next().unwrap_or("");
            if drop_key == Some(line_key) {
                continue; // strip the legacy key entirely
            }
            if line_key == key {
                result.push_str(desired_line);
                result.push('\n');
                replaced_key = true;
                continue;
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    // Still inside the section at EOF without having written the key.
    if in_section && !replaced_key {
        result.push_str(desired_line);
        result.push('\n');
    }

    // Section absent entirely — append it.
    if !found_section {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(section);
        result.push('\n');
        result.push_str(desired_line);
        result.push('\n');
    }

    result
}

/// Checks for GBE log / crash / marker files to verify the emulator is
/// actually loading. GBE writes logs to `<save>/crash_reports/` and creates
/// marker files; call this after the game has run briefly to diagnose a DLL
/// that isn't really a Goldberg/GBE build. Returns `true` if GBE looks active.
pub fn check_gbe_activity(dll_dir: &str) -> bool {
    let root = PathBuf::from(dll_dir);
    let mut found_any = false;

    // Log / crash directories.
    for log_dir in [
        root.join(DROP_GSE_FOLDER).join("crash_reports"),
        root.join(DROP_GSE_FOLDER).join("logs"),
        root.join("crash_reports"),
        root.join("logs"),
    ] {
        if log_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&log_dir) {
                let files: Vec<_> = entries.flatten().filter(|e| e.path().is_file()).collect();
                if !files.is_empty() {
                    info!("[GBE-DIAG] Found {} log/crash files in {}", files.len(), log_dir.display());
                    if let Some(latest) = files.last() {
                        info!("[GBE-DIAG] Latest file: {}", latest.path().display());
                    }
                    found_any = true;
                }
            }
    }

    // GBE-specific marker files.
    for marker in [
        root.join(DROP_GSE_FOLDER).join("user_steam_id.txt"),
        root.join(DROP_GSE_FOLDER).join("account_name.txt"),
    ] {
        if marker.exists() {
            info!("[GBE-DIAG] Found GBE marker file: {}", marker.display());
            found_any = true;
        }
    }

    // Runtime files GBE creates inside drop-goldberg/<AppID>/.
    let save_root = root.join(DROP_GSE_FOLDER);
    if save_root.is_dir()
        && let Ok(entries) = std::fs::read_dir(&save_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Ok(files) = std::fs::read_dir(&path) {
                    let file_names: Vec<String> = files
                        .flatten()
                        .filter_map(|e| e.file_name().to_str().map(str::to_string))
                        .collect();
                    // achievements.json is written by Drop itself, not GBE.
                    if file_names.iter().any(|f| f != "achievements.json") {
                        found_any = true;
                    }
                    info!("[GBE-DIAG] Save dir {}: files = {file_names:?}", path.display());
                }
            }
        }

    if found_any {
        info!("[GBE-DIAG] GBE appears active in {dll_dir}");
    } else {
        warn!(
            "[GBE-DIAG] No GBE activity detected in {dll_dir} — the emulator may not be \
             loading. Check that the steam_api DLL is a Goldberg/GBE build."
        );
    }
    found_any
}
