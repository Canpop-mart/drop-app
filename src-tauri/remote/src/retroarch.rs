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

use database::models::data::{ControllerType, QualityPreset, UserConfiguration};
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
    let autoconfig_dir = emu_root.join("autoconfig");
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

    // Ensure autoconfig dir exists (for controller profiles)
    if let Err(e) = fs::create_dir_all(&autoconfig_dir) {
        warn!(
            "[RETROARCH] Failed to create autoconfig dir {}: {}",
            autoconfig_dir.display(),
            e
        );
    }

    // Build the config overrides
    let mut overrides: HashMap<&str, String> = HashMap::new();

    // Core/system paths
    overrides.insert("libretro_directory", path_to_cfg(&cores_dir));
    overrides.insert("system_directory", path_to_cfg(&system_dir));
    overrides.insert("joypad_autoconfig_dir", path_to_cfg(&autoconfig_dir));
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

    // Point core options to our file so --appendconfig picks up the right path.
    // Without this, RetroArch reads core options from the AppImage's $HOME default.
    // We write core options to both locations, but core_options_path tells
    // RetroArch where to READ them from. Use the emulator root copy since
    // --appendconfig will make this override stick.
    let core_opts_file = emu_root.join("retroarch-core-options.cfg");
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

    // ── Controller layout mapping ────────────────────────────────────────
    if let Some(cfg) = user_config {
        if let Some(controller) = &cfg.controller_type {
            apply_controller_mappings(&mut overrides, controller);
            info!("[RETROARCH] Applied {:?} controller layout", controller);
        }

        // ── Quality preset (retroarch.cfg portion) ──────────────────────
        if let Some(quality) = &cfg.quality_preset {
            apply_quality_preset(&mut overrides, quality);
            info!("[RETROARCH] Applied {:?} quality preset", quality);
        }

        // ── Widescreen toggle ───────────────────────────────────────────
        apply_widescreen(&mut overrides, cfg.widescreen);
        if cfg.widescreen {
            info!("[RETROARCH] Widescreen enabled");
        }
    }

    // Write the main config to the emulator directory (used by --appendconfig)
    let cfg_path = emu_root.join("retroarch.cfg");
    patch_retroarch_cfg(&cfg_path, &overrides);

    // ── Also write config to AppImage.home ──────────────────────────────
    // The RetroArch AppImage overrides $HOME to <AppImage>.home/, so its
    // "real" config lives at <AppImage>.home/.config/retroarch/retroarch.cfg.
    // Writing there ensures our settings are the BASE config, not just an
    // appendconfig overlay. This is critical for Gamescope/Steam Deck
    // where video driver and display settings must be correct from the start.
    let appimage_config_dir = find_appimage_config_dir(&emu_root);
    if let Some(ref ai_cfg_dir) = appimage_config_dir {
        if let Err(e) = fs::create_dir_all(ai_cfg_dir) {
            warn!(
                "[RETROARCH] Failed to create AppImage config dir {}: {}",
                ai_cfg_dir.display(),
                e
            );
        } else {
            let ai_cfg_path = ai_cfg_dir.join("retroarch.cfg");
            patch_retroarch_cfg(&ai_cfg_path, &overrides);
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
    if let Some(cfg) = user_config {
        let core_opts_path = emu_root.join("retroarch-core-options.cfg");
        let mut core_overrides: HashMap<&str, String> = HashMap::new();

        if let Some(quality) = &cfg.quality_preset {
            apply_core_quality_options(&mut core_overrides, quality);
            info!("[RETROARCH] Patched core options for {:?} quality", quality);
        }

        apply_core_widescreen_options(&mut core_overrides, cfg.widescreen);
        if cfg.widescreen {
            info!("[RETROARCH] Patched core options for widescreen");
        }

        if !core_overrides.is_empty() {
            patch_retroarch_cfg(&core_opts_path, &core_overrides);

            // Also write core options to AppImage.home so RetroArch finds them
            if let Some(ref ai_cfg_dir) = appimage_config_dir {
                let ai_core_opts = ai_cfg_dir.join("retroarch-core-options.cfg");
                patch_retroarch_cfg(&ai_core_opts, &core_overrides);
                info!(
                    "[RETROARCH] Also wrote core options to AppImage home: {}",
                    ai_core_opts.display()
                );
            }
        }
    }

    info!(
        "[RETROARCH] Configured: saves={}, states={}",
        savefile_dir.display(),
        savestate_dir.display()
    );

    Some(RetroArchInfo {
        savefile_directory: savefile_dir.to_string_lossy().to_string(),
        savestate_directory: savestate_dir.to_string_lossy().to_string(),
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
fn patch_retroarch_cfg(cfg_path: &Path, overrides: &HashMap<&str, String>) {
    let existing = fs::read_to_string(cfg_path).unwrap_or_default();

    let mut found_keys: HashMap<&str, bool> = overrides.keys().map(|k| (*k, false)).collect();
    let mut lines: Vec<String> = Vec::new();

    for line in existing.lines() {
        let trimmed = line.trim();

        // Check if this line sets one of our override keys
        if let Some(key) = extract_cfg_key(trimmed) {
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

/// Writes `input_player1_*_btn` overrides so the on-screen prompts and
/// the physical button positions match the selected controller family.
///
/// RetroArch's RetroPad layout mirrors Xbox by default (A=south, B=east,
/// X=west, Y=north). Nintendo swaps A↔B and X↔Y. PlayStation just
/// relabels but keeps the same positional layout as Xbox.
///
/// We also write the corresponding `input_player1_*_label` keys so the
/// RetroArch menu shows the correct glyphs.
fn apply_controller_mappings(overrides: &mut HashMap<&str, String>, controller: &ControllerType) {
    match controller {
        ControllerType::Xbox => {
            // Xbox layout (RetroArch default) — south=A, east=B, west=X, north=Y
            overrides.insert("input_player1_a_btn", "1".into());       // south face
            overrides.insert("input_player1_b_btn", "0".into());       // east face
            overrides.insert("input_player1_x_btn", "3".into());       // west face
            overrides.insert("input_player1_y_btn", "2".into());       // north face
            overrides.insert("input_player1_a_btn_label", "\"A\"".into());
            overrides.insert("input_player1_b_btn_label", "\"B\"".into());
            overrides.insert("input_player1_x_btn_label", "\"X\"".into());
            overrides.insert("input_player1_y_btn_label", "\"Y\"".into());
        }
        ControllerType::PlayStation => {
            // PlayStation — same positions as Xbox but different labels
            overrides.insert("input_player1_a_btn", "1".into());       // Cross (south)
            overrides.insert("input_player1_b_btn", "0".into());       // Circle (east)
            overrides.insert("input_player1_x_btn", "3".into());       // Square (west)
            overrides.insert("input_player1_y_btn", "2".into());       // Triangle (north)
            overrides.insert("input_player1_a_btn_label", "\"Cross\"".into());
            overrides.insert("input_player1_b_btn_label", "\"Circle\"".into());
            overrides.insert("input_player1_x_btn_label", "\"Square\"".into());
            overrides.insert("input_player1_y_btn_label", "\"Triangle\"".into());
        }
        ControllerType::Nintendo => {
            // Nintendo — A/B swapped, X/Y swapped relative to Xbox
            // Nintendo A = east face, Nintendo B = south face
            overrides.insert("input_player1_a_btn", "0".into());       // east face (Nintendo A)
            overrides.insert("input_player1_b_btn", "1".into());       // south face (Nintendo B)
            overrides.insert("input_player1_x_btn", "2".into());       // north face (Nintendo X)
            overrides.insert("input_player1_y_btn", "3".into());       // west face (Nintendo Y)
            overrides.insert("input_player1_a_btn_label", "\"A\"".into());
            overrides.insert("input_player1_b_btn_label", "\"B\"".into());
            overrides.insert("input_player1_x_btn_label", "\"X\"".into());
            overrides.insert("input_player1_y_btn_label", "\"Y\"".into());
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
        QualityPreset::High => {
            overrides.insert("video_smooth", "true".into());
            overrides.insert("video_shader_enable", "false".into());
            overrides.insert("video_scale_integer", "false".into());
            overrides.insert("video_gpu_screenshot", "true".into());
        }
    }
}

/// Applies per-core internal resolution options to `retroarch-core-options.cfg`.
///
/// Each emulator core has its own option key for internal resolution scaling.
/// We write all known core resolution keys so the setting applies regardless
/// of which core ends up running the game.
fn apply_core_quality_options(overrides: &mut HashMap<&str, String>, quality: &QualityPreset) {
    let (dolphin_efb, n64_res, pcsx_rearmed, beetle_psx_res, ppsspp_res) = match quality {
        QualityPreset::Low => ("1", "320x240", "1", "1x(native)", "1"),
        QualityPreset::Medium => ("3", "640x480", "2", "2x(native)", "2"),
        QualityPreset::High => ("5", "1280x960", "4", "4x(native)", "4"),
    };

    // Dolphin (GameCube/Wii) — internal EFB scale
    overrides.insert("dolphin_efb_scale", format!("\"{}\"", dolphin_efb));

    // Mupen64Plus-Next (N64) — resolution
    overrides.insert("mupen64plus-Resolution", format!("\"{}\"", n64_res));
    overrides.insert("parallel-n64-screensize", format!("\"{}\"", n64_res));

    // PCSX ReARMed (PS1) — resolution multiplier
    overrides.insert("pcsx_rearmed_neon_enhancement_enable", format!("\"{}\"",
        if matches!(quality, QualityPreset::Low) { "disabled" } else { "enabled" }));
    overrides.insert("pcsx_rearmed_neon_enhancement_no_main", "\"disabled\"".into());

    // Beetle PSX HW (PS1 HW) — internal GPU resolution
    overrides.insert("beetle_psx_hw_internal_resolution", format!("\"{}\"", beetle_psx_res));

    // SwanStation (PS1) — GPU resolution scale
    overrides.insert("swanstation_GPU.ResolutionScale", format!("\"{}\"", pcsx_rearmed));

    // PPSSPP (PSP) — internal resolution
    overrides.insert("ppsspp_internal_resolution", format!("\"{}\"", ppsspp_res));

    // mGBA (GBA) — no internal resolution option, but color correction
    overrides.insert("mgba_color_correction", format!("\"{}\"",
        if matches!(quality, QualityPreset::Low) { "OFF" } else { "Game Boy Advance" }));

    // Snes9x — hi-res blending
    overrides.insert("snes9x_hires_blend", format!("\"{}\"",
        if matches!(quality, QualityPreset::Low) { "disabled" } else { "merge" }));
}

// ── Widescreen helpers ─────────────────────────────────────────────────

/// Applies widescreen settings to the main retroarch.cfg.
///
/// When enabled, sets the aspect ratio to 16:9. When disabled, uses the
/// core-provided aspect ratio (the default).
fn apply_widescreen(overrides: &mut HashMap<&str, String>, enabled: bool) {
    if enabled {
        // aspect_ratio_index: 1 = 16:9 in RetroArch's built-in list
        overrides.insert("aspect_ratio_index", "1".into());
        overrides.insert("video_aspect_ratio_auto", "false".into());
    } else {
        // aspect_ratio_index: 22 = "Core provided" (let the emulator decide)
        overrides.insert("aspect_ratio_index", "22".into());
        overrides.insert("video_aspect_ratio_auto", "true".into());
    }
}

/// Applies per-core widescreen hacks to `retroarch-core-options.cfg`.
///
/// Many cores have their own widescreen hack option that renders natively
/// wide instead of just stretching. We enable these when widescreen is on.
fn apply_core_widescreen_options(overrides: &mut HashMap<&str, String>, enabled: bool) {
    let val = if enabled { "enabled" } else { "disabled" };
    let on_off = if enabled { "ON" } else { "OFF" };

    // Dolphin (GameCube/Wii) — native widescreen hack
    overrides.insert("dolphin_widescreen_hack", format!("\"{}\"", val));

    // Mupen64Plus-Next (N64) — widescreen
    overrides.insert("mupen64plus-aspect", format!("\"{}\"",
        if enabled { "16:9" } else { "4:3" }));
    overrides.insert("parallel-n64-aspect", format!("\"{}\"",
        if enabled { "16:9" } else { "4:3" }));

    // PPSSPP (PSP) — native 16:9 support in many games
    overrides.insert("ppsspp_widescreen_hack", format!("\"{}\"", on_off));

    // Beetle PSX HW — widescreen mode hack
    overrides.insert("beetle_psx_hw_widescreen_hack", format!("\"{}\"", val));
    overrides.insert("beetle_psx_hw_widescreen_hack_aspect_ratio", "\"16:9\"".into());

    // SwanStation (PS1) — GPU widescreen hack
    overrides.insert("swanstation_GPU.WidescreenHack", format!("\"{}\"",
        if enabled { "true" } else { "false" }));

    // PCSX ReARMed — widescreen
    overrides.insert("pcsx_rearmed_widescreen", format!("\"{}\"", val));

    // Snes9x — widescreen (available in some builds)
    overrides.insert("snes9x_aspect_ratio", format!("\"{}\"",
        if enabled { "16:9" } else { "4:3" }));
}

/// Known mapping of ROM file extensions to RetroArch core name fragments.
/// Each entry is (extension, list of core name substrings to search for, in priority order).
/// Core filenames look like `mgba_libretro.dll` or `dolphin_libretro.so`.
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
    ("iso", &["dolphin", "pcsx2", "ppsspp", "mednafen_saturn"]),
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

/// Given a ROM file path and the RetroArch install directory, try to find a
/// matching core in `<emu_root>/cores/`. Returns the absolute path to the core
/// library if found, or `None` if no match.
pub fn resolve_core_for_rom(emu_root: &Path, rom_path: &str) -> Option<PathBuf> {
    let rom_ext = Path::new(rom_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())?;

    let candidates = EXTENSION_CORE_MAP
        .iter()
        .find(|(ext, _)| *ext == rom_ext.as_str())
        .map(|(_, cores)| *cores)?;

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