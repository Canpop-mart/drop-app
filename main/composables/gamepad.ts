/**
 * Gamepad input via the Web Gamepad API (navigator.getGamepads).
 *
 * Replaces the previous gilrs/Tauri-event approach because the gilrs WGI
 * backend on Windows intermittently fails to deliver input for generic HID
 * controllers.  The browser Gamepad API works reliably in Tauri's webview.
 *
 * Polls at ~60 Hz via requestAnimationFrame, diffs state each frame, and
 * fires the same callback interface the rest of Big Picture Mode expects.
 */

import { devLog } from "./dev-mode";

// ── Types ────────────────────────────────────────────────────────────────────

export interface GamepadButtonEvent {
  button: string;
  pressed: boolean;
  controller_id: number;
}

export interface GamepadAxisEvent {
  axis: string;
  value: number;
  controller_id: number;
}

export interface GamepadConnectionEvent {
  controller_id: number;
  name: string;
}

export type ButtonCallback = (event: GamepadButtonEvent) => void;

export interface ButtonSubOptions {
  /**
   * If true, this subscriber still receives events while the global input
   * lock is held (i.e. while a modal / overlay has acquired focus-nav's
   * input lock).  Default false — page-level handlers are auto-silenced
   * when a modal opens, so they don't compete with its key handlers.
   *
   * Modal / overlay components that wire their own gamepad handlers for
   * in-modal navigation should pass `{ bypassInputLock: true }` so their
   * handlers keep firing while the lock they acquired is active.
   */
  bypassInputLock?: boolean;
}

// ── Standard button names ───────────────────────────────────────────────────

export const GamepadButton = {
  South: "South", // A / Cross
  East: "East", // B / Circle
  North: "North", // Y / Triangle
  West: "West", // X / Square
  LeftBumper: "LeftBumper", // LB / L1
  RightBumper: "RightBumper", // RB / R1
  LeftTrigger: "LeftTrigger", // LT / L2
  RightTrigger: "RightTrigger", // RT / R2
  Select: "Select", // Back / Share
  Start: "Start", // Menu / Options
  Guide: "Guide", // Xbox / PS button
  LeftStick: "LeftStick", // L3
  RightStick: "RightStick", // R3
  DPadUp: "DPadUp",
  DPadDown: "DPadDown",
  DPadLeft: "DPadLeft",
  DPadRight: "DPadRight",
} as const;

// ── Web Gamepad API button index → name mapping ─────────────────────────────
// Standard Gamepad layout: https://w3c.github.io/gamepad/#remapping

const BUTTON_MAP_STANDARD: Record<number, string> = {
  0: GamepadButton.South,
  1: GamepadButton.East,
  2: GamepadButton.West,
  3: GamepadButton.North,
  4: GamepadButton.LeftBumper,
  5: GamepadButton.RightBumper,
  6: GamepadButton.LeftTrigger,
  7: GamepadButton.RightTrigger,
  8: GamepadButton.Select,
  9: GamepadButton.Start,
  10: GamepadButton.LeftStick,
  11: GamepadButton.RightStick,
  12: GamepadButton.DPadUp,
  13: GamepadButton.DPadDown,
  14: GamepadButton.DPadLeft,
  15: GamepadButton.DPadRight,
  16: GamepadButton.Guide,
};

// Active button map — always standard. The Deck X↔Y swap is handled at the
// handler registration level (each page swaps which logical button triggers
// search vs sort on Gamescope) rather than at the mapping level, because the
// mapping approach had reliability issues with Steam Input on Windows.
const activeButtonMap = BUTTON_MAP_STANDARD;

const AXIS_NAMES: Record<number, string> = {
  0: "LeftStickX",
  1: "LeftStickY",
  2: "RightStickX",
  3: "RightStickY",
};

// ── Constants ───────────────────────────────────────────────────────────────

const STICK_DEAD_ZONE = 0.15;
const AXIS_CHANGE_THRESHOLD = 0.05;
const TRIGGER_PRESS_THRESHOLD = 0.5; // triggers are analog 0→1

// ── Singleton state ──────────────────────────────────────────────────────────

const buttons = reactive(new Map<string, boolean>());
const axes = reactive(new Map<string, number>());
const connected = ref(false);
const controllerName = ref("");
const controllerId = ref<number | null>(null);

interface ButtonSub {
  fn: ButtonCallback;
  bypassLock: boolean;
}

const buttonCallbacks = new Map<string, Set<ButtonSub>>();
const buttonReleaseCallbacks = new Map<string, Set<ButtonSub>>();
const anyButtonCallbacks = new Set<ButtonSub>();

/**
 * Global input lock set by `focus-navigation.ts` when a modal acquires
 * the input lock.  When true, subscribers without `bypassInputLock: true`
 * do NOT receive button events — their handlers are silenced for the
 * duration of the lock so they don't compete with the modal's own handlers.
 */
let globalInputLock = false;

export function setGlobalInputLock(value: boolean) {
  if (globalInputLock !== value) {
    devLog("state", `globalInputLock ${globalInputLock} -> ${value}`);
  }
  globalInputLock = value;
}

// Previous frame state for diffing
const prevButtons = new Map<string, boolean>();
const prevAxes = new Map<string, number>();

let polling = false;
let rafId: number | null = null;

// ── Polling loop ────────────────────────────────────────────────────────────

function applyDeadZone(value: number): number {
  return Math.abs(value) < STICK_DEAD_ZONE ? 0 : value;
}

function pollFrame() {
  if (!polling) return;

  const gamepads = navigator.getGamepads();
  let foundConnected = false;

  for (const gp of gamepads) {
    if (!gp || !gp.connected) continue;
    foundConnected = true;

    const cid = gp.index;

    // Detect new connection
    if (!connected.value || controllerId.value !== cid) {
      connected.value = true;
      controllerId.value = cid;
      controllerName.value = gp.id;

      devLog("gamepad",
        `[GAMEPAD] Controller connected: ${gp.id} (index ${cid})`,
      );
    }

    // ── Buttons ──────────────────────────────────────────────────
    for (let i = 0; i < gp.buttons.length; i++) {
      const name = activeButtonMap[i];
      if (!name) continue;

      const btn = gp.buttons[i];
      // Triggers (index 6,7) use analog value, others use .pressed
      const pressed =
        i === 6 || i === 7 ? btn.value > TRIGGER_PRESS_THRESHOLD : btn.pressed;

      const wasPressedPrev = prevButtons.get(name) ?? false;

      if (pressed !== wasPressedPrev) {
        prevButtons.set(name, pressed);
        buttons.set(name, pressed);

        const payload: GamepadButtonEvent = {
          button: name,
          pressed,
          controller_id: cid,
        };

        if (pressed) {
          const cbs = buttonCallbacks.get(name);
          const subCount = (cbs?.size ?? 0) + anyButtonCallbacks.size;
          devLog(
            "gamepad",
            `press ${name} (cid=${cid}, subs=${subCount}, lock=${globalInputLock})`,
          );
          if (cbs) {
            for (const sub of cbs) {
              if (globalInputLock && !sub.bypassLock) continue;
              sub.fn(payload);
            }
          }
          for (const sub of anyButtonCallbacks) {
            if (globalInputLock && !sub.bypassLock) continue;
            sub.fn(payload);
          }
        } else {
          const cbs = buttonReleaseCallbacks.get(name);
          devLog(
            "gamepad",
            `release ${name} (cid=${cid}, subs=${cbs?.size ?? 0})`,
          );
          if (cbs) {
            for (const sub of cbs) {
              if (globalInputLock && !sub.bypassLock) continue;
              sub.fn(payload);
            }
          }
        }
      }
    }

    // ── Axes ─────────────────────────────────────────────────────
    for (let i = 0; i < Math.min(gp.axes.length, 4); i++) {
      const name = AXIS_NAMES[i];
      if (!name) continue;

      const filtered = applyDeadZone(gp.axes[i]);
      const prev = prevAxes.get(name) ?? 0;

      if (Math.abs(filtered - prev) >= AXIS_CHANGE_THRESHOLD) {
        prevAxes.set(name, filtered);
        axes.set(name, filtered);
        devLog("gamepad", `axis ${name}=${filtered.toFixed(2)} (cid=${cid})`);
      }
    }

    // Only process first connected gamepad
    break;
  }

  if (!foundConnected && connected.value) {
    connected.value = false;
    controllerName.value = "";
    controllerId.value = null;
    buttons.clear();
    axes.clear();
    prevButtons.clear();
    prevAxes.clear();
    devLog("gamepad","[GAMEPAD] Controller disconnected");
  }

  rafId = requestAnimationFrame(pollFrame);
}

function startPolling() {
  if (polling) return;
  polling = true;
  devLog("gamepad","[GAMEPAD] Web Gamepad API polling started");
  rafId = requestAnimationFrame(pollFrame);
}

function stopPolling() {
  polling = false;
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

// ── Init: start polling + listen for connect/disconnect ─────────────────────

let initialized = false;

function init() {
  if (initialized) return;
  initialized = true;

  // Listen for browser gamepad events to wake up polling
  window.addEventListener("gamepadconnected", (e) => {
    devLog("gamepad",`[GAMEPAD] gamepadconnected: ${e.gamepad.id}`);
    if (!polling) startPolling();
  });

  window.addEventListener("gamepaddisconnected", (e) => {
    devLog("gamepad",`[GAMEPAD] gamepaddisconnected: ${e.gamepad.id}`);
  });

  // Start polling immediately — some browsers don't fire gamepadconnected
  // until a button is pressed, and the controller may already be connected
  startPolling();
}

// ── Destroy / full teardown ─────────────────────────────────────────────────

/**
 * Completely shuts down the gamepad subsystem: stops polling, clears all
 * callbacks and state.  Called when exiting Big Picture Mode so the
 * animation-frame loop doesn't keep running in the background.
 */
function destroy() {
  stopPolling();

  // Clear all callback registrations
  buttonCallbacks.clear();
  buttonReleaseCallbacks.clear();
  anyButtonCallbacks.clear();

  // Reset connection state
  connected.value = false;
  controllerName.value = "";
  controllerId.value = null;
  buttons.clear();
  axes.clear();
  prevButtons.clear();
  prevAxes.clear();

  // Release the global input lock — when BPM is torn down any modal
  // that was holding it is also gone, so we shouldn't leak a locked
  // state into the next session.
  globalInputLock = false;

  // Allow re-initialization next time BPM is entered
  initialized = false;
  devLog("gamepad","[GAMEPAD] Destroyed — polling stopped, callbacks cleared");
}

// ── Composable ───────────────────────────────────────────────────────────────

export function useGamepad() {
  init();

  function onButton(
    button: string,
    callback: ButtonCallback,
    options?: ButtonSubOptions,
  ): () => void {
    if (!buttonCallbacks.has(button)) {
      buttonCallbacks.set(button, new Set());
    }
    const sub: ButtonSub = {
      fn: callback,
      bypassLock: options?.bypassInputLock ?? false,
    };
    buttonCallbacks.get(button)!.add(sub);
    return () => {
      buttonCallbacks.get(button)?.delete(sub);
    };
  }

  function onButtonRelease(
    button: string,
    callback: ButtonCallback,
    options?: ButtonSubOptions,
  ): () => void {
    if (!buttonReleaseCallbacks.has(button)) {
      buttonReleaseCallbacks.set(button, new Set());
    }
    const sub: ButtonSub = {
      fn: callback,
      bypassLock: options?.bypassInputLock ?? false,
    };
    buttonReleaseCallbacks.get(button)!.add(sub);
    return () => {
      buttonReleaseCallbacks.get(button)?.delete(sub);
    };
  }

  function onAnyButton(
    callback: ButtonCallback,
    options?: ButtonSubOptions,
  ): () => void {
    const sub: ButtonSub = {
      fn: callback,
      bypassLock: options?.bypassInputLock ?? false,
    };
    anyButtonCallbacks.add(sub);
    return () => {
      anyButtonCallbacks.delete(sub);
    };
  }

  function isPressed(button: string): boolean {
    return buttons.get(button) ?? false;
  }

  function axisValue(axis: string): number {
    return axes.get(axis) ?? 0;
  }

  /**
   * Trigger haptic feedback on the connected gamepad.
   * Uses the Gamepad API's vibrationActuator for rumble effects.
   *
   * @param type - "light" for navigation, "medium" for selection, "heavy" for errors
   */
  function vibrate(type: "light" | "medium" | "heavy" = "light") {
    if (controllerId.value == null) return;
    // Respect the user's haptic feedback preference
    if (
      typeof localStorage !== "undefined" &&
      localStorage.getItem("drop:haptic") === "false"
    )
      return;
    try {
      const gamepads = navigator.getGamepads();
      const gp = gamepads[controllerId.value];
      if (!gp?.vibrationActuator) return;

      const profiles = {
        light: { duration: 30, weakMagnitude: 0.15, strongMagnitude: 0.0 },
        medium: { duration: 60, weakMagnitude: 0.3, strongMagnitude: 0.15 },
        heavy: { duration: 120, weakMagnitude: 0.6, strongMagnitude: 0.4 },
      };
      const p = profiles[type];
      (gp.vibrationActuator as any).playEffect?.("dual-rumble", {
        startDelay: 0,
        duration: p.duration,
        weakMagnitude: p.weakMagnitude,
        strongMagnitude: p.strongMagnitude,
      });
    } catch {
      // Haptic feedback is best-effort — silently ignore failures
    }
  }

  return {
    buttons: readonly(buttons),
    axes: readonly(axes),
    connected: readonly(connected),
    controllerName: readonly(controllerName),
    controllerId: readonly(controllerId),

    onButton,
    onButtonRelease,
    onAnyButton,
    isPressed,
    axisValue,
    vibrate,
    destroy,
    stopPolling,
    startPolling,
  };
}
