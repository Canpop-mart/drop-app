// Persistent webview zoom level.
//
// Gamescope + WebKitGTK can render the Drop UI smaller than designed on first
// boot — the viewport meta tag is not always honored by WebKitGTK when the
// compositor reports a non-1x scale factor. A manual zoom control lets the
// user compensate without leaving the app.

import { ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";

const STORAGE_KEY = "drop:uiZoom";
const MIN_ZOOM = 0.7;
const MAX_ZOOM = 1.5;

function detectDefault(): number {
  // Under gamescope, WebKitGTK occasionally renders at the compositor's
  // pre-scale resolution, making the layout look shrunken. Start users at
  // 110% so the first paint is roughly the intended density.
  if (typeof navigator !== "undefined") {
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes("gamescope") || ua.includes("steamdeck")) return 1.1;
  }
  return 1.0;
}

function readInitial(): number {
  if (typeof localStorage === "undefined") return detectDefault();
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return detectDefault();
  const parsed = Number.parseFloat(raw);
  if (!Number.isFinite(parsed)) return detectDefault();
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, parsed));
}

const zoom = ref(readInitial());
let applied = false;

async function apply(value: number): Promise<void> {
  try {
    await getCurrentWebview().setZoom(value);
  } catch (e) {
    console.warn("[UI:ZOOM] failed to set zoom:", e);
  }
}

function init() {
  if (applied) return;
  applied = true;
  apply(zoom.value);
  watch(zoom, (val) => {
    const clamped = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, val));
    if (clamped !== val) {
      zoom.value = clamped;
      return;
    }
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(STORAGE_KEY, String(clamped));
    }
    apply(clamped);
  });
}

export function useUiZoom() {
  init();
  return {
    zoom,
    minZoom: MIN_ZOOM,
    maxZoom: MAX_ZOOM,
    reset() {
      zoom.value = 1.0;
    },
  };
}
