<template>
  <div class="h-full flex flex-col overflow-y-auto">
    <!-- Calm header — title, search, Filters, Collections. Density and the
         quick/active-filter chip rows moved into the drawer so the home feels
         uncluttered. -->
    <div
      class="sticky top-0 z-10 bg-zinc-950/80 backdrop-blur-lg px-8 xl:px-12 pt-6 pb-4 border-b border-zinc-800/40"
    >
      <div class="flex items-center gap-4 flex-wrap">
        <div>
          <h1 class="text-2xl font-display font-bold text-zinc-100">Library</h1>
          <p class="mt-1 text-sm text-zinc-500">
            {{ totalCount }} game{{ totalCount === 1 ? "" : "s" }}
            <span v-if="installedCount > 0">
              ·
              <span class="text-green-500">{{ installedCount }} installed</span>
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
            placeholder="Search your library..."
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
      </div>

      <!-- One quiet line while filtering, instead of the old chip rows. -->
      <div v-if="filterMode === 'flat'" class="mt-2 text-xs text-zinc-500">
        Showing filtered results
        <button
          class="ml-2 text-zinc-400 hover:text-zinc-200 underline"
          @click="clearFilters"
        >
          Clear
        </button>
      </div>
    </div>

    <!-- Loading / empty states. The skeleton mirrors the sections layout
         below (hero, one shelf, the grid) so the page doesn't visibly
         re-flow when the real data lands, and it waits out the ~180ms
         threshold so a cached library never flashes a placeholder. -->
    <div
      v-if="showSkeleton"
      class="flex-1 px-8 xl:px-12 py-8 space-y-12 pb-16"
    >
      <div class="h-64 w-full rounded-2xl bg-zinc-800/50 animate-pulse" />
      <div class="space-y-4">
        <div class="h-4 w-40 rounded bg-zinc-800/50 animate-pulse" />
        <div class="flex gap-4">
          <div
            v-for="i in 6"
            :key="i"
            class="h-52 w-36 shrink-0 rounded-xl bg-zinc-800/50 animate-pulse"
          />
        </div>
      </div>
      <div class="space-y-4">
        <div class="h-4 w-28 rounded bg-zinc-800/50 animate-pulse" />
        <div
          class="grid gap-4 grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-8"
        >
          <div
            v-for="i in 16"
            :key="i"
            class="aspect-[3/4] rounded-xl bg-zinc-800/50 animate-pulse"
          />
        </div>
      </div>
    </div>

    <!-- Populated. Sections-mode is the calm home: hero → a few shelves →
         one All games grid. Filter-mode collapses to a single flat result
         list so a search gets a clean answer. A failed library no longer
         short-circuits this whole block: the sections that have their own
         data source still render, and the grid says what went wrong. -->
    <div v-else-if="!loading" class="flex-1 px-8 xl:px-12 py-8 space-y-12 pb-16">
      <section v-if="filterMode === 'flat'">
        <p class="text-xs uppercase tracking-widest text-zinc-500 mb-4">
          {{ displayedEntries.length }} result{{
            displayedEntries.length === 1 ? "" : "s"
          }}
          <template v-if="searchInput.trim()">
            for "{{ searchInput.trim() }}"
          </template>
        </p>
        <SectionError
          v-if="libraryError"
          title="Couldn't load your library"
          detail="Nothing can be searched until it loads."
          @retry="retryLibrary"
        />
        <LibraryGrid
          v-else-if="displayedEntries.length > 0"
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
        <!-- Continue playing — a tall, cinematic banner hero. Driven by the
             playtime feed, not the library, so a library outage downgrades it
             to cover art rather than removing it. -->
        <section v-if="recentError">
          <SectionError
            title="Couldn't load what you were playing"
            @retry="loadRecentPlaytime"
          />
        </section>
        <section v-else-if="continuePlaying">
          <div
            class="group relative flex min-h-[300px] cursor-pointer items-end overflow-hidden rounded-3xl ring-1 ring-zinc-800/60"
            @click="goToGame(heroGameId)"
          >
            <img
              v-if="heroImageUrl"
              :src="heroImageUrl"
              :alt="heroName"
              class="absolute inset-0 h-full w-full object-cover transition-transform duration-[1200ms] ease-out group-hover:scale-105"
            />
            <div
              v-else
              class="absolute inset-0 bg-gradient-to-br from-blue-900/50 to-zinc-900/60"
            />
            <!-- Legibility gradients: dark from the bottom + the left. -->
            <div
              class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent"
            />
            <div
              class="absolute inset-0 bg-gradient-to-r from-zinc-950/80 via-zinc-950/10 to-transparent"
            />

            <div
              class="relative flex w-full flex-wrap items-end justify-between gap-5 p-8 sm:p-10"
            >
              <div class="min-w-0">
                <p
                  class="mb-2 text-[11px] font-semibold uppercase tracking-[0.25em] text-blue-300"
                >
                  Continue playing
                </p>
                <h2
                  class="truncate font-display text-4xl font-bold leading-none text-white drop-shadow-lg sm:text-5xl"
                >
                  {{ heroName }}
                </h2>
                <p class="mt-3 text-sm text-zinc-300">
                  Last played
                  {{ formatRelativeTime(continuePlaying.recent.lastPlayedAt) }}
                  <template
                    v-if="continuePlaying.recent.totalPlaytimeSeconds > 0"
                  >
                    ·
                    {{
                      formatPlaytime(continuePlaying.recent.totalPlaytimeSeconds)
                    }}
                    total
                  </template>
                  <span
                    v-if="continuePlaying.entry?.updateAvailable"
                    class="ml-2 inline-flex items-center rounded bg-blue-500/30 px-1.5 py-0.5 text-[10px] font-bold uppercase text-blue-200"
                  >
                    Update
                  </span>
                </p>
              </div>

              <!-- Without a library entry we don't know whether the game is
                   installed, so the action drops to a neutral "Open" rather
                   than guessing between Play and Install. -->
              <button
                class="inline-flex shrink-0 items-center gap-2 rounded-xl px-7 py-3.5 text-base font-semibold shadow-lg transition-colors"
                :class="
                  continuePlaying.entry?.installed
                    ? 'bg-blue-600 text-white shadow-blue-600/30 hover:bg-blue-500'
                    : 'bg-white/10 text-white ring-1 ring-white/25 backdrop-blur-sm hover:bg-white/20'
                "
                @click.stop="goToGame(heroGameId)"
              >
                <svg
                  v-if="continuePlaying.entry?.installed"
                  class="size-5"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path d="M8 5v14l11-7z" />
                </svg>
                <svg
                  v-else-if="continuePlaying.entry"
                  class="size-5"
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
                {{ heroActionLabel }}
              </button>
            </div>
          </div>
        </section>

        <!-- Updates available — only when something's behind. -->
        <LibraryShelf
          v-if="updatesEntries.length > 0"
          title="Updates available"
          :entries="updatesEntries"
          :last-played-map="lastPlayedMap"
          @select="goToGame"
        />

        <!-- Collections — visual cards you click into; scrolls like a shelf. -->
        <SectionError
          v-if="shelvesError"
          title="Couldn't load your collections"
          @retry="loadShelves"
        />
        <LibraryRow
          v-else-if="collectionShelves.length > 0"
          title="Collections"
          :count="collectionShelves.length"
        >
          <LibraryCollectionCard
            v-for="shelf in collectionShelves"
            :key="shelf.id"
            :id="shelf.id"
            :name="shelf.name"
            :covers="
              shelf.entries
                .map((e) => e.game.mCoverObjectId)
                .filter((c): c is string => !!c)
            "
          />
        </LibraryRow>

        <!-- Consoles — emulated games grouped by system (toggle in settings),
             followed in the SAME row by the installed emulator hosts (RetroArch
             etc.) that back them. Console cards open a console-themed page;
             emulator cards open a management view (open folder, add a core,
             uninstall). Hidden when there are neither. -->
        <SectionError
          v-if="consolesError || emulatorsError"
          title="Couldn't load your consoles"
          :detail="
            consolesError
              ? 'The console groupings didn\'t load.'
              : 'The installed emulator list didn\'t load.'
          "
          @retry="retryConsoles"
        />
        <LibraryRow
          v-else-if="consoleRows.length > 0 || emulatorHosts.length > 0"
          title="Consoles"
          :count="consoleRows.length + emulatorHosts.length"
        >
          <LibraryConsoleCard
            v-for="row in consoleRows"
            :key="row.id"
            :id="row.id"
            :name="row.shortName"
            :count="row.entries.length"
            :covers="
              row.entries
                .map((e) => e.game.mCoverObjectId)
                .filter((c): c is string => !!c)
            "
          />
          <LibraryEmulatorCard
            v-for="host in emulatorHosts"
            :key="host.id"
            :id="host.id"
            :name="host.name"
            :icon-object-id="host.iconObjectId"
            :retroarch="host.retroarch"
            :core-count="host.cores.length"
          />
        </LibraryRow>

        <!-- All games — Steam-style view switcher + sort over the full grid. -->
        <section>
          <div class="mb-4 flex flex-wrap items-center gap-3">
            <button
              class="rounded-md p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
              :aria-label="allGamesCollapsed ? 'Expand all games' : 'Collapse all games'"
              @click="allGamesCollapsed = !allGamesCollapsed"
            >
              <ChevronDownIcon
                class="size-5 transition-transform"
                :class="{ '-rotate-90': allGamesCollapsed }"
              />
            </button>
            <!-- View switcher (All games / Installed / Not installed). -->
            <Menu as="div" class="relative">
              <MenuButton
                class="inline-flex items-center gap-2 rounded-lg px-2 py-1 font-display text-lg font-semibold text-zinc-100 transition-colors hover:text-white"
              >
                {{ currentViewLabel }}
                <span class="text-xs tabular-nums text-zinc-500">{{
                  allGamesEntries.length
                }}</span>
                <ChevronDownIcon class="size-4 text-zinc-500" />
              </MenuButton>
              <transition
                enter-active-class="transition duration-100 ease-out"
                enter-from-class="opacity-0 scale-95"
                enter-to-class="opacity-100 scale-100"
                leave-active-class="transition duration-75 ease-in"
                leave-from-class="opacity-100 scale-100"
                leave-to-class="opacity-0 scale-95"
              >
                <MenuItems
                  class="absolute left-0 z-20 mt-2 max-h-72 w-56 overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-800 p-1 shadow-xl focus:outline-none"
                >
                  <MenuItem
                    v-for="opt in viewOptions"
                    :key="opt.value"
                    v-slot="{ active }"
                  >
                    <button
                      class="flex w-full items-center rounded-md px-3 py-2 text-left text-sm transition-colors"
                      :class="[
                        active ? 'bg-zinc-700 text-white' : 'text-zinc-300',
                        allView === opt.value ? 'font-semibold text-blue-400' : '',
                      ]"
                      @click="allView = opt.value"
                    >
                      {{ opt.label }}
                    </button>
                  </MenuItem>
                </MenuItems>
              </transition>
            </Menu>

            <div class="flex-1" />

            <!-- Sort. -->
            <Menu as="div" class="relative">
              <MenuButton
                class="inline-flex items-center gap-2 rounded-md bg-zinc-800/50 px-3 py-1.5 text-sm font-medium text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
              >
                <span class="text-zinc-500">Sort by</span>
                {{ currentSortLabel }}
                <ChevronDownIcon class="size-4 text-zinc-500" />
              </MenuButton>
              <transition
                enter-active-class="transition duration-100 ease-out"
                enter-from-class="opacity-0 scale-95"
                enter-to-class="opacity-100 scale-100"
                leave-active-class="transition duration-75 ease-in"
                leave-from-class="opacity-100 scale-100"
                leave-to-class="opacity-0 scale-95"
              >
                <MenuItems
                  class="absolute right-0 z-20 mt-2 w-52 rounded-lg border border-zinc-700 bg-zinc-800 p-1 shadow-xl focus:outline-none"
                >
                  <MenuItem
                    v-for="opt in sortOptions"
                    :key="opt.value"
                    v-slot="{ active }"
                  >
                    <button
                      class="flex w-full items-center rounded-md px-3 py-2 text-left text-sm transition-colors"
                      :class="[
                        active ? 'bg-zinc-700 text-white' : 'text-zinc-300',
                        allSort === opt.value ? 'font-semibold text-blue-400' : '',
                      ]"
                      @click="allSort = opt.value"
                    >
                      {{ opt.label }}
                    </button>
                  </MenuItem>
                </MenuItems>
              </transition>
            </Menu>
          </div>

          <!-- The grid is the one section that genuinely cannot exist without
               the library, so this is where the failure gets reported. It used
               to render the "your library is empty, visit the store" pitch. -->
          <SectionError
            v-if="libraryError"
            title="Couldn't load your library"
            :detail="libraryError"
            @retry="retryLibrary"
          />
          <div
            v-else-if="entries.length === 0"
            class="flex flex-col items-center justify-center py-16 text-center"
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
          <LibraryGrid
            v-else
            v-show="!allGamesCollapsed"
            :entries="allGamesEntries"
            :compact="density === 'compact'"
            :last-played-map="lastPlayedMap"
            :show-hover-action="density !== 'compact'"
            @select="goToGame"
          />
        </section>
      </template>
    </div>

    <!-- Batch compat tester — gated behind dev mode. -->
    <CompatBatchPanel v-if="devMode.enabled.value" class="mx-8 xl:mx-12 mb-8" />

    <!-- Filter drawer — holds the power knobs (incl. density) so the header
         stays calm. -->
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
              View
            </h4>
            <div class="grid grid-cols-2 gap-1.5">
              <button
                v-for="opt in densityOptions"
                :key="opt.value"
                class="inline-flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium transition-colors"
                :class="
                  density === opt.value
                    ? 'bg-blue-600 text-white'
                    : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200'
                "
                @click="density = opt.value"
              >
                <component :is="opt.icon" class="size-4" />
                {{ opt.label }}
              </button>
            </div>
          </section>

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
  AdjustmentsHorizontalIcon,
  XMarkIcon,
  Squares2X2Icon,
  Bars3Icon,
  ChevronDownIcon,
} from "@heroicons/vue/24/outline";
import { Menu, MenuButton, MenuItems, MenuItem } from "@headlessui/vue";
import { invoke } from "@tauri-apps/api/core";
import { parseStatus } from "~/composables/game";
import { useShelves } from "~/composables/shelves";
import {
  useServerApi,
  type RecentPlaytimeEntry,
  type ConsoleGroup,
} from "~/composables/use-server-api";
import { useConsoleSections } from "~/composables/console-sections";
import type { Game, GameStatus, RawGameStatus } from "~/types";
import { InstalledType } from "~/types";
import LibraryGrid from "~/components/LibraryGrid.vue";
import LibraryShelf from "~/components/LibraryShelf.vue";
import LibraryCollectionCard from "~/components/LibraryCollectionCard.vue";
import LibraryConsoleCard from "~/components/LibraryConsoleCard.vue";
import LibraryEmulatorCard from "~/components/LibraryEmulatorCard.vue";
import LibraryRow from "~/components/LibraryRow.vue";
import {
  listInstalledEmulators,
  type EmulatorHost,
} from "~/composables/emulators";

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
const consoleSections = useConsoleSections();

const entries = ref<LibraryEntry[]>([]);
// Console groupings for emulated games — only populated when the toggle is on.
const consoleGroups = ref<ConsoleGroup[]>([]);
// Installed emulator hosts (RetroArch etc.). Independent of the console toggle:
// you should always be able to find the emulator backing your ROMs.
const emulatorHosts = ref<EmulatorHost[]>([]);
const recentPlaytime = ref<RecentPlaytimeEntry[]>([]);
const loading = ref(true);
const showSkeleton = useDeferredLoading(() => loading.value);
const searchInput = ref("");

// Every shelf on this page resolves its own data against `entries`, so a failed
// library fetch used to blank the hero, the collections row and the console
// rows along with the grid — and the page then said "Your library is empty".
// Each source now carries its own failure flag so the sections that can still
// render do, and the ones that cannot say why.
const libraryError = ref<string | null>(null);
const recentError = ref(false);
const consolesError = ref(false);
const emulatorsError = ref(false);
const shelvesError = ref(false);

// Filter / sort / view state persists across navigation (see composable).
// Local `ref`s here would reset every time you open a game and come back.
const {
  density,
  installStateFilter,
  typeFilter,
  sortOrder,
  selectedCollectionIds,
  allGamesCollapsed,
  allView,
  allSort,
} = useLibraryFilters();

const densityOptions = [
  { label: "Cover", value: "cover" as const, icon: Squares2X2Icon },
  { label: "Compact", value: "compact" as const, icon: Bars3Icon },
];

// Transient UI state — should reset on navigation, so it stays local.
const filterDrawerOpen = ref(false);

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
const installedCount = computed(
  () => entries.value.filter((e) => e.installed).length,
);

// gameId → ISO timestamp for the last play session, threaded to tiles.
const lastPlayedMap = computed(() => {
  const map = new Map<string, string>();
  for (const r of recentPlaytime.value) {
    map.set(r.gameId, r.lastPlayedAt);
  }
  return map;
});

// Recently-played entries joined back to the library, ordered desc. Source
// of truth for the Continue Playing hero (entry 0) + the shelf (1..N).
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

/**
 * The hero, decoupled from the library fetch. It prefers a full library entry
 * (which is what knows the banner art and whether the game is installed), but
 * when the library is unavailable the playtime feed on its own still knows what
 * you were last playing, so the hero degrades to cover art and an "Open" action
 * instead of disappearing along with the grid.
 */
const continuePlaying = computed<{
  entry: LibraryEntry | null;
  recent: RecentPlaytimeEntry;
} | null>(() => {
  const joined = recentEntries.value[0];
  if (joined) return joined;
  const first = recentPlaytime.value[0];
  if (libraryError.value && first) return { entry: null, recent: first };
  return null;
});

// Hero display fields, resolved from whichever source is available.
const heroGameId = computed(
  () =>
    continuePlaying.value?.entry?.game.id ??
    continuePlaying.value?.recent.gameId ??
    "",
);
const heroName = computed(
  () =>
    continuePlaying.value?.entry?.game.mName ??
    continuePlaying.value?.recent.gameName ??
    "",
);
const heroImageUrl = computed(() => {
  const banner = continuePlaying.value?.entry?.game.mBannerObjectId;
  if (banner) return useObject(banner);
  const cover = continuePlaying.value?.recent.coverObjectId;
  return cover ? useObject(cover) : "";
});
const heroActionLabel = computed(() => {
  const entry = continuePlaying.value?.entry;
  if (!entry) return "Open";
  return entry.installed ? "Play" : "Install";
});

// Games with an update available — their own shelf when non-empty.
const updatesEntries = computed<LibraryEntry[]>(() =>
  entries.value.filter((e) => e.updateAvailable),
);

// One shelf per collection, games resolved from the in-memory entry list.
const collectionShelves = computed(() => {
  const byId = new Map(entries.value.map((e) => [e.game.id, e]));
  return shelves.value
    .map((shelf) => ({
      id: shelf.id,
      name: shelf.name,
      entries: shelf.entries
        .map((e) => byId.get(e.gameId))
        .filter((e): e is LibraryEntry => !!e),
    }))
    .filter((s) => s.entries.length > 0);
});

// ── Console sections (emulation view) ──────────────────────────────────────
// Resolve each console group's game IDs against the in-memory library, the
// same way collection shelves do. Only consoles with games you actually have
// are kept. Off → empty, so the rows + grid filtering are no-ops.
const consoleRows = computed(() => {
  if (!consoleSections.enabled.value) return [];
  const byId = new Map(entries.value.map((e) => [e.game.id, e]));
  return consoleGroups.value
    .map((group) => ({
      ...group,
      entries: group.gameIds
        .map((id) => byId.get(id))
        .filter((e): e is LibraryEntry => !!e),
    }))
    .filter((g) => g.entries.length > 0);
});

// IDs of emulated games we're surfacing in console rows — pulled OUT of the
// main "All games" grid so they live only under their console (toggle on).
const emulatedGameIds = computed<Set<string>>(() => {
  const set = new Set<string>();
  if (!consoleSections.enabled.value) return set;
  for (const row of consoleRows.value) {
    for (const e of row.entries) set.add(e.game.id);
  }
  return set;
});

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

const activeAdvancedFilterCount = computed(() => {
  let n = 0;
  if (installStateFilter.value !== "all") n += 1;
  if (typeFilter.value !== "all") n += 1;
  if (sortOrder.value !== "name-asc") n += 1;
  n += selectedCollectionIds.value.length;
  return n;
});

// Searching or filtering collapses the curated home into a flat result list.
const filterMode = computed<"sections" | "flat">(() =>
  searchInput.value.trim() || activeAdvancedFilterCount.value > 0
    ? "flat"
    : "sections",
);

const displayedEntries = computed<LibraryEntry[]>(() => {
  const q = searchInput.value.trim().toLowerCase();
  const wantInstalled = installStateFilter.value === "installed";
  const wantNotInstalled = installStateFilter.value === "not-installed";
  const wantUpdates = installStateFilter.value === "updates";
  const wantGame = typeFilter.value === "game";
  const wantTool = typeFilter.value === "tool";

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

// gameId → total playtime seconds, for the "Hours played" sort.
const playtimeMap = computed(() => {
  const map = new Map<string, number>();
  for (const r of recentPlaytime.value) {
    map.set(r.gameId, r.totalPlaytimeSeconds);
  }
  return map;
});

// ── "All games" grid — Steam-style view switcher + sort. ──
// (allGamesCollapsed / allView / allSort come from useLibraryFilters above so
// they survive back-navigation.)

const viewOptions = [
  { label: "All games", value: "all" },
  { label: "Installed", value: "installed" },
  { label: "Not installed", value: "not-installed" },
];

const sortOptions = [
  { label: "Alphabetical (A–Z)", value: "name-asc" as const },
  { label: "Alphabetical (Z–A)", value: "name-desc" as const },
  { label: "Last played", value: "last-played" as const },
  { label: "Hours played", value: "most-played" as const },
];

const currentViewLabel = computed(
  () =>
    viewOptions.find((o) => o.value === allView.value)?.label ?? "All games",
);
const currentSortLabel = computed(
  () =>
    sortOptions.find((o) => o.value === allSort.value)?.label ??
    "Alphabetical (A–Z)",
);

const allGamesEntries = computed<LibraryEntry[]>(() => {
  let list = entries.value;
  // When console sections are on, emulated games live only in their console
  // row — keep them out of the main grid (search/flat mode still shows all).
  if (emulatedGameIds.value.size > 0) {
    list = list.filter((e) => !emulatedGameIds.value.has(e.game.id));
  }
  if (allView.value === "installed") {
    list = list.filter((e) => e.installed);
  } else if (allView.value === "not-installed") {
    list = list.filter((e) => !e.installed);
  } else if (allView.value.startsWith("collection:")) {
    const id = allView.value.slice("collection:".length);
    const shelf = collectionShelves.value.find((s) => s.id === id);
    const ids = new Set((shelf?.entries ?? []).map((e) => e.game.id));
    list = list.filter((e) => ids.has(e.game.id));
  }

  const lastMs = (e: LibraryEntry) => {
    const iso = lastPlayedMap.value.get(e.game.id);
    return iso ? new Date(iso).getTime() : 0;
  };

  const out = [...list];
  switch (allSort.value) {
    case "name-desc":
      out.sort((a, b) => b.game.mName.localeCompare(a.game.mName));
      break;
    case "last-played":
      out.sort((a, b) => lastMs(b) - lastMs(a));
      break;
    case "most-played":
      out.sort(
        (a, b) =>
          (playtimeMap.value.get(b.game.id) ?? 0) -
          (playtimeMap.value.get(a.game.id) ?? 0),
      );
      break;
    default:
      out.sort((a, b) => a.game.mName.localeCompare(b.game.mName));
  }
  return out;
});

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

async function load(hardRefresh = false, silent = false) {
  if (!silent) loading.value = true;
  try {
    const lib = await invoke<FetchLibraryResponse>("fetch_library", {
      hardRefresh,
    });
    const allGames: Game[] = [
      ...lib.library,
      ...lib.collections.flatMap((c) => c.entries.map((e) => e.game)),
      ...lib.other,
      // Installed games no longer in the server library ("delisted"). Big
      // Picture and the sidebar search already include these; the desktop grid
      // was dropping them, so an installed game could silently vanish here.
      ...lib.missing,
    ].filter((g, i, a) => a.findIndex((x) => x.id === g.id) === i);

    // One IPC call for every game's status, instead of a useGame() per game.
    // Each useGame() ran fetch_game, which took a DB write guard whose Drop
    // re-encrypts and rewrites the ENTIRE database to disk — 82 games meant 82
    // full-DB writes and a minutes-long load. fetch_game_statuses reads them all
    // under a single read lock.
    const statusPairs = await invoke<Array<[string, RawGameStatus]>>(
      "fetch_game_statuses",
      { ids: allGames.map((g) => g.id) },
    );
    const statusById = new Map<string, GameStatus>();
    for (const [gameId, raw] of statusPairs) {
      try {
        statusById.set(gameId, parseStatus(raw));
      } catch {
        // No status row yet (a game seeded as [null, null]) — leave it unset;
        // treated as not-installed below, same as a per-game fetch that failed.
      }
    }

    const built: LibraryEntry[] = allGames.map((game) => {
      const status = statusById.get(game.id) ?? null;
      if (!status) {
        return { game, status: null, installed: false, updateAvailable: false };
      }
      const installed =
        status.type === "Installed" &&
        status.install_type.type === InstalledType.Installed;
      const updateAvailable =
        status.type === "Installed" ? status.update_available : false;
      return { game, status, installed, updateAvailable };
    });
    built.sort((a, b) => a.game.mName.localeCompare(b.game.mName));
    entries.value = built;
    libraryError.value = null;
  } catch (e) {
    console.warn("[library] fetch failed:", e);
    // On a silent background revalidation, keep the already-rendered grid
    // rather than blanking it if the refresh fails.
    if (!silent) {
      entries.value = [];
      libraryError.value = e instanceof Error ? e.message : String(e);
    }
  } finally {
    if (!silent) loading.value = false;
  }
}

async function loadRecentPlaytime() {
  recentError.value = false;
  try {
    recentPlaytime.value = await api.playtime.recent();
  } catch (e) {
    console.warn("[library] recent playtime fetch failed:", e);
    recentError.value = true;
    recentPlaytime.value = [];
  }
}

// Only fetch console groupings when the toggle is on (and only once they're
// needed). Soft-fails: the rows just don't appear if the server can't answer.
async function loadConsoles() {
  consolesError.value = false;
  if (!consoleSections.enabled.value) {
    consoleGroups.value = [];
    return;
  }
  try {
    consoleGroups.value = (await api.emulation.consoles()).consoles;
  } catch (e) {
    console.warn("[library] console grouping fetch failed:", e);
    consolesError.value = true;
    consoleGroups.value = [];
  }
}

// Installed emulator hosts for the Emulators section. Independent of both the
// library and the console grouping — it reads local installs, so it still has
// something to show when the server is unreachable.
async function loadEmulators() {
  emulatorsError.value = false;
  try {
    emulatorHosts.value = await listInstalledEmulators();
  } catch (e) {
    console.warn("[library] emulator hosts fetch failed:", e);
    emulatorsError.value = true;
    emulatorHosts.value = [];
  }
}

function retryLibrary() {
  load(true);
}

function loadShelves() {
  shelvesError.value = false;
  return fetchShelves().catch((e) => {
    console.warn("[library] shelves fetch failed:", e);
    shelvesError.value = true;
  });
}

function retryConsoles() {
  loadConsoles();
  loadEmulators();
}

// React to the toggle being flipped on the settings page while we're mounted.
watch(
  () => consoleSections.enabled.value,
  () => loadConsoles(),
);

onMounted(() => {
  // Show the cached library instantly, then silently revalidate against the
  // server. Without the revalidation, fetch_library keeps returning its cached
  // snapshot and games added elsewhere (notably from the store) never appear
  // until each one is opened individually.
  load().then(() => load(true, true));
  loadRecentPlaytime();
  loadConsoles();
  loadEmulators();
  loadShelves();
});
</script>
