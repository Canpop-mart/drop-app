//! Addon shared saves: the client half of an addon's "check out / check in"
//! buttons (first user: the NCAA 14 league's dynasty save).
//!
//! The official save lives with the addon on the server. drop-server tells
//! the client WHERE the save goes as a [`SaveSlot`]: a fixed root plus path
//! segments. Nothing in a slot is trusted:
//!
//! * the root is one this code knows (`winLocalAppData`), never a path;
//! * every segment must pass [`is_safe_segment`] (no `..`, no separators, no
//!   drive letters, no Windows reserved names, no trailing dot or space);
//! * the joined path is checked lexically ([`join_within`]) AND against the
//!   real filesystem: the profile folder must really be inside the profiles
//!   folder, and the save's folder must really be inside that profile
//!   ([`ensure_parent_within`]), so a symlink or junction inside the save
//!   tree can't lead a write outside the player's profile. The save file
//!   itself may not be a symlink. These are checked again right before the
//!   write.
//!
//! Not covered: `write_atomic`'s temp file is created next to the save with a
//! plain write, so a symlink planted at that exact temp name by something
//! already able to write the player's save folder would be followed. Anything
//! with that access can rewrite the save directly anyway.
//!
//! Requests go from here (Rust), never from the webview, with the client's
//! web token, to fixed drop-server routes built from a validated addon name
//! and context id. A download is held in memory (saves are about 8 MB, capped
//! by the slot), checked for size and SHA-1, and only then written, atomically,
//! after the old file is backed up.
//!
//! Per addon context (a league) this keeps, under `<data>/addons/<addon>/<ctx>`:
//! * `receipt.json`: the save path, the version checked out (`base_version`,
//!   sent back on check-in) and the SHA-1 of the last bytes this device wrote
//!   or handed in. A local file with a different hash holds play that was
//!   never handed in, and is not replaced without the player's say-so.
//! * `backups/`: the newest [`BACKUPS_KEPT`] replaced saves, plus the newest
//!   copy of play that was never handed in, which is never pruned.
//!
//! Drop's own cloud saves skip every folder a receipt names
//! ([`is_addon_managed`]), so a pre-launch cloud restore can't overwrite a save
//! that was just checked out.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use database::{borrow_db_checked, db::DATA_ROOT_DIR};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use utils::path_guard::{ensure_parent_within, join_within};

use crate::save_sync::backup::write_atomic;
use crate::utils::DROP_CLIENT_DOWNLOAD;

/// Hard ceiling whatever a slot says.
pub const MAX_SAVE_BYTES: u64 = 64 * 1024 * 1024;
/// Replaced saves kept per context (plus the newest never-handed-in copy).
pub const BACKUPS_KEPT: usize = 2;
const PROFILE_PATTERN: &str = "^[0-9A-F]{16}$";

/// Where an addon's save lives, as drop-server describes it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSlot {
    pub root: String,
    pub profiles_dir: Vec<String>,
    pub profile_pattern: String,
    #[serde(default)]
    pub profile_exclude: Vec<String>,
    pub file: Vec<String>,
    pub max_bytes: u64,
    #[serde(default)]
    pub process_names: Vec<String>,
}

/// Why an addon file step did not happen. `message` is shown to the player
/// (placeholder wording; the owner rewrites player-facing text).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddonError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
}

impl AddonError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            profiles: Vec::new(),
        }
    }
}

fn play_first() -> AddonError {
    AddonError::new(
        "play_first",
        "Play the game once on this device first, so it creates its save folder.",
    )
}

fn unsafe_path() -> AddonError {
    AddonError::new(
        "unsafe_path",
        "The save folder isn't a normal folder on this device (it leads somewhere else), so Drop won't touch it.",
    )
}

fn io_error(what: &str, e: impl std::fmt::Display) -> AddonError {
    warn!("[ADDONS] {what}: {e}");
    AddonError::new("io", format!("Couldn't {what}: {e}"))
}

// ── validation (pure) ────────────────────────────────────────────────────────

pub fn valid_addon_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

pub fn valid_context(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && b.len() <= 9 && b[0] != b'0' && b.iter().all(u8::is_ascii_digit)
}

fn is_windows_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

/// One folder or file name: starts with a letter or digit, then letters,
/// digits, space and `._()-`, at most 64, no trailing dot or space, not a
/// Windows reserved name.
pub fn is_safe_segment(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 64 || !b[0].is_ascii_alphanumeric() {
        return false;
    }
    if !b
        .iter()
        .all(|c| c.is_ascii_alphanumeric() || b" ._()-".contains(c))
    {
        return false;
    }
    if s.ends_with('.') || s.ends_with(' ') {
        return false;
    }
    !is_windows_reserved(s)
}

fn is_profile_name(s: &str) -> bool {
    s.len() == 16
        && s
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c))
}

fn is_process_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b" ._-".contains(&c))
}

pub fn is_sha1_hex(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// Refuse anything about a slot this code doesn't understand.
pub fn check_slot(slot: &SaveSlot) -> Result<(), AddonError> {
    let unsupported = |why: &str| {
        warn!("[ADDONS] refusing save slot: {why}");
        AddonError::new(
            "unsupported",
            "This addon describes its save in a way this version of Drop doesn't support. Update Drop.",
        )
    };
    if slot.root != "winLocalAppData" {
        return Err(unsupported("unknown root"));
    }
    if slot.profile_pattern != PROFILE_PATTERN {
        return Err(unsupported("unknown profile pattern"));
    }
    if slot.profiles_dir.is_empty() || slot.file.is_empty() {
        return Err(unsupported("empty path"));
    }
    if !slot
        .profiles_dir
        .iter()
        .chain(slot.file.iter())
        .all(|s| is_safe_segment(s))
    {
        return Err(unsafe_path());
    }
    if slot.max_bytes == 0 || slot.max_bytes > MAX_SAVE_BYTES {
        return Err(unsupported("bad size limit"));
    }
    if !slot.process_names.iter().all(|p| is_process_name(p)) {
        return Err(unsupported("bad process name"));
    }
    Ok(())
}

// ── resolving the save path ─────────────────────────────────────────────────

/// The Windows build's `%LOCALAPPDATA%`: the real one on Windows, the one
/// inside Drop's Wine/Proton prefix for this game elsewhere. `None` when the
/// game has never run, so there is no prefix yet.
pub fn win_local_appdata(game_id: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let _ = game_id;
        dirs::data_local_dir()
    }
    #[cfg(not(windows))]
    {
        let prefix = crate::goldberg::wine_prefix_for_game(game_id)?;
        let user = crate::goldberg::wine_user_dir(&prefix)?;
        Some(user.join("AppData").join("Local"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSave {
    /// The profile folder, canonical. Writes must stay inside it.
    pub profile_real: PathBuf,
    pub file: PathBuf,
    pub profile: String,
}

fn rel(segments: &[String]) -> PathBuf {
    segments.iter().collect()
}

/// Resolve `slot` under `root`. The profile folder is the single folder under
/// `profiles_dir` named like a profile (16 hex digits) and not excluded;
/// none means the game has not run yet, several means Drop can't know which
/// is the player's and says so.
pub fn resolve_save_in(root: &Path, slot: &SaveSlot) -> Result<ResolvedSave, AddonError> {
    check_slot(slot)?;
    let root_real = root.canonicalize().map_err(|_| play_first())?;
    let profiles = join_within(&root_real, &rel(&slot.profiles_dir)).map_err(|_| unsafe_path())?;
    if !profiles.is_dir() {
        return Err(play_first());
    }
    let profiles_real = profiles.canonicalize().map_err(|_| play_first())?;
    if !profiles_real.starts_with(&root_real) {
        return Err(unsafe_path());
    }

    let mut found: Vec<String> = Vec::new();
    let entries =
        fs::read_dir(&profiles_real).map_err(|e| io_error("read the save folder", e))?;
    for entry in entries.flatten() {
        // `file_type` does not follow links: a symlinked "profile" is skipped.
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_profile_name(&name) && !slot.profile_exclude.iter().any(|x| x == &name) {
            found.push(name);
        }
    }
    found.sort();
    let profile = match found.len() {
        0 => return Err(play_first()),
        1 => found.remove(0),
        _ => {
            let mut e = AddonError::new(
                "several_profiles",
                format!(
                    "This device has more than one game profile ({}). Drop can't tell which is yours, so it changed nothing.",
                    found.join(", ")
                ),
            );
            e.profiles = found;
            return Err(e);
        }
    };

    let profile_real = profiles_real
        .join(&profile)
        .canonicalize()
        .map_err(|_| play_first())?;
    if !profile_real.starts_with(&profiles_real) {
        return Err(unsafe_path());
    }
    let file = join_within(&profile_real, &rel(&slot.file)).map_err(|_| unsafe_path())?;
    let resolved = ResolvedSave {
        profile_real,
        file,
        profile,
    };
    check_still_contained(&resolved)?;
    Ok(resolved)
}

/// The save's folder is really inside the profile and the save is a plain
/// file (or not there yet). Run again right before writing.
fn check_still_contained(r: &ResolvedSave) -> Result<(), AddonError> {
    ensure_parent_within(&r.profile_real, &r.file).map_err(|_| unsafe_path())?;
    if let Ok(meta) = fs::symlink_metadata(&r.file)
        && (meta.file_type().is_symlink() || !meta.is_file())
    {
        return Err(unsafe_path());
    }
    Ok(())
}

/// A Drop game id as it may be used in a path (the Wine prefix folder).
pub fn valid_game_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

pub fn resolve_save(game_id: &str, slot: &SaveSlot) -> Result<ResolvedSave, AddonError> {
    if !valid_game_id(game_id) {
        return Err(AddonError::new("unsupported", "Bad game id."));
    }
    check_slot(slot)?;
    let root = win_local_appdata(game_id).ok_or_else(play_first)?;
    resolve_save_in(&root, slot)
}

// ── receipts, backups, managed folders ──────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub base_version: Option<u64>,
    /// SHA-1 of the bytes this device last wrote (check-out) or handed in.
    #[serde(default)]
    pub known_sha1: Option<String>,
    #[serde(default)]
    pub handed_in: bool,
}

fn context_dir_in(data_root: &Path, addon: &str, ctx: &str) -> PathBuf {
    data_root.join("addons").join(addon).join(ctx)
}

fn context_dir(addon: &str, ctx: &str) -> PathBuf {
    context_dir_in(&DATA_ROOT_DIR, addon, ctx)
}

pub fn read_receipt(addon: &str, ctx: &str) -> Receipt {
    let path = context_dir(addon, ctx).join("receipt.json");
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            warn!("[ADDONS] unreadable receipt {}: {e}", path.display());
            Receipt::default()
        }),
        Err(_) => Receipt::default(),
    }
}

fn write_receipt(addon: &str, ctx: &str, r: &Receipt) -> Result<(), AddonError> {
    let path = context_dir(addon, ctx).join("receipt.json");
    let bytes = serde_json::to_vec_pretty(r).map_err(|e| io_error("save the receipt", e))?;
    write_atomic(&path, &bytes).map_err(|e| io_error("save the receipt", e))?;
    *MANAGED.lock().unwrap_or_else(|p| p.into_inner()) = None;
    Ok(())
}

/// Which backups to delete, given every backup file name in one context's
/// folder. Names start with a sortable timestamp; `-unsent` marks a copy of
/// play that was never handed in. Keeps the newest [`BACKUPS_KEPT`] and the
/// newest `-unsent` one.
pub fn backups_to_prune(names: &[String]) -> Vec<String> {
    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort_by(|a, b| b.cmp(a));
    let newest_unsent = sorted.iter().find(|n| n.contains("-unsent")).cloned();
    sorted
        .iter()
        .enumerate()
        .filter(|(i, n)| *i >= BACKUPS_KEPT && Some(**n) != newest_unsent)
        .map(|(_, n)| (*n).clone())
        .collect()
}

fn backup(addon: &str, ctx: &str, src: &Path, kind: &str) -> Result<PathBuf, AddonError> {
    let dir = context_dir(addon, ctx).join("backups");
    fs::create_dir_all(&dir).map_err(|e| io_error("make the backup folder", e))?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3f");
    let dest = dir.join(format!("{stamp}-{kind}.bin"));
    let data = fs::read(src).map_err(|e| io_error("read the save to back it up", e))?;
    write_atomic(&dest, &data).map_err(|e| io_error("write the backup", e))?;
    // The copy must be what was on disk before anything replaces it.
    let check = fs::read(&dest).map_err(|e| io_error("check the backup", e))?;
    if check != data {
        return Err(AddonError::new(
            "io",
            "The backup didn't match the save, so Drop changed nothing.",
        ));
    }
    if let Ok(entries) = fs::read_dir(&dir) {
        let names: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".bin"))
            .collect();
        for old in backups_to_prune(&names) {
            let _ = fs::remove_file(dir.join(old));
        }
    }
    info!("[ADDONS] backed up {} to {}", src.display(), dest.display());
    Ok(dest)
}

/// Folders holding addon-managed saves (the folder of each receipt's save).
static MANAGED: LazyLock<Mutex<Option<Vec<PathBuf>>>> = LazyLock::new(|| Mutex::new(None));

fn load_managed() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(addons) = fs::read_dir(DATA_ROOT_DIR.join("addons")) else {
        return out;
    };
    for addon in addons.flatten() {
        let Ok(ctxs) = fs::read_dir(addon.path()) else { continue };
        for ctx in ctxs.flatten() {
            let Ok(bytes) = fs::read(ctx.path().join("receipt.json")) else { continue };
            if let Ok(r) = serde_json::from_slice::<Receipt>(&bytes)
                && let Some(dir) = r.path.as_deref().and_then(Path::parent)
            {
                out.push(dir.to_path_buf());
            }
        }
    }
    out
}

/// Pure: a path as compared for "is it inside": separators unified, and on
/// Windows lower-cased (its paths are case-insensitive).
pub fn comparable(p: &Path) -> String {
    let raw = p.to_string_lossy();
    // Windows' canonical form is the verbatim `\\?\C:\...`; everything else
    // (known-folder paths, Ludusavi) uses `C:\...`.
    let raw = if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw.into_owned()
    };
    let s = raw.replace('\\', "/");
    let s = s.trim_end_matches('/').to_string();
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// Pure: whether `path` is `dir` or inside it.
pub fn is_inside(path: &Path, dir: &Path) -> bool {
    let p = comparable(path);
    let d = comparable(dir);
    !d.is_empty() && (p == d || p.starts_with(&format!("{d}/")))
}

/// Whether a path belongs to an addon-managed save. Drop's cloud saves skip
/// these: the addon is the authority for them.
pub fn is_addon_managed(path: &Path) -> bool {
    let mut guard = MANAGED.lock().unwrap_or_else(|p| p.into_inner());
    let dirs = guard.get_or_insert_with(load_managed);
    if dirs.is_empty() {
        return false;
    }
    if dirs.iter().any(|d| is_inside(path, d)) {
        return true;
    }
    // The receipt holds the canonical path; the path asked about may reach
    // the same folder another way (a symlinked data folder, 8.3 names).
    match path.canonicalize() {
        Ok(real) => dirs.iter().any(|d| is_inside(&real, d)),
        Err(_) => path
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .is_some_and(|real| dirs.iter().any(|d| is_inside(&real, d))),
    }
}

/// Whether the save on this device holds play never handed in (it differs
/// from what this device last wrote or handed in). `Err` when it can't be
/// read: callers say so rather than guess.
pub fn local_unsent(addon: &str, ctx: &str, file: &Path) -> Result<bool, AddonError> {
    if !file.exists() {
        return Ok(false);
    }
    let receipt = read_receipt(addon, ctx);
    let sha = sha1_hex(&read_capped(file, MAX_SAVE_BYTES)?);
    Ok(receipt.known_sha1.as_deref() != Some(sha.as_str()))
}

// ── transfers ───────────────────────────────────────────────────────────────

pub fn sha1_hex(data: &[u8]) -> String {
    hex::encode(Sha1::digest(data))
}

fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>, AddonError> {
    let f = fs::File::open(path).map_err(|e| io_error("open the save", e))?;
    let mut data = Vec::new();
    f.take(max + 1)
        .read_to_end(&mut data)
        .map_err(|e| io_error("read the save", e))?;
    if data.len() as u64 > max {
        return Err(AddonError::new("too_large", "This save is too large to send."));
    }
    Ok(data)
}

fn server_auth() -> Result<(String, String), AddonError> {
    let db = borrow_db_checked();
    let signed_out = || AddonError::new("not_signed_in", "Sign in to Drop first.");
    let token = db
        .auth
        .as_ref()
        .and_then(|a| a.web_token.clone())
        .ok_or_else(signed_out)?;
    Ok((db.base_url.trim_end_matches('/').to_string(), token))
}

fn network(e: impl std::fmt::Display) -> AddonError {
    warn!("[ADDONS] request failed: {e}");
    AddonError::new(
        "network",
        "Couldn't reach your Drop server. Check your connection and try again.",
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub path: PathBuf,
    pub profile: String,
    /// The save was already exactly this version; nothing was written.
    pub already_current: bool,
    /// Play that was never handed in was replaced (a copy is in the backups).
    pub replaced_unsent: bool,
}

/// Put a checked-out version into the game's save folder. The caller holds
/// the game against Drop launches; `still_safe` is asked again right before
/// anything on disk changes (the download can take a while, and the game can
/// be started outside Drop meanwhile).
#[allow(clippy::too_many_arguments)]
pub async fn install_save(
    game_id: &str,
    addon: &str,
    ctx: &str,
    version: u64,
    expected_sha1: &str,
    slot: &SaveSlot,
    replace_unsent: bool,
    still_safe: &(dyn Fn() -> Result<(), AddonError> + Send + Sync),
) -> Result<InstallOutcome, AddonError> {
    if !valid_addon_name(addon) || !valid_context(ctx) || !is_sha1_hex(expected_sha1) || version == 0
    {
        return Err(AddonError::new("unsupported", "Bad check-out details."));
    }
    let resolved = resolve_save(game_id, slot)?;
    let receipt = read_receipt(addon, ctx);

    let mut unsent = false;
    if resolved.file.exists() {
        let local = read_capped(&resolved.file, MAX_SAVE_BYTES)?;
        let local_sha = sha1_hex(&local);
        if local_sha == expected_sha1 {
            write_receipt(
                addon,
                ctx,
                &Receipt {
                    path: Some(resolved.file.clone()),
                    base_version: Some(version),
                    known_sha1: Some(local_sha),
                    handed_in: false,
                },
            )?;
            return Ok(InstallOutcome {
                path: resolved.file,
                profile: resolved.profile,
                already_current: true,
                replaced_unsent: false,
            });
        }
        unsent = receipt.known_sha1.as_deref() != Some(local_sha.as_str());
        if unsent && !replace_unsent {
            return Err(AddonError::new(
                "unsent_changes",
                "The save on this device has play that was never checked in. Replace it with the league's? A copy is kept either way.",
            ));
        }
    }

    // Download into memory, checked, before anything on disk changes.
    let (base, token) = server_auth()?;
    let url = format!("{base}/api/v1/addons/{addon}/contexts/{ctx}/versions/{version}/file");
    let mut res = DROP_CLIENT_DOWNLOAD
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(network)?;
    if !res.status().is_success() {
        warn!("[ADDONS] download {url}: HTTP {}", res.status());
        return Err(AddonError::new(
            "bad_download",
            "The league's save couldn't be downloaded. Nothing on this device changed.",
        ));
    }
    let cap = slot.max_bytes.min(MAX_SAVE_BYTES);
    let mut data: Vec<u8> = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(network)? {
        data.extend_from_slice(&chunk);
        if data.len() as u64 > cap {
            return Err(AddonError::new("too_large", "The league's save is too large."));
        }
    }
    let got = sha1_hex(&data);
    if got != expected_sha1 {
        warn!("[ADDONS] download {url}: SHA-1 {got}, expected {expected_sha1}");
        return Err(AddonError::new(
            "bad_download",
            "The downloaded save didn't match the league's copy. Nothing on this device changed.",
        ));
    }

    // Checked again right before writing: the game may have started and the
    // folders may have changed while the download ran.
    still_safe()?;
    check_still_contained(&resolved)?;
    if resolved.file.exists() {
        // Re-read: what is backed up and judged must be what is there now.
        let now = read_capped(&resolved.file, MAX_SAVE_BYTES)?;
        let now_sha = sha1_hex(&now);
        let now_unsent = receipt.known_sha1.as_deref() != Some(now_sha.as_str());
        if now_unsent && !replace_unsent {
            return Err(AddonError::new(
                "unsent_changes",
                "The save on this device has play that was never checked in. Replace it with the league's? A copy is kept either way.",
            ));
        }
        unsent = now_unsent;
        backup(addon, ctx, &resolved.file, if unsent { "unsent" } else { "replaced" })?;
    }
    // Written before the receipt on purpose: a crash in between leaves the
    // old receipt, so the next check-in is refused as diverged and the next
    // check-out lands on "already current". The other order could accept an
    // old file under the new version.
    write_atomic(&resolved.file, &data).map_err(|e| io_error("write the save", e))?;
    write_receipt(
        addon,
        ctx,
        &Receipt {
            path: Some(resolved.file.clone()),
            base_version: Some(version),
            known_sha1: Some(got),
            handed_in: false,
        },
    )?;
    info!(
        "[ADDONS] {addon}/{ctx}: installed version {version} at {}",
        resolved.file.display()
    );
    Ok(InstallOutcome {
        path: resolved.file,
        profile: resolved.profile,
        already_current: false,
        replaced_unsent: unsent,
    })
}

/// Hand the local save in. Returns drop-server's answer as is
/// (`{ ok: true, version }` or `{ ok: false, code, message }`); a refusal
/// that means the play can't be merged keeps a copy in the backups. The local
/// file is never changed.
pub async fn upload_save(
    game_id: &str,
    addon: &str,
    ctx: &str,
    slot: &SaveSlot,
) -> Result<serde_json::Value, AddonError> {
    if !valid_addon_name(addon) || !valid_context(ctx) {
        return Err(AddonError::new("unsupported", "Bad check-in details."));
    }
    let receipt = read_receipt(addon, ctx);
    let Some(base_version) = receipt.base_version else {
        return Err(AddonError::new(
            "no_receipt",
            "This device didn't check this save out, so it can't check it in. Check in from the device you checked out on.",
        ));
    };
    let resolved = resolve_save(game_id, slot)?;
    if !resolved.file.exists() {
        return Err(AddonError::new(
            "missing_save",
            "There's no save on this device to check in.",
        ));
    }
    let data = read_capped(&resolved.file, slot.max_bytes.min(MAX_SAVE_BYTES))?;
    let sha = sha1_hex(&data);
    // Exactly what this device checked out, never played: sending it would
    // hand the league back an unchanged save and end the turn, which is the
    // wrong thing if the player went on to play this turn on another device.
    if !receipt.handed_in && receipt.known_sha1.as_deref() == Some(sha.as_str()) {
        return Err(AddonError::new(
            "nothing_to_send",
            "The save on this device hasn't changed since you checked it out, so there's nothing to check in.",
        ));
    }

    let (base, token) = server_auth()?;
    let url = format!("{base}/api/v1/addons/{addon}/contexts/{ctx}/checkin?base_version={base_version}");
    let res = DROP_CLIENT_DOWNLOAD
        .post(&url)
        .bearer_auth(token)
        .header("content-type", "application/octet-stream")
        .body(data)
        .send()
        .await
        .map_err(network)?;
    if !res.status().is_success() {
        warn!("[ADDONS] check-in {url}: HTTP {}", res.status());
        return Err(AddonError::new(
            "network",
            "Your Drop server refused the check-in. Your save is unchanged on this device.",
        ));
    }
    let body: serde_json::Value = res.json().await.map_err(network)?;
    if body.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        write_receipt(
            addon,
            ctx,
            &Receipt {
                path: Some(resolved.file.clone()),
                base_version: Some(base_version),
                known_sha1: Some(sha),
                handed_in: true,
            },
        )?;
        info!("[ADDONS] {addon}/{ctx}: checked in");
    } else {
        let code = body.get("code").and_then(serde_json::Value::as_str).unwrap_or("");
        warn!("[ADDONS] {addon}/{ctx}: check-in refused: {code}");
        if matches!(code, "not_checked_out" | "diverged")
            && receipt.known_sha1.as_deref() != Some(sha.as_str())
        {
            // This turn can't be merged any more; keep the play for the
            // commissioner. The file in the game's folder stays as it is.
            let _ = backup(addon, ctx, &resolved.file, "unsent");
        }
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> SaveSlot {
        SaveSlot {
            root: "winLocalAppData".into(),
            profiles_dir: vec!["ncaafootball14".into(), "mods".into(), "CFB Revamped".into(), "userdata".into()],
            profile_pattern: PROFILE_PATTERN.into(),
            profile_exclude: vec!["0000000000000000".into()],
            file: vec!["454109B6".into(), "00000001".into(), "DYNASTY-DEFAULT".into(), "DYNASTY-DEFAULT".into()],
            max_bytes: MAX_SAVE_BYTES,
            process_names: vec!["ncaafootball14.exe".into()],
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("drop-addons-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn profiles_dir(root: &Path) -> PathBuf {
        root.join("ncaafootball14/mods/CFB Revamped/userdata")
    }

    #[test]
    fn segments() {
        for ok in ["CFB Revamped", "DYNASTY-DEFAULT", "454109B6", "a.b", "x(1)"] {
            assert!(is_safe_segment(ok), "{ok}");
        }
        for bad in ["", "..", ".", "a/b", "a\\b", "C:", "x.", "x ", " x", ".x", "con", "COM1", "lpt9.txt", "a%2e", &"x".repeat(65)] {
            assert!(!is_safe_segment(bad), "{bad}");
        }
    }

    #[test]
    fn names_and_ids() {
        assert!(valid_addon_name("ncaa14"));
        assert!(!valid_addon_name("NCAA14") && !valid_addon_name("-x") && !valid_addon_name("a/b"));
        assert!(valid_context("12") && !valid_context("0") && !valid_context("01") && !valid_context("1a") && !valid_context("1234567890"));
        assert!(is_sha1_hex(&"a".repeat(40)) && !is_sha1_hex(&"A".repeat(40)) && !is_sha1_hex("abc"));
    }

    #[test]
    fn slot_checks() {
        assert!(check_slot(&slot()).is_ok());
        let mut s = slot();
        s.root = "/etc".into();
        assert_eq!(check_slot(&s).unwrap_err().code, "unsupported");
        let mut s = slot();
        s.file.push("..".into());
        assert_eq!(check_slot(&s).unwrap_err().code, "unsafe_path");
        let mut s = slot();
        s.profile_pattern = ".*".into();
        assert_eq!(check_slot(&s).unwrap_err().code, "unsupported");
        let mut s = slot();
        s.max_bytes = MAX_SAVE_BYTES + 1;
        assert_eq!(check_slot(&s).unwrap_err().code, "unsupported");
    }

    #[test]
    fn no_folder_means_play_first() {
        let root = tmp("nofolder");
        assert_eq!(resolve_save_in(&root, &slot()).unwrap_err().code, "play_first");
        fs::create_dir_all(profiles_dir(&root).join("0000000000000000")).unwrap();
        // only the excluded all-zero profile: still play first
        assert_eq!(resolve_save_in(&root, &slot()).unwrap_err().code, "play_first");
    }

    #[test]
    fn one_profile_resolves() {
        let root = tmp("one");
        fs::create_dir_all(profiles_dir(&root).join("E0000123ABCD4567")).unwrap();
        fs::create_dir_all(profiles_dir(&root).join("not-a-profile")).unwrap();
        let r = resolve_save_in(&root, &slot()).unwrap();
        assert_eq!(r.profile, "E0000123ABCD4567");
        assert!(r.file.ends_with("E0000123ABCD4567/454109B6/00000001/DYNASTY-DEFAULT/DYNASTY-DEFAULT"));
        assert!(r.file.starts_with(&r.profile_real));
    }

    #[test]
    fn several_profiles_are_named_not_guessed() {
        let root = tmp("several");
        fs::create_dir_all(profiles_dir(&root).join("E0000123ABCD4567")).unwrap();
        fs::create_dir_all(profiles_dir(&root).join("E0000123ABCD4568")).unwrap();
        let e = resolve_save_in(&root, &slot()).unwrap_err();
        assert_eq!(e.code, "several_profiles");
        assert_eq!(e.profiles, vec!["E0000123ABCD4567", "E0000123ABCD4568"]);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_lead_outside() {
        use std::os::unix::fs::symlink;
        // a symlinked profiles folder pointing outside the root
        let root = tmp("link-dir");
        let outside = tmp("link-dir-outside");
        fs::create_dir_all(outside.join("E0000123ABCD4567")).unwrap();
        fs::create_dir_all(profiles_dir(&root).parent().unwrap()).unwrap();
        symlink(&outside, profiles_dir(&root)).unwrap();
        assert_eq!(resolve_save_in(&root, &slot()).unwrap_err().code, "unsafe_path");

        // a symlinked folder inside the profile pointing outside
        let root = tmp("link-inner");
        let p = profiles_dir(&root).join("E0000123ABCD4567");
        fs::create_dir_all(&p).unwrap();
        symlink(&outside, p.join("454109B6")).unwrap();
        assert_eq!(resolve_save_in(&root, &slot()).unwrap_err().code, "unsafe_path");

        // the save file itself a symlink
        let root = tmp("link-file");
        let d = profiles_dir(&root).join("E0000123ABCD4567/454109B6/00000001/DYNASTY-DEFAULT");
        fs::create_dir_all(&d).unwrap();
        fs::write(outside.join("target"), b"x").unwrap();
        symlink(outside.join("target"), d.join("DYNASTY-DEFAULT")).unwrap();
        assert_eq!(resolve_save_in(&root, &slot()).unwrap_err().code, "unsafe_path");

        // a symlinked "profile" folder is not a profile at all
        let root = tmp("link-profile");
        fs::create_dir_all(profiles_dir(&root)).unwrap();
        symlink(&outside, profiles_dir(&root).join("E0000123ABCD4567")).unwrap();
        assert_eq!(resolve_save_in(&root, &slot()).unwrap_err().code, "play_first");
    }

    #[test]
    fn pruning_keeps_two_and_the_newest_unsent() {
        let names: Vec<String> = [
            "20260101T000000000-replaced.bin",
            "20260102T000000000-unsent.bin",
            "20260103T000000000-unsent.bin",
            "20260104T000000000-replaced.bin",
            "20260105T000000000-replaced.bin",
            "20260106T000000000-replaced.bin",
        ]
        .map(String::from)
        .to_vec();
        let mut gone = backups_to_prune(&names);
        gone.sort();
        assert_eq!(
            gone,
            vec![
                "20260101T000000000-replaced.bin",
                "20260102T000000000-unsent.bin",
                "20260104T000000000-replaced.bin",
            ]
        );
        assert!(backups_to_prune(&names[..2]).is_empty());
    }

    #[test]
    fn verbatim_windows_paths_compare_equal() {
        assert_eq!(
            comparable(Path::new(r"\\?\C:\Users\A\AppData")),
            comparable(Path::new(r"C:\Users\A\AppData"))
        );
        assert_eq!(comparable(Path::new(r"\\?\UNC\nas\share\x")), "//nas/share/x");
    }

    #[test]
    fn game_ids() {
        assert!(valid_game_id("8fcb1216-8b7a-4da6-8d53-5ac3d4dca1c6"));
        assert!(!valid_game_id("../x") && !valid_game_id("/abs") && !valid_game_id(""));
    }

    #[test]
    fn inside_checks() {
        let d = Path::new("/a/ncaafootball14/x/DYNASTY-DEFAULT");
        assert!(is_inside(Path::new("/a/ncaafootball14/x/DYNASTY-DEFAULT/DYNASTY-DEFAULT"), d));
        assert!(is_inside(d, d));
        assert!(!is_inside(Path::new("/a/ncaafootball14/x/DYNASTY-DEFAULT2/f"), d));
        assert!(!is_inside(Path::new("/a"), d));
        assert!(!is_inside(Path::new("/x"), Path::new("")));
    }

    #[test]
    fn receipt_roundtrip_and_defaults() {
        let r = Receipt {
            path: Some(PathBuf::from("/x/y")),
            base_version: Some(4),
            known_sha1: Some("a".repeat(40)),
            handed_in: true,
        };
        let back: Receipt = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert_eq!(back, r);
        // older or partial receipts load with defaults instead of failing
        let partial: Receipt = serde_json::from_str("{\"baseVersion\":3}").unwrap();
        assert_eq!(partial.base_version, Some(3));
        assert!(!partial.handed_in && partial.path.is_none());
    }

    #[test]
    fn context_dir_layout() {
        assert_eq!(
            context_dir_in(Path::new("/d"), "ncaa14", "3"),
            Path::new("/d/addons/ncaa14/3")
        );
    }
}
