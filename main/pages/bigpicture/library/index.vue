<template>
  <div class="flex flex-col h-full" :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }">
    <!-- Filter tabs + search -->
    <div class="flex items-center gap-2 px-8 py-4 border-b" :style="{ borderColor: 'var(--bpm-border)' }">
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

      <!-- Sort/filter summary -->
      <div class="flex items-center gap-2 px-3 py-2 text-sm text-zinc-500">
        <ArrowsUpDownIcon class="size-4" />
        <span>{{ sortLabel }}</span>
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

    <!-- Sort & Filter overlay -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200"
        leave-active-class="transition-opacity duration-200"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="showFilterMenu"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        >
          <div class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-2xl w-full mx-4">
            <h2 class="text-xl font-semibold font-display text-zinc-100 mb-5">Sort & Filter</h2>

            <div class="grid grid-cols-2 gap-6">
              <!-- Sort section -->
              <div>
                <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">Sort By</p>
                <div class="space-y-1.5">
                  <button
                    v-for="(label, key) in sortLabels"
                    :key="key"
                    class="w-full flex items-center justify-between px-4 py-3 rounded-xl text-sm transition-colors"
                    :class="sortMode === key
                      ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                      : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                    :ref="(el: any) => registerOverlay(el, { onSelect: () => { sortMode = key as SortMode; } })"
                    @click="sortMode = key as SortMode"
                  >
                    <span class="font-medium">{{ label }}</span>
                    <span v-if="sortMode === key" class="text-xs opacity-75">Active</span>
                  </button>
                </div>
              </div>

              <!-- Filter section -->
              <div>
                <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">Show</p>
                <div class="space-y-1.5">
                  <button
                    v-for="f in filters"
                    :key="f.value"
                    class="w-full flex items-center justify-between px-4 py-3 rounded-xl text-sm transition-colors"
                    :class="activeFilter === f.value
                      ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                      : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                    :ref="(el: any) => registerOverlay(el, { onSelect: () => { activeFilter = f.value; } })"
                    @click="activeFilter = f.value"
                  >
                    <span class="font-medium">{{ f.label }}</span>
                    <span class="text-xs opacity-75">{{ f.count }}</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- Close -->
            <button
              :ref="(el: any) => registerOverlay(el, { onSelect: () => { showFilterMenu = false; focusNav.unrestrictFocus('content'); } })"
              class="w-full mt-5 px-4 py-3 rounded-xl text-sm font-medium bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700 transition-colors"
              @click="showFilterMenu = false; focusNav.unrestrictFocus('content')"
            >
              Done
            </button>
          </div>
        </div>
      </Transition>
    </Teleport>

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
    <!-- ═══ Shelves view ═══ -->
    <div
      v-if="activeFilter === 'shelves'"
      class="flex-1 overflow-y-auto px-8 py-6"
      data-bp-scroll
    >
      <!-- Create shelf button -->
      <div class="flex items-center justify-between mb-6">
        <h2 class="text-lg font-semibold font-display" style="color: var(--bpm-text)">Your Shelves</h2>
        <button
          v-if="!showNewShelfInput"
          :ref="(el: any) => registerTile(el, { onSelect: () => (showNewShelfInput = true) })"
          class="px-4 py-2 text-sm font-medium rounded-lg transition-colors"
          style="background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)"
          @click="showNewShelfInput = true"
        >
          + New Shelf
        </button>
      </div>

      <!-- New shelf name input with on-screen keyboard -->
      <BigPictureKeyboard
        :visible="showNewShelfInput"
        :model-value="newShelfName"
        placeholder="Enter shelf name..."
        @update:model-value="newShelfName = $event"
        @close="showNewShelfInput = false"
        @submit="createNewShelf"
      />
      <div v-if="showNewShelfInput && newShelfName" class="flex items-center gap-3 mb-6">
        <span class="text-sm" style="color: var(--bpm-text)">Creating shelf: <strong>{{ newShelfName }}</strong></span>
      </div>

      <!-- Shelf rows -->
      <div v-if="shelvesData.shelves.value.length > 0" class="space-y-8">
        <div v-for="shelf in shelvesData.shelves.value" :key="shelf.id">
          <!-- Shelf header -->
          <div class="flex items-center justify-between mb-3">
            <h3 class="text-base font-semibold" style="color: var(--bpm-text)">
              {{ shelf.name }}
              <span class="text-xs font-normal ml-2" style="color: var(--bpm-muted)">
                {{ shelf.entries.length }} game{{ shelf.entries.length !== 1 ? 's' : '' }}
              </span>
            </h3>
            <div class="flex items-center gap-2">
              <button
                :ref="(el: any) => registerTile(el, { onSelect: () => shelvesData.toggleShelfVisibility(shelf.id, !shelf.isPublic) })"
                class="px-3 py-1 text-xs rounded-lg transition-colors"
                :style="{ color: shelf.isPublic ? 'var(--bpm-accent-hex)' : 'var(--bpm-muted)' }"
                @click="shelvesData.toggleShelfVisibility(shelf.id, !shelf.isPublic)"
              >
                {{ shelf.isPublic ? 'Public' : 'Private' }}
              </button>
              <button
                :ref="(el: any) => registerTile(el, { onSelect: () => shelvesData.deleteShelf(shelf.id) })"
                class="px-3 py-1 text-xs rounded-lg transition-colors"
                style="color: var(--bpm-muted)"
                @click="shelvesData.deleteShelf(shelf.id)"
              >
                Delete
              </button>
            </div>
          </div>

          <!-- Horizontal scroll row of games -->
          <div v-if="shelf.entries.length > 0" class="flex gap-4 overflow-x-auto pb-4 px-1 pt-1" style="scrollbar-width: thin">
            <div
              v-for="entry in shelf.entries"
              :key="entry.gameId"
              class="flex-shrink-0 group"
              style="width: 11rem"
            >
              <div
                class="relative cursor-pointer rounded-lg overflow-hidden transition-transform hover:scale-105"
                style="aspect-ratio: 2/3"
                :ref="(el: any) => registerTile(el, {
                  onSelect: () => {
                    focusNav.saveFocusSnapshot(route.path);
                    $router.push(`/bigpicture/library/${entry.gameId}`);
                  },
                })"
              >
                <img
                  v-if="entry.game.mCoverObjectId"
                  :src="useObject(entry.game.mCoverObjectId)"
                  :alt="entry.game.mName"
                  class="w-full h-full object-cover"
                />
                <div
                  v-else
                  class="w-full h-full flex items-center justify-center text-2xl font-bold"
                  style="background-color: var(--bpm-surface); color: var(--bpm-accent-hex)"
                >
                  {{ entry.game.mName.charAt(0) }}
                </div>
                <!-- Remove button on hover -->
                <button
                  class="absolute top-1 right-1 size-6 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                  style="background-color: rgba(0,0,0,0.7); color: #fff; font-size: 0.7rem"
                  @click.stop="shelvesData.removeFromShelf(shelf.id, entry.gameId)"
                >
                  ✕
                </button>
              </div>
              <p class="text-xs mt-1.5 truncate" style="color: var(--bpm-text)">
                {{ entry.game.mName }}
              </p>
            </div>
          </div>

          <!-- Empty shelf -->
          <div v-else class="py-6 text-center rounded-lg" style="background-color: var(--bpm-surface)">
            <p class="text-sm" style="color: var(--bpm-muted)">
              No games on this shelf yet. Add games from their detail page.
            </p>
          </div>
        </div>
      </div>

      <!-- No shelves at all -->
      <div v-else-if="!shelvesData.loading.value" class="flex items-center justify-center py-24">
        <div class="text-center">
          <svg class="size-16 mx-auto mb-4" style="color: var(--bpm-muted)" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 6.878V6a2.25 2.25 0 012.25-2.25h7.5A2.25 2.25 0 0118 6v.878m-12 0c.235-.083.487-.128.75-.128h10.5c.263 0 .515.045.75.128m-12 0A2.25 2.25 0 004.5 9v.878m13.5-3A2.25 2.25 0 0119.5 9v.878m0 0a2.246 2.246 0 00-.75-.128H5.25c-.263 0-.515.045-.75.128m15 0A2.25 2.25 0 0121 12v6a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 18v-6c0-1.011.672-1.866 1.594-2.144" />
          </svg>
          <h3 class="text-xl font-semibold mb-2" style="color: var(--bpm-text)">No shelves yet</h3>
          <p class="text-sm mb-4" style="color: var(--bpm-muted)">Create a shelf to organize your games into categories</p>
        </div>
      </div>
    </div>

    <!-- ═══ Normal game grid view ═══ -->
    <div
      v-else
      ref="scrollContainer"
      class="flex-1 overflow-y-auto px-8 py-6"
      data-bp-scroll
    >
      <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
        <div
          v-for="(entry, index) in filteredGames"
          :key="entry.game.id"
          class="game-tile-wrapper"
          :class="{ 'tile-visible': tilesReady }"
          :style="{ transitionDelay: `${Math.min(index * 30, 500)}ms` }"
        >
          <div class="relative">
            <BigPictureGameTile
              :ref="
                (el: any) =>
                  registerTile(el, {
                    onSelect: () => {
                      if (multiSelectMode) {
                        toggleSelect(entry.game.id);
                        return;
                      }
                      console.log(`[BPM:LIB] Selecting game: ${entry.game.id} (${entry.game.mName})`);
                      focusNav.saveFocusSnapshot(route.path);
                      $router.push(`/bigpicture/library/${entry.game.id}`).then(() => {
                        console.log(`[BPM:LIB] Navigation complete for: ${entry.game.id}`);
                      }).catch((e: any) => {
                        console.error(`[BPM:LIB] Navigation FAILED for ${entry.game.id}:`, e);
                      });
                    },
                    onContext: () => openContextMenu(entry),
                    onFocus: () => prefetchGame(entry.game.id),
                  })
              "
              :game="entry.game"
              :status="entry.status"
              :hide-titles="hideTitles"
            />
            <!-- Multi-select checkbox -->
            <div
              v-if="multiSelectMode"
              class="absolute top-2 left-2 z-20 size-6 rounded-md flex items-center justify-center transition-colors"
              :class="selectedGames.has(entry.game.id) ? 'bg-blue-500' : 'bg-zinc-800/80 ring-1 ring-zinc-600'"
            >
              <svg v-if="selectedGames.has(entry.game.id)" class="size-4 text-white" fill="none" stroke="currentColor" stroke-width="3" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
              </svg>
            </div>
          </div>
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

    <!-- ═══ Context menu overlay ═══ -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-150"
        leave-active-class="transition-opacity duration-100"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div v-if="contextMenuGame" class="fixed inset-0 z-[200] flex items-center justify-center">
          <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" @click="closeContextMenu" />
          <div
            class="relative z-10 rounded-2xl p-6 w-80 max-w-[90vw]"
            style="background-color: var(--bpm-surface); color: var(--bpm-text)"
          >
            <!-- Game info header -->
            <div class="flex items-center gap-4 mb-5 pb-4 border-b" style="border-color: var(--bpm-border)">
              <img
                v-if="contextMenuGame.game.mCoverObjectId"
                :src="objectUrl(contextMenuGame.game.mCoverObjectId)"
                class="size-14 rounded-lg object-cover"
              />
              <div class="flex-1 min-w-0">
                <p class="font-semibold text-sm truncate">{{ contextMenuGame.game.mName }}</p>
                <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">{{ contextMenuGame.status.type }}</p>
              </div>
            </div>

            <!-- Actions -->
            <div class="space-y-1">
              <button
                v-for="item in contextMenuActions"
                :key="item.id"
                :ref="(el: any) => registerCtxMenu(el, { onSelect: item.action })"
                class="w-full flex items-center gap-3 px-4 py-3 text-sm rounded-xl transition-colors hover:bg-white/5"
                @click="item.action"
              >
                <component :is="item.icon" class="size-5 flex-shrink-0" :style="{ color: item.color }" />
                <span>{{ item.label }}</span>
              </button>
            </div>

            <!-- Multi-select actions (when in select mode) -->
            <div v-if="multiSelectMode && selectedGames.size > 0" class="mt-4 pt-4 border-t" style="border-color: var(--bpm-border)">
              <p class="text-xs mb-3" style="color: var(--bpm-muted)">{{ selectedGames.size }} game{{ selectedGames.size !== 1 ? 's' : '' }} selected</p>
              <div class="flex gap-2">
                <button
                  :ref="(el: any) => registerCtxMenu(el, { onSelect: bulkUninstall })"
                  class="flex-1 px-3 py-2 text-xs rounded-lg bg-red-900/20 text-red-400 hover:bg-red-900/30 transition-colors"
                  @click="bulkUninstall"
                >
                  Uninstall Selected
                </button>
                <button
                  :ref="(el: any) => registerCtxMenu(el, { onSelect: clearSelection })"
                  class="flex-1 px-3 py-2 text-xs rounded-lg transition-colors hover:bg-white/5"
                  style="color: var(--bpm-muted)"
                  @click="clearSelection"
                >
                  Clear Selection
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Multi-select toolbar -->
    <div
      v-if="multiSelectMode"
      class="fixed bottom-0 left-0 right-0 z-[100] flex items-center justify-between px-8 py-4"
      style="background: linear-gradient(to top, var(--bpm-bg), transparent)"
    >
      <div class="flex items-center gap-3">
        <span class="text-sm font-medium" style="color: var(--bpm-text)">
          {{ selectedGames.size }} selected
        </span>
        <button
          class="px-3 py-1.5 text-xs rounded-lg transition-colors hover:bg-white/10"
          style="color: var(--bpm-muted)"
          @click="selectAll"
        >
          Select All
        </button>
      </div>
      <div class="flex items-center gap-2">
        <button
          v-if="selectedGames.size > 0"
          class="px-4 py-2 text-sm rounded-lg bg-red-600 text-white hover:bg-red-500 transition-colors"
          @click="bulkUninstall"
        >
          Uninstall ({{ selectedGames.size }})
        </button>
        <button
          class="px-4 py-2 text-sm rounded-lg transition-colors hover:bg-white/10"
          style="color: var(--bpm-muted)"
          @click="exitMultiSelect"
        >
          Cancel
        </button>
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
  FolderIcon,
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
const showFilterMenu = ref(false);
const loading = ref(true);
const tilesReady = ref(false);
const hideTitles = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:hideTitles") === "true"
    : false,
);
const scrollContainer = ref<HTMLElement | null>(null);
const focusNav = useFocusNavigation();
const registerTile = useBpFocusableGroup("content");
const registerFilter = useBpFocusableGroup("content");
const registerOverlay = useBpFocusableGroup("sort-overlay");

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
    if (showKeyboard.value) return;
    if (contextMenuGame.value) return; // context menu handles its own close
    if (focusNav.contextHandled.value) return; // tile onContext handled it
    if (showFilterMenu.value) {
      showFilterMenu.value = false;
      focusNav.unrestrictFocus("content");
    } else {
      showFilterMenu.value = true;
      focusNav.restrictFocus("sort-overlay");
    }
  }),
);
_unsubs.push(
  gamepad.onButton(GamepadButton.East, () => {
    if (showFilterMenu.value) {
      showFilterMenu.value = false;
      focusNav.unrestrictFocus("content");
    }
  }),
);

// ── Sort options ────────────────────────────────────────────────────────
type SortMode = "name" | "recent" | "status" | "size";
const sortMode = ref<SortMode>("name");

const sortLabels: Record<SortMode, string> = {
  name: "Name",
  recent: "Recent",
  status: "Status",
  size: "Size",
};
const sortLabel = computed(() => sortLabels[sortMode.value]);

// ── Size cache for "size" sort ──────────────────────────────────────────────
const gameSizes = ref<Record<string, number>>({});
const sizesLoading = ref(false);

async function loadGameSizes() {
  if (sizesLoading.value) return;
  sizesLoading.value = true;
  try {
    const installed = library.value.filter(e => e.status.type === "Installed");
    for (const entry of installed) {
      if (gameSizes.value[entry.game.id] != null) continue;
      try {
        const size = await invoke<number>("get_install_size", { gameId: entry.game.id });
        gameSizes.value[entry.game.id] = size;
      } catch {
        gameSizes.value[entry.game.id] = 0;
      }
    }
  } finally {
    sizesLoading.value = false;
  }
}

watch(sortMode, (mode) => {
  if (mode === "size") loadGameSizes();
});

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

    // Batch-fetch all statuses in a single IPC call instead of N individual ones
    const ids = uniqueGames.map((g) => g.id);
    const statusMap = new Map<string, GameStatus>();
    try {
      const batchResults: [string, RawGameStatus][] = await invoke("fetch_game_statuses", { ids });
      for (const [id, raw] of batchResults) {
        try { statusMap.set(id, parseStatus(raw)); } catch { /* skip bad status */ }
      }
    } catch {
      // Fallback: if batch command not available, fetch individually
      const results = await pLimit(
        uniqueGames.map((game) => async () => {
          try {
            const statusData: RawGameStatus = await invoke("fetch_game_status", { id: game.id });
            statusMap.set(game.id, parseStatus(statusData));
          } catch { /* skip */ }
        }),
        15,
      );
    }

    library.value = uniqueGames.map((game) => ({
      game,
      status: statusMap.get(game.id) ?? ({ type: "Remote" } as GameStatus),
    }));
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
  { label: "Shelves", value: "shelves", count: shelvesData.shelves.value.length },
]);

// ── Shelves ─────────────────────────────────────────────────────────────
const shelvesData = useShelves();
const showNewShelfInput = ref(false);
const newShelfName = ref("");

async function createNewShelf() {
  const name = newShelfName.value.trim();
  if (!name) return;
  await shelvesData.createShelf(name);
  newShelfName.value = "";
  showNewShelfInput.value = false;
}

// Load shelves when the page mounts
onMounted(() => {
  shelvesData.fetchShelves();
});

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
    case "size":
      // Largest first; uninstalled games (size 0) go to the end
      games.sort((a, b) => {
        const sA = gameSizes.value[a.game.id] ?? 0;
        const sB = gameSizes.value[b.game.id] ?? 0;
        return sB - sA;
      });
      break;
  }

  return games;
});

// ── Context menu ─────────────────────────────────────────────────────────
import {
  PlayIcon,
  ArrowDownTrayIcon,
  TrashIcon,
  QueueListIcon,
  CheckCircleIcon,
  StopIcon,
} from "@heroicons/vue/24/solid";

const contextMenuGame = ref<LibraryEntry | null>(null);
const registerCtxMenu = useBpFocusableGroup("context-menu");

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

import { serverUrl } from "~/composables/use-server-fetch";

function openContextMenu(entry: LibraryEntry) {
  contextMenuGame.value = entry;
  nextTick(() => {
    focusNav.restrictFocus("context-menu");
  });
}

function closeContextMenu() {
  contextMenuGame.value = null;
  focusNav.unrestrictFocus("content");
}

// Close context menu on B button
_unsubs.push(
  gamepad.onButton(GamepadButton.East, () => {
    if (contextMenuGame.value) {
      closeContextMenu();
    }
  }),
);

const contextMenuActions = computed(() => {
  const entry = contextMenuGame.value;
  if (!entry) return [];

  const actions: { id: string; label: string; icon: any; color: string; action: () => void }[] = [];

  if (entry.status.type === "Installed") {
    actions.push({
      id: "play",
      label: "Play",
      icon: PlayIcon,
      color: "#3b82f6",
      action: async () => {
        closeContextMenu();
        try {
          await invoke("launch_game", { id: entry.game.id });
        } catch (e) {
          console.error("[BPM:LIB] Launch failed:", e);
        }
      },
    });
  } else if (entry.status.type === "Running") {
    actions.push({
      id: "stop",
      label: "Stop",
      icon: StopIcon,
      color: "#ef4444",
      action: async () => {
        closeContextMenu();
        try {
          await invoke("kill_game", { id: entry.game.id });
        } catch (e) {
          console.error("[BPM:LIB] Kill failed:", e);
        }
      },
    });
  } else if (entry.status.type === "Remote") {
    actions.push({
      id: "install",
      label: "Install",
      icon: ArrowDownTrayIcon,
      color: "#22c55e",
      action: async () => {
        closeContextMenu();
        try {
          await invoke("download_game", { id: entry.game.id });
        } catch (e) {
          console.error("[BPM:LIB] Download failed:", e);
        }
      },
    });
  }

  // View details — always available
  actions.push({
    id: "details",
    label: "View Details",
    icon: Square3Stack3DIcon,
    color: "var(--bpm-muted)",
    action: () => {
      closeContextMenu();
      focusNav.saveFocusSnapshot(route.path);
      $router.push(`/bigpicture/library/${entry.game.id}`);
    },
  });

  // Add to shelf
  actions.push({
    id: "shelf",
    label: "Add to Shelf...",
    icon: FolderIcon,
    color: "var(--bpm-muted)",
    action: () => {
      closeContextMenu();
      focusNav.saveFocusSnapshot(route.path);
      $router.push(`/bigpicture/library/${entry.game.id}`);
    },
  });

  // Multi-select
  actions.push({
    id: "select",
    label: multiSelectMode.value ? "Toggle Selection" : "Select Multiple",
    icon: CheckCircleIcon,
    color: "#3b82f6",
    action: () => {
      if (!multiSelectMode.value) {
        multiSelectMode.value = true;
        selectedGames.value.add(entry.game.id);
      } else {
        toggleSelect(entry.game.id);
      }
      closeContextMenu();
    },
  });

  // Uninstall — only for installed
  if (entry.status.type === "Installed") {
    actions.push({
      id: "uninstall",
      label: "Uninstall",
      icon: TrashIcon,
      color: "#ef4444",
      action: async () => {
        closeContextMenu();
        try {
          await invoke("uninstall_game", { id: entry.game.id });
          loadLibrary(true);
        } catch (e) {
          console.error("[BPM:LIB] Uninstall failed:", e);
        }
      },
    });
  }

  return actions;
});

const $router = useRouter();

// ── Multi-select ─────────────────────────────────────────────────────────
const multiSelectMode = ref(false);
const selectedGames = ref<Set<string>>(new Set());

function toggleSelect(gameId: string) {
  const set = new Set(selectedGames.value);
  if (set.has(gameId)) {
    set.delete(gameId);
  } else {
    set.add(gameId);
  }
  selectedGames.value = set;
  // Auto-exit if nothing selected
  if (set.size === 0) multiSelectMode.value = false;
}

function selectAll() {
  const set = new Set<string>();
  for (const entry of filteredGames.value) {
    set.add(entry.game.id);
  }
  selectedGames.value = set;
}

function clearSelection() {
  selectedGames.value = new Set();
  multiSelectMode.value = false;
  closeContextMenu();
}

function exitMultiSelect() {
  selectedGames.value = new Set();
  multiSelectMode.value = false;
}

async function bulkUninstall() {
  closeContextMenu();
  const ids = [...selectedGames.value];
  const installed = ids.filter((id) =>
    library.value.find((e) => e.game.id === id && e.status.type === "Installed"),
  );
  for (const id of installed) {
    try {
      await invoke("uninstall_game", { id });
    } catch (e) {
      console.error(`[BPM:LIB] Bulk uninstall failed for ${id}:`, e);
    }
  }
  selectedGames.value = new Set();
  multiSelectMode.value = false;
  loadLibrary(true);
}
</script>
