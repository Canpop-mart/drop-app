<template>
  <div class="flex flex-col h-full" :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }">
    <!-- Tab navigation -->
    <div class="flex items-center gap-2 px-8 py-4 border-b" :style="{ borderColor: 'var(--bpm-border)' }">
      <button
        v-for="tab in tabs"
        :key="tab.value"
        :ref="(el: any) => registerTab(el, { onSelect: () => (activeTab = tab.value) })"
        class="px-4 py-2 text-sm rounded-lg font-medium transition-colors"
        :class="[
          activeTab === tab.value
            ? 'bg-blue-600/20 text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50',
        ]"
        @click="activeTab = tab.value"
      >
        {{ tab.label }}
      </button>

      <div class="flex-1" />

      <button
        :ref="(el: any) => registerTab(el, { onSelect: () => (showSearch = true) })"
        class="flex items-center gap-2 px-4 py-2 text-sm rounded-lg font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50 transition-colors"
        @click="showSearch = true"
      >
        <MagnifyingGlassIcon class="size-4" />
        <span v-if="searchQuery">{{ searchQuery }}</span>
        <span v-else class="text-zinc-600">Search store...</span>
      </button>
    </div>

    <!-- On-screen keyboard for search -->
    <BigPictureKeyboard
      :visible="showSearch"
      :model-value="searchQuery"
      placeholder="Search the store..."
      @update:model-value="searchQuery = $event"
      @close="showSearch = false"
      @submit="showSearch = false; activeTab = 'browse'"
    />

    <!-- Loading skeleton -->
    <div v-if="loading" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="space-y-6">
        <div class="aspect-[21/9] rounded-2xl bg-zinc-800/50 animate-pulse" />
        <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
          <div v-for="i in 8" :key="i" class="aspect-[3/4] rounded-xl bg-zinc-800/50 animate-pulse" />
        </div>
      </div>
    </div>

    <!-- ═══ Featured tab ═══ -->
    <div v-else-if="activeTab === 'featured'" class="flex-1 overflow-y-auto px-8 py-6" data-bp-scroll>
      <!-- Hero carousel -->
      <div v-if="featured.length > 0" class="mb-8">
        <div
          class="relative rounded-2xl overflow-hidden cursor-pointer aspect-[21/9]"
          :ref="(el: any) => registerFeaturedHero(el, { onSelect: () => goToGame(featured[heroIndex]?.id) })"
          @click="goToGame(featured[heroIndex]?.id)"
        >
          <img
            v-if="featured[heroIndex]?.mBannerObjectId"
            :src="objectUrl(featured[heroIndex].mBannerObjectId)"
            :alt="featured[heroIndex].mName"
            class="w-full h-full object-cover"
          />
          <div class="absolute inset-0 bg-gradient-to-t from-zinc-950/90 via-zinc-950/30 to-transparent" />
          <div class="absolute bottom-0 inset-x-0 p-6">
            <h2 class="text-3xl font-bold font-display text-white mb-1">
              {{ featured[heroIndex]?.mName }}
            </h2>
            <p class="text-sm text-zinc-300 line-clamp-2 max-w-2xl">
              {{ featured[heroIndex]?.mShortDescription }}
            </p>
            <div v-if="featured[heroIndex]?.tags?.length" class="flex gap-2 mt-3">
              <span
                v-for="tag in featured[heroIndex].tags!.slice(0, 4)"
                :key="tag.id"
                class="px-2 py-0.5 rounded-full text-xs font-medium"
                style="background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)"
              >
                {{ tag.name }}
              </span>
            </div>
          </div>
          <!-- Carousel dots -->
          <div v-if="featured.length > 1" class="absolute bottom-4 right-6 flex gap-1.5">
            <button
              v-for="(_, i) in featured"
              :key="i"
              class="size-2 rounded-full transition-colors"
              :class="i === heroIndex ? 'bg-blue-500' : 'bg-zinc-600'"
              @click.stop="heroIndex = i"
            />
          </div>
        </div>
      </div>

      <!-- Trending section (1 row) -->
      <div v-if="trending.length > 0" class="mb-8">
        <h3 class="text-lg font-semibold font-display text-zinc-200 mb-4">
          Most Played This Week
        </h3>
        <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
          <div
            v-for="game in visibleTrending"
            :key="game.id"
            :ref="(el: any) => registerGrid(el, {
              onSelect: () => goToGame(game.id),
            })"
            class="group relative flex flex-col rounded-xl transition-all duration-200 cursor-pointer bp-focus-delegate"
          >
            <div class="bp-focus-ring relative aspect-[3/4] bg-zinc-800 rounded-xl overflow-hidden">
              <img
                v-if="game.mCoverObjectId"
                :src="objectUrl(game.mCoverObjectId)"
                :alt="game.mName"
                class="w-full h-full object-cover"
                loading="lazy"
              />
              <div v-if="game.isEmulated" class="rom-scanlines absolute inset-0 pointer-events-none" />
              <div
                v-if="game.updateAvailable"
                class="absolute top-2 right-2 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase z-10"
                style="background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)"
              >
                Outdated
              </div>
              <div class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent pointer-events-none" />
            </div>
            <div class="px-2 py-2 bg-zinc-900/80">
              <p class="text-sm font-medium text-zinc-200 truncate">{{ game.mName }}</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Recently added section (1 row) -->
      <div v-if="recentGames.length > 0" class="mb-8">
        <h3 class="text-lg font-semibold font-display text-zinc-200 mb-4">
          Recently Added
        </h3>
        <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
          <div
            v-for="game in visibleRecentGames"
            :key="game.id"
            :ref="(el: any) => registerGrid(el, {
              onSelect: () => goToGame(game.id),
            })"
            class="group relative flex flex-col rounded-xl transition-all duration-200 cursor-pointer bp-focus-delegate"
          >
            <div class="bp-focus-ring relative aspect-[3/4] bg-zinc-800 rounded-xl overflow-hidden">
              <img
                v-if="game.mCoverObjectId"
                :src="objectUrl(game.mCoverObjectId)"
                :alt="game.mName"
                class="w-full h-full object-cover"
                loading="lazy"
              />
              <div v-if="game.isEmulated" class="rom-scanlines absolute inset-0 pointer-events-none" />
              <div
                v-if="game.updateAvailable"
                class="absolute top-2 right-2 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase z-10"
                style="background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)"
              >
                Outdated
              </div>
              <div class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent pointer-events-none" />
            </div>
            <div class="px-2 py-2 bg-zinc-900/80">
              <p class="text-sm font-medium text-zinc-200 truncate">{{ game.mName }}</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Random picks section (2 rows) -->
      <div v-if="randomGames.length > 0" class="mb-8">
        <h3 class="text-lg font-semibold font-display text-zinc-200 mb-4">
          Random Picks
        </h3>
        <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
          <div
            v-for="game in visibleRandomGames"
            :key="game.id"
            :ref="(el: any) => registerGrid(el, {
              onSelect: () => goToGame(game.id),
            })"
            class="group relative flex flex-col rounded-xl transition-all duration-200 cursor-pointer bp-focus-delegate"
          >
            <div class="bp-focus-ring relative aspect-[3/4] bg-zinc-800 rounded-xl overflow-hidden">
              <img
                v-if="game.mCoverObjectId"
                :src="objectUrl(game.mCoverObjectId)"
                :alt="game.mName"
                class="w-full h-full object-cover"
                loading="lazy"
              />
              <div v-if="!game.mCoverObjectId" class="w-full h-full flex items-center justify-center">
                <span class="text-2xl font-bold text-zinc-500">{{ game.mName[0] }}</span>
              </div>
              <div v-if="game.isEmulated" class="rom-scanlines absolute inset-0 pointer-events-none" />
              <div
                v-if="game.updateAvailable"
                class="absolute top-2 right-2 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase z-10"
                style="background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)"
              >
                Outdated
              </div>
              <div class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent pointer-events-none" />
            </div>
            <div class="px-2 py-2 bg-zinc-900/80">
              <p class="text-sm font-medium text-zinc-200 truncate">{{ game.mName }}</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ Browse tab ═══ -->
    <div v-else-if="activeTab === 'browse'" class="flex-1 overflow-y-auto px-8 py-6" data-bp-scroll>
      <!-- Filter summary bar -->
      <div class="flex items-center gap-3 mb-4">
        <div class="flex items-center gap-2 text-sm text-zinc-400">
          <ArrowsUpDownIcon class="size-4" />
          <span>{{ browseSortLabel }}</span>
          <template v-if="browseLibraryFilter || browseAchievementFilter">
            <span class="text-zinc-600">|</span>
            <FunnelIcon class="size-3.5" />
            <span v-if="browseLibraryFilter" class="text-blue-400">{{ browseLibraryLabel.replace('Library: ', '') }}</span>
            <span v-if="browseAchievementFilter" class="text-blue-400">Has Achievements</span>
          </template>
        </div>
      </div>

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
            <div class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-3xl w-full mx-4">
              <h2 class="text-xl font-semibold font-display text-zinc-100 mb-5">Sort & Filter</h2>

              <div class="grid grid-cols-3 gap-6">
                <!-- Sort section -->
                <div>
                  <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">Sort By</p>
                  <div class="space-y-1.5">
                    <button
                      v-for="(label, key) in browseSortLabels"
                      :key="key"
                      class="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors"
                      :class="browseSort === key
                        ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                        : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                      :ref="(el: any) => registerFilterMenu(el, { onSelect: () => { browseSort = key; } })"
                      @click="browseSort = key"
                    >
                      <span class="font-medium">{{ label }}</span>
                    </button>
                  </div>
                </div>

                <!-- Library filter section -->
                <div v-if="libraries.length > 0">
                  <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">Library</p>
                  <div class="space-y-1.5 max-h-72 overflow-y-auto pr-1">
                    <button
                      class="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors"
                      :class="!browseLibraryFilter
                        ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                        : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                      :ref="(el: any) => registerFilterMenu(el, { onSelect: () => { browseLibraryFilter = ''; } })"
                      @click="browseLibraryFilter = ''"
                    >
                      <span class="font-medium">All Libraries</span>
                    </button>
                    <button
                      v-for="lib in libraries"
                      :key="lib.id"
                      class="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors"
                      :class="browseLibraryFilter === lib.id
                        ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                        : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                      :ref="(el: any) => registerFilterMenu(el, { onSelect: () => { browseLibraryFilter = lib.id; } })"
                      @click="browseLibraryFilter = lib.id"
                    >
                      <span class="font-medium">{{ lib.name }}</span>
                    </button>
                  </div>
                </div>

                <!-- Achievements filter -->
                <div>
                  <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">Achievements</p>
                  <div class="space-y-1.5">
                    <button
                      class="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors"
                      :class="!browseAchievementFilter
                        ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                        : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                      :ref="(el: any) => registerFilterMenu(el, { onSelect: () => { browseAchievementFilter = ''; } })"
                      @click="browseAchievementFilter = ''"
                    >
                      <span class="font-medium">All Games</span>
                    </button>
                    <button
                      class="w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-colors"
                      :class="browseAchievementFilter === 'has_achievements'
                        ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                        : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                      :ref="(el: any) => registerFilterMenu(el, { onSelect: () => { browseAchievementFilter = 'has_achievements'; } })"
                      @click="browseAchievementFilter = 'has_achievements'"
                    >
                      <span class="font-medium">Has Achievements</span>
                    </button>
                  </div>
                </div>
              </div>

              <!-- Close -->
              <button
                :ref="(el: any) => registerFilterMenu(el, { onSelect: () => { showFilterMenu = false; } })"
                class="w-full mt-5 px-4 py-3 rounded-xl text-sm font-medium bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700 transition-colors"
                @click="showFilterMenu = false"
              >
                Done
              </button>
            </div>
          </div>
        </Transition>
      </Teleport>

      <div
        v-if="browseResults.length > 0"
        ref="browseGridEl"
        class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7"
      >
        <div
          v-for="game in browsePageResults"
          :key="game.id"
          :ref="(el: any) => registerGrid(el, {
            onSelect: () => goToGame(game.id),
          })"
          class="group relative flex flex-col rounded-xl transition-all duration-200 cursor-pointer bp-focus-delegate"
        >
          <div class="bp-focus-ring relative aspect-[3/4] bg-zinc-800 rounded-xl overflow-hidden">
            <img
              v-if="game.mCoverObjectId"
              :src="objectUrl(game.mCoverObjectId)"
              :alt="game.mName"
              class="w-full h-full object-cover"
              loading="lazy"
            />
            <div v-if="!game.mCoverObjectId" class="w-full h-full flex items-center justify-center">
              <span class="text-2xl font-bold text-zinc-500">{{ game.mName[0] }}</span>
            </div>
            <div v-if="game.isEmulated" class="rom-scanlines absolute inset-0 pointer-events-none" />
            <div
              v-if="game.updateAvailable"
              class="absolute top-2 right-2 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase z-10"
              style="background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)"
            >
              Outdated
            </div>
            <div class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent pointer-events-none" />
          </div>
          <div class="px-2 py-2 bg-zinc-900/80">
            <p class="text-sm font-medium text-zinc-200 truncate">{{ game.mName }}</p>
          </div>
        </div>
      </div>

      <!-- Pagination -->
      <div v-if="browseTotalPages > 1" class="flex items-center justify-center gap-4 py-6">
        <button
          :ref="(el: any) => registerTab(el, { onSelect: browsePrevPage })"
          :disabled="browsePage === 0"
          class="px-4 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm font-medium hover:bg-zinc-700 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          @click="browsePrevPage"
        >
          Previous
        </button>
        <span class="text-sm text-zinc-500">
          Page {{ browsePage + 1 }} of {{ browseTotalPages }}
        </span>
        <button
          :ref="(el: any) => registerTab(el, { onSelect: browseNextPage })"
          :disabled="browsePage >= browseTotalPages - 1"
          class="px-4 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm font-medium hover:bg-zinc-700 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          @click="browseNextPage"
        >
          Next
        </button>
      </div>

      <div v-if="browseResults.length === 0 && !browseLoading" class="flex items-center justify-center py-24">
        <div class="text-center">
          <MagnifyingGlassIcon class="size-16 mx-auto mb-4 text-zinc-600" />
          <h3 class="text-2xl font-semibold text-zinc-400 mb-2">
            {{ searchQuery ? `No results for "${searchQuery}"` : 'No games found' }}
          </h3>
          <p class="text-zinc-600">Try a different search or filter</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { MagnifyingGlassIcon, ArrowsUpDownIcon, FunnelIcon } from "@heroicons/vue/24/outline";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { useServerApi, type StoreGame, type TrendingGame } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { deduplicatedInvoke } from "~/composables/game";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useDeckMode } from "~/composables/deck-mode";

definePageMeta({ layout: "bigpicture" });

const api = useServerApi();
const router = useRouter();
const focusNav = useFocusNavigation();
const gamepad = useGamepad();

// Focus groups — all in "content" so D-pad naturally reaches game tiles
const registerTab = useBpFocusableGroup("content");
const registerFeaturedHero = useBpFocusableGroup("content");
const registerGrid = useBpFocusableGroup("content");
const registerFilterMenu = useBpFocusableGroup("filter-menu");

// State
const loading = ref(true);
const browseLoading = ref(false);
const activeTab = ref("featured");
const showSearch = ref(false);
const showFilterMenu = ref(false);
const searchQuery = ref("");
const heroIndex = ref(0);
const browseSort = ref("default");

const browseSortLabels: Record<string, string> = {
  default: "Default",
  newest: "Newest",
  name: "Name",
  recent: "Recently Updated",
};
const browseSortLabel = computed(() => browseSortLabels[browseSort.value] ?? "Default");

function cycleBrowseSort() {
  const modes = ["default", "newest", "name", "recent"];
  const idx = modes.indexOf(browseSort.value);
  browseSort.value = modes[(idx + 1) % modes.length];
}

// Browse filters & pagination
const browseLibraryFilter = ref("");
const browseAchievementFilter = ref("");
const libraries = ref<Array<{ id: string; name: string }>>([]);
const browsePage = ref(0);
const browseGridEl = ref<HTMLElement | null>(null);

// Data
const featured = ref<StoreGame[]>([]);
const trending = ref<TrendingGame[]>([]);
const recentGames = ref<StoreGame[]>([]);
const randomGames = ref<StoreGame[]>([]);
const browseResults = ref<StoreGame[]>([]);
const browseTotal = ref(0);

// Compute grid column count from window width to match Tailwind breakpoints:
// grid-cols-2 sm:3 md:4 lg:5 xl:6 2xl:7
const gridCols = ref(7);

function updateGridCols() {
  const w = window.innerWidth;
  if (w >= 1536) gridCols.value = 7;       // 2xl
  else if (w >= 1280) gridCols.value = 6;  // xl
  else if (w >= 1024) gridCols.value = 5;  // lg
  else if (w >= 768) gridCols.value = 4;   // md
  else if (w >= 640) gridCols.value = 3;   // sm
  else gridCols.value = 2;
}

// Slice data to exact row counts so hidden overflow items aren't focusable
const visibleTrending = computed(() => trending.value.slice(0, gridCols.value));
const visibleRecentGames = computed(() => recentGames.value.slice(0, gridCols.value));
const visibleRandomGames = computed(() => randomGames.value.slice(0, gridCols.value * 2));

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}


function goToGame(gameId?: string) {
  if (!gameId) return;
  console.log(`[BPM:STORE] Navigating to game: ${gameId}`);
  focusNav.saveFocusSnapshot("/bigpicture/store");
  router.push(`/bigpicture/library/${gameId}`).then(() => {
    console.log(`[BPM:STORE] Navigation complete for: ${gameId}`);
  }).catch((e) => {
    console.error(`[BPM:STORE] Navigation FAILED for ${gameId}:`, e);
  });
}

// Hero auto-advance
let heroTimer: ReturnType<typeof setInterval> | null = null;

function startHeroTimer() {
  stopHeroTimer();
  if (featured.value.length <= 1) return;
  heroTimer = setInterval(() => {
    heroIndex.value = (heroIndex.value + 1) % featured.value.length;
  }, 8000);
}

function stopHeroTimer() {
  if (heroTimer) {
    clearInterval(heroTimer);
    heroTimer = null;
  }
}

// Pagination: compute how many items per page based on grid columns × 4 rows
// We fetch a generous batch and paginate client-side so column-count changes
// don't cause jank. Fall back to 28 (7 cols × 4 rows) when we can't measure.
const itemsPerPage = computed(() => {
  const el = browseGridEl.value;
  if (!el) return 28;
  const style = getComputedStyle(el);
  const cols = style.getPropertyValue("grid-template-columns").split(" ").length;
  return cols * 4;
});

const browsePageResults = computed(() => {
  const start = browsePage.value * itemsPerPage.value;
  // Slice to exactly 4 rows worth of items so no overflow items get gamepad focus
  const maxItems = gridCols.value * 4;
  return browseResults.value.slice(start, start + Math.min(itemsPerPage.value, maxItems));
});

const browseTotalPages = computed(() => {
  if (browseResults.value.length === 0) return 0;
  return Math.ceil(browseTotal.value / itemsPerPage.value);
});

function browseNextPage() {
  if (browsePage.value < browseTotalPages.value - 1) {
    browsePage.value++;
    // If we're near the end of loaded results but server has more, fetch next batch
    const nextStart = browsePage.value * itemsPerPage.value;
    if (nextStart + itemsPerPage.value > browseResults.value.length && browseResults.value.length < browseTotal.value) {
      loadBrowseMore();
    }
  }
}

function browsePrevPage() {
  if (browsePage.value > 0) {
    browsePage.value--;
  }
}

const browseLibraryLabel = computed(() => {
  if (!browseLibraryFilter.value) return "Library: All";
  const lib = libraries.value.find((l) => l.id === browseLibraryFilter.value);
  return `Library: ${lib?.name ?? "All"}`;
});

function cycleLibraryFilter() {
  const opts = ["", ...libraries.value.map((l) => l.id)];
  const idx = opts.indexOf(browseLibraryFilter.value);
  browseLibraryFilter.value = opts[(idx + 1) % opts.length];
}

function toggleAchievementFilter() {
  browseAchievementFilter.value = browseAchievementFilter.value
    ? ""
    : "has_achievements";
}

function clearBrowseFilters() {
  searchQuery.value = "";
  browseLibraryFilter.value = "";
  browseAchievementFilter.value = "";
  browsePage.value = 0;
  loadBrowse(true);
}

// Browse functionality
async function loadBrowse(reset = false) {
  if (reset) {
    browseResults.value = [];
    browsePage.value = 0;
  }
  browseLoading.value = true;
  try {
    const effectiveSort = searchQuery.value
      ? "relevance"
      : (browseSort.value as any) || "default";
    const data = await api.store.browse({
      skip: 0,
      take: 55,
      q: searchQuery.value || undefined,
      library: browseLibraryFilter.value || undefined,
      sort: effectiveSort,
      order: effectiveSort === "name" ? "asc" : undefined,
    });
    browseResults.value = data.results;
    browseTotal.value = data.count;
  } catch (e) {
    console.error("Failed to load browse:", e);
  } finally {
    browseLoading.value = false;
  }
}

async function loadBrowseMore() {
  browseLoading.value = true;
  try {
    const effectiveSort = searchQuery.value
      ? "relevance"
      : (browseSort.value as any) || "default";
    const data = await api.store.browse({
      skip: browseResults.value.length,
      take: 55,
      q: searchQuery.value || undefined,
      library: browseLibraryFilter.value || undefined,
      sort: effectiveSort,
      order: effectiveSort === "name" ? "asc" : undefined,
    });
    browseResults.value.push(...data.results);
    browseTotal.value = data.count;
  } catch (e) {
    console.error("Failed to load more browse:", e);
  } finally {
    browseLoading.value = false;
  }
}

// Handle focus restriction when filter menu opens/closes via any method
watch(showFilterMenu, (open) => {
  if (open) {
    focusNav.restrictFocus("filter-menu");
    nextTick(() => focusNav.autoFocusContent("filter-menu"));
  } else {
    focusNav.unrestrictFocus("content");
  }
});

// Reload browse when tab switches to browse
watch(activeTab, (tab) => {
  if (tab === "browse") {
    loadBrowse(true);
  }
});

// Reload browse when sort or filters change
watch(browseSort, () => {
  if (activeTab.value === "browse") {
    loadBrowse(true);
  }
});

watch([browseLibraryFilter, browseAchievementFilter], () => {
  if (activeTab.value === "browse") {
    loadBrowse(true);
  }
});

// Reload browse when search changes (with debounce)
let searchDebounce: ReturnType<typeof setTimeout> | null = null;
watch(searchQuery, () => {
  if (activeTab.value === "browse") {
    if (searchDebounce) clearTimeout(searchDebounce);
    searchDebounce = setTimeout(() => loadBrowse(true), 300);
  }
});

// Face button handlers for Search (Y) and Sort (X).
// On Steam Deck (Gamescope), the Web Gamepad API reports physical Y as index 2
// (mapped to West) and physical X as index 3 (mapped to North). Instead of
// relying on a button-map swap, we register the handlers on the correct
// logical buttons per-platform so the physical buttons always match the
// context bar labels (Y glyph = Search, X glyph = Sort).
const { isGamescope } = useDeckMode();
const searchButton = isGamescope.value ? GamepadButton.West : GamepadButton.North;
const sortButton = isGamescope.value ? GamepadButton.North : GamepadButton.West;

const _unsubs: (() => void)[] = [];
_unsubs.push(
  gamepad.onButton(searchButton, () => {
    if (showFilterMenu.value) return; // ignore while filter menu open
    showSearch.value = !showSearch.value;
    if (showSearch.value) activeTab.value = "browse";
  }),
);
// X / West button — open Sort & Filter overlay (only on Browse tab)
_unsubs.push(
  gamepad.onButton(sortButton, () => {
    if (showSearch.value) return;
    if (activeTab.value === "browse") {
      showFilterMenu.value = !showFilterMenu.value;
    }
  }),
);
// B / East — close filter menu when open
_unsubs.push(
  gamepad.onButton(GamepadButton.East, () => {
    if (showFilterMenu.value) {
      showFilterMenu.value = false;
    }
  }),
);

// Initial data load
onMounted(async () => {
  updateGridCols();
  window.addEventListener("resize", updateGridCols);
  try {
    const [featuredData, trendingData, recentData, randomData, librariesData] = await Promise.all([
      api.store.featured().catch(() => [] as StoreGame[]),
      api.store.trending(10, 7).catch(() => ({ results: [] as TrendingGame[] })),
      api.store.browse({ take: 10, sort: "recent" }).catch(() => ({ results: [] as StoreGame[], count: 0 })),
      api.store.browse({ take: 20, sort: "random" }).catch(() => ({ results: [] as StoreGame[], count: 0 })),
      api.store.libraries().catch(() => [] as Array<{ id: string; name: string }>),
    ]);

    featured.value = featuredData;
    trending.value = trendingData.results;
    recentGames.value = recentData.results;
    randomGames.value = randomData.results;
    libraries.value = librariesData;

    startHeroTimer();
  } catch (e) {
    console.error("Failed to load store data:", e);
  } finally {
    loading.value = false;
    nextTick(() => {
      if (!focusNav.restoreFocusSnapshot("/bigpicture/store")) {
        focusNav.autoFocusContent("content");
      }
    });
  }
});

onUnmounted(() => {
  stopHeroTimer();
  window.removeEventListener("resize", updateGridCols);
  if (searchDebounce) clearTimeout(searchDebounce);
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;
});

const tabs = [
  { label: "Featured", value: "featured" },
  { label: "Browse", value: "browse" },
];
</script>

<style scoped>
/*
 * CRT scanline overlay for ROM game covers.
 * Subtle horizontal lines + slight vignette to give a retro feel.
 */
.rom-scanlines {
  background: repeating-linear-gradient(
    to bottom,
    transparent 0px,
    transparent 2px,
    rgba(0, 0, 0, 0.08) 2px,
    rgba(0, 0, 0, 0.08) 4px
  );
  /* Vignette effect */
  box-shadow: inset 0 0 40px rgba(0, 0, 0, 0.15);
}

</style>
