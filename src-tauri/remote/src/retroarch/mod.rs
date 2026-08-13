//! RetroArch pre-launch configuration.
//!
//! When a game uses RetroArch as its emulator, Drop writes (or patches) a
//! `retroarch.cfg` and `retroarch-core-options.cfg` so the game launches
//! zero-config:
//!
//! - core / system / assets directories point at the install,
//! - saves & states go to a per-game `drop-saves/<game_id>/` directory so the
//!   cloud-save system can find them,
//! - controller autodetect + a sane fallback layout are enabled,
//! - the user's per-game controller layout / quality preset / aspect ratio /
//!   CRT shader choices are applied,
//! - RetroAchievements is enabled and, if credentials exist, auto-logged-in.
//!
//! # Module layout
//!
//! This was a single 2622-line file; it is now split by concern. Every public
//! item is re-exported here so `remote::retroarch::Foo` paths used by the
//! `process` and root crates keep working unchanged.
//!
//! * [`discovery`]  — detecting a RetroArch install + AppImage paths.
//! * [`cfg`]        — the `key = "value"` config-file patch primitives and the
//!   host-path → Wine-path conversion Proton installs need.
//! * [`logs`]       — reading RetroArch's own log after a session.
//! * [`cores`]      — the data-driven ROM-extension → libretro-core table and
//!   ROM→core resolution (incl. ISO disc-header sniffing).
//! * [`bios`]       — BIOS/firmware detection and auto-placement.
//! * [`controllers`]— controller layout, hotkeys and per-core remap files.
//! * [`presets`]    — quality-preset and aspect-ratio config.
//! * [`shaders`]    — CRT-shader selection and auto-apply preset writing.
//! * [`ra`]         — RetroAchievements: credentials + ROM-hash verification.
//!
//! The big [`configure_retroarch_for_game`] orchestrator stays in this file —
//! it is the launch-time entry point that drives every sub-module in order.

pub mod bios;
pub mod cfg;
pub mod controllers;
pub mod cores;
pub mod discovery;
pub mod logs;
pub mod presets;
pub mod ra;
pub mod shaders;

use database::models::data::{AspectRatio, ControllerType, UserConfiguration};
use log::{error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// Re-export the public surface so existing `remote::retroarch::*` call sites
// in the `process` crate and the root crate keep compiling without edits.
pub use cfg::PathStyle;
pub use controllers::{detect_pad_family, PadFamily};
pub use cores::{resolve_core_for_rom, EXTENSION_CORE_MAP};
pub use logs::detect_fatal_video_error;
pub use ra::{
    check_rom_hash, detect_ra_login_failure, fetch_ra_credentials, hash_rom,
    mark_credentials_expired, RACredentials, RAHashEntry, RAHashesResponse, RomHashStatus,
};

/// Keys older Drop versions wrote into `retroarch.cfg` that never belonged in
/// that file, deleted on every patch alongside [`controllers::STALE_INPUT_KEYS`].
///
/// `dolphin_renderer` is a libretro **core** option: Dolphin reads it from
/// `retroarch-core-options.cfg` and has never looked at `retroarch.cfg`, so the
/// line we used to write there did nothing but sit in the file looking like it
/// worked. It is written to the right file now (see
/// [`presets::apply_core_quality_options`]); this clears the orphan out of
/// installs that already have one.
const STALE_MISPLACED_KEYS: &[&str] = &["dolphin_renderer"];

/// Result of configuring RetroArch for a game launch.
#[derive(Debug, Clone)]
pub struct RetroArchInfo {
    /// Absolute path to this game's save-file directory.
    pub savefile_directory: String,
    /// Absolute path to this game's save-state directory.
    pub savestate_directory: String,
    /// BIOS warnings for the frontend to display (empty if all OK).
    pub bios_warnings: Vec<String>,
    /// CRT shader path if enabled and found, `None` otherwise.
    pub crt_shader_path: Option<String>,
    /// The `video_driver` value Drop settled on for this launch, unquoted.
    /// `None` when Drop left the choice to RetroArch. Carried so the exit path
    /// can name the driver if RetroArch dies on video init.
    pub video_driver: Option<String>,
}

/// Detects whether the emulator at `emulator_install_dir` is RetroArch and,
/// if so, writes/patches its config for a zero-config launch.
///
/// `user_id` + `game_id` key the per-game save directory — this is the writer
/// half of [`crate::save_sync::emu_saves_root`], and it must agree with the
/// scanner or RetroArch writes saves somewhere sync never looks. `None` is the
/// signed-out layout. `ra_credentials`, when present, is injected so RetroArch
/// logs into RetroAchievements automatically. `user_config` carries the
/// per-game controller / quality / aspect-ratio / CRT choices. `rom_path` is
/// used to scope BIOS warnings and pick a shader/controller-device appropriate
/// to the ROM's platform.
///
/// `detected_pad` is the layout family of whatever pad is plugged in right now,
/// resolved by the caller (which owns the input backend). It only decides
/// anything when the user's per-game controller setting is "Auto"; an explicit
/// choice always wins, so a misdetected pad stays correctable by hand.
///
/// `under_proton` says this launch runs the **Windows** `retroarch.exe` through
/// umu/Proton rather than a build native to this machine. The caller knows it
/// because it is the same condition that made it build a umu command
/// (`ProcessHandler::runs_under_proton`). It changes two things, and nothing
/// else: every path written into a config file is spelled for Wine
/// ([`cfg::PathStyle`]), and the Gamescope video driver becomes the one that
/// works through Proton rather than RetroArch's native-Linux Vulkan.
///
/// Returns `Some(RetroArchInfo)` if RetroArch was detected and configured, or
/// `None` if this is not a RetroArch install.
#[allow(clippy::too_many_arguments)]
pub fn configure_retroarch_for_game(
    emulator_install_dir: &str,
    user_id: Option<&str>,
    game_id: &str,
    ra_credentials: Option<&RACredentials>,
    user_config: Option<&UserConfiguration>,
    detected_pad: PadFamily,
    rom_path: Option<&str>,
    under_proton: bool,
) -> Option<RetroArchInfo> {
    let emu_root = PathBuf::from(emulator_install_dir);
    let style = if under_proton {
        PathStyle::Wine
    } else {
        PathStyle::Native
    };

    if !discovery::is_retroarch(&emu_root) {
        warn!(
            "[RETROARCH] No RetroArch detected in {emulator_install_dir} — checked for: \
             retroarch, retroarch.exe, retroarch.AppImage, retroarch.cfg, cores/ dir. Skipping."
        );
        if let Ok(entries) = fs::read_dir(&emu_root) {
            let files: Vec<String> = entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            warn!("[RETROARCH] Files in {emulator_install_dir}: {files:?}");
        }
        return None;
    }

    info!(
        "[RETROARCH] Detected RetroArch in {emulator_install_dir}, configuring for game {game_id} \
         (under_proton={under_proton}, path style {style:?})"
    );

    // Absolute paths for every directory RetroArch needs.
    let cores_dir = emu_root.join("cores");
    let system_dir = emu_root.join("system");
    let assets_dir = emu_root.join("assets");
    // Mark this account's root the moment it exists, not only when the
    // migration creates it. An account that first signs in after the migration
    // has already finished would otherwise have an unmarked root, and the next
    // layout sweep would read it as a stray game directory and file another
    // person's saves under its own id.
    if let Some(user_id) = user_id
        && let Err(e) = crate::save_sync::ensure_user_root(&emu_root, user_id)
    {
        warn!("[RETROARCH] {e}");
    }
    // Not `emu_saves_root`: while the one-time move into the per-user layout
    // is unfinished the saves are still in the legacy directory, and pointing
    // RetroArch at the per-user path would boot the game from a blank save and
    // write a second divergent copy of it.
    let saves_base = crate::save_sync::resolve_emu_saves_root(&emu_root, user_id, game_id);
    let savefile_dir = saves_base.join("saves");
    let savestate_dir = saves_base.join("states");
    let logs_dir = emu_root.join("logs");

    for dir in [&savefile_dir, &savestate_dir, &system_dir, &logs_dir] {
        if let Err(e) = fs::create_dir_all(dir) {
            warn!("[RETROARCH] Failed to create dir {}: {e}", dir.display());
        }
    }

    // ── BIOS detection & auto-copy ───────────────────────────────────────
    let current_rom_ext: Option<String> = rom_path
        .and_then(|rp| Path::new(rp).extension())
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    // Which core will really load this ROM, so the BIOS check can scope its
    // warnings to that system. Extension alone is not enough: a PS1 disc is
    // .cue/.bin/.chd, and so are the PS2, Sega CD and Saturn rows, so a game
    // that launches fine would report three missing BIOSes to the frontend.
    let resolved_core: Option<String> = rom_path
        .and_then(|rp| cores::resolve_core_for_rom(&emu_root, rp))
        .and_then(|core| core.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    let bios_warnings = bios::check_and_place_bios(
        &system_dir,
        current_rom_ext.as_deref(),
        resolved_core.as_deref(),
    );

    // ── retroarch.cfg overrides ──────────────────────────────────────────
    let mut overrides: HashMap<&str, String> = HashMap::new();
    let remaps_dir = emu_root.join("config").join("remaps");
    let core_opts_file = emu_root.join("retroarch-core-options.cfg");

    apply_path_overrides(&mut overrides, &cores_dir, &system_dir, &assets_dir, &emu_root, style);
    apply_save_overrides(&mut overrides, &savefile_dir, &savestate_dir, &saves_base, style);
    apply_baseline_overrides(&mut overrides, &remaps_dir, &core_opts_file, &logs_dir, style);
    apply_video_input_overrides(&mut overrides, under_proton);

    // Emulated controller device type, scoped to the ROM's platform.
    if let Some(rp) = rom_path {
        apply_controller_device(&mut overrides, rp);
    }

    // RetroAchievements — enable cheevos + inject Connect credentials.
    let ra_token_expired = apply_cheevos_overrides(&mut overrides, ra_credentials);

    // One family decides both the face buttons and the hotkey combos. An
    // explicit per-game choice wins over detection; "Auto" takes the pad Drop
    // can actually see. Resolved here rather than inside `apply_user_config`
    // so the hotkeys, which are written first, cannot disagree with the face
    // buttons written later.
    let effective_pad = user_config
        .and_then(|cfg| cfg.controller_type.as_ref())
        .map(controllers::family_for_controller_type)
        .unwrap_or(detected_pad);
    controllers::apply_hotkey_bindings(&mut overrides, effective_pad);

    // ── Per-game user config ─────────────────────────────────────────────
    let mut crt_shader_path: Option<String> = None;
    if let Some(cfg) = user_config {
        apply_user_config(
            cfg,
            &mut overrides,
            &emu_root,
            &remaps_dir,
            rom_path,
            detected_pad,
            &mut crt_shader_path,
            style,
        );
    } else {
        // No per-game config at all (a ROM imported by a disk scan, before its
        // GameVersion has been synced). Still write the detected pad's
        // fallback — the alternative is the XInput table by default, which is
        // exactly the bug.
        controllers::cleanup_nintendo_remaps(&remaps_dir);
        controllers::set_face_button_fallback(&mut overrides, detected_pad);
    }

    log_diagnostic_overrides(&overrides);
    // Read after every override pass, so this is the driver RetroArch will
    // really try — including the Dolphin + CRT override written above.
    let video_driver = resolved_video_driver(&overrides);

    // ── Write the main config (used by --appendconfig) ───────────────────
    // Both write sites share one deletion list. A key dropped from only one of
    // them leaves the AppImage portable $HOME stale, and on the Deck that copy
    // is the BASE config rather than an overlay.
    let mut stale_keys: Vec<&str> = controllers::STALE_INPUT_KEYS
        .iter()
        .chain(STALE_MISPLACED_KEYS)
        .copied()
        .collect();

    // Only delete the token when we've stopped injecting it. The deletion pass
    // runs before the append pass, so a key that is in both lists survives as
    // an override — listing it unconditionally would quietly do nothing.
    if ra_token_expired {
        stale_keys.push("cheevos_token");
    }

    let cfg_path = emu_root.join("retroarch.cfg");
    info!("[RETROARCH] Writing retroarch.cfg ({} keys) to {}", overrides.len(), cfg_path.display());
    // Treat a primary-cfg write failure as a hard abort: previously this was
    // a silent `warn!` and the launch proceeded against whatever stale or
    // half-written cfg was on disk, which surfaced as the "game starts and
    // then freezes mysteriously" pattern. Returning None aborts the RA
    // configuration; the caller falls back to launching against the raw
    // RetroArch install instead of pretending our patches landed.
    if let Err(e) = cfg::patch_retroarch_cfg_with_deletions(&cfg_path, &overrides, &stale_keys) {
        error!(
            "[RETROARCH] Failed to write retroarch.cfg ({e}) — aborting RA configuration to avoid launching against stale settings"
        );
        return None;
    }

    // Also write to the AppImage portable $HOME so our settings are the BASE
    // config, not just an --appendconfig overlay (critical on Steam Deck).
    // This one is a copy of the primary write — if it fails we keep going,
    // because the `--appendconfig` overlay path is still in effect.
    let appimage_config_dir = discovery::find_appimage_config_dir(&emu_root);
    if let Some(ai_cfg_dir) = &appimage_config_dir {
        if let Err(e) = fs::create_dir_all(ai_cfg_dir) {
            warn!("[RETROARCH] Failed to create AppImage config dir {}: {e}", ai_cfg_dir.display());
        } else {
            let ai_cfg_path = ai_cfg_dir.join("retroarch.cfg");
            if let Err(e) =
                cfg::patch_retroarch_cfg_with_deletions(&ai_cfg_path, &overrides, &stale_keys)
            {
                warn!(
                    "[RETROARCH] Failed to write AppImage config copy at {} ({e}) — primary config was written, continuing",
                    ai_cfg_path.display()
                );
            } else {
                info!("[RETROARCH] Also wrote config to AppImage home: {}", ai_cfg_path.display());
            }
        }
    }

    // ── Core options + stale per-core override cleanup ───────────────────
    clean_stale_per_core_overrides(&emu_root);
    if let Some(cfg) = user_config {
        write_core_options(cfg, &emu_root, &appimage_config_dir);
    }

    // ── Core-specific Nintendo remaps (Xbox/Auto/PS only) ────────────────
    // In Nintendo mode `apply_controller_mappings` already remapped every core
    // with the full A<->B + X<->Y swap; running this too would clobber it.
    let is_nintendo_mode = user_config
        .and_then(|cfg| cfg.controller_type.as_ref())
        .is_some_and(|ct| matches!(ct, ControllerType::Nintendo));
    if is_nintendo_mode {
        info!("[RETROARCH] Skipping N64/GC core remaps — Nintendo mode handles all cores");
    } else {
        controllers::write_nintendo_core_remaps(&emu_root, &appimage_config_dir);
    }

    info!(
        "[RETROARCH] Configured: saves={}, states={}",
        savefile_dir.display(),
        savestate_dir.display()
    );

    Some(RetroArchInfo {
        savefile_directory: savefile_dir.to_string_lossy().to_string(),
        savestate_directory: savestate_dir.to_string_lossy().to_string(),
        bios_warnings,
        crt_shader_path,
        video_driver,
    })
}

// ── Override-group helpers ───────────────────────────────────────────────
//
// Each fills a slice of the `overrides` map; splitting them out keeps the
// orchestrator readable and the comments next to the keys they explain.

/// Core / system / assets directory paths. `joypad_autoconfig_dir` is
/// intentionally *not* set — the AppImage bundles its own profiles, and
/// pointing it at an empty dir triggers "not configured" fallback warnings.
fn apply_path_overrides(
    overrides: &mut HashMap<&str, String>,
    cores_dir: &Path,
    system_dir: &Path,
    assets_dir: &Path,
    emu_root: &Path,
    style: PathStyle,
) {
    overrides.insert("libretro_directory", cfg::path_to_cfg(cores_dir, style));
    overrides.insert("system_directory", cfg::path_to_cfg(system_dir, style));
    overrides.insert("assets_directory", cfg::path_to_cfg(assets_dir, style));
    overrides.insert("rgui_browser_directory", cfg::path_to_cfg(emu_root, style));
}

/// Per-game save isolation. Drop manages save paths itself, so RetroArch's own
/// content-directory sorting is disabled.
fn apply_save_overrides(
    overrides: &mut HashMap<&str, String>,
    savefile_dir: &Path,
    savestate_dir: &Path,
    saves_base: &Path,
    style: PathStyle,
) {
    overrides.insert("savefile_directory", cfg::path_to_cfg(savefile_dir, style));
    overrides.insert("savestate_directory", cfg::path_to_cfg(savestate_dir, style));
    overrides.insert(
        "screenshot_directory",
        cfg::path_to_cfg(&saves_base.join("screenshots"), style),
    );
    for key in [
        "sort_savefiles_enable",
        "sort_savestates_enable",
        "sort_savefiles_by_content_enable",
        "sort_savestates_by_content_enable",
    ] {
        overrides.insert(key, "false".into());
    }
    overrides.insert("savestate_auto_save", "false".into());
    overrides.insert("savestate_auto_load", "false".into());
}

/// "Just works" baseline: autodetect on, single-press quit, core-options file
/// + remap directory wired up.
fn apply_baseline_overrides(
    overrides: &mut HashMap<&str, String>,
    remaps_dir: &Path,
    core_opts_file: &Path,
    logs_dir: &Path,
    style: PathStyle,
) {
    overrides.insert("input_autodetect_enable", "true".into());
    overrides.insert("pause_nonactive", "false".into());
    overrides.insert("menu_driver", "ozone".into());
    overrides.insert("video_font_enable", "true".into()); // RA unlock toasts
    overrides.insert("quit_press_twice", "false".into());

    // Core-specific input remaps (Nintendo A<->B swap etc.).
    overrides.insert("input_remap_binds_enable", "true".into());
    overrides.insert("input_autoload_remaps", "true".into());
    overrides.insert("remaps_directory", cfg::path_to_cfg(remaps_dir, style));

    // global_core_options stops RetroArch writing per-core .opt files that
    // would outrank our core_options_path after the first launch.
    overrides.insert("global_core_options", "true".into());
    overrides.insert("core_options_path", cfg::path_to_cfg(core_opts_file, style));

    // Permanent file logging. RetroArch ships with log_to_file off and a
    // malformed relative log_dir (":\logs"), so when an emulated game takes the
    // whole session down there is *no* emulator-side evidence to look at —
    // every launch-crash investigation so far has had nothing but Drop's own
    // log. The verbose level is the point: driver init, shader compilation and
    // core-option parsing all live below the default log level, and those are
    // exactly the lines that matter. log_to_file_timestamp gives one file per
    // launch instead of one ever-growing blob.
    overrides.insert("log_to_file", "true".into());
    overrides.insert("log_to_file_timestamp", "true".into());
    overrides.insert("log_verbosity", "true".into());
    overrides.insert("frontend_log_level", "0".into()); // 0 = DEBUG
    overrides.insert("log_dir", cfg::path_to_cfg(logs_dir, style));
}

/// True when Drop is running inside Gamescope (Steam Deck Game Mode).
///
/// Probed from the environment because Drop is launched *by* Gamescope, not
/// the other way round — there is no setting to read. Compiled out to a
/// constant `false` off Linux, where nothing nests us in a Wayland compositor.
fn in_gamescope() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok() || std::env::var("SteamGamepadUI").is_ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// The Gamescope video driver for a **native** RetroArch build.
///
/// Forced because the AppImage's bundled Mesa was too old for RDNA2
/// auto-detection.
const GAMESCOPE_NATIVE_VIDEO_DRIVER: &str = "vulkan";

/// The Gamescope video driver for the **Windows** RetroArch running under
/// Proton.
///
/// `vulkan` here is not RetroArch talking to Mesa — it is RetroArch's Win32
/// Vulkan backend asking winevulkan for a `VK_KHR_win32_surface` inside a
/// nested Wayland compositor, and on the Deck that is exactly what failed:
///
/// ```text
/// [ERROR] [Vulkan] Failed to set video mode.
/// [ERROR] [Video] Cannot open video driver. Exiting...
/// ```
///
/// `d3d11` is RetroArch's own compiled-in default on Windows and reaches the
/// GPU through DXVK, the path Proton is built around and the one every DX11
/// title in Game Mode already uses. `gl`/`glcore` are deliberately not chosen
/// here: under Proton they run Wine's opengl32 straight into Mesa radeonsi,
/// which on this hardware has faulted the driver badly enough to reset the GPU
/// and kill the whole gamescope session.
///
/// If this turns out to be wrong for some core, the failure is no longer
/// silent — [`logs::detect_fatal_video_error`] reads RetroArch's own log on
/// exit and the launch-failure dialog names the driver that was tried.
const GAMESCOPE_PROTON_VIDEO_DRIVER: &str = "d3d11";

/// Fullscreen video + input-driver settings, with a Gamescope/Steam-Deck
/// special case (borderless fullscreen, forced video driver, SDL2 input).
fn apply_video_input_overrides(overrides: &mut HashMap<&str, String>, under_proton: bool) {
    if in_gamescope() {
        // Gamescope composites everything as fullscreen. Borderless fullscreen
        // avoids exclusive-mode / resolution-switching failures in a nested
        // compositor. SDL2 input auto-maps the Deck pad.
        // Note this is *not* the last word on video_driver — apply_user_config
        // runs later and overwrites it with glcore for Dolphin + CRT.
        let driver = if under_proton {
            GAMESCOPE_PROTON_VIDEO_DRIVER
        } else {
            GAMESCOPE_NATIVE_VIDEO_DRIVER
        };
        overrides.insert("video_fullscreen", "true".into());
        overrides.insert("video_windowed_fullscreen", "true".into());
        overrides.insert("video_driver", format!("\"{driver}\""));
        overrides.insert("input_joypad_driver", "sdl2".into());
        info!(
            "[RETROARCH] Gamescope detected — borderless fullscreen + {driver} + SDL2 input \
             (under_proton={under_proton})"
        );
    } else {
        overrides.insert("video_fullscreen", "true".into());
    }
}

/// Reads back the `video_driver` Drop settled on, unquoted, for the exit path
/// to quote in a failure message. `None` means Drop wrote no driver and left
/// the choice to RetroArch.
fn resolved_video_driver(overrides: &HashMap<&str, String>) -> Option<String> {
    overrides
        .get("video_driver")
        .map(|v| v.trim_matches('"').to_owned())
        .filter(|v| !v.is_empty())
}

/// Sets the emulated controller device type based on the ROM platform.
///
/// Each libretro core defines its own device IDs; setting the wrong one breaks
/// input or crashes. Only Wii ROMs need an explicit device (Classic Controller
/// Pro = 1281 in Dolphin); GameCube and non-Nintendo cores use their default.
fn apply_controller_device(overrides: &mut HashMap<&str, String>, rom_path: &str) {
    let ext = Path::new(rom_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        // Wii-exclusive formats — Classic Controller Pro for gamepad compat.
        "wbfs" | "wad" | "wia" => {
            overrides.insert("input_libretro_device_p1", "1281".into());
            info!("[RETROARCH] Wii ROM (.{ext}) — device = Classic Controller Pro");
        }
        // .iso can be GC or Wii — sniff the disc.
        "iso" => match cores::detect_iso_disc_type(Path::new(rom_path)) {
            cores::IsoDiscType::Wii => {
                overrides.insert("input_libretro_device_p1", "1281".into());
                info!("[RETROARCH] Wii ISO — device = Classic Controller Pro");
            }
            cores::IsoDiscType::GameCube => {
                info!("[RETROARCH] GameCube ISO — using default GC controller");
            }
            cores::IsoDiscType::Other => {
                info!("[RETROARCH] Non-Nintendo ISO — using core default device");
            }
        },
        // .rvz is almost always GameCube; GC-only formats use the default pad.
        "rvz" | "gcm" | "gcz" | "dol" | "elf" => {
            info!("[RETROARCH] GameCube ROM (.{ext}) — using default GC controller");
        }
        // All other platforms — core default, no override.
        _ => {}
    }
}

/// RetroAchievements config: cheevos on (non-hardcore), rich presence on, and
/// Connect credentials injected when available.
///
/// Returns true when injection was *suppressed* because the token has already
/// been rejected once — the caller uses that to delete the dead `cheevos_token`
/// from the cfg instead of writing it back.
fn apply_cheevos_overrides(
    overrides: &mut HashMap<&str, String>,
    ra_credentials: Option<&RACredentials>,
) -> bool {
    overrides.insert("cheevos_enable", "true".into());
    overrides.insert("cheevos_hardcore_mode_enable", "false".into());
    overrides.insert("cheevos_richpresence_enable", "true".into());
    let Some(creds) = ra_credentials else {
        return false;
    };

    // RetroArch blanks a rejected token in its own config on exit. Writing it
    // back here is what made expiry unrecoverable: the user could never sign
    // in by hand because every launch restored the dead token, and RetroArch
    // reports the failure nowhere the user would look.
    if ra::is_expired_token(&creds.connect_token) {
        warn!(
            "[RETROARCH] RA token for {} was rejected — not injecting it again; \
             RetroArch will ask for its own login until the account is re-linked",
            creds.username
        );
        return true;
    }

    overrides.insert("cheevos_username", format!("\"{}\"", creds.username));
    overrides.insert("cheevos_token", format!("\"{}\"", creds.connect_token));
    info!("[RETROARCH] Injecting RA credentials for user {}", creds.username);
    false
}

/// Applies the per-game user config (controller layout, quality, aspect ratio,
/// CRT shader) into the `retroarch.cfg` overrides map.
#[allow(clippy::too_many_arguments)]
fn apply_user_config(
    cfg: &UserConfiguration,
    overrides: &mut HashMap<&str, String>,
    emu_root: &Path,
    remaps_dir: &Path,
    rom_path: Option<&str>,
    detected_pad: PadFamily,
    crt_shader_path: &mut Option<String>,
    style: PathStyle,
) {
    // Controller layout.
    if let Some(controller) = &cfg.controller_type {
        controllers::apply_controller_mappings(overrides, controller, remaps_dir);
        info!("[RETROARCH] Applied {controller:?} controller layout (user override)");
    } else {
        // "Auto" — clean any stale remap files and bind the pad Drop detected.
        controllers::cleanup_nintendo_remaps(remaps_dir);
        controllers::set_face_button_fallback(overrides, detected_pad);
    }

    // Quality preset (frontend half).
    if let Some(quality) = &cfg.quality_preset {
        presets::apply_quality_preset(overrides, quality);
        info!("[RETROARCH] Applied {quality:?} quality preset");
    }

    // Aspect ratio.
    presets::apply_widescreen(overrides, &cfg.widescreen);
    if cfg.widescreen != AspectRatio::Standard {
        // Integer scaling locks display to the source's native pixel ratio,
        // blocking widescreen — force it off for any non-standard ratio.
        overrides.insert("video_scale_integer", "false".into());
        info!("[RETROARCH] Aspect ratio: {:?} (video_scale_integer forced off)", cfg.widescreen);
    }

    // Fullscreen toggle. apply_video_input_overrides set `video_fullscreen =
    // true` unconditionally as the system default; if the user explicitly
    // opted into windowed mode we flip it here. `video_windowed_fullscreen`
    // (set on Gamescope) is harmless when video_fullscreen is false, so we
    // don't touch it.
    if cfg.fullscreen == Some(false) {
        overrides.insert("video_fullscreen", "false".into());
        info!("[RETROARCH] User opted out of fullscreen — launching windowed");
    }

    // CRT shader.
    let high_res_3d = rom_path.map(cores::rom_implies_high_res_3d_core).unwrap_or(false);
    if cfg.crt_shader {
        if high_res_3d {
            info!("[RETROARCH] High-res 3D core for ROM {rom_path:?} — using resolution-tolerant CRT shader");
        }
        *crt_shader_path = shaders::apply_crt_shader(overrides, emu_root, high_res_3d, style);
        info!("[RETROARCH] CRT shader enabled, path: {crt_shader_path:?}");

        // Dolphin's libretro HW backend renders into whatever context the
        // RetroArch video driver created, and the shader has to be able to
        // sample that context. Every other driver is ruled out:
        //   * D3D11 — RetroArch's compiled-in default on Windows — hands the
        //     shader a context it can't read, giving a black screen,
        //   * the legacy "gl" driver crashes RetroArch outright with this core,
        //   * vulkan is fine for slang presets in general but not for Dolphin's
        //     HW output here.
        // glcore is the only driver where CRT + Dolphin works on every
        // platform, so it is written unconditionally. Deleting this line does
        // not "let RetroArch decide" — it falls back to D3D11 and black-screens.
        //
        // Known cost, deliberately not addressed here: on the Deck this GL path
        // runs through Wine -> Mesa radeonsi and can fault the AMD driver badly
        // enough to reset the GPU and take the whole gamescope session with it.
        // Routing GL through Zink is the fix for that and is being validated
        // separately — don't fold it in here.
        //
        // `rom_runs_on_dolphin` (not `rom_uses_dolphin_core`) so a GameCube/Wii
        // .iso, which the disc sniff sends to Dolphin, gets the same treatment.
        if rom_path.map(cores::rom_runs_on_dolphin).unwrap_or(false) {
            overrides.insert("video_driver", "\"glcore\"".into());
            info!(
                "[RETROARCH] Forcing video_driver=glcore for Dolphin CRT-shader compat (gamescope={})",
                in_gamescope()
            );
        }
    } else {
        shaders::disable_crt_shader(overrides, emu_root);
    }
}

/// Writes per-core quality + widescreen options to `retroarch-core-options.cfg`
/// (and the AppImage copy).
fn write_core_options(
    cfg: &UserConfiguration,
    emu_root: &Path,
    appimage_config_dir: &Option<PathBuf>,
) {
    let core_opts_path = emu_root.join("retroarch-core-options.cfg");
    let mut core_overrides: HashMap<&str, String> = HashMap::new();

    // Both passes skip cores that aren't in cores/, so the key counts logged
    // below describe options a core will really read rather than every key
    // Drop knows about.
    let installed = presets::InstalledCores::scan(emu_root);

    if let Some(quality) = &cfg.quality_preset {
        presets::apply_core_quality_options(&mut core_overrides, quality, &installed);
        info!("[RETROARCH] Patching core options for {quality:?} quality — {} keys", core_overrides.len());
    }
    let before_widescreen = core_overrides.len();
    presets::apply_core_widescreen_options(&mut core_overrides, &cfg.widescreen, &installed);
    if cfg.widescreen != AspectRatio::Standard {
        info!(
            "[RETROARCH] Patched core options for {:?} — {} keys",
            cfg.widescreen,
            core_overrides.len() - before_widescreen
        );
    }

    if core_overrides.is_empty() {
        info!("[RETROARCH] No core options to write");
        return;
    }

    info!("[RETROARCH] Writing core options ({} keys) to {}", core_overrides.len(), core_opts_path.display());
    // Core-options write is best-effort — RetroArch falls back to per-core
    // defaults if the file is missing, so a failure here means quality preset
    // / widescreen hack just won't apply for this launch. Surface it loud
    // (`error!`) so the user can correlate "preset I picked didn't take"
    // with the log, but don't abort the launch over it.
    if let Err(e) = cfg::patch_retroarch_cfg(&core_opts_path, &core_overrides) {
        error!(
            "[RETROARCH] Failed to write core options at {} ({e}) — quality preset and widescreen hack will fall back to core defaults",
            core_opts_path.display()
        );
    }

    if let Some(ai_cfg_dir) = appimage_config_dir {
        let ai_core_opts = ai_cfg_dir.join("retroarch-core-options.cfg");
        if let Err(e) = cfg::patch_retroarch_cfg(&ai_core_opts, &core_overrides) {
            warn!(
                "[RETROARCH] Failed to write AppImage core-options copy at {} ({e}) — primary copy was written, continuing",
                ai_core_opts.display()
            );
        } else {
            info!("[RETROARCH] Also wrote core options to AppImage home: {}", ai_core_opts.display());
        }
    }
}

/// Removes stale per-core / per-game `.opt` and `.cfg` override files under
/// `config/<core>/`. RetroArch's "Save Core/Game Overrides" writes these and
/// they silently outrank `retroarch.cfg` / `retroarch-core-options.cfg` for
/// settings like `aspect_ratio_index` and `video_shader`.
fn clean_stale_per_core_overrides(emu_root: &Path) {
    let per_core_config_dir = emu_root.join("config");
    if !per_core_config_dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(&per_core_config_dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(&path) else { continue };
        for file in files.flatten() {
            let fp = file.path();
            if let Some("opt" | "cfg") = fp.extension().and_then(|e| e.to_str()) { match fs::remove_file(&fp) {
                Ok(_) => info!("[RETROARCH] Removed stale per-core override: {}", fp.display()),
                Err(e) => warn!("[RETROARCH] Failed to remove stale override {}: {e}", fp.display()),
            } }
        }
    }
}

/// Logs a fixed set of key settings so a launch log shows the final config.
fn log_diagnostic_overrides(overrides: &HashMap<&str, String>) {
    const DIAGNOSTIC_KEYS: &[&str] = &[
        "aspect_ratio_index",
        "video_aspect_ratio_auto",
        "input_autodetect_enable",
        "video_shader_enable",
        "auto_shaders_enable",
        "video_fullscreen",
        "video_driver",
        // So Drop's own log says where RetroArch's log went — without this the
        // first step of any crash post-mortem is guessing the path.
        "log_to_file",
        "log_dir",
    ];
    for dk in DIAGNOSTIC_KEYS {
        if let Some(val) = overrides.get(dk) {
            info!("[RETROARCH] config: {dk} = {val}");
        }
    }
}
