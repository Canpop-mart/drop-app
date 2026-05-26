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

        <!-- Density toggle — cover (default) vs compact list. Power-user
             affordance for large libraries; persists for the session
             only so a casual click can't change the default view across
             reloads. -->
        <div
          class="flex items-center rounded-md bg-zinc-800/50 ring-1 ring-zinc-700/40 p-0.5"
        >
          <button
            v-for="opt in densityOptions"
            :key="opt.value"
            class="rounded-[5px] p-1.5 transition-colors"
            :class="
              density === opt.value
                ? 'bg-zinc-700 text-zinc-100'
                : 'text-zinc-500 hover:text-zinc-300'
            "
            :title="opt.label"
            :aria-label="opt.label"
            @click="density = opt.value"
          >
            <component :is="opt.icon" class="size-4" />
          </button>
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

      <!-- Quick-filter chip row. Surfaces the highest-traffic filters
           (Install state, Updates, Recently played) as one-click chips
           instead of forcing the user into the drawer. The drawer
           still owns the deeper knobs (collections, type, sort). -->
      <div class="flex flex-wrap gap-1.5 mt-3 items-center">
        <button
          v-for="chip in quickFilterChips"
          :key="chip.value"
          class="inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-medium transition-colors"
          :class="
            chip.active
              ? 'bg-blue-500/20 text-blue-200 ring-1 ring-blue-500/40'
              : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800'
          "
          @click="applyQuickFilter(chip.value)"
        >
          {{ chip.label }}
          <span
            v-if="chip.count !== null"
            class="text-[10px] tabular-nums"
            :class="chip.active ? 'text-blue-300/80' : 'text-zinc-500'"
          >
            {{ chip.count }}
          </span>
        </button>

        <span
          v-if="activeFilterChips.length > 0"
          class="mx-2 h-3.5 w-px bg-zinc-700"
        />

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
          v-if="hasActiveAdvancedFilters"
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

    <!-- Populated library. Sections-mode renders Continue Playing →
         Recently played → Installed → Not installed; filter-mode
         collapses to a single flat result list so users get a clean
         answer to their query. -->
    <div
      v-else
      class="flex-1 px-8 xl:px-12 py-6 space-y-10 pb-12"
    >
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
          :compact="density === 'compact'"
          :last-played-map="lastPlayedMap"
          :show-hover-action="density !== 'compact'"
          @select="goToGame"
        />
        <p v-else class="text-sm text-zinc-500">
          No games match those filters.
        </p>
      </section>

      <template v-else>
        <!-- "Continue playing" hero — the most-recently-played game,
             rendered loud at the top so resuming is always one click
             away. Skipped when there's no play history. -->
        <section v-if="continuePlaying">
          <div
            class="relative overflow-hidden rounded-2xl bg-gradient-to-r from-blue-900/40 via-zinc-900/60 to-purple-900/30 ring-1 ring-blue-500/20 hover:ring-blue-500/40 transition-colors cursor-pointer group"
            @click="goToGame(continuePlaying.entry.game.id)"
          >
            <!-- Soft banner backdrop. The hero stands on its own dark
                 gradient even without a banner image; we layer the
                 banner over it at low opacity so the card still reads
                 as "this game" without overwhelming the type. -->
            <img
              v-if="continuePlaying.entry.game.mBannerObjectId"
              :src="useObject(continuePlaying.entry.game.mBannerObjectId)"
              :alt="continuePlaying.entry.game.mName"
              class="absolute inset-0 w-full h-full object-cover opacity-25 group-hover:opacity-35 transition-opacity"
            />
            <div
              class="absolute inset-0 bg-gradient-to-r from-zinc-950 via-zinc-950/85 to-zinc-950/30"
            />

            <div class="relative flex items-center gap-5 sm:gap-6 p-5 sm:p-6">
              <!-- Cover thumbnail anchor — uses the existing GameTile
                   fallback when no cover is available so this surface
                   never renders a broken-looking gray box. -->
              <div
                class="shrink-0 w-20 h-28 sm:w-24 sm:h-32 rounded-xl overflow-hidden ring-1 ring-zinc-700/60 bg-zinc-900"
              >
                <img
                  v-if="continuePlaying.entry.game.mCoverObjectId"
                  :src="useObject(continuePlaying.entry.game.mCoverObjectId)"
                  :alt="continuePlaying.entry.game.mName"
                  class="w-full h-full object-cover"
                />
                <div
                  v-else
                  class="w-full h-full flex items-center justify-center text-4xl font-display font-bold text-zinc-100/90"
                >
                  {{ continuePlaying.entry.game.mName.charAt(0).toUpperCase() }}
                </div>
              </div>

              <div class="flex-1 min-w-0">
                <p
                  class="text-[10px] tracking-[0.2em] uppercase text-blue-300/80 font-medium mb-1"
                >
                  Continue playing
                </p>
                <h2
                  class="text-2xl sm:text-3xl font-display font-bold text-zinc-100 leading-tight truncate"
                >
                  {{ continuePlaying.entry.game.mName }}
                </h2>
                <p class="text-sm text-zinc-400 mt-1 truncate">
                  Last played
                  {{ formatRelativeTime(continuePlaying.recent.lastPlayedAt) }}
                  <template
                    v-if="continuePlaying.recent.totalPlaytimeSeconds > 0"
                  >
                    · {{
                      formatPlaytime(continuePlaying.recent.totalPlaytimeSeconds)
                    }}
                    total
                  </template>
                  <span
                    v-if="continuePlaying.entry.updateAvailable"
                    class="ml-2 inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-bold uppercase bg-blue-500/30 text-blue-200"
                  >
                    Update
                  </span>
                </p>
              </div>

              <button
                class="shrink-0 inline-flex items-center gap-2 rounded-lg px-4 sm:px-5 py-2.5 sm:py-3 text-sm font-semibold transition-colors shadow-lg"
                :class="
                  continuePlaying.entry.installed
                    ? 'bg-blue-600 hover:bg-blue-500 text-white shadow-blue-600/30'
                    : 'bg-zinc-800/80 hover:bg-zinc-700 text-zinc-100 ring-1 ring-zinc-600/40 shadow-zinc-900/40'
                "
                @click.stop="goToGame(continuePlaying.entry.game.id)"
              >
                <svg
                  v-if="continuePlaying.entry.installed"
                  class="size-4"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path d="M8 5v14l11-7z" />
                </svg>
                <svg
                  v-else
                  class="size-4"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path
                    d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"
                  />
                </svg>
                {{ continuePlaying.entry.installed ? "Play" : "Install" }}
              </button>
            </div>
          </div>
        </section>

        <!-- Recently played — single horizontal-scroll row of tiles
             behind Continue Playing. Hidden entirely when there's <2
             recents (the hero already shows the only one). The shelf
             is capped at RECENT_SHELF_MAX so it stays a "lately"
             surface, not a third grid. Arrow buttons page by ~80% of
             the visible width and disable at the edges. -->
        <section v-if="recentShelfEntries.length > 0">
          <div class="flex items-baseline justify-between mb-4">
            <div class="flex items-baseline gap-3">
              <h2 class="text-lg font-display font-semibold text-zinc-100">
                Recently played
              </h2>
              <span class="text-xs text-zinc-500 tabular-nums">
                {{ recentShelfEntries.length }}
              </span>
            </div>
            <div class="flex items-center gap-1.5">
              <button
                class="rounded-md p-1.5 transition-colors"
                :class="
                  canScrollRecentLeft
                    ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
                    : 'bg-zinc-900/40 text-zinc-700 cursor-not-allowed'
                "
                :disabled="!canScrollRecentLeft"
                aria-label="Scroll recently played left"
                @click="scrollRecent(-1)"
              >
                <ChevronLeftIcon class="size-4" />
              </button>
              <button
                class="rounded-md p-1.5 transition-colors"
                :class="
                  canScrollRecentRight
                    ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
                    : 'bg-zinc-900/40 text-zinc-700 cursor-not-allowed'
                "
                :disabled="!canScrollRecentRight"
                aria-label="Scroll recently played right"
                @click="scrollRecent(1)"
              >
                <ChevronRightIcon class="size-4" />
              </button>
            </div>
          </div>
          <div
            ref="recentScrollEl"
            class="flex gap-3 overflow-x-auto pb-2 -mx-1 px-1 recent-scroll-row"
            @scroll="updateRecentScrollState"
          >
            <div
              v-for="entry in recentShelfEntries"
              :key="entry.game.id"
              class="shrink-0 w-[150px]"
            >
              <GameTile
                :cover-url="
                  entry.game.mCoverObjectId
                    ? useObject(entry.game.mCoverObjectId)
                    : null
                "
                :name="entry.game.mName"
                :installed="entry.installed"
                :update-available="entry.updateAvailable"
                :last-played="lastPlayedMap.get(entry.game.id) ?? null"
                :hover-action="entry.installed ? 'play' : 'install'"
                @select="goToGame(entry.game.id)"
              />
            </div>
          </div>
        </section>

        <!-- Installed — the main interaction surface. Empty state guides
             toward installing something when nothing's installed yet. -->
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
              <span class="text-xs text-zinc-500 tabular-nums">
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
              :compact="density === 'compact'"
              :last-played-map="lastPlayedMap"
              :show-hover-action="density !== 'compact'"
              @select="goToGame"
            />
            <p v-else class="text-sm text-zinc-500 italic py-3">
              No installed games yet — pick one below to install.
            </p>
          </div>
        </section>

        <!-- Not installed — collapsible to keep the page focused on the
             installed games. -->
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
            :compact="density === 'compact'"
            :last-played-map="lastPlayedMap"
            :show-hover-action="density !== 'compact'"
            @select="goToGame"
          />
        </section>
      </template>
    </div>

    <!-- Batch compat tester — gated behind dev mode. -->
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
        <div
          class="sticky top-0 bg-zinc-900 border-b border-zinc-800 px-5 py-4 flex items-center justify-between"
        >
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

        <div
          class="sticky bottom-0 bg-zinc-900 border-t border-zinc-800 px-5 py-3 flex items-center gap-2"
        >
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
  ChevronLeftIcon,
  ChevronRightIcon,
  AdjustmentsHorizontalIcon,
  XMarkIcon,
  Squares2X2Icon,
  Bars3Icon,
} from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import { useGame } from "~/composables/game";
import { useShelves } from "~/composables/shelves";
import {
  useServerApi,
  type RecentPlaytimeEntry,
} from "~/composables/use-server-api";
import type { Game, GameStatus } from "~/types";
import { InstalledType } from "~/types";
import LibraryGrid from "~/components/LibraryGrid.vue";
import GameTile from "~/components/GameTile.vue";

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
const api = useServerApi();
const { shelves, fetchShelves } = useShelves();

const entries = ref<LibraryEntry[]>([]);
const recentPlaytime = ref<RecentPlaytimeEntry[]>([]);
const loading = ref(true);
const searchInput = ref("");

// Section collapse state — both default open.
const showInstalled = ref(true);
const showNotInstalled = ref(true);

// Layout density — large covers (default) vs compact rows. Session-
// scoped: a single click on the toggle changes the layout immediately,
// but doesn't persist to settings (avoids a permanent flip from a
// stray click).
const density = ref<"cover" | "compact">("cover");
const densityOptions = [
  { label: "Cover view", value: "cover" as const, icon: Squares2X2Icon },
  { label: "Compact list", value: "compact" as const, icon: Bars3Icon },
];

// ── Filter state ──────────────────────────────────────────────────────────
//
// Everything here is local-only — we already have the full library entry
// list in memory, so filters are just predicates on that array.
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

// gameId → ISO timestamp for the last play session. Built once from the
// recent-playtime endpoint and threaded through to LibraryGrid so each
// tile can render its own "Played X ago" line.
const lastPlayedMap = computed(() => {
  const map = new Map<string, string>();
  for (const r of recentPlaytime.value) {
    map.set(r.gameId, r.lastPlayedAt);
  }
  return map;
});

// Library entries that have a recent-playtime record, joined back to
// the entry list and ordered by `lastPlayedAt` desc. This is the source
// of truth for both the Continue Playing hero (entry 0) and the
// Recently Played shelf (entries 1..N).
const recentEntries = computed<
  Array<{ entry: LibraryEntry; recent: RecentPlaytimeEntry }>
>(() => {
  const byId = new Map(entries.value.map((e) => [e.game.id, e]));
  const out: Array<{ entry: LibraryEntry; recent: RecentPlaytimeEntry }> = [];
  for (const r of recentPlaytime.value) {
    const entry = byId.get(r.gameId);
    if (entry) out.push({ entry, recent: r });
  }
  return out;
});

const continuePlaying = computed(() => recentEntries.value[0] ?? null);

// Recently-played shelf — capped at 15 entries (the recent endpoint
// returns up to 20 distinct games; the first goes to the hero, so we
// take the next 15 for the scroll row). Anything beyond that is more
// than a "lately" surface should carry.
const RECENT_SHELF_MAX = 15;
const recentShelfEntries = computed<LibraryEntry[]>(() =>
  recentEntries.value.slice(1, 1 + RECENT_SHELF_MAX).map((r) => r.entry),
);

// Horizontal-scroll state for the Recently Played shelf. The arrows in
// the section header use these to (a) decide whether to disable and
// (b) drive the scrollBy call.
const recentScrollEl = ref<HTMLElement | null>(null);
const recentScrollLeft = ref(0);
const recentScrollMax = ref(0);

function updateRecentScrollState() {
  const el = recentScrollEl.value;
  if (!el) return;
  recentScrollLeft.value = el.scrollLeft;
  recentScrollMax.value = Math.max(0, el.scrollWidth - el.clientWidth);
}

const canScrollRecentLeft = computed(() => recentScrollLeft.value > 4);
const canScrollRecentRight = computed(
  () => recentScrollLeft.value < recentScrollMax.value - 4,
);

function scrollRecent(direction: -1 | 1) {
  const el = recentScrollEl.value;
  if (!el) return;
  // Scroll by ~80% of the visible width so the user keeps a sliver of
  // context (last tile or two) overlapping between pages.
  const delta = direction * Math.round(el.clientWidth * 0.8);
  el.scrollBy({ left: delta, behavior: "smooth" });
}

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

// Drives the badge on the "Filters" button. The quick-filter chip row
// counts toward this too — `installStateFilter !== 'all'` covers both
// the chip and the drawer's install-state radio.
const activeAdvancedFilterCount = computed(() => {
  let n = 0;
  if (installStateFilter.value !== "all") n += 1;
  if (typeFilter.value !== "all") n += 1;
  if (sortOrder.value !== "name-asc") n += 1;
  n += selectedCollectionIds.value.length;
  return n;
});

const hasActiveAdvancedFilters = computed(
  () => activeAdvancedFilterCount.value > 0,
);

// True if the user has narrowed in any way (filter or text search).
const filterMode = computed<"sections" | "flat">(() =>
  searchInput.value.trim() || activeAdvancedFilterCount.value > 0
    ? "flat"
    : "sections",
);

// Quick-filter chip row. Trimmed to the two highest-traffic filters:
// `All` (default / clear-all) and `Installed`. The drawer's broader
// state options (Not installed, Updates) stay there — they're useful
// but don't earn a permanent slot in the chip row. "Recently played"
// is its own section already; a chip would be redundant.
type QuickFilter =
  | { label: string; value: "all"; count: null; active: boolean }
  | {
      label: string;
      value: "installed";
      count: number;
      active: boolean;
    };

const quickFilterChips = computed<QuickFilter[]>(() => [
  {
    label: "All",
    value: "all",
    count: null,
    active:
      installStateFilter.value === "all" && !searchInput.value.trim(),
  },
  {
    label: "Installed",
    value: "installed",
    count: installedCount.value,
    active: installStateFilter.value === "installed",
  },
]);

function applyQuickFilter(value: QuickFilter["value"]) {
  if (value === "all") {
    clearFilters();
    return;
  }
  // Toggle off if the user clicks the active chip again — gives them a
  // one-click escape back to the default view without having to find
  // the "All" chip.
  if (installStateFilter.value === value) {
    installStateFilter.value = "all";
    return;
  }
  installStateFilter.value = value;
}

// Apply all filters to the library.
const displayedEntries = computed<LibraryEntry[]>(() => {
  const q = searchInput.value.trim().toLowerCase();
  const wantInstalled = installStateFilter.value === "installed";
  const wantNotInstalled = installStateFilter.value === "not-installed";
  const wantUpdates = installStateFilter.value === "updates";
  const wantGame = typeFilter.value === "game";
  const wantTool = typeFilter.value === "tool";

  // Build the per-game collection membership lookup once per derivation.
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

  if (sortOrder.value === "name-desc") {
    return [...filtered].sort((a, b) =>
      b.game.mName.localeCompare(a.game.mName),
    );
  }
  return filtered;
});

// Sections mode derives from the unfiltered list.
const installedEntries = computed(() =>
  entries.value.filter((e) => e.installed),
);
const notInstalledEntries = computed(() =>
  entries.value.filter((e) => !e.installed),
);

// Filter chips — one per active filter for one-click removal. The
// quick-filter chip row above already covers install state, so we
// only emit chips for the deeper-drawer filters here to avoid duplication.
type FilterChip =
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

function formatRelativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  if (!Number.isFinite(then)) return "";
  const diffMs = Date.now() - then;
  const seconds = Math.max(0, Math.floor(diffMs / 1000));
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days >= 30) {
    const months = Math.floor(days / 30);
    return `${months} month${months === 1 ? "" : "s"} ago`;
  }
  if (days >= 2) return `${days} days ago`;
  if (days === 1) return "yesterday";
  if (hours >= 1) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  if (minutes >= 1) return `${minutes} min ago`;
  return "just now";
}

function formatPlaytime(totalSeconds: number): string {
  const hours = totalSeconds / 3600;
  if (hours >= 1) {
    const rounded = Math.round(hours * 10) / 10;
    return `${rounded} hour${rounded === 1 ? "" : "s"}`;
  }
  const minutes = Math.max(1, Math.round(totalSeconds / 60));
  return `${minutes} min`;
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
    built.sort((a, b) => a.game.mName.localeCompare(b.game.mName));
    entries.value = built;
  } catch (e) {
    console.warn("[library] fetch failed:", e);
    entries.value = [];
  } finally {
    loading.value = false;
  }
}

async function loadRecentPlaytime() {
  try {
    recentPlaytime.value = await api.playtime.recent();
  } catch (e) {
    // Soft-fail — the hero/shelf hide automatically when the list is
    // empty, so an offline server doesn't blank the whole page.
    console.warn("[library] recent playtime fetch failed:", e);
    recentPlaytime.value = [];
  }
}

onMounted(() => {
  load();
  loadRecentPlaytime();
  // Shelves drive the collection filter; non-fatal if it fails.
  fetchShelves().catch((e) =>
    console.warn("[library] shelves fetch failed:", e),
  );
});

// Re-measure the Recently Played scroll bounds whenever the shelf is
// re-populated (initial fetch, density toggle changes that affect the
// tile width, or play history mutates).  Without this the right-arrow
// stays disabled on first render even when overflow exists, because
// the `scroll` event hasn't fired yet.
watch(
  () => recentShelfEntries.value.length,
  () => {
    nextTick(updateRecentScrollState);
  },
);
watch(density, () => {
  nextTick(updateRecentScrollState);
});
</script>

<style scoped>
/* Hide the native horizontal scrollbar on the Recently Played row.
   The arrow buttons in the section header are the canonical control;
   trackpad / wheel swipes still work, the bar just isn't visible. */
.recent-scroll-row {
  scrollbar-width: none;
}
.recent-scroll-row::-webkit-scrollbar {
  display: none;
}
</style>
