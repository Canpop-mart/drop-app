/**
 * Big Picture reduced-motion preference.
 *
 * Persisted in `localStorage` under `bpm:reducedMotion`. Defaults to `true`
 * on detected Steam Deck hardware (the iGPU chokes on full-screen backdrop
 * blur) and `false` everywhere else, unless the user has explicitly toggled
 * it.
 *
 * The BPM settings page writes this key and dispatches a `bpm:reducedMotion`
 * window event; we listen for that so the change applies without a reload.
 */

import { ref, readonly } from "vue";
import { useDeckMode } from "~/composables/deck-mode";

const STORAGE_KEY = "bpm:reducedMotion";

function readInitial(): boolean {
  if (typeof localStorage === "undefined") return false;
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "true") return true;
  if (stored === "false") return false;
  // No explicit choice yet — fall back to hardware detection. We must read
  // this lazily because useDeckMode() needs to be init'd first.
  try {
    return useDeckMode().isSteamDeckHardware.value;
  } catch {
    return false;
  }
}

const reducedMotion = ref(readInitial());
let wired = false;

function wireOnce() {
  if (wired || typeof window === "undefined") return;
  wired = true;

  window.addEventListener("storage", (e) => {
    if (e.key === STORAGE_KEY) {
      reducedMotion.value = e.newValue === "true";
    }
  });

  // Same-tab updates from the settings page go through a custom event —
  // the `storage` event only fires for other tabs.
  window.addEventListener("bpm:reducedMotion", (e: Event) => {
    const val = (e as CustomEvent<boolean>).detail;
    if (typeof val === "boolean") {
      reducedMotion.value = val;
    }
  });
}

export function useReducedMotion() {
  wireOnce();
  return {
    reducedMotion: readonly(reducedMotion),
  };
}
