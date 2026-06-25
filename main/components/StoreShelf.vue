<template>
  <section v-if="games.length > 0" class="mb-10">
    <!-- Section header: title left, arrow controls right. Mirrors the
         library "Recently played" shelf so the two surfaces feel like
         the same pattern. -->
    <div class="flex items-baseline justify-between mb-4">
      <div class="flex items-baseline gap-3">
        <h3 class="text-lg font-display font-semibold text-zinc-100">
          {{ title }}
        </h3>
        <span class="text-xs text-zinc-500 tabular-nums">
          {{ visibleGames.length }}
        </span>
      </div>
      <div class="flex items-center gap-1.5">
        <button
          class="rounded-md p-1.5 transition-colors"
          :class="
            canScrollLeft
              ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
              : 'bg-zinc-900/40 text-zinc-700 cursor-not-allowed'
          "
          :disabled="!canScrollLeft"
          :aria-label="`Scroll ${title} left`"
          @click="scrollBy(-1)"
        >
          <ChevronLeftIcon class="size-4" />
        </button>
        <button
          class="rounded-md p-1.5 transition-colors"
          :class="
            canScrollRight
              ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
              : 'bg-zinc-900/40 text-zinc-700 cursor-not-allowed'
          "
          :disabled="!canScrollRight"
          :aria-label="`Scroll ${title} right`"
          @click="scrollBy(1)"
        >
          <ChevronRightIcon class="size-4" />
        </button>
      </div>
    </div>

    <!-- Horizontal scroller. Native scrollbar is hidden (the arrows are
         canonical), but trackpad/wheel still works for power users. -->
    <div
      ref="scrollEl"
      class="flex gap-3 overflow-x-auto pb-2 -mx-1 px-1 store-shelf-scroll"
      @scroll="updateScrollState"
    >
      <div
        v-for="game in visibleGames"
        :key="game.id"
        class="shrink-0 w-[150px]"
      >
        <GameTile
          :cover-url="game.mCoverObjectId ? objectUrl(game.mCoverObjectId) : null"
          :name="game.mName"
          :rom="game.isEmulated"
          :update-available="game.updateAvailable ?? false"
          @select="$emit('select', game.id)"
        />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
/**
 * One horizontal "shelf" of store tiles — used for Featured-tab
 * sections like "Most Played This Week" and "Recently Added".
 *
 * Renders as a true horizontal-scroll row (not a wrapped grid) so the
 * Featured tab stops feeling like three stacked Browse pages.  Each
 * shelf carries its own arrow controls + paging behaviour; the native
 * scrollbar is hidden but trackpad / wheel scrolling still works.
 *
 * Visual rhythm matches the library page's "Recently played" row so
 * the two patterns reinforce each other across surfaces.
 */
import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/vue/24/outline";
import type { StoreGame, TrendingGame } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

const props = withDefaults(
  defineProps<{
    title: string;
    games: (StoreGame | TrendingGame)[];
    /** Cap on how many tiles to render. Default keeps a shelf focused;
     *  callers showing trending / recent shelves rarely need more. */
    max?: number;
  }>(),
  { max: 25 },
);

defineEmits<{
  (e: "select", gameId: string): void;
}>();

const visibleGames = computed(() => props.games.slice(0, props.max));

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

// ── Horizontal-scroll state ──────────────────────────────────────────────
const scrollEl = ref<HTMLElement | null>(null);
const scrollLeft = ref(0);
const scrollMax = ref(0);

function updateScrollState() {
  const el = scrollEl.value;
  if (!el) return;
  scrollLeft.value = el.scrollLeft;
  scrollMax.value = Math.max(0, el.scrollWidth - el.clientWidth);
}

const canScrollLeft = computed(() => scrollLeft.value > 4);
const canScrollRight = computed(() => scrollLeft.value < scrollMax.value - 4);

function scrollBy(direction: -1 | 1) {
  const el = scrollEl.value;
  if (!el) return;
  // Page by ~80% of visible width so a sliver of context overlaps
  // between pages — the user keeps their bearings without losing the
  // last tile or two.
  const delta = direction * Math.round(el.clientWidth * 0.8);
  el.scrollBy({ left: delta, behavior: "smooth" });
}

// Re-measure whenever the input data changes (initial load, refetch,
// or a different shelf reusing the same component instance). Without
// this the right arrow would stay disabled on first render.
watch(
  () => visibleGames.value.length,
  () => {
    nextTick(updateScrollState);
  },
);

// Same on-mount + on-resize trigger as StoreRecentlyUpdated: without
// it, the right-arrow stays hidden on first render because the
// scroll handler hasn't fired yet.
onMounted(() => {
  nextTick(updateScrollState);
  window.addEventListener("resize", updateScrollState);
});
onUnmounted(() => {
  window.removeEventListener("resize", updateScrollState);
});
</script>

<style scoped>
.store-shelf-scroll {
  scrollbar-width: none;
}
.store-shelf-scroll::-webkit-scrollbar {
  display: none;
}
</style>
