<template>
  <div class="bg-zinc-950 p-4 min-h-full space-y-4">
    <!-- Header: pause/resume + speed stats + history graph -->
    <div
      class="h-16 overflow-hidden relative rounded-xl flex flex-row items-stretch border border-zinc-900 bg-zinc-900"
    >
      <button
        v-if="queue.queue.length > 0"
        @click="togglePause"
        :disabled="transitioning !== null"
        :aria-label="isPaused ? 'Resume downloads' : 'Pause downloads'"
        class="w-16 flex-shrink-0 z-10 flex items-center justify-center border-r border-zinc-800 transition-colors disabled:cursor-wait"
        :class="
          transitioning !== null
            ? 'bg-zinc-800 text-zinc-500'
            : isPaused
              ? 'bg-green-600 hover:bg-green-500 text-white'
              : 'hover:bg-zinc-800 text-zinc-200'
        "
      >
        <ArrowPathIcon v-if="transitioning !== null" class="size-6 animate-spin" />
        <PlayIcon v-else-if="isPaused" class="size-6" />
        <PauseIcon v-else class="size-6" />
      </button>
      <div
        class="z-10 flex flex-col justify-center px-3 min-w-[140px] font-display"
      >
        <span class="font-bold text-zinc-100">
          {{ headerPrimary }}
        </span>
        <span class="text-xs text-zinc-400">
          {{ headerSecondary }}
        </span>
      </div>
      <div class="flex-1 relative">
        <div
          class="absolute inset-0 h-full flex flex-row items-end justify-end space-x-[1px] pointer-events-none"
        >
          <div
            v-for="bar in speedHistory"
            :style="{ height: `${(bar / speedMax) * 100}%` }"
            class="w-[3px] bg-blue-600 rounded-t-full"
          />
        </div>
      </div>
    </div>
    <draggable v-model="queue.queue" @end="onEnd">
      <template #item="{ element, index: qIdx }: ListIterable">
        <li
          v-if="games[element.meta.id]"
          :key="element.meta.id"
          class="mb-4 bg-zinc-900 rounded-lg flex flex-row justify-between gap-x-6 py-5 px-4"
          :class="{ 'opacity-60': isCancelling(element.meta.id) }"
        >
          <div class="w-full flex items-center max-w-md gap-x-4 relative">
            <div
              v-if="queue.queue.length > 1"
              class="flex flex-col gap-1 flex-shrink-0 z-10"
            >
              <button
                :disabled="qIdx === 0 || isCancelling(element.meta.id)"
                aria-label="Move up in queue"
                class="size-6 inline-flex items-center justify-center rounded bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                @click.stop="reorderDownload(qIdx, qIdx - 1)"
              >
                <ChevronUpIcon class="size-4" />
              </button>
              <button
                :disabled="qIdx === queue.queue.length - 1 || isCancelling(element.meta.id)"
                aria-label="Move down in queue"
                class="size-6 inline-flex items-center justify-center rounded bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                @click.stop="reorderDownload(qIdx, qIdx + 1)"
              >
                <ChevronDownIcon class="size-4" />
              </button>
            </div>
            <img
              class="size-24 flex-none bg-zinc-800 object-cover rounded"
              :src="games[element.meta.id].cover"
              alt=""
            />
            <div class="min-w-0 flex-auto">
              <p class="text-xl font-semibold text-zinc-100">
                <NuxtLink :href="`/library/${element.meta.id}`" class="">
                  <span class="absolute inset-x-0 -top-px bottom-0" />
                  {{ games[element.meta.id].game.mName }}
                </NuxtLink>
              </p>
              <p class="mt-1 flex text-xs/5 text-gray-500">
                {{ games[element.meta.id].game.mShortDescription }}
              </p>
            </div>
          </div>
          <div class="flex shrink-0 items-center gap-x-4">
            <div class="hidden sm:flex sm:flex-col sm:items-end">
              <p class="text-md text-zinc-500 uppercase font-display font-bold">
                {{ itemStatusLabel(element) }}
              </p>
              <div
                v-if="element.dl_progress"
                class="mt-1 w-96 bg-zinc-800 rounded-lg overflow-hidden"
              >
                <div
                  class="h-2 bg-blue-600"
                  :style="{ width: `${Math.min(element.dl_progress * 100, 100)}%` }"
                />
              </div>
              <span
                class="mt-2 inline-flex items-center gap-x-1 text-zinc-400 text-sm font-display"
                ><span class="text-zinc-300"
                  >{{ formatKilobytes(Math.min(element.dl_current, element.dl_max) / 1000) }}B</span
                >
                /
                <span class=""
                  >{{ formatKilobytes(element.dl_max / 1000) }}B</span
                ><CloudIcon class="size-5"
              /></span>
              <div
                v-if="element.dl_max !== element.disk_max"
                class="h-[1px] my-2 w-full bg-zinc-700"
              />
              <div
                v-if="
                  element.disk_progress && element.dl_max !== element.disk_max
                "
                class="mt-1 w-96 bg-zinc-800 rounded-lg overflow-hidden"
              >
                <div
                  class="h-2 bg-blue-600"
                  :style="{ width: `${element.disk_progress * 100}%` }"
                />
              </div>
              <span
                v-if="element.dl_max !== element.disk_max"
                class="mt-2 inline-flex items-center gap-x-1 text-zinc-400 text-sm font-display"
                ><span class="text-zinc-300"
                  >{{ formatKilobytes(Math.min(element.disk_current, element.disk_max) / 1000) }}B</span
                >
                /
                <span class=""
                  >{{ formatKilobytes(element.disk_max / 1000) }}B</span
                ><ServerIcon class="size-5"
              /></span>
            </div>
            <button
              @click="() => cancelGame(element.meta)"
              :disabled="isCancelling(element.meta.id)"
              :aria-label="
                isCancelling(element.meta.id)
                  ? 'Cancelling download'
                  : 'Cancel download'
              "
              class="group disabled:cursor-wait"
            >
              <ArrowPathIcon
                v-if="isCancelling(element.meta.id)"
                class="size-8 flex-none text-zinc-400 animate-spin"
              />
              <XMarkIcon
                v-else
                class="transition size-8 flex-none text-zinc-600 group-hover:text-zinc-300"
                aria-hidden="true"
              />
            </button>
          </div>
        </li>
        <p v-else>Loading...</p>
      </template>
    </draggable>
    <div
      class="text-zinc-600 uppercase font-semibold font-display w-full text-center"
      v-if="queue.queue.length == 0"
    >
      No items in the queue
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  ServerIcon,
  XMarkIcon,
  CloudIcon,
  ChevronUpIcon,
  ChevronDownIcon,
  PlayIcon,
  PauseIcon,
  ArrowPathIcon,
} from "@heroicons/vue/20/solid";
import { invoke } from "@tauri-apps/api/core";
import { type DownloadableMetadata, type Game, type GameStatus } from "~/types";

const windowWidth = ref(window.innerWidth);
window.addEventListener("resize", (event) => {
  windowWidth.value = window.innerWidth;
});

const queue = useQueueState();
const stats = useStatsState();
const speedHistory = useDownloadHistory();
const speedHistoryMax = computed(() => windowWidth.value / 4);
const speedMax = computed(() => {
  const hist = speedHistory.value;
  if (hist.length === 0) return 1;
  const max = hist.reduce((a, b) => (a > b ? a : b));
  return Math.max(max, 1) * 1.1;
});
const previousGameId = useState<string | undefined>("previous_game");

const isPaused = computed(() => queue.value?.status === "Paused");

// Optimistic transition state. The backend's drain (download_manager_builder.rs)
// can take a few seconds between the user clicking Pause/Resume and the queue
// status flipping — without local feedback, the UI looks frozen. We set this
// to "pausing"/"resuming" on click and clear it as soon as the queue status
// reaches the expected terminal state. A safety timeout clears it after 8s
// so a stuck backend can't permanently disable the button.
const transitioning = ref<"pausing" | "resuming" | null>(null);
let transitionTimer: ReturnType<typeof setTimeout> | null = null;
function startTransition(kind: "pausing" | "resuming") {
  transitioning.value = kind;
  if (transitionTimer) clearTimeout(transitionTimer);
  transitionTimer = setTimeout(() => {
    transitioning.value = null;
    transitionTimer = null;
  }, 8000);
}
function clearTransitionIfMatches(targetStatus: "Paused" | "Downloading") {
  if (transitioning.value === null) return;
  const expected =
    transitioning.value === "pausing" ? "Paused" : "Downloading";
  if (expected === targetStatus) {
    transitioning.value = null;
    if (transitionTimer) {
      clearTimeout(transitionTimer);
      transitionTimer = null;
    }
  }
}

// Same idea for cancel — between clicking X and the queue item disappearing,
// we want a visible spinner. Track per-meta-id so multiple cancels look right.
const cancelling = ref<Set<string>>(new Set());
function isCancelling(id: string): boolean {
  return cancelling.value.has(id);
}
const cancelTimers = new Map<string, ReturnType<typeof setTimeout>>();

const headerPrimary = computed(() => {
  if (transitioning.value === "pausing") return "Pausing…";
  if (transitioning.value === "resuming") return "Resuming…";
  if (isPaused.value) return "Paused";
  return `${formatKilobytes(stats.value.speed)}B/s`;
});
const headerSecondary = computed(() => {
  if (transitioning.value !== null) return "working on it";
  if (isPaused.value) return "click ▶ to resume";
  return `${formatTime(stats.value.time)} left`;
});

function itemStatusLabel(el: (typeof queue.value.queue)[number]): string {
  if (isCancelling(el.meta.id)) return "Cancelling…";
  if (transitioning.value === "pausing") return "Pausing…";
  if (transitioning.value === "resuming") return "Resuming…";
  return el.status;
}

type ListIterable = { element: (typeof queue.value.queue)[0]; index: number };

const games: Ref<{
  [key: string]: { game: Game; status: Ref<GameStatus>; cover: string };
}> = ref({});

function resetHistoryGraph() {
  speedHistory.value = [];
  stats.value = { time: 0, speed: 0 };
}
function checkReset(v: QueueState) {
  const currentGame = v.queue.at(0)?.meta.id;

  if (!currentGame) {
    if (previousGameId.value) {
      previousGameId.value = undefined;
      resetHistoryGraph();
    }
    return;
  }

  if (!previousGameId.value) {
    previousGameId.value = currentGame;
    resetHistoryGraph();
    return;
  }

  if (currentGame !== previousGameId.value) {
    previousGameId.value = currentGame;
    resetHistoryGraph();
  }
}
watch(queue, (v) => {
  loadGamesForQueue(v);
  checkReset(v);

  // Clear pause/resume transition once the backend agrees.
  if (v?.status === "Paused") clearTransitionIfMatches("Paused");
  else if (v?.status === "Downloading") clearTransitionIfMatches("Downloading");

  // Clear cancel state for any items that have left the queue.
  const liveIds = new Set(v.queue.map((q) => q.meta.id));
  for (const id of cancelling.value) {
    if (!liveIds.has(id)) {
      cancelling.value.delete(id);
      const t = cancelTimers.get(id);
      if (t) {
        clearTimeout(t);
        cancelTimers.delete(id);
      }
    }
  }
});

watch(stats, (v) => {
  if (v.speed == 0) return;
  const newLength = speedHistory.value.push(v.speed);
  if (newLength > speedHistoryMax.value) {
    speedHistory.value.splice(0, newLength - speedHistoryMax.value);
  }
  checkReset(queue.value);
});

function loadGamesForQueue(v: typeof queue.value) {
  for (const {
    meta: { id },
  } of v.queue) {
    if (games.value[id]) continue;
    (async () => {
      const gameData = await useGame(id);
      const cover = await useObject(gameData.game.mCoverObjectId);
      games.value[id] = { ...gameData, cover };
    })();
  }
}

loadGamesForQueue(queue.value);

async function onEnd(event: { oldIndex: number; newIndex: number }) {
  await invoke("move_download_in_queue", {
    oldIndex: event.oldIndex,
    newIndex: event.newIndex,
  });
}

async function reorderDownload(oldIndex: number, newIndex: number) {
  if (newIndex < 0 || newIndex >= queue.value.queue.length) return;
  try {
    await invoke("move_download_in_queue", { oldIndex, newIndex });
  } catch (e) {
    console.error("Failed to reorder download:", e);
  }
}

async function togglePause() {
  if (transitioning.value !== null) return;
  try {
    if (isPaused.value) {
      startTransition("resuming");
      await invoke("resume_downloads");
    } else {
      startTransition("pausing");
      await invoke("pause_downloads");
    }
  } catch (e) {
    transitioning.value = null;
    if (transitionTimer) {
      clearTimeout(transitionTimer);
      transitionTimer = null;
    }
    console.error("Failed to toggle pause:", e);
  }
}

async function cancelGame(meta: DownloadableMetadata) {
  if (cancelling.value.has(meta.id)) return;
  cancelling.value.add(meta.id);
  // Safety: if the queue never updates (backend wedged), drop the spinner
  // after 8s so the user can try again.
  const t = setTimeout(() => {
    cancelling.value.delete(meta.id);
    cancelTimers.delete(meta.id);
  }, 8000);
  cancelTimers.set(meta.id, t);

  try {
    await invoke("cancel_game", { meta });
  } catch (e) {
    cancelling.value.delete(meta.id);
    clearTimeout(t);
    cancelTimers.delete(meta.id);
    console.error("Failed to cancel download:", e);
  }
}

function formatTime(seconds: number): string {
  if (seconds == 0) {
    return `0s`;
  }
  if (seconds < 60) {
    return `${Math.round(seconds)}s`;
  }

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m ${Math.round(seconds % 60)}s`;
  }

  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

onUnmounted(() => {
  if (transitionTimer) clearTimeout(transitionTimer);
  for (const t of cancelTimers.values()) clearTimeout(t);
  cancelTimers.clear();
});
</script>
