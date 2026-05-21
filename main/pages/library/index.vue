<template>
  <div class="h-full flex flex-col overflow-y-auto">
    <!-- Header row — title, count, search, collections. -->
    <div
      class="sticky top-0 z-10 bg-zinc-950/80 backdrop-blur-lg px-8 xl:px-12 pt-6 pb-4 border-b border-zinc-800/40"
    >
      <div class="flex items-end gap-6 flex-wrap">
        <div>
          <h1 class="text-2xl font-display font-bold text-zinc-100">
            Library
          </h1>
          <p class="mt-1 text-sm text-zinc-500">
            {{ totalCount }} game{{ totalCount === 1 ? "" : "s" }}
            <span v-if="installedCount > 0">
              ·
              <span class="text-green-500">{{ installedCount }} installed</span>
            </span>
            <span v-if="recentEntries.length > 0">
              · {{ recentEntries.length }} recently played
            </span>
          </p>
        </div>

        <div class="flex-1" />

        <div class="relative">
          <MagnifyingGlassIcon
            class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-zinc-500 pointer-events-none"
          />
          <input
            v-model="searchInput"
            type="text"
            placeholder="Filter library..."
            class="rounded-lg border border-zinc-700 bg-zinc-800/50 pl-9 pr-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:bg-zinc-800 focus:ring-2 focus:ring-blue-500 outline-none transition-colors w-72"
          />
        </div>

        <button
          class="inline-flex items-center gap-x-2 rounded-md px-4 py-2 text-sm font-semibold transition-colors"
          :class="
            activeAdvancedFilterCount > 0
              ? 'bg-blue-600/20 text-blue-300 ring-1 ring-blue-500/40'
              : 'bg-zinc-800/50 text-zinc-200 hover:bg-zinc-800'
          "
          @click="filterDrawerOpen = true"
        >
          <AdjustmentsHorizontalIcon class="size-4" />
          Filters
          <span
            v-if="activeAdvancedFilterCount > 0"
            class="ml-1 rounded-full bg-blue-500 text-white text-[10px] font-bold px-1.5 leading-4"
          >
            {{ activeAdvancedFilterCount }}
          </span>
        </button>

        <NuxtLink
          to="/library/collections"
          class="inline-flex items-center gap-x-2 rounded-md bg-zinc-800/50 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-800 transition-colors"
        >
          <Square3Stack3DIcon class="size-4" />
          Collections
        </NuxtLink>
      </div>

      <!-- Active filter chips. Mirrors the store page — quick-remove
           handles so the user doesn't have to re-open the drawer to back
           out of a single filter. -->
      <div
        v-if="activeFilterChips.length > 0"
        class="flex flex-wrap gap-1.5 mt-3"
      >
        <button
          v-for="chip in activeFilterChips"
          :key="chip.key"
          class="inline-flex items-center gap-1 rounded-full bg-blue-500/15 text-blue-300 ring-1 ring-blue-500/30 px-2.5 py-1 text-xs hover:bg-blue-500/25 transition-colors"
          @click="removeFilterChip(chip)"
        >
          <span class="text-blue-400/70">{{ chip.label }}:</span>
          <span>{{ chip.value }}</span>
          <XMarkIcon class="size-3" />
        </button>
        <button
          class="text-xs text-zinc-500 hover:text-zinc-300 underline self-center ml-2"
          @click="clearFilters"
        >
          Clear all
        </button>
      </div>
    </div>

    <!-- Loading / empty states. -->
    <div
      v-if="loading"
      class="flex-1 flex items-center justify-center text-sm text-zinc-500"
    >
      Loading your library...
    </div>

    <div
      v-else-if="entries.length === 0"
      class="flex-1 flex flex-col items-center justify-center text-center px-8"
    >
      <div class="rounded-2xl bg-zinc-800/50 p-6 mb-4">
        <RocketLaunchIcon class="size-10 text-zinc-500" />
      </div>
      <p class="text-sm text-zinc-400 max-w-md">
        Your library is empty. Visit the
        <NuxtLink to="/store" class="text-blue-400 hover:text-blue-300">
          store
        </NuxtLink>
        to add games.
      </p>
    </div>

    <!-- Populated library — three sections, each a tile grid. The "Recently
         played" row floats above the install-state buckets so resuming a
         game in progress is always one click away regardless of install
         state. -->
    <div
      v-else
      class="flex-1 px-8 xl:px-12 py-6 space-y-10 pb-12"
    >
      <!-- Search-active OR filter-active mode: hide section headers and
           show flat results so the user gets a clean answer to their query
           without scrolling past "Installed (0)" empty buckets. -->
      <section v-if="filterMode === 'flat'">
        <p class="text-xs uppercase tracking-widest text-zinc-500 mb-4">
          {{ displayedEntries.length }} result{{
            displayedEntries.length === 1 ? "" : "s"
          }}
          <template v-if="searchInput.trim()">
            for "{{ searchInput.trim() }}"
          </template>
        </p>
        <LibraryGrid
          v-if="displayedEntries.length > 0"
          :entries="displayedEntries"
          @select="goToGame"
        />
        <p v-else class="text-sm text-zinc-500">
          No games match those filters.
        </p>
      </section>

      <template v-else>
        <!-- Recently played — shows games with playtime > 0, sorted by
             last-played desc (relies on the server stats endpoint; we
             only display what we have locally). -->
        <section v-if="recentEntries.length > 0">
          <div class="flex items-baseline justify-between mb-4">
            <h2 class="text-lg font-display font-semibold text-zinc-100">
              Recently played
            </h2>
            <span class="text-xs text-zinc-500 tabular-nums">
              {{ recentEntries.length }}
            </span>
          </div>
          <LibraryGrid :entries="recentEntries" @select="goToGame" />
        </section>

        <!-- Installed — always expanded; this is the section users
             interact with most often. Empty state guides toward installing
             something when nothing's installed yet. -->
        <section>
          <button
            class="w-full flex items-baseline justify-between mb-4 group"
            @click="showInstalled = !showInstalled"
          >
            <div class="flex items-baseline gap-3">
              <h2
                class="text-lg font-display font-semibold text-zinc-100 group-hover:text-blue-400 transition-colors"
              >
                Installed
              </h2>
              <span
                class="text-xs text-zinc-500 tabular-nums"
              >
                {{ installedEntries.length }}
              </span>
            </div>
            <ChevronDownIcon
              class="size-4 text-zinc-500 transition-transform"
              :class="{ '-rotate-90': !showInstalled }"
            />
          </button>
          <div v-show="showInstalled">
            <LibraryGrid
              v-if="installedEntries.length > 0"
              :entries="installedEntries"
              @select="goToGame"
            />
            <p
              v-else
              class="text-sm text-zinc-500 italic py-3"
            >
              No installed games yet — pick one below to install.
            </p>
          </div>
        </section>

        <!-- Not installed — collapsible to keep the page focused on the
             installed games. The bulk of libraries live here; hiding it
             by default would make the page feel emptier than it is. -->
        <section v-if="notInstalledEntries.length > 0">
          <button
            class="w-full flex items-baseline justify-between mb-4 group"
            @click="showNotInstalled = !showNotInstalled"
          >
            <div class="flex items-baseline gap-3">
              <h2
                class="text-lg font-display font-semibold text-zinc-100 group-hover:text-blue-400 transition-colors"
              >
                Not installed
              </h2>
              <span class="text-xs text-zinc-500 tabular-nums">
                {{ notInstalledEntries.length }}
              </span>
            </div>
            <ChevronDownIcon
              class="size-4 text-zinc-500 transition-transform"
              :class="{ '-rotate-90': !showNotInstalled }"
            />
          </button>
          <LibraryGrid
            v-show="showNotInstalled"
            :entries="notInstalledEntries"
            @select="goToGame"
          />
        </section>
      </template>
    </div>

    <!-- Batch compat tester — gated behind dev mode. Stays as a footer
         under the grid so power users can run it without leaving the
         library. -->
    <CompatBatchPanel
      v-if="devMode.enabled.value"
      class="mx-8 xl:mx-12 mb-8"
    />

    <!-- Filter drawer — same pattern as the store page, surfacing the
         knobs that don't fit in the header. Library filters operate on
         locally cached entry data so the result updates the second the
         user clicks a checkbox. -->
    <Transition
      enter-active-class="transition-opacity duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition-opacity duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="filterDrawerOpen"
        class="fixed inset-0 bg-zinc-950/60 backdrop-blur-sm z-40"
        @click="filterDrawerOpen = false"
      />
    </Transition>
    <Transition
      enter-active-class="transition-transform duration-200 ease-out"
      enter-from-class="translate-x-full"
      enter-to-class="translate-x-0"
      leave-active-class="transition-transform duration-150 ease-in"
      leave-from-class="translate-x-0"
      leave-to-class="translate-x-full"
    >
      <aside
        v-if="filterDrawerOpen"
        class="fixed top-0 right-0 bottom-0 w-96 bg-zinc-900 border-l border-zinc-800 z-50 overflow-y-auto"
      >
        <div class="sticky top-0 bg-zinc-900 border-b border-zinc-800 px-5 py-4 flex items-center justify-between">
          <h3 class="text-base font-display font-semibold text-zinc-100">
            Filters
          </h3>
          <button
            class="rounded-md p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
            @click="filterDrawerOpen = false"
          >
            <XMarkIcon class="size-5" />
          </button>
        </div>

        <div class="px-5 py-4 space-y-6 text-sm">
          <!-- Install state — three-way pick. Default is "all" so we don't
               accidentally hide most of the library when the user opens
               the drawer for the first time. -->
          <section>
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Install state
            </h4>
            <div class="grid grid-cols-2 gap-1.5">
              <button
                v-for="opt in installStateOptions"
                :key="opt.value"
                class="px-2 py-1.5 rounded-md text-xs font-medium transition-colors"
                :class="
                  installStateFilter === opt.value
                    ? 'bg-blue-600 text-white'
                    : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200'
                "
                @click="installStateFilter = opt.value"
              >
                {{ opt.label }}
              </button>
            </div>
          </section>

          <!-- Type filter — useful because Drop's library includes Tools
               and Redistributables alongside Games. Hide them when the
               user just wants to see games. -->
          <section>
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Type
            </h4>
            <div class="grid grid-cols-3 gap-1.5">
              <button
                v-for="opt in typeOptions"
                :key="opt.value"
                class="px-2 py-1.5 rounded-md text-xs font-medium transition-colors"
                :class="
                  typeFilter === opt.value
                    ? 'bg-blue-600 text-white'
                    : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200'
                "
                @click="typeFilter = opt.value"
              >
                {{ opt.label }}
              </button>
            </div>
          </section>

          <!-- Sort — default A-Z (already done by the base load) but the
               user might want Z-A or to keep server order. -->
          <section>
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Sort
            </h4>
            <select
              v-model="sortOrder"
              class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-3 py-1.5 text-xs text-zinc-100 focus:ring-2 focus:ring-blue-500 outline-none"
            >
              <option value="name-asc">Name (A → Z)</option>
              <option value="name-desc">Name (Z → A)</option>
            </select>
          </section>

          <!-- Collection multi-select — show only games in the selected
               collections. Loaded from the shelves composable. -->
          <section v-if="shelves.length > 0">
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Collections
            </h4>
            <div class="max-h-48 overflow-y-auto pr-1 space-y-1">
              <label
                v-for="shelf in shelves"
                :key="shelf.id"
                class="flex items-center gap-2 px-2 py-1 rounded hover:bg-zinc-800/60 cursor-pointer"
              >
                <input
                  type="checkbox"
                  :checked="selectedCollectionIds.includes(shelf.id)"
                  class="size-3.5 rounded bg-zinc-800 border-zinc-700 text-blue-500 focus:ring-blue-500 focus:ring-offset-0"
                  @change="toggleCollection(shelf.id)"
                />
                <span class="text-xs text-zinc-300 truncate">
                  {{ shelf.name }}
                </span>
                <span class="ml-auto text-[10px] text-zinc-500">
                  {{ shelf.entries.length }}
                </span>
              </label>
            </div>
          </section>
        </div>

        <div class="sticky bottom-0 bg-zinc-900 border-t border-zinc-800 px-5 py-3 flex items-center gap-2">
          <button
            class="flex-1 rounded-md bg-zinc-800 px-3 py-2 text-xs font-semibold text-zinc-300 hover:bg-zinc-700 transition-colors"
            @click="clearFilters"
          >
            Reset
          </button>
          <button
            class="flex-1 rounded-md bg-blue-600 px-3 py-2 text-xs font-semibold text-white hover:bg-blue-500 transition-colors"
            @click="filterDrawerOpen = false"
          >
            Done
          </button>
        </div>
      </aside>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import {
  MagnifyingGlassIcon,
  RocketLaunchIcon,
  Square3Stack3DIcon,
  ChevronDownIcon,
  AdjustmentsHorizontalIcon,
  XMarkIcon,
} from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import { useGame } from "~/composables/game";
import { useShelves } from "~/composables/shelves";
import type { Game, GameStatus } from "~/types";
import { InstalledType } from "~/types";
import LibraryGrid from "~/components/LibraryGrid.vue";

interface LibraryEntry {
  game: Game;
  status: GameStatus | null;
  installed: boolean;
  updateAvailable: boolean;
}

type FetchLibraryResponse = {
  library: Game[];
  collections: Array<{ entries: Array<{ game: Game }> }>;
  other: Game[];
  missing: Game[];
};

const devMode = useDevMode();
const router = useRouter();
const { shelves, fetchShelves } = useShelves();

const entries = ref<LibraryEntry[]>([]);
const loading = ref(true);
const searchInput = ref("");

// Section collapse state — both default open. The user toggles them when
// they want to focus, but the default view should show everything they
// have so the library feels populated.
const showInstalled = ref(true);
const showNotInstalled = ref(true);

// ── Filter state ──────────────────────────────────────────────────────────
//
// Everything here is local-only — we already have the full library entry
// list in memory, so filters are just predicates on that array. Server
// round-trips aren't needed and shouldn't be added: the library is the
// "stuff I already have" surface and should feel instantaneous.
const filterDrawerOpen = ref(false);
const installStateFilter = ref<
  "all" | "installed" | "not-installed" | "updates"
>("all");
const typeFilter = ref<"all" | "game" | "tool">("all");
const sortOrder = ref<"name-asc" | "name-desc">("name-asc");
const selectedCollectionIds = ref<string[]>([]);

const installStateOptions = [
  { label: "All", value: "all" as const },
  { label: "Installed", value: "installed" as const },
  { label: "Not installed", value: "not-installed" as const },
  { label: "Updates", value: "updates" as const },
];

const typeOptions = [
  { label: "All", value: "all" as const },
  { label: "Games", value: "game" as const },
  { label: "Tools", value: "tool" as const },
];

const totalCount = computed(() => entries.value.length);
const installedCount = computed(() =>
  entries.value.filter((e) => e.installed).length,
);

// Recently played: stub for now — the playtime/recent endpoint would
// populate this. Empty array hides the row entirely.
//
// TODO once we wire `/api/v1/client/playtime/recent` into this page, set
// recentEntries to the first 6 results that also exist in `entries`.
const recentEntries = computed<LibraryEntry[]>(() => []);

function toggleCollection(id: string) {
  const i = selectedCollectionIds.value.indexOf(id);
  if (i === -1) selectedCollectionIds.value.push(id);
  else selectedCollectionIds.value.splice(i, 1);
}

function clearFilters() {
  searchInput.value = "";
  installStateFilter.value = "all";
  typeFilter.value = "all";
  sortOrder.value = "name-asc";
  selectedCollectionIds.value = [];
}

// Count of filters narrowing the library beyond the default view. Drives
// the badge on the "Filters" button so the user can see at a glance
// whether they're looking at a filtered slice.
const activeAdvancedFilterCount = computed(() => {
  let n = 0;
  if (installStateFilter.value !== "all") n += 1;
  if (typeFilter.value !== "all") n += 1;
  if (sortOrder.value !== "name-asc") n += 1;
  n += selectedCollectionIds.value.length;
  return n;
});

// True if the user has narrowed in any way (filter or text search). When
// true we drop the three-section layout and show a flat result list so
// the user gets a clean answer to their query.
const filterMode = computed<"sections" | "flat">(() =>
  searchInput.value.trim() || activeAdvancedFilterCount.value > 0
    ? "flat"
    : "sections",
);

// Apply all filters to the library. Order matters only for performance;
// cheap predicates first.
const displayedEntries = computed<LibraryEntry[]>(() => {
  const q = searchInput.value.trim().toLowerCase();
  const wantInstalled = installStateFilter.value === "installed";
  const wantNotInstalled = installStateFilter.value === "not-installed";
  const wantUpdates = installStateFilter.value === "updates";
  const wantGame = typeFilter.value === "game";
  const wantTool = typeFilter.value === "tool";

  // Build the per-game collection membership lookup once per derivation.
  // `selectedCollectionIds` empty means "no collection filter" — skip
  // building the set entirely.
  let collectionGameIds: Set<string> | null = null;
  if (selectedCollectionIds.value.length > 0) {
    collectionGameIds = new Set();
    for (const shelf of shelves.value) {
      if (selectedCollectionIds.value.includes(shelf.id)) {
        for (const entry of shelf.entries) {
          collectionGameIds.add(entry.gameId);
        }
      }
    }
  }

  const filtered = entries.value.filter((e) => {
    if (q && !e.game.mName.toLowerCase().includes(q)) return false;
    if (wantInstalled && !e.installed) return false;
    if (wantNotInstalled && e.installed) return false;
    if (wantUpdates && !e.updateAvailable) return false;
    if (wantGame && e.game.type !== "Game") return false;
    if (wantTool && e.game.type === "Game") return false;
    if (collectionGameIds && !collectionGameIds.has(e.game.id)) return false;
    return true;
  });

  // Sort. Default A-Z is already applied by `load()` but a Z-A toggle
  // needs to reverse here so the sections-mode default stays cheap.
  if (sortOrder.value === "name-desc") {
    return [...filtered].sort((a, b) => b.game.mName.localeCompare(a.game.mName));
  }
  return filtered;
});

// Sections mode derives from the unfiltered list (because the filter
// chips force `flat` mode anyway). Keeping the original computed shape
// here so the template doesn't have to branch.
const installedEntries = computed(() =>
  entries.value.filter((e) => e.installed),
);
const notInstalledEntries = computed(() =>
  entries.value.filter((e) => !e.installed),
);

// Filter chips — one per active filter for one-click removal.
type FilterChip =
  | { key: string; kind: "install"; label: string; value: string }
  | { key: string; kind: "type"; label: string; value: string }
  | { key: string; kind: "sort"; label: string; value: string }
  | {
      key: string;
      kind: "collection";
      collectionId: string;
      label: string;
      value: string;
    };

const activeFilterChips = computed<FilterChip[]>(() => {
  const chips: FilterChip[] = [];
  if (installStateFilter.value !== "all") {
    const labels = {
      installed: "Installed",
      "not-installed": "Not installed",
      updates: "Has updates",
    } as const;
    chips.push({
      key: "install",
      kind: "install",
      label: "Show",
      value: labels[installStateFilter.value as keyof typeof labels],
    });
  }
  if (typeFilter.value !== "all") {
    chips.push({
      key: "type",
      kind: "type",
      label: "Type",
      value: typeFilter.value === "game" ? "Games" : "Tools",
    });
  }
  if (sortOrder.value !== "name-asc") {
    chips.push({
      key: "sort",
      kind: "sort",
      label: "Sort",
      value: "Z → A",
    });
  }
  for (const id of selectedCollectionIds.value) {
    const shelf = shelves.value.find((s) => s.id === id);
    chips.push({
      key: `collection:${id}`,
      kind: "collection",
      collectionId: id,
      label: "Collection",
      value: shelf?.name ?? id,
    });
  }
  return chips;
});

function removeFilterChip(chip: FilterChip) {
  switch (chip.kind) {
    case "install":
      installStateFilter.value = "all";
      return;
    case "type":
      typeFilter.value = "all";
      return;
    case "sort":
      sortOrder.value = "name-asc";
      return;
    case "collection":
      toggleCollection(chip.collectionId);
      return;
  }
}

function goToGame(gameId: string) {
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/library/${gameId}`);
}

async function load() {
  loading.value = true;
  try {
    const lib = await invoke<FetchLibraryResponse>("fetch_library", {
      hardRefresh: false,
    });
    const allGames: Game[] = [
      ...lib.library,
      ...lib.collections.flatMap((c) => c.entries.map((e) => e.game)),
      ...lib.other,
    ].filter((g, i, a) => a.findIndex((x) => x.id === g.id) === i);

    const built: LibraryEntry[] = [];
    const batchSize = 5;
    for (let i = 0; i < allGames.length; i += batchSize) {
      const batch = allGames.slice(i, i + batchSize);
      const results = await Promise.all(
        batch.map((g) => useGame(g.id).catch(() => null)),
      );
      for (let j = 0; j < batch.length; j++) {
        const r = results[j];
        const game = batch[j];
        if (!r) {
          built.push({
            game,
            status: null,
            installed: false,
            updateAvailable: false,
          });
          continue;
        }
        const status = r.status.value;
        const installed =
          status.type === "Installed" &&
          status.install_type.type === InstalledType.Installed;
        const updateAvailable =
          status.type === "Installed" ? status.update_available : false;
        built.push({ game, status, installed, updateAvailable });
      }
    }
    // Sort within each bucket: A→Z. (Cross-bucket sort happens via the
    // computed splits above.)
    built.sort((a, b) => a.game.mName.localeCompare(b.game.mName));
    entries.value = built;
  } catch (e) {
    console.warn("[library] fetch failed:", e);
    entries.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  load();
  // Shelves drive the collection filter; non-fatal if it fails.
  fetchShelves().catch((e) =>
    console.warn("[library] shelves fetch failed:", e),
  );
});
</script>
