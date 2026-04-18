//! Sunshine-based remote play / game streaming management.
//!
//! Drop manages Sunshine as a bundled tool — auto-downloading it on first use,
//! generating config files, and controlling it as a child process.
//!
//! Sunshine API: https://localhost:{SUNSHINE_WEB_PORT}/api/*
//! Protocol: Moonlight/GameStream

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use database::{borrow_db_checked, GameDownloadStatus};
use log::{info, warn};
use rand::Rng;
use remote::streaming_sessions;
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex};

// ── Constants ─────────────────────────────────────────────────────────

const SUNSHINE_VERSION: &str = "2025.924.154138";

#[cfg(target_os = "windows")]
const SUNSHINE_ARCHIVE: &str = "Sunshine-Windows-AMD64-portable.zip";
#[cfg(target_os = "linux")]
const SUNSHINE_ARCHIVE: &str = "sunshine.AppImage";
#[cfg(target_os = "macos")]
const SUNSHINE_ARCHIVE: &str = "sunshine.rb"; // macOS uses Homebrew

/// Default port family for Sunshine (web UI, RTSP, etc derive from this base).
const SUNSHINE_BASE_PORT: u16 = 47989;
/// Web UI / API port = base + 1.
const SUNSHINE_WEB_PORT: u16 = 47990;

// ── Display resolution management (Windows only) ─────────────────────

/// Saved original display resolution for restoration after streaming ends.
#[cfg(target_os = "windows")]
struct SavedResolution {
    width: u32,
    height: u32,
}

#[cfg(target_os = "windows")]
static SAVED_RESOLUTION: std::sync::LazyLock<Mutex<Option<SavedResolution>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Change the primary display resolution (Windows only).
/// Returns the previous resolution so it can be restored later.
#[cfg(target_os = "windows")]
fn set_display_resolution(width: u32, height: u32) -> Result<(u32, u32), String> {
    use winapi::um::wingdi::{
        DEVMODEW, DM_PELSWIDTH, DM_PELSHEIGHT,
    };
    use winapi::um::winuser::{
        EnumDisplaySettingsW, ChangeDisplaySettingsW,
        CDS_FULLSCREEN, DISP_CHANGE_SUCCESSFUL, ENUM_CURRENT_SETTINGS,
    };

    unsafe {
        // Get current resolution
        let mut current: DEVMODEW = std::mem::zeroed();
        current.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut current) == 0 {
            return Err("Failed to get current display settings".to_string());
        }
        let old_width = current.dmPelsWidth;
        let old_height = current.dmPelsHeight;

        if old_width == width && old_height == height {
            info!("[DISPLAY] Resolution already {}x{}, no change needed", width, height);
            return Ok((old_width, old_height));
        }

        // Set new resolution
        let mut new_mode = current;
        new_mode.dmPelsWidth = width;
        new_mode.dmPelsHeight = height;
        new_mode.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT;

        let result = ChangeDisplaySettingsW(&mut new_mode, CDS_FULLSCREEN);
        if result == DISP_CHANGE_SUCCESSFUL as i32 {
            info!("[DISPLAY] Changed resolution from {}x{} to {}x{}", old_width, old_height, width, height);
            Ok((old_width, old_height))
        } else {
            Err(format!("ChangeDisplaySettingsW failed with code {}", result))
        }
    }
}

/// Restore the display resolution to what it was before streaming started.
#[cfg(target_os = "windows")]
async fn restore_display_resolution() {
    let saved = {
        let mut guard = SAVED_RESOLUTION.lock().await;
        guard.take()
    };
    if let Some(res) = saved {
        match set_display_resolution(res.width, res.height) {
            Ok(_) => info!("[DISPLAY] Restored resolution to {}x{}", res.width, res.height),
            Err(e) => warn!("[DISPLAY] Failed to restore resolution: {e}"),
        }
    }
}

// ── Tool management ───────────────────────────────────────────────────

/// Get Drop's tools directory.
fn tools_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("drop")
        .join("tools")
}

/// Get the Sunshine installation directory.
fn sunshine_dir() -> PathBuf {
    tools_dir().join("sunshine")
}

/// Get the Sunshine config directory (separate from binary).
fn sunshine_config_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("drop")
        .join("sunshine-config")
}

/// Find the Sunshine binary — check Drop's tools dir, then PATH.
fn find_sunshine() -> Option<PathBuf> {
    // Check Drop's bundled tools directory
    #[cfg(target_os = "windows")]
    let bundled = sunshine_dir().join("sunshine.exe");
    #[cfg(target_os = "linux")]
    let bundled = sunshine_dir().join("sunshine.AppImage");
    #[cfg(target_os = "macos")]
    let bundled = sunshine_dir().join("sunshine");

    if bundled.exists() {
        return Some(bundled);
    }

    // Check PATH
    let name = if cfg!(target_os = "windows") { "sunshine.exe" } else { "sunshine" };
    if let Ok(output) = Command::new(name).arg("--version").output() {
        if output.status.success() {
            return Some(PathBuf::from(name));
        }
    }

    // Check common system locations
    #[cfg(target_os = "linux")]
    {
        for path in &["/usr/bin/sunshine", "/usr/local/bin/sunshine"] {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Check if Sunshine is installed and return its path.
#[tauri::command]
pub fn check_sunshine() -> Option<String> {
    find_sunshine().map(|p| p.to_string_lossy().to_string())
}

/// Download and install Sunshine to Drop's tools directory.
#[tauri::command]
pub async fn install_sunshine() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        return Err("On macOS, install Sunshine via Homebrew: brew install sunshine".to_string());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let download_url = format!(
            "https://github.com/LizardByte/Sunshine/releases/download/v{}/{}",
            SUNSHINE_VERSION, SUNSHINE_ARCHIVE
        );

        let install_dir = sunshine_dir();
        std::fs::create_dir_all(&install_dir)
            .map_err(|e| format!("Failed to create sunshine dir: {e}"))?;

        info!("[SUNSHINE] Downloading from {}", download_url);

        let response = reqwest::get(&download_url)
            .await
            .map_err(|e| format!("Download failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Download failed: HTTP {}", response.status()));
        }

        let bytes = response.bytes().await.map_err(|e| format!("Download failed: {e}"))?;
        info!("[SUNSHINE] Downloaded {} bytes", bytes.len());

        #[cfg(target_os = "windows")]
        {
            // Extract portable zip
            let cursor = std::io::Cursor::new(bytes);
            let mut archive = zip::ZipArchive::new(cursor)
                .map_err(|e| format!("Failed to open archive: {e}"))?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)
                    .map_err(|e| format!("Archive error: {e}"))?;
                let name = file.name().to_string();
                if name.ends_with('/') {
                    let dir = install_dir.join(&name);
                    let _ = std::fs::create_dir_all(&dir);
                    continue;
                }
                // Strip leading directory if present (e.g. "Sunshine/sunshine.exe" → "sunshine.exe")
                let out_name = name.rsplit('/').next().unwrap_or(&name);
                let out_path = install_dir.join(out_name);
                if let Some(parent) = out_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let mut out_file = std::fs::File::create(&out_path)
                    .map_err(|e| format!("Failed to create file: {e}"))?;
                std::io::copy(&mut file, &mut out_file)
                    .map_err(|e| format!("Failed to extract: {e}"))?;
            }

            let exe = install_dir.join("sunshine.exe");
            info!("[SUNSHINE] Installed to {}", exe.display());
            Ok(exe.to_string_lossy().to_string())
        }

        #[cfg(target_os = "linux")]
        {
            // AppImage — just write it and make executable
            let out_path = install_dir.join("sunshine.AppImage");
            std::fs::write(&out_path, &bytes)
                .map_err(|e| format!("Failed to write AppImage: {e}"))?;

            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to set permissions: {e}"))?;

            info!("[SUNSHINE] Installed to {}", out_path.display());
            Ok(out_path.to_string_lossy().to_string())
        }
    }
}

// ── Configuration generation ──────────────────────────────────────────

/// Sunshine app entry for apps.json.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct SunshineApp {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    #[serde(default)]
    pub auto_detach: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prep_cmd: Vec<PrepCmd>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PrepCmd {
    #[serde(rename = "do")]
    pub do_cmd: String,
    pub undo: String,
}

/// The top-level apps.json structure.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SunshineAppsConfig {
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub apps: Vec<SunshineApp>,
}

/// Quality presets for streaming.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum StreamQuality {
    LowLatency,
    Balanced,
    Quality,
}

/// Generate sunshine.conf with Drop-specific settings.
fn generate_sunshine_conf(
    config_dir: &Path,
    admin_username: &str,
    admin_password: &str,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;

    let conf_path = config_dir.join("sunshine.conf");
    let apps_path = config_dir.join("apps.json");
    let credentials_path = config_dir.join("credentials.json");
    let state_path = config_dir.join("state.json");

    let conf = format!(
        r#"# Drop-managed Sunshine configuration
# Do not edit manually — Drop regenerates this file.

# Network
port = {base_port}
origin_pin_allowed = lan
origin_web_ui_allowed = lan

# Paths
file_state = {state}
credentials_file = {creds}
file_apps = {apps}

# Display
fps = [30, 60, 120]
resolutions = [
  352x240,
  480x360,
  858x480,
  1280x720,
  1280x800,
  1920x1080,
  1920x1200,
  2560x1440,
  3840x2160
]

# Streaming defaults
channels = 1
fec_percentage = 20

# Logging
min_log_level = 2
"#,
        base_port = SUNSHINE_BASE_PORT,
        state = state_path.to_string_lossy().replace('\\', "/"),
        creds = credentials_path.to_string_lossy().replace('\\', "/"),
        apps = apps_path.to_string_lossy().replace('\\', "/"),
    );

    std::fs::write(&conf_path, conf)
        .map_err(|e| format!("Failed to write sunshine.conf: {e}"))?;

    // Create empty apps.json if it doesn't exist
    if !apps_path.exists() {
        let empty_apps = SunshineAppsConfig::default();
        let json = serde_json::to_string_pretty(&empty_apps)
            .map_err(|e| format!("Failed to serialize apps.json: {e}"))?;
        std::fs::write(&apps_path, json)
            .map_err(|e| format!("Failed to write apps.json: {e}"))?;
    }

    info!("[SUNSHINE] Generated config at {}", conf_path.display());
    Ok(conf_path)
}

/// Register a game in Sunshine's apps.json so it can be launched by Moonlight.
pub fn register_game_app(
    game_id: &str,
    game_name: &str,
    launch_cmd: Option<&str>,
    cover_path: Option<&str>,
) -> Result<(), String> {
    let config_dir = sunshine_config_dir();
    let apps_path = config_dir.join("apps.json");

    let mut config = if apps_path.exists() {
        let json = std::fs::read_to_string(&apps_path)
            .map_err(|e| format!("Failed to read apps.json: {e}"))?;
        serde_json::from_str::<SunshineAppsConfig>(&json)
            .unwrap_or_default()
    } else {
        SunshineAppsConfig::default()
    };

    // Remove existing entry for this game (if any)
    config.apps.retain(|a| a.name != game_name);

    // Add the new entry
    config.apps.push(SunshineApp {
        name: game_name.to_string(),
        cmd: launch_cmd.map(|s| s.to_string()),
        working_dir: None,
        image_path: cover_path.map(|s| s.to_string()),
        auto_detach: true,
        prep_cmd: Vec::new(),
    });

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize apps.json: {e}"))?;
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;
    std::fs::write(&apps_path, json)
        .map_err(|e| format!("Failed to write apps.json: {e}"))?;

    info!("[SUNSHINE] Registered app '{}' (game_id={})", game_name, game_id);
    Ok(())
}

/// Unregister a game from Sunshine's apps.json.
pub fn unregister_game_app(game_name: &str) -> Result<(), String> {
    let apps_path = sunshine_config_dir().join("apps.json");
    if !apps_path.exists() {
        return Ok(());
    }

    let json = std::fs::read_to_string(&apps_path)
        .map_err(|e| format!("Failed to read apps.json: {e}"))?;
    let mut config: SunshineAppsConfig = serde_json::from_str(&json)
        .unwrap_or_default();

    config.apps.retain(|a| a.name != game_name);

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize apps.json: {e}"))?;
    std::fs::write(&apps_path, json)
        .map_err(|e| format!("Failed to write apps.json: {e}"))?;

    Ok(())
}

// ── Process management ────────────────────────────────────────────────

/// Global handle to the running Sunshine process.
static SUNSHINE_PROCESS: std::sync::LazyLock<Mutex<Option<std::process::Child>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Tracks active host-side streaming sessions with cancellation signals.
/// Sending `true` on the watch channel signals the heartbeat loop to stop.
static ACTIVE_HOST_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, watch::Sender<bool>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Sunshine process status.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SunshineStatus {
    pub installed: bool,
    pub running: bool,
    pub binary_path: Option<String>,
    pub web_ui_port: u16,
    pub version: String,
}

/// Get the current Sunshine status.
#[tauri::command]
pub async fn sunshine_status() -> SunshineStatus {
    info!("[SUNSHINE] sunshine_status() called");
    let binary_path = find_sunshine();
    let installed = binary_path.is_some();
    info!("[SUNSHINE] installed={}, path={:?}", installed, binary_path.as_ref().map(|p| p.display().to_string()));

    let running = {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        if let Some(ref mut child) = *guard {
            // Check if process is still alive
            match child.try_wait() {
                Ok(None) => {
                    info!("[SUNSHINE] Process is still running");
                    true
                }
                Ok(Some(status)) => {
                    info!("[SUNSHINE] Process exited with status: {}", status);
                    *guard = None; // exited — clean up
                    false
                }
                Err(e) => {
                    warn!("[SUNSHINE] Failed to check process status: {e}");
                    *guard = None;
                    false
                }
            }
        } else {
            info!("[SUNSHINE] No managed Sunshine process");
            false
        }
    };

    let status = SunshineStatus {
        installed,
        running,
        binary_path: binary_path.map(|p| p.to_string_lossy().to_string()),
        web_ui_port: SUNSHINE_WEB_PORT,
        version: SUNSHINE_VERSION.to_string(),
    };
    info!("[SUNSHINE] Returning status: installed={}, running={}", status.installed, status.running);
    status
}

/// Start the Sunshine process with Drop's config.
/// Returns the web UI URL.
#[tauri::command]
pub async fn start_sunshine(
    admin_username: String,
    admin_password: String,
) -> Result<String, String> {
    let binary = find_sunshine()
        .ok_or("Sunshine is not installed. Install it first.")?;

    // Check if already running
    {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        if let Some(ref mut child) = *guard {
            if child.try_wait().map_or(false, |s| s.is_none()) {
                return Ok(format!("https://localhost:{}", SUNSHINE_WEB_PORT));
            }
            *guard = None;
        }
    }

    // Generate config
    let config_dir = sunshine_config_dir();
    let conf_path = generate_sunshine_conf(&config_dir, &admin_username, &admin_password)?;

    info!("[SUNSHINE] Starting: {} {}", binary.display(), conf_path.display());

    let child = Command::new(&binary)
        .arg(conf_path.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start Sunshine: {e}"))?;

    let pid = child.id();
    info!("[SUNSHINE] Started with PID {}", pid);

    {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        *guard = Some(child);
    }

    Ok(format!("https://localhost:{}", SUNSHINE_WEB_PORT))
}

/// Stop the running Sunshine process.
#[tauri::command]
pub async fn stop_sunshine() -> Result<(), String> {
    let mut guard = SUNSHINE_PROCESS.lock().await;
    if let Some(mut child) = guard.take() {
        info!("[SUNSHINE] Stopping process (PID {})", child.id());

        // Try graceful shutdown first
        #[cfg(unix)]
        {
            unsafe {
                libc::kill(child.id() as i32, libc::SIGTERM);
            }
            // Give it a moment to clean up
            std::thread::sleep(std::time::Duration::from_millis(500));
            if child.try_wait().map_or(true, |s| s.is_none()) {
                let _ = child.kill();
            }
        }

        #[cfg(not(unix))]
        {
            let _ = child.kill();
        }

        let _ = child.wait();
        info!("[SUNSHINE] Stopped");
        Ok(())
    } else {
        Ok(()) // Not running — that's fine
    }
}

// ── Sunshine API client (talks to the running Sunshine instance) ──────

/// Make an authenticated request to the Sunshine API.
async fn sunshine_api_request(
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    username: &str,
    password: &str,
) -> Result<serde_json::Value, String> {
    let url = format!("https://localhost:{}/api{}", SUNSHINE_WEB_PORT, path);

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // Sunshine uses self-signed certs
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let mut req = client.request(method, &url)
        .basic_auth(username, Some(password));

    if let Some(body) = body {
        req = req.json(&body);
    }

    let resp = req.send().await
        .map_err(|e| format!("Sunshine API request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Sunshine API error: {} - {}", status, text));
    }

    resp.json::<serde_json::Value>().await
        .map_err(|e| format!("Failed to parse Sunshine response: {e}"))
}

/// Send a PIN to Sunshine for Moonlight pairing.
#[tauri::command]
pub async fn sunshine_send_pin(
    pin: String,
    client_name: String,
    username: String,
    password: String,
) -> Result<bool, String> {
    let body = serde_json::json!({
        "pin": pin,
        "name": client_name,
    });

    let result = sunshine_api_request(
        reqwest::Method::POST,
        "/pin",
        Some(body),
        &username,
        &password,
    ).await?;

    // The API returns a status indicating success
    Ok(result.get("status").and_then(|s| s.as_bool()).unwrap_or(false))
}

/// List apps registered in Sunshine.
#[tauri::command]
pub async fn sunshine_list_apps(
    username: String,
    password: String,
) -> Result<serde_json::Value, String> {
    sunshine_api_request(
        reqwest::Method::GET,
        "/apps",
        None,
        &username,
        &password,
    ).await
}

/// Register a game for streaming via the Sunshine API.
/// This creates/updates the app in the running Sunshine instance.
#[tauri::command]
pub async fn sunshine_register_game(
    game_id: String,
    game_name: String,
    launch_command: Option<String>,
    username: String,
    password: String,
) -> Result<(), String> {
    // Update apps.json on disk
    register_game_app(&game_id, &game_name, launch_command.as_deref(), None)?;

    // Also push to the running instance via API
    let body = serde_json::json!({
        "name": game_name,
        "cmd": launch_command.unwrap_or_default(),
        "auto-detach": true,
    });

    let _ = sunshine_api_request(
        reqwest::Method::POST,
        "/apps",
        Some(body),
        &username,
        &password,
    ).await; // Non-fatal if API fails — disk config is the source of truth

    Ok(())
}

/// Get list of connected/paired Moonlight clients.
#[tauri::command]
pub async fn sunshine_list_clients(
    username: String,
    password: String,
) -> Result<serde_json::Value, String> {
    sunshine_api_request(
        reqwest::Method::GET,
        "/clients/list",
        None,
        &username,
        &password,
    ).await
}

// ── Server-side streaming session management ────────────────────────
//
// These commands talk to the Drop server (not the local Sunshine instance)
// using JWT client auth via `make_authenticated_post` / `make_authenticated_get`.


/// Create a new streaming session on the server.
#[tauri::command]
pub async fn streaming_create_session(
    game_id: Option<String>,
    host_local_ip: Option<String>,
) -> Result<String, String> {
    info!("[STREAMING] streaming_create_session called: game_id={:?}, host_local_ip={:?}", game_id, host_local_ip);
    let result = streaming_sessions::start_streaming_session(
        game_id.as_deref(),
        host_local_ip.as_deref(),
    )
    .await
    .map_err(|e| {
        warn!("[STREAMING] create_session failed: {e}");
        e.to_string()
    });
    if let Ok(ref id) = result {
        info!("[STREAMING] Session created: {}", id);
    }
    result
}

/// Mark a streaming session as ready on the server.
#[tauri::command]
pub async fn streaming_mark_ready(
    session_id: String,
    pairing_pin: Option<String>,
) -> Result<(), String> {
    info!("[STREAMING] streaming_mark_ready called: session_id={}, has_pin={}", session_id, pairing_pin.is_some());
    streaming_sessions::mark_session_ready(
        &session_id,
        pairing_pin.as_deref(),
    )
    .await
    .map_err(|e| {
        warn!("[STREAMING] mark_ready failed: {e}");
        e.to_string()
    })
}

/// Stop a streaming session on the server.
#[tauri::command]
pub async fn streaming_stop_session(session_id: String) -> Result<(), String> {
    info!("[STREAMING] streaming_stop_session called: session_id={}", session_id);
    streaming_sessions::stop_streaming_session(&session_id)
        .await
        .map_err(|e| {
            warn!("[STREAMING] stop_session failed: {e}");
            e.to_string()
        })
}

/// Stop all host-side streaming sessions (cancels heartbeat loops, stops server sessions, kills Sunshine).
#[tauri::command]
pub async fn stop_all_host_sessions() -> Result<u32, String> {
    let mut sessions = ACTIVE_HOST_SESSIONS.lock().await;
    let count = sessions.len() as u32;
    info!("[STREAMING] Stopping {} active host session(s)", count);
    for (sid, tx) in sessions.drain() {
        info!("[STREAMING] Cancelling host session {}", sid);
        let _ = tx.send(true);
    }
    // Restore display resolution (Windows only)
    #[cfg(target_os = "windows")]
    {
        restore_display_resolution().await;
    }

    // Also stop Sunshine if running
    {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        if let Some(ref mut child) = *guard {
            info!("[STREAMING] Killing Sunshine process");
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;
    }
    Ok(count)
}

/// Send a heartbeat for an active streaming session.
#[tauri::command]
pub async fn streaming_heartbeat(
    session_id: String,
    status: Option<String>,
) -> Result<(), String> {
    streaming_sessions::heartbeat_streaming(
        &session_id,
        status.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// List all active streaming sessions for this user.
#[tauri::command]
pub async fn streaming_list_sessions() -> Result<Vec<streaming_sessions::StreamingSession>, String> {
    streaming_sessions::list_streaming_sessions()
        .await
        .map_err(|e| e.to_string())
}

/// Get connection info for joining a streaming session.
#[tauri::command]
pub async fn streaming_get_connection_info(
    session_id: String,
) -> Result<streaming_sessions::StreamingConnectionInfo, String> {
    streaming_sessions::get_streaming_connection_info(&session_id)
        .await
        .map_err(|e| e.to_string())
}

// ── Moonlight client (receiver side) ──────────────────────────────────

/// Find the Moonlight binary — check PATH, then common locations.
fn find_moonlight() -> Option<PathBuf> {
    // Check PATH first
    #[cfg(target_os = "windows")]
    let names = &["moonlight.exe", "Moonlight.exe"];
    #[cfg(not(target_os = "windows"))]
    let names = &["moonlight"];

    for name in names {
        if let Ok(output) = Command::new(name).arg("--version").output() {
            if output.status.success() || !output.stdout.is_empty() {
                return Some(PathBuf::from(name));
            }
        }
    }

    // Check common locations
    #[cfg(target_os = "windows")]
    {
        let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        for path in &[
            format!("{}\\drop\\tools\\moonlight\\Moonlight.exe", appdata),
            format!("{}\\Moonlight Game Streaming\\Moonlight.exe", program_files),
            format!("{}\\Moonlight\\Moonlight.exe", program_files),
        ] {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Check common binary locations
        for path in &["/usr/bin/moonlight", "/usr/local/bin/moonlight", "/usr/bin/moonlight-qt"] {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }

        // Check flatpak (common on Steam Deck)
        let flatpak_bin = if Path::new("/usr/bin/flatpak").exists() {
            "/usr/bin/flatpak"
        } else {
            "flatpak"
        };
        if let Ok(output) = Command::new(flatpak_bin)
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .env_remove("APPDIR")
            .env_remove("APPIMAGE")
            .env("LD_LIBRARY_PATH", "")
            .args(["info", "com.moonlight_stream.Moonlight"])
            .output()
        {
            if output.status.success() {
                // Return a sentinel — we'll launch via flatpak run
                return Some(PathBuf::from("flatpak:com.moonlight_stream.Moonlight"));
            }
        }
    }

    None
}

/// Build a `Command` for Moonlight, handling flatpak sentinel.
/// Clears LD_LIBRARY_PATH for flatpak to avoid AppImage OpenSSL conflicts.
fn moonlight_command(moonlight_str: &str) -> Command {
    if moonlight_str.starts_with("flatpak:") {
        let flatpak_bin = if Path::new("/usr/bin/flatpak").exists() {
            "/usr/bin/flatpak"
        } else {
            "flatpak"
        };
        let mut cmd = Command::new(flatpak_bin);
        cmd.env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .env_remove("APPDIR")
            .env_remove("APPIMAGE")
            .env("LD_LIBRARY_PATH", "")
            .arg("run")
            .arg("com.moonlight_stream.Moonlight");
        cmd
    } else {
        Command::new(moonlight_str)
    }
}

/// Install Moonlight if not already present.
/// On Linux (including Steam Deck), installs via flatpak from Flathub.
/// On Windows, downloads the portable installer.
async fn install_moonlight() -> Result<PathBuf, String> {
    info!("[MOONLIGHT] Moonlight not found, attempting auto-install...");

    #[cfg(target_os = "linux")]
    {
        // Install via flatpak (most reliable on Steam Deck).
        // IMPORTANT: Clear LD_LIBRARY_PATH so the system flatpak binary doesn't
        // pick up the AppImage's bundled OpenSSL libs (which cause version conflicts).
        info!("[MOONLIGHT] Installing via flatpak...");

        // Use /usr/bin/flatpak explicitly and clear LD_LIBRARY_PATH to escape AppImage sandbox
        let flatpak = if Path::new("/usr/bin/flatpak").exists() {
            "/usr/bin/flatpak"
        } else {
            "flatpak"
        };

        // Ensure flathub remote is added.
        // Clear ALL AppImage env vars so the system flatpak & its deps (libostree)
        // don't accidentally load the AppImage-bundled OpenSSL.
        let _ = Command::new(flatpak)
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .env_remove("APPDIR")
            .env_remove("APPIMAGE")
            .env("LD_LIBRARY_PATH", "")
            .args(["remote-add", "--if-not-exists", "--user", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"])
            .output();

        let output = Command::new(flatpak)
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .env_remove("APPDIR")
            .env_remove("APPIMAGE")
            .env("LD_LIBRARY_PATH", "")
            .args(["install", "--user", "-y", "flathub", "com.moonlight_stream.Moonlight"])
            .output()
            .map_err(|e| format!("Failed to run flatpak install: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Flatpak install failed: {}", stderr.trim()));
        }

        info!("[MOONLIGHT] Installed via flatpak successfully");
        Ok(PathBuf::from("flatpak:com.moonlight_stream.Moonlight"))
    }

    #[cfg(target_os = "windows")]
    {
        // Download portable Moonlight from GitHub
        let version = "6.1.0";
        let url = format!(
            "https://github.com/moonlight-stream/moonlight-qt/releases/download/v{}/MoonlightPortable-x64-{}.zip",
            version, version
        );

        let install_dir = PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()))
            .join("drop")
            .join("tools")
            .join("moonlight");
        std::fs::create_dir_all(&install_dir)
            .map_err(|e| format!("Failed to create moonlight dir: {e}"))?;

        info!("[MOONLIGHT] Downloading from {}", url);
        let response = reqwest::get(&url)
            .await
            .map_err(|e| format!("Download failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Download failed: HTTP {}", response.status()));
        }

        let bytes = response.bytes().await.map_err(|e| format!("Download failed: {e}"))?;
        info!("[MOONLIGHT] Downloaded {} bytes", bytes.len());

        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| format!("Failed to open archive: {e}"))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| format!("Archive error: {e}"))?;
            let name = file.name().to_string();
            if name.ends_with('/') {
                let _ = std::fs::create_dir_all(install_dir.join(&name));
                continue;
            }
            let out_path = install_dir.join(&name);
            if let Some(parent) = out_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut out_file = std::fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create file: {e}"))?;
            std::io::copy(&mut file, &mut out_file)
                .map_err(|e| format!("Failed to extract: {e}"))?;
        }

        let exe = install_dir.join("Moonlight.exe");
        if exe.exists() {
            info!("[MOONLIGHT] Installed to {}", exe.display());
            Ok(exe)
        } else {
            // Try to find it in a subdirectory
            for entry in std::fs::read_dir(&install_dir).map_err(|e| format!("{e}"))? {
                if let Ok(entry) = entry {
                    let candidate = entry.path().join("Moonlight.exe");
                    if candidate.exists() {
                        info!("[MOONLIGHT] Installed to {}", candidate.display());
                        return Ok(candidate);
                    }
                }
            }
            Err("Moonlight.exe not found after extraction".to_string())
        }
    }

    #[cfg(target_os = "macos")]
    {
        Err("Auto-install not supported on macOS. Please install Moonlight manually.".to_string())
    }
}

/// Global handle to the running Moonlight process (receiver side).
static MOONLIGHT_PROCESS: std::sync::LazyLock<Mutex<Option<std::process::Child>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Kill the running Moonlight process (called when the streaming session ends).
#[tauri::command]
pub async fn kill_moonlight() -> Result<(), String> {
    let mut guard = MOONLIGHT_PROCESS.lock().await;
    if let Some(mut child) = guard.take() {
        info!("[MOONLIGHT] Killing Moonlight process (PID {})", child.id());
        let _ = child.kill();
        let _ = child.wait();
    }

    // On Linux, the child handle may only be the flatpak wrapper which exits
    // immediately while the real Moonlight GUI keeps running.  Use system-level
    // kill to ensure the actual process is gone.
    #[cfg(target_os = "linux")]
    {
        // Try flatpak kill first (cleanest for flatpak installs)
        let flatpak_bin = if Path::new("/usr/bin/flatpak").exists() {
            "/usr/bin/flatpak"
        } else {
            "flatpak"
        };
        let _ = Command::new(flatpak_bin)
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .env_remove("APPDIR")
            .env_remove("APPIMAGE")
            .env("LD_LIBRARY_PATH", "")
            .args(["kill", "com.moonlight_stream.Moonlight"])
            .output();

        // Also pkill as a fallback for non-flatpak installs
        let _ = Command::new("pkill").args(["-f", "moonlight"]).output();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "Moonlight.exe"])
            .output();
    }

    info!("[MOONLIGHT] Moonlight killed");
    Ok(())
}

/// Launch Moonlight pointed at a specific host for streaming.
/// If `pin` is provided, Moonlight will attempt to auto-pair first.
/// Auto-installs Moonlight if not found.
#[tauri::command]
pub async fn launch_moonlight(
    host: String,
    port: u16,
    pin: Option<String>,
    app_name: Option<String>,
) -> Result<(), String> {
    let moonlight = match find_moonlight() {
        Some(m) => m,
        None => install_moonlight().await?,
    };

    let moonlight_str = moonlight.to_string_lossy().to_string();
    info!("[MOONLIGHT] Found at: {}", moonlight_str);
    info!("[MOONLIGHT] Connecting to {}:{}, pin={}", host, port, pin.is_some());

    let address = format!("{}:{}", host, port);

    // Try to pair using the PIN, but only if not already paired.
    // `moonlight list <address>` succeeds and shows apps when already paired.
    if let Some(ref pin_value) = pin {
        let mut already_paired = false;

        // Check if we're already paired by listing apps on the host
        let mut list_cmd = moonlight_command(&moonlight_str);
        list_cmd.args(["list", &address]);
        if let Ok(output) = list_cmd.output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // If the list command succeeds and returns app names, we're paired
            if output.status.success() && !stdout.trim().is_empty() {
                info!("[MOONLIGHT] Already paired with {} (apps listed), skipping pair step", address);
                already_paired = true;
            }
        }

        if !already_paired {
            info!("[MOONLIGHT] Attempting to pair with PIN...");
            let mut pair_cmd = moonlight_command(&moonlight_str);
            pair_cmd.args(["pair", &address, "--pin", pin_value]);

            match pair_cmd.output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        info!("[MOONLIGHT] Pairing successful: {}", stdout.trim());
                    } else {
                        warn!("[MOONLIGHT] Pairing failed: {} {}", stdout.trim(), stderr.trim());
                    }
                }
                Err(e) => {
                    warn!("[MOONLIGHT] Pairing command failed: {e}");
                }
            }
        }
    }

    // Kill any existing Moonlight process before launching a new one
    {
        let mut guard = MOONLIGHT_PROCESS.lock().await;
        if let Some(mut old) = guard.take() {
            info!("[MOONLIGHT] Killing previous Moonlight instance");
            let _ = old.kill();
            let _ = old.wait();
        }
    }

    // Launch Moonlight in stream mode — stream the registered game app (or Desktop as fallback)
    let stream_app = app_name.unwrap_or_else(|| "Desktop".to_string());
    info!("[MOONLIGHT] Starting stream to {} app '{}'...", address, stream_app);
    let mut cmd = moonlight_command(&moonlight_str);
    cmd.args(["stream", &address, &stream_app]);

    let child = cmd.spawn()
        .map_err(|e| format!("Failed to launch Moonlight: {e}"))?;

    info!("[MOONLIGHT] Moonlight launched (PID {})", child.id());
    {
        let mut guard = MOONLIGHT_PROCESS.lock().await;
        *guard = Some(child);
    }

    Ok(())
}

// ── Device listing & remote install ──────────────────────────────────

/// List all registered client devices for the current user.
/// Filters out the current device (by `isSelf` from server, plus a hostname
/// fallback to catch stale client registrations).
#[tauri::command]
pub async fn list_devices(game_id: Option<String>) -> Result<Vec<streaming_sessions::ClientDevice>, String> {
    let mut devices = streaming_sessions::list_devices(game_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    // The server marks the current client as `isSelf`, but stale registrations
    // of the same machine (e.g. after re-auth) won't have that flag.  Also
    // filter out any device whose name matches this machine's hostname pattern.
    let local_name = format!(
        "{} (Desktop)",
        gethostname::gethostname().to_string_lossy()
    )
    .to_lowercase();
    let local_platform = std::env::consts::OS.to_lowercase();

    devices.retain(|d| {
        if d.is_self {
            return false;
        }
        // Catch stale registrations of the same machine
        let same_host = d.name.to_lowercase() == local_name
            && d.platform.to_lowercase() == local_platform;
        !same_host
    });

    Ok(devices)
}

/// Request a remote install of a game on another device.
#[tauri::command]
pub async fn request_remote_install(
    game_id: String,
    target_client_id: Option<String>,
) -> Result<(), String> {
    streaming_sessions::request_remote_install(&game_id, target_client_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Sync this client's installed game IDs to the server.
#[tauri::command]
pub async fn sync_installed_games() -> Result<(), String> {
    let game_ids: Vec<String> = {
        let db = borrow_db_checked();
        db.applications
            .game_statuses
            .iter()
            .filter(|(_, status)| matches!(status, GameDownloadStatus::Installed { .. }))
            .map(|(id, _)| id.clone())
            .collect()
    };
    info!("[STREAMING] Syncing {} installed games to server", game_ids.len());
    streaming_sessions::sync_installed_games(game_ids)
        .await
        .map_err(|e| e.to_string())
}

// ── Push-based streaming (background poller on host side) ─────────

/// Request a stream from another device (called by the receiving client, e.g. Steam Deck).
#[tauri::command]
pub async fn streaming_request_stream(
    game_id: String,
    target_client_id: Option<String>,
) -> Result<String, String> {
    info!("[STREAMING] streaming_request_stream called: game_id={}, target={:?}", game_id, target_client_id);
    let session_id = streaming_sessions::request_stream(&game_id, target_client_id.as_deref())
        .await
        .map_err(|e| {
            warn!("[STREAMING] request_stream failed: {e}");
            e.to_string()
        })?;
    info!("[STREAMING] Stream requested, session_id={}", session_id);
    Ok(session_id)
}

/// Background task that polls for incoming stream requests and auto-fulfills them.
/// Spawned once on app startup. Runs every 10 seconds.
pub fn spawn_stream_request_poller() {
    tokio::spawn(async {
        info!("[STREAM-POLLER] Background stream request poller started");
        // Track session IDs we've already started processing to avoid duplicate spawns
        let mut processing: std::collections::HashSet<String> = std::collections::HashSet::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;

            // Check if we have auth configured (skip if not logged in)
            {
                let db = borrow_db_checked();
                if db.auth.is_none() {
                    continue;
                }
            }

            // Prune old entries from processing set (keep it from growing forever)
            // Active host sessions map tells us which are still alive
            {
                let active = ACTIVE_HOST_SESSIONS.lock().await;
                processing.retain(|sid| active.contains_key(sid));
            }

            match streaming_sessions::poll_pending_requests().await {
                Ok(requests) => {
                    if !requests.is_empty() {
                        info!("[STREAM-POLLER] Found {} pending stream request(s)", requests.len());
                    }
                    for req in requests {
                        // Skip if we're already processing this session
                        if processing.contains(&req.session_id) {
                            continue;
                        }

                        if let Some(game_id) = &req.game_id {
                            // Check if this game is installed locally
                            let is_installed = {
                                let db = borrow_db_checked();
                                matches!(
                                    db.applications.game_statuses.get(game_id),
                                    Some(GameDownloadStatus::Installed { .. })
                                )
                            };

                            // Mark as processing BEFORE spawning to prevent duplicates
                            processing.insert(req.session_id.clone());

                            if is_installed {
                                info!(
                                    "[STREAM-POLLER] Game {} is installed, accepting request {}",
                                    game_id, req.session_id
                                );
                                let game_name = req.game
                                    .as_ref()
                                    .map(|g| g.m_name.clone())
                                    .unwrap_or_else(|| game_id.clone());
                                tokio::spawn(fulfill_stream_request(
                                    req.session_id.clone(),
                                    game_id.clone(),
                                    game_name,
                                ));
                            } else {
                                // Game not installed — this might be a remote install request.
                                // Accept the request (to clear it from pending) and emit an
                                // event so the frontend can trigger the download.
                                info!(
                                    "[STREAM-POLLER] Game {} is NOT installed locally — treating as remote install request {}",
                                    game_id, req.session_id
                                );
                                let sid = req.session_id.clone();
                                let gid = game_id.clone();
                                let gname = req.game
                                    .as_ref()
                                    .map(|g| g.m_name.clone())
                                    .unwrap_or_else(|| gid.clone());
                                tokio::spawn(async move {
                                    // Accept the request so it doesn't keep showing up
                                    if let Err(e) = streaming_sessions::accept_stream_request(&sid, None, None).await {
                                        warn!("[STREAM-POLLER] Failed to accept remote install request: {e}");
                                        return;
                                    }
                                    // Emit event for frontend to handle the download
                                    {
                                        use remote::utils::DROP_APP_HANDLE;
                                        use tauri::Emitter;
                                        let lock = DROP_APP_HANDLE.lock().await;
                                        if let Some(app) = &*lock {
                                            let _ = app.emit("remote-install-request", serde_json::json!({
                                                "gameId": gid,
                                                "gameName": gname,
                                                "sessionId": sid,
                                            }));
                                            info!("[STREAM-POLLER] Emitted remote-install-request for game {}", gid);
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    // Silently ignore poll errors (network issues, not logged in, etc.)
                    let _ = e;
                }
            }
        }
    });
}

/// Fulfill a stream request: accept it, start Sunshine, register the game, launch it.
async fn fulfill_stream_request(session_id: String, game_id: String, game_name: String) {
    info!("[STREAM-FULFILL] Fulfilling stream request {} for game {}", session_id, game_id);

    // 1. Read Sunshine credentials from settings
    let (sun_user, sun_pass) = {
        let db = borrow_db_checked();
        let user = if db.settings.sunshine_username.is_empty() {
            "sunshine".to_string()
        } else {
            db.settings.sunshine_username.clone()
        };
        let pass = if db.settings.sunshine_password.is_empty() {
            "sunshine".to_string()
        } else {
            db.settings.sunshine_password.clone()
        };
        (user, pass)
    };

    // 2. Generate a pairing PIN
    let pin = format!("{:04}", rand::rng().random_range(0u16..10000));

    // 2b. Detect local IP (open a UDP socket to a public IP, check local addr)
    let local_ip = std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|sock| {
            sock.connect("8.8.8.8:80")?;
            sock.local_addr()
        })
        .ok()
        .map(|addr| addr.ip().to_string());
    info!("[STREAM-FULFILL] Detected local IP: {:?}", local_ip);

    // 3. Accept the request on the server
    if let Err(e) = streaming_sessions::accept_stream_request(&session_id, Some(&pin), local_ip.as_deref()).await {
        warn!("[STREAM-FULFILL] Failed to accept request: {e}");
        return;
    }

    // 4. Make sure Sunshine is running
    {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        let needs_start = match *guard {
            Some(ref mut child) => child.try_wait().map_or(true, |s| s.is_some()),
            None => true,
        };
        if needs_start {
            *guard = None;
            drop(guard);

            let config_dir = sunshine_config_dir();
            match generate_sunshine_conf(&config_dir, &sun_user, &sun_pass) {
                Ok(conf_path) => {
                    let binary = match find_sunshine() {
                        Some(b) => b,
                        None => {
                            warn!("[STREAM-FULFILL] Sunshine not installed, cannot fulfill request");
                            return;
                        }
                    };
                    info!("[STREAM-FULFILL] Starting Sunshine: {} {}", binary.display(), conf_path.display());
                    match Command::new(&binary)
                        .arg(conf_path.to_string_lossy().as_ref())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                    {
                        Ok(child) => {
                            info!("[STREAM-FULFILL] Sunshine started with PID {}", child.id());
                            let mut guard = SUNSHINE_PROCESS.lock().await;
                            *guard = Some(child);
                        }
                        Err(e) => {
                            warn!("[STREAM-FULFILL] Failed to start Sunshine: {e}");
                            return;
                        }
                    }
                    // Give Sunshine time to initialize
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                Err(e) => {
                    warn!("[STREAM-FULFILL] Failed to generate Sunshine config: {e}");
                    return;
                }
            }
        }
    }

    // 5. Register the game in Sunshine's apps
    if let Err(e) = register_game_app(&game_id, &game_name, Some("drop-launch"), None) {
        warn!("[STREAM-FULFILL] Failed to register game: {e}");
        // Non-fatal
    }

    // 6. Send the PIN to Sunshine's API for pre-pairing
    let pin_body = serde_json::json!({ "pin": pin, "name": "Drop Client" });
    if let Err(e) = sunshine_api_request(
        reqwest::Method::POST,
        "/pin",
        Some(pin_body),
        &sun_user,
        &sun_pass,
    ).await {
        warn!("[STREAM-FULFILL] Failed to send PIN to Sunshine (may not need pairing): {e}");
    }

    // 7. Mark the session as Ready
    if let Err(e) = streaming_sessions::mark_session_ready(&session_id, Some(&pin)).await {
        warn!("[STREAM-FULFILL] Failed to mark session ready: {e}");
        return;
    }
    info!("[STREAM-FULFILL] Session {} marked Ready", session_id);

    // 8. Change resolution for Steam Deck streaming (Windows only)
    #[cfg(target_os = "windows")]
    {
        // Steam Deck native resolution is 1280x800
        match set_display_resolution(1280, 800) {
            Ok((old_w, old_h)) => {
                let mut guard = SAVED_RESOLUTION.lock().await;
                *guard = Some(SavedResolution { width: old_w, height: old_h });
                info!("[STREAM-FULFILL] Saved original resolution {}x{}, switched to 1280x800", old_w, old_h);
            }
            Err(e) => warn!("[STREAM-FULFILL] Failed to set streaming resolution: {e}"),
        }
    }

    // 9. Launch the game (on a blocking thread — launch_game uses block_on internally)
    info!("[STREAM-FULFILL] Launching game {}", game_id);
    {
        use crate::process::launch_game;
        let gid = game_id.clone();
        match tokio::task::spawn_blocking(move || launch_game(gid, 0)).await {
            Ok(Ok(_)) => info!("[STREAM-FULFILL] Game launched successfully"),
            Ok(Err(e)) => warn!("[STREAM-FULFILL] Failed to launch game: {e:?}"),
            Err(e) => warn!("[STREAM-FULFILL] Launch task panicked: {e}"),
        }
    }

    // 10. Register this session and start heartbeating in background (cancellable)
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    {
        let mut sessions = ACTIVE_HOST_SESSIONS.lock().await;
        sessions.insert(session_id.clone(), cancel_tx);
    }

    let sid = session_id.clone();
    let hb_game_id = game_id.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;

            // Check if cancellation was signalled
            if *cancel_rx.borrow() {
                info!("[STREAM-FULFILL] Session {} cancelled, stopping heartbeat", sid);
                let _ = streaming_sessions::stop_streaming_session(&sid).await;
                break;
            }

            // Check if the game process has exited — auto-stop the session
            let game_exited = process::PROCESS_MANAGER.lock().is_game_running(&hb_game_id) == false;
            if game_exited {
                info!("[STREAM-FULFILL] Game {} exited, auto-stopping session {}", hb_game_id, sid);
                let _ = streaming_sessions::stop_streaming_session(&sid).await;
                break;
            }

            if let Err(e) = streaming_sessions::heartbeat_streaming(&sid, Some("Streaming")).await {
                warn!("[STREAM-FULFILL] Heartbeat failed for {}: {e}", sid);
                break;
            }
        }
        // Clean up from the active sessions map
        let mut sessions = ACTIVE_HOST_SESSIONS.lock().await;
        sessions.remove(&sid);

        // Restore display resolution (Windows only)
        #[cfg(target_os = "windows")]
        {
            restore_display_resolution().await;
        }

        // Stop Sunshine if no more active sessions
        if sessions.is_empty() {
            drop(sessions); // Release lock before stopping Sunshine
            info!("[STREAM-FULFILL] No more active sessions, stopping Sunshine");
            let _ = stop_sunshine().await;
        }
    });
}
