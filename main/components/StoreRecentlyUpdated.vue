<template>
  <section v-if="games.length > 0" class="mb-12">
    <!-- Section header: title left, "Browse all" CTA right. Mirrors
         Steam's Recently Updated widget. -->
    <div class="flex items-center justify-between mb-5">
      <h3 class="text-xl font-display font-semibold text-zinc-100">
        Recently Updated
      </h3>
      <button
        class="text-xs font-semibold text-zinc-300 hover:text-zinc-100 px-3 py-1.5 rounded-md bg-zinc-800/60 ring-1 ring-zinc-700/50 hover:ring-zinc-600/60 transition-colors uppercase tracking-wider"
        @click="$emit('browse-all')"
      >
        Browse all
      </button>
    </div>

    <!-- Card carousel. Cards are wider now (~360px) so only 4 are
         visible at a time on standard viewports — matches Steam's
         layout where each card has real breathing room. Arrow buttons
         overlay the rail at the edges. -->
    <div class="relative group">
      <button
        v-if="canScrollLeft"
        class="absolute left-0 top-1/3 -translate-y-1/2 z-10 size-12 -ml-3 rounded-full bg-zinc-900/95 ring-1 ring-zinc-700/60 text-zinc-100 hover:bg-zinc-900 hover:text-blue-400 transition-colors flex items-center justify-center shadow-xl"
        aria-label="Scroll left"
        @click="scrollBy(-1)"
      >
        <ChevronLeftIcon class="size-6" />
      </button>
      <button
        v-if="canScrollRight"
        class="absolute right-0 top-1/3 -translate-y-1/2 z-10 size-12 -mr-3 rounded-full bg-zinc-900/95 ring-1 ring-zinc-700/60 text-zinc-100 hover:bg-zinc-900 hover:text-blue-400 transition-colors flex items-center justify-center shadow-xl"
        aria-label="Scroll right"
        @click="scrollBy(1)"
      >
        <ChevronRightIcon class="size-6" />
      </button>

      <div
        ref="scrollEl"
        class="flex gap-5 overflow-x-auto pb-3 recently-updated-scroll snap-x snap-mandatory"
        @scroll="updateScrollState"
      >
        <article
          v-for="g in games"
          :key="g.id"
          class="shrink-0 w-[380px] rounded-lg overflow-hidden bg-zinc-800/70 ring-1 ring-zinc-700/40 hover:ring-blue-500/50 hover:bg-zinc-800/90 transition-all snap-start cursor-pointer group/card flex flex-col"
          @click="$emit('select', g.id)"
        >
          <!-- Banner section — taller (16:9 → 1.78:1) gives the
               banner room to breathe like Steam's. -->
          <div class="relative aspect-[16/9] bg-zinc-900 overflow-hidden">
            <img
              v-if="g.mBannerObjectId"
              :src="objectUrl(g.mBannerObjectId)"
              :alt="g.mName"
              class="w-full h-full object-cover group-hover/card:scale-[1.03] transition-transform duration-500"
              loading="lazy"
            />
            <BannerFallback v-else :name="g.mName" text-size="text-6xl" />

            <!-- "IN LIBRARY" badge — top-left.  Slightly larger now so
                 it reads as a real status indicator, not a footnote. -->
            <span
              v-if="libraryGameIds?.has(g.id)"
              class="absolute top-3 left-3 inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-bold uppercase tracking-wider bg-blue-500/95 text-white backdrop-blur-sm shadow-lg"
            >
              <span class="size-1.5 rounded-full bg-white" />
              In Library
            </span>

            <!-- "Updated" badge — bottom-right. Position in the list
                 is sorted by recency, so the leftmost cards are the
                 most recent.  Static label since /store doesn't yet
                 expose a per-game updated timestamp. -->
            <span
              class="absolute bottom-3 right-3 px-2.5 py-1 rounded-md text-[11px] font-bold uppercase tracking-wider bg-zinc-950/85 text-zinc-100 backdrop-blur-sm ring-1 ring-zinc-700/40"
            >
              Updated
            </span>
          </div>

          <!-- Body — name + description + CTA. Pushed apart with
               flex-grow so all cards in the row line their CTA up at
               the same baseline regardless of description length. -->
          <div
            class="p-5 flex-1 flex flex-col bg-gradient-to-b from-blue-950/20 via-zinc-900/40 to-zinc-900/60"
          >
            <h4
              class="text-base font-display font-semibold text-zinc-100 truncate mb-2"
            >
              {{ g.mName }}
            </h4>
            <p
              class="text-sm text-zinc-400 leading-relaxed line-clamp-3 mb-4 flex-1"
            >
              {{
                g.mShortDescription ||
                "New catalog entry — view details for what changed."
              }}
            </p>
            <p
              class="text-[11px] text-blue-400 font-bold uppercase tracking-[0.15em] group-hover/card:text-blue-300 transition-colors"
            >
              View update details →
            </p>
          </div>
        </article>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
/**
 * "Recently Updated" — a horizontal card carousel on the Store
 * Featured tab. Designed to mirror Steam's Recently Updated widget:
 * banner-led cards (16:9 banner + name + short description + CTA),
 * "IN LIBRARY" badge for games the caller already owns, side arrows
 * + snap-scroll for navigation.
 *
 * Data shape matches `StoreGame` from the existing /api/v1/store
 * endpoint with `sort: 'updated'` so the order reflects which
 * catalogue entries changed most recently.
 */
import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/vue/24/outline";
import type { StoreGame } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import BannerFallback from "~/components/BannerFallback.vue";

const props = withDefaults(
  defineProps<{
    games: StoreGame[];
    /** Optional set of game IDs the caller has in their library, used
     *  to render the "IN LIBRARY" badge.  Pass `undefined` to skip
     *  the badge entirely. */
    libraryGameIds?: Set<string>;
  }>(),
  {},
);

defineEmits<{
  (e: "select", gameId: string): void;
  (e: "browse-all"): void;
}>();

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

// ── Horizontal-scroll state ─────────────────────────────────────────
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
  const delta = direction * Math.round(el.clientWidth * 0.85);
  el.scrollBy({ left: delta, behavior: "smooth" });
}

watch(
  () => props.games.length,
  () => nextTick(updateScrollState),
);

// Call updateScrollState on mount so the right-arrow shows up
// immediately when the data is already loaded — the scroll handler
// only fires on actual scroll events, so without this the arrow
// would stay hidden until the user touched the rail.  Also listen
// for window resize because shrinking the viewport can newly create
// overflow (or eliminate it).
onMounted(() => {
  nextTick(updateScrollState);
  window.addEventListener("resize", updateScrollState);
});
onUnmounted(() => {
  window.removeEventListener("resize", updateScrollState);
});
</script>

<style scoped>
.recently-updated-scroll {
  scrollbar-width: none;
}
.recently-updated-scroll::-webkit-scrollbar {
  display: none;
}
</style>
