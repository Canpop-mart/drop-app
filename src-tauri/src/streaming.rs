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
use std::sync::atomic::{AtomicBool, Ordering};

use database::{borrow_db_checked, borrow_db_mut_checked, GameDownloadStatus};
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

/// Paths — relative to whichever directory Sunshine's binary lives in — that a
/// working install must have. The Windows portable zip ships the web UI and the
/// helper tools as siblings of the exe, so an extract that flattens the archive
/// leaves `sunshine.exe` sitting there looking installed while every one of
/// these is gone. `true` marks a directory.
#[cfg(target_os = "windows")]
const SUNSHINE_REQUIRED_PATHS: &[(&str, bool)] = &[
    ("sunshine.exe", false),
    ("assets/web/index.html", false),
    ("assets/web/assets/css/sunshine.css", false),
    ("assets/shaders/directx", true),
    ("tools/dxgi-info.exe", false),
];
#[cfg(target_os = "linux")]
const SUNSHINE_REQUIRED_PATHS: &[(&str, bool)] = &[("sunshine.AppImage", false)];
#[cfg(target_os = "macos")]
const SUNSHINE_REQUIRED_PATHS: &[(&str, bool)] = &[("sunshine", false)];

/// Shown whenever a download succeeded but left an install that can't serve a
/// stream. Points at Repair because that is the only thing that fixes it.
const SUNSHINE_UNPACK_FAILED: &str =
    "Sunshine downloaded but did not unpack correctly. Try Repair.";

/// Required paths missing under `root`, in declaration order. Empty = healthy.
fn missing_sunshine_files(root: &Path) -> Vec<&'static str> {
    SUNSHINE_REQUIRED_PATHS
        .iter()
        .filter(|(rel, is_dir)| {
            let path = root.join(rel);
            if *is_dir { !path.is_dir() } else { !path.is_file() }
        })
        .map(|(rel, _)| *rel)
        .collect()
}

/// The directory Sunshine has to run from. `SUNSHINE_ASSETS_DIR` defaults to
/// the *relative* string "assets", so a Sunshine launched with Drop's working
/// directory looks for its web UI under Drop instead of next to itself.
/// `None` for a bare name resolved off PATH — there is no directory to use.
fn sunshine_working_dir(binary: &Path) -> Option<PathBuf> {
    binary
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
}

/// Find the Sunshine binary — check Drop's tools dir, then PATH.
fn find_sunshine() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let exe_name = "sunshine.exe";
    #[cfg(target_os = "linux")]
    let exe_name = "sunshine.AppImage";
    #[cfg(target_os = "macos")]
    let exe_name = "sunshine";

    // Flat is the layout Drop extracts to. The nested one is what an install
    // that kept the archive's own `Sunshine/` wrapper looks like — the same
    // fallback `install_moonlight` does after its extract.
    for bundled in [
        sunshine_dir().join(exe_name),
        sunshine_dir().join("Sunshine").join(exe_name),
    ] {
        if bundled.exists() {
            return Some(bundled);
        }
    }

    // Check PATH
    let name = if cfg!(target_os = "windows") { "sunshine.exe" } else { "sunshine" };
    if let Ok(output) = Command::new(name).arg("--version").output()
        && output.status.success()
    {
        return Some(PathBuf::from(name));
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

/// What one look at the filesystem says about Sunshine. Shared by
/// `sunshine_status` and the post-extract verification so the two can never
/// disagree about what is on disk.
struct SunshineProbe {
    binary: Option<PathBuf>,
    /// Required paths missing next to that binary. Always empty when Sunshine
    /// came off PATH or a system location: Drop didn't unpack that one, so its
    /// layout isn't Drop's to judge.
    missing: Vec<&'static str>,
}

impl SunshineProbe {
    fn installed(&self) -> bool {
        self.binary.is_some()
    }

    /// Installed *and* complete enough to actually serve a stream.
    fn healthy(&self) -> bool {
        self.binary.is_some() && self.missing.is_empty()
    }
}

fn probe_sunshine_install() -> SunshineProbe {
    let binary = find_sunshine();
    let install_dir = sunshine_dir();
    let missing = match binary.as_deref() {
        Some(path) if path.starts_with(&install_dir) => {
            missing_sunshine_files(path.parent().unwrap_or(install_dir.as_path()))
        }
        _ => Vec::new(),
    };
    SunshineProbe { binary, missing }
}

/// Check if Sunshine is installed and return its path.
#[tauri::command]
pub fn check_sunshine() -> Option<String> {
    find_sunshine().map(|p| p.to_string_lossy().to_string())
}

// ── Archive extraction ────────────────────────────────────────────────

/// Normalise a zip entry name to forward slashes and reject anything that
/// could write outside the destination: `..`, a rooted path, or a drive
/// letter. `None` also covers entries with no usable name left.
///
/// The colon rule has to apply to *every* component, not just the first. A
/// drive letter buried mid-path (`Sunshine/C:/Windows/evil.dll`) is five
/// ordinary components as far as `Path` is concerned, so it sails past
/// `enclosed_name`, and then `strip_wrapper` promotes the `C:` to the front and
/// makes the result absolute. Rejecting the colon outright also kills NTFS
/// alternate data streams (`index.html:evil.exe`), which are never legitimate
/// in an archive Drop unpacks.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn sanitise_zip_entry(raw: &str) -> Option<String> {
    let name = raw.replace('\\', "/");
    if name.starts_with('/') {
        return None;
    }

    let mut parts = Vec::new();
    for part in name.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            _ if part.contains(':') => return None,
            _ => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The single top-level directory every entry shares, if there is one.
///
/// Sunshine's portable zip wraps its whole tree in `Sunshine/`, so writing the
/// names verbatim would nest to `sunshine/Sunshine/sunshine.exe`. Stripping is
/// only safe when *every* entry sits under the same root, and only when at
/// least one entry actually lives inside it — otherwise a flat single-file
/// archive would have its one file stripped away to nothing.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn shared_wrapper_dir(names: &[String]) -> Option<String> {
    let candidate = names.first()?.split('/').next()?.to_string();
    let prefix = format!("{candidate}/");
    let mut has_nested = false;
    for name in names {
        if *name == candidate {
            continue;
        }
        match name.strip_prefix(&prefix) {
            Some(rest) if !rest.is_empty() => has_nested = true,
            _ => return None,
        }
    }
    has_nested.then_some(candidate)
}

/// Drop the wrapper component from a sanitised entry name. `None` when nothing
/// is left, i.e. the wrapper's own directory entry.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn strip_wrapper(name: &str, wrapper: Option<&str>) -> Option<String> {
    match wrapper {
        Some(w) if name == w => None,
        Some(w) => name.strip_prefix(&format!("{w}/")).map(str::to_string),
        None => Some(name.to_string()),
    }
}

/// Extract a zip into `dest`, stripping the archive's wrapper directory when
/// it has one. Unsafe entries are dropped rather than written.
#[cfg(target_os = "windows")]
fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Failed to open archive: {e}"))?;

    // Two passes: the wrapper can only be identified once every name is known.
    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("Archive error: {e}"))?;
        // `enclosed_name` is zip's own traversal guard; when it refuses, fall
        // back to the raw name so our sanitiser gets its own shot at rejecting.
        let raw = file
            .enclosed_name()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.name().to_string());
        entries.push((i, file.is_dir(), sanitise_zip_entry(&raw)));
    }

    let names: Vec<String> = entries.iter().filter_map(|(_, _, n)| n.clone()).collect();
    let wrapper = shared_wrapper_dir(&names);
    if let Some(w) = &wrapper {
        info!("[SUNSHINE] Archive is wrapped in '{w}/' — stripping that one component");
    }

    for (index, is_dir, name) in entries {
        let Some(name) = name else {
            warn!("[SUNSHINE] Skipped archive entry #{index}: unsafe path");
            continue;
        };
        let Some(rel) = strip_wrapper(&name, wrapper.as_deref()) else {
            continue;
        };
        let out_path = dest.join(&rel);
        // Last line of defence, checked on the path that is actually written
        // rather than on the name the sanitiser saw. `Path::join` silently
        // replaces the whole buffer when what it is given turns out to be
        // absolute, so anything that ends up outside `dest` is dropped here no
        // matter which earlier guard let it through.
        if !out_path.starts_with(dest) {
            warn!("[SUNSHINE] Skipped archive entry #{index}: '{rel}' escapes the install directory");
            continue;
        }

        if is_dir {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create {}: {e}", out_path.display()))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
        let mut file = archive
            .by_index(index)
            .map_err(|e| format!("Archive error: {e}"))?;
        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| format!("Failed to create {}: {e}", out_path.display()))?;
        std::io::copy(&mut file, &mut out_file)
            .map_err(|e| format!("Failed to extract {rel}: {e}"))?;
    }

    Ok(())
}

/// A Sunshine running out of Drop's own install directory, if there is one.
///
/// `stop_sunshine` only ends a process Drop is currently holding a handle to,
/// so it misses the two states where something else still owns files here: a
/// Sunshine adopted from an earlier Drop run (left alive on purpose), and one
/// orphaned by a Drop restart. Wiping under either deletes the assets, leaves
/// the locked exe behind, and the extract then dies on a raw "Access is denied".
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn sunshine_running_from_install_dir() -> Option<String> {
    if !sunshine_port_open() {
        return None;
    }
    let install_dir = sunshine_dir();
    running_sunshine_binaries()
        .into_iter()
        .find(|path| Path::new(path).starts_with(&install_dir))
}

/// Remove a previous install so a fresh extract can't inherit its debris —
/// except `config/`, which is where a Sunshine left to its own defaults would
/// put the cacert.pem/cakey.pem that every Moonlight pairing depends on. Drop
/// pins those into `sunshine-config/credentials/` instead (see
/// `generate_sunshine_conf`), so this skip is belt and braces rather than the
/// thing keeping pairings alive.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn wipe_sunshine_install(install_dir: &Path) -> Result<(), String> {
    if !install_dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(install_dir)
        .map_err(|e| format!("Failed to read {}: {e}", install_dir.display()))?;
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("config")
        {
            continue;
        }
        let path = entry.path();
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(e) = removed {
            // Usually a file still open by a running Sunshine. Whether that
            // actually mattered is what the post-extract probe decides.
            warn!("[SUNSHINE] Couldn't remove {}: {e}", path.display());
        }
    }
    Ok(())
}

/// Confirm an install actually landed. The old extractor happily reported
/// success while writing a directory that could never serve a stream, so a
/// download is only "installed" once the files are on disk.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn verify_sunshine_install(install_dir: &Path) -> Result<(), String> {
    let missing = missing_sunshine_files(install_dir);
    if missing.is_empty() {
        return Ok(());
    }
    warn!(
        "[SUNSHINE] Install at {} is missing: {}",
        install_dir.display(),
        missing.join(", ")
    );
    Err(SUNSHINE_UNPACK_FAILED.to_string())
}

/// Download and install Sunshine to Drop's tools directory.
#[tauri::command]
pub async fn install_sunshine() -> Result<String, String> {
    let path = download_and_install_sunshine().await?;
    configure_firewall_once().await;
    Ok(path)
}

/// Open the firewall as part of setup, at most once per PC.
///
/// This is where the UAC prompt belongs: the user just asked to install remote
/// play, so a prompt is expected and answering it once is the end of it. A
/// declined prompt is not fatal to the install — `sunshine_configure_firewall`
/// retries it, and `fulfill_stream_request` says so in plain words if a stream
/// is attempted while the firewall is still shut.
async fn configure_firewall_once() {
    if borrow_db_checked().settings.streaming_firewall_configured {
        return;
    }
    match tokio::task::spawn_blocking(ensure_sunshine_firewall).await {
        Ok(Ok(())) => info!("[SUNSHINE] Windows Firewall opened for remote play"),
        Ok(Err(e)) => warn!("[SUNSHINE] Firewall not configured: {e}"),
        Err(e) => warn!("[SUNSHINE] Firewall task failed: {e}"),
    }
}

/// Wipe and reinstall Sunshine.
///
/// Same work as `install_sunshine`; it exists as its own command so the UI can
/// offer it when Sunshine is present but broken — which is the state every
/// install left by the old flattening extractor is in.
#[tauri::command]
pub async fn repair_sunshine() -> Result<String, String> {
    info!("[SUNSHINE] Repair requested");
    // Our own child would hold the exe open. A Sunshine Drop didn't spawn is
    // left strictly alone: `stop_sunshine` only touches what Drop spawned.
    let _ = stop_sunshine().await;
    let path = download_and_install_sunshine().await?;
    configure_firewall_once().await;
    Ok(path)
}

async fn download_and_install_sunshine() -> Result<String, String> {
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

        // Nothing below can succeed while a live Sunshine holds files in here,
        // and the wipe would strip its assets out from under it. Say so before
        // spending a download on it.
        if let Some(path) = sunshine_running_from_install_dir() {
            return Err(format!(
                "Sunshine is still running from {path}. Close it, then try again."
            ));
        }

        info!("[SUNSHINE] Downloading from {}", download_url);

        let response = reqwest::get(&download_url)
            .await
            .map_err(|e| format!("Download failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Download failed: HTTP {}", response.status()));
        }

        let bytes = response.bytes().await.map_err(|e| format!("Download failed: {e}"))?;
        info!("[SUNSHINE] Downloaded {} bytes", bytes.len());

        // Only now that the replacement is in hand. Wiping any earlier means a
        // dropped connection or a GitHub outage turns a working install into no
        // install at all, with nothing to roll back to.
        wipe_sunshine_install(&install_dir)?;
        std::fs::create_dir_all(&install_dir)
            .map_err(|e| format!("Failed to create sunshine dir: {e}"))?;

        #[cfg(target_os = "windows")]
        {
            extract_zip(&bytes, &install_dir)?;
            verify_sunshine_install(&install_dir)?;
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

            verify_sunshine_install(&install_dir)?;
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

/// Quality profiles for streaming, chosen on the client (the device running
/// Moonlight). Each maps to an fps + bitrate handed to Moonlight; resolution and
/// HDR are separate settings. The encoder-quality knobs (NVENC preset, spatial
/// AQ, two-pass) are set globally in `sunshine.conf` and benefit every profile,
/// so the profile only varies the bandwidth/framerate trade-off.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub enum StreamQuality {
    /// Lowest latency / weakest network: 60 fps, ~18 Mbps.
    Performance,
    /// Default: 60 fps, ~30 Mbps.
    Balanced,
    /// Sharpest at 60 fps: ~50 Mbps. Good for story games over a solid link.
    Quality,
    /// Maxed out for a wired/LAN link: 120 fps, ~80 Mbps.
    Ultra,
}

impl StreamQuality {
    /// Parse the persisted settings string. Unknown/empty falls back to
    /// Balanced. Legacy values (`dataSaver`, `highQuality`) are still accepted.
    fn from_setting(s: &str) -> Self {
        match s {
            "performance" | "dataSaver" => Self::Performance,
            "quality" | "highQuality" => Self::Quality,
            "ultra" => Self::Ultra,
            _ => Self::Balanced,
        }
    }

    /// Moonlight stream parameters: (fps, bitrate_kbps).
    fn params(self) -> (u32, u32) {
        match self {
            Self::Performance => (60, 18_000),
            Self::Balanced => (60, 30_000),
            Self::Quality => (60, 50_000),
            Self::Ultra => (120, 80_000),
        }
    }
}

/// Parse the `streaming_resolution` setting into explicit dimensions, or `None`
/// to mean "leave the display alone / let Moonlight pick" (the `"native"`
/// option). Accepts `"1280x800"`, `"1920x1080"`, `"2560x1440"`, etc.
fn parse_stream_resolution(s: &str) -> Option<(u32, u32)> {
    let s = s.trim().to_ascii_lowercase();
    if s.is_empty() || s == "native" || s == "off" {
        return None;
    }
    let (w, h) = s.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// What `launch_moonlight` should ask the host for.
struct StreamResolution {
    /// The `--resolution` Moonlight requests, or `None` to omit the flag and let
    /// Moonlight pick its own default.
    request: Option<(u32, u32)>,
    /// Whether the *host's* display mode may be changed to match.
    ///
    /// Drives Moonlight's `--game-optimization`, which is the only thing that
    /// makes Sunshine act on `dd_resolution_option`. Kept false whenever the
    /// user has not explicitly picked a fixed resolution, so the desktop is left
    /// alone by default.
    change_host_mode: bool,
}

/// Resolve the resolution Moonlight should request (`--resolution`).
///
/// With `streaming_auto_resolution` on (the default), use the client's current
/// display size — passed from the frontend as `client_resolution` (e.g.
/// `"1920x1080"`) so docking the Deck to a TV "just works" without touching a
/// setting. With it off, use the manual `streaming_resolution`.
///
/// Auto-resolution deliberately does *not* set `change_host_mode`: the host
/// keeps rendering at its native mode and the client scales, because forcing the
/// host down to a handheld's panel size would just upscale and look soft. Only
/// the manual setting, which the user picked on purpose, is allowed to move the
/// host's mode.
///
/// Note: the client size is passed as data from the webview rather than read via
/// a `WebviewWindow` here — Drop's frontend is a *child webview*, not a
/// `WebviewWindow`, so injecting one into the command fails ("current webview is
/// not a webviewwindow").
fn resolve_stream_resolution(client_resolution: Option<&str>) -> StreamResolution {
    let (auto, manual) = {
        let db = borrow_db_checked();
        (
            db.settings.streaming_auto_resolution,
            db.settings.streaming_resolution.clone(),
        )
    };
    if auto {
        if let Some(res) = client_resolution.and_then(parse_stream_resolution) {
            info!(
                "[MOONLIGHT] Auto-resolution: streaming at the client's current display ({}x{})",
                res.0, res.1
            );
            return StreamResolution {
                request: Some(res),
                change_host_mode: false,
            };
        }
        warn!(
            "[MOONLIGHT] Auto-resolution is on but no usable client display size was provided; \
             falling back to the manual streaming_resolution setting"
        );
    }
    let request = parse_stream_resolution(&manual);
    StreamResolution {
        // "Don't change my resolution" parses to None, and that is exactly the
        // case where the host must be left alone. Neither does the auto
        // fallback: the user never picked that value for the host, it is just
        // the last resort for what to ask the client for.
        change_host_mode: !auto && request.is_some(),
        request,
    }
}

// ── Windows Firewall ──────────────────────────────────────────────────

/// The two inbound rules remote play needs, and the names Drop owns.
///
/// GameStream uses TCP 47984 (HTTPS) / 47989 (HTTP) / 47990 (web UI) / 48010
/// (RTSP) and UDP 47998-48000 (video/control/audio) + 48002 (mic).
#[cfg(target_os = "windows")]
const SUNSHINE_FIREWALL_RULES: &[(&str, &str, &str)] = &[
    ("Drop Sunshine (TCP)", "TCP", "47984-48010"),
    ("Drop Sunshine (UDP)", "UDP", "47998-48010"),
];

/// Told to the user when other devices cannot reach this PC.
pub const STREAM_ERROR_FIREWALL: &str =
    "Other devices cannot reach this PC until Windows Firewall allows remote play.";

/// Cached answer to "can anything reach Sunshine from outside this PC?".
/// 0 = not asked yet, 1 = allowed, 2 = blocked. Querying costs a `netsh` and a
/// PowerShell round trip, and the answer only changes when Drop itself adds the
/// rules — which resets this.
#[cfg(target_os = "windows")]
static FIREWALL_STATE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Open Sunshine's inbound ports in Windows Firewall, elevating if needed.
///
/// Drop runs the *portable* Sunshine as a child process, which — unlike the
/// Sunshine installer — never registers firewall rules. Without them Windows
/// silently drops inbound GameStream traffic, so Moonlight on another device
/// fails with "failed to connect to <host>:47989" even though Sunshine is
/// running locally.
///
/// `netsh advfirewall firewall add rule` needs elevation, so the unelevated
/// attempt is tried first (it succeeds outright when Drop is already running as
/// administrator) and only a nonzero exit escalates to a UAC prompt. Same
/// mechanism as `add_defender_exclusions`: one elevated PowerShell doing all of
/// the work, base64-encoded so nothing has to survive two rounds of quoting.
///
/// Sunshine ships `scripts/add-firewall-rule.bat`, which was the other option.
/// It is not used: it adds *program* rules named "Sunshine", which collide with
/// a separate Sunshine install's own rules, it appends a duplicate pair every
/// time it runs, it derives the exe path from `%~dp0\..` (wrong on installs the
/// old extractor flattened), and run through ShellExecuteW "runas" it reports
/// nothing back — no way to tell a declined UAC prompt from a success.
#[cfg(target_os = "windows")]
pub fn ensure_sunshine_firewall() -> Result<(), String> {
    // `any` short-circuits, so a first rule that needs elevation skips straight
    // to the elevated pass — which redoes both anyway.
    let unelevated_failed = SUNSHINE_FIREWALL_RULES.iter().any(|(name, proto, ports)| {
        // Drop any stale rule of the same name first so repeated runs stay
        // idempotent instead of stacking duplicates.
        let _ = netsh(&["advfirewall", "firewall", "delete", "rule", &format!("name={name}")]);
        let added = netsh(&[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={name}"),
            "dir=in",
            "action=allow",
            &format!("protocol={proto}"),
            &format!("localport={ports}"),
            "enable=yes",
            "profile=any",
        ]);
        match added {
            Ok(true) => {
                info!("[SUNSHINE] Firewall rule '{name}' added");
                false
            }
            Ok(false) => true,
            Err(e) => {
                warn!("[SUNSHINE] netsh failed for '{name}': {e}");
                true
            }
        }
    });

    if unelevated_failed {
        info!("[SUNSHINE] Firewall rules need administrator rights — asking for elevation");
        add_firewall_rules_elevated()?;
    }

    // Ask the firewall rather than trusting the exit codes: this is the one
    // place that can promise "set up", and a wrong yes here is what put the
    // user back to a Deck that connects to nothing.
    FIREWALL_STATE.store(0, Ordering::Relaxed);
    if !firewall_allows_sunshine() {
        return Err("Windows Firewall still isn't allowing remote play.".to_string());
    }
    borrow_db_mut_checked().settings.streaming_firewall_configured = true;
    Ok(())
}

/// Run one netsh command, hidden. `Ok(true)` when it exited zero.
#[cfg(target_os = "windows")]
fn netsh(args: &[&str]) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("netsh")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .map_err(|e| e.to_string())
}

/// Add both rules from a single elevated PowerShell. The UAC prompt IS the
/// consent, and declining it makes `Start-Process` throw, so the outer shell
/// exits nonzero and the user gets told rather than silently left unreachable.
#[cfg(target_os = "windows")]
fn add_firewall_rules_elevated() -> Result<(), String> {
    let inner = SUNSHINE_FIREWALL_RULES
        .iter()
        .map(|(name, proto, ports)| {
            format!(
                "netsh advfirewall firewall delete rule name='{name}' | Out-Null; \
                 netsh advfirewall firewall add rule name='{name}' dir=in action=allow \
                 protocol={proto} localport={ports} enable=yes profile=any | Out-Null"
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    let encoded = {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let utf16: Vec<u8> = inner.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        STANDARD.encode(utf16)
    };
    let outer = format!(
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden \
         -ArgumentList '-NoProfile','-EncodedCommand','{encoded}'"
    );

    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &outer])
        .status()
        .map_err(|e| format!("Failed to start elevated PowerShell: {e}"))?;
    if !status.success() {
        return Err(
            "Windows Firewall wasn't opened for remote play. The administrator prompt may have \
             been declined."
                .to_string(),
        );
    }
    info!("[SUNSHINE] Firewall rules added with elevation");
    Ok(())
}

/// Can anything reach Sunshine from outside this PC?
///
/// Checked before a stream so a device on the other side of a closed firewall
/// is told why instead of waiting out a 60s timeout. Three questions, cheapest
/// first, and any one "yes" is enough:
///
/// 1. Are Drop's own rules there? Literal strings Drop wrote, so a localized
///    Windows cannot fool the match.
/// 2. Is the firewall even switched on? `Get-NetFirewallProfile` returns
///    booleans rather than translated words.
/// 3. Did somebody else already let Sunshine through?
///
/// Question 3 is the one that stops this refusing a setup that works. Drop's
/// rule names are far from the usual way inbound traffic gets permitted: the
/// Windows first-run "Allow sunshine.exe on private networks?" dialog, the
/// Sunshine installer's own rules and any hand-made rule all leave something
/// Drop did not name, and on that evidence alone the first two questions say
/// "blocked" for a PC that has been streaming happily for months.
///
/// Anything it can't determine still counts as allowed. Blocking a stream that
/// would have worked is worse than the timeout this exists to avoid.
#[cfg(target_os = "windows")]
fn firewall_allows_sunshine() -> bool {
    match FIREWALL_STATE.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }

    let allowed = drop_firewall_rules_present().unwrap_or(true)
        || !any_firewall_profile_enabled()
        || other_rule_allows_sunshine().unwrap_or(true);
    FIREWALL_STATE.store(if allowed { 1 } else { 2 }, Ordering::Relaxed);
    allowed
}

/// Is there an inbound allow rule for Sunshine that Drop did not write?
///
/// `Some(false)` is the only answer that refuses a stream, so this errs towards
/// `None` (unknown, which the caller reads as allowed) whenever the firewall
/// cannot be asked properly.
///
/// PowerShell rather than `netsh advfirewall show rule`: netsh prints
/// translated field names and one flat block of text, while the cmdlets return
/// typed fields. Two questions, both scoped so Windows does the filtering:
///
/// * an enabled inbound allow rule naming this exact `sunshine.exe`, which is
///   what the first-run dialog leaves behind, and
/// * an enabled inbound allow rule that explicitly opens the GameStream port.
///
/// A program rule reports `LocalPort = Any`, which is no evidence about
/// Sunshine at all, so only a rule naming the port itself counts for the second
/// question. Otherwise every browser on the machine would answer it.
#[cfg(target_os = "windows")]
fn other_rule_allows_sunshine() -> Option<bool> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// `@EXE@` and `@PORT@` are substituted below. The whole thing goes to
    /// PowerShell as a single `-Command` argument, so no shell ever sees it and
    /// the only quoting that matters is PowerShell's own.
    const QUERY: &str = concat!(
        "$ErrorActionPreference='Stop'; $answer='unknown'; $exe='@EXE@'; $port=@PORT@; ",
        "try { ",
        "$hit = @(Get-NetFirewallApplicationFilter -Program $exe -ErrorAction SilentlyContinue ",
        "| Get-NetFirewallRule -ErrorAction SilentlyContinue ",
        "| Where-Object { $_.Enabled -eq 'True' -and $_.Direction -eq 'Inbound' -and $_.Action -eq 'Allow' }).Count; ",
        "if ($hit -eq 0) { ",
        "$hit = @(Get-NetFirewallPortFilter | Where-Object { $m=$false; ",
        "foreach ($v in @($_.LocalPort)) { ",
        "if ($v -match '^\\d+$' -and [int]$v -eq $port) { $m=$true } ",
        "elseif ($v -match '^(\\d+)-(\\d+)$' -and [int]$Matches[1] -le $port -and $port -le [int]$Matches[2]) { $m=$true } ",
        "}; $m } ",
        "| Get-NetFirewallRule -ErrorAction SilentlyContinue ",
        "| Where-Object { $_.Enabled -eq 'True' -and $_.Direction -eq 'Inbound' -and $_.Action -eq 'Allow' }).Count ",
        "}; ",
        "if ($hit -gt 0) { $answer='allowed' } else { $answer='blocked' } ",
        "} catch { $answer='unknown' }; $answer",
    );

    // No binary means nothing to ask about, and `host_can_serve` has already
    // refused for a better reason by the time this could matter.
    let exe = find_sunshine()?;
    let script = QUERY
        .replace("@EXE@", &exe.to_string_lossy().replace('\'', "''"))
        .replace("@PORT@", &SUNSHINE_BASE_PORT.to_string());

    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    match String::from_utf8_lossy(&out.stdout).trim() {
        "allowed" => {
            info!("[SUNSHINE] Inbound traffic is already allowed by a rule Drop didn't write");
            Some(true)
        }
        "blocked" => Some(false),
        other => {
            warn!("[SUNSHINE] Could not tell whether the firewall allows remote play: {other}");
            None
        }
    }
}

/// `Some(true)` when both of Drop's rules exist, `None` when netsh couldn't be
/// asked. `show rule` needs no elevation; it exits zero either way and says
/// "No rules match the specified criteria" when there is nothing, so the rule
/// name in the output is the signal.
#[cfg(target_os = "windows")]
fn drop_firewall_rules_present() -> Option<bool> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut all_present = true;
    for (name, _, _) in SUNSHINE_FIREWALL_RULES {
        let out = Command::new("netsh")
            .args(["advfirewall", "firewall", "show", "rule"])
            .arg(format!("name={name}"))
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !String::from_utf8_lossy(&out.stdout).contains(name) {
            all_present = false;
        }
    }
    Some(all_present)
}

/// Is Windows Firewall switched on for any profile? Unknown counts as on.
#[cfg(target_os = "windows")]
fn any_firewall_profile_enabled() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-NetFirewallProfile).Enabled -contains $true",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        Ok(o) => !String::from_utf8_lossy(&o.stdout)
            .trim()
            .eq_ignore_ascii_case("false"),
        Err(_) => true,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_sunshine_firewall() -> Result<(), String> {
    // Nothing to open: Linux and macOS hosts don't get a firewall Drop can
    // reason about, and pretending otherwise would put a dead button in the UI.
    borrow_db_mut_checked().settings.streaming_firewall_configured = true;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn firewall_allows_sunshine() -> bool {
    true
}

/// Open Windows Firewall for remote play, prompting for administrator rights.
///
/// Run once as part of setup rather than on every stream — the old code shelled
/// netsh unelevated at every single start and only logged the failure, which is
/// why the user's log is full of it. The UI calls this to retry after a declined
/// prompt.
#[tauri::command]
pub async fn sunshine_configure_firewall() -> Result<(), String> {
    tokio::task::spawn_blocking(ensure_sunshine_firewall)
        .await
        .map_err(|e| format!("Firewall task failed: {e}"))?
}

/// What the settings page needs to show about the firewall.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FirewallStatus {
    /// False on platforms where Drop has no firewall to configure.
    pub supported: bool,
    /// Drop has successfully added the rules at some point on this PC.
    pub configured: bool,
    /// Nothing is standing between another device and Sunshine right now.
    pub allowed: bool,
}

/// Whether remote play can be reached from other devices.
///
/// Always asks Windows again rather than reading the cached answer. This is the
/// call behind a status row somebody is looking at, and a user who fixed the
/// firewall outside Drop would otherwise be told it is still shut until Drop
/// restarts.
#[tauri::command]
pub async fn sunshine_firewall_status() -> FirewallStatus {
    #[cfg(target_os = "windows")]
    FIREWALL_STATE.store(0, Ordering::Relaxed);
    let configured = borrow_db_checked().settings.streaming_firewall_configured;
    let allowed = tokio::task::spawn_blocking(firewall_allows_sunshine)
        .await
        .unwrap_or(true);
    FirewallStatus {
        supported: cfg!(target_os = "windows"),
        configured,
        allowed,
    }
}

/// Wait until Sunshine's HTTP port is actually accepting connections.
///
/// First-run Sunshine generates certificates and can take several seconds to
/// bind — longer than a fixed sleep — so the session is marked Ready (and the
/// client told to connect) only once the port answers, up to a ~10s timeout.
/// Returns `true` once the port answers, `false` if it never does within the
/// timeout.
async fn wait_for_sunshine_ready() -> bool {
    for _ in 0..20 {
        if sunshine_port_open() {
            info!("[SUNSHINE] HTTP port {SUNSHINE_BASE_PORT} ready");
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    warn!("[SUNSHINE] HTTP port {SUNSHINE_BASE_PORT} not ready after ~10s");
    false
}

/// Is anything at all listening on Sunshine's HTTP port? Says nothing about
/// *whose* Sunshine it is — only one process can hold the GameStream ports.
fn sunshine_port_open() -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr = SocketAddr::from(([127, 0, 0, 1], SUNSHINE_BASE_PORT));
    TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_ok()
}

/// Append `key = value` to a sunshine.conf fragment, skipping empty values.
///
/// Sunshine treats a present-but-empty key as an explicit "" rather than as
/// absent, and an empty `output_name` or `audio_sink` fails to match any
/// device — worse than never writing the key at all, because the automatic
/// selection Sunshine would otherwise do never runs.
fn push_conf_setting(out: &mut String, key: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(value);
    out.push('\n');
}

/// The display/adapter half of the generated conf.
///
/// `output_name` on its own is the wrong-monitor fix — Sunshine's own help for
/// it reads "Manually specify a display device id to use for capture. If unset,
/// the primary display is captured." No `dd_*` key is needed to make the choice
/// stick.
///
/// The `dd_*` keys are only here for host-side resolution switching, and they
/// are deliberately kept to the settings that cannot strand the desktop:
///
/// * `ensure_active` turns the chosen display on if it is off and otherwise
///   leaves the layout alone. `ensure_only_display` would deactivate every other
///   monitor, and since the revert only runs on a clean Sunshine shutdown that
///   can leave the user staring at one screen with no way back but Win+P.
/// * `dd_resolution_option = auto` lets Sunshine match the host mode to what the
///   client asked for. It is inert unless the client sends the game-optimization
///   flag, which `launch_moonlight` sets only when the user has actually chosen
///   a fixed resolution — so a default install still never has its desktop
///   touched.
/// * `dd_config_revert_on_disconnect = enabled` starts the revert the moment the
///   client drops. Sunshine's shipped default defers it to "app close or last
///   session termination", which for Drop means it may never run at all.
///
/// The `dd_*` family only exists in Sunshine's Windows build, so `windows_host`
/// keeps them out of a Linux host's config rather than leaving keys there that
/// nothing reads.
fn display_conf_block(display: &str, adapter: &str, windows_host: bool) -> String {
    let mut out = String::new();
    push_conf_setting(&mut out, "output_name", display);
    push_conf_setting(&mut out, "adapter_name", adapter);
    if windows_host {
        push_conf_setting(
            &mut out,
            "dd_configuration_option",
            if display.trim().is_empty() {
                // Nothing to switch on, and `disabled` would make the resolution
                // option below inert even when the user has asked for it.
                "verify_only"
            } else {
                "ensure_active"
            },
        );
        push_conf_setting(&mut out, "dd_resolution_option", "auto");
        push_conf_setting(&mut out, "dd_config_revert_on_disconnect", "enabled");
    }
    out
}

/// The audio half of the generated conf.
///
/// `virtual_sink` is the fix for sound coming out of the host's speakers
/// instead of the client. Sunshine's default capture is a WASAPI loopback of
/// the current default endpoint, which does not mute the host — the PC keeps
/// playing the game audio into whatever room it is in. Pointing `virtual_sink`
/// at a virtual device makes Sunshine switch the default render endpoint to it
/// for the duration of the stream and restore the real one afterwards, so the
/// host goes quiet and the audio follows the client.
fn audio_conf_block(audio_sink: &str, virtual_sink: &str, windows_host: bool) -> String {
    let mut out = String::new();
    push_conf_setting(&mut out, "audio_sink", audio_sink);
    push_conf_setting(&mut out, "virtual_sink", virtual_sink);
    if windows_host {
        // Lets Sunshine install Steam's Streaming Speakers driver if it is
        // missing, which is what supplies the virtual sink in the first place.
        // Windows-only in Sunshine.
        push_conf_setting(&mut out, "install_steam_audio_drivers", "enabled");
    }
    out
}

/// The `virtual_sink` value to write: whatever the user pinned, or Steam's
/// virtual sink if one is installed.
///
/// Auto-detection reads the endpoint's real name rather than assuming one.
/// Steam registers it with an output-form prefix — `Speakers (Steam Streaming
/// Speakers)` on the machine this was built against — and Sunshine matches the
/// name literally, so the short form would be a silent no-op.
fn resolve_virtual_sink(configured: &str) -> String {
    if !configured.trim().is_empty() {
        return configured.trim().to_string();
    }
    match crate::host_devices::find_steam_virtual_sink() {
        Some(name) => {
            info!("[SUNSHINE] Using auto-detected virtual audio sink '{name}'");
            name
        }
        None => {
            info!(
                "[SUNSHINE] No Steam virtual audio sink found — leaving virtual_sink unset, so \
                 host speakers will keep playing during a stream"
            );
            String::new()
        }
    }
}

/// List the displays this PC could stream, for the settings picker.
///
/// Runs off the main thread: it shells out to `dxgi-info.exe` and walks
/// SetupAPI, which together take long enough to stutter the UI.
#[tauri::command]
pub async fn sunshine_list_displays() -> Result<Vec<crate::host_devices::DisplayEntry>, String> {
    let tool = sunshine_dir().join("tools").join("dxgi-info.exe");
    // Older installs were extracted flat, before Stage 1 started stripping the
    // archive's `Sunshine/` wrapper, so the tool can still sit in the root.
    let tool = if tool.exists() {
        tool
    } else {
        sunshine_dir().join("dxgi-info.exe")
    };
    let displays = tokio::task::spawn_blocking(move || crate::host_devices::list_displays(&tool))
        .await
        .map_err(|e| format!("Display enumeration task failed: {e}"))??;
    info!("[DISPLAY] Found {} capturable display(s)", displays.len());
    Ok(displays)
}

/// List this PC's audio outputs, for the settings picker.
#[tauri::command]
pub async fn sunshine_list_audio_sinks() -> Result<Vec<crate::host_devices::AudioSinkEntry>, String>
{
    let sinks = tokio::task::spawn_blocking(crate::host_devices::list_audio_sinks)
        .await
        .map_err(|e| format!("Audio enumeration task failed: {e}"))??;
    info!("[AUDIO] Found {} render endpoint(s)", sinks.len());
    Ok(sinks)
}

/// Has the user pinned any of the host capture settings that only reach
/// Sunshine through Drop's generated conf?
fn host_device_settings_chosen() -> bool {
    let db = borrow_db_checked();
    [
        &db.settings.streaming_display,
        &db.settings.streaming_adapter,
        &db.settings.streaming_audio_sink,
        &db.settings.streaming_virtual_sink,
    ]
    .iter()
    .any(|v| !v.trim().is_empty())
}

/// Is a Sunshine that Drop did not start holding the GameStream ports?
///
/// Only one process can bind them, so when the port answers and Drop has no
/// child of its own, everything below is running on somebody else's config file.
/// Drop only ever writes the capture display and audio routing into the conf it
/// generates for its *own* child, so in that state the pickers save happily and
/// change nothing. The settings pages ask for this so they can say so instead of
/// flashing "Saved." over a no-op.
#[tauri::command]
pub async fn sunshine_is_foreign() -> bool {
    {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        let ours_alive = guard
            .as_mut()
            .is_some_and(|child| child.try_wait().is_ok_and(|status| status.is_none()));
        if ours_alive {
            return false;
        }
    }
    // A 300ms blocking connect, so keep it off the async worker.
    tokio::task::spawn_blocking(sunshine_port_open)
        .await
        .unwrap_or(false)
}

/// Generate sunshine.conf with Drop-specific settings.
fn generate_sunshine_conf(config_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;

    let conf_path = config_dir.join("sunshine.conf");
    let apps_path = config_dir.join("apps.json");
    let credentials_path = config_dir.join("credentials.json");
    let state_path = config_dir.join("state.json");

    // The CA keypair every Moonlight pairing is signed against. Sunshine's
    // defaults for these are *relative*, so where they land depends on the
    // working directory Sunshine happens to be started from — which for Drop is
    // the install directory that Repair wipes. Pin them next to the rest of the
    // config so a reinstall can't un-pair every device the user owns.
    let cert_dir = config_dir.join("credentials");
    std::fs::create_dir_all(&cert_dir)
        .map_err(|e| format!("Failed to create credentials dir: {e}"))?;
    let pkey_path = cert_dir.join("cakey.pem");
    let cert_path = cert_dir.join("cacert.pem");

    let (display, adapter, audio_sink, configured_virtual_sink) = {
        let db = borrow_db_checked();
        (
            db.settings.streaming_display.clone(),
            db.settings.streaming_adapter.clone(),
            db.settings.streaming_audio_sink.clone(),
            db.settings.streaming_virtual_sink.clone(),
        )
    };
    let virtual_sink = resolve_virtual_sink(&configured_virtual_sink);
    let display_block = display_conf_block(&display, &adapter, cfg!(windows));
    let audio_block = audio_conf_block(&audio_sink, &virtual_sink, cfg!(windows));

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
pkey = {pkey}
cert = {cert}

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
{display_block}
# Audio
{audio_block}
# Streaming defaults
channels = 1
fec_percentage = 20

# Encoding — tuned by Drop for sharp, low-added-latency streams. Without these,
# Sunshine falls back to its fastest/lowest-quality encoder preset (NVENC P1),
# which looks soft. These knobs are global and benefit every quality profile;
# the profile only varies fps/bitrate (client-negotiated by Moonlight).
#   NVENC (NVIDIA): preset 1=fastest(P1)..7=slowest(P7); P5 is a large quality
#   gain for negligible added latency on a modern GPU.
nvenc_preset = 5
nvenc_twopass = quarter_res
nvenc_spatial_aq = enabled
#   AMD AMF
amd_quality = quality
amd_preanalysis = enabled
amd_vbaq = enabled
#   Intel QuickSync
qsv_preset = slower

# Logging
min_log_level = 2
"#,
        base_port = SUNSHINE_BASE_PORT,
        state = state_path.to_string_lossy().replace('\\', "/"),
        creds = credentials_path.to_string_lossy().replace('\\', "/"),
        apps = apps_path.to_string_lossy().replace('\\', "/"),
        pkey = pkey_path.to_string_lossy().replace('\\', "/"),
        cert = cert_path.to_string_lossy().replace('\\', "/"),
        display_block = display_block,
        audio_block = audio_block,
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

// ── Credentials ───────────────────────────────────────────────────────

/// The admin credentials for Drop's Sunshine, minting a password on first use.
///
/// Nothing but Drop ever signs in to this instance, so the password is
/// generated rather than asked for. It is persisted the first time so the same
/// one is reused for every later start — regenerating it would invalidate the
/// credentials file Sunshine already wrote.
fn sunshine_credentials() -> (String, String) {
    {
        let db = borrow_db_checked();
        if !db.settings.sunshine_password.is_empty()
            && !db.settings.sunshine_username.trim().is_empty()
        {
            return (
                db.settings.sunshine_username.clone(),
                db.settings.sunshine_password.clone(),
            );
        }
    }

    use rand::distr::{Alphanumeric, SampleString};
    let password = Alphanumeric.sample_string(&mut rand::rng(), 32);

    let mut db = borrow_db_mut_checked();
    if db.settings.sunshine_username.trim().is_empty() {
        db.settings.sunshine_username = "sunshine".to_string();
    }
    db.settings.sunshine_password = password.clone();
    info!("[SUNSHINE] Generated a new admin password for the local Sunshine");
    (db.settings.sunshine_username.clone(), password)
}

/// Have Sunshine write its own credentials file.
///
/// Sunshine stores the password as uppercase hex of SHA-256(password + salt)
/// with the digest bytes reversed. Reimplementing that is a great way to lock
/// Drop out of its own web UI, so shell out and let Sunshine do it. The conf
/// path must come FIRST: `config::parse` runs before Sunshine dispatches the
/// command, and that is what makes it honour our `credentials_file` instead of
/// writing one next to the exe.
fn ensure_sunshine_credentials(
    binary: &Path,
    conf_path: &Path,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let creds_path = sunshine_config_dir().join("credentials.json");

    let mut cmd = Command::new(binary);
    cmd.arg(conf_path).arg("--creds").arg(username).arg(password);
    if let Some(dir) = sunshine_working_dir(binary) {
        cmd.current_dir(dir);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run Sunshine --creds: {e}"))?;

    if !output.status.success() {
        warn!(
            "[SUNSHINE] --creds exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    if !creds_path.exists() {
        return Err(format!(
            "Sunshine could not save its login details to {}. Remote play cannot sign in.",
            creds_path.display()
        ));
    }
    info!("[SUNSHINE] Credentials written to {}", creds_path.display());
    Ok(())
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
// Paired with `register_game_app` as a complete API; not yet called from a
// path that compiles on every platform, so allow the dead-code warning.
#[allow(dead_code)]
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

/// Set when Drop is using a Sunshine it did not start. Drop must never kill a
/// process it didn't spawn, so every teardown path checks this first.
static SUNSHINE_ADOPTED: AtomicBool = AtomicBool::new(false);

// ── Why the host can't serve a stream ─────────────────────────────────
//
// Every one of these ends up on the *other* device — the Deck someone just
// pressed Play on — so they are written for that person, name this PC, and say
// what to do next. They are also what `stream-host-error` shows on the host.

/// Another install owns the GameStream ports and won't take Drop's login.
/// Nothing Drop can safely do from here: it did not start that process, so the
/// only way out is a person closing it. Names the host, like the others: read on
/// a Deck in another room, "this PC" is whichever one you last thought about.
fn stream_error_port_busy() -> String {
    format!(
        "Another copy of Sunshine is already running on {}. Close it, then try again.",
        host_name()
    )
}

/// Sunshine is missing, or unpacked badly enough that it can't serve.
fn stream_error_not_set_up() -> String {
    format!(
        "Remote play is not set up on {} yet. Open Drop on that PC and run remote play setup.",
        host_name()
    )
}

/// Sunshine is up but won't accept Drop's admin login. The instruction has to
/// match a control that exists: it is the "Reset remote play sign-in" button in
/// the troubleshooting section of Settings > Remote play, on the host.
fn stream_error_credentials() -> String {
    format!(
        "Drop could not sign in to remote play on {}. Open Drop on that PC, go to Settings, \
         Remote play, and choose \"Reset remote play sign-in\".",
        host_name()
    )
}

/// The game itself refused to start on the host.
fn stream_error_launch_failed(game_name: &str) -> String {
    format!("{game_name} would not start on {}.", host_name())
}

/// This PC's name as the rest of Drop shows it: the user's own device label
/// when they set one, the hostname otherwise.
fn host_name() -> String {
    remote::save_sync::machine_name()
}

/// A host-side refusal, in the vocabulary above. Kept as a type rather than a
/// string so the reason survives the trip out of `spawn_sunshine` without
/// anyone having to pattern-match on prose.
#[derive(Clone, Debug, PartialEq, Eq)]
enum HostFailure {
    NotSetUp,
    PortBusy,
    Credentials,
    LaunchFailed(String),
    /// Something that has no user-facing shorthand. The string is already
    /// written for the user.
    Other(String),
}

impl HostFailure {
    fn message(&self) -> String {
        match self {
            HostFailure::NotSetUp => stream_error_not_set_up(),
            HostFailure::PortBusy => stream_error_port_busy(),
            HostFailure::Credentials => stream_error_credentials(),
            HostFailure::LaunchFailed(game) => stream_error_launch_failed(game),
            HostFailure::Other(msg) => msg.clone(),
        }
    }
}

/// Tracks active host-side streaming sessions with cancellation signals.
/// Sending `true` on the watch channel signals the heartbeat loop to stop.
static ACTIVE_HOST_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, watch::Sender<bool>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Sunshine process status.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SunshineStatus {
    pub installed: bool,
    /// Installed *and* complete. False here with `installed` true means the
    /// files are damaged and only a repair will fix it.
    pub healthy: bool,
    pub running: bool,
    pub binary_path: Option<String>,
    pub web_ui_port: u16,
    pub version: String,
}

/// Get the current Sunshine status.
#[tauri::command]
pub async fn sunshine_status() -> SunshineStatus {
    info!("[SUNSHINE] sunshine_status() called");
    let probe = probe_sunshine_install();
    let installed = probe.installed();
    let healthy = probe.healthy();
    let binary_path = probe.binary.clone();
    info!(
        "[SUNSHINE] installed={installed}, healthy={healthy}, path={:?}, missing={:?}",
        binary_path.as_ref().map(|p| p.display().to_string()),
        probe.missing
    );

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
        } else if SUNSHINE_ADOPTED.load(Ordering::Relaxed) && sunshine_port_open() {
            // Not Drop's child, but Drop is using it.
            info!("[SUNSHINE] Using an adopted Sunshine started outside Drop");
            true
        } else {
            info!("[SUNSHINE] No managed Sunshine process");
            false
        }
    };

    let status = SunshineStatus {
        installed,
        healthy,
        running,
        binary_path: binary_path.map(|p| p.to_string_lossy().to_string()),
        web_ui_port: SUNSHINE_WEB_PORT,
        version: SUNSHINE_VERSION.to_string(),
    };
    info!(
        "[SUNSHINE] Returning status: installed={}, healthy={}, running={}",
        status.installed, status.healthy, status.running
    );
    status
}

/// Forward a child's stdout/stderr into Drop's log.
///
/// Both pipes have to be *drained*, not merely captured: with `Stdio::piped()`
/// and nobody reading, Sunshine blocks the moment the ~64KB pipe buffer fills,
/// which it does during startup — that is why binding the HTTP port used to
/// take three attempts. Discarding to null would unblock it too, but these
/// lines are the only diagnosis anyone gets when a stream won't start.
fn forward_child_output(child: &mut std::process::Child) {
    use std::io::{BufRead, BufReader};

    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                info!("[SUNSHINE-OUT] {line}");
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                warn!("[SUNSHINE-ERR] {line}");
            }
        });
    }
}

/// Poll a child until it exits, up to `timeout`. `true` if it exited.
///
/// Yields between polls rather than blocking the worker: this sits on the path
/// that ends a stream, and Sunshine's display teardown is worth waiting seconds
/// for.
#[cfg(not(unix))]
async fn wait_for_child_exit(
    child: &mut std::process::Child,
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            // Can't tell, so don't stall the caller waiting for an answer.
            Err(_) => return false,
            Ok(None) => {}
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

/// Best-effort: which sunshine binaries are running right now. Only used for
/// the log line that says whose Sunshine answered.
fn running_sunshine_binaries() -> Vec<String> {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut paths: Vec<String> = system
        .processes()
        .values()
        .filter(|p| {
            p.name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("sunshine")
        })
        .filter_map(|p| p.exe().map(|e| e.display().to_string()))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Start Sunshine as Drop's own child: config, credentials, spawn.
///
/// The firewall is not touched here. It used to be, on every single start, and
/// every one of those attempts failed unelevated — see `ensure_sunshine_firewall`,
/// which is now a setup step.
async fn spawn_sunshine(username: &str, password: &str) -> Result<(), HostFailure> {
    let probe = probe_sunshine_install();
    if !probe.healthy() {
        warn!(
            "[SUNSHINE] Cannot start: installed={}, missing={:?}",
            probe.installed(),
            probe.missing
        );
        return Err(HostFailure::NotSetUp);
    }
    let binary = probe.binary.ok_or(HostFailure::NotSetUp)?;
    let conf_path =
        generate_sunshine_conf(&sunshine_config_dir()).map_err(HostFailure::Other)?;
    ensure_sunshine_credentials(&binary, &conf_path, username, password)
        .map_err(|e| {
            warn!("[SUNSHINE] {e}");
            HostFailure::Credentials
        })?;

    info!(
        "[SUNSHINE] Starting: {} {}",
        binary.display(),
        conf_path.display()
    );
    let mut cmd = Command::new(&binary);
    cmd.arg(conf_path.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(dir) = sunshine_working_dir(&binary) {
        cmd.current_dir(dir);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| HostFailure::Other(format!("Sunshine would not start on {}: {e}", host_name())))?;
    info!("[SUNSHINE] Started with PID {}", child.id());
    forward_child_output(&mut child);

    {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        *guard = Some(child);
    }
    SUNSHINE_ADOPTED.store(false, Ordering::Relaxed);
    Ok(())
}

/// Make sure a Sunshine that Drop can talk to is running, and record who owns
/// it.
///
/// Only one process can hold the GameStream ports, so a port that already
/// answers means somebody else's Sunshine is up — Drop's from an earlier run,
/// or a separate install entirely. Killing it is off the table (Drop didn't
/// start it), so the only question is whether Drop can sign in to it.
async fn ensure_sunshine_running(username: &str, password: &str) -> Result<(), HostFailure> {
    // Our own child, still alive? Nothing to do.
    {
        let mut guard = SUNSHINE_PROCESS.lock().await;
        let alive = guard
            .as_mut()
            .is_some_and(|child| child.try_wait().is_ok_and(|status| status.is_none()));
        if alive {
            return Ok(());
        }
        *guard = None;
    }
    SUNSHINE_ADOPTED.store(false, Ordering::Relaxed);

    if !sunshine_port_open() {
        return spawn_sunshine(username, password).await;
    }

    let running = running_sunshine_binaries();
    for path in &running {
        info!("[SUNSHINE] Sunshine already running from {path}");
    }
    match sunshine_api_request(reqwest::Method::GET, "/apps", None, username, password).await {
        Ok(_) => {
            SUNSHINE_ADOPTED.store(true, Ordering::Relaxed);
            info!("[SUNSHINE] Adopted the Sunshine already running on this PC — Drop will not stop it");
            // An adopted Sunshine reads its own config file, so Drop's generated
            // conf is never involved and none of the host device settings are in
            // effect. Say so loudly; the settings pages warn about the same thing
            // via `sunshine_is_foreign`.
            if host_device_settings_chosen() {
                warn!(
                    "[SUNSHINE] The capture display and audio settings are NOT in effect: this \
                     Sunshine was started outside Drop and is using its own configuration"
                );
            }
            Ok(())
        }
        Err(e) => {
            warn!("[SUNSHINE] Cannot sign in to the Sunshine holding port {SUNSHINE_BASE_PORT}: {e}");
            // Whose Sunshine is it? One from Drop's own install dir — an
            // orphan of an earlier run — means the password on disk and the
            // one in settings have drifted apart, which the user fixes by
            // resetting the credentials. Any other path is a separate install
            // that Drop must not touch at all.
            let install_dir = sunshine_dir();
            if running.iter().any(|p| Path::new(p).starts_with(&install_dir)) {
                Err(HostFailure::Credentials)
            } else {
                Err(HostFailure::PortBusy)
            }
        }
    }
}

/// Start Sunshine with Drop's config, or adopt one that is already running.
/// Returns the web UI URL.
#[tauri::command]
pub async fn start_sunshine() -> Result<String, String> {
    let (username, password) = sunshine_credentials();
    ensure_sunshine_running(&username, &password)
        .await
        .map_err(|f| f.message())?;
    wait_for_sunshine_ready().await;
    Ok(format!("https://localhost:{}", SUNSHINE_WEB_PORT))
}

/// Stop the running Sunshine process.
#[tauri::command]
pub async fn stop_sunshine() -> Result<(), String> {
    if SUNSHINE_ADOPTED.swap(false, Ordering::Relaxed) {
        info!("[SUNSHINE] Leaving the adopted Sunshine running — Drop didn't start it");
        return Ok(());
    }

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

        // Windows has no SIGTERM, and `Child::kill` is TerminateProcess: the
        // process dies where it stands, so Sunshine never runs the teardown that
        // puts the user's display configuration back. `taskkill` without `/F`
        // asks politely first (WM_CLOSE to the process' windows), which is the
        // closest equivalent, and the hard kill stays as the fallback.
        #[cfg(not(unix))]
        {
            let pid = child.id();
            let mut ask = Command::new("taskkill");
            ask.args(["/PID", &pid.to_string(), "/T"]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                ask.creation_flags(CREATE_NO_WINDOW);
            }
            // taskkill refuses politely ("can only be terminated forcefully")
            // when the target has no window to close, so there is nothing to
            // wait for in that case.
            let asked = ask.output().map(|o| o.status.success()).unwrap_or(false);
            let exited = asked
                && wait_for_child_exit(&mut child, std::time::Duration::from_secs(5)).await;
            if !exited {
                warn!("[SUNSHINE] PID {pid} didn't close on request, terminating it");
                let _ = child.kill();
            }
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

/// Did a `sunshine_api_request` error come back as a rejected login?
///
/// The error is already flattened to a string by the time callers see it, and
/// only the status line is reliable — Sunshine's body for a 401 is a bare HTML
/// page in some builds and JSON in others.
fn is_auth_failure(error: &str) -> bool {
    error.contains("401") || error.to_ascii_lowercase().contains("unauthorized")
}

/// Send a PIN to Sunshine for Moonlight pairing.
#[tauri::command]
pub async fn sunshine_send_pin(pin: String, client_name: String) -> Result<bool, String> {
    let (username, password) = sunshine_credentials();
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
pub async fn sunshine_list_apps() -> Result<serde_json::Value, String> {
    let (username, password) = sunshine_credentials();
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
) -> Result<(), String> {
    let (username, password) = sunshine_credentials();
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

/// A Moonlight device that has paired with this PC.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SunshinePairedClient {
    /// Sunshine's own id for the pairing, and what `sunshine_unpair_client`
    /// takes.
    pub uuid: String,
    /// The name the device gave when it paired. Sunshine allows this to be
    /// blank, so a placeholder stands in rather than an empty row.
    pub name: String,
}

/// The paired devices, ready to render.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SunshineClientList {
    pub count: usize,
    pub clients: Vec<SunshinePairedClient>,
}

/// List the Moonlight devices paired with this PC.
///
/// Sunshine answers `{ "status": true, "named_certs": [{ "name", "uuid", ... }] }`
/// (verified against the web UI bundled in this install: `api/clients/list`).
/// The raw shape isn't much use to a UI, so it comes back flattened, and a
/// missing `named_certs` — which is what Sunshine sends when nothing is paired —
/// is an empty list rather than an error.
#[tauri::command]
pub async fn sunshine_list_clients() -> Result<SunshineClientList, String> {
    let (username, password) = sunshine_credentials();
    let raw = sunshine_api_request(
        reqwest::Method::GET,
        "/clients/list",
        None,
        &username,
        &password,
    )
    .await?;

    let clients = parse_paired_clients(&raw);
    info!("[SUNSHINE] {} paired client(s)", clients.len());
    Ok(SunshineClientList {
        count: clients.len(),
        clients,
    })
}

/// Pull the paired devices out of Sunshine's `/clients/list` response.
fn parse_paired_clients(raw: &serde_json::Value) -> Vec<SunshinePairedClient> {
    raw.get("named_certs")
        .and_then(|c| c.as_array())
        .map(|certs| {
            certs
                .iter()
                .filter_map(|cert| {
                    let uuid = cert.get("uuid")?.as_str()?.trim();
                    if uuid.is_empty() {
                        // Nothing can be unpaired without one, so it would only
                        // ever be a row that does nothing.
                        return None;
                    }
                    let name = cert
                        .get("name")
                        .and_then(|n| n.as_str())
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .unwrap_or("Unnamed device");
                    Some(SunshinePairedClient {
                        uuid: uuid.to_string(),
                        name: name.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Unpair one Moonlight device.
///
/// `POST /api/clients/unpair` with `{ "uuid": ... }`, the same call the bundled
/// Sunshine web UI makes from its own client list. The device has to pair again
/// with a fresh PIN afterwards.
#[tauri::command]
pub async fn sunshine_unpair_client(uuid: String) -> Result<(), String> {
    if uuid.trim().is_empty() {
        return Err("No device was selected to unpair.".to_string());
    }
    let (username, password) = sunshine_credentials();
    let result = sunshine_api_request(
        reqwest::Method::POST,
        "/clients/unpair",
        Some(serde_json::json!({ "uuid": uuid })),
        &username,
        &password,
    )
    .await?;

    // Sunshine answers 200 with `status: false` when it didn't recognise the
    // uuid, so the HTTP status alone would report a success that never happened.
    if result.get("status").and_then(|s| s.as_bool()) == Some(false) {
        return Err("Sunshine did not unpair that device.".to_string());
    }
    info!("[SUNSHINE] Unpaired client {uuid}");
    Ok(())
}

/// Forget Sunshine's admin password so the next start mints a new one.
///
/// The way out of `stream_error_credentials`: Drop generates the password and
/// Sunshine stores a hash of it, so once those two drift apart — a half-finished
/// install, a credentials file restored from elsewhere — every API call 401s
/// forever and no amount of retrying helps.
///
/// Reached from the "Reset remote play sign-in" button in the troubleshooting
/// section of Settings > Remote play, which is the action that message names.
#[tauri::command]
pub async fn sunshine_reset_credentials() -> Result<(), String> {
    // Stopping Sunshine takes every stream this PC is hosting down with it, and
    // the person clicking the button cannot see the other rooms. Now that this
    // has a button in Settings, refusing is the only safe answer.
    {
        let held = ACTIVE_HOST_SESSIONS.lock().await.len();
        if held == 1 {
            return Err(
                "A device is streaming from this PC right now. Stop that stream first."
                    .to_string(),
            );
        }
        if held > 1 {
            return Err(format!(
                "{held} devices are streaming from this PC right now. Stop those streams first."
            ));
        }
    }
    // Sunshine writes the new credentials file on its next start, and it only
    // reads that file at startup, so the running one has to go first.
    let _ = stop_sunshine().await;
    {
        let mut db = borrow_db_mut_checked();
        db.settings.sunshine_password = String::new();
    }
    info!("[SUNSHINE] Admin password cleared — a new one is generated on the next start");
    Ok(())
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

/// Let go of Sunshine if nothing else is streaming.
///
/// Takes the locked session map so the check and the shutdown can't race a
/// session that registers in between. Callers must have already removed their
/// own entry: whoever empties the map is the last holder and owns the teardown.
/// Goes through `stop_sunshine`, which leaves an adopted foreign Sunshine alone.
async fn stop_sunshine_if_last_holder(
    sessions: tokio::sync::MutexGuard<'_, HashMap<String, watch::Sender<bool>>>,
) {
    if !sessions.is_empty() {
        return;
    }
    drop(sessions); // Release the lock before stopping Sunshine.
    info!("[STREAMING] No more active sessions, stopping Sunshine");
    let _ = stop_sunshine().await;
}

/// Stop the streaming sessions *this PC is hosting* and release Sunshine.
///
/// Only ever touches sessions in `ACTIVE_HOST_SESSIONS`, which is this host's
/// own map. The server's session list is not the right input here: everyone in
/// the household shares a user, so "stop everything the server knows about"
/// reaches across and kills somebody else's live stream.
///
/// When this host holds nothing, Sunshine is left running. An empty map means
/// Drop is not the reason it is up — an adopted install, or a leftover from
/// before a Drop restart that may well be mid-stream.
#[tauri::command]
pub async fn stop_all_host_sessions() -> Result<u32, String> {
    let mut sessions = ACTIVE_HOST_SESSIONS.lock().await;
    let count = sessions.len() as u32;
    if count == 0 {
        info!("[STREAMING] No host sessions on this PC — leaving Sunshine alone");
        return Ok(0);
    }
    info!("[STREAMING] Stopping {} active host session(s)", count);
    for (sid, tx) in sessions.drain() {
        info!("[STREAMING] Cancelling host session {}", sid);
        // The heartbeat loop stops the server session and kills the game; it
        // finds its entry already gone and skips its own teardown, which this
        // call is doing instead.
        let _ = tx.send(true);
    }
    // Sunshine reverts any display change it made when the session ends, so
    // there is nothing for Drop to restore here.
    stop_sunshine_if_last_holder(sessions).await;
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
        if let Ok(output) = Command::new(name).arg("--version").output()
            && (output.status.success() || !output.stdout.is_empty())
        {
            return Some(PathBuf::from(name));
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
            && output.status.success()
        {
            // Return a sentinel — we'll launch via flatpak run
            return Some(PathBuf::from("flatpak:com.moonlight_stream.Moonlight"));
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
            for entry in std::fs::read_dir(&install_dir).map_err(|e| format!("{e}"))?.flatten() {
                let candidate = entry.path().join("Moonlight.exe");
                if candidate.exists() {
                    info!("[MOONLIGHT] Installed to {}", candidate.display());
                    return Ok(candidate);
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

/// Cancel signal for the Moonlight session watcher.
/// When a stream starts, we spawn a background task that polls the session
/// and kills Moonlight when the session ends.  This lives in Rust so it
/// works even if the Vue page navigates away or unmounts.
static MOONLIGHT_WATCHER_CANCEL: std::sync::LazyLock<Mutex<Option<watch::Sender<bool>>>> =
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

    // Cancel the session watcher (if running) so it doesn't try to double-kill
    {
        let mut guard = MOONLIGHT_WATCHER_CANCEL.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(true);
        }
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
    _app_name: Option<String>,
    client_resolution: Option<String>,
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

    // Launch Moonlight in stream mode.
    // Always stream "Desktop" because Drop launches the game independently
    // (via fulfill_stream_request step 7).  Sunshine's per-app entries would
    // try to launch the game a second time, causing conflicts.  The game is
    // already running on the PC desktop — Moonlight just captures the screen.
    // Apply the user's quality preset (fps + bitrate) and resolution, read from
    // settings so they persist across streams. Moonlight takes bitrate in Kbps.
    // Resolution is its own setting so it can be raised when docked to a TV; the
    // "native" option omits `--resolution` and lets Moonlight use its default.
    let (qfps, qbitrate, hdr) = {
        let db = borrow_db_checked();
        let (fps, bitrate) =
            StreamQuality::from_setting(&db.settings.streaming_quality).params();
        (fps, bitrate, db.settings.streaming_hdr)
    };
    let resolution = resolve_stream_resolution(client_resolution.as_deref());
    let fps_str = qfps.to_string();
    let bitrate_str = qbitrate.to_string();
    info!(
        "[MOONLIGHT] Starting stream to {} (Desktop capture, {} @ {}fps, {}kbps, hdr={}, host mode change={})...",
        address,
        resolution
            .request
            .map(|(w, h)| format!("{w}x{h}"))
            .unwrap_or_else(|| "native".to_string()),
        qfps,
        qbitrate,
        hdr,
        resolution.change_host_mode
    );
    let mut args: Vec<String> = vec!["stream".to_string()];
    if let Some((w, h)) = resolution.request {
        args.push("--resolution".to_string());
        args.push(format!("{w}x{h}"));
    }
    args.push("--fps".to_string());
    args.push(fps_str);
    args.push("--bitrate".to_string());
    args.push(bitrate_str);
    if hdr {
        args.push("--hdr".to_string());
    }
    // "Optimize game settings" is what puts sops=1 in the launch request, and
    // Sunshine ignores dd_resolution_option without it — it even logs
    // "Sunshine is configured to change resolution automatically, but the
    // "Optimize game settings" is not set in the client! Resolution will not be
    // changed." Moonlight otherwise inherits whatever the user last left in
    // their own profile, so state it either way rather than letting the host's
    // behaviour hinge on a checkbox Drop cannot see. Both spellings are real:
    // moonlight-qt's addToggleOption registers "--x" and "--no-x" as a pair
    // (verified against the v6.1.0 source install_moonlight downloads).
    args.push(
        if resolution.change_host_mode {
            "--game-optimization"
        } else {
            "--no-game-optimization"
        }
        .to_string(),
    );
    args.push(address.clone());
    args.push("Desktop".to_string());
    let mut cmd = moonlight_command(&moonlight_str);
    cmd.args(&args);

    let child = cmd.spawn()
        .map_err(|e| format!("Failed to launch Moonlight: {e}"))?;

    info!("[MOONLIGHT] Moonlight launched (PID {})", child.id());
    {
        let mut guard = MOONLIGHT_PROCESS.lock().await;
        *guard = Some(child);
    }

    Ok(())
}

/// Start a background watcher that polls the session status and kills Moonlight
/// when the session ends.  This is the **authoritative** kill mechanism — it
/// lives in Rust and keeps running even if the Vue component unmounts.
#[tauri::command]
pub async fn watch_moonlight_session(session_id: String) -> Result<(), String> {
    // Cancel any previous watcher
    {
        let mut guard = MOONLIGHT_WATCHER_CANCEL.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(true);
        }
    }

    let (cancel_tx, cancel_rx) = watch::channel(false);
    {
        let mut guard = MOONLIGHT_WATCHER_CANCEL.lock().await;
        *guard = Some(cancel_tx);
    }

    info!("[MOONLIGHT-WATCHER] Starting session watcher for {}", session_id);
    let sid = session_id.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            // Check if we were cancelled (e.g. manual kill_moonlight)
            if *cancel_rx.borrow() {
                info!("[MOONLIGHT-WATCHER] Watcher cancelled for {}", sid);
                break;
            }

            // Poll the session from the server
            match streaming_sessions::list_streaming_sessions().await {
                Ok(sessions) => {
                    let found = sessions.iter().find(|s| s.id == sid);
                    match found {
                        None => {
                            // Session gone (filtered out because status is Stopped)
                            info!("[MOONLIGHT-WATCHER] Session {} gone from server, killing Moonlight", sid);
                            let _ = kill_moonlight().await;
                            break;
                        }
                        Some(s) if s.status == "Stopped" => {
                            info!("[MOONLIGHT-WATCHER] Session {} stopped, killing Moonlight", sid);
                            let _ = kill_moonlight().await;
                            break;
                        }
                        _ => {} // Still active, keep watching
                    }
                }
                Err(e) => {
                    warn!("[MOONLIGHT-WATCHER] Failed to poll sessions: {e}");
                    // Don't break on transient errors — keep trying
                }
            }
        }
        info!("[MOONLIGHT-WATCHER] Watcher exiting for {}", sid);
        // Clean up the cancel handle
        let mut guard = MOONLIGHT_WATCHER_CANCEL.lock().await;
        *guard = None;
    });

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
/// `game_config` is the JSON-serialized UserConfiguration from this client,
/// so the host PC can apply the Deck's widescreen/quality settings during streaming.
#[tauri::command]
pub async fn streaming_request_stream(
    game_id: String,
    target_client_id: Option<String>,
    game_config: Option<String>,
) -> Result<String, String> {
    info!("[STREAMING] streaming_request_stream called: game_id={}, target={:?}, has_config={}", game_id, target_client_id, game_config.is_some());
    let session_id = streaming_sessions::request_stream(&game_id, target_client_id.as_deref(), game_config)
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
                                // Deserialize game config from the stream request if present
                                let game_cfg: Option<database::models::data::UserConfiguration> =
                                    req.game_config.as_ref().and_then(|json_str| {
                                        serde_json::from_str(json_str).ok()
                                    });
                                tokio::spawn(fulfill_stream_request(
                                    req.session_id.clone(),
                                    game_id.clone(),
                                    game_name,
                                    game_cfg,
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

/// Tell the desktop UI why a stream request could not be served.
///
/// The requesting device only ever sees the session end, and the host had no
/// voice at all on this path: `start_sunshine` surfaces its errors as a command
/// result, but a pushed request has nobody to return to. Refusing to host is
/// often the right call; refusing without saying so leaves the user with a Deck
/// that gives up for no visible reason.
async fn emit_stream_host_error(session_id: &str, game_id: &str, reason: &str) {
    use remote::utils::DROP_APP_HANDLE;
    use tauri::Emitter;

    let lock = DROP_APP_HANDLE.lock().await;
    if let Some(app) = &*lock {
        let _ = app.emit(
            "stream-host-error",
            serde_json::json!({
                "sessionId": session_id,
                "gameId": game_id,
                "reason": reason,
            }),
        );
    }
}

/// Give up on a stream request, out loud.
///
/// Both halves matter. `stream-host-error` tells whoever is sitting at this PC,
/// and stopping the session *with the reason attached* is what reaches the
/// device that pressed Play: it sees Stopped on its next poll, with words, in
/// seconds. Leaving the session open instead means the requester waits out its
/// own 60s timeout and blames the network, and the server's stale sweep does
/// not clear the row for five minutes.
async fn fail_stream(session_id: &str, game_id: &str, reason: &str) {
    warn!("[STREAM-FULFILL] Session {session_id} cannot be served: {reason}");
    emit_stream_host_error(session_id, game_id, reason).await;
    if let Err(e) =
        streaming_sessions::stop_streaming_session_with_error(session_id, Some(reason)).await
    {
        warn!("[STREAM-FULFILL] Could not stop session {session_id} after failing it: {e}");
    }
}

/// Hand Sunshine back after a stream that never got as far as the heartbeat
/// loop, which is what normally owns this. Drops this request's own reservation
/// first: it is in the map from before Sunshine was started, so leaving it there
/// would make the last-holder check see a holder that no longer exists.
///
/// Any *other* live session keeps Sunshine up, which is the whole point of going
/// through the last-holder check rather than calling `stop_sunshine` directly.
async fn release_reservation(session_id: &str) {
    let mut sessions = ACTIVE_HOST_SESSIONS.lock().await;
    sessions.remove(session_id);
    stop_sunshine_if_last_holder(sessions).await;
}

/// Can this PC host a stream at all? Checked before the request is accepted so
/// a hopeless one is refused in seconds rather than after a Sunshine start.
///
/// Neither question applies to a Sunshine that is already up. Drop will try to
/// adopt that one, which needs nothing from Drop's own copy of the files, and
/// whoever installed it registered their own firewall rules — Drop's rule names
/// say nothing about those.
fn host_can_serve() -> Result<(), HostFailure> {
    if sunshine_port_open() {
        return Ok(());
    }
    let probe = probe_sunshine_install();
    if !probe.healthy() {
        warn!(
            "[STREAM-FULFILL] Sunshine is not usable here: installed={}, missing={:?}",
            probe.installed(),
            probe.missing
        );
        return Err(HostFailure::NotSetUp);
    }
    if !firewall_allows_sunshine() {
        return Err(HostFailure::Other(STREAM_ERROR_FIREWALL.to_string()));
    }
    Ok(())
}

/// Fulfill a stream request: accept it, start Sunshine, launch the game.
/// `game_config` is the requesting client's per-game configuration (widescreen,
/// quality preset, etc.) — applied as an override on the host so the Deck's
/// settings take effect during streaming.
///
/// Every way out of here that isn't a running stream calls `fail_stream`. The
/// requesting device has no other source of truth about this PC.
async fn fulfill_stream_request(
    session_id: String,
    game_id: String,
    game_name: String,
    game_config: Option<database::models::data::UserConfiguration>,
) {
    info!("[STREAM-FULFILL] Fulfilling stream request {} for game {}", session_id, game_id);

    // 0. Refuse before accepting if this PC plainly cannot serve. The checks
    //    block (a socket connect, a PATH probe, netsh), so keep them off the
    //    async worker. An unknown answer means carry on: better to try and fail
    //    than to refuse a stream that would have worked.
    match tokio::task::spawn_blocking(host_can_serve).await {
        Ok(Ok(())) => {}
        Ok(Err(failure)) => {
            fail_stream(&session_id, &game_id, &failure.message()).await;
            return;
        }
        Err(e) => {
            warn!("[STREAM-FULFILL] Host readiness check failed to run: {e}");
        }
    }

    // 1. Sunshine's admin credentials — generated and persisted on first use.
    let (sun_user, sun_pass) = sunshine_credentials();

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
        // A failed accept usually means the request was already cancelled or
        // another device took it. The stop is then a no-op — it only matches a
        // session this client hosts — so a session that became somebody else's
        // is left alone.
        fail_stream(
            &session_id,
            &game_id,
            &format!("{} could not pick up the stream request.", host_name()),
        )
        .await;
        return;
    }

    // 4. Take a place in the active map BEFORE touching Sunshine, and hold it
    //    until the heartbeat loop takes over at step 8.
    //
    //    `stop_sunshine_if_last_holder` reads this map to decide whether anyone
    //    still needs Sunshine, and everything between here and step 8 takes
    //    real time: up to 10s waiting for the port, then a game launch that can
    //    run to tens of seconds. Registering only at the end left that whole
    //    stretch invisible, so a session ending in the middle of it — a Deck
    //    that quit ten seconds ago and whose heartbeat loop has just noticed —
    //    saw an empty map and stopped the Sunshine this request was mid-handover
    //    to, leaving it Ready with nothing listening. The reservation carries
    //    the cancel channel that loop later uses, so `stop_all_host_sessions`
    //    can still reach this session while it is being set up.
    let (cancel_tx, cancel_rx) = watch::channel(false);
    {
        let mut sessions = ACTIVE_HOST_SESSIONS.lock().await;
        sessions.insert(session_id.clone(), cancel_tx);
    }

    //    Now make sure Sunshine is running: Drop's own, or one already up that
    //    Drop can sign in to. If neither, the session is dead on arrival, so
    //    stop it rather than leave the client waiting on a host that can't serve.
    if let Err(failure) = ensure_sunshine_running(&sun_user, &sun_pass).await {
        fail_stream(&session_id, &game_id, &failure.message()).await;
        release_reservation(&session_id).await;
        return;
    }

    // Whether we just started Sunshine or it was already running, make sure it
    // is actually accepting connections before sending the pairing PIN and
    // marking the session Ready. Without this the host reports "Ready" while
    // Sunshine is still binding (or not running at all), so Moonlight on the
    // client races ahead and gets no response on :47989. If it never comes up,
    // abort the session rather than tell the client to connect to a dead host.
    if !wait_for_sunshine_ready().await {
        warn!(
            "[STREAM-FULFILL] Sunshine never started listening on port {SUNSHINE_BASE_PORT}; \
             aborting session {session_id} instead of telling the client to connect"
        );
        fail_stream(
            &session_id,
            &game_id,
            &format!("Remote play did not finish starting up on {}.", host_name()),
        )
        .await;
        release_reservation(&session_id).await;
        return;
    }

    // 5. Send the PIN to Sunshine's API for pre-pairing (Moonlight streams
    //    "Desktop" so no per-game app registration is needed)
    let pin_body = serde_json::json!({ "pin": pin, "name": "Drop Client" });
    if let Err(e) = sunshine_api_request(
        reqwest::Method::POST,
        "/pin",
        Some(pin_body),
        &sun_user,
        &sun_pass,
    ).await {
        // A rejected login is fatal — an unpaired Moonlight has nothing to
        // connect with, and every later API call would fail the same way.
        // Anything else (a Sunshine build with no /pin, a client that is
        // already paired) is not worth cancelling a stream over.
        if is_auth_failure(&e) {
            fail_stream(&session_id, &game_id, &stream_error_credentials()).await;
            release_reservation(&session_id).await;
            return;
        }
        warn!("[STREAM-FULFILL] Failed to send PIN to Sunshine (may not need pairing): {e}");
    }

    // 6. Mark the session as Ready
    if let Err(e) = streaming_sessions::mark_session_ready(&session_id, Some(&pin)).await {
        warn!("[STREAM-FULFILL] Failed to mark session ready: {e}");
        fail_stream(
            &session_id,
            &game_id,
            &format!(
                "Remote play is running on {}, but Drop could not hand the stream over.",
                host_name()
            ),
        )
        .await;
        release_reservation(&session_id).await;
        return;
    }
    info!("[STREAM-FULFILL] Session {} marked Ready", session_id);

    // 7. Launch the game (on a blocking thread — launch_game uses block_on internally)
    //    Use launch_game_streaming so save sync conflicts are auto-resolved
    //    (the conflict dialog would appear on the host PC, unreachable from the Deck).
    //    Also pass the game_config override so the Deck's quality/widescreen settings
    //    are applied on the host.
    info!("[STREAM-FULFILL] Launching game {} (streaming mode)", game_id);
    {
        use crate::process::launch_game_streaming;
        let gid = game_id.clone();
        let cfg = game_config;
        // A stream with no game in it is not a stream. The heartbeat loop below
        // would tear the session down anyway on its first "is the game running"
        // check, five seconds later and without a word about why.
        let launch_failed = match tokio::task::spawn_blocking(move || launch_game_streaming(gid, 0, cfg)).await {
            Ok(Ok(_)) => {
                info!("[STREAM-FULFILL] Game launched successfully");
                false
            }
            Ok(Err(e)) => {
                warn!("[STREAM-FULFILL] Failed to launch game: {e:?}");
                true
            }
            Err(e) => {
                warn!("[STREAM-FULFILL] Launch task panicked: {e}");
                true
            }
        };
        if launch_failed {
            fail_stream(
                &session_id,
                &game_id,
                &HostFailure::LaunchFailed(game_name).message(),
            )
            .await;
            release_reservation(&session_id).await;
            return;
        }
    }

    // 8. Start heartbeating in background. This loop is the host-side lifecycle
    //    owner: it keeps the session alive, and tears everything down — INCLUDING
    //    killing the game on this PC — when the stream ends, whichever side ends it.
    //    It takes over the reservation made at step 4 rather than registering
    //    afresh, so there is no window where this session is absent from the map.
    let sid = session_id.clone();
    let hb_game_id = game_id.clone();
    tokio::spawn(async move {
        // Whether the game is still running when the stream ends and so must be
        // killed on the host. (When the game exits on its own, it's already gone.)
        let mut kill_game_on_exit = false;
        // The server rejects heartbeats for Stopped/expired sessions (404), which
        // is exactly what happens the moment the Deck ends the stream. Require two
        // strikes (~10s) so a single transient network blip never kills a live game.
        let mut consecutive_hb_failures = 0u32;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            // (a) Host UI asked to stop hosting (stop_all_host_sessions). The game
            //     is still running here, so it needs killing too.
            if *cancel_rx.borrow() {
                info!("[STREAM-FULFILL] Session {} cancelled by host, stopping", sid);
                kill_game_on_exit = true;
                let _ = streaming_sessions::stop_streaming_session(&sid).await;
                break;
            }

            // (b) Game exited on the host on its own — just tear the session down;
            //     the Deck's watcher then closes Moonlight. Nothing to kill.
            if !process::PROCESS_MANAGER.lock().is_game_running(&hb_game_id) {
                info!("[STREAM-FULFILL] Game {} exited on host, stopping session {}", hb_game_id, sid);
                let _ = streaming_sessions::stop_streaming_session(&sid).await;
                break;
            }

            // (c) Heartbeat. A sustained failure means the client ended the
            //     stream (the Deck called stop → server 404s the heartbeat), so
            //     kill the still-running game so it exits on the host too —
            //     this is the "quit on the Deck → game closes on the PC" path.
            match streaming_sessions::heartbeat_streaming(&sid, Some("Streaming")).await {
                Ok(()) => consecutive_hb_failures = 0,
                Err(e) => {
                    consecutive_hb_failures += 1;
                    warn!(
                        "[STREAM-FULFILL] Heartbeat failed for {} ({}/2): {e}",
                        sid, consecutive_hb_failures
                    );
                    if consecutive_hb_failures >= 2 {
                        info!(
                            "[STREAM-FULFILL] Session {} ended from the client side — killing game {} on host",
                            sid, hb_game_id
                        );
                        kill_game_on_exit = true;
                        break;
                    }
                }
            }
        }

        // Kill the game on the host if the stream ended while it was still running.
        if kill_game_on_exit {
            match process::PROCESS_MANAGER.lock().kill_game(hb_game_id.clone()) {
                Ok(()) => info!("[STREAM-FULFILL] Killed game {} on host after stream ended", hb_game_id),
                Err(e) => warn!("[STREAM-FULFILL] Failed to kill game {} on host: {e}", hb_game_id),
            }
        }

        // Clean up from the active sessions map, then hand Sunshine back if
        // this was the last session holding it.
        let mut sessions = ACTIVE_HOST_SESSIONS.lock().await;
        sessions.remove(&sid);
        stop_sunshine_if_last_holder(sessions).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    // ── Wrapper detection ─────────────────────────────────────────────

    #[test]
    fn wrapped_archive_strips_one_component() {
        // What Sunshine-Windows-AMD64-portable.zip actually looks like.
        let entries = names(&[
            "Sunshine",
            "Sunshine/sunshine.exe",
            "Sunshine/assets/web/index.html",
            "Sunshine/tools/dxgi-info.exe",
        ]);
        assert_eq!(shared_wrapper_dir(&entries).as_deref(), Some("Sunshine"));
        assert_eq!(strip_wrapper("Sunshine", Some("Sunshine")), None);
        assert_eq!(
            strip_wrapper("Sunshine/assets/web/index.html", Some("Sunshine")).as_deref(),
            Some("assets/web/index.html")
        );
    }

    #[test]
    fn unwrapped_archive_keeps_its_names() {
        let entries = names(&["sunshine.exe", "assets/web/index.html", "tools/dxgi-info.exe"]);
        assert_eq!(shared_wrapper_dir(&entries), None);
        assert_eq!(
            strip_wrapper("assets/web/index.html", None).as_deref(),
            Some("assets/web/index.html")
        );
    }

    #[test]
    fn mixed_roots_are_left_alone() {
        let entries = names(&["Sunshine/sunshine.exe", "README.txt"]);
        assert_eq!(shared_wrapper_dir(&entries), None);
    }

    #[test]
    fn single_file_archive_is_not_stripped_to_nothing() {
        // "sunshine.exe" is shared by every entry, but stripping it would
        // leave the archive with no files at all.
        assert_eq!(shared_wrapper_dir(&names(&["sunshine.exe"])), None);
    }

    #[test]
    fn empty_archive_has_no_wrapper() {
        assert_eq!(shared_wrapper_dir(&[]), None);
    }

    // ── Zip-slip rejection ────────────────────────────────────────────

    #[test]
    fn traversal_entries_are_rejected() {
        assert_eq!(sanitise_zip_entry("../evil.exe"), None);
        assert_eq!(sanitise_zip_entry("Sunshine/../../evil.exe"), None);
        assert_eq!(sanitise_zip_entry(r"Sunshine\..\evil.exe"), None);
    }

    #[test]
    fn rooted_and_drive_qualified_entries_are_rejected() {
        assert_eq!(sanitise_zip_entry("/etc/passwd"), None);
        assert_eq!(sanitise_zip_entry("C:/Windows/System32/evil.dll"), None);
        assert_eq!(sanitise_zip_entry(r"\Windows\evil.dll"), None);
    }

    /// The escape the pre-strip drive-letter check missed. `enclosed_name`
    /// reads `C:` in the middle of a path as an ordinary component, so this
    /// entry looked safe right up until `strip_wrapper` moved the drive letter
    /// to the front and made the whole thing absolute.
    #[test]
    fn a_drive_letter_behind_the_wrapper_is_rejected() {
        assert_eq!(sanitise_zip_entry("Sunshine/C:/Windows/Temp/evil.dll"), None);
        assert_eq!(sanitise_zip_entry("Sunshine/assets/D:/evil.dll"), None);
    }

    /// A colon anywhere else opens an NTFS alternate data stream rather than a
    /// file. Contained, but never something an archive should get to do.
    #[test]
    fn alternate_data_streams_are_rejected() {
        assert_eq!(sanitise_zip_entry("assets/index.html:evil.exe"), None);
    }

    /// Whatever the name looked like, the path written has to be under `dest`.
    #[cfg(target_os = "windows")]
    #[test]
    fn the_join_guard_catches_an_absolute_name() {
        let dest = Path::new(r"C:\drop\tools\sunshine");
        assert!(!dest.join("C:/Windows/Temp/evil.dll").starts_with(dest));
        assert!(dest.join("assets/web/index.html").starts_with(dest));
    }

    #[test]
    fn empty_entries_are_rejected() {
        assert_eq!(sanitise_zip_entry(""), None);
        assert_eq!(sanitise_zip_entry("./"), None);
    }

    #[test]
    fn backslashes_and_redundant_separators_are_normalised() {
        assert_eq!(
            sanitise_zip_entry(r"Sunshine\assets\web\index.html").as_deref(),
            Some("Sunshine/assets/web/index.html")
        );
        assert_eq!(
            sanitise_zip_entry("Sunshine//assets/./web/").as_deref(),
            Some("Sunshine/assets/web")
        );
    }

    // ── sunshine.conf lines ───────────────────────────────────────────

    #[test]
    fn a_setting_with_no_value_is_left_out_entirely() {
        let mut out = String::new();
        push_conf_setting(&mut out, "output_name", "");
        push_conf_setting(&mut out, "adapter_name", "   ");
        assert!(out.is_empty());
    }

    #[test]
    fn a_setting_with_a_value_is_written_and_trimmed() {
        let mut out = String::new();
        push_conf_setting(&mut out, "audio_sink", "  Speakers (Realtek USB Audio) ");
        assert_eq!(out, "audio_sink = Speakers (Realtek USB Audio)\n");
    }

    #[test]
    fn a_chosen_display_is_activated_without_switching_the_others_off() {
        let block = display_conf_block(
            "{326cc288-6d34-54f5-8c7d-5cb7fa9e2a49}",
            "NVIDIA GeForce RTX 4070 Ti SUPER",
            true,
        );
        assert_eq!(
            block,
            "output_name = {326cc288-6d34-54f5-8c7d-5cb7fa9e2a49}\n\
             adapter_name = NVIDIA GeForce RTX 4070 Ti SUPER\n\
             dd_configuration_option = ensure_active\n\
             dd_resolution_option = auto\n\
             dd_config_revert_on_disconnect = enabled\n"
        );
        // ensure_only_display deactivates every other monitor, and the revert
        // that would bring them back is not guaranteed to run. Never emit it.
        assert!(!block.contains("ensure_only_display"));
    }

    #[test]
    fn no_chosen_display_leaves_the_desktop_layout_alone() {
        let block = display_conf_block("", "", true);
        assert!(!block.contains("output_name"));
        assert!(!block.contains("adapter_name"));
        // verify_only never changes anything, but still lets Sunshine own the
        // resolution switch when the client asks for one.
        assert!(block.contains("dd_configuration_option = verify_only\n"));
        assert!(block.contains("dd_resolution_option = auto\n"));
    }

    #[test]
    fn the_display_revert_is_always_asked_for_on_windows() {
        // Sunshine's shipped default defers the revert to app close, which a
        // killed Sunshine never reaches.
        for display in ["", "{326cc288-6d34-54f5-8c7d-5cb7fa9e2a49}"] {
            assert!(
                display_conf_block(display, "", true)
                    .contains("dd_config_revert_on_disconnect = enabled\n"),
                "revert key missing for display {display:?}"
            );
        }
    }

    #[test]
    fn a_linux_host_gets_no_windows_only_keys() {
        let display = display_conf_block("0", "/dev/dri/renderD128", false);
        assert_eq!(
            display,
            "output_name = 0\nadapter_name = /dev/dri/renderD128\n"
        );
        assert!(audio_conf_block("", "", false).is_empty());
    }

    #[test]
    fn audio_keys_appear_only_when_a_device_was_chosen() {
        let empty = audio_conf_block("", "", true);
        assert_eq!(empty, "install_steam_audio_drivers = enabled\n");

        let full = audio_conf_block(
            "Headphones (Realtek USB Audio)",
            "Speakers (Steam Streaming Speakers)",
            true,
        );
        assert_eq!(
            full,
            "audio_sink = Headphones (Realtek USB Audio)\n\
             virtual_sink = Speakers (Steam Streaming Speakers)\n\
             install_steam_audio_drivers = enabled\n"
        );
    }

    // ── Health probe ──────────────────────────────────────────────────

    fn lay_down_required(root: &Path) {
        for (rel, is_dir) in SUNSHINE_REQUIRED_PATHS {
            let path = root.join(rel);
            if *is_dir {
                std::fs::create_dir_all(&path).unwrap();
            } else {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, b"x").unwrap();
            }
        }
    }

    #[test]
    fn health_probe_passes_on_a_complete_install() {
        let dir = tempfile::tempdir().unwrap();
        lay_down_required(dir.path());
        assert!(missing_sunshine_files(dir.path()).is_empty());
    }

    #[test]
    fn health_probe_flags_an_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            missing_sunshine_files(dir.path()).len(),
            SUNSHINE_REQUIRED_PATHS.len()
        );
    }

    /// The flattened extract this stage exists to fix: the exe is there and
    /// every support file it needs is not.
    #[test]
    fn health_probe_flags_a_flattened_install() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SUNSHINE_REQUIRED_PATHS[0].0), b"x").unwrap();
        let missing = missing_sunshine_files(dir.path());
        assert!(!missing.contains(&SUNSHINE_REQUIRED_PATHS[0].0));
        assert_eq!(missing.len(), SUNSHINE_REQUIRED_PATHS.len() - 1);
    }

    #[test]
    fn health_probe_wants_a_directory_where_a_directory_is_required() {
        let Some((rel, _)) = SUNSHINE_REQUIRED_PATHS.iter().find(|(_, is_dir)| *is_dir) else {
            return; // no directory requirements on this platform
        };
        let dir = tempfile::tempdir().unwrap();
        lay_down_required(dir.path());
        let path = dir.path().join(rel);
        std::fs::remove_dir_all(&path).unwrap();
        std::fs::write(&path, b"not a directory").unwrap();
        assert!(missing_sunshine_files(dir.path()).contains(rel));
    }

    #[test]
    fn working_dir_is_the_binarys_own_directory() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("sunshine.exe");
        assert_eq!(sunshine_working_dir(&exe).as_deref(), Some(dir.path()));
        // A bare name off PATH has no directory to run from.
        assert_eq!(sunshine_working_dir(Path::new("sunshine.exe")), None);
    }

    #[test]
    fn wipe_keeps_config_and_removes_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/cacert.pem"), b"cert").unwrap();
        std::fs::create_dir_all(dir.path().join("assets/web")).unwrap();
        std::fs::write(dir.path().join("assets/web/index.html"), b"page").unwrap();
        std::fs::write(dir.path().join("sunshine.exe"), b"exe").unwrap();

        wipe_sunshine_install(dir.path()).unwrap();

        assert!(dir.path().join("config/cacert.pem").is_file());
        assert!(!dir.path().join("assets").exists());
        assert!(!dir.path().join("sunshine.exe").exists());
    }

    // ── Paired clients ────────────────────────────────────────────────

    #[test]
    fn paired_clients_are_flattened_to_name_and_uuid() {
        let raw = serde_json::json!({
            "status": true,
            "named_certs": [
                { "name": "Steam Deck", "uuid": "abc-123", "cert": "..." },
                { "name": "Living Room TV", "uuid": "def-456" },
            ]
        });
        let clients = parse_paired_clients(&raw);
        assert_eq!(clients.len(), 2);
        assert_eq!(clients[0].name, "Steam Deck");
        assert_eq!(clients[0].uuid, "abc-123");
        assert_eq!(clients[1].uuid, "def-456");
    }

    #[test]
    fn a_client_that_paired_without_a_name_still_renders() {
        let raw = serde_json::json!({
            "status": true,
            "named_certs": [{ "name": "   ", "uuid": "abc-123" }]
        });
        assert_eq!(parse_paired_clients(&raw)[0].name, "Unnamed device");
    }

    #[test]
    fn a_client_with_no_uuid_is_dropped() {
        // Nothing could be unpaired from that row, so it would only ever be a
        // button that does nothing.
        let raw = serde_json::json!({
            "status": true,
            "named_certs": [{ "name": "Ghost" }, { "name": "Deck", "uuid": "" }]
        });
        assert!(parse_paired_clients(&raw).is_empty());
    }

    #[test]
    fn no_pairings_is_an_empty_list_not_an_error() {
        // What Sunshine actually answers when nothing has ever paired.
        assert!(parse_paired_clients(&serde_json::json!({ "status": true })).is_empty());
    }

    // ── Sunshine API error classification ─────────────────────────────

    #[test]
    fn a_rejected_login_is_recognised() {
        assert!(is_auth_failure(
            "Sunshine API error: 401 Unauthorized - <html>Unauthorized</html>"
        ));
        assert!(is_auth_failure("Sunshine API error: 401 - "));
    }

    #[test]
    fn other_api_errors_are_not_credential_failures() {
        // A missing endpoint or a refused connection must not send the user off
        // resetting credentials that were fine.
        assert!(!is_auth_failure("Sunshine API error: 404 Not Found - "));
        assert!(!is_auth_failure(
            "Sunshine API request failed: error sending request for url"
        ));
    }
}
