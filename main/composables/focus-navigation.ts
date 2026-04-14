import { GamepadButton, useGamepad, type ButtonCallback } from "./gamepad";
import { useBpAudio } from "./bp-audio";

// ── Types ────────────────────────────────────────────────────────────────────

export interface FocusableElement {
  el: HTMLElement;
  group: string;
  onSelect?: () => void;
  onContext?: () => void;
  onFocus?: () => void;
}

interface FocusGroup {
  elements: Set<FocusableElement>;
  lastFocused: FocusableElement | null;
}

// ── Grid navigation (Phase 1a) ──────────────────────────────────────────────
// Groups registered as grids use index-aligned navigation instead of spatial.
// When moving up/down, the column index is preserved ("sticky column").
// Left/right moves within a row and updates the sticky column.

interface GridRow {
  elements: FocusableElement[];
  top: number; // average Y position of this row (for sorting)
}

interface GridContext {
  /** Remembered column index — survives vertical navigation across rows of different lengths. */
  stickyCol: number;
}

/** Set of group names that should use grid navigation. */
const gridGroups = new Set<string>();

/** Per-group grid navigation context (sticky column memory). */
const gridContexts = new Map<string, GridContext>();

/**
 * Compute the visual grid layout from a set of focusable elements.
 * Groups elements into rows based on their vertical position (within a
 * tolerance), then sorts each row left-to-right.
 */
function computeGridLayout(elements: FocusableElement[]): GridRow[] {
  const connected = elements.filter((e) => e.el.isConnected);
  if (connected.length === 0) return [];

  // Measure once
  const measured = connected.map((e) => ({
    el: e,
    rect: e.el.getBoundingClientRect(),
  }));

  // Sort by Y then X
  measured.sort((a, b) => {
    const rowDiff = a.rect.top - b.rect.top;
    if (Math.abs(rowDiff) > 10) return rowDiff;
    return a.rect.left - b.rect.left;
  });

  // Group into rows
  const rows: GridRow[] = [];
  let currentRow: typeof measured = [];
  let currentRowTop = -Infinity;

  for (const item of measured) {
    if (
      currentRow.length === 0 ||
      Math.abs(item.rect.top - currentRowTop) <= 10
    ) {
      currentRow.push(item);
      if (currentRow.length === 1) currentRowTop = item.rect.top;
    } else {
      rows.push({
        elements: currentRow.map((m) => m.el),
        top: currentRowTop,
      });
      currentRow = [item];
      currentRowTop = item.rect.top;
    }
  }
  if (currentRow.length > 0) {
    rows.push({
      elements: currentRow.map((m) => m.el),
      top: currentRowTop,
    });
  }

  return rows;
}

/**
 * Find (rowIndex, colIndex) of `target` within the given grid layout.
 * Returns null if not found.
 */
function findInGrid(
  rows: GridRow[],
  target: FocusableElement,
): { row: number; col: number } | null {
  for (let r = 0; r < rows.length; r++) {
    const col = rows[r].elements.indexOf(target);
    if (col !== -1) return { row: r, col };
  }
  return null;
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
 * When set to a group name, the focus system will ONLY navigate within
 * that group — cross-group fallback is disabled. Used for modal overlays
 * like the sort/filter menu that need to trap focus.
 */
const focusRestriction = ref<string | null>(null);

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

// ── Focus history for back-navigation restoration (Phase 1b) ────────────────
// Stores per-route snapshot so returning to a page can restore focus position.

interface FocusSnapshot {
  group: string;
  /** Index of the focused element within its group (DOM order). */
  index: number;
  /** Scroll position of the nearest scrollable ancestor. */
  scrollTop: number;
}

/** Per-route focus snapshots for back-navigation restoration. */
const focusHistory = new Map<string, FocusSnapshot>();

/**
 * Save the current focus state for a route path (module-level).
 */
function _saveFocusSnapshot(routePath: string) {
  if (!currentFocused.value) return;

  const group = groups.get(currentFocused.value.group);
  if (!group) return;

  const connected = Array.from(group.elements).filter((e) => e.el.isConnected);
  const index = connected.indexOf(currentFocused.value);

  let scrollTop = 0;
  let parent = currentFocused.value.el.parentElement;
  while (parent) {
    if (parent.scrollHeight > parent.clientHeight) {
      scrollTop = parent.scrollTop;
      break;
    }
    parent = parent.parentElement;
  }

  focusHistory.set(routePath, {
    group: currentFocused.value.group,
    index: Math.max(index, 0),
    scrollTop,
  });
}

/**
 * Restore focus from a saved snapshot (module-level).
 */
function _restoreFocusSnapshot(routePath: string): boolean {
  const snapshot = focusHistory.get(routePath);
  if (!snapshot) return false;

  const group = groups.get(snapshot.group);
  if (!group || group.elements.size === 0) return false;

  const connected = Array.from(group.elements).filter((e) => e.el.isConnected);
  if (connected.length === 0) return false;

  const targetIndex = Math.min(snapshot.index, connected.length - 1);
  const target = connected[targetIndex];

  applyFocus(target);

  nextTick(() => {
    let parent = target.el.parentElement;
    while (parent) {
      if (parent.scrollHeight > parent.clientHeight) {
        parent.scrollTop = snapshot.scrollTop;
        break;
      }
      parent = parent.parentElement;
    }
  });

  return true;
}

// ── CSS class applied to focused element ─────────────────────────────────────

const FOCUS_CLASS = "bp-focused";
const RING_FOCUS_CLASS = "bp-ring-focused";

/**
 * When the focused wrapper has .bp-focus-delegate, find and mark the
 * first .bp-focus-ring descendant so the glow hugs the art, not the wrapper.
 */
function applyRingFocus(el: HTMLElement) {
  const ring = el.querySelector(".bp-focus-ring");
  if (ring) ring.classList.add(RING_FOCUS_CLASS);
}
function removeRingFocus(el: HTMLElement) {
  const ring = el.querySelector("." + RING_FOCUS_CLASS);
  if (ring) ring.classList.remove(RING_FOCUS_CLASS);
}

/**
 * Check if an element is fully visible within its scrollable ancestor.
 * Returns true when scrolling is NOT needed.
 */
function isVisibleInScrollParent(el: HTMLElement): boolean {
  const rect = el.getBoundingClientRect();
  let parent = el.parentElement;
  while (parent) {
    if (parent.scrollHeight > parent.clientHeight) {
      const parentRect = parent.getBoundingClientRect();
      // Element is fully visible if its entire rect is within the parent viewport
      const margin = 20; // px of padding from edges
      return (
        rect.top >= parentRect.top + margin &&
        rect.bottom <= parentRect.bottom - margin
      );
    }
    parent = parent.parentElement;
  }
  // No scrollable parent — check viewport
  return rect.top >= 0 && rect.bottom <= window.innerHeight;
}

function applyFocus(element: FocusableElement | null, fromGroupCycle = false) {
  // Remove from previous
  if (currentFocused.value) {
    removeRingFocus(currentFocused.value.el);
    currentFocused.value.el.classList.remove(FOCUS_CLASS);
  }

  currentFocused.value = element;

  if (element) {
    element.el.classList.add(FOCUS_CLASS);
    // If this wrapper delegates glow, mark the inner ring element
    if (element.el.classList.contains("bp-focus-delegate")) {
      applyRingFocus(element.el);
    }
    // Play focus feedback sound
    useBpAudio().play("focus");

    // Only scroll if the element is not already visible — avoids the
    // jarring "scroll fight" reported in issue G.
    if (!isVisibleInScrollParent(element.el) || fromGroupCycle) {
      const scrollBlock = fromGroupCycle ? "center" : "nearest";
      element.el.scrollIntoView({ block: scrollBlock, behavior: "smooth" });
    }

    currentGroup.value = element.group;

    // Update group memory
    const group = groups.get(element.group);
    if (group) {
      group.lastFocused = element;
    }

    // Fire onFocus callback (e.g. for data prefetching)
    element.onFocus?.();
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

  const groupName = currentFocused.value.group;
  const group = groups.get(groupName);
  if (!group) return;

  // ── Grid navigation (Phase 1a) ──────────────────────────────────────────
  if (gridGroups.has(groupName)) {
    const target = navigateGrid(
      currentFocused.value,
      group,
      groupName,
      direction,
    );
    if (target) {
      applyFocus(target);
      const gp = useGamepad();
      gp.vibrate("light");
      return;
    }
    // Grid navigation returned null — fall through to spatial search
    // so focus can escape the grid (e.g. up from top row to filter bar)
  }

  // ── Spatial search (within current group first) ─────────────────────────
  const candidates = Array.from(group.elements)
    .filter((f) => f.el.isConnected)
    .map((f) => f.el);

  let next = findNearest(currentFocused.value.el, candidates, direction);

  // Cross-group fallback: if no candidate found in the current group,
  // search all other groups so focus can naturally flow between sections
  // (e.g. from store tabs down to game tiles, or from content to nav rail).
  // Skip cross-group search when focus is restricted to a specific group (modal overlays).
  if (!next && !focusRestriction.value) {
    const allOtherElements: FocusableElement[] = [];
    for (const [name, g] of groups) {
      if (name === groupName) continue;
      for (const el of g.elements) {
        if (el.el.isConnected) allOtherElements.push(el);
      }
    }
    const otherCandidates = allOtherElements.map((f) => f.el);
    const crossGroupEl = findNearest(
      currentFocused.value.el,
      otherCandidates,
      direction,
    );
    if (crossGroupEl) {
      const focusable = allOtherElements.find((f) => f.el === crossGroupEl);
      if (focusable) {
        applyFocus(focusable);
        const gp = useGamepad();
        gp.vibrate("light");
      }
      return;
    }
  }

  if (next) {
    const focusable = Array.from(group.elements).find((f) => f.el === next);
    if (focusable) {
      applyFocus(focusable);
      const gp = useGamepad();
      gp.vibrate("light");
    }
  }
}

/**
 * Index-aligned grid navigation.
 * - Left/Right: move within the row, update stickyCol.
 * - Up/Down: move to the same stickyCol in the adjacent row.
 *   If the target row is shorter, clamp to the last column.
 *   Does NOT wrap at edges.
 */
function navigateGrid(
  current: FocusableElement,
  group: FocusGroup,
  groupName: string,
  direction: Direction,
): FocusableElement | null {
  const elements = Array.from(group.elements);
  const rows = computeGridLayout(elements);
  if (rows.length === 0) return null;

  const pos = findInGrid(rows, current);
  if (!pos) return null;

  // Ensure grid context exists
  if (!gridContexts.has(groupName)) {
    gridContexts.set(groupName, { stickyCol: pos.col });
  }
  const ctx = gridContexts.get(groupName)!;

  let targetRow = pos.row;
  let targetCol = pos.col;

  switch (direction) {
    case "left":
      if (pos.col > 0) {
        targetCol = pos.col - 1;
        ctx.stickyCol = targetCol;
      } else {
        return null; // no wrap
      }
      break;

    case "right":
      if (pos.col < rows[pos.row].elements.length - 1) {
        targetCol = pos.col + 1;
        ctx.stickyCol = targetCol;
      } else {
        return null; // no wrap
      }
      break;

    case "up":
      if (pos.row > 0) {
        targetRow = pos.row - 1;
        // Use sticky column, clamped to row length
        targetCol = Math.min(ctx.stickyCol, rows[targetRow].elements.length - 1);
      } else {
        return null; // no wrap
      }
      break;

    case "down":
      if (pos.row < rows.length - 1) {
        targetRow = pos.row + 1;
        // Use sticky column, clamped to row length
        targetCol = Math.min(ctx.stickyCol, rows[targetRow].elements.length - 1);
      } else {
        return null; // no wrap
      }
      break;
  }

  return rows[targetRow].elements[targetCol] ?? null;
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

// ── Left stick navigation state (module-level for cleanup in destroy()) ──────

let leftStickDirection: Direction | null = null;
let leftStickRepeatTimer: ReturnType<typeof setTimeout> | null = null;
let leftStickRepeatInterval: ReturnType<typeof setInterval> | null = null;

function stopLeftStickRepeat() {
  if (leftStickRepeatTimer) {
    clearTimeout(leftStickRepeatTimer);
    leftStickRepeatTimer = null;
  }
  if (leftStickRepeatInterval) {
    clearInterval(leftStickRepeatInterval);
    leftStickRepeatInterval = null;
  }
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
      onFocus?: () => void;
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
      onFocus: options?.onFocus,
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

  /**
   * Register a focus group as a grid layout.
   * Navigation within this group will use index-aligned column-sticky
   * movement instead of spatial cone search.
   */
  function registerGrid(group: string) {
    gridGroups.add(group);
  }

  /**
   * Unregister a group from grid navigation (reverts to spatial).
   */
  function unregisterGrid(group: string) {
    gridGroups.delete(group);
    gridContexts.delete(group);
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
    stopLeftStickRepeat();
    leftStickDirection = null;

    // Stop stick polling
    if (stickPollInterval) {
      clearInterval(stickPollInterval);
      stickPollInterval = null;
    }

    // C5 fix: unsubscribe all gamepad bindings
    for (const unsub of gamepadUnsubs) unsub();
    gamepadUnsubs.length = 0;

    // Clear all focus groups, grid registrations, history, and state
    groups.clear();
    gridGroups.clear();
    gridContexts.clear();
    focusHistory.clear();
    currentFocused.value = null;
    currentGroup.value = "";
    enabled.value = false;
    inputLocked.value = false;
    groupOrder.value = [];

    // Allow re-wiring on next BPM enter
    gamepadWired = false;

    console.log("[FOCUS-NAV] Destroyed — all listeners removed");
  }

  // ── Focus history (Phase 1b) — delegate to module-level functions ──────

  const saveFocusSnapshot = _saveFocusSnapshot;
  const restoreFocusSnapshot = _restoreFocusSnapshot;

  /**
   * Auto-focus the first element in `preferredGroup` (default "content").
   * Call this from onMounted in BPM pages so the user always has
   * something focused when they land on the page.
   *
   * Waits one tick so that template refs have been registered.
   */
  function autoFocusContent(preferredGroup = "content") {
    nextTick(() => {
      // Don't override if something is already focused
      if (currentFocused.value) return;

      const group = groups.get(preferredGroup);
      if (group && group.elements.size > 0) {
        const target = group.lastFocused || (group.elements.values().next().value ?? null);
        applyFocus(target);
      }
    });
  }

  /**
   * Restrict focus navigation to a single group (for modal overlays).
   * While restricted, cross-group fallback is disabled — gamepad can
   * only move between elements within the specified group.
   * Also focuses the first element in that group.
   */
  function restrictFocus(groupName: string) {
    focusRestriction.value = groupName;
    nextTick(() => focusGroup(groupName));
  }

  /**
   * Release the focus restriction and optionally refocus a group.
   */
  function unrestrictFocus(refocusGroup?: string) {
    focusRestriction.value = null;
    if (refocusGroup) {
      nextTick(() => focusGroup(refocusGroup));
    }
  }

  return {
    // State
    currentFocused: readonly(currentFocused),
    currentGroup: readonly(currentGroup),
    enabled,
    inputLocked,

    // Methods
    registerElement,
    registerGrid,
    unregisterGrid,
    setGroupOrder,
    focusGroup,
    clearFocus,
    navigate,
    cycleGroup,
    acquireInputLock,
    releaseInputLock,
    restrictFocus,
    unrestrictFocus,
    saveFocusSnapshot,
    restoreFocusSnapshot,
    autoFocusContent,
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

  // A = Select / Confirm (with press feedback — Phase 1d)
  const PRESS_CLASS = "bp-pressed";
  const PRESS_DURATION = 80; // ms

  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!enabled.value || inputLocked.value) return;
      stopRepeat();

      const el = currentFocused.value?.el;
      if (el) {
        // Apply press feedback: brief scale-down animation
        el.classList.add(PRESS_CLASS);
        setTimeout(() => el.classList.remove(PRESS_CLASS), PRESS_DURATION);
      }

      if (currentFocused.value?.onSelect) {
        console.log("[FOCUS] A pressed — calling onSelect for:", el?.tagName, el?.textContent?.slice(0, 30));
        useBpAudio().play("select");
        gamepad.vibrate("medium");
        try {
          currentFocused.value.onSelect();
        } catch (e) {
          console.error("[FOCUS] onSelect THREW:", e);
        }
      } else if (el) {
        console.log("[FOCUS] A pressed — clicking element:", el.tagName, el.textContent?.slice(0, 30));
        useBpAudio().play("select");
        gamepad.vibrate("medium");
        el.click();
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
          useBpAudio().play("back");
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
        // Save focus snapshot before navigating back (Phase 1b)
        _saveFocusSnapshot(path);
        // On a deep page (e.g. /bigpicture/library/xyz) — navigate to parent
        // Profile pages are reached from community, so go back there
        const parentPath =
          segments[0] === "profile"
            ? "/bigpicture/community"
            : "/bigpicture/" + segments[0];
        useBpAudio().play("back");
        router.push(parentPath);
      } else {
        // At the root level with nowhere to go back — play error sound
        useBpAudio().play("error");
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

  // ── LT/RT = Page scroll (Phase 1c) ───────────────────────────────────────
  // Global trigger-based page scroll. Finds the nearest scrollable ancestor
  // (or the layout's [data-bp-scroll] container) and scrolls by one viewport.

  function findScrollContainer(): HTMLElement | null {
    // First try the focused element's scrollable parent
    if (currentFocused.value?.el) {
      let parent = currentFocused.value.el.parentElement;
      while (parent) {
        if (parent.scrollHeight > parent.clientHeight) return parent;
        parent = parent.parentElement;
      }
    }
    // Fallback: find the best [data-bp-scroll] container — prefer one that
    // actually has overflow (inner page scroll) over the layout wrapper.
    const candidates = document.querySelectorAll<HTMLElement>("[data-bp-scroll]");
    for (let i = candidates.length - 1; i >= 0; i--) {
      if (candidates[i].scrollHeight > candidates[i].clientHeight) {
        return candidates[i];
      }
    }
    // If none are scrollable, return the last one (deepest in DOM)
    return candidates.length > 0 ? candidates[candidates.length - 1] : null;
  }

  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.LeftTrigger, () => {
      if (!enabled.value || inputLocked.value) return;
      const container = findScrollContainer();
      if (container) {
        container.scrollBy({
          top: -container.clientHeight,
          behavior: "smooth",
        });
      }
    }),
  );

  gamepadUnsubs.push(
    gamepad.onButton(GamepadButton.RightTrigger, () => {
      if (!enabled.value || inputLocked.value) return;
      const container = findScrollContainer();
      if (container) {
        container.scrollBy({
          top: container.clientHeight,
          behavior: "smooth",
        });
      }
    }),
  );

  // ── Left stick → D-pad navigation ─────────────────────────────────────────
  // The left analog stick should navigate just like the D-pad.
  // We convert stick deflection into discrete navigation events with repeat.
  // State variables (leftStickDirection, timers, stopLeftStickRepeat) are at
  // module scope so destroy() can clean them up.

  const LEFT_STICK_NAV_THRESHOLD = 0.55; // deflection needed to trigger nav
  const LEFT_STICK_REPEAT_DELAY = 300; // ms before repeat starts
  const LEFT_STICK_REPEAT_RATE = 120; // ms between repeats

  function getLeftStickDirection(
    lx: number,
    ly: number,
  ): Direction | null {
    const absX = Math.abs(lx);
    const absY = Math.abs(ly);

    // Must exceed threshold
    if (absX < LEFT_STICK_NAV_THRESHOLD && absY < LEFT_STICK_NAV_THRESHOLD) {
      return null;
    }

    // Pick the dominant axis
    if (absY > absX) {
      return ly < 0 ? "up" : "down";
    } else {
      return lx < 0 ? "left" : "right";
    }
  }

  function handleLeftStickNav(dir: Direction | null) {
    if (dir === leftStickDirection) return; // no change

    stopLeftStickRepeat();
    leftStickDirection = dir;

    if (!dir || !enabled.value || inputLocked.value) return;

    // Immediate navigation on first deflection
    navigate(dir);

    // Then repeat after delay
    leftStickRepeatTimer = setTimeout(() => {
      if (leftStickDirection === dir) {
        navigate(dir);
        leftStickRepeatInterval = setInterval(() => {
          if (leftStickDirection === dir && enabled.value && !inputLocked.value) {
            navigate(dir);
          } else {
            stopLeftStickRepeat();
          }
        }, LEFT_STICK_REPEAT_RATE);
      }
    }, LEFT_STICK_REPEAT_DELAY);
  }

  // Right stick Y axis scrolling for native pages
  // M5 fix: added acceleration curve — stronger stick deflection = faster scroll
  function startStickPolling() {
    if (stickPollInterval) return;

    stickPollInterval = setInterval(() => {
      // Left stick → D-pad navigation
      const lx = gamepad.axisValue("LeftStickX");
      const ly = gamepad.axisValue("LeftStickY");
      handleLeftStickNav(getLeftStickDirection(lx, ly));

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