/**
 * Dev Mode — toggleable firehose of debug output for "everything".
 *
 * When enabled, instrumentation throughout the app (gamepad, focus-nav,
 * useListen, downloads, Tauri invoke, route changes, lifecycle, audio,
 * theme, window events, etc.) emits `[DEV:<CATEGORY>]` tagged console
 * messages. The existing on-screen debug overlay in `layouts/bigpicture.vue`
 * picks up anything starting with `[DEV:` so the firehose is visible
 * on-device without leaving BPM.
 *
 * Persistence: `drop:devMode` (JSON) — `{ enabled, categories: Record<cat, bool> }`.
 * The `drop:` prefix matches the convention used by drop:startInBPM,
 * drop:deckMode, drop:haptic, drop:hideTitles.
 *
 * Categories are individually toggleable so a user chasing a controller
 * bug can leave only `gamepad` + `focus` on and not be drowned in network
 * chatter.
 *
 * Usage:
 *
 *   import { useDevMode } from "~/composables/dev-mode";
 *   const dev = useDevMode();
 *   dev.log("gamepad", "button pressed", evt);  // no-op unless enabled+cat on
 *
 * Or from a plain module (outside a component):
 *
 *   import { devLog } from "~/composables/dev-mode";
 *   devLog("focus", "applyFocus", target?.id);
 *
 * Design notes:
 *
 * - `enabled` is a Vue ref so UI toggles react. The underlying value is
 *   mirrored into a module-level boolean so `devLog()` callers don't need
 *   to instantiate the composable (and pay the reactive overhead) on
 *   every hot-path log call.
 * - Category set is closed (DEV_CATEGORIES below). Adding a new category
 *   means adding it to the list and the type — no dynamic strings, so
 *   typos surface at compile time.
 */

const STORAGE_KEY = "drop:devMode";

export const DEV_CATEGORIES = [
  "gamepad", // button presses, axis changes, connect/disconnect
  "focus", // applyFocus, cycleGroup, restrict/unrestrict, input lock
  "route", // vue-router navigation
  "api", // server fetches (useServerApi, useServerFetch)
  "invoke", // Tauri command invocations
  "download", // queue/stats updates, completion, progress diffs
  "launch", // game launch/exit, umu env, error dialog triggers
  "audio", // audio profile, playback calls
  "theme", // theme / mode switches
  "state", // bpm mode enter/exit, idle state, input lock
  "event", // Tauri event subscriptions & emissions (useListen)
  "lifecycle", // component mount / unmount in BPM pages
  "window", // window resize/focus/blur
] as const;

export type DevCategory = (typeof DEV_CATEGORIES)[number];

type Persisted = {
  enabled: boolean;
  categories: Partial<Record<DevCategory, boolean>>;
};

function readPersisted(): Persisted {
  if (typeof window === "undefined") {
    return { enabled: false, categories: {} };
  }
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return { enabled: false, categories: {} };
    const parsed = JSON.parse(raw) as Persisted;
    return {
      enabled: !!parsed.enabled,
      categories: parsed.categories ?? {},
    };
  } catch {
    return { enabled: false, categories: {} };
  }
}

function writePersisted(state: Persisted) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // localStorage may be unavailable (private mode, etc.) — dev mode is
    // a diagnostic; silently fall back to session-only state.
  }
}

// ── Module-level mirrors (accessed by devLog hot path) ──────────────────
let _enabled = false;
const _enabledCats = new Set<DevCategory>();

function applyPersistedToModule(state: Persisted) {
  _enabled = state.enabled;
  _enabledCats.clear();
  // Default: if enabled and no per-category overrides, all categories on.
  const anyCat = Object.keys(state.categories).length > 0;
  for (const cat of DEV_CATEGORIES) {
    const override = state.categories[cat];
    const on = anyCat ? !!override : true;
    if (on) _enabledCats.add(cat);
  }
}

// Load once at module init — cheap, and ensures devLog works before any
// component wires up useDevMode().
applyPersistedToModule(readPersisted());

// ── Singleton reactive refs (shared across all useDevMode() callers) ────
const enabledRef = ref(_enabled);
const categoriesRef = ref<Record<DevCategory, boolean>>(
  Object.fromEntries(
    DEV_CATEGORIES.map((c) => [c, _enabledCats.has(c)]),
  ) as Record<DevCategory, boolean>,
);

function persistAndMirror() {
  const cats: Partial<Record<DevCategory, boolean>> = {};
  for (const cat of DEV_CATEGORIES) cats[cat] = categoriesRef.value[cat];
  writePersisted({ enabled: enabledRef.value, categories: cats });
  applyPersistedToModule({
    enabled: enabledRef.value,
    categories: cats,
  });
}

// ── Hot-path loggers (safe to call from any module, no Vue context needed) ──

/**
 * Emit a `[DEV:CAT]` info log if dev mode is enabled AND the category is on.
 * No-op otherwise — safe to sprinkle liberally through hot paths.
 */
export function devLog(cat: DevCategory, ...args: unknown[]): void {
  if (!_enabled) return;
  if (!_enabledCats.has(cat)) return;
  console.log(`[DEV:${cat.toUpperCase()}]`, ...args);
}

export function devWarn(cat: DevCategory, ...args: unknown[]): void {
  if (!_enabled) return;
  if (!_enabledCats.has(cat)) return;
  console.warn(`[DEV:${cat.toUpperCase()}]`, ...args);
}

export function devError(cat: DevCategory, ...args: unknown[]): void {
  if (!_enabled) return;
  if (!_enabledCats.has(cat)) return;
  console.error(`[DEV:${cat.toUpperCase()}]`, ...args);
}

/** Cheap check for call sites that want to skip expensive argument prep. */
export function isDevEnabled(cat?: DevCategory): boolean {
  if (!_enabled) return false;
  if (!cat) return true;
  return _enabledCats.has(cat);
}

/**
 * Traced wrapper around Tauri's `invoke`. Use this at call sites you
 * specifically want traced even if the global monkey-patch in
 * `plugins/dev-invoke.client.ts` couldn't install (Tauri freezes the
 * internals object, so patching is best-effort).
 *
 *   import { devInvoke } from "~/composables/dev-mode";
 *   const games = await devInvoke<Game[]>("fetch_library");
 */
export async function devInvoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  // Lazy-import so non-Tauri contexts (SSR build step) don't try to load it.
  const { invoke } = await import("@tauri-apps/api/core");
  if (!isDevEnabled("invoke")) {
    return invoke<T>(cmd, args);
  }
  const t0 = performance.now();
  devLog("invoke", `call  ${cmd}  ${previewJson(args)}`);
  try {
    const result = await invoke<T>(cmd, args);
    const ms = (performance.now() - t0).toFixed(1);
    devLog("invoke", `ok    ${cmd}  ${ms}ms  ${previewJson(result)}`);
    return result;
  } catch (err) {
    const ms = (performance.now() - t0).toFixed(1);
    devError("invoke", `fail  ${cmd}  ${ms}ms  ${err instanceof Error ? err.message : String(err)}`);
    throw err;
  }
}

function previewJson(v: unknown): string {
  if (v === undefined || v === null) return "";
  try {
    const s = JSON.stringify(v);
    if (!s) return "";
    return s.length > 120 ? s.slice(0, 117) + "..." : s;
  } catch {
    return "[unserialisable]";
  }
}

// ── Composable ──────────────────────────────────────────────────────────

export function useDevMode() {
  function toggle() {
    enabledRef.value = !enabledRef.value;
    persistAndMirror();
    // Announce the state change ourselves — bypass the devLog gate
    // because a user flipping it off still wants to see the "off"
    // confirmation once.
    console.log(
      `[DEV:STATE] Dev mode ${enabledRef.value ? "ENABLED" : "DISABLED"}`,
    );
  }

  function setEnabled(v: boolean) {
    if (enabledRef.value === v) return;
    enabledRef.value = v;
    persistAndMirror();
    console.log(
      `[DEV:STATE] Dev mode ${enabledRef.value ? "ENABLED" : "DISABLED"}`,
    );
  }

  function toggleCategory(cat: DevCategory) {
    categoriesRef.value = {
      ...categoriesRef.value,
      [cat]: !categoriesRef.value[cat],
    };
    persistAndMirror();
  }

  function setAllCategories(v: boolean) {
    const next = { ...categoriesRef.value };
    for (const c of DEV_CATEGORIES) next[c] = v;
    categoriesRef.value = next;
    persistAndMirror();
  }

  return {
    enabled: enabledRef,
    categories: categoriesRef,
    ALL_CATEGORIES: DEV_CATEGORIES,
    toggle,
    setEnabled,
    toggleCategory,
    setAllCategories,
    log: devLog,
    warn: devWarn,
    error: devError,
    isEnabled: isDevEnabled,
  };
}
