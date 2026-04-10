import { getCurrentWindow } from "@tauri-apps/api/window";
import { GamepadButton, useGamepad } from "./gamepad";
import { useFocusNavigation } from "./focus-navigation";

// ── Singleton state ──────────────────────────────────────────────────────────

const isActive = ref(false);
const previousRoute = ref("/library");

// ── Composable ───────────────────────────────────────────────────────────────

export function useBigPictureMode() {
  const router = useRouter();
  const focusNav = useFocusNavigation();

  async function enter() {
    if (isActive.value) return;

    previousRoute.value = router.currentRoute.value.fullPath;
    isActive.value = true;
    focusNav.enabled.value = true;

    // Go fullscreen
    try {
      const win = getCurrentWindow();
      await win.setFullscreen(true);
    } catch (e) {
      console.warn("Failed to set fullscreen:", e);
    }

    // Navigate to Big Picture library (main landing page)
    await router.push("/bigpicture/library");
  }

  async function exit() {
    if (!isActive.value) return;

    isActive.value = false;

    // H2 fix: fully tear down BPM subsystems (stops gamepad polling,
    // clears all focus-nav listeners, clears repeat timers, etc.)
    focusNav.destroy();

    const gamepad = useGamepad();
    gamepad.destroy();

    // Exit fullscreen
    try {
      const win = getCurrentWindow();
      await win.setFullscreen(false);
    } catch (e) {
      console.warn("Failed to exit fullscreen:", e);
    }

    // Return to previous route
    await router.push(previousRoute.value);
  }

  async function toggle() {
    if (isActive.value) {
      await exit();
    } else {
      await enter();
    }
  }

  return {
    isActive: readonly(isActive),
    enter,
    exit,
    toggle,
  };
}

// ── Global Guide button binding ──────────────────────────────────────────────

let guideWired = false;

export function useGuideButtonToggle() {
  if (guideWired) return;
  guideWired = true;

  const gamepad = useGamepad();
  const bigPicture = useBigPictureMode();

  gamepad.onButton(GamepadButton.Guide, () => {
    bigPicture.toggle();
  });
}

/**
 * Reset the guide wiring flag so it can be re-initialized after a destroy/re-enter cycle.
 * Called internally when the gamepad subsystem is destroyed on BPM exit.
 */
export function _resetGuideWired() {
  guideWired = false;
}
