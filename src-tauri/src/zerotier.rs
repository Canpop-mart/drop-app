//! ZeroTier virtual-LAN management for co-op "rooms".
//!
//! Drop bundles the full `zerotier-one` client and runs it ON-DEMAND (per room),
//! controlling it through its local HTTP API on `127.0.0.1:9993`. Joining a room
//! puts this device on a private virtual LAN minted by the self-hosted controller
//! (see drop-server `docs/zerotier-controller.md`); the game's own LAN multiplayer
//! then discovers peers across it.
//!
//! Elevation: on Linux the daemon needs `CAP_NET_ADMIN`/`CAP_NET_RAW` to create
//! its TUN device. Rather than run all of Drop as root, we copy the bundled
//! binary into a writable tools dir and grant it file capabilities ONCE via
//! `pkexec setcap` — after that it runs unprivileged. (Capabilities are dropped
//! when a file is replaced, so we re-grant whenever we re-stage the binary.)

use std::path::PathBuf;
use std::process::Command;

use log::info;
use remote::requests::{generate_url, make_authenticated_get, make_authenticated_post};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

/// ZeroTier's primary port — both the UDP data plane and the local HTTP control API.
const ZT_API_PORT: u16 = 9993;

/// A ZeroTier network id is 64-bit → 16 hex chars.
fn is_valid_network_id(s: &str) -> bool {
    s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit())
}

// ── Paths ─────────────────────────────────────────────────────────────

fn tools_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("drop")
        .join("tools")
}

fn zerotier_dir() -> PathBuf {
    tools_dir().join("zerotier")
}

/// The daemon's home/data dir (identity, authtoken, joined-network state).
fn zerotier_data_dir() -> PathBuf {
    zerotier_dir().join("data")
}

/// Staged shared libraries the bundled binary needs (Linux only).
#[cfg(target_os = "linux")]
fn zerotier_libs_dir() -> PathBuf {
    zerotier_dir().join("libs")
}

/// The managed copy of the zerotier-one binary that Drop runs.
fn zerotier_binary() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        zerotier_dir().join("zerotier-one.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        zerotier_dir().join("zerotier-one")
    }
}

// ── Process handle ────────────────────────────────────────────────────

/// Global handle to the running zerotier-one daemon (Drop-managed child).
static ZT_DAEMON: std::sync::LazyLock<Mutex<Option<std::process::Child>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

// ── Status ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ZerotierStatus {
    /// The managed binary is staged and runnable.
    pub installed: bool,
    /// The daemon is currently running (as our child).
    pub running: bool,
    /// On Linux: the binary has the capabilities it needs to make a TUN device.
    pub caps_ready: bool,
    /// This node's 10-hex ZeroTier id (once the daemon has come up), for the
    /// server to authorize onto a room's network.
    pub node_id: Option<String>,
}

// ── Binary staging (install) ──────────────────────────────────────────

/// Locate the bundled zerotier-one shipped inside the AppImage, returning the
/// binary path and the directory holding its `.so` deps.
#[cfg(target_os = "linux")]
fn bundled_source() -> Option<(PathBuf, PathBuf)> {
    // APPDIR is exported by the AppImage runtime. We ship zerotier-one at
    // usr/bin/zerotier-one with libminiupnpc/libnatpmp in usr/lib.
    let appdir = std::env::var("APPDIR").ok()?;
    let bin = PathBuf::from(&appdir).join("usr/bin/zerotier-one");
    let libdir = PathBuf::from(&appdir).join("usr/lib");
    if bin.exists() {
        Some((bin, libdir))
    } else {
        None
    }
}

/// Find a usable zerotier-one: the managed copy first, then a system install.
fn find_zerotier() -> Option<PathBuf> {
    let managed = zerotier_binary();
    if managed.exists() {
        return Some(managed);
    }

    #[cfg(target_os = "linux")]
    {
        for p in ["/usr/sbin/zerotier-one", "/usr/bin/zerotier-one"] {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Official Windows install location.
        let pf = std::env::var("ProgramFiles(x86)")
            .or_else(|_| std::env::var("ProgramFiles"))
            .unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());
        let path = PathBuf::from(pf).join("ZeroTier\\One\\zerotier-one_x64.exe");
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// The official ZeroTier service's state dir on Windows (under ProgramData).
#[cfg(target_os = "windows")]
fn windows_zt_global_dir() -> PathBuf {
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    PathBuf::from(pd).join("ZeroTier").join("One")
}

/// Whether ZeroTier is available to Drop here — bundled (AppImage), a system
/// install, or the official Windows service. Drives the UI's "available" state.
fn is_installed() -> bool {
    if find_zerotier().is_some() {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        if bundled_source().is_some() {
            return true;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if windows_zt_global_dir().exists() {
            return true;
        }
    }
    false
}

/// Stage zerotier-one (and, on Linux, its libs) into the writable tools dir so we
/// can run it — and, on Linux, so we can `setcap` it. Idempotent: re-copies if the
/// bundled binary is newer/missing.
#[cfg(target_os = "linux")]
fn stage_binary() -> Result<PathBuf, String> {
    let target = zerotier_binary();
    let (src_bin, src_libs) = match bundled_source() {
        Some(s) => s,
        None => {
            // Not running from the AppImage (e.g. a system install). If a system
            // zerotier-one exists, use it directly — it's already privileged via
            // its service/setuid and we won't manage its lifecycle here.
            if let Some(system) = find_zerotier() {
                return Ok(system);
            }
            return Err(
                "Bundled zerotier-one not found (APPDIR unset and no system install)."
                    .to_string(),
            );
        }
    };

    std::fs::create_dir_all(zerotier_dir())
        .map_err(|e| format!("Failed to create zerotier dir: {e}"))?;
    std::fs::create_dir_all(zerotier_libs_dir())
        .map_err(|e| format!("Failed to create zerotier libs dir: {e}"))?;

    // Copy the binary if missing or smaller/older than the bundled one.
    let needs_copy = match (std::fs::metadata(&target), std::fs::metadata(&src_bin)) {
        (Ok(t), Ok(s)) => t.len() != s.len(),
        _ => true,
    };
    if needs_copy {
        std::fs::copy(&src_bin, &target)
            .map_err(|e| format!("Failed to copy zerotier-one: {e}"))?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to chmod zerotier-one: {e}"))?;
        info!("[ZEROTIER] Staged binary to {}", target.display());
    }

    // Copy the two extra libs SteamOS lacks (libminiupnpc, libnatpmp).
    if let Ok(entries) = std::fs::read_dir(&src_libs) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("libminiupnpc.so") || name_str.starts_with("libnatpmp.so") {
                let dest = zerotier_libs_dir().join(&name);
                if !dest.exists() {
                    let _ = std::fs::copy(entry.path(), &dest);
                }
            }
        }
    }

    Ok(target)
}

// ── Capabilities (Linux elevation) ────────────────────────────────────

/// Does the managed binary already have the capabilities it needs?
#[cfg(target_os = "linux")]
fn caps_present(binary: &std::path::Path) -> bool {
    // `getcap` prints a non-empty line when the file has capabilities set.
    match Command::new("getcap").arg(binary).output() {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            s.contains("cap_net_admin") && s.contains("cap_net_raw")
        }
        Err(_) => false,
    }
}

/// Grant the managed binary CAP_NET_ADMIN/CAP_NET_RAW via a single `pkexec setcap`
/// prompt, so the daemon can create its TUN device without running Drop as root.
#[cfg(target_os = "linux")]
fn ensure_caps(binary: &std::path::Path) -> Result<(), String> {
    if caps_present(binary) {
        return Ok(());
    }
    info!("[ZEROTIER] Requesting capabilities via pkexec setcap (one-time)");
    let status = Command::new("pkexec")
        .arg("setcap")
        .arg("cap_net_admin,cap_net_raw+ep")
        .arg(binary)
        .status()
        .map_err(|e| format!("Failed to run pkexec setcap: {e}"))?;
    if !status.success() {
        return Err(
            "Granting network capabilities was declined or failed. Co-op rooms need this once \
             to create the virtual-LAN interface."
                .to_string(),
        );
    }
    if !caps_present(binary) {
        return Err("setcap reported success but capabilities are not present.".to_string());
    }
    Ok(())
}

// ── Windows: official-service integration ─────────────────────────────

/// On Windows we drive the officially-installed ZeroTier service. Its auth token
/// lives in a ProgramData dir only admins can read, so copy it once (via a UAC
/// prompt) into Drop's data dir, which `read_auth_token` then reads.
#[cfg(target_os = "windows")]
fn ensure_windows_zerotier() -> Result<(), String> {
    let global_token = windows_zt_global_dir().join("authtoken.secret");
    if !global_token.exists() {
        return Err(
            "ZeroTier isn't installed. Install the official ZeroTier client from zerotier.com, \
             then try again."
                .to_string(),
        );
    }
    let cached = zerotier_data_dir().join("authtoken.secret");
    if cached.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(zerotier_data_dir())
        .map_err(|e| format!("Failed to create zerotier data dir: {e}"))?;
    copy_authtoken_elevated(&global_token, &cached)?;
    if !cached.exists() {
        return Err("Could not read ZeroTier's auth token (the copy did not complete).".to_string());
    }
    Ok(())
}

/// Copy ZeroTier's admin-only auth token into Drop's data dir via one UAC prompt.
/// Mirrors `settings::add_defender_exclusions`' elevation approach.
#[cfg(target_os = "windows")]
fn copy_authtoken_elevated(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let inner = format!(
        "Copy-Item -LiteralPath '{}' -Destination '{}' -Force; \
         icacls '{}' /grant:r \"$($env:USERNAME):R\"",
        src.display().to_string().replace('\'', "''"),
        dst.display().to_string().replace('\'', "''"),
        dst.display().to_string().replace('\'', "''"),
    );
    let encoded = {
        let utf16: Vec<u8> = inner.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        STANDARD.encode(utf16)
    };
    let outer = format!(
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden \
         -ArgumentList '-NoProfile','-EncodedCommand','{encoded}'"
    );
    info!("[ZEROTIER] Requesting one-time elevation to read the ZeroTier auth token");
    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &outer])
        .status()
        .map_err(|e| format!("Failed to start elevated PowerShell: {e}"))?;
    if !status.success() {
        return Err(
            "Elevation was declined. Co-op rooms need to read ZeroTier's auth token once."
                .to_string(),
        );
    }
    Ok(())
}

// ── Local control API ─────────────────────────────────────────────────

/// Read the daemon's local API auth token (written into our data dir on boot).
fn read_auth_token() -> Result<String, String> {
    let path = zerotier_data_dir().join("authtoken.secret");
    std::fs::read_to_string(&path)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("Failed to read zerotier authtoken: {e}"))
}

/// Call the local zerotier-one control API.
async fn zt_api(
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let token = read_auth_token()?;
    let url = format!("http://127.0.0.1:{ZT_API_PORT}{path}");
    let client = reqwest::Client::new();
    let mut req = client.request(method, &url).header("X-ZT1-Auth", token);
    if let Some(body) = body {
        req = req.json(&body);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("ZeroTier API request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("ZeroTier API error: HTTP {}", resp.status()));
    }
    // Some endpoints (join/leave) return the network object; tolerate empty bodies.
    let text = resp.text().await.unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("Failed to parse ZeroTier response: {e}"))
}

/// Fetch this node's 10-hex id from the local API.
async fn fetch_node_id() -> Option<String> {
    let status = zt_api(reqwest::Method::GET, "/status", None).await.ok()?;
    status
        .get("address")
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
}

/// Block until the local API answers (the daemon has finished coming up), or time out.
async fn wait_for_api_ready() -> bool {
    for _ in 0..20 {
        if zt_api(reqwest::Method::GET, "/status", None).await.is_ok() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }
    false
}

// ── Daemon lifecycle ──────────────────────────────────────────────────

/// True if our managed daemon child is still alive (and reaped if not).
async fn daemon_alive() -> bool {
    let mut guard = ZT_DAEMON.lock().await;
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(None) => true,
            _ => {
                *guard = None;
                false
            }
        }
    } else {
        false
    }
}

/// Ensure ZeroTier is ready — Windows: the official service is installed and its
/// auth token is readable. Safe to call repeatedly.
#[cfg(target_os = "windows")]
async fn ensure_daemon() -> Result<(), String> {
    ensure_windows_zerotier()?;
    if !wait_for_api_ready().await {
        return Err(
            "The ZeroTier service isn't responding. Make sure the 'ZeroTier One' service is running."
                .to_string(),
        );
    }
    Ok(())
}

/// Ensure ZeroTier is ready — Linux: stage, capability-grant, and spawn our own
/// bundled daemon. Safe to call repeatedly.
#[cfg(target_os = "linux")]
async fn ensure_daemon() -> Result<(), String> {
    if daemon_alive().await {
        return Ok(());
    }

    let binary = stage_binary()?;
    ensure_caps(&binary)?;

    std::fs::create_dir_all(zerotier_data_dir())
        .map_err(|e| format!("Failed to create zerotier data dir: {e}"))?;

    let mut cmd = Command::new(&binary);
    // -U skips zerotier-one's "must be run as root" uid check so it runs as the
    // user, relying on the CAP_NET_ADMIN/CAP_NET_RAW we granted via setcap (caps
    // alone don't satisfy the uid check). The absolute RPATH baked into the
    // binary lets it find the staged libs even under secure-execution mode.
    cmd.arg("-U")
        .arg(format!("-p{ZT_API_PORT}"))
        .arg(zerotier_data_dir());
    // Belt-and-suspenders for non-capability runs (ignored under secure-exec).
    cmd.env("LD_LIBRARY_PATH", zerotier_libs_dir());

    let child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start zerotier-one: {e}"))?;
    info!("[ZEROTIER] Daemon started (PID {})", child.id());

    {
        let mut guard = ZT_DAEMON.lock().await;
        *guard = Some(child);
    }

    if !wait_for_api_ready().await {
        return Err("zerotier-one started but its control API never became ready.".to_string());
    }
    Ok(())
}

/// Co-op rooms aren't supported on this platform yet.
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
async fn ensure_daemon() -> Result<(), String> {
    Err("Co-op rooms aren't supported on this platform yet.".to_string())
}

// ── Tauri commands ────────────────────────────────────────────────────

/// Report the current ZeroTier state for the UI.
#[tauri::command]
pub async fn zerotier_status() -> ZerotierStatus {
    let installed = is_installed();

    #[cfg(target_os = "linux")]
    let caps_ready = find_zerotier().as_deref().map(caps_present).unwrap_or(false);
    #[cfg(not(target_os = "linux"))]
    let caps_ready = true;

    let running = daemon_alive().await;
    let node_id = if running { fetch_node_id().await } else { None };

    ZerotierStatus {
        installed,
        running,
        caps_ready,
        node_id,
    }
}

/// Bring the daemon up (staging + one-time elevation as needed) and return this
/// node's id — the value the server needs to authorize us onto a room's network.
#[tauri::command]
pub async fn zerotier_prepare() -> Result<String, String> {
    ensure_daemon().await?;
    fetch_node_id()
        .await
        .ok_or_else(|| "Could not read this node's ZeroTier id.".to_string())
}

/// Join a room's virtual-LAN network (after the server has authorized this node).
#[tauri::command]
pub async fn zerotier_join(network_id: String) -> Result<(), String> {
    if !is_valid_network_id(&network_id) {
        return Err("Invalid network id.".to_string());
    }
    ensure_daemon().await?;
    zt_api(
        reqwest::Method::POST,
        &format!("/network/{}", network_id.to_lowercase()),
        Some(serde_json::json!({})),
    )
    .await?;
    info!("[ZEROTIER] Joined network {network_id}");
    Ok(())
}

/// Leave a room's network.
#[tauri::command]
pub async fn zerotier_leave(network_id: String) -> Result<(), String> {
    if !daemon_alive().await {
        return Ok(());
    }
    let _ = zt_api(
        reqwest::Method::DELETE,
        &format!("/network/{}", network_id.to_lowercase()),
        None,
    )
    .await;
    info!("[ZEROTIER] Left network {network_id}");
    Ok(())
}

/// Stop the daemon entirely (e.g. when the last room session ends).
#[tauri::command]
pub async fn zerotier_stop() -> Result<(), String> {
    let mut guard = ZT_DAEMON.lock().await;
    if let Some(mut child) = guard.take() {
        info!("[ZEROTIER] Stopping daemon (PID {})", child.id());
        #[cfg(unix)]
        {
            unsafe {
                libc::kill(child.id() as i32, libc::SIGTERM);
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
            if child.try_wait().map_or(true, |s| s.is_none()) {
                let _ = child.kill();
            }
        }
        #[cfg(not(unix))]
        {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
    Ok(())
}

// ── Co-op rooms (orchestration: daemon + drop-server controller) ──────
//
// These commands tie the local daemon to the server's room API. The room
// endpoints are JWT/cert-authed (`defineClientEventHandler`), so they go through
// the authenticated `remote` client rather than the `server://` web-token path.

/// A room as returned by drop-server. `short_code` is present when hosting.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RoomInfo {
    pub room_id: String,
    #[serde(default)]
    pub short_code: Option<String>,
    pub network_id: String,
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// Host a new co-op room: bring the daemon up, ask the server to mint a network
/// and authorize this node, then join it. Returns the shareable short code.
#[tauri::command]
pub async fn room_host(
    game_id: Option<String>,
    name: Option<String>,
) -> Result<RoomInfo, String> {
    let node_id = zerotier_prepare().await?;
    let url = generate_url(&["/api/v1/client/room"], &[]).map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "zerotierNodeId": node_id, "gameId": game_id, "name": name });
    let resp = make_authenticated_post(url, &body)
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server rejected hosting a room: HTTP {}", resp.status()));
    }
    let info: RoomInfo = resp.json().await.map_err(|e| e.to_string())?;
    zerotier_join(info.network_id.clone()).await?;
    Ok(info)
}

/// Join an existing room by its short code: bring the daemon up, ask the server
/// to authorize this node onto the room's network, then join it.
#[tauri::command]
pub async fn room_join(short_code: String) -> Result<RoomInfo, String> {
    let node_id = zerotier_prepare().await?;
    let url = generate_url(&["/api/v1/client/room/join"], &[]).map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "shortCode": short_code, "zerotierNodeId": node_id });
    let resp = make_authenticated_post(url, &body)
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Could not join that room (code may be wrong): HTTP {}", resp.status()));
    }
    let info: RoomInfo = resp.json().await.map_err(|e| e.to_string())?;
    zerotier_join(info.network_id.clone()).await?;
    Ok(info)
}

/// Leave a room: tell the server (best-effort), drop off the overlay, stop the daemon.
#[tauri::command]
pub async fn room_leave(room_id: String, network_id: String) -> Result<(), String> {
    let path = format!("/api/v1/client/room/{room_id}/leave");
    if let Ok(url) = generate_url(&[path.as_str()], &[]) {
        let _ = make_authenticated_post(url, &serde_json::json!({})).await;
    }
    let _ = zerotier_leave(network_id).await;
    zerotier_stop().await
}

/// Fetch a room's current members/status from the server.
#[tauri::command]
pub async fn room_members(room_id: String) -> Result<Value, String> {
    let path = format!("/api/v1/client/room/{room_id}");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        // A 404 means the room is gone (host ended it / it expired) — surface a
        // stable marker so the UI can treat it as "session ended", not an error.
        if resp.status().as_u16() == 404 {
            return Err("room_not_found".to_string());
        }
        return Err(format!("Could not fetch room: HTTP {}", resp.status()));
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

/// Fetch the list of currently-joinable rooms from the server, so the user can
/// join one without the host sharing a code first.
#[tauri::command]
pub async fn room_browse() -> Result<Value, String> {
    let url = generate_url(&["/api/v1/client/room/browse"], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Could not list rooms: HTTP {}", resp.status()));
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}
