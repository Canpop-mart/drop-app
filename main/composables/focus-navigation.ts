import { GamepadButton, useGamepad, type ButtonCallback } from "./gamepad";

// ── Types ────────────────────────────────────────────────────────────────────

export interface FocusableElement {
  el: HTMLElement;
  group: string;
  onSelect?: () => void;
  onContext?: () => void;
}

interface FocusGroup {
  elements: Set<FocusableElement>;
  lastFocused: FocusableElement | null;
}

// ── Direction helpers ────────────────────────────────────────────────────────

type Direction = "up" | "down" | "left" | "right";

function getCenter(el: HTMLElement): { x: number; y: number } {
  const rect = el.getBoundingClientRect();
  return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
}

/**
 * Given a direction, find the nearest focusable element from `current`.
 * Uses a cone-shaped search: strongly prefer elements in the pressed
 * direction, with a secondary preference for proximity.
 *
 * M2 fix: reduced threshold from 10px to 2px so tightly-packed grids
 * don't skip elements.
 */
function findNearest(
  current: HTMLElement,
  candidates: HTMLElement[],
  direction: Direction,
): HTMLElement | null {
  const from = getCenter(current);
  let best: HTMLElement | null = null;
  let bestScore = Infinity;

  for (const candidate of candidates) {
    if (candidate === current) continue;
    const to = getCenter(candidate);
    const dx = to.x - from.x;
    const dy = to.y - from.y;

    // Filter: candidate must be in the correct direction (2px threshold)
    const inDirection =
      (direction === "up" && dy < -2) ||
      (direction === "down" && dy > 2) ||
      (direction === "left" && dx < -2) ||
      (direction === "right" && dx > 2);

    if (!inDirection) continue;

    // Score: prefer alignment on the perpendicular axis
    const primary =
      direction === "up" || direction === "down" ? Math.abs(dy) : Math.abs(dx);
    const secondary =
      direction === "up" || direction === "down" ? Math.abs(dx) : Math.abs(dy);

    // Weighted score: perpendicular misalignment costs 2x distance
    const score = primary + secondary * 2;

    if (score < bestScore) {
      bestScore = score;
      best = candidate;
    }
  }

  return best;
}

// ── Singleton state ──────────────────────────────────────────────────────────

const groups = reactive(new Map<string, FocusGroup>());
const currentFocused = ref<FocusableElement | null>(null);
const currentGroup = ref<string>("");
const enabled = ref(false);

/**
 * When true, the focus system ignores ALL gamepad input.
 * Used by overlays like the on-screen keyboard that handle
 * D-pad / A / B themselves and don't want the focus system
 * to also react.
 */
const inputLocked = ref(false);

/**
 * Right stick polling for continuous scrolling.
 * Scrolls the focused element's container when right stick Y is moved.
 */
let stickPollInterval: ReturnType<typeof setInterval> | null = null;

/**
 * Ownership-based input lock.  Each acquireInputLock() returns a unique ID.
 * releaseInputLock(id) only unlocks if the caller is still the current owner.
 * This prevents a race where an old component's onUnmounted undoes a newer
 * component's lock (e.g. navigating between two iframe pages).
 */
let _inputLockId = 0;

// Ordered list of group names for LB/RB cycling
const groupOrder = ref<string[]>([]);

// ── CSS class applied to focused element ─────────────────────────────────────

const FOCUS_CLASS = "bp-focused";

function applyFocus(element: FocusableElement | null, fromGroupCycle = false) {
  // Remove from previous
  if (currentFocused.value) {
    currentFocused.value.el.classList.remove(FOCUS_CLASS);
  }

  currentFocused.value = element;

  if (element) {
    element.el.classList.add(FOCUS_CLASS);
    // When cycling groups, use 'center' to give more prominent visual feedback
    const scrollBlock = fromGroupCycle ? "center" : "nearest";
    element.el.scrollIntoView({ block: scrollBlock, behavior: "smooth" });
    currentGroup.value = element.group;

    // Update group memory
    const group = groups.get(element.group);
    if (group) {
      group.lastFocused = element;
    }
  }
}

// ── Navigation ───────────────────────────────────────────────────────────────

function navigate(direction: Direction) {
  if (!currentFocused.value) {
    // Focus first element in current group or first available
    const group =
      groups.get(currentGroup.value) || groups.values().next().value;
    if (group && group.elements.size > 0) {
      applyFocus(group.lastFocused || (group.elements.values().next().value ?? null));
    }
    return;
  }

  // Find candidates within the same group
  const group = groups.get(currentFocused.value.group);
  if (!group) return;

  const candidates = Array.from(group.elements)
    .filter((f) => f.el.isConnected) // skip detached elements
    .map((f) => f.el);

  const next = findNearest(currentFocused.value.el, candidates, direction);
  if (next) {
    const focusable = Array.from(group.elements).find((f) => f.el === next);
    if (focusable) {
      applyFocus(focusable);
      // Haptic feedback on navigation
      const gp = useGamepad();
      gp.vibrate("light");
    }
  }
}

function cycleGroup(forward: boolean) {
  if (groupOrder.value.length <= 1) return;

  // M3 fix: if current group not in order list, start from index 0
  let idx = groupOrder.value.indexOf(currentGroup.value);
  if (idx === -1) idx = 0;

  // Try each group in order, skipping empty ones
  for (let i = 1; i < groupOrder.value.length; i++) {
    const nextIdx = forward
      ? (idx + i) % groupOrder.value.length
      : (idx - i + groupOrder.value.length) % groupOrder.value.length;

    const nextGroupName = groupOrder.value[nextIdx];
    const nextGroup = groups.get(nextGroupName);

    // Skip empty groups (e.g. iframe pages with no content elements)
    if (!nextGroup || nextGroup.elements.size === 0) continue;

    // Restore last focused or pick first
    const target =
      nextGroup.lastFocused || (nextGroup.elements.values().next().value ?? null);
    applyFocus(target, true); // true indicates this is from a group cycle
    return;
  }
}

// ── D-pad repeat for held buttons ────────────────────────────────────────────

const REPEAT_DELAY = 250; // ms before repeat starts
const REPEAT_RATE_INITIAL = 80; // ms between repeats initially
const REPEAT_RATE_FAST = 50; // ms between repeats after acceleration
const REPEAT_ACCELERATION_MS = 400; // L2 fix: switch to fast rate after 400ms of repeating

let repeatTimer: ReturnType<typeof setTimeout> | null = null;
let repeatInterval: ReturnType<typeof setInterval> | null = null;
let repeatStartTime = 0;

function startRepeat(direction: Direction) {
  stopRepeat();
  repeatStartTime = Date.now();
  repeatTimer = setTimeout(() => {
    navigate(direction);
    // L2 fix: start at initial rate, switch to fast rate based on elapsed time
    repeatInterval = setInterval(() => {
      navigate(direction);
      // After REPEAT_ACCELERATION_MS, switch to faster rate
      if (
        Date.now() - repeatStartTime > REPEAT_ACCELERATION_MS &&
        repeatInterval
      ) {
        clearInterval(repeatInterval);
        repeatInterval = setInterval(() => {
          navigate(direction);
        }, REPEAT_RATE_FAST);
      }
    }, REPEAT_RATE_INITIAL);
  }, REPEAT_DELAY);
}

function stopRepeat() {
  if (repeatTimer) {
    clearTimeout(repeatTimer);
    repeatTimer = null;
  }
  if (repeatInterval) {
    clearInterval(repeatInterval);
    repeatInterval = null;
  }
  repeatStartTime = 0;
}

// ── Composable ───────────────────────────────────────────────────────────────

let gamepadWired = false;
// C5 fix: store all gamepad unsubscribe functions for cleanup
const gamepadUnsubs: (() => void)[] = [];

export function useFocusNavigation() {
  if (!gamepadWired) {
    gamepadWired = true;
    wireGamepad();
  }

  function registerElement(
    el: HTMLElement,
    group: string,
    options?: {
      onSelect?: () => void;
      onContext?: () => void;
    },
  ) {
    if (!groups.has(group)) {
      groups.set(group, { elements: new Set(), lastFocused: null });
    }

    const focusable: FocusableElement = {
      el,
      group,
      onSelect: options?.onSelect,
      onContext: options?.onContext,
    };

    groups.get(group)!.elements.add(focusable);

    // Mark element for CSS targeting
    el.setAttribute("data-focusable", "");

    // Return unregister function
    return () => {
      const g = groups.get(group);
      if (g) {
        g.elements.delete(focusable);
        if (g.lastFocused === focusable) g.lastFocused = null;
        if (currentFocused.value === focusable) {
          currentFocused.value = null;
        }
      }
    };
  }

  function setGroupOrder(order: string[]) {
    groupOrder.value = order;
  }

  function focusGroup(groupName: string) {
    const group = groups.get(groupName);
    if (!group || group.elements.size === 0) return;
    const target = group.lastFocused || (group.elements.values().next().value ?? null);
    applyFocus(target);
  }

  function clearFocus() {
    applyFocus(null);
  }

  /**
   * Acquire the input lock. Returns a unique ID the caller must
   * pass to releaseInputLock() when it wants to give up the lock.
   */
  function acquireInputLock(): number {
    _inputLockId++;
    inputLocked.value = true;
    return _inputLockId;
  }

  /**
   * Release the input lock — but only if the caller is still the
   * current owner (i.e. no newer lock was acquired in the meantime).
   */
  function releaseInputLock(id: number) {
    if (id === _inputLockId) {
      inputLocked.value = false;
    }
  }

  /**
   * Completely tear down the focus navigation system.
   * Called when exiting Big Picture Mode.
   */
  function destroy() {
    // Stop repeat timers
    stopRepeat();

    // Stop stick polling
    if (stickPollInterval) {
      clearInterval(stickPollInterval);
      stickPollInterval = null;
    }

    // C5 fix: unsubscribe all gamepad bindings
    for (const unsub of gamepadUnsubs) unsub();
    gamepadUnsubs.length = 0;

    // Clear all focus groups and state
    groups.clear();
    currentFocused.value = null;
    currentGroup.value = "";
    enabled.value = false;
    inputLocked.value = false;
    groupOrder.value = [];

    // Allow re-wiring on next BPM enter
    gamepadWired = false;

    console.log("[FOCUS-NAV] Destroyed — all listeners removed");
  }

  return {
    // State
    currentFocused: readonly(currentFocused),
    currentGroup: readonly(currentGroup),
    enabled,
    inputLocked,

    // Methods
    registerElement,
    setGroupOrder,
    focusGroup,
    clearFocus,
    navigate,
    cycleGroup,
    acquireInputLock,
    releaseInputLock,
    destroy,
  };
}

// ── Wire gamepad buttons to focus system ─────────────────────────────────────

function wireGamepad() {
  const gamepad = useGamepad();

  // D-pad navigation
  const directionMap: Record<string, Direction> = {
    [GamepadButton.DPadUp]: "up",
    [GamepadButton.DPadDown]: "down",
    [GamepadButton.DPadLeft]: "left",
    [GamepadButton.DPadRight]: "right",
  };

  // C5 fix: store all unsubscribe functions
  // D-pad press → navigate + start repeat
  for (const [button, direction] of Object.entries(directionMap)) {
    gamepadUnsubs.push(
      gamepad.onButton(button, () => {
        if (!enabled.value || inputLocked.value) return;
        navigate(direction);
        startRepeat(direction);
      }),
    );

    // D-pad release → stop repeat
    gamepadUnsubs.push(
      gamepad.onButtonRelease(button, () => {
        stopRepeat();
      }),
    );
  }

  // A = Select / Confirm
  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!enabled.value || inputLocked.value) return;
      stopRepeat();
      if (currentFocused.value?.onSelect) {
        gamepad.vibrate("medium");
        currentFocused.value.onSelect();
      } else if (currentFocused.value?.el) {
        gamepad.vibrate("medium");
        currentFocused.value.el.click();
      }
    }),
  );

  // B = Back (focus hierarchy: content → nav, nav → do nothing)
  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!enabled.value || inputLocked.value) return;
      stopRepeat();

      // If we're in a group other than "nav", go back to nav
      if (currentGroup.value && currentGroup.value !== "nav") {
        const navGroup = groups.get("nav");
        if (navGroup && navGroup.elements.size > 0) {
          const target =
            navGroup.lastFocused || (navGroup.elements.values().next().value ?? null);
          applyFocus(target);
          return;
        }
      }

      // L1 fix: use router's own history tracking instead of unreliable window.history.length
      const router = useRouter();
      const path = router.currentRoute.value.path;
      const segments = path
        .replace("/bigpicture/", "")
        .split("/")
        .filter(Boolean);

      if (segments.length > 1) {
        // On a deep page (e.g. /bigpicture/library/xyz) — navigate to parent
        const parentPath = "/bigpicture/" + segments[0];
        router.push(parentPath);
      }
    }),
  );

  // X = Context action
  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.West, () => {
      if (!enabled.value || inputLocked.value) return;
      if (currentFocused.value?.onContext) {
        currentFocused.value.onContext();
      }
    }),
  );

  // LB/RB = Cycle groups
  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.LeftBumper, () => {
      if (!enabled.value || inputLocked.value) return;
      cycleGroup(false);
    }),
  );

  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.RightBumper, () => {
      if (!enabled.value || inputLocked.value) return;
      cycleGroup(true);
    }),
  );

  // Right stick Y axis scrolling for native pages
  // M5 fix: added acceleration curve — stronger stick deflection = faster scroll
  function startStickPolling() {
    if (stickPollInterval) return;

    stickPollInterval = setInterval(() => {
      if (!enabled.value || inputLocked.value) return;

      const rightStickY = gamepad.axisValue("RightStickY");
      // Dead zone: ignore small movements
      if (Math.abs(rightStickY) < 0.3) return;

      // Find the scrollable container (try focused element's parent chain)
      let scrollable: HTMLElement | null = null;
      if (currentFocused.value?.el) {
        let parent = currentFocused.value.el.parentElement;
        while (parent) {
          if (
            parent.scrollHeight > parent.clientHeight ||
            parent.scrollWidth > parent.clientWidth
          ) {
            scrollable = parent;
            break;
          }
          parent = parent.parentElement;
        }
      }

      if (scrollable) {
        // M5 fix: acceleration curve — square the deflection for non-linear speed
        const normalised = Math.abs(rightStickY);
        const accelerated = normalised * normalised; // quadratic curve
        const scrollSpeed = 20 + accelerated * 100; // 20..120 px/poll
        const delta = Math.sign(rightStickY) * scrollSpeed;
        scrollable.scrollBy({ top: delta });
      }
    }, 50);
  }

  function stopStickPolling() {
    if (stickPollInterval) {
      clearInterval(stickPollInterval);
      stickPollInterval = null;
    }
  }

  // Start stick polling when focus system is enabled
  watch(enabled, (value) => {
    if (value) {
      startStickPolling();
    } else {
      stopStickPolling();
    }
  });
}