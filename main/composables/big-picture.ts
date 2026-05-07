import { getCurrentWindow } from "@tauri-apps/api/window";
import { GamepadButton, useGamepad } from "./gamepad";
import { useFocusNavigation } from "./focus-navigation";
import { useDeckMode } from "./deck-mode";
import { devLog } from "./dev-mode";

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
    devLog("state", `BPM enter (from "${previousRoute.value}")`);
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
    await router.push("/bigpicture");
  }

  async function exit() {
    if (!isActive.value) return;

    // On Steam Deck in Gaming Mode (Gamescope), there's no windowed
    // desktop to return to — exiting BPM would cause a white screen.
    const { isGamescope } = useDeckMode();
    if (isGamescope.value) {
      devLog("state","[BPM] Exit blocked — running in Gamescope session");
      return;
    }

    devLog("state", `BPM exit (-> "${previousRoute.value}")`);
    isActive.value = false;

    // Navigate FIRST — this triggers the layout switch from bigpicture → default.
    // Doing this before destroying subsystems avoids white-screen races where
    // the bigpicture layout's components lose their dependencies mid-render.
    await router.push(previousRoute.value);

    // Exit fullscreen after navigation so the new layout renders at the
    // correct windowed size.
    try {
      const win = getCurrentWindow();
      await win.setFullscreen(false);
    } catch (e) {
      console.warn("Failed to exit fullscreen:", e);
    }

    // H2 fix: fully tear down BPM subsystems (stops gamepad polling,
    // clears all focus-nav listeners, clears repeat timers, etc.)
    // Done last so bigpicture layout's onUnmounted has clean state.
    focusNav.destroy();

    const gamepad = useGamepad();
    gamepad.destroy();
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
