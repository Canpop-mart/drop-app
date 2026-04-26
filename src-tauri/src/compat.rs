//! Compatibility-testing orchestrator (Phase C of the compat-test feature).
//!
//! Wraps the existing launch path with a "test mode" that:
//!   1. Launches the game using the same path as a normal play action.
//!   2. Watches the process for a fixed observation window.
//!   3. Reads the per-launch wine log to extract a crash fingerprint.
//!   4. Classifies the outcome and POSTs it to drop-server's
//!      `POST /api/v1/client/compat/results` endpoint.
//!   5. Kills the process so the next test in a batch starts clean.
//!
//! Most of the launch and log-reading machinery already exists — this module
//! is glue that calls those primitives in test order, plus the classifier.
//!
//! Phase D will add a server-side queue and a background worker loop; for
//! now the user triggers each test manually via `start_compat_test`.

use std::time::{Duration, Instant};

use ::process::{PROCESS_MANAGER, error::ProcessError};
#[cfg(target_os = "linux")]
use database::borrow_db_checked;
use ::remote::{
    error::RemoteAccessError,
    requests::{generate_url, make_authenticated_post},
};
use log::{info, warn};
use serde::{Deserialize, Serialize};

const DEFAULT_TIMEOUT_SECS: u64 = 45;
const POLL_INTERVAL_MS: u64 = 1500;
const LOG_TAIL_LINES: usize = 600;
const LOG_EXCERPT_BYTES: usize = 16 * 1024;

/// Mirrors `GameCompatibilityStatus` from drop-server's Prisma schema.
/// Serialized as snake_case strings to match the API enum mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatStatus {
    AliveRenders,
    AliveNoRender,
    EarlyExit,
    Crash,
    NoLaunch,
    InstallFailed,
}

impl CompatStatus {
    fn as_api_value(self) -> &'static str {
        match self {
            // Match the @map values in compatibility.prisma
            CompatStatus::AliveRenders => "AliveRenders",
            CompatStatus::AliveNoRender => "AliveNoRender",
            CompatStatus::EarlyExit => "EarlyExit",
            CompatStatus::Crash => "Crash",
            CompatStatus::NoLaunch => "NoLaunch",
            CompatStatus::InstallFailed => "InstallFailed",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompatResultPayload<'a> {
    game_id: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proton_version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    log_excerpt: Option<&'a str>,
}

/// Data the frontend sees when a test finishes. Kept narrow on purpose —
/// the frontend uses `status` to decide which follow-up dialog to show
/// (e.g. "did the menu render?" only fires for `AliveNoRender`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatTestOutcome {
    /// One of: `AliveRenders` | `AliveNoRender` | `EarlyExit` | `Crash`
    /// | `NoLaunch` | `InstallFailed` — same string posted to the server.
    pub status: String,
    pub signature: Option<String>,
    pub elapsed_secs: u64,
    /// True if the result was successfully POSTed to drop-server. False
    /// means we tested but the server didn't take it (offline mode, auth
    /// problem); the frontend can warn the user.
    pub posted: bool,
    /// Proton/Wine version that was used to run the test, if detected.
    /// `None` on Windows. On Linux this is best-effort: reflects the
    /// user's default Proton, not necessarily a per-game override.
    pub proton_version: Option<String>,
}

/// Errors surfaced to the frontend. We hand-roll Display + Serialize to
/// stay consistent with the rest of the codebase, which doesn't depend on
/// thiserror.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum CompatTestError {
    LaunchFailed(String),
    Network(String),
}

impl std::fmt::Display for CompatTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LaunchFailed(s) => write!(f, "launch failed: {s}"),
            Self::Network(s) => write!(f, "network error: {s}"),
        }
    }
}

impl std::error::Error for CompatTestError {}

impl From<ProcessError> for CompatTestError {
    fn from(value: ProcessError) -> Self {
        Self::LaunchFailed(value.to_string())
    }
}

impl From<RemoteAccessError> for CompatTestError {
    fn from(value: RemoteAccessError) -> Self {
        Self::Network(value.to_string())
    }
}

/// Optional caller-provided knobs. All defaults are sane for ad-hoc
/// "Test this game" UI use.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatTestOptions {
    pub timeout_secs: Option<u64>,
    /// If true, leaves the game running after the observation window so
    /// the user can poke at the menu and decide whether it rendered.
    /// Defaults to false (auto-kill so batches stay clean).
    pub leave_running: Option<bool>,
    /// Free-form note attached to the result row. Useful when re-testing
    /// after a fix: "after vsync=off patch".
    pub notes: Option<String>,
    /// Detected proton/wine version string for the launch (e.g.
    /// "GE-Proton10-32"). Reported to the server so reruns under different
    /// runtimes show as distinct rows.
    pub proton_version: Option<String>,
}

/// Launch a game, observe it for `timeout_secs`, classify the outcome,
/// optionally kill it, and POST the result. Returns the classification
/// to the frontend so it can prompt the user to promote AliveNoRender →
/// AliveRenders.
#[tauri::command]
pub async fn start_compat_test(
    game_id: String,
    version_index: Option<usize>,
    options: Option<CompatTestOptions>,
) -> Result<CompatTestOutcome, CompatTestError> {
    let opts = options.unwrap_or_default();
    let timeout = Duration::from_secs(opts.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));
    let leave_running = opts.leave_running.unwrap_or(false);
    let index = version_index.unwrap_or(0);

    info!(
        "[compat] starting test for game {} (timeout {:?}, leave_running={})",
        game_id, timeout, leave_running
    );

    // Best-effort Proton version detection for Linux launches. We read the
    // user's currently-configured default proton path and use its basename
    // as the version label (e.g. "GE-Proton10-32"). This is an approximation
    // — per-game overrides aren't reflected — but it's accurate for the
    // common case where the user picks one Proton and tests against it.
    // On Windows there's no Proton involved, so we leave the field None.
    let detected_proton_version = detect_proton_version();
    let proton_version_for_post = opts
        .proton_version
        .clone()
        .or_else(|| detected_proton_version.clone());

    // ── 1. Launch ─────────────────────────────────────────────────────
    // launch_process is sync but calls block_on internally (e.g. Ludusavi
    // save sync). Calling it directly from this async context panics with
    // "Cannot start a runtime from within a runtime" because the tokio
    // multi-thread scheduler refuses nested block_on calls. spawn_blocking
    // moves the work onto tokio's blocking thread pool where block_on is
    // permitted — same trick the existing `launch_game` Tauri command
    // gets for free by being declared `pub fn` (not `pub async fn`).
    let launch_result = {
        let game_id_clone = game_id.clone();
        tokio::task::spawn_blocking(move || {
            let mut process_manager_lock = PROCESS_MANAGER.lock();
            process_manager_lock.launch_process(game_id_clone, index)
        })
        .await
        .map_err(|e| CompatTestError::LaunchFailed(format!("join error: {e}")))?
    };

    match launch_result {
        Ok(()) => {}
        Err(ProcessError::RequiredDependency(_, _)) => {
            // The game (or one of its dependencies) wasn't installed.
            // Treat as InstallFailed since "we couldn't even attempt to
            // launch it for compat reasons" is the most accurate bucket.
            return finish(
                &game_id,
                proton_version_for_post.as_deref(),
                &opts,
                CompatStatus::InstallFailed,
                Some("required dependency not installed".to_string()),
                0,
                None,
            )
            .await;
        }
        Err(other) => {
            let sig = format!("launch error: {other}");
            return finish(
                &game_id,
                proton_version_for_post.as_deref(),
                &opts,
                CompatStatus::NoLaunch,
                Some(sig),
                0,
                None,
            )
            .await;
        }
    }

    // ── 2. Observe ────────────────────────────────────────────────────
    let started_at = Instant::now();
    let mut ever_alive = false;
    while started_at.elapsed() < timeout {
        let alive = {
            let lock = PROCESS_MANAGER.lock();
            lock.is_game_running(&game_id)
        };
        if alive {
            ever_alive = true;
        }
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }

    let still_alive = {
        let lock = PROCESS_MANAGER.lock();
        lock.is_game_running(&game_id)
    };
    let elapsed_secs = started_at.elapsed().as_secs();

    // ── 3. Read log + classify ────────────────────────────────────────
    let log_tail = read_log_safe(&game_id);
    let crash_signature = log_tail.as_ref().and_then(|t| extract_crash_signature(t));
    let render_failure_signature = log_tail
        .as_ref()
        .and_then(|t| extract_render_failure_signature(t));

    let (status, signature) = match (still_alive, &crash_signature, ever_alive) {
        // Process still alive — almost always AliveNoRender unless we
        // can be more specific. If the log shows known render-pipeline
        // failure markers (vkd3d-proton swap_chain spam, Godot "all
        // display drivers failed", etc.), surface them in the signature
        // so the user (and any future auto-classifier) sees "this game
        // looks alive but here's why it probably isn't" without having
        // to dig through the log themselves.
        (true, _, _) => (CompatStatus::AliveNoRender, render_failure_signature),
        (false, Some(sig), _) => (CompatStatus::Crash, Some(sig.clone())),
        (false, None, true) => (
            CompatStatus::EarlyExit,
            log_tail
                .as_deref()
                .and_then(extract_last_err_line)
                .map(|s| s.to_string()),
        ),
        (false, None, false) => (CompatStatus::NoLaunch, None),
    };

    // ── 4. Cleanup ────────────────────────────────────────────────────
    // kill_game on Windows just drops the Child handle, which sends a
    // soft termination. Many games (especially launchers and Godot/Unity
    // titles with crashpad helpers) take a few seconds to actually exit,
    // and some don't respect the soft signal at all — leaving them alive
    // would stack windows during a batch run. Verify with retries; if
    // the game still won't die after several attempts, log a loud
    // warning and move on so the batch doesn't deadlock.
    if still_alive && !leave_running {
        ensure_killed(&game_id).await;
    }

    finish(
        &game_id,
        proton_version_for_post.as_deref(),
        &opts,
        status,
        signature,
        elapsed_secs,
        log_tail,
    )
    .await
}

/// Asks drop-server for the next compat-test work item belonging to the
/// authenticated client. Returns `None` (HTTP 204) when there's nothing
/// left to test in the user's installed library.
///
/// Drives the batch worker's outer loop — the frontend polls this, runs
/// `start_compat_test` against each returned game, and stops when this
/// returns `None`. Pure read: doesn't reserve, doesn't lock; if the same
/// client polls twice it gets the same item until that item is tested.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatWorkItem {
    pub game_id: String,
    pub name: String,
    pub metadata_id: String,
    pub last_tested_at: Option<String>,
    pub platform: Option<String>,
}

#[tauri::command]
pub async fn fetch_next_compat_work() -> Result<Option<CompatWorkItem>, CompatTestError> {
    let platform = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        ""
    };
    let url = generate_url(
        &["api", "v1", "client", "compat", "work", "next"],
        &[("platform", platform)],
    )
    .map_err(CompatTestError::from)?;

    let response = ::remote::requests::make_authenticated_get(url)
        .await
        .map_err(|e| CompatTestError::Network(e.to_string()))?;

    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(CompatTestError::Network(format!(
            "fetch_next_compat_work returned {}",
            response.status()
        )));
    }

    let item: CompatWorkItem = response
        .json()
        .await
        .map_err(|e| CompatTestError::Network(format!("parse: {e}")))?;
    Ok(Some(item))
}

/// Promote an `AliveNoRender` result to `AliveRenders` after the user
/// confirms the menu rendered. The frontend calls this from the
/// post-test dialog. Implemented as a separate POST so the user's
/// "yes I saw the menu" answer becomes a new history row rather than
/// silently mutating the prior result.
#[tauri::command]
pub async fn confirm_compat_render(
    game_id: String,
    rendered: bool,
    notes: Option<String>,
) -> Result<(), CompatTestError> {
    info!(
        "[compat] confirm_compat_render invoked for {game_id} (rendered={rendered})"
    );
    let status = if rendered {
        CompatStatus::AliveRenders
    } else {
        CompatStatus::AliveNoRender
    };
    let outcome = post_result(
        &game_id,
        status,
        None,
        None,
        notes.as_deref(),
        None,
    )
    .await;
    match &outcome {
        Ok(()) => info!("[compat] promotion POST succeeded for {game_id}"),
        Err(e) => warn!("[compat] promotion POST failed for {game_id}: {e}"),
    }
    outcome.map_err(CompatTestError::from)
}

// ── helpers ─────────────────────────────────────────────────────────────

async fn finish(
    game_id: &str,
    proton_version: Option<&str>,
    opts: &CompatTestOptions,
    status: CompatStatus,
    signature: Option<String>,
    elapsed_secs: u64,
    log_tail: Option<String>,
) -> Result<CompatTestOutcome, CompatTestError> {
    let posted = match post_result(
        game_id,
        status,
        signature.as_deref(),
        proton_version,
        opts.notes.as_deref(),
        log_tail.as_deref(),
    )
    .await
    {
        Ok(()) => true,
        Err(e) => {
            warn!("[compat] failed to POST result for {game_id}: {e}");
            false
        }
    };

    Ok(CompatTestOutcome {
        status: status.as_api_value().to_string(),
        signature,
        elapsed_secs,
        posted,
        proton_version: proton_version.map(|s| s.to_string()),
    })
}

/// Aggressively kill a game after a compat test, verifying it actually
/// died and retrying if it didn't. The existing kill_game on Windows just
/// drops the Child handle, which sends a soft termination — many games
/// take seconds to react, and some launchers don't react at all. Without
/// verification the next test in a batch run would launch on top of the
/// previous one, leaving windows stacked on screen.
///
/// Strategy:
///   1. Send the soft kill via process_manager
///   2. Poll is_game_running with backoff up to KILL_VERIFY_ATTEMPTS times
///   3. If still alive, re-issue the soft kill (which on Windows can
///      sometimes succeed on a retry when the first attempt raced
///      with a child-process spawn)
///   4. If still alive after all attempts, log loudly and move on so the
///      batch doesn't deadlock on a stuck game
async fn ensure_killed(game_id: &str) {
    const KILL_VERIFY_ATTEMPTS: usize = 5;
    const KILL_VERIFY_INTERVAL_MS: u64 = 800;

    // First soft kill
    let game_id_owned = game_id.to_string();
    let _ = tokio::task::spawn_blocking(move || {
        PROCESS_MANAGER.lock().kill_game(game_id_owned)
    })
    .await;

    for attempt in 1..=KILL_VERIFY_ATTEMPTS {
        tokio::time::sleep(Duration::from_millis(KILL_VERIFY_INTERVAL_MS)).await;

        let game_id_owned = game_id.to_string();
        let still_alive = tokio::task::spawn_blocking(move || {
            PROCESS_MANAGER.lock().is_game_running(&game_id_owned)
        })
        .await
        .unwrap_or(false);

        if !still_alive {
            return;
        }
        info!(
            "[compat] {game_id} still running after kill attempt {attempt}/{KILL_VERIFY_ATTEMPTS}, retrying"
        );
        let game_id_owned = game_id.to_string();
        let _ = tokio::task::spawn_blocking(move || {
            PROCESS_MANAGER.lock().kill_game(game_id_owned)
        })
        .await;
    }

    warn!(
        "[compat] {game_id} survived {KILL_VERIFY_ATTEMPTS} kill attempts — moving on, but \
         the next compat test may launch on top of it. Check process_manager.kill_game logic \
         (likely needs taskkill /F /T on Windows or wineserver -k on Linux)."
    );
}

/// Look for known render-pipeline failure markers in the wine/Godot log
/// when the process is "alive" at the end of the observation window.
/// Lots of games are technically running but completely failing to render
/// (STS2's vkd3d-proton swap_chain spam is the canonical example) — the
/// marker count is a much stronger signal than "process exists".
///
/// Returns a short signature like "vkd3d-proton swap_chain x615" that
/// drops into the GameCompatibilityResult.signature column. The user
/// reviewing AliveNoRender results sees this without having to dig
/// through the raw log.
fn extract_render_failure_signature(log: &str) -> Option<String> {
    let mut swap_chain_resize = 0usize;
    let mut display_server_failed = false;
    let mut window_creation_failed = false;
    let mut last_godot_error: Option<&str> = None;

    for line in log.lines() {
        if line.contains("swap_chain_resize") || line.contains("ResizeBuffers") {
            swap_chain_resize += 1;
        }
        if line.contains("Unable to create DisplayServer") {
            display_server_failed = true;
        }
        if line.contains("Failed to create Windows OS window")
            || line.contains("Failed to create main window")
        {
            window_creation_failed = true;
        }
        if line.starts_with("ERROR:") || line.contains(" | ERROR | ") {
            last_godot_error = Some(line);
        }
    }

    // Order matters — pick the most specific signal first.
    if window_creation_failed {
        return Some("window creation failed (likely missing renderer)".to_string());
    }
    if display_server_failed {
        return Some("DisplayServer init failed".to_string());
    }
    if swap_chain_resize > 10 {
        return Some(format!("vkd3d-proton swap_chain x{swap_chain_resize}"));
    }
    if let Some(err) = last_godot_error {
        // Trim to a short fingerprint
        let trimmed = err.trim();
        if trimmed.len() > 180 {
            return Some(trimmed[..180].to_string());
        }
        return Some(trimmed.to_string());
    }
    None
}

/// Best-effort detection of the Proton/Wine version that will run a game.
/// Reads `applications.default_proton_path` from the local DB and returns
/// its directory basename (typically "GE-Proton10-32" or similar).
/// `None` on Windows, or on Linux when no default is set / readable.
#[cfg(target_os = "linux")]
fn detect_proton_version() -> Option<String> {
    let db = borrow_db_checked();
    let path = db.applications.default_proton_path.as_ref()?;
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

#[cfg(not(target_os = "linux"))]
fn detect_proton_version() -> Option<String> {
    None
}

async fn post_result(
    game_id: &str,
    status: CompatStatus,
    signature: Option<&str>,
    proton_version: Option<&str>,
    notes: Option<&str>,
    log_excerpt: Option<&str>,
) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["api", "v1", "client", "compat", "results"], &[])?;
    // Trim log to keep request bodies bounded; the server caps it server-side
    // anyway, but pre-trimming saves bandwidth when the log is huge.
    let trimmed_log = log_excerpt.map(|s| {
        if s.len() > LOG_EXCERPT_BYTES {
            &s[s.len() - LOG_EXCERPT_BYTES..]
        } else {
            s
        }
    });

    let payload = CompatResultPayload {
        game_id,
        status: status.as_api_value(),
        signature,
        proton_version,
        notes,
        log_excerpt: trimmed_log,
    };

    let response = make_authenticated_post(url, &payload).await?;

    if !response.status().is_success() {
        return Err(RemoteAccessError::UnparseableResponse(format!(
            "compat POST returned {}",
            response.status()
        )));
    }
    Ok(())
}

/// Returns the tail of the most recent per-launch log for `game_id`, or
/// None when no log is available. Falls back to empty rather than failing
/// hard — a missing log just means we classify on process state alone.
fn read_log_safe(game_id: &str) -> Option<String> {
    let dir = {
        let lock = PROCESS_MANAGER.lock();
        lock.get_log_dir(game_id)
    };
    if !dir.exists() {
        return None;
    }
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut newest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str())?;
        // Skip *-error.log; we want the combined wine debug log
        if !name.ends_with(".log") || name.ends_with("-error.log") {
            continue;
        }
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        match &newest {
            None => newest = Some((path, mtime)),
            Some((_, prev)) if mtime > *prev => newest = Some((path, mtime)),
            _ => {}
        }
    }
    let (path, _) = newest?;
    let content = std::fs::read_to_string(&path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(LOG_TAIL_LINES);
    Some(lines[start..].join("\n"))
}

/// Find the typical Wine page-fault marker and return a short signature
/// that's stable across runs (so multiple games crashing the same way
/// share a fingerprint). Looks for:
///   "Unhandled page fault on read access to ... at address <hex>"
///   "Unhandled exception: ... at <hex>"
fn extract_crash_signature(log: &str) -> Option<String> {
    for line in log.lines().rev().take(200) {
        if let Some(rest) = line.find("Unhandled page fault") {
            // Capture the address — `at address 00006FFFFF9BE5E0`
            if let Some(addr_idx) = line[rest..].find("at address ") {
                let addr_start = rest + addr_idx + "at address ".len();
                let addr: String = line[addr_start..]
                    .chars()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .collect();
                if !addr.is_empty() {
                    return Some(format!("page fault 0x{addr}"));
                }
            }
            return Some("Unhandled page fault".to_string());
        }
        if line.contains("Unhandled exception") {
            // Address often appears at end like `(0x006fffff9be5e0)`
            if let Some(open) = line.rfind('(') {
                let inner: String = line[open + 1..]
                    .chars()
                    .take_while(|c| *c != ')')
                    .collect();
                if inner.starts_with("0x") {
                    return Some(format!("Unhandled exception {inner}"));
                }
            }
            return Some("Unhandled exception".to_string());
        }
    }
    None
}

/// For early_exit: pick the last `err:` line from the wine debug stream.
/// It's the most actionable thing in a clean exit log.
fn extract_last_err_line(log: &str) -> Option<&str> {
    log.lines()
        .rev()
        .take(200)
        .find(|line| line.contains("err:") || line.contains("ERROR:"))
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.len() > 180 {
                &trimmed[..180]
            } else {
                trimmed
            }
        })
}
