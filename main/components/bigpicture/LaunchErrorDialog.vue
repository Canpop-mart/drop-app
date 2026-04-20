<template>
  <Teleport to="body">
    <Transition name="bp-dialog">
      <div
        v-if="visible"
        class="fixed inset-0 z-[70] flex items-center justify-center bg-black/80 backdrop-blur-sm"
      >
        <!-- Error dialog body -->
        <div
          v-if="!logOpen"
          class="bg-zinc-900 border border-red-500/30 rounded-2xl shadow-2xl p-8 max-w-xl w-full mx-4"
        >
          <div class="flex items-start gap-4 mb-4">
            <div class="shrink-0 mt-1 size-10 rounded-full bg-red-500/15 border border-red-500/40 flex items-center justify-center">
              <svg class="size-6 text-red-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" y2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <div class="flex-1 min-w-0">
              <h2 class="text-xl font-semibold font-display text-zinc-100">
                {{ gameName ? `${gameName} failed to start` : "Launch failed" }}
              </h2>
              <p class="text-zinc-400 text-sm mt-1">
                The game exited unexpectedly after launch. The log below may
                help diagnose the problem.
              </p>
            </div>
          </div>

          <div class="rounded-xl border border-zinc-700/50 bg-zinc-800/40 px-4 py-3 text-sm text-zinc-300 mb-4 space-y-1">
            <div v-if="detail?.exitCode !== null && detail?.exitCode !== undefined">
              <span class="text-zinc-500">Exit code:</span>
              <span class="font-mono text-zinc-200 ml-2">{{ detail?.exitCode }}</span>
            </div>
            <div v-if="detail?.elapsedSecs !== undefined">
              <span class="text-zinc-500">Ran for:</span>
              <span class="font-mono text-zinc-200 ml-2">{{ detail?.elapsedSecs }}s</span>
            </div>
            <div v-if="detail?.ioError" class="break-words">
              <span class="text-zinc-500">IO error:</span>
              <span class="font-mono text-red-300 ml-2">{{ detail?.ioError }}</span>
            </div>
            <div v-if="logPath" class="break-all text-xs text-zinc-500">
              {{ logPath }}
            </div>
          </div>

          <!-- Buttons -->
          <div class="flex flex-wrap items-center justify-end gap-3">
            <button
              class="px-4 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="focusedButton === 'dismiss'
                ? 'bg-zinc-700 text-zinc-100 ring-2 ring-blue-500'
                : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200'"
              @click="dismiss"
            >
              <BigPictureButtonPrompt button="B" label="Dismiss" size="sm" />
            </button>
            <button
              class="px-4 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="focusedButton === 'view'
                ? 'bg-blue-600 text-white ring-2 ring-blue-400 shadow-lg shadow-blue-500/30'
                : 'bg-blue-600/80 text-blue-100 hover:bg-blue-600'"
              :disabled="!logTail && !loadingLog"
              @click="openLog"
            >
              <BigPictureButtonPrompt button="A" :label="loadingLog ? 'Loading…' : 'View log'" size="sm" />
            </button>
            <button
              class="px-4 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="focusedButton === 'copy'
                ? 'bg-zinc-700 text-zinc-100 ring-2 ring-blue-500'
                : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200'"
              :disabled="!logTail"
              @click="copyLog"
            >
              <BigPictureButtonPrompt button="X" :label="copied ? 'Copied!' : 'Copy log'" size="sm" />
            </button>
            <button
              class="px-4 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="focusedButton === 'report'
                ? 'bg-red-600 text-white ring-2 ring-red-300 shadow-lg shadow-red-500/30'
                : 'bg-red-600/80 text-red-100 hover:bg-red-600'"
              @click="reportBug"
            >
              <BigPictureButtonPrompt button="Y" label="Report bug" size="sm" />
            </button>
          </div>
        </div>

        <!-- Log viewer overlay -->
        <div
          v-else
          class="bg-zinc-950 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-5xl w-full mx-4 flex flex-col"
          style="max-height: 80vh"
        >
          <div class="flex items-center justify-between mb-3">
            <div class="min-w-0">
              <h2 class="text-base font-semibold font-display text-zinc-100">
                Launch log
              </h2>
              <p class="text-xs text-zinc-500 break-all">
                {{ logPath }}<span v-if="logTruncated"> &middot; tail only</span>
              </p>
            </div>
            <button
              class="px-3 py-2 rounded-lg text-xs font-medium transition-colors bg-zinc-800 text-zinc-300 hover:bg-zinc-700 ring-2"
              :class="logOpen ? 'ring-blue-500' : 'ring-transparent'"
              @click="closeLog"
            >
              <BigPictureButtonPrompt button="B" label="Back" size="sm" />
            </button>
          </div>

          <pre
            ref="logPre"
            class="flex-1 overflow-auto rounded-lg border border-zinc-800 bg-black p-3 text-[11px] leading-tight font-mono text-zinc-300 whitespace-pre-wrap break-words"
          >{{ logTail || "(log is empty)" }}</pre>

          <div class="flex items-center justify-end gap-2 mt-3">
            <button
              class="px-3 py-2 rounded-lg text-xs font-medium transition-colors bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
              @click="copyLog"
            >
              <BigPictureButtonPrompt button="X" :label="copied ? 'Copied!' : 'Copy'" size="sm" />
            </button>
            <button
              class="px-3 py-2 rounded-lg text-xs font-medium transition-colors bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
              @click="toggleStderr"
            >
              <BigPictureButtonPrompt button="Y" :label="showStderr ? 'Show stdout' : 'Show stderr'" size="sm" />
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useDeckMode } from "~/composables/deck-mode";
import { useListen } from "~/composables/useListen";
import { invoke } from "@tauri-apps/api/core";

// Under gamescope the physical X button reports as North and physical Y
// reports as West — invert which logical button triggers "copy" vs "report"
// so the on-screen X/Y prompts match the physical buttons on Deck.
const { isGamescope: _isGS } = useDeckMode();
const _copyBtn = _isGS.value ? GamepadButton.North : GamepadButton.West;
const _reportBtn = _isGS.value ? GamepadButton.West : GamepadButton.North;

type FocusableButton = "dismiss" | "view" | "copy" | "report";
const BUTTON_ORDER: FocusableButton[] = ["dismiss", "view", "copy", "report"];

interface LaunchErrorDetail {
  gameId: string;
  exitCode: number | null;
  elapsedSecs: number;
  ioError: string | null;
}

interface LaunchLogTail {
  path: string;
  tail: string;
  truncated: boolean;
}

const visible = ref(false);
const logOpen = ref(false);
const detail = ref<LaunchErrorDetail | null>(null);
const gameName = ref<string | null>(null);
const logTail = ref("");
const logPath = ref("");
const logTruncated = ref(false);
const loadingLog = ref(false);
const showStderr = ref(false);
const copied = ref(false);
const focusedButton = ref<FocusableButton>("view");

const logPre = ref<HTMLPreElement | null>(null);

const focusNav = useFocusNavigation();
const gamepad = useGamepad();
const router = useRouter();
let lockId = 0;
const unsubs: (() => void)[] = [];

useListen<LaunchErrorDetail>("launch_external_error_detail", async (event) => {
  if (!event.payload) return;
  detail.value = event.payload;
  const gid = event.payload.gameId;
  gameName.value = await resolveGameName(gid);
  visible.value = true;
  logOpen.value = false;
  focusedButton.value = "view";
  copied.value = false;
  lockId = focusNav.acquireInputLock();
  wireGamepad();
  await loadLog();
});

async function resolveGameName(gid: string): Promise<string | null> {
  try {
    const result: any = await invoke("fetch_game", { gameId: gid });
    return result?.game?.mName ?? result?.mName ?? null;
  } catch (e) {
    console.warn("[BPM:LaunchError] Could not resolve game name for", gid, e);
    return null;
  }
}

async function loadLog() {
  if (!detail.value) return;
  loadingLog.value = true;
  try {
    const result = await invoke<LaunchLogTail>("read_latest_launch_log", {
      gameId: detail.value.gameId,
      maxLines: 400,
      stderr: showStderr.value,
    });
    logTail.value = result.tail;
    logPath.value = result.path;
    logTruncated.value = result.truncated;
  } catch (e) {
    console.warn("[BPM:LaunchError] Failed to read launch log:", e);
    logTail.value = `(Failed to read log: ${e})`;
  } finally {
    loadingLog.value = false;
  }
}

function openLog() {
  if (!logTail.value && !loadingLog.value) return;
  logOpen.value = true;
  nextTick(() => {
    logPre.value?.scrollTo({ top: logPre.value.scrollHeight });
  });
}

function closeLog() {
  logOpen.value = false;
}

async function toggleStderr() {
  showStderr.value = !showStderr.value;
  await loadLog();
  nextTick(() => {
    logPre.value?.scrollTo({ top: logPre.value.scrollHeight });
  });
}

async function copyLog() {
  if (!logTail.value) return;
  try {
    await navigator.clipboard.writeText(logTail.value);
    copied.value = true;
    setTimeout(() => { copied.value = false; }, 2000);
  } catch (e) {
    console.warn("[BPM:LaunchError] Clipboard write failed:", e);
  }
}

function reportBug() {
  const gid = detail.value?.gameId ?? "";
  const exit = detail.value?.exitCode;
  const title = gameName.value
    ? `${gameName.value} fails to launch`
    : `Game failed to launch`;
  const body = [
    `Game ID: ${gid}`,
    exit !== null && exit !== undefined ? `Exit code: ${exit}` : "",
    detail.value?.ioError ? `IO error: ${detail.value.ioError}` : "",
    `Ran for ~${detail.value?.elapsedSecs ?? 0}s before exit.`,
  ].filter(Boolean).join("\n");
  dismiss();
  router.push({
    path: "/bigpicture/bugreport",
    query: { title, body, attachLog: "1" },
  });
}

function dismiss() {
  visible.value = false;
  logOpen.value = false;
  detail.value = null;
  logTail.value = "";
  logPath.value = "";
  unwireGamepad();
  focusNav.releaseInputLock(lockId);
}

function cycleFocus(delta: number) {
  const i = BUTTON_ORDER.indexOf(focusedButton.value);
  const next = (i + delta + BUTTON_ORDER.length) % BUTTON_ORDER.length;
  focusedButton.value = BUTTON_ORDER[next];
}

function wireGamepad() {
  unwireGamepad();

  unsubs.push(
    gamepad.onButton(GamepadButton.DPadLeft, () => {
      if (!visible.value || logOpen.value) return;
      cycleFocus(-1);
    }),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadRight, () => {
      if (!visible.value || logOpen.value) return;
      cycleFocus(1);
    }),
  );

  // A — activate focused button; in log view, A does nothing (use X/Y/B)
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!visible.value) return;
      if (logOpen.value) return;
      switch (focusedButton.value) {
        case "dismiss": dismiss(); break;
        case "view": openLog(); break;
        case "copy": copyLog(); break;
        case "report": reportBug(); break;
      }
    }),
  );

  // B — close log viewer if open, else dismiss dialog
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!visible.value) return;
      if (logOpen.value) {
        closeLog();
      } else {
        dismiss();
      }
    }),
  );

  // X — copy log from anywhere (physical X on Deck via _copyBtn)
  unsubs.push(
    gamepad.onButton(_copyBtn, () => {
      if (!visible.value) return;
      copyLog();
    }),
  );

  // Y — report bug / toggle stderr (contextual; physical Y on Deck via _reportBtn)
  unsubs.push(
    gamepad.onButton(_reportBtn, () => {
      if (!visible.value) return;
      if (logOpen.value) {
        toggleStderr();
      } else {
        reportBug();
      }
    }),
  );

  // Scroll log pre when in log view
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!visible.value || !logOpen.value) return;
      logPre.value?.scrollBy({ top: -80, behavior: "smooth" });
    }),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!visible.value || !logOpen.value) return;
      logPre.value?.scrollBy({ top: 80, behavior: "smooth" });
    }),
  );
}

function unwireGamepad() {
  for (const u of unsubs) u();
  unsubs.length = 0;
}

onUnmounted(() => {
  if (visible.value) {
    focusNav.releaseInputLock(lockId);
  }
  unwireGamepad();
});
</script>

<style scoped>
.bp-dialog-enter-active,
.bp-dialog-leave-active {
  transition: opacity 0.2s ease;
}
.bp-dialog-enter-active > div,
.bp-dialog-leave-active > div {
  transition: transform 0.2s ease, opacity 0.2s ease;
}
.bp-dialog-enter-from,
.bp-dialog-leave-to {
  opacity: 0;
}
.bp-dialog-enter-from > div {
  transform: scale(0.95);
  opacity: 0;
}
.bp-dialog-leave-to > div {
  transform: scale(0.95);
  opacity: 0;
}
</style>
