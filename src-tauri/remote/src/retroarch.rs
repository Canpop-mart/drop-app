//! RetroArch pre-launch configuration.
//!
//! When a game uses RetroArch as its emulator, Drop writes a
//! `retroarch.cfg` (or patches the existing one) so that:
//!
//! - Core directory, system/BIOS directory, and autoconfig directory
//!   point to the correct absolute paths inside the RetroArch install.
//! - Save files and save states are placed in a per-game directory
//!   (`drop-saves/<game_id>/`) inside the RetroArch install, making
//!   them easy to locate for cloud save sync.
//! - Controller autodetect is enabled so gamepads work out of the box.
//! - Fullscreen is enabled for a console-like experience.
//!
//! The module follows the same pattern as `goldberg.rs`: called from
//! the process manager before spawning the child process.

use database::models::data::{AspectRatio, ControllerType, QualityPreset, UserConfiguration};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::requests::{generate_url, make_authenticated_get};

/// Directories that RetroArch saves into, relative to its install root.
/// The cloud save system can look for `<emulator_install>/drop-saves/<game_id>/`
/// to find per-game saves.
const DROP_SAVES_DIR: &str = "drop-saves";

/// RetroAchievements Connect credentials for RetroArch authentication.
#[derive(Debug, Clone)]
pub struct RACredentials {
    /// RA username (used as `cheevos_username`).
    pub username: String,
    /// Connect token from `dorequest.php?r=login2` (used as `cheevos_token`).
    pub connect_token: String,
}

/// Fetches RetroAchievements Connect credentials from the Drop server.
/// Returns `None` if the user has no linked RA account or no Connect token.
pub async fn fetch_ra_credentials() -> Option<RACredentials> {
    let url = match generate_url(&["api", "v1", "client", "user", "ra-credentials"], &[]) {
        Ok(u) => u,
        Err(e) => {
            debug!("[RETROARCH] Failed to build RA credentials URL: {}", e);
            return None;
        }
    };

    let response = match make_authenticated_get(url).await {
        Ok(r) => r,
        Err(e) => {
            debug!("[RETROARCH] Failed to fetch RA credentials: {}", e);
            return None;
        }
    };

    if !response.status().is_success() {
        debug!(
            "[RETROARCH] RA credentials endpoint returned {}",
            response.status()
        );
        return None;
    }

    #[derive(serde::Deserialize)]
    struct RACreds {
        username: String,
        #[serde(rename = "connectToken")]
        connect_token: String,
    }

    match response.json::<RACreds>().await {
        Ok(creds) if !creds.connect_token.is_empty() => {
            info!(
                "[RETROARCH] Got RA credentials for user {}",
                creds.username
            );
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
            warn!("[RETROARCH] Failed to parse RA credentials: {}", e);
            None
        }
    }
}

/// Result of configuring RetroArch for a game launch.
#[derive(Debug, Clone)]
pub struct RetroArchInfo {
    /// Absolute path to the save file directory for this game.
    pub savefile_directory: String,
    /// Absolute path to the save state directory for this game.
    pub savestate_directory: String,
    /// BIOS warnings for the frontend to display (empty if all OK).
    pub bios_warnings: Vec<String>,
    /// CRT shader path if enabled and found, None otherwise.
    pub crt_shader_path: Option<String>,
}

/// Detects whether the emulator at `emulator_install_dir` is RetroArch
/// and, if so, writes/patches its config for a zero-config launch.
///
/// `game_id` is the ROM game's ID, used to create a per-game save directory.
/// `ra_credentials` optionally provides RA Connect credentials so RetroArch
/// can authenticate with RetroAchievements without manual login.
///
/// Returns `Some(RetroArchInfo)` if RetroArch was detected and configured,
/// or `None` if this isn't a RetroArch installation.
pub fn configure_retroarch_for_game(
    emulator_install_dir: &str,
    game_id: &str,
    ra_credentials: Option<&RACredentials>,
    user_config: Option<&UserConfiguration>,
    rom_path: Option<&str>,
) -> Option<RetroArchInfo> {
    let emu_root = PathBuf::from(emulator_install_dir);

    if !is_retroarch(&emu_root) {
        warn!(
            "[RETROARCH] No RetroArch detected in {} — checked for: retroarch, retroarch.exe, retroarch.AppImage, retroarch.cfg, cores/ dir. Skipping config.",
            emulator_install_dir
        );
        // Log what files DO exist in emu_root for debugging
        if let Ok(entries) = fs::read_dir(&emu_root) {
            let files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            warn!("[RETROARCH] Files in {}: {:?}", emulator_install_dir, files);
        }
        return None;
    }

    info!(
        "[RETROARCH] Detected RetroArch in {}, configuring for game {}",
        emulator_install_dir, game_id
    );

    // Build absolute paths for all directories RetroArch needs
    let cores_dir = emu_root.join("cores");
    let system_dir = emu_root.join("system");
    let assets_dir = emu_root.join("assets");

    // Per-game save directories under drop-saves/<game_id>/
    let saves_base = emu_root.join(DROP_SAVES_DIR).join(game_id);
    let savefile_dir = saves_base.join("saves");
    let savestate_dir = saves_base.join("states");

    // Ensure save directories exist
    if let Err(e) = fs::create_dir_all(&savefile_dir) {
        warn!(
            "[RETROARCH] Failed to create save dir {}: {}",
            savefile_dir.display(),
            e
        );
    }
    if let Err(e) = fs::create_dir_all(&savestate_dir) {
        warn!(
            "[RETROARCH] Failed to create savestate dir {}: {}",
            savestate_dir.display(),
            e
        );
    }

    // Ensure system dir exists (for BIOS files)
    if let Err(e) = fs::create_dir_all(&system_dir) {
        warn!(
            "[RETROARCH] Failed to create system dir {}: {}",
            system_dir.display(),
            e
        );
    }

    // Track BIOS warnings to surface to the frontend
    let mut bios_warnings: Vec<String> = Vec::new();
    let mut crt_shader_path: Option<String> = None;

    // ── BIOS detection & auto-copy ─────────────────────────────────────
    // Many libretro cores require BIOS/firmware files in specific subdirs
    // under system/. Users often drop them in system/ directly, so we
    // detect them there and auto-copy to the correct location.
    //
    // Each entry: (label, core_subdir relative to system/, match fn, example, severity)
    //   core_subdir: where the core actually looks (None = system/ root)
    //   match_fn:    returns true if a filename is a BIOS for this system
    //   example:     shown in the warning message
    //   severity:    "crash" (game won't boot) or "warn" (may black-screen)
    struct BiosSpec {
        label: &'static str,
        /// ROM file extensions that use this system's BIOS.
        /// Only warn if the current game matches one of these.
        /// Empty slice = always check (shared emulator setup).
        rom_extensions: &'static [&'static str],
        /// Subdirectory under system/ where the core looks. None = root.
        core_subdir: Option<&'static str>,
        /// Returns true if `lowercase_filename` is a BIOS for this system.
        matches: fn(&str) -> bool,
        example: &'static str,
        severity: &'static str,
    }

    fn is_ps1_bios(name: &str) -> bool {
        (name == "scph5501.bin" || name == "scph1001.bin" || name == "psxonpsp660.bin")
    }
    fn is_ps2_bios(name: &str) -> bool {
        (name.starts_with("scph") || name.starts_with("ps2"))
            && name.ends_with(".bin")
            && !name.contains("scph5501") && !name.contains("scph1001")
    }
    fn is_nds_bios(name: &str) -> bool {
        name == "bios7.bin" || name == "bios9.bin" || name == "firmware.bin"
    }
    fn is_segacd_bios(name: &str) -> bool {
        name.starts_with("bios_cd_") && name.ends_with(".bin")
            || name == "bios_md.bin"
    }
    fn is_saturn_bios(name: &str) -> bool {
        name == "sega_101.bin" || name == "mpr-17933.bin"
            || name == "saturn_bios.bin"
    }
    fn is_gba_bios(name: &str) -> bool {
        name == "gba_bios.bin"
    }

    let bios_specs: &[BiosSpec] = &[
        BiosSpec {
            label: "PS1",
            rom_extensions: &["cue", "bin", "chd", "pbp"],
            core_subdir: None,
            matches: is_ps1_bios,
            example: "scph5501.bin or scph1001.bin",
            severity: "warn",
        },
        BiosSpec {
            label: "PS2",
            rom_extensions: &["iso", "chd", "cue", "bin"],
            core_subdir: Some("pcsx2/bios"),
            matches: is_ps2_bios,
            example: "SCPH-70012.bin",
            severity: "crash",
        },
        BiosSpec {
            label: "NDS",
            rom_extensions: &["nds"],
            core_subdir: None,
            matches: is_nds_bios,
            example: "bios7.bin, bios9.bin, firmware.bin",
            severity: "warn",
        },
        BiosSpec {
            label: "Sega CD",
            rom_extensions: &["chd", "cue", "bin"],
            core_subdir: None,
            matches: is_segacd_bios,
            example: "bios_cd_u.bin",
            severity: "warn",
        },
        BiosSpec {
            label: "Saturn",
            rom_extensions: &["chd", "cue", "bin"],
            core_subdir: None,
            matches: is_saturn_bios,
            example: "sega_101.bin or mpr-17933.bin",
            severity: "warn",
        },
        BiosSpec {
            label: "GBA",
            rom_extensions: &["gba"],
            core_subdir: None,
            matches: is_gba_bios,
            example: "gba_bios.bin",
            severity: "warn",
        },
    ];

    // Determine the current ROM extension so we only warn about relevant BIOS
    let current_rom_ext: Option<String> = rom_path
        .and_then(|rp| Path::new(rp).extension())
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    for spec in bios_specs {
        // Skip BIOS checks that aren't relevant to the current game
        if !spec.rom_extensions.is_empty() {
            if let Some(ref ext) = current_rom_ext {
                if !spec.rom_extensions.contains(&ext.as_str()) {
                    continue;
                }
            }
        }

        let target_dir = match spec.core_subdir {
            Some(sub) => system_dir.join(sub),
            None => system_dir.clone(),
        };

        // Ensure target directory exists (only matters for subdirs)
        if spec.core_subdir.is_some() {
            if let Err(e) = fs::create_dir_all(&target_dir) {
                warn!("[RETROARCH] Failed to create {} BIOS dir {}: {}", spec.label, target_dir.display(), e);
            }
        }

        // Check if BIOS already present in the correct location
        let has_bios_in_target = fs::read_dir(&target_dir)
            .into_iter()
            .flat_map(|entries| entries.filter_map(|e| e.ok()))
            .any(|e| (spec.matches)(&e.file_name().to_string_lossy().to_lowercase()));

        if !has_bios_in_target && spec.core_subdir.is_some() {
            // Core expects a subdirectory — check system/ root and auto-copy
            let root_bios: Vec<_> = fs::read_dir(&system_dir)
                .into_iter()
                .flat_map(|entries| entries.filter_map(|e| e.ok()))
                .filter(|e| (spec.matches)(&e.file_name().to_string_lossy().to_lowercase()))
                .collect();

            if !root_bios.is_empty() {
                info!(
                    "[RETROARCH] Found {} {} BIOS file(s) in system/ — copying to {} where the core expects them",
                    root_bios.len(), spec.label, target_dir.display()
                );
                for entry in &root_bios {
                    let dest = target_dir.join(entry.file_name());
                    if !dest.exists() {
                        match fs::copy(entry.path(), &dest) {
                            Ok(_) => info!("[RETROARCH] Copied BIOS: {} → {}", entry.path().display(), dest.display()),
                            Err(e) => warn!("[RETROARCH] Failed to copy BIOS {} → {}: {}", entry.path().display(), dest.display(), e),
                        }
                    }
                }
            }
        }

        // Re-check after potential copy
        let final_dir = match spec.core_subdir {
            Some(sub) => system_dir.join(sub),
            None => system_dir.clone(),
        };
        let has_bios = fs::read_dir(&final_dir)
            .into_iter()
            .flat_map(|entries| entries.filter_map(|e| e.ok()))
            .any(|e| (spec.matches)(&e.file_name().to_string_lossy().to_lowercase()));

        if !has_bios {
            let action = if spec.severity == "crash" {
                "will crash on launch"
            } else {
                "may not boot correctly"
            };
            let msg = format!(
                "No {} BIOS found. {} games {}. Place {} in {}",
                spec.label, spec.label, action, spec.example, final_dir.display()
            );
            warn!("[RETROARCH] {}", msg);
            bios_warnings.push(msg);
        } else {
            info!("[RETROARCH] {} BIOS found in {}", spec.label, final_dir.display());
        }
    }

    // NOTE: We do NOT create or override joypad_autoconfig_dir. The
    // RetroArch AppImage bundles its own autoconfig profiles internally.
    // Overriding the path to an empty directory causes "not configured —
    // using fallback" messages because no profiles are found.

    // Build the config overrides
    let mut overrides: HashMap<&str, String> = HashMap::new();

    // Core/system paths
    overrides.insert("libretro_directory", path_to_cfg(&cores_dir));
    overrides.insert("system_directory", path_to_cfg(&system_dir));
    // joypad_autoconfig_dir is intentionally NOT overridden — the AppImage
    // bundles its own profiles (e.g. "Valve Software Steam Controller").
    overrides.insert("assets_directory", path_to_cfg(&assets_dir));

    // Save paths — per-game isolation
    overrides.insert("savefile_directory", path_to_cfg(&savefile_dir));
    overrides.insert("savestate_directory", path_to_cfg(&savestate_dir));
    overrides.insert(
        "screenshot_directory",
        path_to_cfg(&saves_base.join("screenshots")),
    );

    // Sort saves into content directories is OFF since we manage paths ourselves
    overrides.insert("sort_savefiles_enable", "false".into());
    overrides.insert("sort_savestates_enable", "false".into());
    overrides.insert("sort_savefiles_by_content_enable", "false".into());
    overrides.insert("sort_savestates_by_content_enable", "false".into());

    // Controller autoconfig
    overrides.insert("input_autodetect_enable", "true".into());

    // Sane defaults for a "just works" experience
    overrides.insert("pause_nonactive", "false".into());
    overrides.insert("savestate_auto_save", "false".into());
    overrides.insert("savestate_auto_load", "false".into());

    // ── Gamescope / Steam Deck ──────────────────────────────────────────
    // Gamescope (SteamOS Gaming Mode) is a nested Wayland compositor that
    // composites every window as fullscreen. We use borderless fullscreen
    // (video_fullscreen + video_windowed_fullscreen) so RetroArch fills
    // the Gamescope surface without trying exclusive mode or resolution
    // switching. Also force Vulkan so cores that default to OpenGL
    // (e.g. mupen64plus_next) get a visible surface, and SDL2 input so
    // the Steam Deck controls are auto-detected.
    #[cfg(target_os = "linux")]
    let in_gamescope = std::env::var("GAMESCOPE_WAYLAND_DISPLAY").is_ok()
        || std::env::var("SteamGamepadUI").is_ok();
    #[cfg(not(target_os = "linux"))]
    let in_gamescope = false;

    if in_gamescope {
        // Gamescope (SteamOS Gaming Mode) composites everything as
        // fullscreen. Use BORDERLESS fullscreen so RetroArch fills the
        // Gamescope surface without attempting exclusive mode or
        // resolution switching (which can fail in a nested compositor).
        overrides.insert("video_fullscreen", "true".into());
        overrides.insert("video_windowed_fullscreen", "true".into());
        // Do NOT force a specific video_driver — let RetroArch auto-detect
        // the best driver for the system. The AppImage may bundle its own
        // mesa/vulkan libraries; forcing "glcore" or "vulkan" can cause
        // silent failures where audio works but no video surface is created.
        // RetroArch tries drivers in order: vulkan → gl → glcore → sdl2.
        //
        // SDL2 joypad driver has built-in Xbox/Steam Deck controller
        // mappings via gamecontrollerdb — no autoconfig profiles needed.
        overrides.insert("input_joypad_driver", "sdl2".into());
        info!("[RETROARCH] Gamescope detected — borderless fullscreen + auto video driver + SDL2 input");
    } else {
        overrides.insert("video_fullscreen", "true".into());
    }

    // Modern menu driver — controller-friendly if user opens the menu
    overrides.insert("menu_driver", "ozone".into());

    // Content browser starts at the emulator root
    overrides.insert("rgui_browser_directory", path_to_cfg(&emu_root));

    // OSD text — needed for RetroAchievements unlock toasts
    overrides.insert("video_font_enable", "true".into());

    // Single press to quit back to Drop (no double-tap confirmation)
    overrides.insert("quit_press_twice", "false".into());

    // Set the emulated controller device type based on the ROM platform.
    // Each libretro core defines its own device types — setting the wrong
    // one can cause broken input or crashes.
    //
    // Device IDs (Dolphin-specific):
    //   1=Wiimote, 513=Wiimote(SW), 769=Wiimote+Nunchuk,
    //   1025=Classic Controller, 1281=Classic Controller Pro, 1537=GC on Wii
    //
    // Previously this was hardcoded to 1281 (Classic Controller Pro) for ALL
    // games, which broke GameCube titles (they need the default GC controller)
    // and was meaningless for non-Dolphin cores (N64, PS1, PS2, etc.).
    if let Some(rp) = rom_path {
        let ext = Path::new(rp)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            // Wii-exclusive formats: use Classic Controller Pro for gamepad compat
            "wbfs" | "wad" | "wia" => {
                overrides.insert("input_libretro_device_p1", "1281".into());
                info!("[RETROARCH] Wii ROM detected (.{}) — device = Classic Controller Pro", ext);
            }
            // .rvz can be GC or Wii; .iso can be anything. Check disc type.
            "rvz" | "iso" => {
                // For .iso we already detected disc type in resolve_core_for_rom,
                // but configure runs separately. Re-detect (fast: 32-byte read).
                if ext == "iso" {
                    match detect_iso_disc_type(Path::new(rp)) {
                        IsoDiscType::Wii => {
                            overrides.insert("input_libretro_device_p1", "1281".into());
                            info!("[RETROARCH] Wii ISO detected — device = Classic Controller Pro");
                        }
                        IsoDiscType::GameCube => {
                            info!("[RETROARCH] GameCube ISO detected — using default GC controller");
                            // Don't set device — let Dolphin use its default (GC pad)
                        }
                        IsoDiscType::Other => {
                            info!("[RETROARCH] Non-Nintendo ISO detected — using core default device");
                            // PS2/PSP/Saturn — let the core decide
                        }
                    }
                } else {
                    // .rvz: almost always GC. Wii typically uses WBFS/WIA.
                    // Don't force device type — let Dolphin auto-detect.
                    info!("[RETROARCH] RVZ ROM — using Dolphin default device");
                }
            }
            // GameCube-only formats: let Dolphin use default GC controller
            "gcm" | "gcz" | "dol" | "elf" => {
                info!("[RETROARCH] GameCube ROM detected (.{}) — using default GC controller", ext);
            }
            // All other platforms (N64, PS1, PS2, GBA, etc.): no override needed
            _ => {}
        }
    }

    // Enable core-specific input remaps (for Nintendo A↔B swap etc.)
    // input_remap_binds_enable allows remaps to take effect at all,
    // input_autoload_remaps tells RetroArch to auto-load .rmp files
    // from the remaps directory when a core starts.
    let remaps_dir = emu_root.join("config").join("remaps");
    overrides.insert("input_remap_binds_enable", "true".into());
    overrides.insert("input_autoload_remaps", "true".into());
    overrides.insert("remaps_directory", path_to_cfg(&remaps_dir));

    // Point core options to our file so --appendconfig picks up the right path.
    // global_core_options = true prevents RetroArch from creating per-core
    // .opt files (e.g. config/dolphin-emu/dolphin-emu.opt) that would
    // override our core_options_path. Without this, after the first launch
    // the per-core file takes precedence and our quality settings are ignored.
    // See: https://github.com/libretro/RetroArch/issues/12901
    let core_opts_file = emu_root.join("retroarch-core-options.cfg");
    overrides.insert("global_core_options", "true".into());
    overrides.insert("core_options_path", path_to_cfg(&core_opts_file));

    // RetroAchievements — enable cheevos so RetroArch handles in-game
    // unlock popups. If we have Connect credentials, inject them so
    // RetroArch authenticates automatically without manual login.
    overrides.insert("cheevos_enable", "true".into());
    overrides.insert("cheevos_hardcore_mode_enable", "false".into());
    overrides.insert("cheevos_richpresence_enable", "true".into());

    if let Some(creds) = ra_credentials {
        overrides.insert("cheevos_username", format!("\"{}\"", creds.username));
        overrides.insert("cheevos_token", format!("\"{}\"", creds.connect_token));
        info!(
            "[RETROARCH] Injecting RA credentials for user {}",
            creds.username
        );
    }

    // ── Hotkey bindings ────────────────────────────────────────────────────
    // Keyboard hotkeys work on all platforms (desktop + Steam Deck w/ virtual KB).
    // RetroArch defaults: Escape=quit, F2=save, F4=load, space=fast-forward.
    // We set them explicitly so they survive any base config that disables them.
    overrides.insert("input_exit_emulator", "escape".into());
    overrides.insert("input_save_state", "f2".into());
    overrides.insert("input_load_state", "f4".into());
    overrides.insert("input_toggle_fast_forward", "space".into());
    overrides.insert("input_state_slot_increase", "f7".into());
    overrides.insert("input_state_slot_decrease", "f6".into());

    // Controller button combos — hold R3 (right stick click) + press a
    // button for quick actions without opening the RetroArch menu.
    //
    // Button indices differ by input driver:
    //   SDL2 (Linux):  R3=8  Start=6  L1=9  R1=10  R2(btn)=5  DL=13 DR=14
    //   XInput (Win):  R3=9  Start=7  LB=4  RB=5   RT=axis+5
    //   (XInput DPad = hat, not buttons; RT = analog axis, use +5)
    //
    // Combos (hold R3 + press):
    //   R3 + Start       → Quit RetroArch
    //   R3 + R1/RB       → Save state
    //   R3 + L1/LB       → Load state
    //   R3 + RT/R2       → Toggle fast forward
    //   R3 + DPad R/L    → Save slot nav (Linux only; use F6/F7 on Windows)
    #[cfg(target_os = "linux")]
    {
        overrides.insert("input_enable_hotkey_btn", "8".into());     // R3
        overrides.insert("input_exit_emulator_btn", "6".into());     // Start
        overrides.insert("input_save_state_btn", "10".into());       // R1
        overrides.insert("input_load_state_btn", "9".into());        // L1
        overrides.insert("input_toggle_fast_forward_btn", "5".into()); // R2 (as button)
        overrides.insert("input_state_slot_increase_btn", "14".into()); // DPad Right
        overrides.insert("input_state_slot_decrease_btn", "13".into()); // DPad Left
    }
    #[cfg(not(target_os = "linux"))]
    {
        overrides.insert("input_enable_hotkey_btn", "9".into());     // R3
        overrides.insert("input_exit_emulator_btn", "7".into());     // Start
        overrides.insert("input_save_state_btn", "5".into());        // RB
        overrides.insert("input_load_state_btn", "4".into());        // LB
        overrides.insert("input_toggle_fast_forward_axis", "+5".into()); // RT (analog axis on XInput)
        // DPad doesn't register as buttons on XInput — use F6/F7 keyboard for slot nav
    }
    info!("[RETROARCH] Applied hotkey bindings (keyboard + R3 controller combos)");

    // ── Controller layout mapping ────────────────────────────────────────
    if let Some(cfg) = user_config {
        if let Some(controller) = &cfg.controller_type {
            apply_controller_mappings(&mut overrides, controller, &remaps_dir);
            info!("[RETROARCH] Applied {:?} controller layout", controller);
        } else {
            // No controller type selected ("Auto") — clean up any stale remap files
            // that may override autoconfig when user switches back from Nintendo mode.
            cleanup_nintendo_remaps(&remaps_dir);
        }

        // ── Quality preset (retroarch.cfg portion) ──────────────────────
        if let Some(quality) = &cfg.quality_preset {
            apply_quality_preset(&mut overrides, quality);
            info!("[RETROARCH] Applied {:?} quality preset", quality);
        }

        // ── Aspect ratio ────────────────────────────────────────────────
        apply_widescreen(&mut overrides, &cfg.widescreen);
        if cfg.widescreen != AspectRatio::Standard {
            info!("[RETROARCH] Aspect ratio: {:?}", cfg.widescreen);
        }

        // ── CRT shader toggle ──────────────────────────────────────────
        if cfg.crt_shader {
            crt_shader_path = apply_crt_shader(&mut overrides, &emu_root);
            info!("[RETROARCH] CRT shader enabled, path: {:?}", crt_shader_path);
        }
    }

    // Keys to DELETE from the config file. These are stale settings from
    // previous Drop versions that interfere with RetroArch's built-in defaults.
    let stale_keys: &[&str] = &[
        // Old empty autoconfig dir caused "not configured" fallback warnings
        "joypad_autoconfig_dir",
        // Old Nintendo mode manually mapped all axes/buttons/triggers.
        // These stale keys override autoconfig and break sticks if left behind.
        // NOTE: input_autodetect_enable is NOT deleted — it's set to "true"
        // in the overrides above and must remain in the config.
        "input_player1_l_btn",
        "input_player1_r_btn",
        "input_player1_select_btn",
        "input_player1_start_btn",
        "input_player1_up_btn",
        "input_player1_down_btn",
        "input_player1_left_btn",
        "input_player1_right_btn",
        "input_player1_l3_btn",
        "input_player1_r3_btn",
        "input_player1_l_x_plus_axis",
        "input_player1_l_x_minus_axis",
        "input_player1_l_y_plus_axis",
        "input_player1_l_y_minus_axis",
        "input_player1_r_x_plus_axis",
        "input_player1_r_x_minus_axis",
        "input_player1_r_y_plus_axis",
        "input_player1_r_y_minus_axis",
        "input_player1_l2_axis",
        "input_player1_r2_axis",
        // Old Nintendo mode wrote face button overrides to retroarch.cfg.
        // Now uses remap files instead. Remove stale cfg entries so
        // autoconfig runs cleanly.
        "input_player1_a_btn",
        "input_player1_b_btn",
        "input_player1_x_btn",
        "input_player1_y_btn",
        // Old fast-forward was mapped to Back/Select button; now uses RT axis
        "input_toggle_fast_forward_btn",
    ];

    // Write the main config to the emulator directory (used by --appendconfig)
    let cfg_path = emu_root.join("retroarch.cfg");
    patch_retroarch_cfg_with_deletions(&cfg_path, &overrides, stale_keys);

    // ── Also write config to AppImage.home ──────────────────────────────
    // The RetroArch AppImage overrides $HOME to <AppImage>.home/, so its
    // "real" config lives at <AppImage>.home/.config/retroarch/retroarch.cfg.
    // Writing there ensures our settings are the BASE config, not just an
    // appendconfig overlay. This is critical for Gamescope/Steam Deck
    // where video driver and display settings must be correct from the start.
    let appimage_config_dir = find_appimage_config_dir(&emu_root);
    if let Some(ai_cfg_dir) = &appimage_config_dir {
        if let Err(e) = fs::create_dir_all(ai_cfg_dir) {
            warn!(
                "[RETROARCH] Failed to create AppImage config dir {}: {}",
                ai_cfg_dir.display(),
                e
            );
        } else {
            let ai_cfg_path = ai_cfg_dir.join("retroarch.cfg");
            patch_retroarch_cfg_with_deletions(&ai_cfg_path, &overrides, stale_keys);
            info!(
                "[RETROARCH] Also wrote config to AppImage home: {}",
                ai_cfg_path.display()
            );
        }
    }

    // ── Core options (retroarch-core-options.cfg) ─────────────────────────
    // The main retroarch.cfg only affects windowed scaling. For fullscreen,
    // internal resolution is controlled by per-core options stored in a
    // separate file. We patch that for quality presets and widescreen hacks.
    //
    // Clean up any stale per-core .opt files left from before we set
    // global_core_options = true. These would take precedence over our
    // core_options_path and silently ignore quality changes.
    let per_core_config_dir = emu_root.join("config");
    if per_core_config_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&per_core_config_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Look for <core_name>.opt files inside each core config dir
                    if let Ok(files) = fs::read_dir(&path) {
                        for file in files.flatten() {
                            let fp = file.path();
                            if fp.extension().and_then(|e| e.to_str()) == Some("opt") {
                                if let Err(e) = fs::remove_file(&fp) {
                                    warn!("[RETROARCH] Failed to remove stale .opt file {}: {}", fp.display(), e);
                                } else {
                                    info!("[RETROARCH] Removed stale per-core options: {}", fp.display());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(cfg) = user_config {
        let core_opts_path = emu_root.join("retroarch-core-options.cfg");
        let mut core_overrides: HashMap<&str, String> = HashMap::new();

        if let Some(quality) = &cfg.quality_preset {
            apply_core_quality_options(&mut core_overrides, quality);
            info!("[RETROARCH] Patching core options for {:?} quality — {} keys", quality, core_overrides.len());
            for (k, v) in &core_overrides {
                info!("[RETROARCH] core option: {} = {}", k, v);
            }
        }

        apply_core_widescreen_options(&mut core_overrides, &cfg.widescreen);
        if cfg.widescreen != AspectRatio::Standard {
            info!("[RETROARCH] Patched core options for {:?}", cfg.widescreen);
        }

        if !core_overrides.is_empty() {
            patch_retroarch_cfg(&core_opts_path, &core_overrides);
            info!("[RETROARCH] Wrote core options to: {}", core_opts_path.display());

            // Also write core options to AppImage.home so RetroArch finds them
            if let Some(ai_cfg_dir) = &appimage_config_dir {
                let ai_core_opts = ai_cfg_dir.join("retroarch-core-options.cfg");
                patch_retroarch_cfg(&ai_core_opts, &core_overrides);
                info!(
                    "[RETROARCH] Also wrote core options to AppImage home: {}",
                    ai_core_opts.display()
                );
            }
        } else {
            info!("[RETROARCH] No core options to write (quality: {:?}, widescreen: {})",
                cfg.quality_preset, cfg.widescreen);
        }
    }

    // ── Core-specific button remaps ────────────────────────────────────────
    // Nintendo console emulators (Dolphin for GC/Wii, Mupen64Plus for N64)
    // map their console's A button (right-side position) to RetroPad B (east).
    // This means on an Xbox-layout controller, the physical A button (south)
    // sends the wrong input when the game says "Press A".
    //
    // Fix: Write core-specific remap files that swap A↔B for these cores.
    // RetroArch remap indices: 0=B, 1=Y, 2=Select, 3=Start, 4=Up, 5=Down,
    //   6=Left, 7=Right, 8=A, 9=X, 10=L, 11=R, 12=L2, 13=R2, 14=L3, 15=R3
    write_nintendo_core_remaps(&emu_root, &appimage_config_dir);

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

/// Finds the AppImage `.home` config directory inside the emulator root.
///
/// RetroArch AppImages create a portable `$HOME` at
/// `<AppImage-filename>.home/` next to the AppImage binary.
/// RetroArch reads its config from `$HOME/.config/retroarch/retroarch.cfg`
/// inside this directory. To ensure our settings are actually used, we
/// need to write config there — not just to the emulator root.
///
/// Returns `Some(path)` to the `.config/retroarch/` directory inside
/// the AppImage home, or `None` if no AppImage is found.
fn find_appimage_config_dir(emu_root: &Path) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(emu_root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let name_lower = name_str.to_lowercase();
            if name_lower.contains("retroarch") && name_lower.ends_with(".appimage") {
                // Found the AppImage — derive the .home path
                let home_dir = emu_root.join(format!("{}.home", name_str));
                let config_dir = home_dir.join(".config").join("retroarch");
                info!(
                    "[RETROARCH] AppImage home config dir: {}",
                    config_dir.display()
                );
                return Some(config_dir);
            }
        }
    }
    None
}

/// Write core-specific remap files for Nintendo console emulators.
///
/// The Dolphin (GameCube/Wii) and Mupen64Plus (N64) libretro cores map the
/// console's A button (right-side position) to RetroPad B (east), because
/// that matches the physical position on the original controller. However,
/// on modern Xbox-layout controllers (including Steam Deck), RetroPad A is
/// the south/bottom button — so pressing the physical "A" button doesn't
/// trigger the game's "A" action.
///
/// This writes remap files that swap A↔B for these cores, making the
/// physical A button on an Xbox-layout controller match the game's A prompt.
///
/// RetroArch remap file button indices:
///   0=B  1=Y  2=Select  3=Start  4=Up  5=Down  6=Left  7=Right
///   8=A  9=X  10=L  11=R  12=L2  13=R2  14=L3  15=R3
fn write_nintendo_core_remaps(emu_root: &Path, appimage_config_dir: &Option<PathBuf>) {
    // Remap content: swap A(8)↔B(0) so physical south=A goes to core B
    // and physical east=B goes to core A. All other buttons stay default.
    let remap_content = r#"input_player1_btn_a = "0"
input_player1_btn_b = "8"
input_player1_btn_x = "9"
input_player1_btn_y = "1"
input_player1_btn_select = "2"
input_player1_btn_start = "3"
input_player1_btn_up = "4"
input_player1_btn_down = "5"
input_player1_btn_left = "6"
input_player1_btn_right = "7"
input_player1_btn_l = "10"
input_player1_btn_r = "11"
input_player1_btn_l2 = "12"
input_player1_btn_r2 = "13"
input_player1_btn_l3 = "14"
input_player1_btn_r3 = "15"
"#;

    // Core names that need the Nintendo A↔B remap
    let nintendo_cores = &[
        "dolphin-emu",     // Dolphin (GameCube/Wii)
        "Mupen64Plus-Next", // N64
        "parallel_n64",    // N64 (alternate)
    ];

    for core_name in nintendo_cores {
        // Write to emu_root/config/remaps/<core>/<core>.rmp
        let remap_dir = emu_root.join("config").join("remaps").join(core_name);
        write_remap_file(&remap_dir, core_name, remap_content);

        // Also write to AppImage.home if present
        if let Some(ai_cfg_dir) = &appimage_config_dir {
            let ai_remap_dir = ai_cfg_dir.join("config").join("remaps").join(core_name);
            write_remap_file(&ai_remap_dir, core_name, remap_content);
        }
    }
}

/// Helper to write a single remap file.
fn write_remap_file(remap_dir: &Path, core_name: &str, content: &str) {
    if let Err(e) = fs::create_dir_all(remap_dir) {
        warn!("[RETROARCH] Failed to create remap dir {}: {}", remap_dir.display(), e);
        return;
    }
    let remap_path = remap_dir.join(format!("{}.rmp", core_name));
    match fs::write(&remap_path, content) {
        Ok(_) => info!("[RETROARCH] Wrote remap file: {}", remap_path.display()),
        Err(e) => warn!("[RETROARCH] Failed to write remap {}: {}", remap_path.display(), e),
    }
}

/// Returns `true` if the directory looks like a RetroArch installation.
fn is_retroarch(dir: &Path) -> bool {
    // Check for well-known exact names first
    let executables = [
        "retroarch",
        "retroarch.exe",
        "RetroArch.exe",
        "retroarch.AppImage",
    ];
    for exe in &executables {
        if dir.join(exe).exists() {
            info!("[RETROARCH] is_retroarch: matched exact name {:?}", exe);
            return true;
        }
    }

    // Scan directory for any file whose name contains "retroarch" (case-insensitive).
    // This catches variants like "RetroArch-Linux-x86_64.AppImage".
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_lower = name.to_string_lossy().to_lowercase();
            if name_lower.contains("retroarch")
                && (name_lower.ends_with(".appimage")
                    || name_lower.ends_with(".exe")
                    || !name_lower.contains('.'))
            {
                info!(
                    "[RETROARCH] is_retroarch: matched by scan: {:?}",
                    entry.file_name()
                );
                return true;
            }
        }
    }

    // Check for retroarch.cfg as a fallback indicator
    if dir.join("retroarch.cfg").exists() {
        info!("[RETROARCH] is_retroarch: matched via retroarch.cfg");
        return true;
    }

    // Check for a cores/ directory (common RetroArch structure)
    if dir.join("cores").is_dir() {
        info!("[RETROARCH] is_retroarch: matched via cores/ directory");
        return true;
    }

    // Log what we actually found for debugging
    if let Ok(entries) = std::fs::read_dir(dir) {
        let files: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        info!(
            "[RETROARCH] is_retroarch: NO match in {:?}, contents: {:?}",
            dir, files
        );
    }

    false
}

/// Converts a path to RetroArch config format.
/// RetroArch uses forward slashes even on Windows, and wraps paths in quotes.
fn path_to_cfg(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    format!("\"{}\"", s)
}

/// Reads an existing `retroarch.cfg`, applies overrides, and writes it back.
/// Creates the file if it doesn't exist.
///
/// RetroArch config format is simple `key = "value"` lines.
/// We only override keys in our set, preserving everything else.
/// Keys listed in `delete_keys` are REMOVED from the file entirely.
fn patch_retroarch_cfg(cfg_path: &Path, overrides: &HashMap<&str, String>) {
    patch_retroarch_cfg_with_deletions(cfg_path, overrides, &[]);
}

/// Like `patch_retroarch_cfg` but also removes lines whose keys appear
/// in `delete_keys`. This is needed to clean up stale settings from
/// previous launches (e.g. `joypad_autoconfig_dir` pointing to an empty dir).
fn patch_retroarch_cfg_with_deletions(
    cfg_path: &Path,
    overrides: &HashMap<&str, String>,
    delete_keys: &[&str],
) {
    let existing = fs::read_to_string(cfg_path).unwrap_or_default();

    let mut found_keys: HashMap<&str, bool> = overrides.keys().map(|k| (*k, false)).collect();
    let mut lines: Vec<String> = Vec::new();

    for line in existing.lines() {
        let trimmed = line.trim();

        // Check if this line sets one of our override keys
        if let Some(key) = extract_cfg_key(trimmed) {
            // Delete stale keys entirely
            if delete_keys.iter().any(|dk| *dk == key) {
                info!("[RETROARCH] Removing stale config key: {}", key);
                continue;
            }

            if let Some(value) = overrides.get(key) {
                // Replace with our value
                lines.push(format!("{} = {}", key, value));
                found_keys.insert(key, true);
                continue;
            }
        }

        // Keep the line as-is
        lines.push(line.to_string());
    }

    // Append any keys that weren't already in the file
    for (key, was_found) in &found_keys {
        if !was_found {
            if let Some(value) = overrides.get(key) {
                lines.push(format!("{} = {}", key, value));
            }
        }
    }

    // Ensure trailing newline
    let content = lines.join("\n") + "\n";

    if let Err(e) = fs::write(cfg_path, &content) {
        warn!(
            "[RETROARCH] Failed to write config {}: {}",
            cfg_path.display(),
            e
        );
    } else {
        debug!("[RETROARCH] Wrote config to {}", cfg_path.display());
    }
}

/// Extracts the key name from a RetroArch config line (`key = "value"` or `key = value`).
/// Returns `None` for comments, blank lines, or malformed lines.
fn extract_cfg_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    trimmed.split('=').next().map(|k| k.trim()).filter(|k| !k.is_empty())
}

// ── Controller layout helpers ────────────────────────────────────────────

/// Applies controller layout configuration for the selected controller family.
///
/// RetroArch's RetroPad layout mirrors Xbox by default (A=south, B=east,
/// X=west, Y=north). With `input_autodetect_enable = true` and the SDL2
/// joypad driver, autoconfig profiles set `input_player1_*_btn` bindings
/// automatically — and those OVERRIDE anything written to retroarch.cfg.
///
/// Strategy:
/// - **Xbox / PlayStation**: Leave autoconfig as-is (default = Xbox layout).
///   Only set display labels. Remove any stale Nintendo remap files.
/// - **Nintendo**: Write RetroArch `.rmp` (remap) files that swap A↔B and
///   X↔Y. Remaps apply AFTER autoconfig and take priority, so they
///   reliably override the physical button layout regardless of input driver.
///
/// ## Why remap files instead of retroarch.cfg?
///
/// Writing `input_player1_a_btn` to retroarch.cfg does NOT work because:
/// 1. SDL2 autoconfig detects the controller and sets its own button indices
/// 2. Autoconfig runs AFTER retroarch.cfg is loaded
/// 3. The autoconfig values replace whatever retroarch.cfg specified
///
/// Remap files (`.rmp`) are loaded AFTER autoconfig, so they reliably
/// override the autodetected button assignments.
fn apply_controller_mappings(
    overrides: &mut HashMap<&str, String>,
    controller: &ControllerType,
    remaps_dir: &Path,
) {
    match controller {
        ControllerType::Xbox => {
            overrides.insert("input_player1_a_btn_label", "\"A\"".into());
            overrides.insert("input_player1_b_btn_label", "\"B\"".into());
            overrides.insert("input_player1_x_btn_label", "\"X\"".into());
            overrides.insert("input_player1_y_btn_label", "\"Y\"".into());
            // Remove any Nintendo remaps so autoconfig default (Xbox) applies
            cleanup_nintendo_remaps(remaps_dir);
        }
        ControllerType::PlayStation => {
            overrides.insert("input_player1_a_btn_label", "\"Cross\"".into());
            overrides.insert("input_player1_b_btn_label", "\"Circle\"".into());
            overrides.insert("input_player1_x_btn_label", "\"Square\"".into());
            overrides.insert("input_player1_y_btn_label", "\"Triangle\"".into());
            cleanup_nintendo_remaps(remaps_dir);
        }
        ControllerType::Nintendo => {
            overrides.insert("input_player1_a_btn_label", "\"A\"".into());
            overrides.insert("input_player1_b_btn_label", "\"B\"".into());
            overrides.insert("input_player1_x_btn_label", "\"X\"".into());
            overrides.insert("input_player1_y_btn_label", "\"Y\"".into());
            // Write remap files for all common cores. Remaps swap RetroPad
            // buttons at the core interface level, so the swap happens
            // regardless of which physical controller or autoconfig is active.
            write_nintendo_remaps(remaps_dir);
        }
    }
}

/// The remap content that swaps A↔B and X↔Y on the RetroPad.
///
/// RetroArch remap indices map RetroPad buttons to RetroPad buttons:
///   0=B, 1=Y, 2=Select, 3=Start, 4=Up, 5=Down, 6=Left, 7=Right,
///   8=A, 9=X, 10=L, 11=R, 12=L2, 13=R2, 14=L3, 15=R3
///
/// Default (identity): `input_remap_port_p1 = 0` (pass through)
/// To swap A↔B: set index 0 (B slot) to produce A (8) and index 8 (A slot) to produce B (0)
/// To swap X↔Y: set index 1 (Y slot) to produce X (9) and index 9 (X slot) to produce Y (1)
const NINTENDO_REMAP_CONTENT: &str = "\
input_player1_btn_b = 8\n\
input_player1_btn_y = 9\n\
input_player1_btn_a = 0\n\
input_player1_btn_x = 1\n";

/// List of core directory names to write Nintendo remaps into.
/// Each gets a `<core>/<core>.rmp` file in the remaps directory.
const REMAP_CORE_NAMES: &[&str] = &[
    "dolphin-emu",
    "Mupen64Plus-Next",
    "mupen64plus",
    "parallel_n64",
    "Gambatte",
    "mGBA",
    "Snes9x",
    "bsnes",
    "Beetle PSX HW",
    "SwanStation",
    "PCSX-ReARMed",
    "PCSX2",
    "Mesen",
    "FCEUmm",
    "melonDS",
    "Genesis Plus GX",
    "PPSSPP",
];

/// Writes Nintendo A↔B / X↔Y remap files for all known cores.
fn write_nintendo_remaps(remaps_dir: &Path) {
    for core_name in REMAP_CORE_NAMES {
        let core_dir = remaps_dir.join(core_name);
        if let Err(e) = fs::create_dir_all(&core_dir) {
            warn!("[RETROARCH] Failed to create remap dir {}: {}", core_dir.display(), e);
            continue;
        }
        let rmp_path = core_dir.join(format!("{}.rmp", core_name));
        if let Err(e) = fs::write(&rmp_path, NINTENDO_REMAP_CONTENT) {
            warn!("[RETROARCH] Failed to write remap {}: {}", rmp_path.display(), e);
        }
    }
    info!("[RETROARCH] Wrote Nintendo A↔B/X↔Y remap files for {} cores", REMAP_CORE_NAMES.len());
}

/// Removes Nintendo remap files for all known cores (used when switching
/// back to Xbox/PlayStation/Auto layout).
fn cleanup_nintendo_remaps(remaps_dir: &Path) {
    for core_name in REMAP_CORE_NAMES {
        let rmp_path = remaps_dir.join(core_name).join(format!("{}.rmp", core_name));
        if rmp_path.exists() {
            if let Err(e) = fs::remove_file(&rmp_path) {
                warn!("[RETROARCH] Failed to remove remap {}: {}", rmp_path.display(), e);
            }
        }
    }
}

// ── Quality preset helpers ──────────────────────────────────────────────

/// Applies quality settings to the main retroarch.cfg.
///
/// `video_scale` only affects windowed mode, so in fullscreen the main
/// levers are `video_smooth` (bilinear filtering) and `video_shader_enable`.
/// The real internal resolution work is done in `apply_core_quality_options`.
fn apply_quality_preset(overrides: &mut HashMap<&str, String>, quality: &QualityPreset) {
    match quality {
        QualityPreset::Low => {
            overrides.insert("video_smooth", "false".into());
            overrides.insert("video_shader_enable", "false".into());
            overrides.insert("video_scale_integer", "false".into());
            overrides.insert("video_gpu_screenshot", "false".into());
        }
        QualityPreset::Medium => {
            overrides.insert("video_smooth", "true".into());
            overrides.insert("video_shader_enable", "false".into());
            overrides.insert("video_scale_integer", "false".into());
            overrides.insert("video_gpu_screenshot", "true".into());
        }
        QualityPreset::High | QualityPreset::Ultra => {
            overrides.insert("video_smooth", "true".into());
            overrides.insert("video_shader_enable", "false".into());
            overrides.insert("video_scale_integer", "false".into());
            overrides.insert("video_gpu_screenshot", "true".into());
        }
    }
}

/// Enables CRT shader in RetroArch by locating a suitable `.slangp` shader preset.
///
/// RetroArch ships with CRT shaders in `shaders/shaders_slang/crt/`.
/// We look for common CRT presets in order of preference:
/// 1. `crt-easymode.slangp` — lightweight, good-looking CRT
/// 2. `crt-royale.slangp` — high-quality, GPU-intensive CRT emulation
/// 3. `crt-lottes.slangp` — classic CRT look
/// 4. Any `.slangp` file in the crt directory
///
/// If shaders aren't found on disk, we try to extract them from the
/// RetroArch AppImage (Linux) using `--appimage-extract`. The AppImage
/// bundles shaders internally in its squashfs, so they're only accessible
/// at runtime unless explicitly extracted.
fn apply_crt_shader(overrides: &mut HashMap<&str, String>, emu_root: &std::path::Path) -> Option<String> {
    overrides.insert("video_shader_enable", "true".into());

    let preferred_presets = [
        "crt-easymode.slangp",
        "crt-royale.slangp",
        "crt-lottes.slangp",
        "crt-easymode.glslp",
        "crt-royale.glslp",
        "crt-lottes.glslp",
    ];

    // Build the list of directories to search (emu_root + AppImage.home)
    let mut shader_dirs: Vec<PathBuf> = vec![
        emu_root.join("shaders").join("shaders_slang").join("crt"),
        emu_root.join("shaders").join("shaders_glsl").join("crt"),
        emu_root.join("shaders_slang").join("crt"),
        emu_root.join("shaders_glsl").join("crt"),
    ];

    // Log which directories we're checking for debugging
    for dir in &shader_dirs {
        info!("[RETROARCH] CRT shader search dir: {} (exists: {})", dir.display(), dir.is_dir());
    }

    // Also check AppImage.home shader paths (RetroArch AppImage creates a
    // portable HOME at <AppImage>.home/ — shaders may be cached there)
    if let Some(ai_cfg_dir) = find_appimage_config_dir(emu_root) {
        // ai_cfg_dir = <AppImage>.home/.config/retroarch/
        // Shaders may be in <AppImage>.home/.config/retroarch/shaders/...
        shader_dirs.push(ai_cfg_dir.join("shaders/shaders_slang/crt"));
        shader_dirs.push(ai_cfg_dir.join("shaders/shaders_glsl/crt"));
    }

    // Also check for shaders extracted from AppImage (squashfs-root/)
    let squashfs_root = emu_root.join("squashfs-root");
    shader_dirs.push(squashfs_root.join("usr/share/libretro/shaders/shaders_slang/crt"));
    shader_dirs.push(squashfs_root.join("usr/share/retroarch/shaders/shaders_slang/crt"));

    // First pass: search existing directories
    if let Some(path) = find_crt_shader_in_dirs(&shader_dirs, &preferred_presets) {
        info!("[RETROARCH] Using CRT shader: {}", path.display());
        let s = path.to_string_lossy().into_owned();
        overrides.insert("video_shader", s.clone());
        return Some(s);
    }

    // Second pass: try extracting shaders from the RetroArch AppImage
    #[cfg(target_os = "linux")]
    {
        if let Some(appimage_path) = find_appimage_binary(emu_root) {
            info!("[RETROARCH] No shaders found on disk — extracting from AppImage: {}", appimage_path.display());
            extract_appimage_shaders(emu_root, &appimage_path);

            // Re-check after extraction
            let mut retry_dirs = vec![
                emu_root.join("shaders/shaders_slang/crt"),
            ];
            retry_dirs.push(squashfs_root.join("usr/share/libretro/shaders/shaders_slang/crt"));
            retry_dirs.push(squashfs_root.join("usr/share/retroarch/shaders/shaders_slang/crt"));

            if let Some(path) = find_crt_shader_in_dirs(&retry_dirs, &preferred_presets) {
                info!("[RETROARCH] Using CRT shader (after extraction): {}", path.display());
                let s = path.to_string_lossy().into_owned();
                overrides.insert("video_shader", s.clone());
                return Some(s);
            }
        }
    }

    warn!("[RETROARCH] No CRT shader preset found in any shader directory — CRT toggle is ON but no shader will be applied. Install shader presets to {}/shaders/shaders_slang/crt/", emu_root.display());
    None
}

/// Searches shader directories for a CRT shader preset.
fn find_crt_shader_in_dirs(dirs: &[PathBuf], preferred: &[&str]) -> Option<PathBuf> {
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        // Try preferred presets first
        for preset in preferred {
            let path = dir.join(preset);
            if path.is_file() {
                return Some(path);
            }
        }
        // Fallback: any .slangp or .glslp in the directory
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "slangp" || e == "glslp") {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Finds the RetroArch AppImage binary in the emulator directory.
#[cfg(target_os = "linux")]
fn find_appimage_binary(emu_root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(emu_root).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("retroarch") && name.ends_with(".appimage") {
            return Some(entry.path());
        }
    }
    None
}

/// Extracts shader files from the RetroArch AppImage using `--appimage-extract`.
/// Only extracts the `usr/share/` shader directories to minimize extraction time.
#[cfg(target_os = "linux")]
fn extract_appimage_shaders(emu_root: &Path, appimage_path: &Path) {
    use std::process::Command;

    // Extract only shader-related files to emu_root/squashfs-root/
    let result = Command::new(appimage_path)
        .arg("--appimage-extract")
        .arg("usr/share/libretro/shaders/shaders_slang/crt/*")
        .current_dir(emu_root)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            info!("[RETROARCH] Extracted CRT shaders from AppImage (slang)");
        }
        Ok(output) => {
            // First pattern didn't match — try alternative path
            info!("[RETROARCH] First shader extract path failed (status {}), trying alternative...", output.status);
            let alt = Command::new(appimage_path)
                .arg("--appimage-extract")
                .arg("usr/share/retroarch/shaders/shaders_slang/crt/*")
                .current_dir(emu_root)
                .output();
            match alt {
                Ok(o) if o.status.success() => {
                    info!("[RETROARCH] Extracted CRT shaders from AppImage (alt path)");
                }
                _ => {
                    warn!("[RETROARCH] Failed to extract shaders from AppImage — CRT shader unavailable");
                }
            }
        }
        Err(e) => {
            warn!("[RETROARCH] AppImage shader extraction failed: {}", e);
        }
    }
}

/// Applies per-core internal resolution options to `retroarch-core-options.cfg`.
///
/// Each emulator core has its own option key for internal resolution scaling.
/// We write all known core resolution keys so the setting applies regardless
/// of which core ends up running the game.
fn apply_core_quality_options(overrides: &mut HashMap<&str, String>, quality: &QualityPreset) {
    //                          dolphin_efb  n64_res       pcsx_rearmed  beetle_psx_res  ppsspp_res
    let (dolphin_efb, n64_res, _pcsx_rearmed, beetle_psx_res, ppsspp_res) = match quality {
        QualityPreset::Low    => ("1", "320x240",  "1", "1x(native)",  "1"),
        QualityPreset::Medium => ("3", "640x480",  "2", "2x(native)",  "2"),
        QualityPreset::High   => ("5", "1280x960", "4", "4x(native)",  "4"),
        QualityPreset::Ultra  => ("6", "1920x1440","8", "8x(native)",  "8"),
    };

    // Dolphin (GameCube/Wii) — internal EFB scale (Ultra = 6x = 3840×3168, max)
    overrides.insert("dolphin_efb_scale", format!("\"{}\"", dolphin_efb));

    // Mupen64Plus-Next (N64) — resolution (Ultra = 1920×1440 = 6x native)
    overrides.insert("mupen64plus-Resolution", format!("\"{}\"", n64_res));
    overrides.insert("parallel-n64-screensize", format!("\"{}\"", n64_res));

    // PCSX ReARMed (PS1) — resolution multiplier
    overrides.insert("pcsx_rearmed_neon_enhancement_enable", format!("\"{}\"",
        if matches!(quality, QualityPreset::Low) { "disabled" } else { "enabled" }));
    overrides.insert("pcsx_rearmed_neon_enhancement_no_main", "\"disabled\"".into());

    // Beetle PSX HW (PS1 HW) — internal GPU resolution (Ultra = 8x native)
    overrides.insert("beetle_psx_hw_internal_resolution", format!("\"{}\"", beetle_psx_res));

    // SwanStation / DuckStation (PS1) — GPU resolution scale
    let ps1_res_scale = match quality {
        QualityPreset::Low => "1",
        QualityPreset::Medium => "2",
        QualityPreset::High => "4",
        QualityPreset::Ultra => "8",
    };
    overrides.insert("swanstation_GPU.ResolutionScale", format!("\"{}\"", ps1_res_scale));
    overrides.insert("duckstation_GPU.ResolutionScale", format!("\"{}\"", ps1_res_scale));

    // PPSSPP (PSP) — internal resolution (Ultra = 8x)
    overrides.insert("ppsspp_internal_resolution", format!("\"{}\"", ppsspp_res));

    // mGBA (GBA) — color correction
    overrides.insert("mgba_color_correction", format!("\"{}\"",
        if matches!(quality, QualityPreset::Low) { "OFF" } else { "Game Boy Advance" }));

    // Snes9x — hi-res blending
    overrides.insert("snes9x_hires_blend", format!("\"{}\"",
        if matches!(quality, QualityPreset::Low) { "disabled" } else { "merge" }));

    // Beetle PSX HW — additional quality settings
    // Ultra adds perspective-correct texturing for PS1
    let (psx_dither, psx_filter, psx_pgxp) = match quality {
        QualityPreset::Low => ("1x(native)", "nearest", "disabled"),
        QualityPreset::Medium => ("1x(native)", "nearest", "enabled"),
        QualityPreset::High => ("disabled", "bilinear", "enabled"),
        QualityPreset::Ultra => ("disabled", "bilinear", "enabled"),
    };
    overrides.insert("beetle_psx_hw_dither_mode", format!("\"{}\"", psx_dither));
    overrides.insert("beetle_psx_hw_filter", format!("\"{}\"", psx_filter));
    overrides.insert("beetle_psx_hw_pgxp_mode", format!("\"{}\"", psx_pgxp));
    // Ultra: PGXP perspective-correct texturing eliminates PS1 texture warping
    if matches!(quality, QualityPreset::Ultra) {
        overrides.insert("beetle_psx_hw_pgxp_texture", "\"enabled\"".into());
    } else {
        overrides.insert("beetle_psx_hw_pgxp_texture", "\"disabled\"".into());
    }

    // Mupen64Plus-Next — texture filtering
    let (n64_txfilter, n64_aspect) = match quality {
        QualityPreset::Low => ("None", "4:3"),
        QualityPreset::Medium => ("None", "4:3"),
        QualityPreset::High | QualityPreset::Ultra => ("6xBRZ", "16:9 adjusted"),
    };
    overrides.insert("mupen64plus-txFilterMode", format!("\"{}\"", n64_txfilter));
    overrides.insert("mupen64plus-aspect", format!("\"{}\"", n64_aspect));

    // Dolphin — anti-aliasing (Ultra = 8x MSAA)
    let (dolphin_aa, dolphin_efb_copy) = match quality {
        QualityPreset::Low => ("None", "disabled"),
        QualityPreset::Medium => ("2x MSAA", "enabled"),
        QualityPreset::High => ("4x MSAA", "enabled"),
        QualityPreset::Ultra => ("8x MSAA", "enabled"),
    };
    overrides.insert("dolphin_anti_aliasing", format!("\"{}\"", dolphin_aa));
    overrides.insert("dolphin_efb_access_enable", format!("\"{}\"", dolphin_efb_copy));

    // PPSSPP — texture filtering + scaling (Ultra adds xBRZ texture upscaling)
    let ppsspp_texfilter = match quality {
        QualityPreset::Low | QualityPreset::Medium => "Auto",
        QualityPreset::High | QualityPreset::Ultra => "Linear",
    };
    overrides.insert("ppsspp_texture_filtering", format!("\"{}\"", ppsspp_texfilter));
    if matches!(quality, QualityPreset::Ultra) {
        overrides.insert("ppsspp_texture_scaling_type", "\"xBRZ\"".into());
        overrides.insert("ppsspp_texture_scaling_level", "\"3\"".into());
    }
}

// ── Widescreen helpers ─────────────────────────────────────────────────

/// Applies aspect ratio settings to the main retroarch.cfg.
///
/// RetroArch's built-in aspect ratio indices:
///   0 = 4:3,  1 = 16:9,  2 = 16:10,  22 = Core provided
fn apply_widescreen(overrides: &mut HashMap<&str, String>, ratio: &AspectRatio) {
    match ratio {
        AspectRatio::Standard => {
            // Let the core decide (usually 4:3 for retro consoles)
            overrides.insert("aspect_ratio_index", "22".into());
            overrides.insert("video_aspect_ratio_auto", "true".into());
        }
        AspectRatio::Wide16_9 => {
            overrides.insert("aspect_ratio_index", "1".into());
            overrides.insert("video_aspect_ratio_auto", "false".into());
        }
        AspectRatio::Wide16_10 => {
            overrides.insert("aspect_ratio_index", "2".into());
            overrides.insert("video_aspect_ratio_auto", "false".into());
        }
    }
}

/// Applies per-core widescreen hacks to `retroarch-core-options.cfg`.
///
/// Many cores have their own widescreen hack option that renders natively
/// wide instead of just stretching. We enable these for both 16:9 and 16:10
/// aspect ratios — core widescreen hacks typically render at native wide
/// resolution regardless of the exact ratio, and the aspect ratio override
/// in apply_widescreen() handles the final display ratio.
fn apply_core_widescreen_options(overrides: &mut HashMap<&str, String>, ratio: &AspectRatio) {
    let enabled = !matches!(ratio, AspectRatio::Standard);
    let val = if enabled { "enabled" } else { "disabled" };
    let on_off = if enabled { "ON" } else { "OFF" };
    let ratio_str = match ratio {
        AspectRatio::Standard => "4:3",
        AspectRatio::Wide16_9 => "16:9",
        AspectRatio::Wide16_10 => "16:10",
    };

    // Dolphin (GameCube/Wii) — native widescreen hack
    overrides.insert("dolphin_widescreen_hack", format!("\"{}\"", val));

    // Mupen64Plus-Next (N64) — widescreen
    // N64 cores only support "16:9" or "4:3"; 16:10 uses 16:9 hack + ratio override
    let n64_aspect = if enabled { "16:9" } else { "4:3" };
    overrides.insert("mupen64plus-aspect", format!("\"{}\"", n64_aspect));
    overrides.insert("parallel-n64-aspect", format!("\"{}\"", n64_aspect));

    // PPSSPP (PSP) — native 16:9 support in many games
    overrides.insert("ppsspp_widescreen_hack", format!("\"{}\"", on_off));

    // Beetle PSX HW — widescreen mode hack
    overrides.insert("beetle_psx_hw_widescreen_hack", format!("\"{}\"", val));
    overrides.insert("beetle_psx_hw_widescreen_hack_aspect_ratio", format!("\"{}\"", ratio_str));

    // SwanStation / DuckStation (PS1) — GPU widescreen hack
    let ws_bool = if enabled { "true" } else { "false" };
    overrides.insert("swanstation_GPU.WidescreenHack", format!("\"{}\"", ws_bool));
    overrides.insert("duckstation_GPU.WidescreenHack", format!("\"{}\"", ws_bool));

    // PCSX ReARMed — widescreen
    overrides.insert("pcsx_rearmed_widescreen", format!("\"{}\"", val));

    // Snes9x — widescreen (available in some builds)
    overrides.insert("snes9x_aspect_ratio", format!("\"{}\"",
        if enabled { "16:9" } else { "4:3" }));
}

/// Known mapping of ROM file extensions to RetroArch core name fragments.
/// Each entry is (extension, list of core name substrings to search for, in priority order).
/// Core filenames look like `mgba_libretro.dll` or `dolphin_libretro.so`.
///
/// **Ambiguous extensions** (`.iso`, `.chd`) are handled specially by
/// `resolve_core_for_rom` — see `detect_iso_disc_type` for `.iso` files.
const EXTENSION_CORE_MAP: &[(&str, &[&str])] = &[
    // Nintendo
    ("gba", &["mgba", "vba_next", "gpsp"]),
    ("gbc", &["gambatte", "mgba", "sameboy"]),
    ("gb", &["gambatte", "mgba", "sameboy"]),
    ("nes", &["mesen", "nestopia", "fceumm", "quicknes"]),
    ("fds", &["mesen", "nestopia", "fceumm"]),
    ("sfc", &["snes9x", "bsnes", "mesen-s"]),
    ("smc", &["snes9x", "bsnes", "mesen-s"]),
    ("n64", &["mupen64plus_next", "mupen64plus", "parallel_n64"]),
    ("z64", &["mupen64plus_next", "mupen64plus", "parallel_n64"]),
    ("v64", &["mupen64plus_next", "mupen64plus", "parallel_n64"]),
    ("nds", &["melonds", "desmume"]),
    ("3ds", &["citra"]),
    ("gcm", &["dolphin"]),
    // .iso — ambiguous, resolved at runtime by detect_iso_disc_type()
    // Fallback order if disc detection fails: pcsx2 > dolphin (PS2 ISOs far more common)
    ("iso", &["pcsx2", "dolphin", "ppsspp", "mednafen_saturn"]),
    ("gcz", &["dolphin"]),
    ("rvz", &["dolphin"]),
    ("wbfs", &["dolphin"]),
    ("wad", &["dolphin"]),
    ("dol", &["dolphin"]),
    ("elf", &["dolphin"]),
    ("wia", &["dolphin"]),
    // Sony
    ("cue", &["beetle_psx", "mednafen_psx", "pcsx_rearmed", "swanstation", "pcsx2"]),
    ("bin", &["beetle_psx", "mednafen_psx", "pcsx_rearmed", "genesis_plus_gx"]),
    ("chd", &["beetle_psx", "mednafen_psx", "swanstation", "pcsx2", "dolphin"]),
    ("pbp", &["beetle_psx", "pcsx_rearmed", "ppsspp"]),
    ("cso", &["ppsspp"]),
    ("psp", &["ppsspp"]),
    // Sega
    ("md", &["genesis_plus_gx", "picodrive", "blastem"]),
    ("gen", &["genesis_plus_gx", "picodrive", "blastem"]),
    ("smd", &["genesis_plus_gx", "picodrive"]),
    ("32x", &["picodrive"]),
    ("sms", &["genesis_plus_gx", "picodrive"]),
    ("gg", &["genesis_plus_gx"]),
    ("cdi", &["flycast"]),
    ("gdi", &["flycast"]),
    // Atari
    ("a26", &["stella"]),
    ("a78", &["prosystem"]),
    ("lnx", &["mednafen_lynx", "handy"]),
    // Other
    ("pce", &["beetle_pce", "mednafen_pce"]),
    ("ngp", &["beetle_ngp", "mednafen_ngp"]),
    ("ngc", &["beetle_ngp", "mednafen_ngp"]),
    ("ws", &["beetle_wswan", "mednafen_wswan"]),
    ("wsc", &["beetle_wswan", "mednafen_wswan"]),
    ("vec", &["vecx"]),
    ("col", &["bluemsx"]),
];

/// Disc type detected from an ISO file's header magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IsoDiscType {
    /// GameCube disc — magic word 0xC2339F3D at offset 0x1C
    GameCube,
    /// Wii disc — magic word 0x5D1C9EA3 at offset 0x18
    Wii,
    /// Not a Nintendo disc — likely PS2, PSP, or Saturn
    Other,
}

/// Reads the first 32 bytes of an ISO file and checks for Nintendo disc
/// magic words to distinguish GameCube/Wii ISOs from PS2/PSP/Saturn ISOs.
///
/// GameCube discs have `0xC2339F3D` at byte offset 0x1C.
/// Wii discs have `0x5D1C9EA3` at byte offset 0x18.
/// If neither magic is found, the ISO is assumed to be non-Nintendo (PS2, etc.).
fn detect_iso_disc_type(iso_path: &Path) -> IsoDiscType {
    use std::io::Read;

    let mut buf = [0u8; 32];
    let Ok(mut f) = fs::File::open(iso_path) else {
        warn!("[RETROARCH] Cannot open ISO for disc detection: {}", iso_path.display());
        return IsoDiscType::Other;
    };
    if f.read_exact(&mut buf).is_err() {
        warn!("[RETROARCH] ISO too small for disc detection: {}", iso_path.display());
        return IsoDiscType::Other;
    }

    // Wii magic: 0x5D1C9EA3 at offset 0x18
    let wii_magic = u32::from_be_bytes([buf[0x18], buf[0x19], buf[0x1A], buf[0x1B]]);
    if wii_magic == 0x5D1C_9EA3 {
        info!("[RETROARCH] ISO disc detection: Wii disc (magic 0x5D1C9EA3 at 0x18) — {}", iso_path.display());
        return IsoDiscType::Wii;
    }

    // GameCube magic: 0xC2339F3D at offset 0x1C
    let gc_magic = u32::from_be_bytes([buf[0x1C], buf[0x1D], buf[0x1E], buf[0x1F]]);
    if gc_magic == 0xC233_9F3D {
        info!("[RETROARCH] ISO disc detection: GameCube disc (magic 0xC2339F3D at 0x1C) — {}", iso_path.display());
        return IsoDiscType::GameCube;
    }

    info!("[RETROARCH] ISO disc detection: non-Nintendo disc (no GC/Wii magic found) — {}", iso_path.display());
    IsoDiscType::Other
}

/// Given a ROM file path and the RetroArch install directory, try to find a
/// matching core in `<emu_root>/cores/`. Returns the absolute path to the core
/// library if found, or `None` if no match.
///
/// For ambiguous extensions like `.iso` (which could be GameCube, Wii, PS2,
/// PSP, or Saturn), reads the disc header magic bytes to determine the
/// correct platform before selecting a core.
pub fn resolve_core_for_rom(emu_root: &Path, rom_path: &str) -> Option<PathBuf> {
    let rom = Path::new(rom_path);
    let rom_ext = rom
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())?;

    // .m3u playlists (multi-disc): read the first entry to determine the
    // actual ROM type, then resolve the core based on that file's extension.
    if rom_ext == "m3u" {
        if let Ok(contents) = fs::read_to_string(rom) {
            if let Some(first_line) = contents.lines().find(|l| !l.trim().is_empty() && !l.starts_with('#')) {
                let first_disc = first_line.trim();
                info!("[RETROARCH] .m3u playlist — resolving core from first disc: {}", first_disc);
                return resolve_core_for_rom(emu_root, first_disc);
            }
        }
        warn!("[RETROARCH] .m3u file is empty or unreadable: {}", rom_path);
        return None;
    }

    // For .iso files, detect the disc type from header magic bytes to pick
    // the right core. Without this, the static EXTENSION_CORE_MAP ordering
    // would always pick whichever core appears first (previously dolphin
    // for .iso, which broke PS2 games).
    let candidates: &[&str] = if rom_ext == "iso" {
        match detect_iso_disc_type(rom) {
            IsoDiscType::GameCube | IsoDiscType::Wii => {
                // Nintendo disc — use Dolphin
                &["dolphin"]
            }
            IsoDiscType::Other => {
                // Non-Nintendo ISO — try PS2 first, then PSP, then fallback
                &["pcsx2", "ppsspp", "mednafen_saturn", "dolphin"]
            }
        }
    } else {
        EXTENSION_CORE_MAP
            .iter()
            .find(|(ext, _)| *ext == rom_ext.as_str())
            .map(|(_, cores)| *cores)?
    };

    let cores_dir = emu_root.join("cores");
    if !cores_dir.is_dir() {
        warn!("[RETROARCH] cores/ directory not found in {}", emu_root.display());
        return None;
    }

    let entries: Vec<_> = fs::read_dir(&cores_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.contains("_libretro")
        })
        .collect();

    // Search in priority order
    for candidate in candidates {
        let needle = candidate.to_lowercase();
        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains(&needle) && name.contains("_libretro") {
                let core_path = entry.path();
                info!(
                    "[RETROARCH] Resolved core for .{}: {} (matched '{}')",
                    rom_ext,
                    core_path.display(),
                    candidate
                );
                return Some(core_path);
            }
        }
    }

    warn!(
        "[RETROARCH] No core found for .{} extension in {}",
        rom_ext,
        cores_dir.display()
    );
    None
}