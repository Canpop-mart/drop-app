<template>
  <div class="flex flex-col h-full">
    <!-- Filter tabs + search -->
    <div class="flex items-center gap-2 px-8 py-4 border-b border-zinc-800/30">
      <button
        v-for="filter in filters"
        :key="filter.value"
        :ref="
          (el: any) =>
            registerFilter(el, {
              onSelect: () => (activeFilter = filter.value),
            })
        "
        class="px-4 py-2 text-sm rounded-lg font-medium transition-colors"
        :class="[
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

      <!-- Sort indicator (cycled via X button) -->
      <div class="flex items-center gap-1.5 px-3 py-2 text-sm font-medium text-zinc-500">
        <ArrowsUpDownIcon class="size-4" />
        {{ sortLabel }}
      </div>

      <!-- Search -->
      <button
        class="flex items-center gap-2 px-4 py-2 text-sm rounded-lg font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50 transition-colors"
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
    <div v-if="loading" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
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
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
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
                  onSelect: () => {
                    console.log(`[BPM:LIB] Selecting game: ${entry.game.id} (${entry.game.mName})`);
                    focusNav.saveFocusSnapshot(route.path);
                    $router.push(`/bigpicture/library/${entry.game.id}`).then(() => {
                      console.log(`[BPM:LIB] Navigation complete for: ${entry.game.id}`);
                    }).catch((e: any) => {
                      console.error(`[BPM:LIB] Navigation FAILED for ${entry.game.id}:`, e);
                    });
                  },
                  onFocus: () => prefetchGame(entry.game.id),
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
  ArrowsUpDownIcon,
} from "@heroicons/vue/24/outline";
import BigPictureGameTile from "~/components/bigpicture/BigPictureGameTile.vue";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { parseStatus, deduplicatedInvoke } from "~/composables/game";
import { useGamepad, GamepadButton } from "~/composables/gamepad";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useDeckMode } from "~/composables/deck-mode";
import type { Game, GameStatus, Collection, RawGameStatus } from "~/types";

function prefetchGame(gameId: string) {
  deduplicatedInvoke("fetch_game", { gameId }).catch(() => {});
}

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
const focusNav = useFocusNavigation();
const registerTile = useBpFocusableGroup("content");
const registerFilter = useBpFocusableGroup("content");

const gamepad = useGamepad();
const _unsubs: (() => void)[] = [];

// Swap search/sort buttons on Gamescope (Deck reports Y↔X swapped)
const { isGamescope: _isGS } = useDeckMode();
const _searchBtn = _isGS.value ? GamepadButton.West : GamepadButton.North;
const _sortBtn = _isGS.value ? GamepadButton.North : GamepadButton.West;

_unsubs.push(
  gamepad.onButton(_searchBtn, () => {
    showKeyboard.value = !showKeyboard.value;
  }),
);

_unsubs.push(
  gamepad.onButton(_sortBtn, () => {
    cycleSort();
  }),
);

// ── Sort options ────────────────────────────────────────────────────────
type SortMode = "name" | "recent" | "status";
const sortMode = ref<SortMode>("name");

const sortLabels: Record<SortMode, string> = {
  name: "Name",
  recent: "Recent",
  status: "Status",
};
const sortLabel = computed(() => sortLabels[sortMode.value]);

function cycleSort() {
  const modes: SortMode[] = ["name", "recent", "status"];
  const idx = modes.indexOf(sortMode.value);
  sortMode.value = modes[(idx + 1) % modes.length];
}

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

    for (const game of allRawGames) {
      if (!seen.has(game.id)) {
        seen.add(game.id);
        uniqueGames.push(game);
      }
    }

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
      5,
    );

    library.value = statusResults;
  } catch (e) {
    console.error("Failed to fetch library:", e);
  } finally {
    loading.value = false;
    nextTick(() => {
      tilesReady.value = true;
    });
  }
}

const route = useRoute();

onMounted(async () => {
  await loadLibrary(true);
  if (!focusNav.restoreFocusSnapshot(route.path)) {
    focusNav.autoFocusContent("content");
  }
});

let refreshTimeout: ReturnType<typeof setTimeout> | null = null;
let _unlistenLibrary: (() => void) | undefined;

onMounted(async () => {
  _unlistenLibrary = await listen("update_library", () => {
    if (refreshTimeout) clearTimeout(refreshTimeout);
    refreshTimeout = setTimeout(() => loadLibrary(true), 500);
  });
});

onUnmounted(() => {
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;
  _unlistenLibrary?.();
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
  let games = [...library.value];

  // Status filter
  if (activeFilter.value === "installed") {
    games = games.filter((e) => e.status.type === "Installed");
  } else if (activeFilter.value === "remote") {
    games = games.filter((e) => e.status.type !== "Installed");
  }

  // Search filter
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.trim().toLowerCase();
    games = games.filter((e) => e.game.mName.toLowerCase().includes(q));
  }

  // Sort
  switch (sortMode.value) {
    case "name":
      games.sort((a, b) => a.game.mName.localeCompare(b.game.mName));
      break;
    case "status":
      // Installed first, then running, then remote
      const statusOrder: Record<string, number> = {
        Running: 0,
        Downloading: 1,
        Installed: 2,
        Remote: 3,
        Queued: 4,
      };
      games.sort(
        (a, b) =>
          (statusOrder[a.status.type] ?? 99) -
          (statusOrder[b.status.type] ?? 99),
      );
      break;
    case "recent":
      // Keep server order (most recently added first)
      games.reverse();
      break;
  }

  return games;
});
</script>
