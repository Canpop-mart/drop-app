<template>
  <div class="px-8 py-6">
    <!-- Download stats header (speed + ETA) -->
    <div
      v-if="queue.length > 0 && (stats.speed > 0 || stats.time > 0)"
      class="flex items-center justify-between mb-4 px-1"
    >
      <div class="flex items-center gap-4">
        <div class="flex items-center gap-1.5">
          <ArrowDownTrayIcon class="size-4 text-blue-400" />
          <span class="text-sm font-medium text-zinc-300">
            {{ formatSpeed(stats.speed) }}/s
          </span>
        </div>
        <div v-if="stats.time > 0" class="flex items-center gap-1.5">
          <ClockIcon class="size-4 text-zinc-500" />
          <span class="text-sm text-zinc-500">
            {{ formatETA(stats.time) }} remaining
          </span>
        </div>
      </div>
      <div class="flex gap-2">
        <button
          :ref="(el: any) => registerAction(el, { onSelect: togglePause })"
          class="px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
          :class="
            isPaused
              ? 'bg-green-600/20 text-green-400 hover:bg-green-600/30'
              : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
          "
          @click="togglePause"
        >
          {{ isPaused ? "Resume" : "Pause" }}
        </button>
      </div>
    </div>

    <div
      v-if="queue.length === 0"
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

    <div v-else class="space-y-3">
      <div
        v-for="item in queue"
        :key="item.meta.id"
        :ref="
          (el: any) =>
            registerItem(el, { onSelect: () => navigateToGame(item.meta.id) })
        "
        class="flex items-center gap-6 bg-zinc-900/50 rounded-xl p-6"
      >
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
              {{ formatBytes(item.dl_current) }} /
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
              :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
            />
          </div>
          <p class="text-xs text-zinc-500 mt-1 text-right">
            {{ (item.dl_progress * 100).toFixed(0) }}%
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { ArrowDownTrayIcon, ClockIcon } from "@heroicons/vue/24/outline";
import {
  useQueueState,
  useStatsState,
  formatKilobytes,
} from "~/composables/downloads";
import { useGame } from "~/composables/game";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
definePageMeta({ layout: "bigpicture" });
const queueState = useQueueState();
const statsState = useStatsState();
const queue = computed(() => queueState.value?.queue ?? []);
const stats = computed(() => statsState.value ?? { speed: 0, time: 0 });
const isPaused = computed(() => queueState.value?.status === "Paused");

// Fetch game names for queue items (they only have IDs in meta)
const gameNames = ref<Record<string, { name: string; coverUrl?: string }>>({});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

async function loadGameNames() {
  for (const item of queue.value) {
    const id = item.meta.id;
    if (gameNames.value[id]) continue;
    try {
      const data = await useGame(id);
      gameNames.value[id] = {
        name: data.game.mName,
        coverUrl: data.game.mCoverObjectId ? objectUrl(data.game.mCoverObjectId) : undefined,
      };
    } catch {
      // Game data not available — will keep showing ID
    }
  }
}

watch(queue, () => loadGameNames(), { immediate: true });

// C6 fix: register items with focus group so controller can interact
const focusNav = useFocusNavigation();
const registerItem = useBpFocusableGroup("content");
const registerAction = useBpFocusableGroup("content");

onMounted(() => {
  focusNav.autoFocusContent("content");
});

// Download listeners are set up in app.vue via useDownloadListeners(),
// which updates the shared useState("queue") that useQueueState() reads.

// C6 fix: allow navigating to game detail from download item
const router = useRouter();
function navigateToGame(gameId: string) {
  router.push(`/bigpicture/library/${gameId}`);
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

function formatSpeed(bytesPerSec: number): string {
  return _formatBytesImpl(bytesPerSec) + "/s";
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