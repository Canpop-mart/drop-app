/**
 * Forwards gamepad events to a `server://` iframe via postMessage.
 *
 * Immediately acquires an input lock on creation so D-pad / A
 * only control the iframe content — not the nav rail. B releases
 * the lock and returns focus to nav.
 *
 * Uses an ownership-based lock (acquireInputLock / releaseInputLock)
 * so that when navigating between two iframe pages the old page's
 * onUnmounted cannot accidentally undo the new page's lock.
 */

import { GamepadButton, useGamepad } from "./gamepad";
import { useFocusNavigation } from "./focus-navigation";

export function useIframeController(iframeRef: Ref<HTMLIFrameElement | null>) {
  const gamepad = useGamepad();
  const focusNav = useFocusNavigation();
  const unsubs: (() => void)[] = [];

  // Track whether this iframe controller owns input
  const active = ref(true);

  // Acquire ownership-based lock IMMEDIATELY (during script setup,
  // before any gamepad callbacks can fire).  The returned ID ensures
  // only THIS controller can release the lock.
  let lockId = focusNav.acquireInputLock();

  // Also clear any existing nav focus highlight
  focusNav.clearFocus();

  function post(data: Record<string, unknown>) {
    if (!active.value) return;
    const iframe = iframeRef.value;
    if (!iframe?.contentWindow) return;
    try {
      iframe.contentWindow.postMessage(data, "*");
    } catch {
      // cross-origin or iframe not ready
    }
  }

  function deactivate() {
    active.value = false;
    focusNav.releaseInputLock(lockId);
  }

  function reactivate() {
    active.value = true;
    // Acquire a fresh lock (new ID, so the old one can't clobber us)
    lockId = focusNav.acquireInputLock();
    focusNav.clearFocus();
  }

  // D-pad → navigate inside iframe
  const directions: Record<string, string> = {
    [GamepadButton.DPadUp]: "up",
    [GamepadButton.DPadDown]: "down",
    [GamepadButton.DPadLeft]: "left",
    [GamepadButton.DPadRight]: "right",
  };

  for (const [button, direction] of Object.entries(directions)) {
    unsubs.push(
      gamepad.onButton(button, () => {
        if (!active.value) return;
        post({ type: "bp-controller", action: "navigate", direction });
      }),
    );
  }

  // A → if inactive (user is on nav), reactivate and take over input.
  //     if active, forward select to iframe.
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!active.value) {
        // User pressed A while on nav pointing at this page —
        // reclaim input for the iframe
        reactivate();
        return;
      }
      post({ type: "bp-controller", action: "select" });
    }),
  );

  // B → deactivate, return focus to nav
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!active.value) return;
      deactivate();
      focusNav.focusGroup("nav");
    }),
  );

  // Right stick Y → scroll
  let scrollInterval: ReturnType<typeof setInterval> | null = null;

  onMounted(() => {
    scrollInterval = setInterval(() => {
      if (!active.value) return;
      const val = gamepad.axisValue("RightStickY");
      if (Math.abs(val) > 0.3) {
        post({
          type: "bp-controller",
          action: "scroll",
          amount: Math.round(val * -12),
        });
      }
    }, 50);
  });

  onUnmounted(() => {
    // deactivate releases the lock only if this controller still owns it.
    // If a newer iframe page already acquired its own lock, this is a no-op.
    deactivate();
    for (const unsub of unsubs) unsub();
    if (scrollInterval) clearInterval(scrollInterval);
  });
}
