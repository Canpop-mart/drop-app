//! ZeroTier virtual-LAN management for co-op "rooms" and Archipelago sessions.
//!
//! On Linux Drop runs its own copy of `zerotier-one` ON-DEMAND, controlling it
//! through its local HTTP API on `127.0.0.1:9993` with a private data dir. If a
//! system ZeroTier service already owns that port, Drop uses it instead when it
//! can read that service's auth token, and refuses with a clear message when it
//! can't (it never starts a second daemon on the same port). On Windows Drop
//! drives the official ZeroTier service and never starts or stops it.
//!
//! Joining a room puts this device on a private virtual LAN minted by the
//! self-hosted controller (see drop-server `docs/zerotier-controller.md`); the
//! game's own LAN multiplayer then discovers peers across it.
//!
//! Elevation: on Linux the daemon needs `CAP_NET_ADMIN`/`CAP_NET_RAW` to create
//! its TUN device. Rather than run all of Drop as root, we copy the bundled
//! binary into a writable tools dir and grant it file capabilities ONCE via
//! `pkexec setcap`; after that it runs unprivileged. Capabilities are dropped
//! when a file is replaced, so we re-grant whenever we re-stage the binary.
//! Inside gamescope (Game Mode) there is usually no polkit agent to answer the
//! prompt, so there we stop and ask the user to do that one step in Desktop Mode.
//!
//! Co-op and Archipelago share the daemon. Each leaves only its own network,
//! and Drop's own Linux daemon is stopped only once no network is joined at all.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use log::{info, warn};
use remote::requests::{generate_url, make_authenticated_get, make_authenticated_post};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

/// ZeroTier's primary port: both the UDP data plane and the local HTTP control API.
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

/// Where Drop's own daemon writes stdout/stderr (truncated on each start).
#[cfg(target_os = "linux")]
fn zerotier_log_path() -> PathBuf {
    zerotier_dir().join("zerotier-one.log")
}

/// Records which bundled binary (and which libs path) the staged copy was built
/// from, so an unchanged AppImage never triggers a re-copy (and a new
/// capability prompt).
#[cfg(target_os = "linux")]
fn staged_marker_path() -> PathBuf {
    zerotier_dir().join("zerotier-one.source")
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

// ── Process handle / which daemon we talk to ──────────────────────────

/// Global handle to the running zerotier-one daemon (Drop-managed child).
static ZT_DAEMON: std::sync::LazyLock<Mutex<Option<std::process::Child>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Serializes everything that starts or stops the daemon or changes which
/// networks are joined (ensure + join POST, leave, sweeps, idle stop), so a stop
/// can't land between a join's daemon start and its join request, and a sweep
/// can't compute its keep list and then delete a network a Host just joined.
/// Waiting for a join to finish configuring happens outside it.
static ZT_OP: std::sync::LazyLock<Mutex<()>> = std::sync::LazyLock::new(|| Mutex::new(()));

/// The co-op room network this app session is in (set once the join worked,
/// cleared on leave), so an Archipelago sweep keeps it.
static ACTIVE_COOP_NETWORK: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn active_coop_network() -> Option<String> {
    ACTIVE_COOP_NETWORK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn set_active_coop_network(id: Option<String>) {
    *ACTIVE_COOP_NETWORK.lock().unwrap_or_else(|e| e.into_inner()) = id;
}

/// When set, the local API is a SYSTEM ZeroTier service and this is the token
/// file that opens it. Drop never stops a daemon it didn't start. None means
/// Drop's own daemon (Linux) or the cached copy of the service token (Windows).
static TOKEN_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

fn token_override() -> Option<PathBuf> {
    TOKEN_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

#[cfg(target_os = "linux")]
fn set_token_override(path: Option<PathBuf>) {
    *TOKEN_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = path;
}

// ── Status ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ZerotierStatus {
    /// ZeroTier is available here in some form (bundled, staged, system, or the
    /// Windows service).
    pub installed: bool,
    /// The local control API answers.
    pub running: bool,
    /// On Linux: Drop's copy has the capabilities it needs to make a TUN device.
    pub caps_ready: bool,
    /// This node's 10-hex ZeroTier id (once the daemon has come up), for the
    /// server to authorize onto a room's network.
    pub node_id: Option<String>,
    /// "windows", "linux" or "other", so the UI can word its hints per platform.
    pub platform: &'static str,
    /// On Linux: this build ships its own zerotier-one (the AppImage), or an
    /// earlier AppImage run already staged one.
    pub bundled: bool,
    /// The one-time capability setup still has to happen and we're in Game Mode,
    /// where it can't. The UI tells the user to do it from Desktop Mode.
    pub needs_desktop_setup: bool,
    /// Getting ZeroTier ready now could show a UAC or password prompt. False
    /// when it's already running, or on Linux when Drop's own copy has its
    /// capabilities and staging won't replace it (see `start_may_prompt`).
    pub start_needs_prompt: bool,
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

/// Find a zerotier-one on disk: the managed copy first, then a system install.
/// Only used to report availability; Drop never runs or setcaps a system binary.
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

/// Whether ZeroTier is available to Drop here: bundled (AppImage), a system
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

// ── RPATH placeholder rewrite (Linux) ─────────────────────────────────
//
// zerotier-one needs CAP_NET_ADMIN, which puts the dynamic loader in
// secure-execution mode where LD_LIBRARY_PATH is ignored. The release workflow
// therefore bakes a fixed-length placeholder RPATH into the bundled binary, and
// staging rewrites it in place to this user's real libs dir. The rewrite never
// changes the file's size or layout: the real path is written over the start
// of the placeholder and the rest is NUL-padded.

/// Must match `ZT_RPATH_PLACEHOLDER` in `.github/workflows/release.yml`.
#[cfg(any(target_os = "linux", test))]
const RPATH_PLACEHOLDER_PREFIX: &str = "/drop-zt-libs-placeholder/";
#[cfg(any(target_os = "linux", test))]
const RPATH_PLACEHOLDER_LEN: usize = 200;

#[cfg(any(target_os = "linux", test))]
fn rpath_placeholder() -> Vec<u8> {
    let mut p = RPATH_PLACEHOLDER_PREFIX.as_bytes().to_vec();
    p.resize(RPATH_PLACEHOLDER_LEN, b'x');
    p
}

/// Byte-substring search (the binaries are a few MB; a linear scan is fine).
#[cfg(any(target_os = "linux", test))]
fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            out.push(i);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Replace every placeholder RPATH in `bytes` with `real_path`, NUL-padded.
/// Returns how many were rewritten (0 = the binary carries no placeholder).
/// Refuses a path that doesn't fit, or that contains a NUL or a `:` (which
/// the loader would read as a second search dir).
#[cfg(any(target_os = "linux", test))]
fn rewrite_rpath_placeholder(bytes: &mut [u8], real_path: &str) -> Result<usize, String> {
    let placeholder = rpath_placeholder();
    let real = real_path.as_bytes();
    if real.len() > placeholder.len() {
        return Err(format!(
            "Drop's data folder path is too long for the bundled ZeroTier ({} characters, the limit is {}): {real_path}",
            real.len(),
            placeholder.len()
        ));
    }
    if real.is_empty() || real.contains(&0) || real.contains(&b':') {
        return Err(format!("Drop's data folder path can't be used by ZeroTier: {real_path}"));
    }
    let hits = find_all(bytes, &placeholder);
    for &at in &hits {
        let slot = &mut bytes[at..at + placeholder.len()];
        slot.fill(0);
        slot[..real.len()].copy_from_slice(real);
    }
    Ok(hits.len())
}

/// Does `bytes` contain `path` as a complete NUL-terminated string? Used to
/// recognise a binary staged by an older release whose baked RPATH already
/// points at this user's libs dir (the old fixed `/home/deck/...` path, on the
/// `deck` user).
#[cfg(any(target_os = "linux", test))]
fn contains_c_string(bytes: &[u8], path: &str) -> bool {
    let mut needle = path.as_bytes().to_vec();
    needle.push(0);
    find_all(bytes, &needle)
        .into_iter()
        .any(|at| at == 0 || bytes[at - 1] == 0)
}

/// What `stage_binary` records once it has staged this exact bundled binary
/// for this libs dir; a different recorded value means it will copy again.
#[cfg(target_os = "linux")]
fn staging_marker(src_bytes: &[u8], libs_str: &str) -> String {
    format!("{:x}\n{libs_str}", md5::compute(src_bytes))
}

/// Would starting Drop's own daemon now ask for a password? Not when the
/// staged copy has its capabilities and `stage_binary` won't replace it
/// (replacing drops them): nothing different is bundled, or this isn't the
/// AppImage, or it's Game Mode, where a capable copy is kept. A copy staged
/// before markers existed counts as a prompt, which only costs a Reconnect
/// press. Blocking: reads the bundled binary.
#[cfg(target_os = "linux")]
fn start_may_prompt(game_mode: bool) -> bool {
    let target = zerotier_binary();
    if !(target.exists() && caps_present(&target)) {
        return true;
    }
    if game_mode {
        return false;
    }
    let Some((src_bin, _)) = bundled_source() else {
        return false;
    };
    let Ok(src_bytes) = std::fs::read(&src_bin) else {
        return true;
    };
    let libs_str = zerotier_libs_dir().to_string_lossy().to_string();
    match std::fs::read_to_string(staged_marker_path()) {
        Ok(recorded) => recorded.trim() != staging_marker(&src_bytes, &libs_str),
        Err(_) => true,
    }
}

/// Stage zerotier-one (and its libs) into the writable tools dir so we can
/// `setcap` and run it. Only ever stages the binary bundled in the AppImage;
/// a system zerotier-one is never copied or given capabilities. Idempotent:
/// re-copies only when the bundled binary or the libs path changed.
#[cfg(target_os = "linux")]
fn stage_binary() -> Result<PathBuf, String> {
    let target = zerotier_binary();
    let Some((src_bin, src_libs)) = bundled_source() else {
        // Not running from the AppImage. A copy staged by an earlier AppImage
        // run is still fine to use.
        if target.exists() {
            return Ok(target);
        }
        return Err(
            "This copy of Drop doesn't include ZeroTier. Use the Drop AppImage, or install \
             ZeroTier and start its service."
                .to_string(),
        );
    };

    std::fs::create_dir_all(zerotier_libs_dir())
        .map_err(|e| format!("Failed to create zerotier libs dir: {e}"))?;

    let libs_dir = zerotier_libs_dir();
    let libs_str = libs_dir.to_string_lossy().to_string();
    let src_bytes =
        std::fs::read(&src_bin).map_err(|e| format!("Failed to read bundled zerotier-one: {e}"))?;
    let marker = staging_marker(&src_bytes, &libs_str);
    let recorded = std::fs::read_to_string(staged_marker_path()).ok();

    let needs_copy = if !target.exists() {
        true
    } else if let Some(recorded) = recorded {
        recorded.trim() != marker
    } else {
        // Staged by a release that predates the marker. If it already has its
        // capabilities and its baked RPATH is this user's libs dir, keep it
        // rather than asking for the password again.
        let adopt = caps_present(&target)
            && std::fs::read(&target)
                .map(|b| contains_c_string(&b, &libs_str))
                .unwrap_or(false);
        if adopt {
            info!("[ZEROTIER] Keeping zerotier-one staged by an earlier release");
            if let Err(e) = std::fs::write(staged_marker_path(), &marker) {
                warn!("[ZEROTIER] Could not write staging marker: {e}");
            }
        }
        !adopt
    };

    // Replacing the binary drops its capabilities, and in Game Mode they can't
    // be granted again. So there, keep a working capable copy and update it the
    // next time Drop runs in Desktop Mode.
    let needs_copy = if needs_copy && target.exists() && caps_present(&target) && in_game_mode() {
        info!("[ZEROTIER] A newer zerotier-one is bundled; keeping the current one until Drop runs in Desktop Mode");
        false
    } else {
        needs_copy
    };

    if needs_copy {
        let mut bytes = src_bytes;
        match rewrite_rpath_placeholder(&mut bytes, &libs_str)? {
            0 => warn!(
                "[ZEROTIER] Bundled zerotier-one has no RPATH placeholder; it finds its libs only if its baked RPATH is {libs_str}"
            ),
            n => info!("[ZEROTIER] Rewrote {n} RPATH placeholder(s) to {libs_str}"),
        }
        // Write beside the target then rename, so a half-written binary is
        // never left where getcap/setcap or the spawn would pick it up.
        let tmp = zerotier_dir().join("zerotier-one.staging");
        std::fs::write(&tmp, &bytes).map_err(|e| format!("Failed to stage zerotier-one: {e}"))?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to chmod zerotier-one: {e}"))?;
        std::fs::rename(&tmp, &target).map_err(|e| format!("Failed to stage zerotier-one: {e}"))?;
        if let Err(e) = std::fs::write(staged_marker_path(), &marker) {
            warn!("[ZEROTIER] Could not write staging marker: {e}");
        }
        info!("[ZEROTIER] Staged binary to {}", target.display());
    }

    // Copy the two extra libs SteamOS lacks (libminiupnpc, libnatpmp).
    match std::fs::read_dir(&src_libs) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("libminiupnpc.so") || name_str.starts_with("libnatpmp.so") {
                    let dest = libs_dir.join(&name);
                    if !dest.exists()
                        && let Err(e) = std::fs::copy(entry.path(), &dest)
                    {
                        warn!("[ZEROTIER] Could not stage {}: {e}", dest.display());
                    }
                }
            }
        }
        Err(e) => warn!("[ZEROTIER] Could not read bundled libs at {}: {e}", src_libs.display()),
    }

    Ok(target)
}

// ── Capabilities (Linux elevation) ────────────────────────────────────

/// Does the managed binary already have the capabilities it needs?
#[cfg(target_os = "linux")]
fn caps_present(binary: &Path) -> bool {
    // `getcap` prints a non-empty line when the file has capabilities set.
    match Command::new("getcap").arg(binary).output() {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            s.contains("cap_net_admin") && s.contains("cap_net_raw")
        }
        Err(_) => false,
    }
}

/// Is this a gamescope *session* (SteamOS Game Mode), where no polkit agent is
/// running to answer pkexec? Deliberately narrower than
/// `SessionType::detect()`, which also counts `GAMESCOPE_WAYLAND_DISPLAY` and
/// `SteamGamepadUI`: those are set for Drop launched from Steam Big Picture or a
/// nested gamescope inside a normal desktop, where the desktop's polkit agent
/// still works. The gamescope session itself sets `XDG_CURRENT_DESKTOP=gamescope`.
#[cfg(any(target_os = "linux", test))]
fn is_gamescope_session(xdg_current_desktop: Option<&str>) -> bool {
    xdg_current_desktop.is_some_and(|d| {
        d.split(':')
            .any(|part| part.trim().eq_ignore_ascii_case("gamescope"))
    })
}

/// `ID=steamos` in `/etc/os-release`.
#[cfg(any(target_os = "linux", test))]
fn os_release_is_steamos(os_release: &str) -> bool {
    os_release.lines().any(|l| {
        l.trim()
            .strip_prefix("ID=")
            .is_some_and(|v| v.trim_matches('"').eq_ignore_ascii_case("steamos"))
    })
}

#[cfg(target_os = "linux")]
fn is_steamos() -> bool {
    std::fs::read_to_string("/etc/os-release")
        .map(|t| os_release_is_steamos(&t))
        .unwrap_or(false)
}

/// Is a SteamOS gamescope session (`gamescope-session` /
/// `gamescope-session-plus`) running? It exists only while the Deck is in Game
/// Mode: switching to Desktop Mode ends it and starts Plasma instead.
#[cfg(target_os = "linux")]
fn gamescope_session_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name();
        if !name.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        std::fs::read(e.path().join("cmdline"))
            .map(|c| {
                c.split(|b| *b == 0)
                    .any(|arg| arg.windows(17).any(|w| w == b"gamescope-session"))
            })
            .unwrap_or(false)
    })
}

/// Game Mode, for the purposes of "can a pkexec prompt be answered here".
/// Two signals, either is enough:
/// - `XDG_CURRENT_DESKTOP=gamescope`, which the gamescope session exports;
/// - SteamOS with a gamescope-session process running, which holds whatever
///   environment Steam hands a non-Steam shortcut, since it's read from the
///   system rather than from our env.
///
/// Blocking (reads /proc); call from a blocking context or rarely.
#[cfg(target_os = "linux")]
fn in_game_mode() -> bool {
    is_gamescope_session(std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref())
        || (is_steamos() && gamescope_session_running())
}

/// Log what the Game Mode check sees, so it can be checked on a Deck.
#[cfg(target_os = "linux")]
fn log_game_mode_detection() {
    let xdg = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let xdg_session = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
    let steamos = is_steamos();
    let session = gamescope_session_running();
    let result = is_gamescope_session(Some(&xdg)) || (steamos && session);
    info!(
        "[ZEROTIER] Game Mode check: XDG_CURRENT_DESKTOP={xdg:?} XDG_SESSION_DESKTOP={xdg_session:?} \
         steamos={steamos} gamescope-session running={session} -> game mode={result}"
    );
}

/// Placeholder: shown when the one-time setup is needed inside Game Mode.
#[cfg(target_os = "linux")]
const GAME_MODE_SETUP_MSG: &str = "Co-op needs a one-time setup that can't be done in Game Mode. \
     Switch to Desktop Mode, open Drop, and host or join a room once. After that it works in Game Mode.";

/// Grant the managed binary CAP_NET_ADMIN/CAP_NET_RAW via a single `pkexec setcap`
/// prompt, so the daemon can create its TUN device without running Drop as root.
#[cfg(target_os = "linux")]
fn ensure_caps(binary: &Path) -> Result<(), String> {
    if caps_present(binary) {
        return Ok(());
    }
    if in_game_mode() {
        warn!("[ZEROTIER] Capabilities missing and running in Game Mode; not attempting pkexec");
        return Err(GAME_MODE_SETUP_MSG.to_string());
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
fn copy_authtoken_elevated(src: &Path, dst: &Path) -> Result<(), String> {
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

/// Read the local API auth token: a system service's token when we're using
/// one, otherwise the one in Drop's data dir.
fn read_auth_token() -> Result<String, String> {
    let path = token_override().unwrap_or_else(|| zerotier_data_dir().join("authtoken.secret"));
    std::fs::read_to_string(&path)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("Failed to read zerotier authtoken: {e}"))
}

/// How a single local API call failed.
enum ZtError {
    /// No token file to read.
    NoToken(String),
    /// The daemon answered but refused the token (a different daemon than the
    /// token belongs to).
    Unauthorized,
    Other(String),
}

/// Placeholder: the Windows service refused Drop's saved copy of its token
/// (typically after ZeroTier was reinstalled). Hosting or joining copies it
/// again (one Windows permission prompt).
#[cfg(target_os = "windows")]
const WINDOWS_TOKEN_REFUSED_MSG: &str = "ZeroTier refused Drop's saved access key, probably because \
     ZeroTier was reinstalled. Host or join a room to set it up again (Windows asks for permission once).";

impl std::fmt::Display for ZtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZtError::NoToken(e) | ZtError::Other(e) => f.write_str(e),
            #[cfg(target_os = "windows")]
            ZtError::Unauthorized => f.write_str(WINDOWS_TOKEN_REFUSED_MSG),
            #[cfg(not(target_os = "windows"))]
            ZtError::Unauthorized => f.write_str("ZeroTier refused Drop's access token."),
        }
    }
}

async fn zt_request(
    token: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<&Value>,
) -> Result<Value, ZtError> {
    let url = format!("http://127.0.0.1:{ZT_API_PORT}{path}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| ZtError::Other(format!("ZeroTier API client error: {e}")))?;
    let mut req = client.request(method, &url).header("X-ZT1-Auth", token);
    if let Some(body) = body {
        req = req.json(body);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| ZtError::Other(format!("ZeroTier API request failed: {e}")))?;
    if matches!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        return Err(ZtError::Unauthorized);
    }
    if !resp.status().is_success() {
        return Err(ZtError::Other(format!("ZeroTier API error: HTTP {}", resp.status())));
    }
    // Some endpoints (join/leave) return the network object; tolerate empty bodies.
    let text = resp
        .text()
        .await
        .map_err(|e| ZtError::Other(format!("Failed to read ZeroTier response: {e}")))?;
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text)
        .map_err(|e| ZtError::Other(format!("Failed to parse ZeroTier response: {e}")))
}

/// Linux: work out again which daemon owns the port and which token opens it
/// (Drop's own, or a system service's). Returns true when the token to use
/// changed, i.e. a retry is worth it.
#[cfg(target_os = "linux")]
async fn re_resolve_daemon() -> bool {
    let before = token_override();
    let after = match probe_existing_daemon().await {
        Probe::Ours => None,
        Probe::System(p) => Some(p),
        Probe::Nothing | Probe::ForeignNoToken => return false,
    };
    if before == after {
        return false;
    }
    match &after {
        Some(p) => info!("[ZEROTIER] Using the system ZeroTier service (token {})", p.display()),
        None => info!("[ZEROTIER] Using Drop's own zerotier-one"),
    }
    set_token_override(after);
    true
}

/// Call the local zerotier-one control API. This is the one place that decides
/// which daemon we're talking to: when the current token is missing or refused
/// (Linux), it re-resolves (Drop's own daemon, or a system service with a
/// readable token) and retries once. So a system ZeroTier is used correctly
/// even before anything has called `ensure_daemon` this app session.
async fn zt_api(
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    zt_api_raw(method, path, body).await.map_err(|e| e.to_string())
}

/// `zt_api` keeping the failure kind, for callers that must tell "refused the
/// token" from "nothing is running".
async fn zt_api_raw(
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, ZtError> {
    let first = match read_auth_token() {
        Ok(t) => zt_request(&t, method.clone(), path, body.as_ref()).await,
        Err(e) => Err(ZtError::NoToken(e)),
    };
    let err = match first {
        Ok(v) => return Ok(v),
        Err(e) => e,
    };
    #[cfg(target_os = "linux")]
    if matches!(err, ZtError::Unauthorized | ZtError::NoToken(_)) && re_resolve_daemon().await {
        let t = read_auth_token().map_err(ZtError::NoToken)?;
        return zt_request(&t, method, path, body.as_ref()).await;
    }
    Err(err)
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
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    false
}

/// The joined networks the local daemon reports.
async fn list_networks() -> Result<Vec<Value>, String> {
    let v = zt_api(reqwest::Method::GET, "/network", None).await?;
    Ok(v.as_array().cloned().unwrap_or_default())
}

// ── Network join status ───────────────────────────────────────────────

/// What a network's local status means for a join in progress.
#[derive(Debug, PartialEq)]
enum NetPhase {
    /// Joined. `ip` is the first assigned IPv4 (without the /bits), if any yet.
    Ok { ip: Option<String> },
    /// Still waiting for the controller to send the network config.
    Pending,
    /// The controller refused this node.
    Denied,
    /// The controller says the network doesn't exist.
    NotFound,
    /// Any other ZeroTier status (PORT_ERROR, CLIENT_TOO_OLD, ...).
    Other(String),
}

/// Read a `/network/<id>` object from the local API.
fn classify_network(net: &Value) -> NetPhase {
    let status = net.get("status").and_then(|s| s.as_str()).unwrap_or("");
    match status {
        "OK" => {
            let ip = net
                .get("assignedAddresses")
                .and_then(|a| a.as_array())
                .and_then(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.split('/').next().unwrap_or(s))
                        .find(|s| s.parse::<std::net::Ipv4Addr>().is_ok())
                })
                .map(|s| s.to_string());
            NetPhase::Ok { ip }
        }
        "" | "REQUESTING_CONFIGURATION" => NetPhase::Pending,
        "ACCESS_DENIED" => NetPhase::Denied,
        "NOT_FOUND" => NetPhase::NotFound,
        other => NetPhase::Other(other.to_string()),
    }
}

/// What a join is for, so its errors can say the right thing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum JoinKind {
    /// Hosting a room, or the host rejoining its own room.
    Host,
    /// Joining someone else's room.
    Joiner,
    /// The Archipelago overlay.
    Archipelago,
}

impl JoinKind {
    fn network(self) -> &'static str {
        match self {
            JoinKind::Host | JoinKind::Joiner => "the room's network",
            JoinKind::Archipelago => "the Archipelago network",
        }
    }

    /// Placeholders: why a join failed, worded for who's joining.
    fn denied(self) -> &'static str {
        match self {
            JoinKind::Host => "The room's network refused this device. Try hosting again.",
            JoinKind::Joiner => {
                "The room's network refused this device. Try again, or ask the host to start a new room."
            }
            JoinKind::Archipelago => {
                "The Archipelago network refused this device. Try joining the session again."
            }
        }
    }

    fn not_found(self) -> &'static str {
        match self {
            JoinKind::Host => "The room's network was removed from the server. Try hosting again.",
            JoinKind::Joiner => "The room's network no longer exists. The room may have ended.",
            JoinKind::Archipelago => {
                "The Archipelago network no longer exists on the server. Try joining the session again."
            }
        }
    }
}

/// How many join flows (room host/join, Archipelago create/join) are between
/// readying the daemon and finishing their join. Their network isn't joined
/// yet while they wait on the server, so the idle stop would otherwise see an
/// empty network list and stop the daemon out from under them.
static DAEMON_USERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// Only Linux stops a daemon of its own (and the tests read it).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn daemon_users() -> usize {
    DAEMON_USERS.load(std::sync::atomic::Ordering::SeqCst)
}

/// Counts in `DAEMON_USERS` for as long as it lives.
struct DaemonInUse;

impl DaemonInUse {
    fn new() -> Self {
        DAEMON_USERS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self
    }
}

impl Drop for DaemonInUse {
    fn drop(&mut self) {
        DAEMON_USERS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Networks a join is currently waiting on. Sweeps always keep them, so a
/// sweep running for something else can't pull a network out from under a
/// join that's still configuring.
static JOINS_IN_FLIGHT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn joins_in_flight() -> Vec<String> {
    JOINS_IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Registers a network in `JOINS_IN_FLIGHT` for as long as it lives.
struct InFlightJoin(String);

impl InFlightJoin {
    fn new(nid: &str) -> Self {
        JOINS_IN_FLIGHT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(nid.to_string());
        Self(nid.to_string())
    }
}

impl Drop for InFlightJoin {
    fn drop(&mut self) {
        let mut v = JOINS_IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = v.iter().position(|n| *n == self.0) {
            v.remove(i);
        }
    }
}

/// How long to wait for the controller to hand this node the network config.
const JOIN_CONFIG_TIMEOUT: Duration = Duration::from_secs(45);
/// How long to wait (in total) for an assigned address once the config is in.
/// ZeroTier can take 20-40s to assign an IP on a freshly-minted network.
const JOIN_ADDRESS_TIMEOUT: Duration = Duration::from_secs(60);
/// ACCESS_DENIED / NOT_FOUND must persist this long before we believe them, so
/// a stale answer cached from an earlier membership doesn't fail a fresh join.
const JOIN_REFUSAL_GRACE: Duration = Duration::from_secs(4);

/// Join `network_id` on the local daemon and wait until the network is actually
/// usable. Returns the assigned IPv4 when one arrived in time (a missing address
/// after the wait is logged, not an error: the room still works once ZeroTier
/// assigns it). Leaves the network again on failure so it doesn't linger.
async fn join_and_wait(network_id: &str, kind: JoinKind) -> Result<Option<String>, String> {
    if !is_valid_network_id(network_id) {
        return Err("Invalid network id.".to_string());
    }
    let nid = network_id.to_lowercase();
    let _in_flight = InFlightJoin::new(&nid);
    pre_daemon_setup().await?;
    {
        let _op = ZT_OP.lock().await;
        ensure_daemon().await?;
        zt_api(
            reqwest::Method::POST,
            &format!("/network/{nid}"),
            Some(serde_json::json!({})),
        )
        .await?;
    }
    info!("[ZEROTIER] Join requested for network {nid}");

    let started = std::time::Instant::now();
    let mut refused_since: Option<std::time::Instant> = None;
    let mut last_err = String::new();
    let failure = loop {
        let elapsed = started.elapsed();
        match zt_api(reqwest::Method::GET, &format!("/network/{nid}"), None).await {
            Ok(net) => {
                let phase = classify_network(&net);
                if !matches!(phase, NetPhase::Denied | NetPhase::NotFound) {
                    refused_since = None;
                }
                match &phase {
                    NetPhase::Ok { ip: Some(ip) } => {
                        info!("[ZEROTIER] Joined network {nid} as {ip} after {:.1}s", elapsed.as_secs_f32());
                        return Ok(Some(ip.clone()));
                    }
                    NetPhase::Ok { ip: None } if elapsed >= JOIN_ADDRESS_TIMEOUT => {
                        warn!("[ZEROTIER] Joined network {nid} but no address was assigned within {}s", JOIN_ADDRESS_TIMEOUT.as_secs());
                        return Ok(None);
                    }
                    NetPhase::Ok { ip: None } => {}
                    NetPhase::Pending if elapsed >= JOIN_CONFIG_TIMEOUT => {
                        break format!(
                            "Couldn't get the settings for {} from the server in time. \
                             Check that the Drop server's ZeroTier controller is reachable, then try again.",
                            kind.network()
                        );
                    }
                    NetPhase::Pending => {}
                    NetPhase::Denied | NetPhase::NotFound => {
                        let since = *refused_since.get_or_insert_with(std::time::Instant::now);
                        if since.elapsed() >= JOIN_REFUSAL_GRACE {
                            break if phase == NetPhase::Denied {
                                kind.denied().to_string()
                            } else {
                                kind.not_found().to_string()
                            };
                        }
                    }
                    NetPhase::Other(s) => {
                        break format!("ZeroTier couldn't join {} (status {s}).", kind.network());
                    }
                }
            }
            Err(e) => {
                last_err = e;
                if elapsed >= JOIN_CONFIG_TIMEOUT {
                    break format!(
                        "ZeroTier stopped answering while joining {}: {last_err}",
                        kind.network()
                    );
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    if !last_err.is_empty() {
        warn!("[ZEROTIER] last local API error during join of {nid}: {last_err}");
    }
    warn!("[ZEROTIER] Join of {nid} failed: {failure}");
    if let Err(e) = leave_network(&nid).await {
        warn!("[ZEROTIER] Could not leave {nid} after the failed join: {e}");
    }
    Err(failure)
}

// ── Daemon lifecycle ──────────────────────────────────────────────────

/// Last few lines of Drop's own daemon output, for error messages.
#[cfg(target_os = "linux")]
fn daemon_log_tail(lines: usize) -> String {
    let text = std::fs::read_to_string(zerotier_log_path()).unwrap_or_default();
    let all: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    all[all.len().saturating_sub(lines)..].join(" | ")
}

/// True if our managed daemon child is still alive (and reaped if not).
async fn daemon_alive() -> bool {
    let mut guard = ZT_DAEMON.lock().await;
    let Some(child) = guard.as_mut() else {
        return false;
    };
    match child.try_wait() {
        Ok(None) => true,
        Ok(Some(status)) => {
            #[cfg(target_os = "linux")]
            warn!(
                "[ZEROTIER] Daemon exited ({status}). Last output: {}",
                daemon_log_tail(10)
            );
            #[cfg(not(target_os = "linux"))]
            warn!("[ZEROTIER] Daemon exited ({status})");
            *guard = None;
            false
        }
        Err(e) => {
            warn!("[ZEROTIER] Could not check the daemon: {e}");
            *guard = None;
            false
        }
    }
}

/// Is ZeroTier usable here? On Linux that's a daemon (ours or a system one we
/// have a token for) answering; on Windows Drop drives the OFFICIAL service and
/// owns no child, so "usable" means the local control API answers.
async fn zt_available() -> bool {
    daemon_alive().await || zt_api(reqwest::Method::GET, "/status", None).await.is_ok()
}

/// What is already listening on the ZeroTier port before we start anything.
#[cfg(target_os = "linux")]
enum Probe {
    /// Nothing answers: start Drop's own daemon.
    Nothing,
    /// Drop's own daemon, left running by an earlier session of the app.
    Ours,
    /// A system ZeroTier service, and a token file that opens it.
    System(PathBuf),
    /// Something answers but none of the tokens we can read open it.
    ForeignNoToken,
}

#[cfg(target_os = "linux")]
fn system_token_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = dirs::home_dir() {
        // The per-user copy `zerotier-cli` reads.
        v.push(home.join(".zeroTierOneAuthToken"));
    }
    v.push(PathBuf::from("/var/lib/zerotier-one/authtoken.secret"));
    v
}

#[cfg(target_os = "linux")]
async fn probe_existing_daemon() -> Probe {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return Probe::Nothing;
    };
    let url = format!("http://127.0.0.1:{ZT_API_PORT}/status");
    let opens = |token: String| {
        let req = client.get(&url).header("X-ZT1-Auth", token);
        async move { req.send().await.is_ok_and(|r| r.status().is_success()) }
    };

    // Is anything listening at all?
    if client.get(&url).send().await.is_err() {
        return Probe::Nothing;
    }
    if let Ok(t) = std::fs::read_to_string(zerotier_data_dir().join("authtoken.secret"))
        && opens(t.trim().to_string()).await
    {
        return Probe::Ours;
    }
    for path in system_token_candidates() {
        if let Ok(t) = std::fs::read_to_string(&path)
            && opens(t.trim().to_string()).await
        {
            return Probe::System(path);
        }
    }
    Probe::ForeignNoToken
}

/// Placeholder: a system ZeroTier service is running and Drop can't use it.
#[cfg(target_os = "linux")]
const SYSTEM_ZT_NO_TOKEN_MSG: &str = "A ZeroTier service is already running on this computer and Drop can't \
     read its access token. Either stop that service, or copy its token for your user with: \
     sudo cp /var/lib/zerotier-one/authtoken.secret ~/.zeroTierOneAuthToken && \
     sudo chown $USER ~/.zeroTierOneAuthToken";

/// Run blocking work (process spawns that wait, prompts, big file reads) off
/// the async runtime threads.
async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("ZeroTier setup task failed: {e}"))?
}

/// Serializes `pre_daemon_setup` with itself (never with `ZT_OP`). Two flows
/// starting at once (the automatic room resume and an Archipelago join, say)
/// would otherwise each raise a password or UAC prompt and race on the same
/// staging file and token copy.
static ZT_SETUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Slow and possibly interactive setup (UAC / pkexec prompts, staging the
/// binary) that must happen BEFORE taking `ZT_OP`, so nothing else waits on a
/// password prompt. The second of two concurrent callers waits here and then
/// finds the work already done.
async fn pre_daemon_setup() -> Result<(), String> {
    let _setup = ZT_SETUP.lock().await;
    pre_daemon_setup_unlocked().await
}

#[cfg(target_os = "windows")]
async fn pre_daemon_setup_unlocked() -> Result<(), String> {
    run_blocking(ensure_windows_zerotier).await?;
    if let Err(ZtError::Unauthorized) = zt_api_raw(reqwest::Method::GET, "/status", None).await {
        // The service no longer accepts our copy (e.g. ZeroTier was reinstalled
        // and made a new token): copy it again.
        warn!("[ZEROTIER] The service refused the cached token; copying it again");
        let cached = zerotier_data_dir().join("authtoken.secret");
        if let Err(e) = std::fs::remove_file(&cached)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!("Couldn't replace Drop's copy of ZeroTier's access key: {e}"));
        }
        run_blocking(ensure_windows_zerotier).await?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn pre_daemon_setup_unlocked() -> Result<(), String> {
    if daemon_alive().await {
        return Ok(());
    }
    // Only when nothing answers do we need our own binary; anything else is
    // handled (and reported) by ensure_daemon.
    if !matches!(probe_existing_daemon().await, Probe::Nothing) {
        return Ok(());
    }
    run_blocking(|| {
        let binary = stage_binary()?;
        ensure_caps(&binary)
    })
    .await
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
async fn pre_daemon_setup_unlocked() -> Result<(), String> {
    Ok(())
}

/// Ensure ZeroTier is ready (Windows): the service answers with our token.
/// Caller holds `ZT_OP` and has run `pre_daemon_setup`.
#[cfg(target_os = "windows")]
async fn ensure_daemon() -> Result<(), String> {
    if !wait_for_api_ready().await {
        if let Err(ZtError::Unauthorized) = zt_api_raw(reqwest::Method::GET, "/status", None).await {
            return Err(WINDOWS_TOKEN_REFUSED_MSG.to_string());
        }
        return Err(
            "The ZeroTier service isn't responding. Make sure the 'ZeroTier One' service is running."
                .to_string(),
        );
    }
    Ok(())
}

/// Ensure ZeroTier is ready (Linux): reuse a daemon that's already up (ours, or
/// a system one we can open), otherwise spawn our own staged daemon. Caller
/// holds `ZT_OP` and has run `pre_daemon_setup`, which did the staging and the
/// one-time capability grant outside the lock; this never prompts.
#[cfg(target_os = "linux")]
async fn ensure_daemon() -> Result<(), String> {
    if daemon_alive().await {
        return Ok(());
    }

    match probe_existing_daemon().await {
        Probe::Ours => {
            set_token_override(None);
            info!("[ZEROTIER] Reusing Drop's zerotier-one left running by an earlier session");
            return Ok(());
        }
        Probe::System(token) => {
            info!(
                "[ZEROTIER] Using the system ZeroTier service on port {ZT_API_PORT} (token {})",
                token.display()
            );
            set_token_override(Some(token));
            return Ok(());
        }
        Probe::ForeignNoToken => {
            warn!("[ZEROTIER] Port {ZT_API_PORT} is held by a ZeroTier service whose token Drop can't read");
            return Err(SYSTEM_ZT_NO_TOKEN_MSG.to_string());
        }
        Probe::Nothing => set_token_override(None),
    }

    let binary = zerotier_binary();
    let ready = {
        let b = binary.clone();
        run_blocking(move || Ok(b.exists() && caps_present(&b))).await?
    };
    if !ready {
        // pre_daemon_setup saw a daemon running, and it stopped before we got
        // here. Don't prompt while holding the lock.
        return Err("ZeroTier stopped while it was being set up. Try again.".to_string());
    }

    std::fs::create_dir_all(zerotier_data_dir())
        .map_err(|e| format!("Failed to create zerotier data dir: {e}"))?;

    let log_path = zerotier_log_path();
    let log = std::fs::File::create(&log_path)
        .map_err(|e| format!("Failed to create {}: {e}", log_path.display()))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("Failed to open {}: {e}", log_path.display()))?;

    let mut cmd = Command::new(&binary);
    // -U skips zerotier-one's "must be run as root" uid check so it runs as the
    // user, relying on the CAP_NET_ADMIN/CAP_NET_RAW we granted via setcap (caps
    // alone don't satisfy the uid check). The absolute RPATH rewritten into the
    // binary at staging lets it find the staged libs even under secure-execution.
    cmd.arg("-U")
        .arg(format!("-p{ZT_API_PORT}"))
        .arg(zerotier_data_dir());
    // Belt-and-suspenders for non-capability runs (ignored under secure-exec).
    cmd.env("LD_LIBRARY_PATH", zerotier_libs_dir());

    let child = cmd
        .stdout(std::process::Stdio::from(log))
        .stderr(std::process::Stdio::from(log_err))
        .spawn()
        .map_err(|e| format!("Failed to start zerotier-one: {e}"))?;
    info!(
        "[ZEROTIER] Daemon started (PID {}), output in {}",
        child.id(),
        log_path.display()
    );

    {
        let mut guard = ZT_DAEMON.lock().await;
        *guard = Some(child);
    }

    if !wait_for_api_ready().await {
        let alive = daemon_alive().await;
        let tail = daemon_log_tail(5);
        warn!("[ZEROTIER] Control API never became ready (daemon alive: {alive}). Output: {tail}");
        return Err(if tail.is_empty() {
            "ZeroTier started but never became ready.".to_string()
        } else {
            format!("ZeroTier didn't start: {tail}")
        });
    }
    Ok(())
}

/// Co-op rooms aren't supported on this platform yet.
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
async fn ensure_daemon() -> Result<(), String> {
    Err("Co-op rooms aren't supported on this platform yet.".to_string())
}

/// The pid of Drop's own daemon from an earlier app session, if its pid file
/// names a live process whose command line includes our data dir (so a
/// recycled pid or a system daemon never matches).
#[cfg(target_os = "linux")]
fn own_daemon_pid() -> Option<i32> {
    let data_dir = zerotier_data_dir();
    let pid_text = std::fs::read_to_string(data_dir.join("zerotier-one.pid")).ok()?;
    let pid = pid_text.trim().parse::<i32>().ok()?;
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let data_str = data_dir.to_string_lossy().to_string();
    cmdline
        .split(|b| *b == 0)
        .any(|arg| arg == data_str.as_bytes())
        .then_some(pid)
}

/// Stop Drop's own Linux daemon: the child we hold, or (after an app restart)
/// the one recorded in its pid file (see `own_daemon_pid`).
#[cfg(target_os = "linux")]
async fn stop_managed_daemon() {
    let child = ZT_DAEMON.lock().await.take();
    if let Some(mut child) = child {
        info!("[ZEROTIER] Stopping daemon (PID {})", child.id());
        // Waiting on the child blocks, so it runs off the async threads.
        let result = run_blocking(move || {
            // SAFETY: plain signal to a pid we spawned and still hold (not yet reaped).
            unsafe {
                libc::kill(child.id() as i32, libc::SIGTERM);
            }
            std::thread::sleep(Duration::from_millis(400));
            if child.try_wait().map_or(true, |s| s.is_none())
                && let Err(e) = child.kill()
            {
                warn!("[ZEROTIER] Could not kill the daemon: {e}");
            }
            child
                .wait()
                .map(|_| ())
                .map_err(|e| format!("Could not reap the daemon: {e}"))
        })
        .await;
        if let Err(e) = result {
            warn!("[ZEROTIER] {e}");
        }
        return;
    }

    let Some(pid) = own_daemon_pid() else {
        return;
    };
    info!("[ZEROTIER] Stopping daemon left by an earlier session (PID {pid})");
    // SAFETY: plain signal; own_daemon_pid just checked the pid is our daemon.
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
}

/// Stop Drop's own Linux daemon once no network is joined on it any more.
/// Never stops a system ZeroTier service, and never touches the Windows service.
async fn stop_daemon_if_idle() {
    let _op = ZT_OP.lock().await;
    stop_daemon_if_idle_locked().await;
}

/// `stop_daemon_if_idle` for callers already holding `ZT_OP`.
async fn stop_daemon_if_idle_locked() {
    #[cfg(target_os = "linux")]
    {
        // Resolve which daemon answers first: that's what tells a system
        // service (never ours to stop) from Drop's own.
        if !zt_available().await {
            return;
        }
        if token_override().is_some() {
            return; // a system service: not ours to stop
        }
        if daemon_users() > 0 {
            info!("[ZEROTIER] Keeping the daemon running: a join is in progress");
            return;
        }
        match list_networks().await {
            Ok(nets) if nets.is_empty() => stop_managed_daemon().await,
            Ok(nets) => info!(
                "[ZEROTIER] Keeping the daemon running: {} network(s) still joined",
                nets.len()
            ),
            Err(e) => warn!("[ZEROTIER] Could not list networks ({e}); leaving the daemon running"),
        }
    }
}

// ── Drop-network identification (for the stale-network sweep) ─────────

/// Where we cache the Drop controller's 10-hex node id. It prefixes every
/// network id the controller mints (rooms and the Archipelago overlay), so a
/// sweep can tell a Drop network from the user's hand-joined networks.
fn controller_id_cache_path() -> PathBuf {
    zerotier_dir().join("controller_node_id")
}

/// The Archipelago overlay this device last joined. Sweeps keep it while the
/// server says an Archipelago session is open for this device.
fn ap_network_cache_path() -> PathBuf {
    zerotier_dir().join("ap_network_id")
}

/// Read the cached Drop controller node id, if we've ever joined a room.
fn read_cached_controller_node_id() -> Option<String> {
    let s = std::fs::read_to_string(controller_id_cache_path()).ok()?;
    let s = s.trim().to_lowercase();
    (s.len() == 10 && s.chars().all(|c| c.is_ascii_hexdigit())).then_some(s)
}

fn read_cached_ap_network_id() -> Option<String> {
    let s = std::fs::read_to_string(ap_network_cache_path()).ok()?;
    let s = s.trim().to_lowercase();
    is_valid_network_id(&s).then_some(s)
}

/// Cache the controller node id from a network id we just joined. The first
/// 10 hex of a 16-hex network id is the minting controller's node id. A failed
/// write only weakens later stale-network sweeps, so it's logged, not returned.
fn cache_controller_node_id(network_id: &str) {
    let id = network_id.to_lowercase();
    if id.len() < 10 || !id[..10].chars().all(|c| c.is_ascii_hexdigit()) {
        return;
    }
    let prefix = &id[..10];
    if read_cached_controller_node_id().as_deref() != Some(prefix)
        && let Err(e) = std::fs::create_dir_all(zerotier_dir())
            .and_then(|_| std::fs::write(controller_id_cache_path(), prefix))
    {
        warn!("[ZEROTIER] Could not cache the controller id: {e}");
    }
}

/// The name drop-server gives its single Archipelago overlay network (room
/// networks are named `drop-<CODE>`). ZeroTier reports it locally once the
/// network config has arrived, and keeps it in the saved config across restarts.
const AP_NETWORK_NAME: &str = "drop-archipelago";

/// A network as the local daemon lists it.
struct JoinedNet {
    id: String,
    name: String,
}

/// Which of `joined` to leave: Drop networks (prefixed by `controller`) that
/// aren't in `keep`, and, when `keep_ap`, aren't the Archipelago overlay. The
/// overlay is recognised by name as well as by the cached id, so an Archipelago
/// session joined before the id was cached survives too. Pure, so the sweep's
/// rules are unit-tested.
fn networks_to_sweep(joined: &[JoinedNet], controller: &str, keep: &[String], keep_ap: bool) -> Vec<String> {
    joined
        .iter()
        .filter(|n| !(keep_ap && n.name == AP_NETWORK_NAME))
        .map(|n| n.id.to_lowercase())
        .filter(|id| id.len() == 16 && id.starts_with(controller))
        .filter(|id| !keep.iter().any(|k| k.eq_ignore_ascii_case(id)))
        .collect()
}

/// Leave every Drop network this node has joined except those in `keep` (and
/// the Archipelago overlay when `keep_ap`). The user's hand-joined ZeroTier
/// networks are never touched. Fails CLOSED: with no validated controller id,
/// nothing is left. Caller holds `ZT_OP`.
async fn leave_drop_networks_except_locked(keep: &[String], keep_ap: bool) {
    let Some(controller) = read_cached_controller_node_id() else {
        return;
    };
    // A join still configuring is never swept, whoever it belongs to.
    let mut keep = keep.to_vec();
    keep.extend(joins_in_flight());
    let keep = &keep;
    let joined: Vec<JoinedNet> = match list_networks().await {
        Ok(nets) => nets
            .iter()
            .filter_map(|n| {
                let id = n.get("id").and_then(|v| v.as_str())?.to_string();
                let name = n.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Some(JoinedNet { id, name })
            })
            .collect(),
        Err(e) => {
            warn!("[ZEROTIER] sweep: could not list networks: {e}");
            return;
        }
    };
    for id in networks_to_sweep(&joined, &controller, keep, keep_ap) {
        match zt_api(reqwest::Method::DELETE, &format!("/network/{id}"), None).await {
            Ok(_) => info!("[ZEROTIER] sweep: left stale Drop network {id}"),
            Err(e) => warn!("[ZEROTIER] sweep: failed to leave {id}: {e}"),
        }
    }
}

/// Which Archipelago overlay to keep: (ids to keep, keep a `drop-archipelago`
/// network by name too). Only while the server says this device has an open
/// session; a stale overlay (a fresh Linux daemon rejoins it from its saved
/// state) is otherwise left like any other stale network. If the server can't
/// be asked, only the cached id is kept.
async fn ap_keep() -> (Vec<String>, bool) {
    let cached = read_cached_ap_network_id();
    match ap_has_open_session().await {
        Ok(true) => (cached.into_iter().collect(), true),
        Ok(false) => {
            if cached.is_some() {
                forget_ap_network();
            }
            (Vec::new(), false)
        }
        Err(e) => {
            info!("[ZEROTIER] couldn't check Archipelago sessions ({e}); keeping only its cached network");
            (cached.into_iter().collect(), false)
        }
    }
}

/// After joining a co-op room: leave any other Drop network still joined (an
/// old room left by a crash, or one a fresh Linux daemon rejoined from its
/// saved state), keeping the Archipelago overlay only while a session is open.
async fn sweep_after_coop_join(current: &str) {
    // Server call first: ZT_OP is never held across one.
    let (mut keep, keep_ap) = ap_keep().await;
    keep.push(current.to_lowercase());
    let _op = ZT_OP.lock().await;
    leave_drop_networks_except_locked(&keep, keep_ap).await;
}

/// After joining the Archipelago overlay: leave stale co-op room networks,
/// keeping the room this app session is in, a room the server says this device
/// can still rejoin, and any join in flight. Skipped if the server can't be
/// asked.
async fn sweep_after_ap_join(ap_network: &str) {
    let mut keep = vec![ap_network.to_lowercase()];
    match fetch_my_room().await {
        Ok(room) => keep.extend(
            room.and_then(|r| r.get("networkId").and_then(|v| v.as_str()).map(str::to_lowercase)),
        ),
        Err(e) => {
            info!("[ZEROTIER] Skipping the sweep after joining Archipelago: couldn't ask the server for this device's room ({e})");
            return;
        }
    }
    let _op = ZT_OP.lock().await;
    // Cheap re-check under the lock: a room joined while we asked the server.
    keep.extend(active_coop_network());
    leave_drop_networks_except_locked(&keep, false).await;
}

/// Does the server still list an open Archipelago session for this device?
async fn ap_has_open_session() -> Result<bool, String> {
    let url = generate_url(&["/api/v1/client/archipelago"], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Could not list sessions").await);
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v.as_array().is_some_and(|a| !a.is_empty()))
}

/// Startup cleanup: leave Drop networks orphaned by a previous run so their
/// adapters don't collide with the next join, but keep the ones the server says
/// this device is still part of (a co-op room it can rejoin, an open
/// Archipelago session). If the server can't be asked, nothing is left. Either
/// way Drop's own Linux daemon is stopped if nothing is joined on it.
pub async fn startup_cleanup() {
    #[cfg(target_os = "linux")]
    if let Err(e) = run_blocking(|| {
        log_game_mode_detection();
        Ok(())
    })
    .await
    {
        warn!("[ZEROTIER] {e}");
    }
    if !is_installed() || !zt_available().await {
        return;
    }
    if read_cached_controller_node_id().is_none() {
        // Never joined anything through Drop: nothing to sweep.
        stop_daemon_if_idle().await;
        return;
    }
    // Ask the server everything first: ZT_OP is never held across a server call.
    let mut keep = Vec::new();
    match fetch_my_room().await {
        Ok(Some(room)) => {
            if let Some(nid) = room.get("networkId").and_then(|v| v.as_str()) {
                info!("[ZEROTIER] startup: still in a co-op room on the server; keeping {nid} for rejoin");
                keep.push(nid.to_lowercase());
            }
        }
        Ok(None) => {}
        Err(e) => {
            info!("[ZEROTIER] startup sweep skipped: couldn't ask the server which room this device is in ({e})");
            stop_daemon_if_idle().await;
            return;
        }
    }
    let (ap, keep_ap) = ap_keep().await;
    keep.extend(ap);

    let _op = ZT_OP.lock().await;
    // Cheap re-check under the lock: a room hosted or joined while we were
    // asking the server (joins in flight are kept by the sweep itself).
    keep.extend(active_coop_network());
    leave_drop_networks_except_locked(&keep, keep_ap).await;
    stop_daemon_if_idle_locked().await;
}

// ── Tauri commands ────────────────────────────────────────────────────

/// Report the current ZeroTier state for the UI.
#[tauri::command]
pub async fn zerotier_status() -> ZerotierStatus {
    let installed = is_installed();
    let running = zt_available().await;
    let node_id = if running { fetch_node_id().await } else { None };

    // getcap and the /proc scan block, so they run off the async threads.
    #[cfg(target_os = "linux")]
    let (caps_ready, game_mode, start_needs_prompt) = run_blocking(move || {
        let managed = zerotier_binary();
        let game_mode = in_game_mode();
        Ok((
            managed.exists() && caps_present(&managed),
            game_mode,
            !running && start_may_prompt(game_mode),
        ))
    })
    .await
    .unwrap_or_else(|e| {
        warn!("[ZEROTIER] status check failed: {e}");
        (false, false, true)
    });
    // Windows: a service that isn't answering may need its token copied (UAC).
    #[cfg(not(target_os = "linux"))]
    let start_needs_prompt = !running;
    #[cfg(target_os = "linux")]
    let (bundled, platform) = (bundled_source().is_some() || zerotier_binary().exists(), "linux");
    #[cfg(target_os = "windows")]
    let (caps_ready, bundled, platform) = (true, false, "windows");
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let (caps_ready, bundled, platform) = (true, false, "other");

    #[cfg(target_os = "linux")]
    let needs_desktop_setup = !running && !caps_ready && bundled && game_mode;
    #[cfg(not(target_os = "linux"))]
    let needs_desktop_setup = false;

    ZerotierStatus {
        installed,
        running,
        caps_ready,
        node_id,
        platform,
        bundled,
        needs_desktop_setup,
        start_needs_prompt,
    }
}

/// Bring the daemon up (staging + one-time elevation as needed) and return this
/// node's id: the value the server needs to authorize us onto a room's network.
#[tauri::command]
pub async fn zerotier_prepare() -> Result<String, String> {
    pre_daemon_setup().await?;
    {
        let _op = ZT_OP.lock().await;
        ensure_daemon().await?;
    }
    fetch_node_id()
        .await
        .ok_or_else(|| "Could not read this node's ZeroTier id.".to_string())
}

/// Join a network (after the server has authorized this node) and wait until
/// ZeroTier reports it usable.
#[tauri::command]
pub async fn zerotier_join(network_id: String) -> Result<(), String> {
    join_and_wait(&network_id, JoinKind::Joiner).await.map(|_| ())
}

/// Drop's own daemon's saved config for `network_id` (it rejoins every saved
/// network when it starts).
#[cfg(target_os = "linux")]
fn saved_network_files(network_id: &str) -> [PathBuf; 2] {
    let dir = zerotier_data_dir().join("networks.d");
    [
        dir.join(format!("{network_id}.conf")),
        dir.join(format!("{network_id}.local.conf")),
    ]
}

/// Forget `network_id` from Drop's own (stopped) daemon's saved state, so it
/// isn't rejoined on the next start. Only ever touches Drop's private data dir.
#[cfg(target_os = "linux")]
fn forget_saved_network(network_id: &str) -> Result<(), String> {
    for path in saved_network_files(network_id) {
        match std::fs::remove_file(&path) {
            Ok(()) => info!("[ZEROTIER] Forgot saved network {} (daemon not running)", path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "Couldn't remove {} from ZeroTier's saved networks: {e}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

/// Is a system ZeroTier installed (its state dir or binary exists)? Then a
/// network we can't see may be joined there.
#[cfg(target_os = "linux")]
fn system_zerotier_installed() -> bool {
    ["/var/lib/zerotier-one", "/usr/sbin/zerotier-one", "/usr/bin/zerotier-one"]
        .iter()
        .any(|p| Path::new(p).exists())
}

/// Placeholder: a leave that couldn't reach ZeroTier. The UI has already
/// cleared the room, so this says what happens next rather than asking for a
/// retry that no longer exists.
const LEAVE_NOT_RUNNING_MSG: &str = "ZeroTier isn't running or isn't responding, so this device \
     couldn't be taken off the network yet. Drop takes it off the next time it starts with ZeroTier running.";

/// Leave one network (takes `ZT_OP`).
///
/// When the API refuses our token, that's reported as a token problem. When
/// nothing answers, the network is only treated as left if we can show it:
/// it's saved in Drop's own data dir and Drop's own daemon is positively not
/// running (no child, no live pid from its pid file), so deleting the saved
/// config is the leave; or it's saved nowhere we can see and no system
/// ZeroTier is installed. Anything else (a stopped Windows or system service,
/// our daemon alive but not answering) is an error.
async fn leave_network(network_id: &str) -> Result<(), String> {
    let nid = network_id.to_lowercase();
    let _op = ZT_OP.lock().await;
    match zt_api_raw(reqwest::Method::GET, "/status", None).await {
        Ok(_) => {
            zt_api(reqwest::Method::DELETE, &format!("/network/{nid}"), None)
                .await
                .inspect_err(|e| warn!("[ZEROTIER] Failed to leave network {nid}: {e}"))?;
            info!("[ZEROTIER] Left network {nid}");
            return Ok(());
        }
        Err(ZtError::Unauthorized) => {
            let e = ZtError::Unauthorized.to_string();
            warn!("[ZEROTIER] Can't leave {nid}: {e}");
            return Err(e);
        }
        Err(e) => info!("[ZEROTIER] Leaving {nid} with no daemon answering ({e})"),
    }
    #[cfg(target_os = "linux")]
    if token_override().is_none() && !daemon_alive().await && own_daemon_pid().is_none() {
        if saved_network_files(&nid).iter().any(|p| p.exists()) {
            return forget_saved_network(&nid);
        }
        if !system_zerotier_installed() {
            info!("[ZEROTIER] {nid} isn't saved anywhere; nothing to leave");
            return Ok(());
        }
    }
    warn!("[ZEROTIER] Can't leave {nid}: ZeroTier isn't running or isn't responding");
    Err(LEAVE_NOT_RUNNING_MSG.to_string())
}

/// Leave one network. Failure is returned, not swallowed.
#[tauri::command]
pub async fn zerotier_leave(network_id: String) -> Result<(), String> {
    if !is_valid_network_id(&network_id) {
        return Err("Invalid network id.".to_string());
    }
    leave_network(&network_id).await
}

/// Stop Drop's own daemon if nothing is joined on it any more. On Windows (the
/// official service) and with a system service this does nothing.
#[tauri::command]
pub async fn zerotier_stop() -> Result<(), String> {
    stop_daemon_if_idle().await;
    Ok(())
}

/// Read this node's assigned ZeroTier IPv4 on a network (e.g. "10.242.7.153"),
/// or None if ZeroTier hasn't assigned one yet. The host self-reports this so
/// joiners can connect to the game by IP.
#[tauri::command]
pub async fn zerotier_network_ip(network_id: String) -> Result<Option<String>, String> {
    if !is_valid_network_id(&network_id) {
        return Err("Invalid network id.".to_string());
    }
    let net = zt_api(
        reqwest::Method::GET,
        &format!("/network/{}", network_id.to_lowercase()),
        None,
    )
    .await?;
    Ok(match classify_network(&net) {
        NetPhase::Ok { ip } => ip,
        _ => None,
    })
}

/// POST the host's assigned ZeroTier IP to the server so joiners read it back.
async fn report_host_address(room_id: &str, address: &str) {
    let path = format!("/api/v1/client/room/{room_id}/address");
    let url = match generate_url(&[path.as_str()], &[]) {
        Ok(u) => u,
        Err(e) => {
            warn!("[ZEROTIER] could not build address-report url: {e}");
            return;
        }
    };
    let body = serde_json::json!({ "address": address });
    match make_authenticated_post(url, &body).await {
        Ok(resp) if resp.status().is_success() => {
            info!("[ZEROTIER] reported host address {address} for room {room_id}")
        }
        Ok(resp) => {
            let msg = server_error_message(resp, "host-address report rejected").await;
            warn!("[ZEROTIER] {msg}")
        }
        Err(e) => warn!("[ZEROTIER] host-address report failed: {e}"),
    }
}

/// Report the host's address now if we already have it, otherwise poll for it
/// in the background. The server also derives the host address straight from
/// the controller, so this self-report is a fallback for the controller-miss
/// case and older servers; hosting never fails because it's slow.
fn spawn_host_address_report(room_id: String, network_id: String, known: Option<String>) {
    tokio::spawn(async move {
        if let Some(ip) = known {
            report_host_address(&room_id, &ip).await;
            return;
        }
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if remote::coop::current_room_id().as_deref() != Some(room_id.as_str()) {
                return; // left the room meanwhile
            }
            if let Ok(Some(ip)) = zerotier_network_ip(network_id.clone()).await {
                report_host_address(&room_id, &ip).await;
                return;
            }
        }
        warn!("[ZEROTIER] host IP not assigned within 60s for room {room_id}");
    });
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
    /// Set by the join endpoint when the caller is the room's host (rejoining
    /// its own room). Never persisted.
    #[serde(default)]
    pub is_host: bool,
}

/// While `room_id` stays the active room, periodically re-seed every co-op
/// game's `custom_broadcasts.txt` with the room's current peers. This is the
/// fallback for `room_members` (which applies peer changes on every UI poll);
/// it exits when the active room changes (leave / rejoin).
fn spawn_coop_reseed_loop(room_id: String) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(12)).await;
            if remote::coop::current_room_id().as_deref() != Some(room_id.as_str()) {
                break;
            }
            remote::coop::reseed_all().await;
        }
    });
}

/// Mark `info` as the active room locally (after a successful network join).
async fn activate_room(info: &RoomInfo) {
    cache_controller_node_id(&info.network_id);
    set_active_coop_network(Some(info.network_id.to_lowercase()));
    let already = remote::coop::current_room_id().as_deref() == Some(info.room_id.as_str());
    remote::coop::set_active_room(&info.room_id);
    if !already {
        spawn_coop_reseed_loop(info.room_id.clone());
    }
    sweep_after_coop_join(&info.network_id).await;
}

/// Tell the server this device is leaving (or, for the host, ending) a room.
/// A 404 means the room is already gone, which is what we wanted.
async fn server_leave_room(room_id: &str) -> Result<(), String> {
    let path = format!("/api/v1/client/room/{room_id}/leave");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_post(url, &serde_json::json!({}))
        .await
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() || resp.status().as_u16() == 404 {
        return Ok(());
    }
    Err(server_error_message(resp, "The server didn't accept leaving the room").await)
}

/// A local join failed after the server already registered us: undo the
/// server side so the room (host) or membership (joiner) doesn't linger.
async fn rollback_server_room(room_id: &str, local_error: String) -> String {
    match server_leave_room(room_id).await {
        Ok(()) => {
            info!("[ZEROTIER] Rolled back room {room_id} on the server after a failed local join");
            local_error
        }
        Err(e) => {
            warn!("[ZEROTIER] Could not roll back room {room_id} on the server: {e}");
            format!("{local_error} (The room could not be cleaned up on the server: {e})")
        }
    }
}

/// Host a new co-op room: bring the daemon up, ask the server to mint a network
/// and authorize this node, then join it. Returns the shareable short code.
#[tauri::command]
pub async fn room_host(game_id: Option<String>) -> Result<RoomInfo, String> {
    let result = room_host_inner(game_id).await;
    if result.is_err() {
        // zerotier_prepare may have started Drop's daemon for nothing.
        stop_daemon_if_idle().await;
    }
    result
}

async fn room_host_inner(game_id: Option<String>) -> Result<RoomInfo, String> {
    let _in_use = DaemonInUse::new();
    let node_id = zerotier_prepare().await?;
    let url = generate_url(&["/api/v1/client/room"], &[]).map_err(|e| e.to_string())?;
    let mut body = serde_json::json!({ "zerotierNodeId": node_id });
    if let Some(g) = game_id.filter(|g| !g.is_empty()) {
        body["gameId"] = Value::String(g);
    }
    let resp = make_authenticated_post(url, &body)
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "The server couldn't create a room").await);
    }
    let info: RoomInfo = resp.json().await.map_err(|e| e.to_string())?;
    let ip = match join_and_wait(&info.network_id, JoinKind::Host).await {
        Ok(ip) => ip,
        Err(e) => return Err(rollback_server_room(&info.room_id, e).await),
    };
    activate_room(&info).await;
    spawn_host_address_report(info.room_id.clone(), info.network_id.clone(), ip);
    Ok(info)
}

/// Shared by `room_join` and `room_resume`: authorize this node onto the room
/// by code (the server upserts the membership), then join the network.
async fn join_room_by_code(short_code: &str, rollback_on_fail: bool) -> Result<(RoomInfo, Option<String>), String> {
    let result = join_room_by_code_inner(short_code, rollback_on_fail).await;
    if result.is_err() {
        // zerotier_prepare may have started Drop's daemon for nothing.
        stop_daemon_if_idle().await;
    }
    result
}

async fn join_room_by_code_inner(
    short_code: &str,
    rollback_on_fail: bool,
) -> Result<(RoomInfo, Option<String>), String> {
    let _in_use = DaemonInUse::new();
    let node_id = zerotier_prepare().await?;
    let url = generate_url(&["/api/v1/client/room/join"], &[]).map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "shortCode": short_code, "zerotierNodeId": node_id });
    let resp = make_authenticated_post(url, &body)
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Couldn't join that room").await);
    }
    let info: RoomInfo = resp.json().await.map_err(|e| e.to_string())?;
    let kind = if info.is_host { JoinKind::Host } else { JoinKind::Joiner };
    match join_and_wait(&info.network_id, kind).await {
        Ok(ip) => Ok((info, ip)),
        // Never roll back for the host: leaving as host ends the room for
        // everyone, which a failed rejoin must not do.
        Err(e) if rollback_on_fail && !info.is_host => {
            Err(rollback_server_room(&info.room_id, e).await)
        }
        Err(e) => Err(e),
    }
}

/// Join an existing room by its short code: bring the daemon up, ask the server
/// to authorize this node onto the room's network, then join it. If the local
/// join fails the server membership is undone.
#[tauri::command]
pub async fn room_join(short_code: String) -> Result<RoomInfo, String> {
    let (info, ip) = join_room_by_code(&short_code, true).await?;
    activate_room(&info).await;
    if info.is_host {
        spawn_host_address_report(info.room_id.clone(), info.network_id.clone(), ip);
    }
    Ok(info)
}

/// Rejoin a room the server says this device is still in (after a restart or
/// crash). Goes through the join endpoint so the server re-authorizes this
/// node, which also covers a changed ZeroTier identity. A failure leaves the
/// server side alone so the user can still choose to leave explicitly.
#[tauri::command]
pub async fn room_resume(short_code: String, is_host: bool) -> Result<RoomInfo, String> {
    let (info, ip) = join_room_by_code(&short_code, false).await?;
    activate_room(&info).await;
    if is_host || info.is_host {
        spawn_host_address_report(info.room_id.clone(), info.network_id.clone(), ip);
    }
    Ok(info)
}

/// Leave a room: tell the server, drop off this room's network only, and stop
/// Drop's own daemon if nothing else (e.g. Archipelago) is still using it.
/// Local cleanup always runs; a server failure is reported afterwards.
#[tauri::command]
pub async fn room_leave(room_id: String, network_id: String) -> Result<(), String> {
    remote::coop::clear_active_room();
    set_active_coop_network(None);
    let server = server_leave_room(&room_id).await;
    let local = if is_valid_network_id(&network_id) {
        zerotier_leave(network_id).await
    } else {
        Ok(())
    };
    stop_daemon_if_idle().await;
    match (server, local) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), _) => Err(format!("Left the room on this device, but the server didn't confirm it: {e}")),
        (Ok(()), Err(e)) => Err(format!("Left the room, but ZeroTier couldn't disconnect this device from its network: {e}")),
    }
}

/// The room the server says this device is in (as host or member), or None.
async fn fetch_my_room() -> Result<Option<Value>, String> {
    let url = generate_url(&["/api/v1/client/room/mine"], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Couldn't check for an active room").await);
    }
    resp.json::<Value>().await.map(Some).map_err(|e| e.to_string())
}

/// For restart recovery: the room this device is still in on the server, with
/// `active` telling the UI whether this app session is already connected to it.
#[tauri::command]
pub async fn room_mine() -> Result<Option<Value>, String> {
    let Some(mut room) = fetch_my_room().await? else {
        return Ok(None);
    };
    let active = room
        .get("roomId")
        .and_then(|v| v.as_str())
        .is_some_and(|id| remote::coop::current_room_id().as_deref() == Some(id));
    if let Some(obj) = room.as_object_mut() {
        obj.insert("active".to_string(), Value::Bool(active));
    }
    Ok(Some(room))
}

/// Fetch a room's current members/status from the server. Also applies the
/// returned peer list to every seeded game's `custom_broadcasts.txt` right
/// away, so it is current for that game's next launch (a running game read the
/// file at startup; see `remote::coop`).
#[tauri::command]
pub async fn room_members(room_id: String) -> Result<Value, String> {
    let path = format!("/api/v1/client/room/{room_id}");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        // A 404 means the room is gone (host ended it / it expired). Surface a
        // stable marker so the UI can treat it as "session ended", not an error.
        if resp.status().as_u16() == 404 {
            return Err("room_not_found".to_string());
        }
        return Err(server_error_message(resp, "Couldn't fetch the room").await);
    }
    let detail = resp.json::<Value>().await.map_err(|e| e.to_string())?;
    if remote::coop::current_room_id().as_deref() == Some(room_id.as_str()) {
        let peers: Vec<String> = detail
            .get("peerAddresses")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
            .unwrap_or_default();
        remote::coop::observe_peers(&peers);
    }
    Ok(detail)
}

/// Fetch the list of currently-joinable rooms from the server, so the user can
/// join one without the host sharing a code first.
#[tauri::command]
pub async fn room_browse() -> Result<Value, String> {
    let url = generate_url(&["/api/v1/client/room/browse"], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Couldn't list rooms").await);
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

// ── Archipelago sessions ──────────────────────────────────────────────
//
// Archipelago reuses the ZeroTier machinery above but differs in two ways:
// the server itself is ON the overlay (it hosts the Archipelago server, so
// players need a route to it), and there is ONE long-lived network for all
// sessions rather than one per room — Archipelago reads its advertised address
// from a config file at startup, so that address has to stay stable.

/// A session as returned by drop-server.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApSessionInfo {
    pub session_id: String,
    #[serde(default)]
    pub short_code: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub network_id: String,
    /// The server's own overlay IP — what Archipelago should advertise.
    #[serde(default)]
    pub server_address: Option<String>,
}

/// Pull the server's human-readable reason out of a failed response.
///
/// Worth the effort here specifically because YAML validation errors ("that slot
/// name is already taken", "missing a `game`") are the whole point of uploading
/// through Drop — collapsing them into "HTTP 409" would throw away the feature.
async fn server_error_message(resp: reqwest::Response, fallback: &str) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if let Ok(v) = serde_json::from_str::<Value>(&body) {
        for key in ["statusMessage", "message"] {
            if let Some(msg) = v.get(key).and_then(|m| m.as_str())
                && !msg.is_empty()
            {
                return msg.to_string();
            }
        }
    }
    format!("{fallback}: HTTP {status}")
}

/// Start a session: ensure the shared overlay exists (server-side), authorize
/// this device onto it, and join it locally.
#[tauri::command]
pub async fn ap_session_create(name: Option<String>) -> Result<ApSessionInfo, String> {
    let result = ap_session_create_inner(name).await;
    if result.is_err() {
        // zerotier_prepare may have started Drop's daemon for nothing.
        stop_daemon_if_idle().await;
    }
    result
}

async fn ap_session_create_inner(name: Option<String>) -> Result<ApSessionInfo, String> {
    let _in_use = DaemonInUse::new();
    let node_id = zerotier_prepare().await?;
    let url = generate_url(&["/api/v1/client/archipelago"], &[]).map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "zerotierNodeId": node_id, "name": name });
    let resp = make_authenticated_post(url, &body)
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Could not start a session").await);
    }
    let info: ApSessionInfo = resp.json().await.map_err(|e| e.to_string())?;
    join_and_wait(&info.network_id, JoinKind::Archipelago).await?;
    remember_ap_network(&info.network_id);
    sweep_after_ap_join(&info.network_id).await;
    Ok(info)
}

/// Join a session by short code.
#[tauri::command]
pub async fn ap_session_join(short_code: String) -> Result<ApSessionInfo, String> {
    let result = ap_session_join_inner(short_code).await;
    if result.is_err() {
        // zerotier_prepare may have started Drop's daemon for nothing.
        stop_daemon_if_idle().await;
    }
    result
}

async fn ap_session_join_inner(short_code: String) -> Result<ApSessionInfo, String> {
    let _in_use = DaemonInUse::new();
    let node_id = zerotier_prepare().await?;
    let url = generate_url(&["/api/v1/client/archipelago/join"], &[]).map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "shortCode": short_code, "zerotierNodeId": node_id });
    let resp = make_authenticated_post(url, &body)
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Could not join that session").await);
    }
    let info: ApSessionInfo = resp.json().await.map_err(|e| e.to_string())?;
    join_and_wait(&info.network_id, JoinKind::Archipelago).await?;
    remember_ap_network(&info.network_id);
    sweep_after_ap_join(&info.network_id).await;
    Ok(info)
}

/// Current session state: slots, who has uploaded a YAML, and the connect info.
#[tauri::command]
pub async fn ap_session_get(session_id: String) -> Result<Value, String> {
    let path = format!("/api/v1/client/archipelago/{session_id}");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        // Mirrors `room_members`: a stable marker so the UI can show "session
        // ended" rather than an error.
        if resp.status().as_u16() == 404 {
            return Err("session_not_found".to_string());
        }
        return Err(server_error_message(resp, "Could not fetch session").await);
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

/// The caller's open sessions, so a restarted client can offer to rejoin.
#[tauri::command]
pub async fn ap_session_list() -> Result<Value, String> {
    let url = generate_url(&["/api/v1/client/archipelago"], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Could not list sessions").await);
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

/// WebHost integration config: the browser-openable Archipelago WebHost URL the
/// operator configured (if any) plus the games it supports, so the client can
/// link to it and deep-link to a game's options page. Returns
/// `{ webHostUrl: string | null, games: string[] }`.
#[tauri::command]
pub async fn ap_web_host() -> Result<Value, String> {
    let url = generate_url(&["/api/v1/client/archipelago/config"], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Could not fetch Archipelago config").await);
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

/// Upload a player YAML from disk. The frontend picks the file with the dialog
/// plugin and passes its path.
#[tauri::command]
pub async fn ap_yaml_upload(session_id: String, file_path: String) -> Result<Value, String> {
    let text = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| format!("Could not read {file_path}: {e}"))?;

    let path = format!("/api/v1/client/archipelago/{session_id}/yaml");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_post(url, &serde_json::json!({ "yaml": text }))
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "That YAML was rejected").await);
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

/// Host records the connect string from the Archipelago room page.
#[tauri::command]
pub async fn ap_connect_set(session_id: String, connect_address: String) -> Result<Value, String> {
    let path = format!("/api/v1/client/archipelago/{session_id}/connect");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_post(url, &serde_json::json!({ "connectAddress": connect_address }))
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Could not save the connect address").await);
    }
    resp.json::<Value>().await.map_err(|e| e.to_string())
}

/// Download every valid slot's YAML as one multi-document file, written to
/// `dest_path` (chosen by the frontend's save dialog). Returns the path written.
#[tauri::command]
pub async fn ap_bundle_save(session_id: String, dest_path: String) -> Result<String, String> {
    let path = format!("/api/v1/client/archipelago/{session_id}/bundle");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Could not build the bundle").await);
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    tokio::fs::write(&dest_path, body)
        .await
        .map_err(|e| format!("Could not write {dest_path}: {e}"))?;
    Ok(dest_path)
}

/// Forget the cached Archipelago overlay id (no session is open any more). A
/// failed removal only means a later sweep keeps the overlay by id until the
/// server check runs again.
fn forget_ap_network() {
    if let Err(e) = std::fs::remove_file(ap_network_cache_path())
        && e.kind() != std::io::ErrorKind::NotFound
    {
        warn!("[ZEROTIER] Could not clear the Archipelago network cache: {e}");
    }
}

/// Record the Archipelago overlay so co-op sweeps keep it while a session is
/// open. A failed write only weakens those sweeps.
fn remember_ap_network(network_id: &str) {
    cache_controller_node_id(network_id);
    if let Err(e) = std::fs::create_dir_all(zerotier_dir())
        .and_then(|_| std::fs::write(ap_network_cache_path(), network_id.to_lowercase()))
    {
        warn!("[ZEROTIER] Could not cache the Archipelago network id: {e}");
    }
}

/// Prefix on an `ap_session_leave` error meaning the server never recorded the
/// leave, so nothing local was changed and the UI should keep the session and
/// offer a retry. The rest of the string is the reason.
const AP_LEAVE_NOT_RECORDED: &str = "ap_leave_not_recorded:";

/// Tell the server this device is leaving (or, for the host, closing) a
/// session. The server answers success for a session that no longer exists.
async fn ap_server_leave(session_id: &str) -> Result<(), String> {
    let path = format!("/api/v1/client/archipelago/{session_id}/leave");
    let url = generate_url(&[path.as_str()], &[]).map_err(|e| e.to_string())?;
    let resp = make_authenticated_post(url, &serde_json::json!({}))
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(server_error_message(resp, "Leaving the session was rejected").await);
    }
    Ok(())
}

/// Leave a session and drop off the Archipelago overlay. Only that network is
/// left, so a co-op room in progress keeps working, and Drop's own daemon is
/// stopped only if nothing else is joined.
///
/// The server is told first. If it can't be (offline, rejected), nothing local
/// is touched and the error carries `AP_LEAVE_NOT_RECORDED`, so the UI keeps
/// the session and can retry: leaving the overlay while the server still lists
/// the session would just have the next restore rejoin it.
///
/// The overlay is shared across Archipelago sessions, so this disconnects any
/// other session too. Acceptable for now: running two multiworlds at once
/// isn't a supported flow.
#[tauri::command]
pub async fn ap_session_leave(session_id: String, network_id: Option<String>) -> Result<(), String> {
    if let Err(e) = ap_server_leave(&session_id).await {
        warn!("[ZEROTIER] Could not tell the server we left Archipelago session {session_id}: {e}");
        return Err(format!("{AP_LEAVE_NOT_RECORDED}{e}"));
    }
    let result = ap_leave_overlay(network_id).await;
    if result.is_ok() {
        forget_ap_network();
    }
    stop_daemon_if_idle().await;
    result
}

/// Leave the Archipelago overlay locally: `network_id`, else the cached one.
/// Nothing to leave is success.
async fn ap_leave_overlay(network_id: Option<String>) -> Result<(), String> {
    match network_id.or_else(read_cached_ap_network_id) {
        Some(nid) if is_valid_network_id(&nid) => zerotier_leave(nid).await,
        _ => Ok(()),
    }
}

/// Forget a session on this device only, for when the server can't record a
/// leave (e.g. it is gone for good): leave the Archipelago overlay, clear the
/// cached overlay id and stop Drop's daemon if nothing else is joined. The
/// server is not told, so it may still list this device in the session.
///
/// The cached id is cleared even if leaving the overlay fails: the user asked
/// to be out, and keeping it would only make later sweeps keep the overlay.
/// The leave error is still returned so the UI can say what didn't happen.
#[tauri::command]
pub async fn ap_session_forget(network_id: Option<String>) -> Result<(), String> {
    info!("[ZEROTIER] Forgetting the Archipelago session on this device only");
    let result = ap_leave_overlay(network_id).await;
    forget_ap_network();
    stop_daemon_if_idle().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_in_use_guard_counts_while_alive() {
        let before = daemon_users();
        {
            let _a = DaemonInUse::new();
            let _b = DaemonInUse::new();
            assert_eq!(daemon_users(), before + 2);
        }
        assert_eq!(daemon_users(), before);
    }

    #[test]
    fn placeholder_is_fixed_length_and_matches_the_workflow_prefix() {
        let p = rpath_placeholder();
        assert_eq!(p.len(), RPATH_PLACEHOLDER_LEN);
        assert!(p.starts_with(b"/drop-zt-libs-placeholder/"));
        assert!(!p.contains(&0));
    }

    #[test]
    fn rewrite_replaces_and_nul_pads_without_changing_size() {
        let mut bin = b"\0libc.so.6\0".to_vec();
        bin.extend(rpath_placeholder());
        bin.extend(b"\0tail");
        let before = bin.len();
        let real = "/home/alice/.local/share/drop/tools/zerotier/libs";
        assert_eq!(rewrite_rpath_placeholder(&mut bin, real).unwrap(), 1);
        assert_eq!(bin.len(), before);
        assert!(contains_c_string(&bin, real));
        assert!(find_all(&bin, &rpath_placeholder()).is_empty());
        assert!(bin.ends_with(b"\0tail"));
    }

    #[test]
    fn rewrite_refuses_a_path_longer_than_the_placeholder() {
        let mut bin = rpath_placeholder();
        let long = format!("/{}", "a".repeat(RPATH_PLACEHOLDER_LEN));
        assert!(rewrite_rpath_placeholder(&mut bin, &long).is_err());
        assert_eq!(bin, rpath_placeholder(), "a refused rewrite must not touch the bytes");
        assert!(rewrite_rpath_placeholder(&mut bin, "/a:/b").is_err());
    }

    #[test]
    fn rewrite_without_placeholder_reports_zero() {
        let mut bin = b"\0/home/deck/.local/share/drop/tools/zerotier/libs\0".to_vec();
        assert_eq!(rewrite_rpath_placeholder(&mut bin, "/x").unwrap(), 0);
        assert!(contains_c_string(&bin, "/home/deck/.local/share/drop/tools/zerotier/libs"));
        // A longer path that merely ends with the needle isn't a match.
        assert!(!contains_c_string(&bin, "/share/drop/tools/zerotier/libs"));
    }

    #[test]
    fn only_a_gamescope_session_counts_as_game_mode() {
        assert!(is_gamescope_session(Some("gamescope")));
        assert!(is_gamescope_session(Some("Gamescope")));
        assert!(!is_gamescope_session(Some("KDE")));
        assert!(!is_gamescope_session(Some("ubuntu:GNOME")));
        assert!(!is_gamescope_session(None));
    }

    #[test]
    fn steamos_is_read_from_os_release() {
        assert!(os_release_is_steamos("NAME=\"SteamOS\"\nID=steamos\nID_LIKE=arch\n"));
        assert!(os_release_is_steamos("ID=\"steamos\""));
        assert!(!os_release_is_steamos("ID=arch\nID_LIKE=steamos\n"));
        assert!(!os_release_is_steamos(""));
    }

    #[test]
    fn join_errors_are_worded_for_who_is_joining() {
        let kinds = [JoinKind::Host, JoinKind::Joiner, JoinKind::Archipelago];
        for k in kinds {
            for msg in [k.denied(), k.not_found()] {
                assert!(!msg.contains('\u{2014}'), "em-dash in {msg}");
            }
        }
        assert!(!JoinKind::Host.denied().contains("ask the host"));
        assert!(!JoinKind::Archipelago.denied().contains("room"));
        assert!(!JoinKind::Archipelago.not_found().contains("room"));
        assert!(JoinKind::Archipelago.network().contains("Archipelago"));
    }

    #[test]
    fn network_status_classification() {
        use serde_json::json;
        assert_eq!(
            classify_network(&json!({"status": "OK", "assignedAddresses": ["fd00::1/88", "10.242.7.3/24"]})),
            NetPhase::Ok { ip: Some("10.242.7.3".into()) }
        );
        assert_eq!(classify_network(&json!({"status": "OK", "assignedAddresses": []})), NetPhase::Ok { ip: None });
        assert_eq!(classify_network(&json!({"status": "REQUESTING_CONFIGURATION"})), NetPhase::Pending);
        assert_eq!(classify_network(&json!({})), NetPhase::Pending);
        assert_eq!(classify_network(&json!({"status": "ACCESS_DENIED"})), NetPhase::Denied);
        assert_eq!(classify_network(&json!({"status": "NOT_FOUND"})), NetPhase::NotFound);
        assert_eq!(
            classify_network(&json!({"status": "PORT_ERROR"})),
            NetPhase::Other("PORT_ERROR".into())
        );
    }

    #[test]
    fn leave_marker_matches_the_frontend() {
        // main/composables/archipelago-logic.ts looks for this exact prefix.
        assert_eq!(AP_LEAVE_NOT_RECORDED, "ap_leave_not_recorded:");
    }

    #[test]
    fn sweep_leaves_only_other_drop_networks() {
        let controller = "abcdef0123";
        let net = |id: &str, name: &str| JoinedNet { id: id.into(), name: name.into() };
        let joined = vec![
            net("abcdef0123000001", "drop-ABC234"),      // current room
            net("ABCDEF0123000002", "drop-XYZ789"),      // stale room
            net("abcdef0123000003", ""),                 // archipelago, by cached id
            net("abcdef0123000004", AP_NETWORK_NAME),    // archipelago, id not cached
            net("1234567890000001", "home"),             // user's own network
        ];
        let keep = vec!["abcdef0123000001".to_string(), "ABCDEF0123000003".to_string()];
        assert_eq!(
            networks_to_sweep(&joined, controller, &keep, true),
            vec!["abcdef0123000002".to_string()]
        );
        // No open Archipelago session: its overlay goes too (unless kept by id).
        assert_eq!(
            networks_to_sweep(&joined, controller, &keep, false),
            vec!["abcdef0123000002".to_string(), "abcdef0123000004".to_string()]
        );
    }
}
