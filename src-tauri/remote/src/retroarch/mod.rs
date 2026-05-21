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
//! * [`cfg`]        — the `key = "value"` config-file patch primitives.
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
pub mod presets;
pub mod ra;
pub mod shaders;

use database::models::data::{AspectRatio, ControllerType, UserConfiguration};
use log::{info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// Re-export the public surface so existing `remote::retroarch::*` call sites
// in the `process` crate and the root crate keep compiling without edits.
pub use cores::{resolve_core_for_rom, EXTENSION_CORE_MAP};
pub use ra::{
    check_rom_hash, fetch_ra_credentials, hash_rom, RACredentials, RAHashEntry, RAHashesResponse,
    RomHashStatus,
};

/// Directory (relative to the RetroArch install root) that per-game saves go
/// into. The cloud-save system looks for `<emulator_install>/drop-saves/<id>/`.
const DROP_SAVES_DIR: &str = "drop-saves";

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
}

/// Detects whether the emulator at `emulator_install_dir` is RetroArch and,
/// if so, writes/patches its config for a zero-config launch.
///
/// `game_id` keys the per-game save directory. `ra_credentials`, when present,
/// is injected so RetroArch logs into RetroAchievements automatically.
/// `user_config` carries the per-game controller / quality / aspect-ratio /
/// CRT choices. `rom_path` is used to scope BIOS warnings and pick a
/// shader/controller-device appropriate to the ROM's platform.
///
/// Returns `Some(RetroArchInfo)` if RetroArch was detected and configured, or
/// `None` if this is not a RetroArch install.
pub fn configure_retroarch_for_game(
    emulator_install_dir: &str,
    game_id: &str,
    ra_credentials: Option<&RACredentials>,
    user_config: Option<&UserConfiguration>,
    rom_path: Option<&str>,
) -> Option<RetroArchInfo> {
    let emu_root = PathBuf::from(emulator_install_dir);

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

    info!("[RETROARCH] Detected RetroArch in {emulator_install_dir}, configuring for game {game_id}");

    // Absolute paths for every directory RetroArch needs.
    let cores_dir = emu_root.join("cores");
    let system_dir = emu_root.join("system");
    let assets_dir = emu_root.join("assets");
    let saves_base = emu_root.join(DROP_SAVES_DIR).join(game_id);
    let savefile_dir = saves_base.join("saves");
    let savestate_dir = saves_base.join("states");

    for dir in [&savefile_dir, &savestate_dir, &system_dir] {
        if let Err(e) = fs::create_dir_all(dir) {
            warn!("[RETROARCH] Failed to create dir {}: {e}", dir.display());
        }
    }

    // ── BIOS detection & auto-copy ───────────────────────────────────────
    let current_rom_ext: Option<String> = rom_path
        .and_then(|rp| Path::new(rp).extension())
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    let bios_warnings = bios::check_and_place_bios(&system_dir, current_rom_ext.as_deref());

    // ── retroarch.cfg overrides ──────────────────────────────────────────
    let mut overrides: HashMap<&str, String> = HashMap::new();
    let remaps_dir = emu_root.join("config").join("remaps");
    let core_opts_file = emu_root.join("retroarch-core-options.cfg");

    apply_path_overrides(&mut overrides, &cores_dir, &system_dir, &assets_dir, &emu_root);
    apply_save_overrides(&mut overrides, &savefile_dir, &savestate_dir, &saves_base);
    apply_baseline_overrides(&mut overrides, &remaps_dir, &core_opts_file);
    apply_video_input_overrides(&mut overrides);

    // Emulated controller device type, scoped to the ROM's platform.
    if let Some(rp) = rom_path {
        apply_controller_device(&mut overrides, rp);
    }

    // RetroAchievements — enable cheevos + inject Connect credentials.
    apply_cheevos_overrides(&mut overrides, ra_credentials);

    controllers::apply_hotkey_bindings(&mut overrides);

    // ── Per-game user config ─────────────────────────────────────────────
    let mut crt_shader_path: Option<String> = None;
    if let Some(cfg) = user_config {
        apply_user_config(cfg, &mut overrides, &emu_root, &remaps_dir, rom_path, &mut crt_shader_path);
    }

    log_diagnostic_overrides(&overrides);

    // ── Write the main config (used by --appendconfig) ───────────────────
    let cfg_path = emu_root.join("retroarch.cfg");
    info!("[RETROARCH] Writing retroarch.cfg ({} keys) to {}", overrides.len(), cfg_path.display());
    cfg::patch_retroarch_cfg_with_deletions(&cfg_path, &overrides, controllers::STALE_INPUT_KEYS);

    // Also write to the AppImage portable $HOME so our settings are the BASE
    // config, not just an --appendconfig overlay (critical on Steam Deck).
    let appimage_config_dir = discovery::find_appimage_config_dir(&emu_root);
    if let Some(ai_cfg_dir) = &appimage_config_dir {
        if let Err(e) = fs::create_dir_all(ai_cfg_dir) {
            warn!("[RETROARCH] Failed to create AppImage config dir {}: {e}", ai_cfg_dir.display());
        } else {
            let ai_cfg_path = ai_cfg_dir.join("retroarch.cfg");
            cfg::patch_retroarch_cfg_with_deletions(&ai_cfg_path, &overrides, controllers::STALE_INPUT_KEYS);
            info!("[RETROARCH] Also wrote config to AppImage home: {}", ai_cfg_path.display());
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
) {
    overrides.insert("libretro_directory", cfg::path_to_cfg(cores_dir));
    overrides.insert("system_directory", cfg::path_to_cfg(system_dir));
    overrides.insert("assets_directory", cfg::path_to_cfg(assets_dir));
    overrides.insert("rgui_browser_directory", cfg::path_to_cfg(emu_root));
}

/// Per-game save isolation. Drop manages save paths itself, so RetroArch's own
/// content-directory sorting is disabled.
fn apply_save_overrides(
    overrides: &mut HashMap<&str, String>,
    savefile_dir: &Path,
    savestate_dir: &Path,
    saves_base: &Path,
) {
    overrides.insert("savefile_directory", cfg::path_to_cfg(savefile_dir));
    overrides.insert("savestate_directory", cfg::path_to_cfg(savestate_dir));
    overrides.insert("screenshot_directory", cfg::path_to_cfg(&saves_base.join("screenshots")));
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
) {
    overrides.insert("input_autodetect_enable", "true".into());
    overrides.insert("pause_nonactive", "false".into());
    overrides.insert("menu_driver", "ozone".into());
    overrides.insert("video_font_enable", "true".into()); // RA unlock toasts
    overrides.insert("quit_press_twice", "false".into());

    // Core-specific input remaps (Nintendo A<->B swap etc.).
    overrides.insert("input_remap_binds_enable", "true".into());
    overrides.insert("input_autoload_remaps", "true".into());
    overrides.insert("remaps_directory", cfg::path_to_cfg(remaps_dir));

    // global_core_options stops RetroArch writing per-core .opt files that
    // would outrank our core_options_path after the first launch.
    overrides.insert("global_core_options", "true".into());
    overrides.insert("core_options_path", cfg::path_to_cfg(core_opts_file));
}

/// Fullscreen video + input-driver settings, with a Gamescope/Steam-Deck
/// special case (borderless fullscreen, forced Vulkan, SDL2 input).
fn apply_video_input_overrides(overrides: &mut HashMap<&str, String>) {
    #[cfg(target_os = "linux")]
    let in_gamescope = std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
        || std::env::var("SteamGamepadUI").is_ok();
    #[cfg(not(target_os = "linux"))]
    let in_gamescope = false;

    if in_gamescope {
        // Gamescope composites everything as fullscreen. Borderless fullscreen
        // avoids exclusive-mode / resolution-switching failures in a nested
        // compositor. Vulkan is forced because the AppImage's bundled Mesa was
        // too old for RDNA2 auto-detection; SDL2 input auto-maps the Deck pad.
        overrides.insert("video_fullscreen", "true".into());
        overrides.insert("video_windowed_fullscreen", "true".into());
        overrides.insert("video_driver", "vulkan".into());
        overrides.insert("input_joypad_driver", "sdl2".into());
        info!("[RETROARCH] Gamescope detected — borderless fullscreen + vulkan + SDL2 input");
    } else {
        overrides.insert("video_fullscreen", "true".into());
    }
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
fn apply_cheevos_overrides(overrides: &mut HashMap<&str, String>, ra_credentials: Option<&RACredentials>) {
    overrides.insert("cheevos_enable", "true".into());
    overrides.insert("cheevos_hardcore_mode_enable", "false".into());
    overrides.insert("cheevos_richpresence_enable", "true".into());
    if let Some(creds) = ra_credentials {
        overrides.insert("cheevos_username", format!("\"{}\"", creds.username));
        overrides.insert("cheevos_token", format!("\"{}\"", creds.connect_token));
        info!("[RETROARCH] Injecting RA credentials for user {}", creds.username);
    }
}

/// Applies the per-game user config (controller layout, quality, aspect ratio,
/// CRT shader) into the `retroarch.cfg` overrides map.
fn apply_user_config(
    cfg: &UserConfiguration,
    overrides: &mut HashMap<&str, String>,
    emu_root: &Path,
    remaps_dir: &Path,
    rom_path: Option<&str>,
    crt_shader_path: &mut Option<String>,
) {
    // Controller layout.
    if let Some(controller) = &cfg.controller_type {
        controllers::apply_controller_mappings(overrides, controller, remaps_dir);
        info!("[RETROARCH] Applied {controller:?} controller layout");
    } else {
        // "Auto" — clean any stale remap files and set the XInput fallback.
        controllers::cleanup_nintendo_remaps(remaps_dir);
        controllers::set_xinput_positional_fallback(overrides);
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

    // CRT shader.
    let high_res_3d = rom_path.map(cores::rom_implies_high_res_3d_core).unwrap_or(false);
    if cfg.crt_shader {
        if high_res_3d {
            info!("[RETROARCH] High-res 3D core for ROM {rom_path:?} — using resolution-tolerant CRT shader");
        }
        *crt_shader_path = shaders::apply_crt_shader(overrides, emu_root, high_res_3d);
        info!("[RETROARCH] CRT shader enabled, path: {crt_shader_path:?}");

        // Dolphin's libretro HW backend follows RetroArch's video driver; on
        // Vulkan/D3D11 it renders into a context the slang shader can't see,
        // giving a black screen. glcore lets the shader wrap Dolphin's output.
        if rom_path.map(cores::rom_uses_dolphin_core).unwrap_or(false) {
            overrides.insert("video_driver", "\"glcore\"".into());
            overrides.insert("dolphin_renderer", "\"Hardware\"".into());
            info!("[RETROARCH] Forcing video_driver=glcore + dolphin_renderer=Hardware for CRT-shader compat");
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

    if let Some(quality) = &cfg.quality_preset {
        presets::apply_core_quality_options(&mut core_overrides, quality);
        info!("[RETROARCH] Patching core options for {quality:?} quality — {} keys", core_overrides.len());
    }
    presets::apply_core_widescreen_options(&mut core_overrides, &cfg.widescreen);
    if cfg.widescreen != AspectRatio::Standard {
        info!("[RETROARCH] Patched core options for {:?}", cfg.widescreen);
    }

    if core_overrides.is_empty() {
        info!("[RETROARCH] No core options to write");
        return;
    }

    info!("[RETROARCH] Writing core options ({} keys) to {}", core_overrides.len(), core_opts_path.display());
    cfg::patch_retroarch_cfg(&core_opts_path, &core_overrides);

    if let Some(ai_cfg_dir) = appimage_config_dir {
        let ai_core_opts = ai_cfg_dir.join("retroarch-core-options.cfg");
        cfg::patch_retroarch_cfg(&ai_core_opts, &core_overrides);
        info!("[RETROARCH] Also wrote core options to AppImage home: {}", ai_core_opts.display());
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
            match fp.extension().and_then(|e| e.to_str()) {
                Some("opt" | "cfg") => match fs::remove_file(&fp) {
                    Ok(_) => info!("[RETROARCH] Removed stale per-core override: {}", fp.display()),
                    Err(e) => warn!("[RETROARCH] Failed to remove stale override {}: {e}", fp.display()),
                },
                _ => {}
            }
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
    ];
    for dk in DIAGNOSTIC_KEYS {
        if let Some(val) = overrides.get(dk) {
            info!("[RETROARCH] config: {dk} = {val}");
        }
    }
}
