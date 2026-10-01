//! RetroAchievements integration for RetroArch launches.
//!
//! Two concerns:
//!
//! * **Connect credentials** — fetched from the Drop server (a local copy is
//!   used only when it can't be reached) and injected into `retroarch.cfg` so RetroArch authenticates with
//!   RetroAchievements without a manual login. See [`fetch_ra_credentials`].
//! * **Credential expiry** — Connect tokens are password-derived, last about
//!   45 to 60 days and cannot be refreshed. RetroArch never tells Drop it was
//!   rejected, so expiry is read back out of RetroArch's own log after the
//!   session. See [`detect_ra_login_failure`].
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
use std::time::SystemTime;

use crate::requests::{generate_url, remote_request, RemoteRequest};

/// RetroAchievements Connect credentials for RetroArch authentication.
#[derive(Debug, Clone)]
pub struct RACredentials {
    /// RA username (used as `cheevos_username`).
    pub username: String,
    /// Connect token from `dorequest.php?r=login2` (used as `cheevos_token`).
    pub connect_token: String,
}

/// How long the server lookup may take before the local copy is used. The
/// launch path bounds the whole call at 2 s; this leaves room for the local
/// fallback inside that.
const SERVER_LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

/// What a credential sync found, for the settings screens.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RaSyncReport {
    /// `linked`, `linked_no_token` (linked but the server holds no Connect
    /// token: the player must sign in again once), `not_linked`, or
    /// `unreachable` (the server couldn't be asked; nothing was changed).
    pub status: &'static str,
    /// The RA account the server has, when it said.
    pub server_username: Option<String>,
    /// Who RetroArch will sign in as now. `None`: nobody.
    pub signs_in_as: Option<String>,
    /// Why the server couldn't be asked (`unreachable` only).
    pub error: Option<String>,
}

/// Fetches RetroAchievements Connect credentials for RetroArch.
/// See [`sync_ra_credentials`].
pub async fn fetch_ra_credentials() -> Option<RACredentials> {
    sync_ra_credentials().await.0
}

/// Asks the Drop server for this player's RA credentials and brings the local
/// copy in line. The server is authoritative: RA linking happens there (web
/// page or any client), so what it holds now is what RetroArch signs in as.
///
/// The route always answers 200 with an explicit `linked` flag, so only a
/// well-formed Drop answer can change the local copy; a 404 or any other
/// failure (a reverse proxy while the container restarts, say) never does.
///
/// * `linked: true` with a token: use it and overwrite the local copy. A
///   re-link to another account, a new password, or a token from another
///   device all take effect. A fresh token also clears a recorded "expired"
///   state. A token that is itself the one recorded as rejected is not
///   injected: RetroArch would only fail again.
/// * `linked: true` without a token (accounts linked before tokens were
///   stored): keep using the local token if it is for the same account, and
///   report `linked_no_token` so settings can ask for one sign-in.
/// * `linked: false`: the account was unlinked. Clear the local copy so
///   RetroArch stops signing in as an account the server no longer tracks.
/// * Anything else: use the local copy (unless it is the rejected token) and
///   report `unreachable`.
///
/// Servers from before the `linked` flag answer `{ username, connectToken }`
/// or 404; the first is read as linked, the second as unreachable.
pub async fn sync_ra_credentials() -> (Option<RACredentials>, RaSyncReport) {
    let report = |status: &'static str,
                  server_username: Option<String>,
                  signs_in_as: Option<&RACredentials>,
                  error: Option<String>| RaSyncReport {
        status,
        server_username,
        signs_in_as: signs_in_as.map(|c| c.username.clone()),
        error,
    };

    let url = match generate_url(&["api", "v1", "client", "user", "ra-credentials"], &[]) {
        Ok(u) => u,
        Err(e) => {
            warn!("[RETROARCH] Failed to build RA credentials URL: {e}");
            let local = local_fallback("could not build the request URL");
            let r = report("unreachable", None, local.as_ref(), Some(e.to_string()));
            return (local, r);
        }
    };

    /// Server response shape — inner struct (no `serde_json` in this crate).
    #[derive(Deserialize)]
    struct RACredsResponse {
        /// Missing on servers from before the flag existed.
        #[serde(default)]
        linked: Option<bool>,
        #[serde(default)]
        username: Option<String>,
        #[serde(default, rename = "connectToken")]
        connect_token: Option<String>,
    }

    let answer = tokio::time::timeout(
        SERVER_LOOKUP_TIMEOUT,
        remote_request::<RACredsResponse, _>(RemoteRequest::get(url)),
    )
    .await;

    let resp = match answer {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => {
            let why = e.to_string();
            let local = local_fallback(&why);
            let r = report("unreachable", None, local.as_ref(), Some(why));
            return (local, r);
        }
        Err(_) => {
            let why = "the server did not answer in time".to_string();
            let local = local_fallback(&why);
            let r = report("unreachable", None, local.as_ref(), Some(why));
            return (local, r);
        }
    };

    let username = resp.username.unwrap_or_default();
    let token = resp.connect_token.unwrap_or_default();
    match (resp.linked, username.is_empty(), token.is_empty()) {
        (Some(false), _, _) => {
            info!("[RETROARCH] No RA account linked on the server; clearing any local copy");
            clear_local_copy();
            (None, report("not_linked", None, None, None))
        }
        (Some(true) | None, false, false) => {
            if is_expired_token(&token) {
                warn!(
                    "[RETROARCH] The server's RA token for {username} is the one RetroAchievements \
                     already rejected; not injecting it (sign in again)"
                );
                return (None, report("linked", Some(username), None, None));
            }
            info!("[RETROARCH] Using the server-linked RA account {username}");
            // A token that isn't the recorded dead one: the expiry is history.
            heal_expired_state(&token);
            {
                let mut db = database::borrow_db_mut_checked();
                db.settings.ra_username = username.clone();
                db.settings.ra_token = token.clone();
            }
            let creds = RACredentials { username: username.clone(), connect_token: token };
            let r = report("linked", Some(username), Some(&creds), None);
            (Some(creds), r)
        }
        (Some(true), false, true) => {
            // Keep a local token only if it is for this same account.
            let local = local_fallback("the server has no Connect token for this account")
                .filter(|c| c.username.eq_ignore_ascii_case(&username));
            warn!(
                "[RETROARCH] RA account {username} is linked on the server without a Connect \
                 token; the player needs to sign in again once"
            );
            let r = report("linked_no_token", Some(username), local.as_ref(), None);
            (local, r)
        }
        _ => {
            let why = "unexpected response from the server".to_string();
            let local = local_fallback(&why);
            let r = report("unreachable", None, local.as_ref(), Some(why));
            (local, r)
        }
    }
}

/// The local copy of the server-linked RA credentials, for when the server
/// can't be asked. `None` when there is none or it is the rejected token.
fn local_fallback(why: &str) -> Option<RACredentials> {
    let local = {
        let db = database::borrow_db_checked();
        (!db.settings.ra_username.is_empty() && !db.settings.ra_token.is_empty()).then(|| {
            RACredentials {
                username: db.settings.ra_username.clone(),
                connect_token: db.settings.ra_token.clone(),
            }
        })
    }?;
    if is_expired_token(&local.connect_token) {
        warn!(
            "[RETROARCH] RA server lookup failed ({why}) and the local token for {} has expired",
            local.username
        );
        return None;
    }
    info!(
        "[RETROARCH] RA server lookup failed ({why}); using the local copy for {}",
        local.username
    );
    Some(local)
}

/// Drops the local copy of the RA credentials (the server no longer has an
/// account linked). Expiry state goes with it: there is no token left to be
/// expired.
fn clear_local_copy() {
    let mut db = database::borrow_db_mut_checked();
    if db.settings.ra_username.is_empty() && db.settings.ra_token.is_empty() {
        return;
    }
    db.settings.ra_username = String::new();
    db.settings.ra_token = String::new();
    db.settings.ra_expired_token = String::new();
}

// ── Credential expiry ────────────────────────────────────────────────────
//
// RA's Connect token is derived from the password, lives ~45-60 days and has
// no refresh endpoint. When it dies, RetroArch keeps running (achievements
// just never unlock) and blanks the dead token in its own config on exit,
// which used to be pointless because Drop wrote the same dead token back on
// the next launch. The only place the rejection is ever stated is RetroArch's
// log, so that is where we read it from.

/// Substrings rcheevos logs when RetroAchievements rejects the credentials.
///
/// `Login failed:` is the direct rejection (the Deck's log carries
/// "Invalid user/token combination" as its reason). `Load failed (-28)` is the
/// follow-on: the game's achievement set can't load because the session was
/// never authenticated. Either one on its own is enough.
const RA_LOGIN_FAILURE_MARKERS: &[&str] = &[
    "[RCHEEVOS] Login failed:",
    "Load failed (-28): Login required",
];

/// Scans the newest `emu_root/logs/retroarch__*.log` for an RA login
/// rejection, returning the offending log line.
///
/// `launched_at` is the wall clock at which the session that just ended was
/// started; any log older than that belongs to a previous session and is
/// ignored. Without that bound a rejection logged weeks ago would be
/// attributed to whatever token is current, which permanently latches the
/// expiry flag: the session that would clear it never writes a log of its own
/// when RetroArch is script-wrapped (no `--appendconfig`, so Drop's `log_dir`
/// never reaches it).
///
/// Called on the RetroArch exit path. `None` means either "no rejection" or
/// "no log from this session to read" — both are treated as "credentials
/// still fine", because guessing expiry would lock a working account out of
/// auto-login.
pub fn detect_ra_login_failure(emu_root: &Path, launched_at: SystemTime) -> Option<String> {
    let log_path = super::logs::newest_retroarch_log(emu_root, launched_at)?;

    match super::logs::first_line_matching(&log_path, RA_LOGIN_FAILURE_MARKERS) {
        Some(line) => {
            info!(
                "[RA-AUTH] RetroAchievements login rejection found in {}: {line}",
                log_path.display()
            );
            Some(line)
        }
        None => {
            debug!("[RA-AUTH] No RA login failure in {}", log_path.display());
            None
        }
    }
}

/// Whether `token` is the exact Connect token RetroArch already rejected.
pub fn is_expired_token(token: &str) -> bool {
    let db = database::borrow_db_checked();
    !db.settings.ra_expired_token.is_empty() && db.settings.ra_expired_token == token
}

/// Records `token` as rejected so the next launch stops injecting it and the
/// settings UI can ask for a fresh sign-in.
pub fn mark_credentials_expired(token: &str) {
    let mut db = database::borrow_db_mut_checked();
    db.settings.ra_expired_token = token.to_string();
}

/// Clears the expired-credentials state.
///
/// The settings commands clear the field directly instead of calling this —
/// they already hold the write guard, and the DB lock is not reentrant.
fn clear_expired_credentials() {
    let mut db = database::borrow_db_mut_checked();
    db.settings.ra_expired_token = String::new();
}

/// Clears the expired state if `token` is a different token to the dead one.
fn heal_expired_state(token: &str) {
    let stale = {
        let db = database::borrow_db_checked();
        !db.settings.ra_expired_token.is_empty() && db.settings.ra_expired_token != token
    };
    if stale {
        info!("[RA-AUTH] Fresh RetroAchievements token — clearing expired state");
        clear_expired_credentials();
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

#[cfg(test)]
mod tests {
    use super::detect_ra_login_failure;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    /// Two lines lifted from a real Deck log, one per marker.
    const REJECTED_LOG: &str = "\
[INFO] [RCHEEVOS]: Load started
[INFO] [RCHEEVOS] Login failed: Invalid user/token combination.
[INFO] [RCHEEVOS]: Load failed (-28): Login required
";
    const HEALTHY_LOG: &str = "\
[INFO] [RCHEEVOS]: Load started
[INFO] [RCHEEVOS]: Login succeeded
[INFO] [RCHEEVOS]: Load done
";

    /// Creates an isolated `<tmp>/<name>/logs` tree. The caller removes it.
    fn emu_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("drop-ra-test-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("logs")).unwrap();
        root
    }

    fn write_log(root: &Path, file: &str, body: &str) {
        std::fs::write(root.join("logs").join(file), body).unwrap();
    }

    /// Windows file times move in ~15ms steps while `SystemTime::now` is
    /// precise, so every ordering assertion in these tests is spaced out
    /// rather than trusting two back-to-back operations to differ.
    fn settle() {
        std::thread::sleep(Duration::from_millis(50));
    }

    /// The launch instant for a session whose logs are written next.
    fn launched_now() -> SystemTime {
        let now = SystemTime::now();
        settle();
        now
    }

    #[test]
    fn rejection_is_detected() {
        let root = emu_root("rejection");
        let launched = launched_now();
        write_log(&root, "retroarch__2026_08_11__09_26_25.log", REJECTED_LOG);
        let found =
            detect_ra_login_failure(&root, launched).expect("rejection should be detected");
        assert!(found.contains("Invalid user/token combination"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The second marker alone is enough — a session can fail to load the
    /// achievement set without the login line ever being written.
    #[test]
    fn load_failed_marker_alone_is_enough() {
        let root = emu_root("load-failed");
        let launched = launched_now();
        write_log(
            &root,
            "retroarch__2026_08_11__09_26_25.log",
            "[INFO] [RCHEEVOS]: Load failed (-28): Login required\n",
        );
        assert!(detect_ra_login_failure(&root, launched).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn healthy_log_reports_nothing() {
        let root = emu_root("healthy");
        let launched = launched_now();
        write_log(&root, "retroarch__2026_08_11__09_26_25.log", HEALTHY_LOG);
        assert_eq!(detect_ra_login_failure(&root, launched), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Only the newest log counts. Yesterday's rejection must not keep the
    /// account flagged after the user has re-linked and played again — that
    /// would be the same "can never recover" trap in a new place.
    #[test]
    fn only_the_newest_log_is_read() {
        let root = emu_root("newest");
        let launched = launched_now();
        write_log(&root, "retroarch__2026_08_10__09_00_00.log", REJECTED_LOG);
        settle();
        write_log(&root, "retroarch__2026_08_11__09_26_25.log", HEALTHY_LOG);
        assert_eq!(detect_ra_login_failure(&root, launched), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A non-UTF-8 ROM filename earlier in the log must not stop the scan
    /// before the RCHEEVOS lines, which are written later.
    #[test]
    fn non_utf8_line_does_not_end_the_scan() {
        let root = emu_root("non-utf8");
        let launched = launched_now();
        // Shift-JIS bytes for a ROM name, invalid as UTF-8.
        let mut body = b"[INFO] Loading content: \x83h\x83\x89\x83S\x83\x93.iso\n".to_vec();
        body.extend_from_slice(REJECTED_LOG.as_bytes());
        std::fs::write(
            root.join("logs").join("retroarch__2026_08_11__09_26_25.log"),
            &body,
        )
        .unwrap();
        let found = detect_ra_login_failure(&root, launched)
            .expect("rejection after a non-UTF-8 line should still be found");
        assert!(found.contains("Invalid user/token combination"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The latch guard: a rejection from an earlier session must not be
    /// attributed to the token this session used. A script-wrapped RetroArch
    /// never gets Drop's `log_dir`, so the session that should clear the flag
    /// writes no log at all and only the old rejection is left on disk.
    #[test]
    fn log_older_than_the_launch_is_ignored() {
        let root = emu_root("stale");
        write_log(&root, "retroarch__2026_07_01__09_00_00.log", REJECTED_LOG);
        settle();
        // Launch happens after the only log on disk was written.
        assert_eq!(detect_ra_login_failure(&root, SystemTime::now()), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_log_dir_is_not_a_failure() {
        let root = std::env::temp_dir().join("drop-ra-test-does-not-exist");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(detect_ra_login_failure(&root, SystemTime::UNIX_EPOCH), None);
    }

    /// Files RetroArch didn't write are ignored, even when newer.
    #[test]
    fn unrelated_files_are_ignored() {
        let root = emu_root("unrelated");
        let launched = launched_now();
        write_log(&root, "retroarch__2026_08_11__09_26_25.log", REJECTED_LOG);
        settle();
        write_log(&root, "notes.txt", HEALTHY_LOG);
        assert!(detect_ra_login_failure(&root, launched).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
