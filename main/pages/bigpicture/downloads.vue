<template>
  <div class="px-8 py-6" :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)', minHeight: '100%' }">
    <!-- Download controls header — always visible while a queue exists so
         the user can resume after a pause (speed==0 while paused). -->
    <div
      v-if="queue.length > 0"
      class="flex items-center justify-between mb-4 px-1"
    >
      <div class="flex items-center gap-4">
        <div class="flex items-center gap-1.5">
          <ArrowDownTrayIcon class="size-4 text-blue-400" />
          <span class="text-sm font-medium text-zinc-300">
            {{ isPaused ? "Paused" : formatSpeed(stats.speed) }}
          </span>
        </div>
        <div v-if="!isPaused && stats.time > 0" class="flex items-center gap-1.5">
          <ClockIcon class="size-4 text-zinc-500" />
          <span class="text-sm text-zinc-500">
            {{ formatETA(stats.time) }} remaining
          </span>
        </div>
      </div>
      <div class="flex gap-2">
        <button
          :ref="(el: any) => registerAction(el, { onSelect: togglePause })"
          class="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
          :class="
            isPaused
              ? 'bg-green-600 text-white hover:bg-green-500'
              : 'bg-zinc-800 text-zinc-200 hover:bg-zinc-700'
          "
          @click="togglePause"
        >
          {{ isPaused ? "Resume" : "Pause" }}
        </button>
        <button
          :ref="(el: any) => registerAction(el, { onSelect: cancelCurrentDownload })"
          class="px-4 py-2 rounded-lg text-sm font-medium transition-colors bg-red-900/40 text-red-200 hover:bg-red-900/60"
          @click="cancelCurrentDownload"
        >
          Cancel
        </button>
      </div>
    </div>

    <div
      v-if="queue.length === 0 && completedDownloads.length === 0"
      class="flex items-center justify-center py-24"
    >
      <div class="text-center">
        <ArrowDownTrayIcon
          class="size-20 text-zinc-600 mx-auto mb-4"
        />
        <h2
          class="text-2xl font-semibold font-display text-zinc-300 mb-2"
        >
          Your download queue is empty
        </h2>
        <p class="text-zinc-600 text-sm">Games you download will appear here</p>
      </div>
    </div>

    <!-- Drag-mode hint -->
    <div
      v-if="draggedIdx !== null"
      class="flex items-center gap-2 mb-3 px-4 py-2 rounded-lg bg-blue-600/10 border border-blue-500/30 text-blue-300 text-sm"
    >
      <ArrowsUpDownIcon class="size-4" />
      <span>Reordering — D-pad up/down to move, A to drop, B to cancel</span>
    </div>

    <!-- Active downloads -->
    <div v-if="queue.length > 0" class="space-y-3">
      <div
        v-for="(item, qIdx) in queue"
        :key="item.meta.id"
        :ref="
          (el: any) =>
            registerItem(el, { onSelect: () => onQueueItemSelect(qIdx, item.meta.id) })
        "
        class="flex items-center gap-6 rounded-xl p-6 transition-colors"
        :class="draggedIdx === qIdx
          ? 'bg-blue-600/20 ring-2 ring-blue-500'
          : 'bg-zinc-900/50'"
      >
        <!-- Reorder buttons — controller-friendly size -->
        <div v-if="queue.length > 1" class="flex flex-col gap-1.5 flex-shrink-0">
          <button
            :ref="(el: any) => qIdx > 0 && registerAction(el, { onSelect: () => reorderDownload(qIdx, qIdx - 1) })"
            :disabled="qIdx === 0"
            aria-label="Move up in queue"
            class="size-9 inline-flex items-center justify-center rounded-lg bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            @click.stop="reorderDownload(qIdx, qIdx - 1)"
          >
            <svg class="size-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 15.75l7.5-7.5 7.5 7.5" /></svg>
          </button>
          <button
            :ref="(el: any) => qIdx < queue.length - 1 && registerAction(el, { onSelect: () => reorderDownload(qIdx, qIdx + 1) })"
            :disabled="qIdx === queue.length - 1"
            aria-label="Move down in queue"
            class="size-9 inline-flex items-center justify-center rounded-lg bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            @click.stop="reorderDownload(qIdx, qIdx + 1)"
          >
            <svg class="size-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" /></svg>
          </button>
        </div>
        <!-- Cover art -->
        <div class="size-16 rounded-lg overflow-hidden bg-zinc-800 flex-shrink-0">
          <img
            v-if="gameNames[item.meta.id]?.coverUrl"
            :src="gameNames[item.meta.id].coverUrl"
            class="w-full h-full object-cover"
          />
          <div v-else class="w-full h-full flex items-center justify-center text-zinc-600 text-xl font-bold">
            {{ (gameNames[item.meta.id]?.name || item.meta.id)[0] }}
          </div>
        </div>

        <div class="flex-1 min-w-0">
          <p
            class="text-lg font-medium text-zinc-200"
          >
            {{ gameNames[item.meta.id]?.name || item.meta.id }}
          </p>
          <div class="flex items-center gap-2 mt-0.5">
            <span class="text-xs text-zinc-500">{{ item.status }}</span>
            <span v-if="item.dl_current > 0" class="text-xs text-zinc-600">
              {{ formatBytes(Math.min(item.dl_current, item.dl_max)) }} /
              {{ formatBytes(item.dl_max) }}
            </span>
          </div>
        </div>

        <div
          v-if="item.dl_progress != null"
          class="w-48"
        >
          <div class="h-2 bg-zinc-800 rounded-full overflow-hidden">
            <div
              class="h-full bg-blue-500 rounded-full transition-all duration-300"
              :style="{ width: `${Math.min(item.dl_progress * 100, 100).toFixed(0)}%` }"
            />
          </div>
          <p class="text-xs text-zinc-500 mt-1 text-right">
            {{ Math.min(item.dl_progress * 100, 100).toFixed(0) }}%
          </p>
        </div>
      </div>
    </div>

    <!-- Completed downloads (history) -->
    <div v-if="completedDownloads.length > 0" class="mt-6">
      <h3 class="text-sm font-medium text-zinc-500 mb-3 px-1">Recently Completed</h3>
      <div class="space-y-2">
        <div
          v-for="item in completedDownloads"
          :key="item.gameId + item.completedAt"
          :ref="
            (el: any) =>
              registerItem(el, { onSelect: () => navigateToGame(item.gameId) })
          "
          class="flex items-center gap-6 bg-zinc-900/30 rounded-xl p-5"
        >
          <!-- Cover art -->
          <div class="size-14 rounded-lg overflow-hidden bg-zinc-800 flex-shrink-0">
            <img
              v-if="gameNames[item.gameId]?.coverUrl"
              :src="gameNames[item.gameId].coverUrl"
              class="w-full h-full object-cover"
            />
            <div v-else class="w-full h-full flex items-center justify-center text-zinc-600 text-lg font-bold">
              {{ (gameNames[item.gameId]?.name || item.gameId)[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p class="text-base font-medium text-zinc-300">
              {{ gameNames[item.gameId]?.name || item.gameId }}
            </p>
            <span class="text-xs text-zinc-600">
              {{ formatTimeAgo(item.completedAt) }}
            </span>
          </div>

          <div class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-green-600/10">
            <CheckCircleIcon class="size-4 text-green-500" />
            <span class="text-xs font-medium text-green-400">Installed</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Cancel confirmation -->
    <BigPictureDialog
      :visible="showCancelConfirm"
      title="Cancel Download"
      :message="`Stop downloading ${cancelTargetName || 'this game'}? Any partial data will be removed.`"
      confirm-label="Cancel Download"
      cancel-label="Keep"
      :destructive="true"
      @confirm="confirmCancel"
      @cancel="showCancelConfirm = false"
    />
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { ArrowDownTrayIcon, ArrowsUpDownIcon, ClockIcon } from "@heroicons/vue/24/outline";
import { CheckCircleIcon } from "@heroicons/vue/24/solid";
import {
  useQueueState,
  useStatsState,
  useCompletedDownloads,
  formatKilobytes,
} from "~/composables/downloads";
import { useGame } from "~/composables/game";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
definePageMeta({ layout: "bigpicture" });
const queueState = useQueueState();
const statsState = useStatsState();
const completedDownloads = useCompletedDownloads();
const queue = computed(() => queueState.value?.queue ?? []);
const stats = computed(() => statsState.value ?? { speed: 0, time: 0 });
const isPaused = computed(() => queueState.value?.status === "Paused");

// Fetch game names for queue items (they only have IDs in meta)
const gameNames = ref<Record<string, { name: string; coverUrl?: string }>>({});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

async function loadGameName(id: string) {
  if (gameNames.value[id]) return;
  try {
    const data = await useGame(id);
    gameNames.value[id] = {
      name: data.game.mName,
      coverUrl: data.game.mCoverObjectId ? objectUrl(data.game.mCoverObjectId) : undefined,
    };
  } catch {
    // useGame failed — game may not be in library cache yet (first download).
    // Mark as pending so we retry on the next queue update.
  }
}

// Also retry names that are still showing IDs on each queue update
function hasMissingNames(): boolean {
  for (const item of queue.value) {
    if (!gameNames.value[item.meta.id]) return true;
  }
  return false;
}

async function loadGameNames() {
  // Collect all unique IDs that we don't already have names for
  const ids = new Set<string>();
  for (const item of queue.value) {
    if (!gameNames.value[item.meta.id]) ids.add(item.meta.id);
  }
  for (const item of completedDownloads.value) {
    if (!gameNames.value[item.gameId]) ids.add(item.gameId);
  }
  // Fetch all missing names in parallel
  await Promise.all([...ids].map((id) => loadGameName(id)));
}

// Debounce watch to avoid hammering during active downloads.
// If names are still missing, use a shorter debounce to retry sooner.
let _loadNamesTimer: ReturnType<typeof setTimeout> | null = null;
function debouncedLoadGameNames() {
  if (_loadNamesTimer) clearTimeout(_loadNamesTimer);
  const delay = hasMissingNames() ? 1000 : 300;
  _loadNamesTimer = setTimeout(() => loadGameNames(), delay);
}

// Load names immediately on first render, debounce subsequent updates
loadGameNames();
watch(queue, debouncedLoadGameNames);
watch(completedDownloads, debouncedLoadGameNames);

function formatTimeAgo(timestamp: number): string {
  const diff = Math.floor((Date.now() - timestamp) / 1000);
  if (diff < 60) return "Just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

// C6 fix: register items with focus group so controller can interact
const focusNav = useFocusNavigation();
const gamepad = useGamepad();
const registerItem = useBpFocusableGroup("content");
const registerAction = useBpFocusableGroup("content");

// ── Long-press A to enter reorder mode ──────────────────────────────────────
// Holding A on a queue row for 500ms grabs it. While grabbed, D-pad up/down
// moves the row; A drops; B cancels.
const draggedIdx = ref<number | null>(null);
const LONG_PRESS_MS = 500;
let longPressTimer: ReturnType<typeof setTimeout> | null = null;
let longPressReleaseUnsub: (() => void) | null = null;
let dragInputLockId: number | null = null;
const dragUnsubs: (() => void)[] = [];

function onQueueItemSelect(idx: number, gameId: string) {
  // If already dragging, the focus-nav onSelect fires via our own handler in
  // enterDragMode() — so this path only runs when we're NOT dragging.
  if (draggedIdx.value !== null) return;

  longPressTimer = setTimeout(() => {
    longPressTimer = null;
    longPressReleaseUnsub?.();
    longPressReleaseUnsub = null;
    enterDragMode(idx);
  }, LONG_PRESS_MS);

  longPressReleaseUnsub = gamepad.onButtonRelease(GamepadButton.South, () => {
    longPressReleaseUnsub?.();
    longPressReleaseUnsub = null;
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      longPressTimer = null;
      navigateToGame(gameId);
    }
  });
}

function enterDragMode(idx: number) {
  draggedIdx.value = idx;
  gamepad.vibrate("medium");
  dragInputLockId = focusNav.acquireInputLock();
  const bypass = { bypassInputLock: true };
  dragUnsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, moveDraggedUp, bypass),
    gamepad.onButton(GamepadButton.DPadDown, moveDraggedDown, bypass),
    gamepad.onButton(GamepadButton.South, exitDragMode, bypass),
    gamepad.onButton(GamepadButton.East, exitDragMode, bypass),
  );
}

function exitDragMode() {
  draggedIdx.value = null;
  if (dragInputLockId !== null) {
    focusNav.releaseInputLock(dragInputLockId);
    dragInputLockId = null;
  }
  for (const unsub of dragUnsubs) unsub();
  dragUnsubs.length = 0;
}

async function moveDraggedUp() {
  if (draggedIdx.value === null || draggedIdx.value === 0) return;
  const from = draggedIdx.value;
  await reorderDownload(from, from - 1);
  draggedIdx.value = from - 1;
  gamepad.vibrate("light");
}

async function moveDraggedDown() {
  if (draggedIdx.value === null || draggedIdx.value >= queue.value.length - 1) return;
  const from = draggedIdx.value;
  await reorderDownload(from, from + 1);
  draggedIdx.value = from + 1;
  gamepad.vibrate("light");
}

onMounted(() => {
  focusNav.autoFocusContent("content");
});

onUnmounted(() => {
  if (longPressTimer) clearTimeout(longPressTimer);
  longPressReleaseUnsub?.();
  exitDragMode();
});

// Download listeners are set up in app.vue via useDownloadListeners(),
// which updates the shared useState("queue") that useQueueState() reads.

// C6 fix: allow navigating to game detail from download item
const router = useRouter();
function navigateToGame(gameId: string) {
  const target = `/bigpicture/library/${gameId}`;
  focusNav.setRouteState("backTo", "/bigpicture/downloads", target);
  router.push(target);
}

async function togglePause() {
  try {
    if (isPaused.value) {
      await invoke("resume_downloads");
    } else {
      await invoke("pause_downloads");
    }
  } catch (e) {
    console.error("Failed to toggle pause:", e);
  }
}

async function reorderDownload(oldIndex: number, newIndex: number) {
  try {
    await invoke("move_download_in_queue", { oldIndex, newIndex });
  } catch (e) {
    console.error("Failed to reorder download:", e);
  }
}

const showCancelConfirm = ref(false);
const cancelTargetMeta = ref<any>(null);
const cancelTargetName = ref<string>("");

function cancelCurrentDownload() {
  const current = queue.value[0];
  if (!current) return;
  cancelTargetMeta.value = current.meta;
  cancelTargetName.value = gameNames.value[current.meta.id]?.name ?? "";
  showCancelConfirm.value = true;
}

async function confirmCancel() {
  const meta = cancelTargetMeta.value;
  showCancelConfirm.value = false;
  if (!meta) return;
  try {
    await invoke("cancel_game", { meta });
  } catch (e) {
    console.error("Failed to cancel download:", e);
  } finally {
    cancelTargetMeta.value = null;
    cancelTargetName.value = "";
  }
}

/** Speed arrives from the backend in KB/s — convert to bytes then format. */
function formatSpeed(kbPerSec: number): string {
  return _formatBytesImpl(kbPerSec * 1000) + "/s";
}

function formatBytes(bytes: number): string {
  return _formatBytesImpl(bytes);
}

/** Properly format byte values starting from B (not KB). */
function _formatBytesImpl(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;
  const scalar = 1000;

  while (value >= scalar && unitIndex < units.length - 1) {
    value /= scalar;
    unitIndex++;
  }

  return unitIndex === 0
    ? `${Math.round(value)} ${units[unitIndex]}`
    : `${value.toFixed(1)} ${units[unitIndex]}`;
}

function formatETA(seconds: number): string {
  if (seconds <= 0) return "calculating";
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.round((seconds % 3600) / 60);
  return `${hours}h ${mins}m`;
}
</script>