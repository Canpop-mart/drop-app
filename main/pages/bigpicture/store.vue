<template>
  <div class="flex flex-col h-full">
    <!-- Tab navigation -->
    <div class="flex items-center gap-2 px-8 py-4 border-b border-zinc-800/30">
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
    <div v-else-if="activeTab === 'featured'" class="flex-1 overflow-y-auto px-8 py-6">
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
                class="px-2 py-0.5 rounded-full bg-zinc-800/60 text-xs text-zinc-400"
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

      <!-- Trending section -->
      <div v-if="trending.length > 0" class="mb-8">
        <h3 class="text-lg font-semibold font-display text-zinc-200 mb-4">
          Most Played This Week
        </h3>
        <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
          <div
            v-for="game in trending"
            :key="game.id"
            :ref="(el: any) => registerGrid(el, {
              onSelect: () => goToGame(game.id),
            })"
            class="group relative flex flex-col rounded-xl overflow-hidden transition-all duration-200 cursor-pointer ring-2 ring-transparent"
          >
            <div class="relative aspect-[3/4] bg-zinc-800">
              <img
                v-if="game.mCoverObjectId"
                :src="objectUrl(game.mCoverObjectId)"
                :alt="game.mName"
                class="w-full h-full object-cover"
                loading="lazy"
              />
              <div class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent" />
            </div>
            <div class="px-2 py-2 bg-zinc-900/80">
              <p class="text-sm font-medium text-zinc-200 truncate">{{ game.mName }}</p>
              <p class="text-xs text-zinc-500">{{ game.recentPlayers }} {{ game.recentPlayers === 1 ? 'session' : 'sessions' }} this week</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Recently added section -->
      <div v-if="recentGames.length > 0" class="mb-8">
        <h3 class="text-lg font-semibold font-display text-zinc-200 mb-4">
          Recently Added
        </h3>
        <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
          <div
            v-for="game in recentGames"
            :key="game.id"
            :ref="(el: any) => registerGrid(el, {
              onSelect: () => goToGame(game.id),
            })"
            class="group relative flex flex-col rounded-xl overflow-hidden transition-all duration-200 cursor-pointer ring-2 ring-transparent"
          >
            <div class="relative aspect-[3/4] bg-zinc-800">
              <img
                v-if="game.mCoverObjectId"
                :src="objectUrl(game.mCoverObjectId)"
                :alt="game.mName"
                class="w-full h-full object-cover"
                loading="lazy"
              />
              <div class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent" />
            </div>
            <div class="px-2 py-2 bg-zinc-900/80">
              <p class="text-sm font-medium text-zinc-200 truncate">{{ game.mName }}</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ Browse tab ═══ -->
    <div v-else-if="activeTab === 'browse'" class="flex-1 overflow-y-auto px-8 py-6">
      <!-- Filter bar -->
      <div class="flex flex-wrap items-center gap-3 mb-6">
        <!-- Sort indicator (cycled via X button) -->
        <div class="flex items-center gap-1.5 px-3 py-2 text-sm font-medium text-zinc-500">
          <ArrowsUpDownIcon class="size-4" />
          Sort: {{ browseSortLabel }}
        </div>

        <!-- Clear filters -->
        <button
          v-if="searchQuery"
          class="flex items-center gap-1.5 px-3 py-2 text-sm rounded-lg bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 transition-colors"
          @click="searchQuery = ''; loadBrowse(true)"
        >
          <XMarkIcon class="size-3.5" />
          "{{ searchQuery }}"
        </button>
      </div>

      <div v-if="browseResults.length > 0" class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
        <div
          v-for="game in browseResults"
          :key="game.id"
          :ref="(el: any) => registerGrid(el, {
            onSelect: () => goToGame(game.id),
          })"
          class="group relative flex flex-col rounded-xl overflow-hidden transition-all duration-200 cursor-pointer ring-2 ring-transparent"
        >
          <div class="relative aspect-[3/4] bg-zinc-800">
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
            <div class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent" />
          </div>
          <div class="px-2 py-2 bg-zinc-900/80">
            <p class="text-sm font-medium text-zinc-200 truncate">{{ game.mName }}</p>
            <div v-if="game.tags?.length" class="flex gap-1 mt-1 overflow-hidden">
              <span
                v-for="tag in game.tags.slice(0, 2)"
                :key="tag.id"
                class="px-1.5 py-0.5 rounded-full bg-zinc-800/60 text-[10px] text-zinc-500 truncate"
              >
                {{ tag.name }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- Load more -->
      <div v-if="browseHasMore" class="flex justify-center py-6">
        <button
          :ref="(el: any) => registerTab(el, { onSelect: loadMoreBrowse })"
          class="px-6 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm font-medium hover:bg-zinc-700 transition-colors"
          @click="loadMoreBrowse"
        >
          Load More
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
import { MagnifyingGlassIcon, XMarkIcon, ArrowsUpDownIcon } from "@heroicons/vue/24/outline";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { useServerApi, type StoreGame, type TrendingGame } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { deduplicatedInvoke } from "~/composables/game";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";

definePageMeta({ layout: "bigpicture" });

const api = useServerApi();
const router = useRouter();
const focusNav = useFocusNavigation();
const gamepad = useGamepad();

// Focus groups — all in "content" so D-pad naturally reaches game tiles
const registerTab = useBpFocusableGroup("content");
const registerFeaturedHero = useBpFocusableGroup("content");
const registerGrid = useBpFocusableGroup("content");

// State
const loading = ref(true);
const browseLoading = ref(false);
const activeTab = ref("featured");
const showSearch = ref(false);
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

// Data
const featured = ref<StoreGame[]>([]);
const trending = ref<TrendingGame[]>([]);
const recentGames = ref<StoreGame[]>([]);
const browseResults = ref<StoreGame[]>([]);
const browseTotal = ref(0);

const browseHasMore = computed(() => browseResults.value.length < browseTotal.value);

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId?: string) {
  if (!gameId) return;
  focusNav.saveFocusSnapshot("/bigpicture/store");
  router.push(`/bigpicture/library/${gameId}`);
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

// Browse functionality
async function loadBrowse(reset = false) {
  if (reset) browseResults.value = [];
  browseLoading.value = true;
  try {
    const data = await api.store.browse({
      skip: reset ? 0 : browseResults.value.length,
      take: 20,
      q: searchQuery.value || undefined,
      sort: searchQuery.value
        ? "relevance"
        : (browseSort.value as any) || "default",
    });
    if (reset) {
      browseResults.value = data.results;
    } else {
      browseResults.value.push(...data.results);
    }
    browseTotal.value = data.count;
  } catch (e) {
    console.error("Failed to load browse:", e);
  } finally {
    browseLoading.value = false;
  }
}

function loadMoreBrowse() {
  loadBrowse(false);
}

// Reload browse when tab switches to browse
watch(activeTab, (tab) => {
  if (tab === "browse") {
    loadBrowse(true);
  }
});

// Reload browse when sort changes
watch(browseSort, () => {
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

// Y button = toggle search keyboard
const _unsubs: (() => void)[] = [];
_unsubs.push(
  gamepad.onButton(GamepadButton.North, () => {
    showSearch.value = !showSearch.value;
    if (showSearch.value) activeTab.value = "browse";
  }),
);

// X button = cycle sort mode
_unsubs.push(
  gamepad.onButton(GamepadButton.West, () => {
    cycleBrowseSort();
  }),
);

// Initial data load
onMounted(async () => {
  try {
    const [featuredData, trendingData, recentData] = await Promise.all([
      api.store.featured().catch(() => [] as StoreGame[]),
      api.store.trending(10, 7).catch(() => ({ results: [] as TrendingGame[] })),
      api.store.browse({ take: 10, sort: "recent" }).catch(() => ({ results: [] as StoreGame[], count: 0 })),
    ]);

    featured.value = featuredData;
    trending.value = trendingData.results;
    recentGames.value = recentData.results;

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
  if (searchDebounce) clearTimeout(searchDebounce);
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;
});

const tabs = [
  { label: "Featured", value: "featured" },
  { label: "Browse", value: "browse" },
];
</script>
