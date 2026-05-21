<template>
  <!-- Wide container — the store is the desktop landing, so it should use
       the full window width rather than getting boxed into a centred
       max-w-7xl strip. xl padding keeps the grid from running edge-to-edge
       on ultrawide displays. -->
  <div class="w-full px-10 xl:px-14 py-6">
    <!-- Header — title + tabs -->
    <div class="mb-6">
      <h1 class="text-2xl font-display font-bold text-zinc-100">Store</h1>
      <p class="mt-1 text-sm text-zinc-400">
        Browse and add games to your library.
      </p>
    </div>

    <!-- Top bar: tab nav + search/select toggle -->
    <div class="flex items-center gap-2 mb-6 border-b border-zinc-700/50">
      <button
        v-for="tab in tabs"
        :key="tab.value"
        class="relative px-5 py-3 text-sm font-medium transition-colors"
        :class="
          activeTab === tab.value
            ? 'text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200'
        "
        @click="activeTab = tab.value"
      >
        {{ tab.label }}
        <span
          v-if="activeTab === tab.value"
          class="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500 rounded-full"
        />
      </button>

      <div class="flex-1" />

      <!-- Search box — visible on both tabs, but only the Browse tab acts on
           it for filtered results. Submitting from Featured switches to
           Browse and runs the query. -->
      <div class="relative">
        <MagnifyingGlassIcon
          class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-zinc-500 pointer-events-none"
        />
        <input
          v-model="searchInput"
          type="text"
          placeholder="Search the store..."
          class="rounded-lg border border-zinc-700 bg-zinc-800/50 pl-9 pr-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:bg-zinc-800 focus:ring-2 focus:ring-blue-500 outline-none transition-colors w-72"
          @keydown.enter="submitSearch"
        />
      </div>
    </div>

    <!-- Loading skeleton on first load only -->
    <div v-if="loading" class="space-y-8">
      <div class="aspect-[21/9] rounded-2xl bg-zinc-800/50 animate-pulse" />
      <GameTileGrid>
        <div
          v-for="i in 8"
          :key="i"
          class="aspect-[3/4] rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </GameTileGrid>
    </div>

    <!-- ═══ Featured tab ═══ -->
    <template v-else-if="activeTab === 'featured'">
      <!-- Hero carousel -->
      <section
        v-if="featured.length > 0"
        class="mb-10"
      >
        <div
          class="relative rounded-2xl overflow-hidden cursor-pointer group aspect-[21/9]"
          @click="goToGame(featured[heroIndex]?.id)"
        >
          <img
            v-if="featured[heroIndex]?.mBannerObjectId"
            :src="objectUrl(featured[heroIndex].mBannerObjectId)"
            :alt="featured[heroIndex].mName"
            class="w-full h-full object-cover"
          />
          <div
            class="absolute inset-0 bg-gradient-to-t from-zinc-950/95 via-zinc-950/40 to-transparent"
          />
          <div
            class="absolute inset-0 bg-gradient-to-r from-zinc-950/70 via-transparent to-transparent"
          />
          <div class="absolute bottom-0 inset-x-0 p-8">
            <h2
              class="text-4xl font-display font-bold text-zinc-100 drop-shadow-lg mb-2"
            >
              {{ featured[heroIndex]?.mName }}
            </h2>
            <p
              class="text-sm text-zinc-200/90 line-clamp-2 max-w-3xl mb-3"
            >
              {{ featured[heroIndex]?.mShortDescription }}
            </p>
            <div
              v-if="featured[heroIndex]?.tags?.length"
              class="flex gap-2"
            >
              <span
                v-for="tag in featured[heroIndex].tags!.slice(0, 4)"
                :key="tag.id"
                class="px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-500/30 text-blue-200 backdrop-blur-sm"
              >
                {{ tag.name }}
              </span>
            </div>
          </div>
          <!-- Carousel dots (click to jump) -->
          <div
            v-if="featured.length > 1"
            class="absolute bottom-4 right-6 flex gap-1.5"
          >
            <button
              v-for="(_, i) in featured"
              :key="i"
              class="size-2 rounded-full transition-colors"
              :class="i === heroIndex ? 'bg-blue-500' : 'bg-zinc-500/70 hover:bg-zinc-400'"
              @click.stop="heroIndex = i"
            />
          </div>
        </div>
      </section>

      <!-- Most Played This Week (trending) -->
      <StoreShelf
        v-if="trending.length > 0"
        title="Most Played This Week"
        :games="trending"
        @select="goToGame"
      />

      <!-- Recently Added -->
      <StoreShelf
        v-if="recentGames.length > 0"
        title="Recently Added"
        :games="recentGames"
        @select="goToGame"
      />

      <div
        v-if="featured.length === 0 && trending.length === 0 && recentGames.length === 0"
        class="text-center text-zinc-500 py-20 text-sm"
      >
        No featured games yet. Try the Browse tab to see what's available.
      </div>
    </template>

    <!-- ═══ Browse tab ═══ -->
    <template v-else-if="activeTab === 'browse'">
      <!-- Filter / sort controls -->
      <div class="flex items-center gap-3 mb-3 flex-wrap text-sm">
        <span class="text-zinc-500">Sort:</span>
        <button
          v-for="opt in sortOptions"
          :key="opt.value"
          class="px-3 py-1.5 rounded-md text-xs font-medium transition-colors"
          :class="
            browseSort === opt.value
              ? 'bg-blue-600/20 text-blue-400 ring-1 ring-blue-500/40'
              : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700/50'
          "
          @click="setSort(opt.value)"
        >
          {{ opt.label }}
        </button>

        <span class="mx-2 h-4 w-px bg-zinc-700" />

        <!-- Library / collection selector (server-side filter). -->
        <select
          v-if="libraries.length > 0"
          v-model="selectedLibraryId"
          class="rounded-md bg-zinc-800/50 text-zinc-200 px-2.5 py-1.5 text-xs font-medium border border-zinc-700/50 focus:ring-2 focus:ring-blue-500 outline-none"
          @change="loadBrowse(true)"
        >
          <option value="">All libraries</option>
          <option v-for="lib in libraries" :key="lib.id" :value="lib.id">
            {{ lib.name }}
          </option>
        </select>

        <!-- More filters drawer button — opens side panel with advanced
             knobs that don't fit in the chip row. -->
        <button
          class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors"
          :class="
            activeAdvancedFilterCount > 0
              ? 'bg-blue-600/20 text-blue-400 ring-1 ring-blue-500/40'
              : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700/50'
          "
          @click="filterDrawerOpen = true"
        >
          <AdjustmentsHorizontalIcon class="size-3" />
          Filters
          <span
            v-if="activeAdvancedFilterCount > 0"
            class="ml-1 rounded-full bg-blue-500 text-white text-[10px] font-bold px-1.5 leading-4"
          >
            {{ activeAdvancedFilterCount }}
          </span>
        </button>

        <span
          v-if="hasActiveFilters"
          class="text-xs text-zinc-500"
        >
          {{ displayedResults.length }} of {{ browseTotal }}
        </span>

        <div class="flex-1" />

        <button
          v-if="hasActiveFilters"
          class="text-xs text-zinc-500 hover:text-zinc-300 underline"
          @click="clearBrowseFilters"
        >
          Clear filters
        </button>
      </div>

      <!-- Active filter chips — quick-remove handles for whatever filters
           are on, including selected tags from the drawer. Keeps the
           drawer's state visible without having to re-open it. -->
      <div
        v-if="activeFilterChips.length > 0"
        class="flex flex-wrap gap-1.5 mb-5"
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
      </div>

      <!-- Grid -->
      <div v-if="browseLoading && browseResults.length === 0" class="text-zinc-500 text-sm py-10">
        Loading games...
      </div>
      <div
        v-else-if="displayedResults.length === 0"
        class="text-zinc-500 text-sm py-20 text-center"
      >
        No games match those filters.
      </div>
      <GameTileGrid v-else>
        <GameTile
          v-for="game in displayedResults"
          :key="game.id"
          :cover-url="game.mCoverObjectId ? objectUrl(game.mCoverObjectId) : null"
          :name="game.mName"
          :rom="game.isEmulated"
          :update-available="game.updateAvailable ?? false"
          @select="goToGame(game.id)"
        />
      </GameTileGrid>

      <!-- Load more — manual paginate so initial load stays fast even on
           huge libraries. When client-side filters hide most results we
           still surface the button so the user can pull more candidates
           from the server until something matches. -->
      <div
        v-if="browseResults.length < browseTotal"
        class="mt-8 flex justify-center"
      >
        <button
          class="rounded-md bg-zinc-800/50 px-5 py-2.5 text-sm font-semibold text-zinc-200 hover:bg-zinc-800 transition-colors disabled:opacity-50"
          :disabled="browseLoading"
          @click="loadBrowseMore"
        >
          {{ browseLoading ? "Loading..." : `Load more (${browseTotal - browseResults.length} left)` }}
        </button>
      </div>
    </template>

    <!-- Filter drawer — slides in from the right with advanced controls
         that don't belong in the chip row. Closed by default; opened via
         the "Filters" button. -->
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
          <!-- Tags multi-select. Each tag toggles into the CSV the
               server side actually understands. -->
          <section v-if="allTags.length > 0">
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Tags
            </h4>
            <input
              v-model="tagSearch"
              type="text"
              placeholder="Search tags..."
              class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-3 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 focus:ring-2 focus:ring-blue-500 outline-none mb-2"
            />
            <div class="max-h-64 overflow-y-auto pr-1 space-y-1">
              <label
                v-for="tag in filteredTagList"
                :key="tag.id"
                class="flex items-center gap-2 px-2 py-1 rounded hover:bg-zinc-800/60 cursor-pointer"
              >
                <input
                  type="checkbox"
                  :checked="selectedTagIds.includes(tag.id)"
                  class="size-3.5 rounded bg-zinc-800 border-zinc-700 text-blue-500 focus:ring-blue-500 focus:ring-offset-0"
                  @change="toggleTag(tag.id)"
                />
                <span class="text-xs text-zinc-300">{{ tag.name }}</span>
              </label>
            </div>
          </section>

          <!-- Game type — emulated (ROM) vs native. Maps to client-side
               filter on `isEmulated`. -->
          <section>
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Game type
            </h4>
            <div class="grid grid-cols-3 gap-1.5">
              <button
                v-for="opt in emulatedOptions"
                :key="opt.value"
                class="px-2 py-1.5 rounded-md text-xs font-medium transition-colors"
                :class="
                  emulatedFilter === opt.value
                    ? 'bg-blue-600 text-white'
                    : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200'
                "
                @click="emulatedFilter = opt.value"
              >
                {{ opt.label }}
              </button>
            </div>
          </section>

          <!-- Release year range. Inclusive on both ends; blank = no
               constraint on that side. -->
          <section>
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Release year
            </h4>
            <div class="flex items-center gap-2">
              <input
                v-model.number="releaseYearFrom"
                type="number"
                placeholder="From"
                class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-2.5 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 focus:ring-2 focus:ring-blue-500 outline-none"
              />
              <span class="text-zinc-500">–</span>
              <input
                v-model.number="releaseYearTo"
                type="number"
                placeholder="To"
                class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-2.5 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 focus:ring-2 focus:ring-blue-500 outline-none"
              />
            </div>
          </section>

          <!-- Platform — derived from the launchPlatform field on each
               StoreGame. The server already accepts `platform` as a query
               param, so this filter goes through the wire (not client
               post-filter). -->
          <section v-if="availablePlatforms.length > 0">
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Platform
            </h4>
            <div class="grid grid-cols-2 gap-1.5">
              <button
                v-for="opt in availablePlatforms"
                :key="opt"
                class="px-2 py-1.5 rounded-md text-xs font-medium transition-colors"
                :class="
                  selectedPlatforms.includes(opt)
                    ? 'bg-blue-600 text-white'
                    : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200'
                "
                @click="togglePlatform(opt)"
              >
                {{ opt }}
              </button>
            </div>
          </section>
        </div>

        <div class="sticky bottom-0 bg-zinc-900 border-t border-zinc-800 px-5 py-3 flex items-center gap-2">
          <button
            class="flex-1 rounded-md bg-zinc-800 px-3 py-2 text-xs font-semibold text-zinc-300 hover:bg-zinc-700 transition-colors"
            @click="clearBrowseFilters"
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
  AdjustmentsHorizontalIcon,
  XMarkIcon,
} from "@heroicons/vue/24/outline";
import {
  useServerApi,
  type StoreGame,
  type StoreTag,
  type TrendingGame,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { deduplicatedInvoke } from "~/composables/game";
import StoreShelf from "~/components/StoreShelf.vue";

useHead({ title: "Store" });

const route = useRoute();
const router = useRouter();
const api = useServerApi();

// Deep link: /store?gameId=... opens that game's detail page directly.
// Used by other surfaces (e.g. error pages, library) to send users to a
// specific game's store entry — desktop sends them to the dedicated
// /store/[id] page now (BPM-parity for the click-from-store flow).
const incomingGameId = route.query.gameId?.toString();
if (incomingGameId) {
  router.replace(`/store/${incomingGameId}`);
}

const tabs = [
  { label: "Featured", value: "featured" },
  { label: "Browse", value: "browse" },
] as const;
const activeTab = ref<(typeof tabs)[number]["value"]>("featured");

// State
const loading = ref(true);
const browseLoading = ref(false);

// Search uses a controlled input (`searchInput`) plus a committed value
// (`searchQuery`) that the query layer reads. Enter/submit promotes the
// input to the query and switches to Browse. Keeps the typing experience
// snappy without spamming requests on every keystroke.
const searchInput = ref("");
const searchQuery = ref("");
const heroIndex = ref(0);
const browseSort = ref<"default" | "newest" | "name" | "recent">("default");

// ── Filter state ──────────────────────────────────────────────────────────
//
// Two flavours of filter live side-by-side:
//   • Server-side (sent to /api/v1/store): tag IDs, library, sort, search.
//     The API only knows about these.
//   • Client-side (post-filter on the returned page): developer, publisher,
//     release year range, emulated/native. The server doesn't expose
//     filters for these but every relevant field is already on `StoreGame`,
//     so we can match locally without round-tripping.
//
// The split matters for pagination: server-side filters narrow the result
// set the server is paging over, while client-side filters just hide rows
// from whatever page we've already pulled — so the "Load more" button stays
// honest about how many *server* results remain.
const selectedTagIds = ref<string[]>([]);
const selectedLibraryId = ref<string>("");
const selectedPlatforms = ref<string[]>([]);
const tagSearch = ref("");
const emulatedFilter = ref<"all" | "native" | "rom">("all");
const releaseYearFrom = ref<number | null>(null);
const releaseYearTo = ref<number | null>(null);
const filterDrawerOpen = ref(false);

const emulatedOptions = [
  { label: "All", value: "all" as const },
  { label: "Native", value: "native" as const },
  { label: "ROM", value: "rom" as const },
];

const sortOptions = [
  { label: "Default", value: "default" as const },
  { label: "Newest", value: "newest" as const },
  { label: "Recently updated", value: "recent" as const },
  { label: "A–Z", value: "name" as const },
];

// Featured data
const featured = ref<StoreGame[]>([]);
const trending = ref<TrendingGame[]>([]);
const recentGames = ref<StoreGame[]>([]);

// Browse data
const browseResults = ref<StoreGame[]>([]);
const browseTotal = ref(0);

// Tag + library catalogs (loaded lazily once we land on the Browse tab).
const allTags = ref<StoreTag[]>([]);
const libraries = ref<Array<{ id: string; name: string }>>([]);

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId?: string) {
  if (!gameId) return;
  // Prefetch via deduplicated invoke so the detail page mounts fast.
  // Click from the store always lands on the store presentation
  // (/store/[id]) — the library detail page is reserved for the
  // owns-the-game / install-management surface.
  deduplicatedInvoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/store/${gameId}`);
}

function submitSearch() {
  searchQuery.value = searchInput.value.trim();
  activeTab.value = "browse";
  loadBrowse(true);
}

function setSort(value: typeof browseSort.value) {
  browseSort.value = value;
  loadBrowse(true);
}

function toggleTag(tagId: string) {
  const i = selectedTagIds.value.indexOf(tagId);
  if (i === -1) selectedTagIds.value.push(tagId);
  else selectedTagIds.value.splice(i, 1);
  loadBrowse(true);
}

function togglePlatform(platform: string) {
  const i = selectedPlatforms.value.indexOf(platform);
  if (i === -1) selectedPlatforms.value.push(platform);
  else selectedPlatforms.value.splice(i, 1);
  loadBrowse(true);
}

function clearBrowseFilters() {
  searchInput.value = "";
  searchQuery.value = "";
  selectedTagIds.value = [];
  selectedLibraryId.value = "";
  selectedPlatforms.value = [];
  tagSearch.value = "";
  emulatedFilter.value = "all";
  releaseYearFrom.value = null;
  releaseYearTo.value = null;
  loadBrowse(true);
}

// Platform options — discovered from whatever launchPlatform values
// appear in the loaded result set. Server has no /platforms endpoint, so
// the dropdown grows as the user pages through the catalog.
const availablePlatforms = computed(() => {
  const set = new Set<string>();
  for (const g of browseResults.value) {
    if (g.launchPlatform) set.add(g.launchPlatform);
  }
  return [...set].sort();
});

// Filter the tag list by the in-drawer search box. Avoids forcing the
// user to scroll through hundreds of catalog tags when they know what
// they're looking for.
const filteredTagList = computed(() => {
  const q = tagSearch.value.trim().toLowerCase();
  if (!q) return allTags.value;
  return allTags.value.filter((t) => t.name.toLowerCase().includes(q));
});

// How many advanced (non-default) filters are active. Drives the badge
// count on the "Filters" button so the user can see at a glance whether
// they've narrowed the catalog and need to clear before browsing fresh.
const activeAdvancedFilterCount = computed(() => {
  let n = 0;
  n += selectedTagIds.value.length;
  if (selectedLibraryId.value) n += 1;
  n += selectedPlatforms.value.length;
  if (emulatedFilter.value !== "all") n += 1;
  if (releaseYearFrom.value || releaseYearTo.value) n += 1;
  return n;
});

const hasActiveFilters = computed(
  () => Boolean(searchQuery.value) || activeAdvancedFilterCount.value > 0,
);

// Chip row above the grid — one chip per active filter so the user can
// pop them off without re-opening the drawer.
type FilterChip =
  | { key: string; kind: "tag"; tagId: string; label: string; value: string }
  | { key: string; kind: "library"; label: string; value: string }
  | {
      key: string;
      kind: "platform";
      platform: string;
      label: string;
      value: string;
    }
  | { key: string; kind: "emulated"; label: string; value: string }
  | { key: string; kind: "year"; label: string; value: string };

const activeFilterChips = computed<FilterChip[]>(() => {
  const chips: FilterChip[] = [];
  for (const id of selectedTagIds.value) {
    const tag = allTags.value.find((t) => t.id === id);
    chips.push({
      key: `tag:${id}`,
      kind: "tag",
      tagId: id,
      label: "Tag",
      value: tag?.name ?? id,
    });
  }
  if (selectedLibraryId.value) {
    const lib = libraries.value.find((l) => l.id === selectedLibraryId.value);
    chips.push({
      key: "library",
      kind: "library",
      label: "Library",
      value: lib?.name ?? selectedLibraryId.value,
    });
  }
  for (const p of selectedPlatforms.value) {
    chips.push({
      key: `platform:${p}`,
      kind: "platform",
      platform: p,
      label: "Platform",
      value: p,
    });
  }
  if (emulatedFilter.value !== "all") {
    chips.push({
      key: "emulated",
      kind: "emulated",
      label: "Type",
      value: emulatedFilter.value === "rom" ? "ROM" : "Native",
    });
  }
  if (releaseYearFrom.value || releaseYearTo.value) {
    const from = releaseYearFrom.value ?? "";
    const to = releaseYearTo.value ?? "";
    chips.push({
      key: "year",
      kind: "year",
      label: "Year",
      value: `${from}${from || to ? "–" : ""}${to}`,
    });
  }
  return chips;
});

function removeFilterChip(chip: FilterChip) {
  switch (chip.kind) {
    case "tag":
      toggleTag(chip.tagId);
      return;
    case "library":
      selectedLibraryId.value = "";
      loadBrowse(true);
      return;
    case "platform":
      togglePlatform(chip.platform);
      return;
    case "emulated":
      emulatedFilter.value = "all";
      return;
    case "year":
      releaseYearFrom.value = null;
      releaseYearTo.value = null;
      return;
  }
}

// Apply client-side filters on top of whatever the server returned.
// Server already handled tags/library/platform/sort/search; we layer
// the two fields the API doesn't expose as filters: year and emulation
// type. Both are returned per-game (mReleased + isEmulated) so the
// filter is exact, not heuristic.
const displayedResults = computed<StoreGame[]>(() => {
  const yFrom = releaseYearFrom.value;
  const yTo = releaseYearTo.value;
  const wantRom = emulatedFilter.value === "rom";
  const wantNative = emulatedFilter.value === "native";

  return browseResults.value.filter((g) => {
    if (wantRom && !g.isEmulated) return false;
    if (wantNative && g.isEmulated) return false;

    if (yFrom != null || yTo != null) {
      const released = g.mReleased ? new Date(g.mReleased) : null;
      const year = released && !isNaN(released.getTime())
        ? released.getFullYear()
        : null;
      if (year == null) return false;
      if (yFrom != null && year < yFrom) return false;
      if (yTo != null && year > yTo) return false;
    }

    return true;
  });
});

// Hero auto-advance every 8s, paused while the user is hovering. Matches
// what BPM does, just without the focus-nav considerations.
let heroInterval: ReturnType<typeof setInterval> | null = null;
function startHeroRotation() {
  stopHeroRotation();
  heroInterval = setInterval(() => {
    if (featured.value.length > 1) {
      heroIndex.value = (heroIndex.value + 1) % featured.value.length;
    }
  }, 8_000);
}
function stopHeroRotation() {
  if (heroInterval) {
    clearInterval(heroInterval);
    heroInterval = null;
  }
}

async function loadFeaturedData() {
  try {
    const [feat, trend, recent] = await Promise.all([
      api.store.featured().catch(() => [] as StoreGame[]),
      api.store
        .trending(7, 7)
        .then((d) => d.results)
        .catch(() => [] as TrendingGame[]),
      api.store
        .browse({ sort: "newest", take: 14 })
        .then((d) => d.results)
        .catch(() => [] as StoreGame[]),
    ]);
    featured.value = feat;
    trending.value = trend;
    recentGames.value = recent;
  } catch (e) {
    console.error("[STORE] Failed to load featured:", e);
  }
}

// Build the server-side query — only the params the API actually
// understands. CSV the tag IDs together since that's what /api/v1/store
// expects (single `tags=a,b,c` query string).
type BrowseParams = NonNullable<Parameters<typeof api.store.browse>[0]>;

function buildBrowseParams(skip: number): BrowseParams {
  // Narrow to the literal union the API typings expect — TS otherwise
  // widens `effectiveSort` to plain `string` since it's a conditional
  // result from refs typed as broader unions.
  const effectiveSort: NonNullable<BrowseParams["sort"]> = searchQuery.value
    ? "relevance"
    : browseSort.value;
  return {
    skip,
    take: 35,
    q: searchQuery.value || undefined,
    tags: selectedTagIds.value.length
      ? selectedTagIds.value.join(",")
      : undefined,
    library: selectedLibraryId.value || undefined,
    platform: selectedPlatforms.value.length
      ? selectedPlatforms.value.join(",")
      : undefined,
    sort: effectiveSort,
    order: effectiveSort === "name" ? "asc" : undefined,
  };
}

async function loadBrowse(reset = false) {
  if (reset) {
    browseResults.value = [];
  }
  browseLoading.value = true;
  try {
    const data = await api.store.browse(buildBrowseParams(0));
    browseResults.value = data.results;
    browseTotal.value = data.count;
  } catch (e) {
    console.error("[STORE] Failed to load browse:", e);
  } finally {
    browseLoading.value = false;
  }
}

async function loadBrowseMore() {
  browseLoading.value = true;
  try {
    const data = await api.store.browse(
      buildBrowseParams(browseResults.value.length),
    );
    browseResults.value.push(...data.results);
    browseTotal.value = data.count;
  } catch (e) {
    console.error("[STORE] Failed to load more:", e);
  } finally {
    browseLoading.value = false;
  }
}

// Tags + libraries — cheap-to-fetch reference data we pull once and reuse
// for the drawer. Failures are non-fatal: the drawer just hides the
// corresponding section if either list is empty.
async function loadFilterCatalogs() {
  const [tags, libs] = await Promise.all([
    api.store.tags().catch(() => [] as StoreTag[]),
    api.store.libraries().catch(
      () => [] as Array<{ id: string; name: string }>,
    ),
  ]);
  allTags.value = [...tags].sort((a, b) => a.name.localeCompare(b.name));
  libraries.value = [...libs].sort((a, b) => a.name.localeCompare(b.name));
}

onMounted(async () => {
  await Promise.all([loadFeaturedData(), loadFilterCatalogs()]);
  // Browse is loaded lazily — only when the user actually opens that tab —
  // so the initial Featured render isn't blocked on a full-catalog query.
  loading.value = false;
  startHeroRotation();
});

// Lazy-load browse when the tab is first opened.
watch(activeTab, (tab) => {
  if (tab === "browse" && browseResults.value.length === 0 && !browseLoading.value) {
    loadBrowse(true);
  }
});

onBeforeUnmount(() => {
  stopHeroRotation();
});
</script>
