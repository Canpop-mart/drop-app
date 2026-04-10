/**
 * Steam Deck / handheld detection and responsive Big Picture mode.
 *
 * Detection layers (highest priority first):
 *  1. Rust-side SessionType from AppState (Gamescope env vars, DMI board name)
 *  2. Screen dimensions (1280x800 or 1280x720 internal display)
 *  3. Linux + "SteamDeck" or "Jupiter" in user-agent / OS info
 *  4. Small-screen override for any device ≤ 1280px wide
 *
 * Can also be manually overridden via the `forceOverride` ref:
 *  - "deck"  → always Deck mode
 *  - "desktop" → always desktop mode
 *  - "auto"  → use auto-detection (default)
 *
 * Other components read `isDeckMode` to switch between desktop
 * Big Picture (left nav rail) and Deck layout (bottom tab bar,
 * bigger touch targets, compact chrome).
 */

import { ref, readonly, computed, watch } from "vue";
import type { SessionType } from "~/types";

// ── Singleton state ──────────────────────────────────────────────────────────

const autoDetected = ref(false);
const screenWidth = ref(0);
const screenHeight = ref(0);

/**
 * Session type reported by the Rust backend.
 * Set once during app initialization from AppState.sessionType.
 */
const sessionType = ref<SessionType>("desktop");

/**
 * True when the Rust backend detected a Gamescope session (SteamOS Game Mode).
 * This is the most reliable detection — environment variables set by the compositor.
 */
const isGamescope = computed(() => sessionType.value === "gamescope");

/**
 * True when running on Steam Deck hardware (Game Mode or Desktop Mode).
 */
const isSteamDeckHardware = computed(
  () =>
    sessionType.value === "gamescope" ||
    sessionType.value === "steamDeckDesktop",
);

/**
 * Manual override: "auto" | "deck" | "desktop"
 * M4 fix: persisted in localStorage so it survives restarts.
 */
function loadDeckOverride(): "auto" | "deck" | "desktop" {
  if (typeof localStorage === "undefined") return "auto";
  const stored = localStorage.getItem("drop:deckMode");
  if (stored === "deck" || stored === "desktop") return stored;
  return "auto";
}

const forceOverride = ref<"auto" | "deck" | "desktop">(loadDeckOverride());

// Persist on change
watch(forceOverride, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("drop:deckMode", val);
  }
});

/**
 * Final computed value that respects manual override.
 * In Gamescope sessions, Deck mode is forced on (override is ignored).
 */
const isDeck = computed(() => {
  // Gamescope always means Deck mode — no override can disable it
  if (isGamescope.value) return true;
  if (forceOverride.value === "deck") return true;
  if (forceOverride.value === "desktop") return false;
  return autoDetected.value;
});

let initialized = false;

function detect() {
  if (typeof window === "undefined") return;

  const w = window.innerWidth;
  const h = window.innerHeight;
  screenWidth.value = w;
  screenHeight.value = h;

  // Steam Deck has a 1280x800 display (or 1280x720 in some configs)
  const deckResolution = w <= 1280 && h <= 800;

  // Check user-agent for Steam Deck markers
  const ua = navigator.userAgent.toLowerCase();
  const deckUA = ua.includes("steamdeck") || ua.includes("jupiter");

  // Also detect Linux + small screen as likely Deck/handheld
  const linuxSmall = ua.includes("linux") && deckResolution;

  // Final auto-detection: explicit Deck hardware, or any small-screen device
  // (this also catches other handhelds like ROG Ally, Legion Go, etc.)
  autoDetected.value = deckUA || deckResolution || linuxSmall;
}

function init() {
  if (initialized) return;
  initialized = true;

  detect();

  if (typeof window !== "undefined") {
    window.addEventListener("resize", detect);
  }
}

/**
 * Set the session type from the Rust-side AppState.
 * Called once during app initialization in app.vue.
 */
export function setSessionType(type: SessionType) {
  sessionType.value = type;
}

// ── Composable ───────────────────────────────────────────────────────────────

export function useDeckMode() {
  init();

  return {
    /** True when running in Deck mode (auto-detected or forced) */
    isDeckMode: isDeck,
    /** The auto-detection result (ignoring override) */
    autoDetected: readonly(autoDetected),
    /** True when Gamescope compositor is detected (SteamOS Game Mode) */
    isGamescope: readonly(isGamescope),
    /** True when running on Steam Deck hardware (any mode) */
    isSteamDeckHardware: readonly(isSteamDeckHardware),
    /** The session type from the Rust backend */
    sessionType: readonly(sessionType),
    /** Manual override: "auto" | "deck" | "desktop" */
    forceOverride,
    screenWidth: readonly(screenWidth),
    screenHeight: readonly(screenHeight),
  };
}
