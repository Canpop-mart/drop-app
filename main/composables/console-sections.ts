/**
 * "Organize emulated games by console" — an app-level toggle.
 *
 * When on, the library home pulls emulated games out of the main grid and
 * groups them into per-console rows (each opening a console-themed page). When
 * off (the default), emulated games stay in the normal grid and sort/filter
 * like any other game — nothing changes for people who don't emulate.
 *
 * Persisted in localStorage under the shared `drop:` prefix, mirroring the
 * ui-zoom / dev-mode composables.
 */
const STORAGE_KEY = "drop:consoleSections";

function readInitial(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

// Module-level singleton ref so every caller shares the same reactive state.
const enabled = ref(readInitial());
let initialized = false;

export function useConsoleSections() {
  if (!initialized && typeof window !== "undefined") {
    initialized = true;
    watch(enabled, (v) => {
      try {
        window.localStorage.setItem(STORAGE_KEY, v ? "true" : "false");
      } catch {
        // ignore — a blocked localStorage just means the toggle won't persist
      }
    });
  }

  return {
    enabled,
    toggle() {
      enabled.value = !enabled.value;
    },
    setEnabled(v: boolean) {
      enabled.value = v;
    },
  };
}
