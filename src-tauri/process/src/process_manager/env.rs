//! Launch-environment construction and hardening.
//!
//! Everything in here concerns *what environment a game's child process
//! inherits*. It is deliberately split out of the launch flow because env
//! handling is security-sensitive (a remote launch config must not be able
//! to inject `LD_PRELOAD`) and platform-heavy (AppImage scrubbing, Gamescope
//! display vars, MangoHud) — keeping it isolated makes both auditable.

use std::process::Command;

use log::{info, warn};

/// Env vars we refuse to set from a launch config (server-provided or pasted
/// by the user). These can be used to hijack loading / command resolution and
/// should never be controllable by a remote config. Legitimate game launches
/// should never need to override these — Steam/Proton/Wine set them
/// themselves.
const FORBIDDEN_ENV_KEYS: &[&str] = &[
    "LD_PRELOAD",
    "LD_AUDIT",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FORCE_FLAT_NAMESPACE",
    "PATH",
    "PYTHONPATH",
    "PYTHONHOME",
    "NODE_OPTIONS",
    "NODE_PATH",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CURL_CA_BUNDLE",
    "REQUESTS_CA_BUNDLE",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
];

/// Whether `key` is on the launch-config env denylist (case-insensitive).
pub fn is_env_key_forbidden(key: &str) -> bool {
    FORBIDDEN_ENV_KEYS
        .iter()
        .any(|forbidden| key.eq_ignore_ascii_case(forbidden))
}

/// Apply the `KEY=VALUE` env strings from a parsed launch command to `command`,
/// silently dropping any key on the [`FORBIDDEN_ENV_KEYS`] denylist. Centralised
/// so the primary spawn path and the ENOEXEC retry path stay consistent.
pub fn apply_launch_env(command: &mut Command, env_strings: &[String]) {
    for env_str in env_strings {
        if let Some((key, value)) = env_str.split_once('=') {
            if is_env_key_forbidden(key) {
                warn!("[LAUNCH] Ignoring forbidden env var from launch config: {key}");
                continue;
            }
            command.env(key, value);
        }
    }
}

/// Detect whether *this* process is running from an AppImage. Cheap — just
/// reads an env var — but worth caching into a local so callers don't repeat
/// the syscall for every spawn.
#[cfg(target_os = "linux")]
pub fn running_from_appimage() -> bool {
    std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some()
}

#[cfg(not(target_os = "linux"))]
pub fn running_from_appimage() -> bool {
    false
}

/// Scrub the env pollution an AppImage runtime injects into its children.
///
/// When Drop is packaged as an AppImage, the runtime mounts the image at
/// `/tmp/.mount_DropXXXXXX/` and prepends its `usr/lib/` to `LD_LIBRARY_PATH`
/// (and sets `APPDIR`, `APPIMAGE`, `ARGV0`, sometimes `LD_PRELOAD`). If we
/// naively forward the parent env when spawning tools that link against
/// system libraries — `umu-run`, Proton, Wine, system Python — the bundled
/// older libs shadow the system's and trip version-symbol errors
/// (the one we hit in practice: `OPENSSL_3.3.0 not found` when Python 3.13's
/// `_ssl` loads our older libcrypto).
///
/// AppImage runtimes stash the caller's original `LD_LIBRARY_PATH` in
/// `LD_LIBRARY_PATH_ORIG` before mutating the live var, so we restore from
/// that when available; otherwise we drop `LD_LIBRARY_PATH` entirely so the
/// child uses the OS default search path.
#[cfg(target_os = "linux")]
pub fn sanitize_appimage_env(command: &mut Command) {
    if !running_from_appimage() {
        return;
    }

    match std::env::var("LD_LIBRARY_PATH_ORIG") {
        Ok(orig) if !orig.is_empty() => {
            command.env("LD_LIBRARY_PATH", orig);
        }
        _ => {
            command.env_remove("LD_LIBRARY_PATH");
        }
    }

    // These are either AppImage-runtime bookkeeping that we don't want to
    // propagate (APPDIR, APPIMAGE, ARGV0, LD_LIBRARY_PATH_ORIG) or vectors
    // for symbol injection from the AppImage's bundled stack (LD_PRELOAD,
    // LD_AUDIT). Python-specific vars are cleaned elsewhere by the caller.
    for key in [
        "LD_LIBRARY_PATH_ORIG",
        "LD_PRELOAD",
        "LD_AUDIT",
        "APPDIR",
        "APPIMAGE",
        "ARGV0",
    ] {
        command.env_remove(key);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn sanitize_appimage_env(_command: &mut Command) {}

/// Emit a one-line summary of the launch-relevant env we're passing to the
/// child. Lets a black-screen report be triaged from the log alone: which
/// Proton did umu use, which game id, whether AppImage sanitization kicked
/// in, whether LD_LIBRARY_PATH / LD_PRELOAD escaped. Values that can carry
/// filesystem paths (WINEPREFIX, PROTONPATH) are logged verbatim — they're
/// not secret. We never log full LD_LIBRARY_PATH contents (can be long and
/// noisy); instead we record whether it's set and its byte length.
pub fn log_launch_env_fingerprint(command: &Command, game_id: &str) {
    let env_of = |k: &str| -> Option<String> {
        command
            .get_envs()
            .find(|(key, _)| key.to_string_lossy() == k)
            .and_then(|(_, val)| val.map(|v| v.to_string_lossy().to_string()))
    };
    let removed = |k: &str| -> bool {
        command
            .get_envs()
            .any(|(key, val)| key.to_string_lossy() == k && val.is_none())
    };
    let ld_summary = match env_of("LD_LIBRARY_PATH") {
        Some(v) if v.is_empty() => "empty".to_string(),
        Some(v) => format!("set({} bytes)", v.len()),
        None if removed("LD_LIBRARY_PATH") => "removed".to_string(),
        None => "inherited".to_string(),
    };

    info!(
        "[LAUNCH/env] game_id={game_id} \
         GAMEID={gid:?} \
         PROTONPATH={proton:?} \
         WINEPREFIX={pfx:?} \
         LD_LIBRARY_PATH={ld} \
         LD_PRELOAD={preload} \
         MANGOHUD={mango:?} \
         appimage_scrubbed={scrubbed}",
        gid = env_of("GAMEID").unwrap_or_default(),
        proton = env_of("PROTONPATH").unwrap_or_default(),
        pfx = env_of("WINEPREFIX").unwrap_or_default(),
        ld = ld_summary,
        preload = env_of("LD_PRELOAD").unwrap_or_else(|| "(unset)".into()),
        mango = env_of("MANGOHUD").unwrap_or_default(),
        scrubbed = running_from_appimage(),
    );
}

/// Apply the baseline env scrub every game launch needs, regardless of
/// platform or compositor: drop `RUST_LOG` (Drop's own logging config must
/// not leak into the game) and Steam/Gamescope's bundled-Python vars
/// (`PYTHONHOME`/`PYTHONPATH`) which break `umu-run`'s system Python with
/// "No module named 'encodings'".
pub fn apply_baseline_env_scrub(command: &mut Command) {
    command
        .env_remove("RUST_LOG")
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH");
}

/// Configure Gamescope / Steam Deck Game Mode display env vars.
///
/// Only does anything on Linux inside a Gamescope session. `is_appimage`
/// indicates the game executable is itself an AppImage (e.g. a RetroArch
/// build) — those need their bundled stale Mesa/Vulkan forced aside in favour
/// of the system stack or they black-screen on the Deck's RDNA2 GPU.
#[cfg(target_os = "linux")]
pub fn configure_gamescope_env(command: &mut Command, game_id: &str, is_appimage: bool) {
    use ::client::app_state::SessionType;

    if SessionType::detect() != SessionType::Gamescope {
        return;
    }
    info!("[LAUNCH/env] Gamescope session detected — applying display env");

    // Pass through Gamescope display vars so Proton/Wine can find the
    // correct Wayland/X11 display.
    for var in &[
        "GAMESCOPE_WAYLAND_DISPLAY",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
    ] {
        if let Ok(val) = std::env::var(var) {
            command.env(var, val);
        }
    }

    // Remove Steam's LD_PRELOAD — it injects the 32-bit
    // gameoverlayrenderer.so into 64-bit processes, which fails and can
    // interfere with video surface creation.
    command.env_remove("LD_PRELOAD");

    if is_appimage {
        info!("[LAUNCH/env] AppImage in Gamescope — forcing system Vulkan/Mesa");
        // Drop any library-path override that could pull in the AppImage's
        // stale bundled Mesa.
        command.env_remove("LD_LIBRARY_PATH");
        // Point the Vulkan loader at the system's AMD radv ICD (standard
        // path on SteamOS / Arch-based distros).
        let radv_icd = "/usr/share/vulkan/icd.d/radeon_icd.x86_64.json";
        let alt_icd = "/usr/share/vulkan/icd.d/radeon_icd.json";
        if std::path::Path::new(radv_icd).exists() {
            info!("[LAUNCH/env] Found system Vulkan ICD: {radv_icd}");
            command.env("VK_ICD_FILENAMES", radv_icd);
        } else if std::path::Path::new(alt_icd).exists() {
            info!("[LAUNCH/env] Found system Vulkan ICD (alt): {alt_icd}");
            command.env("VK_ICD_FILENAMES", alt_icd);
        } else {
            warn!(
                "[LAUNCH/env] No system Vulkan ICD at {radv_icd} or {alt_icd} — \
                 Vulkan may fail if AppImage bundles stale mesa"
            );
        }
        // Disable the AppImage's internal library extraction so it uses the
        // host graphics stack entirely.
        command.env("APPIMAGE_EXTRACT_AND_RUN", "1");
    }

    // Enable Steam Game Mode integration for Proton games.
    command.env("SteamGameId", game_id);
    command.env("SteamAppId", game_id);

    // Tell Proton/Wine the target resolution. Without this, some games
    // default to tiny resolutions because Proton doesn't know the display
    // size. The Steam Deck display is 1280x800; a safe default for desktop
    // Gamescope too.
    command.env("STEAM_DISPLAY", ":1");
    if std::env::var("SCREEN_WIDTH").is_err() {
        command.env("SCREEN_WIDTH", "1280");
    }
    if std::env::var("SCREEN_HEIGHT").is_err() {
        command.env("SCREEN_HEIGHT", "800");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn configure_gamescope_env(_command: &mut Command, _game_id: &str, _is_appimage: bool) {}

/// Apply the MangoHud performance-overlay env vars for the resolved preset.
///
/// Linux-only. Per-game configuration takes priority over the global
/// Settings value; the caller resolves which preset is effective and passes
/// it in so this function does not touch the database.
#[cfg(target_os = "linux")]
pub fn configure_mangohud_env(
    command: &mut Command,
    preset: Option<&database::models::data::MangoHudPreset>,
) {
    use database::models::data::MangoHudPreset;

    let Some(preset) = preset else { return };
    match preset {
        MangoHudPreset::Off => {}
        MangoHudPreset::Minimal => {
            command.env("MANGOHUD", "1");
            command.env("MANGOHUD_CONFIG", "fps,no_display");
        }
        MangoHudPreset::Standard => {
            command.env("MANGOHUD", "1");
            command.env(
                "MANGOHUD_CONFIG",
                "fps,frametime,cpu_stats,gpu_stats,ram,vram",
            );
        }
        MangoHudPreset::Full => {
            command.env("MANGOHUD", "1");
            // Full uses MangoHud's default config (shows everything).
        }
    }
}

// MangoHud is Linux-only — on other platforms this stub is never called.
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn configure_mangohud_env(
    _command: &mut Command,
    _preset: Option<&database::models::data::MangoHudPreset>,
) {
}

/// Re-apply Gamescope env vars on the ENOEXEC bash-wrapper retry path.
///
/// The retry rebuilds `Command` from scratch, so it loses the Gamescope env
/// set on the original. Detection here is intentionally lighter than
/// [`configure_gamescope_env`] (env-var presence rather than `SessionType`)
/// because the retry is a rare fallback and we only need the display vars
/// back, not the full AppImage handling.
#[cfg(target_os = "linux")]
pub fn reapply_gamescope_env_for_retry(command: &mut Command, game_id: &str) {
    let in_gamescope = std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
        || std::env::var("SteamGamepadUI").is_ok();
    if !in_gamescope {
        return;
    }
    command.env_remove("LD_PRELOAD");
    for var in &[
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "GAMESCOPE_WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if let Ok(val) = std::env::var(var) {
            command.env(var, val);
        }
    }
    command.env("SteamGameId", game_id);
    command.env("SteamAppId", game_id);
}

// The ENOEXEC bash-wrapper retry that calls this is Linux-only.
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn reapply_gamescope_env_for_retry(_command: &mut Command, _game_id: &str) {}
