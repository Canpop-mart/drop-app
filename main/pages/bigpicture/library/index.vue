<template>
  <div class="flex flex-col h-full">
    <!-- Filter tabs + search -->
    <div
      class="flex items-center gap-2 border-b border-zinc-800/30"
      :class="deck.isDeckMode.value ? 'px-4 py-2' : 'px-8 py-4'"
    >
      <button
        v-for="filter in filters"
        :key="filter.value"
        :ref="
          (el: any) =>
            registerFilter(el, {
              onSelect: () => (activeFilter = filter.value),
            })
        "
        class="rounded-lg font-medium transition-colors"
        :class="[
          deck.isDeckMode.value ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm',
          activeFilter === filter.value
            ? 'bg-blue-600/20 text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50',
        ]"
        @click="activeFilter = filter.value"
      >
        {{ filter.label }}
        <span v-if="filter.count > 0" class="ml-1 text-xs opacity-60">{{
          filter.count
        }}</span>
      </button>

      <div class="flex-1" />

      <button
        class="flex items-center gap-2 rounded-lg font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50 transition-colors"
        :class="
          deck.isDeckMode.value ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm'
        "
        @click="showKeyboard = true"
      >
        <MagnifyingGlassIcon class="size-4" />
        <span v-if="searchQuery">{{ searchQuery }}</span>
        <span v-else class="text-zinc-600">Search...</span>
      </button>
    </div>

    <!-- On-screen keyboard -->
    <BigPictureKeyboard
      :visible="showKeyboard"
      :model-value="searchQuery"
      placeholder="Search your library..."
      @update:model-value="searchQuery = $event"
      @close="showKeyboard = false"
      @submit="showKeyboard = false"
    />

    <!-- Loading state with skeleton grid -->
    <div
      v-if="loading"
      class="flex-1 overflow-y-auto"
      :class="deck.isDeckMode.value ? 'px-4 py-3' : 'px-8 py-6'"
    >
      <div class="grid gap-3" :class="gridCols">
        <div
          v-for="i in 12"
          :key="i"
          class="aspect-[3/4] rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </div>
    </div>

    <!-- Game grid -->
    <div
      v-else
      ref="scrollContainer"
      class="flex-1 overflow-y-auto py-4"
      :class="deck.isDeckMode.value ? 'px-4 py-3' : 'px-8 py-6'"
    >
      <div class="tile-grid grid gap-3" :class="gridCols">
        <div
          v-for="(entry, index) in filteredGames"
          :key="entry.game.id"
          class="game-tile-wrapper"
          :class="{ 'tile-visible': tilesReady }"
          :style="{ transitionDelay: `${Math.min(index * 30, 500)}ms` }"
        >
          <BigPictureGameTile
            :ref="
              (el: any) =>
                registerTile(el, {
                  onSelect: () =>
                    $router.push(`/bigpicture/library/${entry.game.id}`),
                })
            "
            :game="entry.game"
            :status="entry.status"
          />
        </div>
      </div>

      <div
        v-if="filteredGames.length === 0"
        class="flex items-center justify-center py-24"
      >
        <div class="text-center">
          <component
            :is="searchQuery ? MagnifyingGlassIcon : Square3Stack3DIcon"
            class="size-16 mx-auto mb-4 text-zinc-600"
          />
          <h3 class="text-2xl font-semibold text-zinc-400 mb-2">
            {{
              searchQuery
                ? `No games match "${searchQuery}"`
                : activeFilter !== "all"
                  ? `No ${activeFilter === "installed" ? "installed" : "uninstalled"} games`
                  : "No games found"
            }}
          </h3>
          <p class="text-zinc-600 mb-4">
            {{
              searchQuery
                ? "Try a different search term"
                : activeFilter !== "all"
                  ? "Try changing your filter"
                  : "Add games to your library to get started"
            }}
          </p>
          <div class="flex items-center justify-center gap-3">
            <button
              v-if="searchQuery"
              class="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors text-sm font-medium"
              @click="searchQuery = ''"
            >
              <XMarkIcon class="size-4" />
              Clear search
            </button>
            <button
              v-if="activeFilter !== 'all'"
              class="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors text-sm font-medium"
              @click="activeFilter = 'all'"
            >
              <XMarkIcon class="size-4" />
              Show all
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  MagnifyingGlassIcon,
  XMarkIcon,
  Square3Stack3DIcon,
} from "@heroicons/vue/24/outline";
import BigPictureGameTile from "~/components/bigpicture/BigPictureGameTile.vue";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { parseStatus } from "~/composables/game";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useDeckMode } from "~/composables/deck-mode";
import type { Game, GameStatus, Collection, RawGameStatus } from "~/types";

/**
 * Run async tasks with a concurrency limit to avoid request storms.
 */
async function pLimit<T>(
  tasks: (() => Promise<T>)[],
  concurrency: number,
): Promise<T[]> {
  const results: T[] = new Array(tasks.length);
  let nextIndex = 0;

  async function worker() {
    while (nextIndex < tasks.length) {
      const index = nextIndex++;
      results[index] = await tasks[index]();
    }
  }

  const workers = Array.from(
    { length: Math.min(concurrency, tasks.length) },
    () => worker(),
  );
  await Promise.all(workers);
  return results;
}

definePageMeta({ layout: "bigpicture" });

const deck = useDeckMode();

interface LibraryEntry {
  game: Game;
  status: GameStatus;
}

interface FetchLibraryResponse {
  library: Game[];
  collections: Collection[];
  other: Game[];
  missing: Game[];
}

const library: Ref<LibraryEntry[]> = ref([]);
const activeFilter = ref("all");
const searchQuery = ref("");
const showKeyboard = ref(false);
const loading = ref(true);
const tilesReady = ref(false);
const scrollContainer = ref<HTMLElement | null>(null);
const registerTile = useBpFocusableGroup("content");
const registerFilter = useBpFocusableGroup("content");

const gamepad = useGamepad();
// C4 fix: store gamepad unsubscribes for cleanup
const _unsubs: (() => void)[] = [];

_unsubs.push(
  gamepad.onButton(GamepadButton.North, () => {
    showKeyboard.value = !showKeyboard.value;
  }),
);

// Wire LT (LeftTrigger) for page up scroll and RT (RightTrigger) for page down scroll
_unsubs.push(
  gamepad.onButton(GamepadButton.LeftTrigger, () => {
    if (scrollContainer.value) {
      const pageHeight = scrollContainer.value.clientHeight;
      scrollContainer.value.scrollBy({ top: -pageHeight, behavior: "smooth" });
    }
  }),
);

_unsubs.push(
  gamepad.onButton(GamepadButton.RightTrigger, () => {
    if (scrollContainer.value) {
      const pageHeight = scrollContainer.value.clientHeight;
      scrollContainer.value.scrollBy({ top: pageHeight, behavior: "smooth" });
    }
  }),
);

// Deck uses fewer, larger columns; desktop uses more columns
const gridCols = computed(() =>
  deck.isDeckMode.value
    ? "grid-cols-3 sm:grid-cols-4 md:grid-cols-5"
    : "grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7",
);

async function loadLibrary(hardRefresh = false) {
  try {
    const data = await invoke<FetchLibraryResponse>("fetch_library", {
      hardRefresh,
    });

    const seen = new Set<string>();
    const uniqueGames: Game[] = [];

    const allRawGames: Game[] = [
      ...data.library,
      ...data.collections.flatMap((c) => c.entries.map((e) => e.game)),
      ...data.other,
      ...data.missing,
    ];

    // Deduplicate games
    for (const game of allRawGames) {
      if (!seen.has(game.id)) {
        seen.add(game.id);
        uniqueGames.push(game);
      }
    }

    // Fetch game statuses with concurrency limit to avoid request storms
    const statusResults = await pLimit(
      uniqueGames.map((game) => async () => {
        try {
          const statusData: RawGameStatus = await invoke("fetch_game_status", {
            id: game.id,
          });
          return { game, status: parseStatus(statusData) };
        } catch {
          return { game, status: { type: "Remote" } as GameStatus };
        }
      }),
      5, // max 5 concurrent status fetches
    );

    library.value = statusResults.sort((a, b) =>
      a.game.mName.localeCompare(b.game.mName),
    );
  } catch (e) {
    console.error("Failed to fetch library:", e);
  } finally {
    loading.value = false;
    nextTick(() => {
      tilesReady.value = true;
    });
  }
}

// Always hard-refresh on mount so newly-added games (e.g. from store iframe)
// are picked up immediately. The 500ms debounced update_library listener
// handles mid-session changes like downloads completing.
onMounted(() => loadLibrary(true));

// C3 fix: Debounced refresh when library changes — properly store unlisten
let refreshTimeout: ReturnType<typeof setTimeout> | null = null;
let _unlistenLibrary: (() => void) | undefined;

onMounted(async () => {
  _unlistenLibrary = await listen("update_library", () => {
    if (refreshTimeout) clearTimeout(refreshTimeout);
    refreshTimeout = setTimeout(() => loadLibrary(true), 500);
  });
});

onUnmounted(() => {
  // Clean up all gamepad subscriptions
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;

  // Clean up Tauri listener
  _unlistenLibrary?.();

  // Clean up debounce timer
  if (refreshTimeout) {
    clearTimeout(refreshTimeout);
    refreshTimeout = null;
  }
});

const installedCount = computed(
  () => library.value.filter((e) => e.status.type === "Installed").length,
);

const filters = computed(() => [
  { label: "All", value: "all", count: library.value.length },
  { label: "Installed", value: "installed", count: installedCount.value },
  {
    label: "Not Installed",
    value: "remote",
    count: library.value.length - installedCount.value,
  },
]);

const filteredGames = computed(() => {
  let games = library.value;
  if (activeFilter.value === "installed") {
    games = games.filter((e) => e.status.type === "Installed");
  } else if (activeFilter.value === "remote") {
    games = games.filter((e) => e.status.type !== "Installed");
  }
  return games;
});
</script>