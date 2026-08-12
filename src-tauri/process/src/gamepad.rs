//! Gamepad input polling via the `gilrs` crate.
//!
//! ## Status: DORMANT — not wired into the running app.
//!
//! [`start_polling`] is **not called**. The call site in
//! `src-tauri/src/lib.rs` is commented out (see the `feat: Steam Deck native
//! support` commit): Big Picture Mode now reads controllers entirely in the
//! Vue layer via the browser Web Gamepad API (`main/composables/gamepad.ts`),
//! because gilrs's WGI (Windows Gaming Input) backend intermittently fails to
//! deliver input for controllers detected as generic HID devices.
//!
//! Nothing in the Vue layer listens for the `gamepad_*` events below — so
//! while this module still compiles (it is `pub`, hence no dead-code warning)
//! it emits into the void. It is retained as a ready fallback should the
//! webview Gamepad API ever prove insufficient (e.g. background input while
//! the webview is unfocused). **Do not rely on it for live input, and do not
//! "fix" frontend input bugs here — the live path is `gamepad.ts`.**
//!
//! When polling, it spawns one dedicated thread that polls connected
//! controllers at ~60Hz and emits normalised Tauri events:
//!
//! - `gamepad_button`  — button press / release
//! - `gamepad_axis`    — analog stick / trigger movement
//! - `gamepad_connected` / `gamepad_disconnected` — hot-plug events
//!
//! Uses **state-based polling** instead of gilrs events because the WGI
//! backend often fails to deliver events for generic HID devices.

use gilrs::{Axis, Button, GamepadId, Gilrs};
use log::{debug, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

/// Snapshot of one gamepad as seen by `gilrs` from Drop's *own* process.
///
/// Lets the frontend show the user which controllers Drop can see — useful
/// to confirm before launching an emulator that the OS-level input stack is
/// already happy. Native games and emulators (RetroArch, DuckStation,
/// PCSX2, …) inherit the same input access since Drop does not scrub any
/// HID / XInput / DirectInput env vars when spawning them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedGamepad {
    pub id: u32,
    pub name: String,
    pub connected: bool,
    pub power: String,
}

/// List controllers visible to Drop. Best-effort: if `Gilrs::new()` fails
/// (no input subsystem, permission error) we return an empty list rather
/// than an error — the "Drop sees nothing" answer is itself the diagnostic.
pub fn list_connected_gamepads() -> Vec<DetectedGamepad> {
    let Ok(g) = Gilrs::new() else {
        return Vec::new();
    };
    g.gamepads()
        .map(|(id, gp)| {
            let idx: usize = id.into();
            DetectedGamepad {
                id: idx as u32,
                name: gp.name().to_string(),
                connected: gp.is_connected(),
                power: format!("{:?}", gp.power_info()),
            }
        })
        .collect()
}

/// Format a gilrs UUID as an SDL GUID string: 32 lower-case hex chars, bytes
/// in order, no dashes.
///
/// gilrs builds its UUID in SDL's own GUID layout — its Linux backend has a
/// unit test asserting `create_uuid(bus 3, vendor 045e, product 028e, version
/// 2020) == "030000005e0400008e02000020200000"`, the well-known SDL GUID for a
/// wired Xbox 360 pad (`gilrs-core-0.6.7/src/platform/linux/gamepad.rs`).
///
/// Bytes 2-3 are zeroed on the way out. In SDL that field is a CRC of the
/// controller name, and the emulator clears it before matching
/// (`GetGUID`, eden `src/input_common/drivers/sdl_driver.cpp:19-26`), so a
/// GUID that keeps it would never match. gilrs already leaves it zero; doing
/// it here makes that a guarantee rather than a coincidence.
fn sdl_guid_string(uuid: [u8; 16]) -> String {
    let mut bytes = uuid;
    bytes[2] = 0;
    bytes[3] = 0;
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Rebuild an SDL GUID from a pad's USB vendor/product ids.
///
/// Needed on Windows: gilrs's default backend is Windows Gaming Input, which
/// returns a nil UUID for every device it recognises as a gamepad
/// (`gilrs-core-0.6.7/src/platform/windows_wgi/gamepad.rs:554-556`) while
/// still exposing `HardwareVendorId` / `HardwareProductId`. The layout is
/// SDL's: bus (USB = 3), name CRC, vendor, pad, product, pad, version, pad —
/// each a little-endian `u16`. Version is zero, which is also what SDL reports
/// for most Windows entries in its own controller database.
fn derived_sdl_guid(vendor: u16, product: u16) -> String {
    let mut bytes = [0u8; 16];
    bytes[0..2].copy_from_slice(&3u16.to_le_bytes()); // SDL_HARDWARE_BUS_USB
    bytes[4..6].copy_from_slice(&vendor.to_le_bytes());
    bytes[8..10].copy_from_slice(&product.to_le_bytes());
    sdl_guid_string(bytes)
}

/// Identify the controller an emulator should be bound to this launch.
///
/// Picks the first connected pad. `port` is always 0: the emulator's port is
/// the ordinal among joysticks sharing the *same* GUID, and the first pad with
/// a given GUID is by definition the zeroth — a second identical pad is the
/// one case this cannot resolve, and it guesses the first.
///
/// Returns `None` when nothing is connected, or when the pad exposes neither a
/// UUID nor a vendor/product pair, since a binding without a real GUID matches
/// no device and would only overwrite the user's working config with a dud.
pub fn resolve_primary_pad() -> Option<remote::switchemu::PadIdentity> {
    use remote::switchemu::{GuidSource, PadIdentity};

    let Ok(gilrs) = Gilrs::new() else {
        warn!("[GAMEPAD] Input backend unavailable — cannot resolve a pad for the emulator");
        return None;
    };

    for (_id, gp) in gilrs.gamepads() {
        if !gp.is_connected() {
            continue;
        }
        let uuid = gp.uuid();
        let (guid, guid_source) = if uuid != [0u8; 16] {
            (sdl_guid_string(uuid), GuidSource::Observed)
        } else if let (Some(vendor), Some(product)) = (gp.vendor_id(), gp.product_id()) {
            (
                derived_sdl_guid(vendor, product),
                GuidSource::DerivedFromVidPid,
            )
        } else {
            warn!(
                "[GAMEPAD] '{}' exposes neither a UUID nor vendor/product ids — skipping",
                gp.name()
            );
            continue;
        };

        info!(
            "[GAMEPAD] Emulator pad: '{}' guid={guid} source={guid_source:?}",
            gp.name()
        );
        return Some(PadIdentity {
            guid,
            port: 0,
            name: gp.name().to_string(),
            guid_source,
        });
    }

    info!("[GAMEPAD] No controller connected");
    None
}

/// Which button layout the first connected pad uses, for the RetroArch
/// fallback bindings.
///
/// Separate from [`resolve_primary_pad`] because it needs different facts: the
/// Switch-emulator writer needs an SDL GUID and refuses to write without one,
/// while RetroArch only needs to know *which family* the pad belongs to and has
/// a usable default when it can't tell. A pad that exposes no GUID at all —
/// which is the normal case on Windows Gaming Input — still has a vendor id and
/// a product name, so it is placeable here even though it is unusable there.
///
/// Returns [`PadFamily::Unknown`] when nothing is connected or the input
/// backend is unavailable, which the config writer treats as Xbox but logs as a
/// guess.
pub fn detect_primary_pad_family() -> remote::retroarch::PadFamily {
    use remote::retroarch::{detect_pad_family, PadFamily};

    let Ok(gilrs) = Gilrs::new() else {
        warn!("[GAMEPAD] Input backend unavailable — assuming a default pad layout");
        return PadFamily::Unknown;
    };

    for (_id, gp) in gilrs.gamepads() {
        if !gp.is_connected() {
            continue;
        }
        let family = detect_pad_family(gp.vendor_id(), gp.name());
        info!(
            "[GAMEPAD] Pad layout for '{}' (vendor={:?} product={:?}): {family:?}",
            gp.name(),
            gp.vendor_id(),
            gp.product_id()
        );
        return family;
    }

    info!("[GAMEPAD] No controller connected — using the default pad layout");
    PadFamily::Unknown
}

// ── Dead zone ────────────────────────────────────────────────────────────────

const STICK_DEAD_ZONE: f32 = 0.15;
const AXIS_CHANGE_THRESHOLD: f32 = 0.05;

/// How often (in ~60Hz frames) to re-emit every connected gamepad's current
/// axis values, even when nothing changed.
///
/// ## Why a delta filter needs a heartbeat
///
/// `gamepad_axis` events are only emitted on a *change* exceeding
/// [`AXIS_CHANGE_THRESHOLD`] (a delta filter — without it a held stick would
/// flood the bus at 60Hz). Any consumer that *caches* the last value it saw
/// is then exposed to a stale-cache hazard: if a stick settles after a move
/// such that the final step toward rest is below the threshold, the last
/// emitted (non-zero) value is never superseded and the consumer's cache
/// stays pinned away from the stick's true position indefinitely.
///
/// A periodic re-emit bounds that staleness: a settled / drifting stick
/// reliably reports its true value at least every `AXIS_HEARTBEAT_FRAMES`.
///
/// NOTE: this is defensive hardening for *this module's* (currently dormant —
/// see the module docs) event consumers. It does **not** address the live
/// BPM scroll-to-top bug: that bug is in `main/composables/gamepad.ts`, which
/// is an independent Web Gamepad API implementation with the exact same
/// delta-filter-without-heartbeat flaw and is the file that must be fixed.
const AXIS_HEARTBEAT_FRAMES: u64 = 30;


// ── Event payloads ───────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
pub struct GamepadButtonEvent {
    pub button: String,
    pub pressed: bool,
    pub controller_id: u32,
}

#[derive(Clone, Serialize)]
pub struct GamepadAxisEvent {
    pub axis: String,
    pub value: f32,
    pub controller_id: u32,
}

#[derive(Clone, Serialize)]
pub struct GamepadConnectionEvent {
    pub controller_id: u32,
    pub name: String,
}

// ── Button / axis lists to poll ──────────────────────────────────────────────

const ALL_BUTTONS: &[Button] = &[
    Button::South,
    Button::East,
    Button::North,
    Button::West,
    Button::LeftTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::Mode,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

const ALL_AXES: &[Axis] = &[
    Axis::LeftStickX,
    Axis::LeftStickY,
    Axis::RightStickX,
    Axis::RightStickY,
    Axis::LeftZ,
    Axis::RightZ,
];

// ── Name mapping ─────────────────────────────────────────────────────────────

fn button_name(button: Button) -> &'static str {
    match button {
        Button::South => "South",              // A / Cross
        Button::East => "East",                // B / Circle
        Button::North => "North",              // Y / Triangle
        Button::West => "West",                // X / Square
        Button::LeftTrigger => "LeftBumper",    // LB / L1
        Button::LeftTrigger2 => "LeftTrigger",  // LT / L2
        Button::RightTrigger => "RightBumper",  // RB / R1
        Button::RightTrigger2 => "RightTrigger",// RT / R2
        Button::Select => "Select",            // Back / Share
        Button::Start => "Start",              // Menu / Options
        Button::Mode => "Guide",               // Xbox / PS button
        Button::LeftThumb => "LeftStick",       // L3
        Button::RightThumb => "RightStick",     // R3
        Button::DPadUp => "DPadUp",
        Button::DPadDown => "DPadDown",
        Button::DPadLeft => "DPadLeft",
        Button::DPadRight => "DPadRight",
        Button::C => "C",
        Button::Z => "Z",
        Button::Unknown => "Unknown",
    }
}

fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::LeftStickX => "LeftStickX",
        Axis::LeftStickY => "LeftStickY",
        Axis::RightStickX => "RightStickX",
        Axis::RightStickY => "RightStickY",
        Axis::LeftZ => "LeftTrigger",
        Axis::RightZ => "RightTrigger",
        Axis::DPadX => "DPadX",
        Axis::DPadY => "DPadY",
        Axis::Unknown => "Unknown",
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn gamepad_id_to_u32(id: GamepadId) -> u32 {
    let idx: usize = id.into();
    idx as u32
}

fn apply_dead_zone(value: f32) -> f32 {
    if value.abs() < STICK_DEAD_ZONE {
        0.0
    } else {
        value
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn start_polling(app_handle: AppHandle) {
    if RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        debug!("[GAMEPAD] Polling thread already running");
        return;
    }

    std::thread::Builder::new()
        .name("gamepad-poll".into())
        .spawn(move || {
            poll_loop(app_handle);
        })
        .expect("Failed to spawn gamepad polling thread");

    info!("[GAMEPAD] Polling thread started");
}

pub fn stop_polling() {
    RUNNING.store(false, Ordering::SeqCst);
    info!("[GAMEPAD] Polling thread stop requested");
}

// ── State-based poll loop ────────────────────────────────────────────────────

fn poll_loop(app_handle: AppHandle) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            warn!("[GAMEPAD] Failed to initialise gilrs: {e}");
            RUNNING.store(false, Ordering::SeqCst);
            return;
        }
    };

    // Track which gamepads are known-connected
    let mut known_connected: HashMap<GamepadId, bool> = HashMap::new();

    // Previous-frame state for diffing
    let mut prev_buttons: HashMap<(u32, Button), bool> = HashMap::new();
    let mut prev_axes: HashMap<(u32, Axis), f32> = HashMap::new();

    // Emit initial connections
    for (id, gamepad) in gilrs.gamepads() {
        if gamepad.is_connected() {
            let cid = gamepad_id_to_u32(id);
            let name = gamepad.name().to_string();
            info!("[GAMEPAD] Found controller {cid}: {name} (power: {:?})", gamepad.power_info());
            known_connected.insert(id, true);
            let _ = app_handle.emit(
                "gamepad_connected",
                GamepadConnectionEvent {
                    controller_id: cid,
                    name,
                },
            );
        }
    }

    let mut frame_count: u64 = 0;
    let mut event_count: u64 = 0;

    while RUNNING.load(Ordering::SeqCst) {
        // Drain gilrs internal event queue (required to keep state fresh)
        while let Some(ev) = gilrs.next_event() {
            event_count += 1;
            // Log first 10 raw events to see what gilrs is actually producing
            if event_count <= 10 {
                info!("[GAMEPAD] Raw event #{event_count}: {:?}", ev);
            }
        }

        // Check for connection / disconnection changes
        for (id, gamepad) in gilrs.gamepads() {
            let was_connected = known_connected.get(&id).copied().unwrap_or(false);
            let is_connected = gamepad.is_connected();

            if is_connected && !was_connected {
                let cid = gamepad_id_to_u32(id);
                let name = gamepad.name().to_string();
                info!("[GAMEPAD] Controller connected: {name} (id {cid})");
                known_connected.insert(id, true);
                let _ = app_handle.emit(
                    "gamepad_connected",
                    GamepadConnectionEvent {
                        controller_id: cid,
                        name,
                    },
                );
            } else if !is_connected && was_connected {
                let cid = gamepad_id_to_u32(id);
                info!("[GAMEPAD] Controller disconnected: id {cid}");
                known_connected.insert(id, false);
                // Drop this controller's diff state. Otherwise a reconnect
                // (gilrs reuses the GamepadId) inherits stale prev values:
                // a button still recorded as "pressed" would suppress the
                // first real press, and a stale axis value would suppress
                // the first real movement until it happened to cross the
                // change threshold.
                prev_buttons.retain(|(k_cid, _), _| *k_cid != cid);
                prev_axes.retain(|(k_cid, _), _| *k_cid != cid);
                let _ = app_handle.emit(
                    "gamepad_disconnected",
                    GamepadConnectionEvent {
                        controller_id: cid,
                        name: String::new(),
                    },
                );
            }
        }

        // Poll state for each connected gamepad
        for (id, gamepad) in gilrs.gamepads() {
            if !gamepad.is_connected() {
                continue;
            }
            let cid = gamepad_id_to_u32(id);

            // ── Buttons ──────────────────────────────────────────────
            for &button in ALL_BUTTONS {
                let pressed = gamepad.is_pressed(button);
                let key = (cid, button);
                let was_pressed = prev_buttons.get(&key).copied().unwrap_or(false);

                if pressed != was_pressed {
                    prev_buttons.insert(key, pressed);
                    let name = button_name(button);

                    debug!("[GAMEPAD] {} {}", name, if pressed { "PRESSED" } else { "released" });

                    let _ = app_handle.emit(
                        "gamepad_button",
                        GamepadButtonEvent {
                            button: name.to_string(),
                            pressed,
                            controller_id: cid,
                        },
                    );
                }
            }

            // ── Axes ─────────────────────────────────────────────────
            // Emit on a real change (delta filter — keeps a moving stick
            // from flooding the bus) OR on the periodic heartbeat (so the
            // frontend's cached value can never go stale against a settled
            // / drifting stick — see AXIS_HEARTBEAT_FRAMES).
            let axis_heartbeat = frame_count.is_multiple_of(AXIS_HEARTBEAT_FRAMES);
            for &axis in ALL_AXES {
                let raw = gamepad.value(axis);
                let filtered = apply_dead_zone(raw);
                let key = (cid, axis);
                let prev = prev_axes.get(&key).copied().unwrap_or(0.0);

                let changed = (filtered - prev).abs() >= AXIS_CHANGE_THRESHOLD;
                if changed || axis_heartbeat {
                    prev_axes.insert(key, filtered);

                    let name = axis_name(axis);
                    let _ = app_handle.emit(
                        "gamepad_axis",
                        GamepadAxisEvent {
                            axis: name.to_string(),
                            value: filtered,
                            controller_id: cid,
                        },
                    );
                }
            }
        }

        frame_count += 1;

        // Every ~5 seconds, dump diagnostic state
        if frame_count.is_multiple_of(300) {
            for (id, gamepad) in gilrs.gamepads() {
                let cid = gamepad_id_to_u32(id);
                let mut pressed_list = Vec::new();
                for &button in ALL_BUTTONS {
                    if gamepad.is_pressed(button) {
                        pressed_list.push(format!("{:?}", button));
                    }
                }
                let mut axis_list = Vec::new();
                for &axis in ALL_AXES {
                    let v = gamepad.value(axis);
                    if v.abs() > 0.01 {
                        axis_list.push(format!("{:?}={:.2}", axis, v));
                    }
                }
                info!(
                    "[GAMEPAD] Diag frame={} events={} cid={} connected={} pressed=[{}] axes=[{}]",
                    frame_count,
                    event_count,
                    cid,
                    gamepad.is_connected(),
                    pressed_list.join(", "),
                    axis_list.join(", "),
                );
            }
        }

        // ~60Hz
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    info!("[GAMEPAD] Polling thread exiting");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_formats_as_an_sdl_guid() {
        // gilrs's own Linux backend test vector: bus 3, vendor 045e,
        // product 028e, version 2020 — a wired Xbox 360 pad.
        let uuid = [
            0x03, 0x00, 0x00, 0x00, 0x5e, 0x04, 0x00, 0x00, 0x8e, 0x02, 0x00, 0x00, 0x20, 0x20,
            0x00, 0x00,
        ];
        assert_eq!(sdl_guid_string(uuid), "030000005e0400008e02000020200000");
    }

    #[test]
    fn name_crc_bytes_are_cleared() {
        let mut uuid = [0u8; 16];
        uuid[0] = 0x03;
        uuid[2] = 0xab; // SDL's controller-name CRC
        uuid[3] = 0xcd;
        uuid[4] = 0x5e;
        uuid[5] = 0x04;
        assert_eq!(sdl_guid_string(uuid), "030000005e0400000000000000000000");
    }

    #[test]
    fn derived_guid_matches_the_sdl_layout() {
        // Same pad, version zero — the shape SDL reports for most Windows
        // entries in its controller database.
        assert_eq!(
            derived_sdl_guid(0x045e, 0x028e),
            "030000005e0400008e02000000000000"
        );
    }
}
