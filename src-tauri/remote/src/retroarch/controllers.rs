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
//!
//! # Why the fallback is per-pad-family and not one XInput table
//!
//! Every `input_player1_*_btn` value is a **raw joypad-driver button index**,
//! and which physical button carries which index depends on the driver the pad
//! ends up on:
//!
//! * On Windows an Xbox pad goes through RetroArch's **xinput** driver, whose
//!   numbering is fixed by XInput itself: A=0 B=1 X=2 Y=3, LB=4 RB=5, Back=6
//!   Start=7, L3=8 R3=9, triggers on axes 4/5.
//! * Everything that is *not* an XInput device — a DualSense, a Switch Pro
//!   pad — falls through to **DirectInput**, where the index is the pad's own
//!   HID report order. A DualSense reports Square first, so a table written
//!   for XInput rotates every face button by one position, and the hotkey
//!   combos land on buttons the pad either doesn't have or uses for something
//!   else. That is the "A is Square and none of the shortcuts work" report.
//! * On Linux both go through **udev/evdev**, which normalises to
//!   BTN_SOUTH/EAST/NORTH/WEST regardless of vendor, so the families converge
//!   and Drop keeps one table there (see [`apply_hotkey_bindings`]).
//!
//! The Windows per-family numbers below are not guesses: they are read off the
//! `platform:Windows` entries of the SDL controller database vendored at
//! `remote/src/switchemu/gamecontrollerdb.txt`, which is the same source the
//! Switch-emulator writer resolves its raw indices from. SDL's `a`/`b`/`x`/`y`
//! elements are positional (south/east/west/north), so they translate directly.

use database::models::data::ControllerType;
use log::{info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── Pad detection ────────────────────────────────────────────────────────

/// The physical button layout a connected pad uses, which is what decides the
/// raw indices Drop writes.
///
/// Distinct from [`ControllerType`], which is the user's *per-game preference*
/// (and additionally selects the Nintendo A<->B label swap). When the user
/// leaves that on "Auto" this is what Drop falls back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadFamily {
    Xbox,
    PlayStation,
    Nintendo,
    /// Nothing connected, or a pad Drop cannot place. Treated as Xbox, because
    /// that is both the most common pad and the numbering RetroArch's own
    /// Windows default assumes — but logged as a guess rather than a decision.
    Unknown,
}

impl PadFamily {
    /// The layout to actually write. `Unknown` resolves to Xbox.
    fn resolved(self) -> Self {
        match self {
            PadFamily::Unknown => PadFamily::Xbox,
            other => other,
        }
    }
}

// USB vendor ids of the pad makers whose layouts Drop can name outright.
// Anything else falls through to the product-name check below. Valve is here
// because the Steam Deck's built-in pad and the Steam Controller are both
// XInput-positional (`a:b0,b:b1,x:b2,y:b3` in SDL's database).
const VENDOR_SONY: u16 = 0x054c;
const VENDOR_NINTENDO: u16 = 0x057e;
const VENDOR_MICROSOFT: u16 = 0x045e;
const VENDOR_VALVE: u16 = 0x28de;

/// Place a pad in a [`PadFamily`] from what the input backend can actually see.
///
/// Vendor id first, because it is exact. Product name second, because plenty of
/// third-party pads clone a first-party layout under their own vendor id (an
/// 8BitDo in PlayStation mode, a Hori Switch pad), and because a DualSense
/// paired over Bluetooth on Windows reports the generic name
/// `Wireless Controller` with no other clue.
///
/// Deliberately *not* taking a product id: no branch here needs one, and an
/// argument the function never reads would imply a precision it doesn't have.
pub fn detect_pad_family(vendor_id: Option<u16>, name: &str) -> PadFamily {
    match vendor_id {
        Some(VENDOR_SONY) => return PadFamily::PlayStation,
        Some(VENDOR_NINTENDO) => return PadFamily::Nintendo,
        Some(VENDOR_MICROSOFT) | Some(VENDOR_VALVE) => return PadFamily::Xbox,
        _ => {}
    }

    let name = name.trim().to_lowercase();

    // Sony's bare generic HID name, which is what a DualSense reports when
    // paired over Bluetooth on Windows. Matched whole, not as a substring:
    // half the third-party pads on the market have "wireless controller"
    // somewhere in their name and are not PlayStation-layout.
    if name == "wireless controller" {
        return PadFamily::PlayStation;
    }

    const PLAYSTATION_NAMES: &[&str] =
        &["dualsense", "dualshock", "playstation", "ps3", "ps4", "ps5"];
    const NINTENDO_NAMES: &[&str] = &[
        "nintendo",
        "switch",
        "joy-con",
        "joycon",
        "pro controller",
        "gamecube",
    ];
    const XBOX_NAMES: &[&str] = &["xbox", "x-box", "xinput"];

    if PLAYSTATION_NAMES.iter().any(|n| name.contains(n)) {
        PadFamily::PlayStation
    } else if NINTENDO_NAMES.iter().any(|n| name.contains(n)) {
        PadFamily::Nintendo
    } else if XBOX_NAMES.iter().any(|n| name.contains(n)) {
        PadFamily::Xbox
    } else {
        PadFamily::Unknown
    }
}

/// A pad family's face buttons by *position*, as raw joypad-driver indices.
///
/// Positions, not labels: `south` is the bottom button whatever its face says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceButtons {
    pub south: u32,
    pub east: u32,
    pub west: u32,
    pub north: u32,
}

/// How a pad reports its right trigger, which differs by family and decides
/// whether the fast-forward hotkey is a `_btn` or an `_axis` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerBinding {
    /// Analog — written as `input_*_axis = "+N"`.
    Axis(u32),
    /// Digital — written as `input_*_btn = "N"`.
    Button(u32),
}

/// The hotkey-combo indices Drop binds on Windows for one pad family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsHotkeys {
    /// Right-stick click — the modifier every combo is held with.
    pub right_stick: u32,
    pub start: u32,
    pub left_shoulder: u32,
    pub right_shoulder: u32,
    pub right_trigger: TriggerBinding,
}

/// The Windows face-button indices for `family`.
///
/// Sources, all `platform:Windows` rows of the vendored SDL controller
/// database except Xbox, which is fixed by the XInput API itself:
///
/// * **Xbox** — XInput ordering. RetroArch's `xinput` joypad driver is what
///   claims these pads on Windows, and it exposes XInput's own indices.
/// * **PlayStation** — `PS4 Controller` / `PS5 Controller`: `a:b1,b:b2,x:b0,
///   y:b3`. Every modern Sony-vendor row in the database agrees.
/// * **Nintendo** — `Nintendo Switch Pro Controller`: `a:b0,b:b1,x:b2,y:b3`,
///   which happens to coincide with XInput. (The Nintendo *label* swap is a
///   separate mechanism — see [`write_nintendo_remaps`].)
pub const fn windows_face_buttons(family: PadFamily) -> FaceButtons {
    match family {
        PadFamily::PlayStation => FaceButtons { south: 1, east: 2, west: 0, north: 3 },
        PadFamily::Xbox | PadFamily::Nintendo | PadFamily::Unknown => {
            FaceButtons { south: 0, east: 1, west: 2, north: 3 }
        }
    }
}

/// The Windows hotkey indices for `family`.
///
/// Same database rows as [`windows_face_buttons`]:
/// PlayStation `start:b9,rightstick:b11,leftshoulder:b4,rightshoulder:b5,
/// righttrigger:a4`; Nintendo Pro `start:b9,rightstick:b11,leftshoulder:b4,
/// rightshoulder:b5,righttrigger:b7` (digital, not an axis); Xbox is XInput's
/// `Start=7,R3=9,LB=4,RB=5,RT=axis 5`.
///
/// This is why the shortcuts died alongside the face buttons: on a DualSense
/// the old XInput table put the hotkey modifier on button 9, which is Start,
/// and `exit_emulator` on button 7, which that pad does not report at all.
pub const fn windows_hotkeys(family: PadFamily) -> WindowsHotkeys {
    match family {
        PadFamily::PlayStation => WindowsHotkeys {
            right_stick: 11,
            start: 9,
            left_shoulder: 4,
            right_shoulder: 5,
            right_trigger: TriggerBinding::Axis(4),
        },
        PadFamily::Nintendo => WindowsHotkeys {
            right_stick: 11,
            start: 9,
            left_shoulder: 4,
            right_shoulder: 5,
            right_trigger: TriggerBinding::Button(7),
        },
        PadFamily::Xbox | PadFamily::Unknown => WindowsHotkeys {
            right_stick: 9,
            start: 7,
            left_shoulder: 4,
            right_shoulder: 5,
            right_trigger: TriggerBinding::Axis(5),
        },
    }
}

/// The face-button indices for `family` on the host Drop is running on.
///
/// Linux collapses every family onto one positional table. RetroArch's default
/// Linux joypad driver is `udev`, and the kernel's own pad drivers (`xpad`,
/// `hid-playstation`, `hid-nintendo`) all report BTN_SOUTH/EAST/WEST/NORTH in
/// the same order, so a DualSense and an Xbox pad are indistinguishable at this
/// layer — which is why this bug only reproduces on Windows.
pub const fn face_buttons(family: PadFamily) -> FaceButtons {
    #[cfg(target_os = "linux")]
    {
        let _ = family;
        FaceButtons { south: 0, east: 1, west: 2, north: 3 }
    }
    #[cfg(not(target_os = "linux"))]
    {
        windows_face_buttons(family)
    }
}

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

/// The pad family implied by an explicit user choice.
pub fn family_for_controller_type(controller: &ControllerType) -> PadFamily {
    match controller {
        ControllerType::Xbox => PadFamily::Xbox,
        ControllerType::PlayStation => PadFamily::PlayStation,
        ControllerType::Nintendo => PadFamily::Nintendo,
    }
}

/// Applies the controller layout the user explicitly chose into `overrides`.
///
/// The choice sets the physical button numbering for all three families, and
/// **Nintendo** additionally writes the `.rmp` files that swap A<->B and X<->Y
/// so the on-screen labels match a Nintendo pad's faceplate. Xbox and
/// PlayStation clean those files back up.
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

    set_face_button_fallback(overrides, family_for_controller_type(controller));

    match controller {
        ControllerType::Xbox | ControllerType::PlayStation => {
            cleanup_nintendo_remaps(remaps_dir);
        }
        ControllerType::Nintendo => {
            write_nintendo_remaps(remaps_dir);
        }
    }
}

/// Writes the positional face-button fallback for `family`, used when
/// autoconfig finds no matching profile (Drop's portable RetroArch may ship
/// none — the log line for that case is `[Autoconf] ... not configured, using
/// fallback`). If autoconfig *does* match, it overrides these at runtime,
/// which is harmless.
///
/// The mapping is position to position: the pad's south button drives RetroPad
/// B, east drives A, west drives Y and north drives X, because RetroPad's own
/// letters follow the Xbox faceplate.
pub fn set_face_button_fallback(overrides: &mut HashMap<&str, String>, family: PadFamily) {
    let face = face_buttons(family);
    overrides.insert("input_player1_b_btn", face.south.to_string());
    overrides.insert("input_player1_a_btn", face.east.to_string());
    overrides.insert("input_player1_y_btn", face.west.to_string());
    overrides.insert("input_player1_x_btn", face.north.to_string());
    info!(
        "[RETROARCH] Face-button fallback for {:?} pad: south={} east={} west={} north={}",
        family.resolved(),
        face.south,
        face.east,
        face.west,
        face.north
    );
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
/// button; the indices come from the same [`PadFamily`] the face buttons were
/// written from, so a pad can never end up with correct face buttons and dead
/// shortcuts (or the reverse).
///
/// Anything in `UNUSED_HOTKEY_BUTTONS` is nullified so autoconfig profiles
/// can't sneak in a binding for it — without that step a controller's
/// Home / Select / Back / Touchpad would still trigger menu_toggle /
/// screenshot / rewind under whatever pad-specific autoconfig file
/// matches, even though we never set those keys ourselves.
pub fn apply_hotkey_bindings(overrides: &mut HashMap<&str, String>, family: PadFamily) {
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
    #[cfg(target_os = "linux")]
    {
        // udev/SDL2 numbering, which the kernel normalises across vendors —
        // see `face_buttons`. These are the values validated on the Steam Deck.
        //   R3=8 Start=6 L1=9 R1=10 R2(btn)=5 DL=13 DR=14
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
        let hk = windows_hotkeys(family);
        overrides.insert("input_enable_hotkey_btn", hk.right_stick.to_string());
        overrides.insert("input_exit_emulator_btn", hk.start.to_string());
        overrides.insert("input_save_state_btn", hk.right_shoulder.to_string());
        overrides.insert("input_load_state_btn", hk.left_shoulder.to_string());
        // Fast-forward sits on the right trigger, which is an axis on XInput
        // and DirectInput PlayStation pads but a plain button on a Switch Pro
        // pad. Writing the wrong key type is silent — the binding parses and
        // then never fires. Both keys are written every launch, the unused one
        // as "nul", so switching families can't leave the previous family's
        // binding behind still firing.
        let (axis_value, btn_value) = match hk.right_trigger {
            TriggerBinding::Axis(axis) => (format!("+{axis}"), "nul".to_string()),
            TriggerBinding::Button(btn) => ("nul".to_string(), btn.to_string()),
        };
        overrides.insert("input_toggle_fast_forward_axis", axis_value);
        overrides.insert("input_toggle_fast_forward_btn", btn_value);
        // The D-pad is a hat on all three families here, not buttons — slot
        // navigation stays on the F6/F7 keyboard hotkeys set above.
    }
    info!(
        "[RETROARCH] Applied hotkey bindings for {:?} pad (keyboard + R3 combos; {} unused hotkey buttons nullified)",
        family.resolved(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Detection ────────────────────────────────────────────────────────

    #[test]
    fn vendor_id_places_first_party_pads() {
        assert_eq!(detect_pad_family(Some(0x054c), ""), PadFamily::PlayStation);
        assert_eq!(detect_pad_family(Some(0x057e), ""), PadFamily::Nintendo);
        assert_eq!(detect_pad_family(Some(0x045e), ""), PadFamily::Xbox);
        // The Deck's built-in pad and the Steam Controller are both
        // XInput-positional.
        assert_eq!(detect_pad_family(Some(0x28de), ""), PadFamily::Xbox);
    }

    #[test]
    fn product_name_places_pads_with_an_unknown_vendor() {
        // A DualSense over Bluetooth on Windows: third-party-looking vendor,
        // Sony's generic HID product name.
        assert_eq!(
            detect_pad_family(None, "Wireless Controller"),
            PadFamily::PlayStation
        );
        // ...but "wireless controller" as a *substring* must not drag every
        // third-party pad into the PlayStation table.
        assert_eq!(
            detect_pad_family(Some(0x2dc8), "8BitDo Ultimate 2 Wireless Controller"),
            PadFamily::Unknown
        );
        assert_eq!(
            detect_pad_family(Some(0x2dc8), "8BitDo SN30 Pro for PS4"),
            PadFamily::PlayStation
        );
        assert_eq!(
            detect_pad_family(Some(0x0f0d), "HORIPAD for Nintendo Switch"),
            PadFamily::Nintendo
        );
        assert_eq!(
            detect_pad_family(Some(0x24c6), "PowerA Xbox Series Controller"),
            PadFamily::Xbox
        );
    }

    #[test]
    fn vendor_id_wins_over_a_misleading_name() {
        // Sony's own pad, marketed with "Xbox-style" in some third-party
        // listings — the vendor id is the fact, the name is marketing.
        assert_eq!(
            detect_pad_family(Some(0x054c), "Xbox Style Layout Pad"),
            PadFamily::PlayStation
        );
    }

    #[test]
    fn an_unplaceable_pad_is_unknown_not_silently_xbox() {
        let family = detect_pad_family(Some(0x1234), "Generic USB Joystick");
        assert_eq!(family, PadFamily::Unknown);
        // It still *resolves* to Xbox for the config write; the point is that
        // the caller can tell a guess from a decision.
        assert_eq!(family.resolved(), PadFamily::Xbox);
    }

    #[test]
    fn detection_is_case_insensitive() {
        assert_eq!(detect_pad_family(None, "DUALSENSE WIRELESS"), PadFamily::PlayStation);
        assert_eq!(detect_pad_family(None, "nintendo switch pro"), PadFamily::Nintendo);
    }

    // ── Face-button tables ───────────────────────────────────────────────
    //
    // Asserted against the vendored SDL controller database's
    // `platform:Windows` rows, whose a/b/x/y elements are positional
    // (south/east/west/north).

    #[test]
    fn xbox_face_buttons_follow_xinput() {
        let f = windows_face_buttons(PadFamily::Xbox);
        assert_eq!(f, FaceButtons { south: 0, east: 1, west: 2, north: 3 });
    }

    #[test]
    fn playstation_face_buttons_are_rotated_from_xinput() {
        // `PS5 Controller,a:b1,b:b2,x:b0,y:b3,...,platform:Windows`
        let f = windows_face_buttons(PadFamily::PlayStation);
        assert_eq!(f, FaceButtons { south: 1, east: 2, west: 0, north: 3 });
        // The regression this whole change exists for: RetroPad B (the menu's
        // "OK") used to be bound to index 0, which on this pad is Square.
        assert_ne!(f.south, windows_face_buttons(PadFamily::Xbox).south);
    }

    #[test]
    fn switch_pro_face_buttons_coincide_with_xinput() {
        // `Nintendo Switch Pro Controller,a:b0,b:b1,x:b2,y:b3,...`
        assert_eq!(
            windows_face_buttons(PadFamily::Nintendo),
            windows_face_buttons(PadFamily::Xbox)
        );
    }

    #[test]
    fn unknown_face_buttons_fall_back_to_xbox() {
        assert_eq!(
            windows_face_buttons(PadFamily::Unknown),
            windows_face_buttons(PadFamily::Xbox)
        );
    }

    #[test]
    fn every_family_maps_the_four_positions_to_distinct_indices() {
        for family in [
            PadFamily::Xbox,
            PadFamily::PlayStation,
            PadFamily::Nintendo,
            PadFamily::Unknown,
        ] {
            let f = windows_face_buttons(family);
            let mut seen = vec![f.south, f.east, f.west, f.north];
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), 4, "{family:?} maps two positions to one index");
        }
    }

    // ── Hotkey tables ────────────────────────────────────────────────────

    #[test]
    fn hotkeys_differ_where_the_database_says_they_differ() {
        let xbox = windows_hotkeys(PadFamily::Xbox);
        assert_eq!(xbox.right_stick, 9);
        assert_eq!(xbox.start, 7);
        assert_eq!(xbox.right_trigger, TriggerBinding::Axis(5));

        // `PS5 Controller,...,start:b9,rightstick:b11,righttrigger:a4`
        let ps = windows_hotkeys(PadFamily::PlayStation);
        assert_eq!(ps.right_stick, 11);
        assert_eq!(ps.start, 9);
        assert_eq!(ps.right_trigger, TriggerBinding::Axis(4));

        // `Nintendo Switch Pro Controller,...,righttrigger:b7` — digital.
        let switch = windows_hotkeys(PadFamily::Nintendo);
        assert_eq!(switch.right_stick, 11);
        assert_eq!(switch.start, 9);
        assert_eq!(switch.right_trigger, TriggerBinding::Button(7));
    }

    #[test]
    fn shoulders_are_the_same_index_on_every_family() {
        for family in [PadFamily::Xbox, PadFamily::PlayStation, PadFamily::Nintendo] {
            let hk = windows_hotkeys(family);
            assert_eq!((hk.left_shoulder, hk.right_shoulder), (4, 5), "{family:?}");
        }
    }

    // ── Config writes ────────────────────────────────────────────────────

    #[test]
    fn face_fallback_writes_position_to_retropad_letter() {
        let mut overrides = HashMap::new();
        set_face_button_fallback(&mut overrides, PadFamily::PlayStation);
        let face = face_buttons(PadFamily::PlayStation);
        // RetroPad letters follow the Xbox faceplate: B=south, A=east,
        // Y=west, X=north.
        assert_eq!(overrides["input_player1_b_btn"], face.south.to_string());
        assert_eq!(overrides["input_player1_a_btn"], face.east.to_string());
        assert_eq!(overrides["input_player1_y_btn"], face.west.to_string());
        assert_eq!(overrides["input_player1_x_btn"], face.north.to_string());
    }

    #[test]
    fn hotkeys_always_write_both_fast_forward_keys() {
        // Switching family must not leave the previous family's trigger
        // binding live, so exactly one of the pair is real and the other
        // is explicitly nulled.
        for family in [PadFamily::Xbox, PadFamily::PlayStation, PadFamily::Nintendo] {
            let mut overrides = HashMap::new();
            apply_hotkey_bindings(&mut overrides, family);
            let axis = &overrides["input_toggle_fast_forward_axis"];
            let btn = &overrides["input_toggle_fast_forward_btn"];
            assert!(
                (axis == "nul") != (btn == "nul"),
                "{family:?}: axis={axis} btn={btn}"
            );
        }
    }

    #[test]
    fn the_hotkey_modifier_matches_the_family_table() {
        let mut overrides = HashMap::new();
        apply_hotkey_bindings(&mut overrides, PadFamily::PlayStation);
        #[cfg(not(target_os = "linux"))]
        assert_eq!(overrides["input_enable_hotkey_btn"], "11");
        #[cfg(target_os = "linux")]
        assert_eq!(overrides["input_enable_hotkey_btn"], "8");
    }

    #[test]
    fn explicit_controller_choice_maps_onto_a_family() {
        assert_eq!(
            family_for_controller_type(&ControllerType::PlayStation),
            PadFamily::PlayStation
        );
        assert_eq!(family_for_controller_type(&ControllerType::Xbox), PadFamily::Xbox);
        assert_eq!(
            family_for_controller_type(&ControllerType::Nintendo),
            PadFamily::Nintendo
        );
    }
}
