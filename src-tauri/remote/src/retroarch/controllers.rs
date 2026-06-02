//! Controller layout, hotkeys and per-core button remaps.
//!
//! RetroArch's RetroPad mirrors the Xbox layout (A=south, B=east, X=west,
//! Y=north). Two mechanisms set bindings:
//!
//! * **`retroarch.cfg` `input_player1_*` keys** — loaded *before* SDL2
//!   autoconfig, so autoconfig can override them. Drop writes them as a
//!   positional fallback for when no autoconfig profile matches the pad.
//! * **`.rmp` remap files** — loaded *after* autoconfig, at the core
//!   interface level, so they reliably win. Drop uses these for the
//!   Nintendo A<->B / X<->Y swap.
//!
//! All writes are idempotent and the cleanup helpers remove stale `.rmp`
//! files when the user switches layout, so config survives a core update.

use database::models::data::ControllerType;
use log::{info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// `.rmp` content swapping A<->B and X<->Y on the RetroPad.
///
/// RetroArch remap indices map RetroPad buttons to RetroPad buttons:
/// `0=B 1=Y 2=Select 3=Start 4=Up 5=Down 6=Left 7=Right 8=A 9=X 10=L 11=R
/// 12=L2 13=R2 14=L3 15=R3`.
const NINTENDO_REMAP_CONTENT: &str = "\
input_player1_btn_b = 8\n\
input_player1_btn_y = 9\n\
input_player1_btn_a = 0\n\
input_player1_btn_x = 1\n";

/// Core directory names that receive a Nintendo remap. Each gets a
/// `<core>/<core>.rmp` file under the remaps directory.
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

/// Applies the controller layout for the selected family into `overrides`.
///
/// * **Xbox / PlayStation** — positional XInput fallback bindings, stale
///   Nintendo remaps cleaned up.
/// * **Nintendo** — same base bindings plus `.rmp` files that swap A<->B and
///   X<->Y for every known core.
pub fn apply_controller_mappings(
    overrides: &mut HashMap<&str, String>,
    controller: &ControllerType,
    remaps_dir: &Path,
) {
    // Display labels are identical for all families.
    overrides.insert("input_player1_a_btn_label", "\"A\"".into());
    overrides.insert("input_player1_b_btn_label", "\"B\"".into());
    overrides.insert("input_player1_x_btn_label", "\"X\"".into());
    overrides.insert("input_player1_y_btn_label", "\"Y\"".into());

    // XInput positional face-button fallback for when autoconfig finds no
    // matching profile (Drop's portable RetroArch may ship none):
    //   Xbox A (south/btn 0) -> RetroPad B (south)
    //   Xbox B (east/btn 1)  -> RetroPad A (east)
    //   Xbox X (west/btn 2)  -> RetroPad Y (west)
    //   Xbox Y (north/btn 3) -> RetroPad X (north)
    // If autoconfig DOES match, it overrides these at runtime — harmless.
    set_xinput_positional_fallback(overrides);

    match controller {
        ControllerType::Xbox | ControllerType::PlayStation => {
            cleanup_nintendo_remaps(remaps_dir);
        }
        ControllerType::Nintendo => {
            write_nintendo_remaps(remaps_dir);
        }
    }
}

/// Sets the XInput positional face-button fallback bindings. Used for the
/// explicit-controller path and the "Auto" (no controller selected) path.
pub fn set_xinput_positional_fallback(overrides: &mut HashMap<&str, String>) {
    overrides.insert("input_player1_b_btn", "0".into());
    overrides.insert("input_player1_a_btn", "1".into());
    overrides.insert("input_player1_y_btn", "2".into());
    overrides.insert("input_player1_x_btn", "3".into());
}

/// Writes Nintendo A<->B / X<->Y remap files for every known core.
pub fn write_nintendo_remaps(remaps_dir: &Path) {
    for core_name in REMAP_CORE_NAMES {
        let core_dir = remaps_dir.join(core_name);
        if let Err(e) = fs::create_dir_all(&core_dir) {
            warn!("[RETROARCH] Failed to create remap dir {}: {e}", core_dir.display());
            continue;
        }
        let rmp_path = core_dir.join(format!("{core_name}.rmp"));
        if let Err(e) = fs::write(&rmp_path, NINTENDO_REMAP_CONTENT) {
            warn!("[RETROARCH] Failed to write remap {}: {e}", rmp_path.display());
        }
    }
    info!(
        "[RETROARCH] Wrote Nintendo A<->B/X<->Y remap files for {} cores",
        REMAP_CORE_NAMES.len()
    );
}

/// Removes Nintendo remap files for every known core — used when switching
/// back to an Xbox/Auto layout so a stale swap doesn't linger.
pub fn cleanup_nintendo_remaps(remaps_dir: &Path) {
    for core_name in REMAP_CORE_NAMES {
        let rmp_path = remaps_dir.join(core_name).join(format!("{core_name}.rmp"));
        if rmp_path.exists()
            && let Err(e) = fs::remove_file(&rmp_path) {
                warn!("[RETROARCH] Failed to remove remap {}: {e}", rmp_path.display());
            }
    }
}

/// Writes core-specific A<->B-only remaps for Nintendo console emulators
/// (Dolphin for GC/Wii, Mupen64Plus for N64) when the controller is *not* in
/// Nintendo mode.
///
/// These cores map the console's A button (right-side position) to RetroPad B,
/// so on an Xbox-layout pad pressing physical A sends the wrong input. This
/// fixes that without the full X<->Y swap that Nintendo mode applies.
///
/// `.rmp` button indices: `0=B 1=Y 2=Select 3=Start 4=Up 5=Down 6=Left
/// 7=Right 8=A 9=X 10=L 11=R 12=L2 13=R2 14=L3 15=R3`.
pub fn write_nintendo_core_remaps(emu_root: &Path, appimage_config_dir: &Option<std::path::PathBuf>) {
    // Swap A(8)<->B(0); everything else stays default.
    const REMAP_CONTENT: &str = r#"input_player1_btn_a = "0"
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
    const NINTENDO_CORES: &[&str] = &["dolphin-emu", "Mupen64Plus-Next", "parallel_n64"];

    for core_name in NINTENDO_CORES {
        let remap_dir = emu_root.join("config").join("remaps").join(core_name);
        write_remap_file(&remap_dir, core_name, REMAP_CONTENT);

        if let Some(ai_cfg_dir) = appimage_config_dir {
            let ai_remap_dir = ai_cfg_dir.join("config").join("remaps").join(core_name);
            write_remap_file(&ai_remap_dir, core_name, REMAP_CONTENT);
        }
    }
}

/// Writes a single `.rmp` file, creating its directory first.
fn write_remap_file(remap_dir: &Path, core_name: &str, content: &str) {
    if let Err(e) = fs::create_dir_all(remap_dir) {
        warn!("[RETROARCH] Failed to create remap dir {}: {e}", remap_dir.display());
        return;
    }
    let remap_path = remap_dir.join(format!("{core_name}.rmp"));
    match fs::write(&remap_path, content) {
        Ok(_) => info!("[RETROARCH] Wrote remap file: {}", remap_path.display()),
        Err(e) => warn!("[RETROARCH] Failed to write remap {}: {e}", remap_path.display()),
    }
}

/// RetroArch hotkey buttons we deliberately do NOT bind. Autoconfig profiles
/// regularly assign these (Home → menu_toggle, Select → screenshot, Back →
/// rewind, etc.), and because we patch retroarch.cfg without explicitly
/// nullifying them they end up active in-game — producing the "I pressed
/// some random button and a shortcut fired I didn't ask for / multiple ways
/// to activate the same thing" pattern users report. Setting each to "nul"
/// in our cfg overrides the autoconfig-assigned value.
///
/// We do NOT nullify the actions we bind ourselves (`exit_emulator`,
/// `save_state`, `load_state`, `toggle_fast_forward`, `state_slot_*`,
/// `enable_hotkey`) — those are reasserted below.
const UNUSED_HOTKEY_BUTTONS: &[&str] = &[
    "input_menu_toggle_btn",
    "input_menu_toggle_axis",
    "input_pause_toggle_btn",
    "input_pause_toggle_axis",
    "input_screenshot_btn",
    "input_screenshot_axis",
    "input_reset_btn",
    "input_reset_axis",
    "input_rewind_btn",
    "input_rewind_axis",
    "input_grab_mouse_toggle_btn",
    "input_audio_mute_btn",
    "input_volume_up_btn",
    "input_volume_down_btn",
    "input_movie_record_toggle_btn",
    "input_disk_eject_toggle_btn",
    "input_disk_next_btn",
    "input_disk_prev_btn",
    "input_cheat_toggle_btn",
    "input_cheat_index_plus_btn",
    "input_cheat_index_minus_btn",
    "input_shader_toggle_btn",
    "input_shader_next_btn",
    "input_shader_prev_btn",
    "input_recording_toggle_btn",
    "input_streaming_toggle_btn",
    "input_runahead_toggle_btn",
    "input_ai_service_btn",
    "input_vrr_runloop_toggle_btn",
    "input_fps_toggle_btn",
    "input_overlay_next_btn",
    "input_netplay_game_watch_btn",
    "input_netplay_flip_players_btn",
];

/// Inserts keyboard + controller hotkey bindings into `overrides`.
///
/// Keyboard hotkeys work on all platforms. Controller combos hold R3 + a
/// button; the button indices differ by input driver, so the binding set is
/// `cfg`-gated by OS.
///
/// Anything in `UNUSED_HOTKEY_BUTTONS` is nullified so autoconfig profiles
/// can't sneak in a binding for it — without that step a controller's
/// Home / Select / Back / Touchpad would still trigger menu_toggle /
/// screenshot / rewind under whatever pad-specific autoconfig file
/// matches, even though we never set those keys ourselves.
pub fn apply_hotkey_bindings(overrides: &mut HashMap<&str, String>) {
    // Step 1: block autoconfig from claiming hotkey buttons we don't use.
    // Has to land BEFORE the explicit binds below so they win for the
    // buttons we *do* claim.
    for key in UNUSED_HOTKEY_BUTTONS {
        overrides.insert(key, "nul".into());
    }

    // Keyboard hotkeys — explicit so they survive a base config that disables
    // them: Escape=quit, F2=save, F4=load, Space=fast-forward.
    overrides.insert("input_exit_emulator", "escape".into());
    overrides.insert("input_save_state", "f2".into());
    overrides.insert("input_load_state", "f4".into());
    overrides.insert("input_toggle_fast_forward", "space".into());
    overrides.insert("input_state_slot_increase", "f7".into());
    overrides.insert("input_state_slot_decrease", "f6".into());

    // Controller combos — hold R3 (right-stick click) + press a button.
    //   SDL2 (Linux):  R3=8 Start=6 L1=9 R1=10 R2(btn)=5 DL=13 DR=14
    //   XInput (Win):   R3=9 Start=7 LB=4 RB=5 RT=axis+5
    #[cfg(target_os = "linux")]
    {
        overrides.insert("input_enable_hotkey_btn", "8".into()); // R3
        overrides.insert("input_exit_emulator_btn", "6".into()); // Start
        overrides.insert("input_save_state_btn", "10".into()); // R1
        overrides.insert("input_load_state_btn", "9".into()); // L1
        overrides.insert("input_toggle_fast_forward_btn", "5".into()); // R2 (button)
        // Steam Deck triggers are analog — some SDL2 configs only fire the
        // axis event, so set both.
        overrides.insert("input_toggle_fast_forward_axis", "+5".into()); // RT axis
        overrides.insert("input_state_slot_increase_btn", "14".into()); // DPad Right
        overrides.insert("input_state_slot_decrease_btn", "13".into()); // DPad Left
    }
    #[cfg(not(target_os = "linux"))]
    {
        overrides.insert("input_enable_hotkey_btn", "9".into()); // R3
        overrides.insert("input_exit_emulator_btn", "7".into()); // Start
        overrides.insert("input_save_state_btn", "5".into()); // RB
        overrides.insert("input_load_state_btn", "4".into()); // LB
        overrides.insert("input_toggle_fast_forward_axis", "+5".into()); // RT axis
        // XInput DPad isn't buttons — use F6/F7 keyboard for slot nav.
    }
    info!(
        "[RETROARCH] Applied hotkey bindings (keyboard + R3 controller combos; {} unused hotkey buttons nullified)",
        UNUSED_HOTKEY_BUTTONS.len()
    );
}

/// Config keys to delete on every patch — stale settings from older Drop
/// versions that, left behind, override RetroArch's built-in defaults.
///
/// `input_autodetect_enable` and `input_player1_{a,b,x,y}_btn` are *not*
/// listed: they are explicitly re-set every launch.
pub const STALE_INPUT_KEYS: &[&str] = &[
    // Old empty autoconfig dir caused "not configured" fallback warnings.
    "joypad_autoconfig_dir",
    // Old Nintendo mode manually mapped every axis/button/trigger; these stale
    // keys override autoconfig and break sticks if left behind.
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
];
