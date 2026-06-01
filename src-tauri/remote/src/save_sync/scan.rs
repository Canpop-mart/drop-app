//! Local save-file discovery and write-back.
//!
//! Two save sources are scanned:
//!
//! * **Emulator saves** — RetroArch keeps them under `{emu_root}/drop-saves/
//!   {game_id}/{saves,states}`; [`scan_emu_saves`] walks those directories.
//! * **PC saves** — discovered by shelling out to Ludusavi, whose database
//!   knows where each game stores its saves; [`scan_pc_saves`].
//!
//! [`write_downloaded_save`] / [`write_downloaded_pc_save`] put cloud copies
//! back, always backing up any existing file first.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use log::{info, warn};
use once_cell::sync::Lazy;

use super::LocalSaveFile;

/// Filename prefix that namespaces PC saves away from emulator saves.
///
/// Must be free of path separators: the server sanitizes upload filenames with
/// `sanitize-filename`, which strips `/` and `\`. The legacy `"pc/"` prefix was
/// mangled to `"pc"` on the server (`"pc/gen.sav"` → `"pcgen.sav"`), so the same
/// save no longer matched its local counterpart. `"pc__"` survives sanitization
/// intact, so a PC save keeps one stable identity across the upload round-trip.
pub const PC_SAVE_PREFIX: &str = "pc__";

/// Strip the PC-save namespace prefix to recover the on-disk basename.
///
/// Accepts the current `"pc__"` prefix and the legacy `"pc/"` prefix so saves
/// uploaded before the change still restore correctly.
pub fn strip_pc_prefix(filename: &str) -> &str {
    filename
        .strip_prefix(PC_SAVE_PREFIX)
        .or_else(|| filename.strip_prefix("pc/"))
        .unwrap_or(filename)
}

/// Compute the MD5 hash of a file on disk.
pub fn md5_file(path: &Path) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let digest = md5::compute(&data);
    Ok(format!("{:x}", digest))
}

/// Scan RetroArch save directories for a game.
/// Returns a list of local save files with their hashes.
pub fn scan_emu_saves(emu_root: &Path, game_id: &str) -> Vec<LocalSaveFile> {
    let saves_base = emu_root.join("drop-saves").join(game_id);
    let mut files = Vec::new();

    for (subdir, save_type) in &[("saves", "save"), ("states", "state")] {
        let dir = saves_base.join(subdir);
        if !dir.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let meta = match fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let modified_at = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let hash = match md5_file(&path) {
                    Ok(h) => h,
                    Err(e) => {
                        warn!("[SAVE-SYNC] Failed to hash {}: {}", path.display(), e);
                        continue;
                    }
                };
                files.push(LocalSaveFile {
                    filename: filename.clone(),
                    save_type: save_type.to_string(),
                    path,
                    data_hash: hash,
                    size: meta.len(),
                    modified_at,
                });
            }
        }
    }

    files
}

/// Write a downloaded save file to the correct local path.
pub fn write_downloaded_save(
    emu_root: &Path,
    game_id: &str,
    filename: &str,
    save_type: &str,
    data: &[u8],
) -> Result<PathBuf, String> {
    let subdir = match save_type {
        "save" => "saves",
        "state" => "states",
        _ => "saves", // fallback
    };
    let dir = emu_root.join("drop-saves").join(game_id).join(subdir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create save dir {}: {e}", dir.display()))?;
    let dest = dir.join(filename);

    // Create backup if file exists
    if dest.exists() {
        let bak = dest.with_extension(format!(
            "{}.bak",
            dest.extension().unwrap_or_default().to_string_lossy()
        ));
        let _ = fs::copy(&dest, &bak);
    }

    fs::write(&dest, data).map_err(|e| format!("Failed to write save {}: {e}", dest.display()))?;
    Ok(dest)
}

/// Delete a local emulator save (or save state) in response to a server
/// tombstone, after backing it up to `<file>.<ext>.bak`. Returns:
///   * `Ok(Some(path))` — the deleted file's original path,
///   * `Ok(None)`       — no local copy existed (nothing to do),
///   * `Err(msg)`       — backup or delete failed.
///
/// Mirrors [`write_downloaded_save`]'s backup convention so a user who
/// regrets a cross-device delete can recover the bytes manually.
pub fn delete_local_emu_save_for_tombstone(
    emu_root: &Path,
    game_id: &str,
    filename: &str,
) -> Result<Option<PathBuf>, String> {
    let saves_base = emu_root.join("drop-saves").join(game_id);
    // The server tombstone doesn't tell us "save" vs "state"; try both.
    for subdir in &["saves", "states"] {
        let candidate = saves_base.join(subdir).join(filename);
        if candidate.is_file() {
            let bak = candidate.with_extension(format!(
                "{}.bak",
                candidate.extension().unwrap_or_default().to_string_lossy()
            ));
            let _ = fs::copy(&candidate, &bak);
            fs::remove_file(&candidate)
                .map_err(|e| format!("Failed to delete tombstoned save {}: {e}", candidate.display()))?;
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Delete a PC save in response to a server tombstone. The caller passes the
/// resolved local path (from the manifest / `pc_save_paths` map); we don't
/// re-scan because the file may have been already deleted by the user on this
/// machine too. Backs up to `<file>.<ext>.bak` before unlinking.
pub fn delete_local_pc_save_for_tombstone(
    original_path: &Path,
) -> Result<bool, String> {
    if !original_path.is_file() {
        return Ok(false);
    }
    let bak = original_path.with_extension(format!(
        "{}.bak",
        original_path.extension().unwrap_or_default().to_string_lossy()
    ));
    let _ = fs::copy(original_path, &bak);
    fs::remove_file(original_path)
        .map_err(|e| format!("Failed to delete tombstoned PC save {}: {e}", original_path.display()))?;
    Ok(true)
}

// ── Ludusavi PC save scanning ──────────────────────────────────────────

/// Filenames that Ludusavi will sometimes report (typically via
/// Unity-engine `<home>/AppData/LocalLow/<Company>/<Product>` directory
/// matches tagged as "config") but which are NEVER actual save data:
///
/// * `Player.log` / `Player-prev.log` — Unity's per-run diagnostic log files
///   that Unity rewrites on every launch.  Hashes change every session even
///   when the user didn't save anything, so they pollute the cloud save
///   panel and waste bandwidth.
///
/// Matching is case-insensitive (Windows) but only against the *exact*
/// basename — we deliberately do not filter on substrings or extensions
/// because real saves sometimes have `.log` in their name and there is no
/// reliable heuristic beyond a tiny denylist.  When in doubt, keep the file.
const PC_SAVE_BASENAME_DENYLIST: &[&str] = &["Player.log", "Player-prev.log"];

/// Returns `true` if `path`'s basename matches one of the well-known
/// non-save filenames in [`PC_SAVE_BASENAME_DENYLIST`].
fn is_pc_save_denylisted(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // Backups Drop itself writes when restoring/overwriting a save (e.g.
    // "gen.sav.bak"). They sit right next to the real save, so the next
    // Ludusavi scan re-discovers them as brand-new saves — that's what made
    // the save count climb every time a restore ran and filled the panel with
    // phantom ".bak" rows. They are never save data the user cares about.
    if name.to_ascii_lowercase().ends_with(".bak") {
        return true;
    }
    PC_SAVE_BASENAME_DENYLIST
        .iter()
        .any(|deny| name.eq_ignore_ascii_case(deny))
}

/// Find the Ludusavi binary (bundled in Drop's tools dir, or on PATH).
fn find_ludusavi() -> Option<PathBuf> {
    let tools = dirs::data_dir()?.join("drop").join("tools");
    #[cfg(target_os = "windows")]
    let bundled = tools.join("ludusavi").join("ludusavi.exe");
    #[cfg(not(target_os = "windows"))]
    let bundled = tools.join("ludusavi").join("ludusavi");

    if bundled.exists() {
        return Some(bundled);
    }

    // Check PATH
    if let Ok(output) = std::process::Command::new("ludusavi").arg("--version").output() {
        if output.status.success() {
            return Some(PathBuf::from("ludusavi"));
        }
    }

    None
}

/// Pull the first game title out of a `ludusavi find --api` JSON blob.
/// The shape is `{ "games": { "<Canonical Title>": {...} }, ... }`; we
/// take the first key. Returns `None` on parse failure or empty games.
fn first_found_title(stdout: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(stdout);
    serde_json::from_str::<serde_json::Value>(&s)
        .ok()
        .and_then(|v| {
            v.get("games")?
                .as_object()?
                .keys()
                .next()
                .map(|k| k.to_string())
        })
}

/// Resolve a game's Drop **display name** to the canonical title that
/// Ludusavi's manifest actually uses.
///
/// This is the crux of why PC save detection was flaky. Ludusavi's
/// `backup` subcommand matches game names **exactly and case-sensitively**
/// against its manifest. Drop's display names carry trademark symbols
/// (`®`/`™`), all-caps branding ("LEGO"), and "edition"/year suffixes that
/// byte-differ from the manifest's canonical title — e.g. the manifest
/// has `Lego Batman: Legacy of the Dark Knight` but Drop stores
/// `LEGO® Batman™: Legacy of the Dark Knight`. A raw exact `backup` on
/// the display name therefore matches nothing and silently returns zero
/// saves.
///
/// `ludusavi find` resolves by precedence (Steam ID → GOG ID → exact →
/// normalized), and `--normalized` "ignores capitalization, 'edition'
/// suffixes, year suffixes, and some special symbols" — exactly the
/// noise in Drop's display names. We try the Steam ID first (an exact
/// identifier match, most reliable), then fall back to normalized-name
/// resolution. Returns the canonical manifest title, or `None` if
/// Ludusavi doesn't know the game under any of these.
fn resolve_canonical_title(
    ludusavi: &Path,
    game_name: &str,
    steam_app_id: Option<&str>,
) -> Option<String> {
    // 1) Steam ID — highest-precedence, exact identifier match.
    if let Some(id) = steam_app_id {
        if let Ok(output) = std::process::Command::new(ludusavi)
            .args(["find", "--api", "--steam-id", id])
            .output()
            && output.status.success()
            && let Some(name) = first_found_title(&output.stdout)
        {
            return Some(name);
        }
    }

    // 2) Normalized display name — collapses caps / ®™ / edition+year
    //    suffixes onto the manifest's canonical title.
    if let Ok(output) = std::process::Command::new(ludusavi)
        .args(["find", "--api", "--normalized", game_name])
        .output()
        && output.status.success()
        && let Some(name) = first_found_title(&output.stdout)
    {
        return Some(name);
    }

    None
}

// ── Manifest tag awareness ─────────────────────────────────────────────
//
// Ludusavi's `backup --api` output does NOT carry the per-file save/config
// tag — verified against its `schema general-output`: each file (ApiFile)
// has only bytes/change/duplicatedBy/failed/ignored. The save/config tags
// live ONLY in the manifest. The real Ludusavi GUI reads them to separate
// real saves from settings; to match that, we read the manifest ourselves
// and drop files that fall under a *config-only* path (e.g. Grim Dawn's
// "My Games/Grim Dawn/Settings", which holds keybindings.txt + options.txt
// rather than character saves).
//
// We pull the manifest as JSON via `manifest show --api` so it parses with
// serde_json (no YAML dependency), and cache the raw text for the process
// lifetime — the manifest only changes on `manifest update`.
static MANIFEST_JSON_CACHE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// One save-classification rule from a game's manifest entry: the literal
/// (placeholder/wildcard-free) path fragment to substring-match a scanned
/// file against, plus whether that manifest path is tagged config-and-not-
/// save (i.e. should be excluded from save backup).
struct ManifestPathRule {
    needle: String,
    config_only: bool,
}

/// Reduce a Ludusavi manifest path pattern to its longest leading
/// placeholder-free, wildcard-free literal fragment, for a substring match
/// against an absolute scanned path. Separator-normalized to `/`, and
/// lowercased on Windows for case-insensitive matching.
///
///   `<winDocuments>/My Games/Grim Dawn/Settings` → `my games/grim dawn/settings`
///   `<winLocalAppData>/XV83/Saved/SaveGames/**/*.sav` → `xv83/saved/savegames`
fn literal_fragment(pattern: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in pattern.split(['/', '\\']) {
        if seg.contains('<') || seg.contains('*') || seg.contains('[') {
            if out.is_empty() {
                continue; // skip leading placeholder segment(s)
            }
            break; // a wildcard terminates the literal tail
        }
        if !seg.is_empty() {
            out.push(seg);
        }
    }
    let joined = out.join("/");
    if cfg!(target_os = "windows") {
        joined.to_lowercase()
    } else {
        joined
    }
}

/// Return the substring covering the balanced `{...}` object at the start
/// of `s` (skipping any leading whitespace to the first `{`). Respects
/// JSON strings + escapes so braces inside string values don't miscount.
fn slice_balanced_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'{')?;
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for i in start..bytes.len() {
        let c = bytes[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Load (and cache) the manifest as JSON, then extract the path
/// classification rules for `title`. Empty vec if Ludusavi fails, the game
/// isn't in the manifest, or it has no `files` block — callers treat
/// "no rules" as "don't filter".
fn manifest_path_rules(ludusavi: &Path, title: &str) -> Vec<ManifestPathRule> {
    let mut guard = match MANIFEST_JSON_CACHE.lock() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    if guard.is_none() {
        match std::process::Command::new(ludusavi)
            .args(["manifest", "show", "--api"])
            .output()
        {
            Ok(o) if o.status.success() => {
                *guard = Some(String::from_utf8_lossy(&o.stdout).into_owned());
            }
            _ => {
                warn!("[SAVE-SYNC] Could not load Ludusavi manifest for tag filtering");
                return Vec::new();
            }
        }
    }
    let raw = match guard.as_ref() {
        Some(r) => r,
        None => return Vec::new(),
    };

    // The manifest is one big JSON object keyed by game title. Find this
    // game's value by its JSON-encoded key, then brace-match to slice out
    // just that entry — avoids parsing the whole ~9 MB blob into a Value.
    let Ok(key) = serde_json::to_string(title) else {
        return Vec::new();
    };
    let needle = format!("{key}:");
    let Some(key_pos) = raw.find(&needle) else {
        return Vec::new();
    };
    let after_key = &raw[key_pos + needle.len()..];
    let Some(obj) = slice_balanced_object(after_key) else {
        return Vec::new();
    };
    let Ok(entry) = serde_json::from_str::<serde_json::Value>(obj) else {
        return Vec::new();
    };

    let mut rules = Vec::new();
    if let Some(files) = entry.get("files").and_then(|f| f.as_object()) {
        for (pattern, meta) in files {
            let frag = literal_fragment(pattern);
            if frag.is_empty() {
                continue;
            }
            let tags: Vec<&str> = meta
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|arr| arr.iter().filter_map(|t| t.as_str()).collect())
                .unwrap_or_default();
            let has_save = tags.iter().any(|t| *t == "save");
            let has_config = tags.iter().any(|t| *t == "config");
            // config-only == tagged config but NOT save. Untagged paths
            // (no tags) default to save data per the manifest spec, so we
            // do NOT exclude them.
            rules.push(ManifestPathRule {
                needle: frag,
                config_only: has_config && !has_save,
            });
        }
    }
    rules
}

/// Decide whether a scanned PC file is a *save* (vs config) per the game's
/// manifest tags. Dropped only when it matches a config-only path AND no
/// save/untagged path. No rules, or no match, => keep (never silently drop
/// a file the manifest doesn't classify).
fn is_save_tagged(path: &Path, rules: &[ManifestPathRule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let hay = {
        let p = path.to_string_lossy().replace('\\', "/");
        if cfg!(target_os = "windows") {
            p.to_lowercase()
        } else {
            p
        }
    };
    let mut matched_config_only = false;
    let mut matched_keep = false;
    for rule in rules {
        if hay.contains(&rule.needle) {
            if rule.config_only {
                matched_config_only = true;
            } else {
                matched_keep = true;
            }
        }
    }
    if matched_keep {
        true
    } else {
        !matched_config_only
    }
}

/// Scan PC game saves using Ludusavi.
/// `game_name` is the display name to search for; `steam_app_id` is optional.
/// `wine_prefix`, when supplied, is passed to Ludusavi via `--wine-prefix`
/// so it scans Drop's per-game Wine prefix in addition to its default
/// scan locations (Steam compatdata, Lutris, Heroic). On native Linux
/// games (and on Windows hosts) pass `None` to keep the default behaviour.
/// Returns files as `LocalSaveFile` with save_type = "pc".
pub fn scan_pc_saves(
    game_name: &str,
    steam_app_id: Option<&str>,
    wine_prefix: Option<&Path>,
) -> Vec<LocalSaveFile> {
    let ludusavi = match find_ludusavi() {
        Some(p) => p,
        None => {
            info!("[SAVE-SYNC] Ludusavi not found, skipping PC save scan");
            return Vec::new();
        }
    };

    // Resolve the display name to Ludusavi's canonical manifest title
    // (Steam ID, else normalized name). Without this, branded display
    // names like "LEGO® Batman™: …" never match the manifest's
    // "Lego Batman: …" and the scan returns nothing.
    let resolved_name = resolve_canonical_title(&ludusavi, game_name, steam_app_id);

    let search_name = resolved_name.as_deref().unwrap_or(game_name);
    let wine_prefix_str = wine_prefix.map(|p| p.to_string_lossy().to_string());
    if let Some(p) = wine_prefix_str.as_deref() {
        info!("[SAVE-SYNC] Ludusavi scanning for '{}' (wine prefix: {})", search_name, p);
    } else {
        info!("[SAVE-SYNC] Ludusavi scanning for '{}'", search_name);
    }

    // Build args once; injected `--wine-prefix <path>` precedes the game
    // name so it applies to the backup subcommand.
    let build_args = |name: &str| -> Vec<String> {
        let mut args: Vec<String> = vec![
            "backup".into(),
            "--preview".into(),
            "--api".into(),
        ];
        if let Some(p) = wine_prefix_str.as_deref() {
            args.push("--wine-prefix".into());
            args.push(p.to_string());
        }
        args.push(name.to_string());
        args
    };

    // Run "backup --preview --api [--wine-prefix <path>] <name>"
    let output = std::process::Command::new(&ludusavi)
        .args(build_args(search_name))
        .output();

    // Retry with the original name if resolved name found nothing
    let output = match &output {
        Ok(o) if !o.status.success() || o.stdout.len() < 50 => {
            if search_name != game_name {
                info!(
                    "[SAVE-SYNC] Retrying Ludusavi with original name: '{}'",
                    game_name
                );
                std::process::Command::new(&ludusavi)
                    .args(build_args(game_name))
                    .output()
            } else {
                output
            }
        }
        _ => output,
    };

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            warn!("[SAVE-SYNC] Ludusavi command failed: {e}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("No matching") {
            warn!("[SAVE-SYNC] Ludusavi error: {}", stderr);
        }
        return Vec::new();
    }

    // Parse the JSON output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            warn!("[SAVE-SYNC] Failed to parse Ludusavi output: {e}");
            return Vec::new();
        }
    };

    let mut files = Vec::new();

    // Per-game manifest tag rules — lets us drop config-only files
    // (settings, keybinds) the same way the real Ludusavi GUI does. The
    // CLI strips tags from its output, so we read them from the manifest
    // here. `search_name` is the canonical title we resolved above, which
    // is exactly the manifest key.
    let tag_rules = manifest_path_rules(&ludusavi, search_name);
    let mut tag_dropped = 0usize;

    if let Some(games) = json.get("games").and_then(|g| g.as_object()) {
        for (_name, game_data) in games {
            if let Some(game_files) = game_data.get("files").and_then(|f| f.as_object()) {
                for (file_path, file_data) in game_files {
                    let path = PathBuf::from(file_path);
                    if !path.is_file() {
                        continue;
                    }
                    // Drop known non-save files (Unity diagnostic logs etc.)
                    // that Ludusavi's directory-wildcard manifest entries pick
                    // up alongside the real save data.
                    if is_pc_save_denylisted(&path) {
                        info!(
                            "[SAVE-SYNC] Skipping denylisted PC save file: {}",
                            path.display()
                        );
                        continue;
                    }
                    // Honor the manifest's save/config tags: skip files that
                    // live only under a config-tagged path (e.g. Grim Dawn's
                    // Settings/ folder). Matches how the real Ludusavi GUI
                    // separates saves from settings.
                    if !is_save_tagged(&path, &tag_rules) {
                        tag_dropped += 1;
                        info!(
                            "[SAVE-SYNC] Skipping config-tagged file: {}",
                            path.display()
                        );
                        continue;
                    }
                    // Ludusavi's `backup --api` reports file size under the `bytes`
                    // key (not `size`); reading the wrong key reported every file as 0 B.
                    let size = file_data.get("bytes").and_then(|s| s.as_u64()).unwrap_or(0);
                    let hash = match md5_file(&path) {
                        Ok(h) => h,
                        Err(e) => {
                            warn!(
                                "[SAVE-SYNC] Failed to hash PC save {}: {}",
                                path.display(),
                                e
                            );
                            continue;
                        }
                    };
                    let modified_at = fs::metadata(&path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    // Namespace PC saves so their filenames don't collide with emu
                    // saves. Use a "pc__" prefix (NOT "pc/"): the server runs the
                    // upload filename through `sanitize-filename`, which strips path
                    // separators — "pc/gen.sav" became "pcgen.sav" on the server while
                    // the local scan still reported "pc/gen.sav", so the same save
                    // showed up twice (once per side). A separator-free prefix
                    // round-trips unchanged, giving every save one stable identity.
                    let filename = format!(
                        "{}{}",
                        PC_SAVE_PREFIX,
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );

                    files.push(LocalSaveFile {
                        filename,
                        save_type: "pc".to_string(),
                        path,
                        data_hash: hash,
                        size,
                        modified_at,
                    });
                }
            }
        }
    }

    info!(
        "[SAVE-SYNC] Ludusavi found {} PC save file(s) for '{}' ({} config-tagged file(s) filtered out)",
        files.len(),
        search_name,
        tag_dropped
    );
    files
}

/// Resolve the on-disk destination path for a PC save by basename, using
/// Ludusavi's catalogue.
///
/// Resolution is two-tier:
///   1. **Exact match** — if Ludusavi reports a file with this basename, use
///      its path (handles conflicts / re-restores of an existing save).
///   2. **Sibling directory** — otherwise place the save next to the game's
///      other saves (they all live in one folder). This is what lets a
///      cloud-only save the game has *never* written on this device restore to
///      the right place instead of erroring out: as long as Ludusavi found at
///      least one of the game's saves, we know the folder to drop it in.
///
/// Only when the game has NO saves on disk at all (so there's no folder to
/// infer) does this return `Err` asking the user to launch the game once.
/// Also `Err` if Ludusavi can't be found / fails. Basename matching is
/// case-sensitive on Unix, case-insensitive on Windows.
pub fn find_pc_save_destination(
    game_name: &str,
    basename: &str,
    wine_prefix: Option<&Path>,
) -> Result<PathBuf, String> {
    let ludusavi = find_ludusavi()
        .ok_or_else(|| "Ludusavi is not installed on this device".to_string())?;

    // Resolve to the canonical manifest title before the exact-match
    // backup, same as scan_pc_saves — otherwise a branded display name
    // restores to nowhere even though the save exists. No Steam ID is
    // threaded through this path, so we rely on normalized resolution.
    let canonical = resolve_canonical_title(&ludusavi, game_name, None);
    let search_name = canonical.as_deref().unwrap_or(game_name);

    let wine_prefix_str = wine_prefix.map(|p| p.to_string_lossy().to_string());
    let mut args: Vec<String> = vec!["backup".into(), "--preview".into(), "--api".into()];
    if let Some(p) = wine_prefix_str.as_deref() {
        args.push("--wine-prefix".into());
        args.push(p.to_string());
    }
    args.push(search_name.to_string());

    let output = std::process::Command::new(&ludusavi)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run Ludusavi: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Ludusavi error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse Ludusavi output: {e}"))?;

    let basenames_equal = |a: &str, b: &str| -> bool {
        if cfg!(target_os = "windows") {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };

    // Single pass: short-circuit on an exact basename match, otherwise gather
    // fallback directories so a not-yet-existing save can be dropped next to
    // the game's other saves. Prefer a directory that already holds a file
    // with the same extension (so e.g. a ".sav" lands among ".sav" files, not
    // in a sibling "config" folder) and fall back to the first dir we saw.
    let target_ext = Path::new(basename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    let mut sibling_dir: Option<PathBuf> = None;
    let mut ext_match_dir: Option<PathBuf> = None;
    if let Some(games) = json.get("games").and_then(|g| g.as_object()) {
        for (_name, game_data) in games {
            if let Some(game_files) = game_data.get("files").and_then(|f| f.as_object()) {
                for (file_path, _file_data) in game_files {
                    let path = PathBuf::from(file_path);
                    if is_pc_save_denylisted(&path) {
                        continue;
                    }
                    if let Some(name) = path.file_name().and_then(|n| n.to_str())
                        && basenames_equal(name, basename)
                    {
                        return Ok(path);
                    }
                    if let Some(parent) = path.parent() {
                        if sibling_dir.is_none() {
                            sibling_dir = Some(parent.to_path_buf());
                        }
                        if ext_match_dir.is_none()
                            && target_ext.is_some()
                            && path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_ascii_lowercase())
                                == target_ext
                        {
                            ext_match_dir = Some(parent.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    // No file with this exact name exists yet — e.g. restoring a cloud-only
    // save the game has never written on this device. All of a game's PC
    // saves share one directory, so place it next to the ones Ludusavi did
    // find. This is the difference between Restore "just populating" the save
    // and failing with "no save matching".
    if let Some(dir) = ext_match_dir.or(sibling_dir) {
        return Ok(dir.join(basename));
    }

    Err(format!(
        "Ludusavi knows {game_name:?} but found no saves on this device to place {basename:?} next to. \
         Launch the game once so it creates its save folder, then try again."
    ))
}

/// Write a downloaded PC save file back to its original location.
/// PC save filenames carry a namespace prefix — strip it and restore to the
/// original path from the manifest, or use a fallback location.
pub fn write_downloaded_pc_save(
    filename: &str,
    data: &[u8],
    original_path: Option<&Path>,
) -> Result<PathBuf, String> {
    // If we know the original path (from manifest), use it
    if let Some(orig) = original_path {
        if let Some(parent) = orig.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir for PC save: {e}"))?;
        }
        // Backup existing
        if orig.exists() {
            let bak = orig.with_extension(format!(
                "{}.bak",
                orig.extension().unwrap_or_default().to_string_lossy()
            ));
            let _ = fs::copy(orig, &bak);
        }
        fs::write(orig, data).map_err(|e| format!("Failed to write PC save: {e}"))?;
        return Ok(orig.to_path_buf());
    }

    // Fallback: save to data_dir/drop/pc-saves/<filename>
    let clean_name = strip_pc_prefix(filename);
    let fallback = dirs::data_dir()
        .ok_or("No data directory")?
        .join("drop")
        .join("pc-saves")
        .join(clean_name);
    if let Some(parent) = fallback.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create fallback dir: {e}"))?;
    }
    fs::write(&fallback, data).map_err(|e| format!("Failed to write PC save fallback: {e}"))?;
    Ok(fallback)
}
