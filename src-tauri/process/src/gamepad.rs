//! Gamepad input polling via the `gilrs` crate.
//!
//! Spawns a dedicated thread that polls connected controllers at ~60Hz and
//! emits normalised Tauri events to the Vue frontend:
//!
//! - `gamepad_button`  — button press / release
//! - `gamepad_axis`    — analog stick / trigger movement
//! - `gamepad_connected` / `gamepad_disconnected` — hot-plug events
//!
//! Uses **state-based polling** instead of gilrs events because the WGI
//! (Windows Gaming Input) backend often fails to deliver events for
//! controllers detected as generic HID devices.

use gilrs::{Axis, Button, GamepadId, Gilrs};
use log::{debug, info, warn};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

// ── Dead zone ────────────────────────────────────────────────────────────────

const STICK_DEAD_ZONE: f32 = 0.15;
const AXIS_CHANGE_THRESHOLD: f32 = 0.05;


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
            for &axis in ALL_AXES {
                let raw = gamepad.value(axis);
                let filtered = apply_dead_zone(raw);
                let key = (cid, axis);
                let prev = prev_axes.get(&key).copied().unwrap_or(0.0);

                if (filtered - prev).abs() >= AXIS_CHANGE_THRESHOLD {
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
        if frame_count % 300 == 0 {
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
                    "[GAMEPAD] Diag frame={} events={} connected={} pressed=[{}] axes=[{}]",
                    frame_count,
                    event_count,
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
