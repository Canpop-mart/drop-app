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
      class="fixed bottom-14 right-4 w-[32rem] max-h-80 overflow-y-auto bg-black/90 border border-zinc-700 rounded-lg p-3 z-[999] font-mono text-[11px] leading-tight"
    >
      <div class="flex items-center justify-between mb-2">
        <span class="text-green-400 font-bold">BPM Debug Console</span>
        <div class="flex items-center gap-2">
          <button
            class="px-2 py-0.5 text-[10px] bg-zinc-700 hover:bg-zinc-600 text-zinc-300 rounded"
            @click="exportDebugLog"
          >
            Export Log
          </button>
          <button class="text-zinc-500 hover:text-zinc-300" @click="debugVisible = false">✕</button>
        </div>
      </div>
      <div v-if="exportMessage" class="text-green-400 text-[10px] mb-1">{{ exportMessage }}</div>
      <div v-for="(msg, i) in debugMessages" :key="i" :class="msg.color">
        {{ msg.text }}
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
// Hidden by default — toggle with Select+Start on gamepad
const debugVisible = ref(false);
const debugMessages = ref<{ text: string; color: string }[]>([]);
const MAX_DEBUG = 80;

function debugLog(msg: string, level: "info" | "warn" | "error" = "info") {
  const colors = { info: "text-zinc-300", warn: "text-yellow-400", error: "text-red-400" };
  const ts = new Date().toLocaleTimeString("en", { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" });
  debugMessages.value.push({ text: `[${ts}] ${msg}`, color: colors[level] });
  if (debugMessages.value.length > MAX_DEBUG) debugMessages.value.shift();
}

const exportMessage = ref("");

/**
 * Export debug log — writes to a file in the app's data dir AND copies
 * to clipboard so the user can paste it after exiting Gaming Mode.
 */
async function exportDebugLog() {
  const logText = debugMessages.value.map((m) => m.text).join("\n");

  // Try clipboard first
  try {
    await navigator.clipboard.writeText(logText);
    exportMessage.value = "Copied to clipboard!";
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
      exportMessage.value = "Log file downloaded!";
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

function interceptConsole(origFn: Function, level: "info" | "warn" | "error") {
  return function (...args: any[]) {
    origFn.apply(console, args);
    const text = args.map((a) => (typeof a === "string" ? a : JSON.stringify(a)?.slice(0, 200))).join(" ");
    if (text.includes("[BPM:") || text.includes("[useGame") || text.includes("[FOCUS]") || text.includes("[GAMEPAD]") || text.includes("[ERROR")) {
      debugLog(text.slice(0, 300), level);
      // Auto-show overlay on errors
      if (level === "error") debugVisible.value = true;
    }
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
  focusNav.enabled.value = false;
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
