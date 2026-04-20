<template>
  <NuxtLoadingIndicator color="#2563eb" />
  <NuxtLayout class="select-none w-screen h-screen">
    <NuxtPage />
    <ModalStack />
    <AchievementToast />
  </NuxtLayout>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useAppState } from "./composables/app-state.js";
import { useDownloadListeners } from "./composables/downloads.js";
import {
  initialNavigation,
  setupHooks,
} from "./composables/state-navigation.js";
import { listen } from "@tauri-apps/api/event";
import { AppStatus, type AppState } from "./types.js";
import { setSessionType } from "./composables/deck-mode.js";
import { useBigPictureMode } from "./composables/big-picture.js";

const router = useRouter();

const state = useAppState();

useDownloadListeners();

// Wire Xbox Guide button to toggle Big Picture Mode globally
import { useGuideButtonToggle } from "./composables/big-picture.js";
useGuideButtonToggle();

async function fetchState() {
  try {
    state.value = JSON.parse(await invoke("fetch_state"));
    if (!state.value)
      throw createError({
        statusCode: 500,
        statusMessage: `App state is: ${state.value}`,
        fatal: true,
      });
  } catch (e) {
    console.error("failed to parse state", e);
    throw e;
  }
}
await fetchState();

// Propagate Rust-side session detection to the frontend deck-mode composable
if (state.value?.sessionType) {
  setSessionType(state.value.sessionType);
}

const unlistenState = listen("update_state", (event) => {
  state.value = event.payload as AppState;
});
onUnmounted(async () => {
  (await unlistenState)();
});

setupHooks();
initialNavigation(state);

// ── Auto-enter Big Picture Mode ─────────────────────────────────────────────
// Activate BPM automatically if:
//   1. Gamescope session detected (SteamOS Game Mode) — always enter BPM
//   2. User toggled "Start in Big Picture Mode" in BPM settings
// Only activate after successful auth (not on setup/signout screens).
const bigPicture = useBigPictureMode();

function shouldAutoBPM(): boolean {
  // Gamescope session = always enter BPM
  if (state.value?.sessionType === "gamescope") return true;
  // User preference from BPM settings
  if (
    typeof localStorage !== "undefined" &&
    localStorage.getItem("drop:startInBPM") === "true"
  )
    return true;
  return false;
}

function isAuthenticatedStatus(s: AppState | null | undefined): boolean {
  if (!s) return false;
  return (
    s.status !== AppStatus.NotConfigured &&
    s.status !== AppStatus.SignedOut &&
    s.status !== AppStatus.SignedInNeedsReauth &&
    s.status !== AppStatus.ServerUnavailable
  );
}

/** Try to enter BPM if conditions are met and we're not already in it. */
function tryEnterBPM() {
  if (bigPicture.isActive.value) return;
  if (shouldAutoBPM() && isAuthenticatedStatus(state.value)) {
    nextTick(() => {
      bigPicture.enter();
    });
  }
}

// Try on initial load
tryEnterBPM();

// Also re-check after auth completes or state updates (e.g., user was on
// setup/signout screen and just authenticated — the initial check would
// have skipped BPM because isAuthenticated was false at that point).
watch(
  () => state.value?.status,
  () => {
    tryEnterBPM();
  },
);

// ── Suspend/Resume handling ──────────────────────────────────────────────
// On Steam Deck (or any device), the OS may suspend the app. When it wakes,
// re-check connectivity and refresh state to resume downloads/events.
if (typeof document !== "undefined") {
  const handleVisibilityChange = async () => {
    if (document.visibilityState === "visible") {
      try {
        // Re-fetch app state to reconnect event listeners on the Rust side
        state.value = JSON.parse(await invoke("fetch_state"));
      } catch (e) {
        console.warn("Failed to refresh state after wake:", e);
      }
    }
  };
  document.addEventListener("visibilitychange", handleVisibilityChange);
  onUnmounted(() => {
    document.removeEventListener("visibilitychange", handleVisibilityChange);
  });
}

// ── Router error detection for BPM debugging ───────────────────────────
// Catch navigation failures that happen before any page component mounts
router.onError((error, to, from) => {
  console.error(`[BPM:ROUTER] Navigation error from ${from?.fullPath} to ${to?.fullPath}:`, error);
  // If in BPM, try to recover by navigating to library
  if (to?.fullPath?.startsWith("/bigpicture") || from?.fullPath?.startsWith("/bigpicture")) {
    console.error("[BPM:ROUTER] Attempting recovery — redirecting to /bigpicture");
    router.push("/bigpicture").catch(() => {});
  }
});

router.afterEach((to, from) => {
  if (to.fullPath.startsWith("/bigpicture") || from.fullPath.startsWith("/bigpicture")) {
    console.log(`[BPM:ROUTER] Navigation complete: ${from.fullPath} → ${to.fullPath}`);
  }
});

useHead({
  title: "Drop",
});
</script>
