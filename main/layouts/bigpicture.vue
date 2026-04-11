<template>
  <div class="flex h-screen w-screen bg-zinc-950 overflow-hidden">
    <!-- Navigation Rail (left edge) -->
    <BigPictureNavRail />

    <!-- Main content area -->
    <div class="flex-1 flex flex-col min-w-0">
      <BigPictureTopBar />
      <div class="flex-1 overflow-y-auto" data-bp-scroll>
        <!-- No <Transition> wrapper — it caused white screens on WebKitGTK
             (SteamOS / Steam Deck) because position:absolute on the leaving
             page would cover the entering page if the transition didn't
             complete cleanly. Nuxt handles its own page transitions. -->
        <slot />
      </div>
      <BigPictureContextBar />
    </div>

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
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useDeckMode } from "~/composables/deck-mode";

const focusNav = useFocusNavigation();
const { isGamescope } = useDeckMode();

// ── On-screen debug overlay ─────────────────────────────────────────────
// Auto-show on Gamescope (Steam Deck) for debugging during development
const debugVisible = ref(isGamescope.value);
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
/* Focus indicator ring — applied by focus-navigation.ts */
[data-focusable].bp-focused {
  outline: 3px solid rgba(59, 130, 246, 0.8);
  outline-offset: 2px;
  border-radius: 0.75rem;
  transition:
    outline-color 0.12s ease,
    transform 0.08s ease;
}

/* Press feedback — brief scale-down on A button */
[data-focusable].bp-pressed {
  transform: scale(0.97);
  transition: transform 0.08s ease;
}
</style>
