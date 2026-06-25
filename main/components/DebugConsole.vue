<template>
  <!--
    Desktop dev console. The desktop client has no devtools surface for
    users, so this mirrors the Big Picture Mode debug overlay: it
    intercepts every console method + uncaught errors, buffers the output,
    and renders it in a toggleable panel. Gated behind dev mode.

    Toggle: Ctrl+`  (or the bottom-right corner button)
  -->
  <template v-if="devMode.enabled.value">
    <!-- Collapsed — small corner button. -->
    <button
      v-if="!visible"
      class="fixed bottom-3 right-3 z-[9998] inline-flex items-center gap-1.5 rounded-md bg-zinc-800/90 px-2.5 py-1.5 text-[11px] font-mono text-zinc-300 ring-1 ring-zinc-700 shadow-lg hover:bg-zinc-700 hover:text-zinc-100 transition-colors"
      title="Open dev console (Ctrl+`)"
      @click="visible = true"
    >
      <span class="text-green-400">›_</span> Console
      <span
        v-if="errorCount > 0"
        class="rounded-full bg-red-600 px-1.5 text-[10px] font-bold text-white"
        >{{ errorCount }}</span
      >
    </button>

    <!-- Expanded — full panel. -->
    <div
      v-else
      class="fixed bottom-3 right-3 z-[9999] w-[44rem] max-w-[calc(100vw-1.5rem)] flex flex-col rounded-lg border border-zinc-700 bg-black/95 font-mono text-[11px] leading-tight shadow-2xl"
      style="max-height: 60vh"
    >
      <!-- Header. -->
      <div class="flex shrink-0 items-center justify-between px-3 pt-2 pb-1">
        <div class="flex items-center gap-2">
          <span class="font-bold text-green-400">Drop Dev Console</span>
          <span class="text-[10px] text-zinc-500">
            {{ filteredCount }} / {{ rawBuffer.length
            }}{{ rawBuffer.length === MAX_DEBUG ? "+" : "" }}
            <span v-if="paused" class="ml-1 text-yellow-400">· PAUSED</span>
            <span v-else-if="!autoTail" class="ml-1 text-amber-400"
              >· scrolled</span
            >
          </span>
        </div>
        <button
          class="text-zinc-500 hover:text-zinc-300"
          @click="visible = false"
        >
          ✕
        </button>
      </div>

      <!-- Controls. -->
      <div class="flex shrink-0 items-center gap-1 px-3 pb-1">
        <button
          class="rounded px-2 py-0.5 text-[10px] transition-colors"
          :class="
            paused
              ? 'bg-yellow-600 text-white'
              : 'bg-zinc-700 text-zinc-300 hover:bg-zinc-600'
          "
          @click="paused = !paused"
        >
          {{ paused ? "Resume" : "Pause" }}
        </button>
        <button
          class="rounded bg-zinc-700 px-2 py-0.5 text-[10px] text-zinc-300 hover:bg-zinc-600"
          @click="clearLog"
        >
          Clear
        </button>
        <button
          class="rounded px-2 py-0.5 text-[10px] transition-colors"
          :class="
            autoTail
              ? 'bg-emerald-600 text-white'
              : 'bg-zinc-700 text-zinc-300 hover:bg-zinc-600'
          "
          @click="toggleAutoTail"
        >
          Tail
        </button>
        <input
          v-model="filter"
          type="text"
          placeholder="filter…"
          class="min-w-0 flex-1 rounded border border-zinc-700 bg-zinc-900 px-2 py-0.5 text-[10px] text-zinc-200 focus:border-zinc-500 focus:outline-none"
        />
        <button
          class="rounded bg-zinc-700 px-2 py-0.5 text-[10px] text-zinc-300 hover:bg-zinc-600"
          @click="copyLog"
        >
          Copy
        </button>
      </div>
      <div v-if="copyMessage" class="shrink-0 px-3 text-[10px] text-green-400">
        {{ copyMessage }}
      </div>

      <!-- Message list. -->
      <div
        ref="scrollEl"
        class="flex-1 overflow-y-auto px-3 pb-2"
        @scroll="onScroll"
      >
        <div
          v-if="visibleMessages.length === 0"
          class="py-1 text-[10px] italic text-zinc-600"
        >
          {{ filter ? "No matches for filter." : "No console output yet." }}
        </div>
        <div
          v-if="truncated"
          class="mb-1 border-b border-zinc-800 pb-1 text-[10px] italic text-zinc-500"
        >
          Showing last {{ RENDER_CAP }} of {{ filteredCount }} — narrow the
          filter or Copy to get the rest.
        </div>
        <div
          v-for="msg in visibleMessages"
          :key="msg.id"
          :class="msg.color"
          class="break-all whitespace-pre-wrap"
        >
          {{ msg.text }}
        </div>
      </div>
    </div>
  </template>
</template>

<script setup lang="ts">
/**
 * Desktop dev console — see the template comment. Self-contained: drop a
 * single <DebugConsole /> into the desktop layout. It self-gates on dev
 * mode for both the UI and the console interception.
 */
const devMode = useDevMode();

const visible = ref(false);
const paused = ref(false);
const filter = ref("");
const autoTail = ref(true);
const copyMessage = ref("");
const errorCount = ref(0);

type LogLevel = "log" | "info" | "warn" | "error" | "debug";
type LogMsg = { id: number; text: string; color: string };

const MAX_DEBUG = 5000;
const RENDER_CAP = 500;

const rawBuffer = ref<LogMsg[]>([]);
const pausedSnapshot = ref<LogMsg[] | null>(null);
let idCounter = 0;

const LEVEL_COLOR: Record<LogLevel, string> = {
  error: "text-red-400",
  warn: "text-yellow-400",
  info: "text-zinc-300",
  log: "text-zinc-300",
  debug: "text-zinc-500",
};

function push(level: LogLevel, text: string) {
  const ts = new Date().toLocaleTimeString("en", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  rawBuffer.value.push({
    id: ++idCounter,
    text: `[${ts}] ${text}`,
    color: LEVEL_COLOR[level],
  });
  if (rawBuffer.value.length > MAX_DEBUG) rawBuffer.value.shift();
  if (level === "error") {
    errorCount.value++;
    // Surface failures: pop the panel open on the first error so a
    // silent bug can't go unnoticed.
    visible.value = true;
  }
}

// When paused, freeze on a snapshot so the list stays still while the
// user reads it.
const sourceList = computed<LogMsg[]>(() =>
  paused.value && pausedSnapshot.value ? pausedSnapshot.value : rawBuffer.value,
);
const filtered = computed<LogMsg[]>(() => {
  const q = filter.value.trim().toLowerCase();
  if (!q) return sourceList.value;
  return sourceList.value.filter((m) => m.text.toLowerCase().includes(q));
});
const filteredCount = computed(() => filtered.value.length);
const visibleMessages = computed<LogMsg[]>(() =>
  filtered.value.length > RENDER_CAP
    ? filtered.value.slice(-RENDER_CAP)
    : filtered.value,
);
const truncated = computed(() => filtered.value.length > RENDER_CAP);

watch(paused, (p) => {
  pausedSnapshot.value = p ? [...rawBuffer.value] : null;
});

function clearLog() {
  rawBuffer.value = [];
  pausedSnapshot.value = null;
  errorCount.value = 0;
}

// ── Auto-tail ──────────────────────────────────────────────────────────────
const scrollEl = ref<HTMLElement | null>(null);

function scrollToBottom() {
  const el = scrollEl.value;
  if (el) el.scrollTop = el.scrollHeight;
}
function toggleAutoTail() {
  autoTail.value = !autoTail.value;
  if (autoTail.value) nextTick(scrollToBottom);
}
function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 16;
  if (autoTail.value && !atBottom) autoTail.value = false;
  else if (!autoTail.value && atBottom) autoTail.value = true;
}
watch(visibleMessages, () => {
  if (autoTail.value && !paused.value && visible.value) {
    nextTick(scrollToBottom);
  }
});
watch(visible, (v) => {
  if (v && autoTail.value) nextTick(scrollToBottom);
});

// ── Copy ───────────────────────────────────────────────────────────────────
async function copyLog() {
  const text = rawBuffer.value.map((m) => m.text).join("\n");
  try {
    await navigator.clipboard.writeText(text);
    copyMessage.value = `Copied ${rawBuffer.value.length} lines to clipboard.`;
  } catch {
    copyMessage.value = "Clipboard unavailable.";
  }
  setTimeout(() => {
    copyMessage.value = "";
  }, 2500);
}

// ── Console + error interception ───────────────────────────────────────────
const LEVELS: LogLevel[] = ["log", "info", "warn", "error", "debug"];
const originals: Partial<Record<LogLevel, (...a: unknown[]) => void>> = {};
const ourFns: Partial<Record<LogLevel, (...a: unknown[]) => void>> = {};
let attached = false;

function stringifyArg(v: unknown): string {
  if (typeof v === "string") return v;
  if (v instanceof Error) return v.stack || `${v.name}: ${v.message}`;
  try {
    const s = JSON.stringify(v);
    return s ?? String(v);
  } catch {
    return String(v);
  }
}

function makeInterceptor(level: LogLevel, orig: (...a: unknown[]) => void) {
  return (...args: unknown[]) => {
    orig.apply(console, args);
    try {
      push(level, args.map(stringifyArg).join(" ").slice(0, 2000));
    } catch {
      /* never let logging break logging */
    }
  };
}

function onWindowError(e: ErrorEvent) {
  push(
    "error",
    `UNCAUGHT: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`,
  );
}
function onRejection(e: PromiseRejectionEvent) {
  push("error", `UNHANDLED PROMISE REJECTION: ${stringifyArg(e.reason)}`);
}

/** Install the console interceptors + global error listeners. Idempotent. */
function attach() {
  if (attached || typeof window === "undefined") return;
  attached = true;
  for (const lvl of LEVELS) {
    const orig = (console[lvl] ?? console.log) as (...a: unknown[]) => void;
    originals[lvl] = orig;
    const fn = makeInterceptor(lvl, orig);
    ourFns[lvl] = fn;
    (console as unknown as Record<string, unknown>)[lvl] = fn;
  }
  window.addEventListener("error", onWindowError);
  window.addEventListener("unhandledrejection", onRejection);
  push("info", "[DebugConsole] attached — Ctrl+` toggles this panel.");
}

/** Restore the original console methods + remove listeners. Idempotent. */
function detach() {
  if (!attached || typeof window === "undefined") return;
  attached = false;
  for (const lvl of LEVELS) {
    // Only restore if it's still OUR interceptor — avoids clobbering a
    // different layout's interceptor that may have been installed after us.
    if (
      (console as unknown as Record<string, unknown>)[lvl] === ourFns[lvl] &&
      originals[lvl]
    ) {
      (console as unknown as Record<string, unknown>)[lvl] = originals[lvl];
    }
  }
  window.removeEventListener("error", onWindowError);
  window.removeEventListener("unhandledrejection", onRejection);
}

function onKey(e: KeyboardEvent) {
  // Ctrl+` toggles the panel — only while dev mode is on.
  if (devMode.enabled.value && e.ctrlKey && e.key === "`") {
    e.preventDefault();
    visible.value = !visible.value;
  }
}

onMounted(() => {
  if (devMode.enabled.value) attach();
  window.addEventListener("keydown", onKey);
});

// Honour dev mode being toggled mid-session.
watch(
  () => devMode.enabled.value,
  (on) => (on ? attach() : detach()),
);

onUnmounted(() => {
  detach();
  if (typeof window !== "undefined") {
    window.removeEventListener("keydown", onKey);
  }
});
</script>
