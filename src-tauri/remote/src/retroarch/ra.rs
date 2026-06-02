//! RetroAchievements integration for RetroArch launches.
//!
//! Two concerns:
//!
//! * **Connect credentials** — fetched from local settings or the Drop server
//!   and injected into `retroarch.cfg` so RetroArch authenticates with
//!   RetroAchievements without a manual login. See [`fetch_ra_credentials`].
//! * **ROM-hash verification** — RA identifies a game by an MD5-ish hash of
//!   the ROM. Drop computes the local ROM's hash with the bundled `RAHasher`
//!   CLI and compares it against the server's known-good hashes so the UI can
//!   warn when achievements won't trigger. See [`check_rom_hash`].
//!
//! All Drop-server HTTP goes through the shared retrying [`remote_request`]
//! helper. Per the crate constraint, `serde_json` is unavailable here — every
//! request/response shape is an inner `#[derive(Serialize/Deserialize)]`.

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::requests::{generate_url, remote_request, RemoteRequest};

/// RetroAchievements Connect credentials for RetroArch authentication.
#[derive(Debug, Clone)]
pub struct RACredentials {
    /// RA username (used as `cheevos_username`).
    pub username: String,
    /// Connect token from `dorequest.php?r=login2` (used as `cheevos_token`).
    pub connect_token: String,
}

/// Fetches RetroAchievements Connect credentials.
///
/// Lookup order:
/// 1. Local settings (`ra_username` + `ra_token`) — preferred: works offline
///    and allows an RA account not linked to the Drop account.
/// 2. Drop server (`/api/v1/client/user/ra-credentials`) — linked account.
///
/// Returns `None` if neither path yields a username + Connect token. A failed
/// server fetch is logged and swallowed — RA auto-login is nice-to-have, not
/// a launch blocker.
pub async fn fetch_ra_credentials() -> Option<RACredentials> {
    // 1. Local settings first.
    {
        let db = database::borrow_db_checked();
        if !db.settings.ra_username.is_empty() && !db.settings.ra_token.is_empty() {
            info!(
                "[RETROARCH] Using locally-configured RA credentials for {}",
                db.settings.ra_username
            );
            return Some(RACredentials {
                username: db.settings.ra_username.clone(),
                connect_token: db.settings.ra_token.clone(),
            });
        }
    }

    // 2. Drop server, via the shared retrying helper.
    let url = match generate_url(&["api", "v1", "client", "user", "ra-credentials"], &[]) {
        Ok(u) => u,
        Err(e) => {
            debug!("[RETROARCH] Failed to build RA credentials URL: {e}");
            return None;
        }
    };

    /// Server response shape — inner struct (no `serde_json` in this crate).
    #[derive(Deserialize)]
    struct RACreds {
        username: String,
        #[serde(rename = "connectToken")]
        connect_token: String,
    }

    match remote_request::<RACreds, _>(RemoteRequest::get(url)).await {
        Ok(creds) if !creds.connect_token.is_empty() => {
            info!("[RETROARCH] Got RA credentials for user {}", creds.username);
            Some(RACredentials {
                username: creds.username,
                connect_token: creds.connect_token,
            })
        }
        Ok(_) => {
            debug!("[RETROARCH] RA credentials have empty Connect token");
            None
        }
        Err(e) => {
            debug!("[RETROARCH] Failed to fetch RA credentials: {e}");
            None
        }
    }
}

// ── ROM hash verification ────────────────────────────────────────────────

/// A single valid hash entry from the Drop server (originally from RA's API).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RAHashEntry {
    pub hash: String,
    pub label: String,
    #[serde(default)]
    pub patch_url: String,
}

/// Response from `GET /api/v1/client/game/{id}/ra-hashes`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RAHashesResponse {
    pub console_id: Option<i64>,
    pub hashes: Vec<RAHashEntry>,
}

/// Result of comparing a local ROM hash against RA's known hashes.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum RomHashStatus {
    /// ROM hash matches a known RA hash — achievements will identify the game.
    Match { rom_hash: String, matched_label: String },
    /// ROM hash matches nothing — achievements won't identify the game.
    Mismatch {
        rom_hash: String,
        expected_hashes: Vec<RAHashEntry>,
    },
    /// No RA hashes available (game not linked, or RA has none).
    NoHashData,
    /// Hashing failed (RAHasher missing, process error, …).
    Error { message: String },
}

/// Locates the `RAHasher` binary inside (or next to) the RetroArch install.
fn find_rahasher(emu_root: &Path) -> Option<PathBuf> {
    let exe = if cfg!(target_os = "windows") { "RAHasher.exe" } else { "RAHasher" };
    let candidates = [
        emu_root.join(exe),
        emu_root.parent().map(|p| p.join(exe)).unwrap_or_default(),
    ];
    for c in &candidates {
        if c.is_file() {
            info!("[RA-HASH] Found RAHasher at {}", c.display());
            return Some(c.clone());
        }
    }
    debug!("[RA-HASH] RAHasher not found, searched: {candidates:?}");
    None
}

/// Computes the RetroAchievements hash of a ROM using the `RAHasher` CLI.
///
/// `console_id` is the RA console ID (e.g. 21 = PS2), required by RAHasher.
/// Arguments are passed as a discrete argv (no shell), so a ROM path with
/// spaces or shell metacharacters cannot be misinterpreted.
///
/// Returns the lowercased hex hash, or `None` if hashing fails.
pub fn hash_rom(emu_root: &Path, rom_path: &str, console_id: i64) -> Option<String> {
    let rahasher = find_rahasher(emu_root)?;

    info!("[RA-HASH] Hashing ROM: {rom_path} (console_id={console_id})");

    let output = match std::process::Command::new(&rahasher)
        .arg(console_id.to_string())
        .arg(rom_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            warn!("[RA-HASH] Failed to execute RAHasher: {e}");
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("[RA-HASH] RAHasher exited with {}: {stderr}", output.status);
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // RAHasher prints the hash on a line; some versions print "<hash> <file>".
    let hash = stdout
        .lines().rfind(|l| !l.trim().is_empty())
        .and_then(|l| l.split_whitespace().next())
        .map(|s| s.trim().to_lowercase());

    match &hash {
        Some(h) => info!("[RA-HASH] ROM hash: {h}"),
        None => warn!("[RA-HASH] Could not parse hash from RAHasher output: {stdout:?}"),
    }
    hash
}

/// Fetches valid RA hashes for a game from the Drop server.
///
/// Routes through the shared retrying [`remote_request`] helper; a failed
/// fetch is logged and returns `None` (the caller degrades to `NoHashData`).
pub async fn fetch_ra_hashes(game_id: &str) -> Option<RAHashesResponse> {
    let url = match generate_url(
        &[&format!("/api/v1/client/game/{game_id}/ra-hashes")],
        &[],
    ) {
        Ok(u) => u,
        Err(e) => {
            debug!("[RA-HASH] Failed to build ra-hashes URL: {e}");
            return None;
        }
    };

    match remote_request::<RAHashesResponse, _>(RemoteRequest::get(url)).await {
        Ok(data) => {
            info!(
                "[RA-HASH] Got {} hashes for game {game_id} (console_id={:?})",
                data.hashes.len(),
                data.console_id
            );
            Some(data)
        }
        Err(e) => {
            debug!("[RA-HASH] Failed to fetch RA hashes: {e}");
            None
        }
    }
}

/// Checks whether a local ROM's hash matches any known RA hash.
///
/// The main entry point, called from the process manager at launch time:
/// fetches known hashes from the server, computes the local ROM hash with
/// `RAHasher`, and compares.
pub async fn check_rom_hash(emu_root: &Path, game_id: &str, rom_path: &str) -> RomHashStatus {
    let hash_data = match fetch_ra_hashes(game_id).await {
        Some(d) if !d.hashes.is_empty() => d,
        Some(_) => {
            info!("[RA-HASH] No RA hashes registered for game {game_id}");
            return RomHashStatus::NoHashData;
        }
        None => return RomHashStatus::NoHashData,
    };

    let console_id = match hash_data.console_id {
        Some(id) => id,
        None => {
            warn!("[RA-HASH] No console ID for game {game_id} — cannot hash ROM");
            return RomHashStatus::Error {
                message: "No RA console ID available for this game".to_string(),
            };
        }
    };

    let rom_hash = match hash_rom(emu_root, rom_path, console_id) {
        Some(h) => h,
        None => {
            return RomHashStatus::Error {
                message: "Failed to compute ROM hash (RAHasher not found or failed)".to_string(),
            };
        }
    };

    for entry in &hash_data.hashes {
        if entry.hash.to_lowercase() == rom_hash {
            info!(
                "[RA-HASH] ROM hash MATCH for game {game_id}: {rom_hash} ({})",
                entry.label
            );
            return RomHashStatus::Match {
                rom_hash,
                matched_label: entry.label.clone(),
            };
        }
    }

    warn!(
        "[RA-HASH] ROM hash MISMATCH for game {game_id}: local={rom_hash}, expected={:?}",
        hash_data.hashes.iter().map(|h| &h.hash).collect::<Vec<_>>()
    );
    RomHashStatus::Mismatch {
        rom_hash,
        expected_hashes: hash_data.hashes,
    }
}
