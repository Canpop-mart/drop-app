<template>
  <div :class="['flex h-screen w-screen overflow-hidden', themeClass, modeClass, { 'bpm-reduced-motion': reducedMotion }]" :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }">
    <!-- Navigation Rail (left edge) -->
    <BigPictureNavRail />

    <!-- Main content area -->
    <div class="flex-1 flex flex-col min-w-0">
      <BigPictureTopBar />
      <div class="flex-1 overflow-y-auto bp-scroll-hint" data-bp-scroll>
        <!-- No <Transition> wrapper — it caused white screens on WebKitGTK
             (SteamOS / Steam Deck) because position:absolute on the leaving
             page would cover the entering page if the transition didn't
             complete cleanly. Nuxt handles its own page transitions. -->
        <slot />
      </div>
      <BigPictureContextBar />
    </div>

    <!-- Launch error dialog — listens globally for launch_external_error_detail -->
    <LaunchErrorDialog />

    <!-- Download completion toast — watches useCompletedDownloads -->
    <BpmDownloadToast />

    <!-- Debug overlay (toggle with Select button) -->
    <div
      v-if="debugVisible"
      class="fixed bottom-14 right-4 w-[36rem] bg-black/90 border border-zinc-700 rounded-lg z-[999] font-mono text-[11px] leading-tight flex flex-col"
      style="max-height: 32rem"
    >
      <!-- Header: title + count + close -->
      <div class="flex items-center justify-between px-3 pt-2 pb-1 shrink-0">
        <div class="flex items-center gap-2">
          <span class="text-green-400 font-bold">BPM Debug Console</span>
          <span class="text-zinc-500 text-[10px]">
            {{ filteredCount }} / {{ rawBuffer.length }}{{ rawBuffer.length === MAX_DEBUG ? "+" : "" }}
            <span v-if="debugPaused" class="text-yellow-400 ml-1">· PAUSED</span>
            <span v-else-if="!autoTail" class="text-amber-400 ml-1">· scrolled</span>
          </span>
        </div>
        <button class="text-zinc-500 hover:text-zinc-300" @click="debugVisible = false">✕</button>
      </div>

      <!-- Control row: pause / clear / filter / tail / export -->
      <div class="flex items-center gap-1 px-3 pb-1 shrink-0">
        <button
          class="px-2 py-0.5 text-[10px] rounded transition-colors"
          :class="debugPaused ? 'bg-yellow-600 hover:bg-yellow-500 text-white' : 'bg-zinc-700 hover:bg-zinc-600 text-zinc-300'"
          @click="debugPaused = !debugPaused"
        >
          {{ debugPaused ? "Resume" : "Pause" }}
        </button>
        <button
          class="px-2 py-0.5 text-[10px] bg-zinc-700 hover:bg-zinc-600 text-zinc-300 rounded"
          @click="clearDebug"
        >
          Clear
        </button>
        <button
          class="px-2 py-0.5 text-[10px] rounded transition-colors"
          :class="autoTail ? 'bg-emerald-600 hover:bg-emerald-500 text-white' : 'bg-zinc-700 hover:bg-zinc-600 text-zinc-300'"
          @click="toggleAutoTail"
        >
          Tail
        </button>
        <input
          v-model="debugFilter"
          type="text"
          placeholder="filter (substring or DEV:CAT)"
          class="flex-1 min-w-0 px-2 py-0.5 text-[10px] bg-zinc-900 border border-zinc-700 text-zinc-200 rounded focus:outline-none focus:border-zinc-500"
        />
        <button
          class="px-2 py-0.5 text-[10px] bg-zinc-700 hover:bg-zinc-600 text-zinc-300 rounded"
          @click="exportDebugLog"
        >
          Export
        </button>
      </div>

      <div v-if="exportMessage" class="px-3 text-green-400 text-[10px] shrink-0">{{ exportMessage }}</div>

      <!-- Scrollable message list: capped for DOM perf; rawBuffer keeps the rest. -->
      <div
        ref="debugScrollEl"
        class="flex-1 overflow-y-auto px-3 pb-2"
        @scroll="onDebugScroll"
      >
        <div v-if="visibleMessages.length === 0" class="text-zinc-600 italic text-[10px] py-1">
          {{ debugFilter ? "No matches for filter." : "No messages yet — enable dev mode in Settings → Developer." }}
        </div>
        <div
          v-if="truncatedByRenderCap"
          class="text-zinc-500 italic text-[10px] pb-1 border-b border-zinc-800 mb-1"
        >
          Showing last {{ RENDER_CAP }} of {{ filteredCount }} matches — use filter or Export to see the rest.
        </div>
        <div v-for="(msg, i) in visibleMessages" :key="msg.id" :class="msg.color">
          {{ msg.text }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BigPictureNavRail from "~/components/bigpicture/BigPictureNavRail.vue";
import BigPictureTopBar from "~/components/bigpicture/BigPictureTopBar.vue";
import BigPictureContextBar from "~/components/bigpicture/BigPictureContextBar.vue";
import LaunchErrorDialog from "~/components/bigpicture/LaunchErrorDialog.vue";
import BpmDownloadToast from "~/components/bigpicture/BpmDownloadToast.vue";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useDeckMode } from "~/composables/deck-mode";
import { useBpmTheme } from "~/composables/bp-theme";
import { useUiZoom } from "~/composables/ui-zoom";
import { useReducedMotion } from "~/composables/bp-reduced-motion";

const focusNav = useFocusNavigation();
const bpmTheme = useBpmTheme();
const themeClass = computed(() => bpmTheme.themeData.value.cssClass);
const modeClass = computed(() => `bpm-${bpmTheme.mode.value}`);
const { isGamescope } = useDeckMode();
// When true (user preference or Deck default), we strip out backdrop-blur
// and cut animation complexity globally. See .bpm-reduced-motion rules below.
const { reducedMotion } = useReducedMotion();
// Apply persisted UI zoom level. Gamescope + WebKitGTK sometimes renders the
// webview smaller than expected on first paint; this lets the user rescale.
useUiZoom();

// ── On-screen debug overlay ─────────────────────────────────────────────
// Hidden by default — toggle with Select button on gamepad.
//
// Two-tier buffering:
//   rawBuffer — up to MAX_DEBUG entries kept in memory (huge ring, ~5000).
//               Export dumps this in full, filter searches this.
//   visibleMessages — last RENDER_CAP entries of the filtered view that
//               actually render in the DOM. Capped to keep Chromium happy
//               on the Deck's iGPU when dev mode firehoses thousands of
//               messages per minute.
//
// Pause snapshots rawBuffer at the moment Pause is pressed so the user
// can study the output without it scrolling out from under them.
// Auto-tail scrolls to the newest line; disabled automatically if the
// user scrolls up to read history, re-enabled with the Tail button.
const debugVisible = ref(false);
const debugPaused = ref(false);
const debugFilter = ref("");
const autoTail = ref(true);

type DebugMsg = { id: number; text: string; color: string };
const rawBuffer = ref<DebugMsg[]>([]);
// When paused, we render this snapshot instead of the live buffer.
const pausedSnapshot = ref<DebugMsg[] | null>(null);
const MAX_DEBUG = 5000;
const RENDER_CAP = 500;
let _msgIdCounter = 0;

function debugLog(msg: string, level: "info" | "warn" | "error" = "info") {
  const colors = { info: "text-zinc-300", warn: "text-yellow-400", error: "text-red-400" };
  const ts = new Date().toLocaleTimeString("en", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" });
  rawBuffer.value.push({
    id: ++_msgIdCounter,
    text: `[${ts}] ${msg}`,
    color: colors[level],
  });
  if (rawBuffer.value.length > MAX_DEBUG) rawBuffer.value.shift();
}

/**
 * The source list the UI actually filters / displays.
 * When paused, we freeze on the snapshot so scroll position stays stable.
 */
const sourceList = computed<DebugMsg[]>(() =>
  debugPaused.value && pausedSnapshot.value ? pausedSnapshot.value : rawBuffer.value,
);

/** Apply the filter string as a case-insensitive substring match. */
const filtered = computed<DebugMsg[]>(() => {
  const q = debugFilter.value.trim().toLowerCase();
  if (!q) return sourceList.value;
  return sourceList.value.filter((m) => m.text.toLowerCase().includes(q));
});

const filteredCount = computed(() => filtered.value.length);
/** The tail we actually render — capped for DOM perf. */
const visibleMessages = computed<DebugMsg[]>(() =>
  filtered.value.length > RENDER_CAP ? filtered.value.slice(-RENDER_CAP) : filtered.value,
);
const truncatedByRenderCap = computed(() => filtered.value.length > RENDER_CAP);

watch(debugPaused, (paused) => {
  // Take / clear the snapshot when toggling pause so resuming shows live data.
  pausedSnapshot.value = paused ? [...rawBuffer.value] : null;
});

/** Clear the buffer (and the pause snapshot if one is active). */
function clearDebug() {
  rawBuffer.value = [];
  pausedSnapshot.value = null;
  exportMessage.value = "Cleared.";
  setTimeout(() => { exportMessage.value = ""; }, 1500);
}

// Auto-scroll / tail handling ────────────────────────────────────────────
const debugScrollEl = ref<HTMLElement | null>(null);

function scrollToBottom() {
  const el = debugScrollEl.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

function toggleAutoTail() {
  autoTail.value = !autoTail.value;
  if (autoTail.value) nextTick(scrollToBottom);
}

/**
 * If the user scrolls up to read history, auto-disable tail mode so we
 * don't keep yanking them back to the bottom on every new message. If
 * they scroll back to the bottom on their own, re-enable it.
 */
function onDebugScroll() {
  const el = debugScrollEl.value;
  if (!el) return;
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 16;
  if (autoTail.value && !atBottom) autoTail.value = false;
  else if (!autoTail.value && atBottom) autoTail.value = true;
}

// Tail: whenever visibleMessages changes and we're in tail mode, scroll down.
watch(visibleMessages, () => {
  if (autoTail.value && !debugPaused.value) nextTick(scrollToBottom);
});

watch(debugVisible, (v) => {
  if (v && autoTail.value) nextTick(scrollToBottom);
});

const exportMessage = ref("");

/**
 * Export debug log — writes to a file in the app's data dir AND copies
 * to clipboard so the user can paste it after exiting Gaming Mode.
 * Always exports the FULL rawBuffer (not the filtered view) so the
 * dumped log isn't accidentally narrowed by a stale filter.
 */
async function exportDebugLog() {
  const logText = rawBuffer.value.map((m) => m.text).join("\n");

  // Try clipboard first
  try {
    await navigator.clipboard.writeText(logText);
    exportMessage.value = `Copied ${rawBuffer.value.length} lines to clipboard!`;
  } catch {
    exportMessage.value = "Clipboard unavailable — downloading file...";
  }

  // Also offer a download as a file
  try {
    const blob = new Blob([logText], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `bpm-debug-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.log`;
    a.click();
    URL.revokeObjectURL(url);
    if (!exportMessage.value.includes("Copied")) {
      exportMessage.value = `Downloaded ${rawBuffer.value.length} lines.`;
    }
  } catch (e) {
    console.error("[BPM:LAYOUT] Failed to export log:", e);
  }

  setTimeout(() => { exportMessage.value = ""; }, 3000);
}

// Expose globally so other components can use it
if (typeof window !== "undefined") {
  (window as any).__bpmDebug = debugLog;
}

// Intercept console.log/warn/error for [BPM:*] messages
const _origLog = console.log;
const _origWarn = console.warn;
const _origError = console.error;

/**
 * Single regex covering every tag the overlay wants to capture.
 * Used as a CHEAP pre-filter before doing any stringification — on a hot
 * page (lots of 3rd-party console output) avoiding a JSON.stringify per
 * non-string arg per log is a real perf win on the Deck.
 */
const DEBUG_TAG_PATTERN = /\[(BPM:|useGame|FOCUS\]|GAMEPAD\]|ERROR|DEV:)/;

function safeStringify(v: unknown): string {
  // JSON.stringify throws on cyclic structures; guard so an intercepted
  // log never takes down the interceptor and leaves the overlay blind.
  try {
    const s = JSON.stringify(v);
    return s ? s.slice(0, 200) : String(v);
  } catch {
    return "[unserialisable]";
  }
}

function interceptConsole(origFn: Function, level: "info" | "warn" | "error") {
  return function (...args: any[]) {
    origFn.apply(console, args);

    // Fast path: tags are always strings, so if no string arg contains
    // one of the sentinels, bail before doing any stringification.
    let matched = false;
    for (const a of args) {
      if (typeof a === "string" && DEBUG_TAG_PATTERN.test(a)) {
        matched = true;
        break;
      }
    }
    if (!matched) return;

    const text = args
      .map((a) => (typeof a === "string" ? a : safeStringify(a)))
      .join(" ");
    debugLog(text.slice(0, 300), level);
    // Auto-show overlay on errors
    if (level === "error") debugVisible.value = true;
  };
}

// Catch Vue rendering errors in BPM so they get logged instead of
// triggering Nuxt's error page (which breaks out of BPM layout)
onErrorCaptured((err, instance, info) => {
  const name = instance?.$options?.name ?? instance?.$options?.__name ?? "unknown";
  const msg = `Vue error in <${name}>: ${err instanceof Error ? err.message : String(err)} (${info})`;
  console.error("[BPM:LAYOUT]", msg);
  debugLog(msg, "error");
  return false; // prevent propagation to Nuxt error handler
});

// Toggle debug overlay with Select+Start
const gamepad = useGamepad();
const _unsubs: (() => void)[] = [];

// Install console interceptors IMMEDIATELY during setup (before child pages mount)
// so that synchronous logs from page <script setup> are captured too.
if (typeof window !== "undefined") {
  console.log = interceptConsole(_origLog, "info") as any;
  console.warn = interceptConsole(_origWarn, "warn") as any;
  console.error = interceptConsole(_origError, "error") as any;

  // Catch uncaught JS errors and unhandled promise rejections
  window.addEventListener("error", (e) => {
    debugLog(`UNCAUGHT: ${e.message} at ${e.filename}:${e.lineno}`, "error");
    debugVisible.value = true;
  });
  window.addEventListener("unhandledrejection", (e) => {
    debugLog(`UNHANDLED PROMISE: ${e.reason}`, "error");
    debugVisible.value = true;
  });
}

// Log session and UMU info for debugging launch issues
{
  const _st = useState<any>("state");
  const _umuState = _st.value?.umuState ?? "unknown";
  const _sessionType = _st.value?.sessionType ?? "unknown";
  debugLog(`BPM layout setup | session: ${_sessionType} | gamescope: ${isGamescope.value} | UMU: ${_umuState}`, "info");
}

onMounted(() => {
  focusNav.enabled.value = true;

  debugLog("BPM layout mounted", "info");

  // Select button toggles debug overlay
  _unsubs.push(
    gamepad.onButton(GamepadButton.Select, () => {
      debugVisible.value = !debugVisible.value;
    }),
  );
});

// Watch route changes
const route = useRoute();
watch(
  () => route.fullPath,
  (to, from) => {
    debugLog(`Navigate: ${from} → ${to}`);
  },
);

onUnmounted(() => {
  // Deliberately DO NOT set `focusNav.enabled.value = false` here.
  // Layout switches (e.g. /bigpicture/* → /welcome/* for the tutorial) mount
  // the new layout BEFORE unmounting this one, so disabling on unmount races
  // the new layout's `= true` and leaves the D-pad inert.
  // focus-nav is disabled centrally by `bigPicture.exit() → focusNav.destroy()`
  // which fires only on a real BPM exit.
  for (const unsub of _unsubs) unsub();
  // Restore original console methods
  console.log = _origLog;
  console.warn = _origWarn;
  console.error = _origError;
});
</script>

<!-- Global BPM focus & press styles (unscoped so child components inherit) -->
<style>
/* Hint the compositor to promote the BPM scroll container to its own layer.
   Noticeably cheaper than `transform: translateZ(0)` on weak iGPUs (Deck)
   because the browser picks a less expensive backing store. */
.bp-scroll-hint {
  will-change: scroll-position;
}

/* Reduced-motion mode: strip every backdrop-filter in the BPM subtree.
   `backdrop-filter: blur()` is the single most expensive effect on the
   Deck's iGPU — far more than per-frame animations — because it forces
   a copy of the surface behind every blurred element. Killing it
   globally here avoids editing ~15 components one-by-one. */
.bpm-reduced-motion [class*="backdrop-blur"],
.bpm-reduced-motion [class*="backdrop-filter"] {
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}

/* Focus indicator ring — applied by focus-navigation.ts */
[data-focusable].bp-focused {
  outline: 3px solid rgba(var(--bpm-accent, 59 130 246) / 0.8);
  outline-offset: 2px;
  border-radius: 0.75rem;
  transition:
    outline-color 0.12s ease,
    transform 0.08s ease;
}

/* When the wrapper has .bp-focus-delegate, the focus system hides the
   parent outline and shows it on the .bp-focus-ring child instead. */
.bp-focus-delegate[data-focusable].bp-focused {
  outline: none !important;
}
.bp-ring-focused {
  box-shadow: 0 0 0 3px rgba(var(--bpm-accent, 59 130 246) / 0.8);
  border-radius: 0.75rem;
}

/* Press feedback — brief scale-down on A button */
[data-focusable].bp-pressed {
  transform: scale(0.97);
  transition: transform 0.08s ease;
}

/* ══════════════════════════════════════════════════════════════════════
   LIGHT MODE OVERRIDES
   When .bpm-light is active, remap common Tailwind dark-theme classes
   to use the theme's light palette via CSS variables. This avoids
   editing every single component that uses bg-zinc-* or text-zinc-*.
   ══════════════════════════════════════════════════════════════════════ */
.bpm-light .bg-zinc-950 { background-color: var(--bpm-bg) !important; }
.bpm-light .bg-zinc-950\/50 { background-color: var(--bpm-surface) !important; }
.bpm-light .bg-zinc-950\/80 { background-color: var(--bpm-surface) !important; }
.bpm-light .bg-zinc-950\/90 { background-color: var(--bpm-surface) !important; }
.bpm-light .bg-zinc-900 { background-color: var(--bpm-surface) !important; }
.bpm-light .bg-zinc-900\/50 { background-color: var(--bpm-surface) !important; }
.bpm-light .bg-zinc-800 { background-color: var(--bpm-surface-hover) !important; }
.bpm-light .bg-zinc-800\/50 { background-color: var(--bpm-surface) !important; }
.bpm-light .bg-zinc-800\/80 { background-color: var(--bpm-surface) !important; }
.bpm-light .bg-zinc-800\/30 { background-color: transparent !important; }
.bpm-light .bg-zinc-700 { background-color: var(--bpm-surface-hover) !important; }

/* Text overrides */
.bpm-light .text-zinc-100 { color: var(--bpm-text) !important; }
.bpm-light .text-zinc-200 { color: var(--bpm-text) !important; }
.bpm-light .text-zinc-300 { color: var(--bpm-muted) !important; }
.bpm-light .text-zinc-400 { color: var(--bpm-muted) !important; }
.bpm-light .text-zinc-500 { color: var(--bpm-muted) !important; }
.bpm-light .text-zinc-600 { color: var(--bpm-muted) !important; }
/* NOTE: .text-white is NOT overridden — it's used on colored action buttons
   (play, install) where white text must stay white regardless of theme. */

/* Border overrides */
.bpm-light .border-zinc-800\/30 { border-color: var(--bpm-border) !important; }
.bpm-light .border-zinc-800\/50 { border-color: var(--bpm-border) !important; }
.bpm-light .border-zinc-800 { border-color: var(--bpm-border) !important; }
.bpm-light .border-zinc-700 { border-color: var(--bpm-border) !important; }

/* Ring overrides for inputs/buttons */
.bpm-light .ring-zinc-800 { --tw-ring-color: var(--bpm-border) !important; }

/* ══════════════════════════════════════════════════════════════════════
   DARK MODE OVERRIDES
   Ensure text is bright/white in dark mode across all pages.
   The default Tailwind greys (zinc-400, zinc-500) are too dim on
   themed dark backgrounds. Remap them to the theme's text/muted vars.
   ══════════════════════════════════════════════════════════════════════ */
.bpm-dark .text-zinc-100 { color: var(--bpm-text) !important; }
.bpm-dark .text-zinc-200 { color: var(--bpm-text) !important; }
.bpm-dark .text-zinc-300 { color: var(--bpm-text) !important; }
.bpm-dark .text-zinc-400 { color: var(--bpm-muted) !important; }
.bpm-dark .text-zinc-500 { color: var(--bpm-muted) !important; }
.bpm-dark .text-zinc-600 { color: var(--bpm-muted) !important; }

/* Dark mode backgrounds use theme vars */
.bpm-dark .bg-zinc-950 { background-color: var(--bpm-bg) !important; }
.bpm-dark .bg-zinc-950\/50 { background-color: var(--bpm-bg) !important; }
.bpm-dark .bg-zinc-900 { background-color: var(--bpm-surface) !important; }
.bpm-dark .bg-zinc-900\/50 { background-color: var(--bpm-surface) !important; }
.bpm-dark .bg-zinc-900\/40 { background-color: var(--bpm-surface) !important; }
.bpm-dark .bg-zinc-900\/80 { background-color: var(--bpm-surface) !important; }
.bpm-dark .bg-zinc-900\/90 { background-color: var(--bpm-surface) !important; }
.bpm-dark .bg-zinc-800 { background-color: var(--bpm-surface-hover) !important; }
.bpm-dark .bg-zinc-800\/50 { background-color: var(--bpm-surface) !important; }
.bpm-dark .bg-zinc-800\/80 { background-color: var(--bpm-surface) !important; }

/* Dark mode borders */
.bpm-dark .border-zinc-800\/30 { border-color: var(--bpm-border) !important; }
.bpm-dark .border-zinc-800 { border-color: var(--bpm-border) !important; }
.bpm-dark .border-zinc-700 { border-color: var(--bpm-border) !important; }
</style>
