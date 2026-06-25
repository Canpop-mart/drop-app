<template>
  <section v-if="entries.length > 0">
    <div class="mb-4 flex items-baseline justify-between">
      <div class="flex items-baseline gap-3">
        <component
          :is="to ? 'NuxtLink' : 'h2'"
          :to="to"
          class="font-display text-lg font-semibold text-zinc-100"
          :class="to ? 'transition-colors hover:text-blue-400' : ''"
        >
          {{ title }}
        </component>
        <span class="text-xs tabular-nums text-zinc-500">{{
          entries.length
        }}</span>
      </div>
      <div
        v-if="canScrollLeft || canScrollRight"
        class="flex items-center gap-1.5"
      >
        <button
          class="rounded-md p-1.5 transition-colors"
          :class="
            canScrollLeft
              ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
              : 'cursor-not-allowed bg-zinc-900/40 text-zinc-700'
          "
          :disabled="!canScrollLeft"
          :aria-label="`Scroll ${title} left`"
          @click="scroll(-1)"
        >
          <ChevronLeftIcon class="size-4" />
        </button>
        <button
          class="rounded-md p-1.5 transition-colors"
          :class="
            canScrollRight
              ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
              : 'cursor-not-allowed bg-zinc-900/40 text-zinc-700'
          "
          :disabled="!canScrollRight"
          :aria-label="`Scroll ${title} right`"
          @click="scroll(1)"
        >
          <ChevronRightIcon class="size-4" />
        </button>
      </div>
    </div>
    <div
      ref="scrollEl"
      class="shelf-scroll-row -mx-1 flex gap-4 overflow-x-auto px-1 pb-2"
      @scroll="updateScrollState"
    >
      <div
        v-for="entry in entries"
        :key="entry.game.id"
        class="w-[190px] shrink-0"
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
          :last-played="lastPlayedMap?.get(entry.game.id) ?? null"
          :hover-action="entry.installed ? 'play' : 'install'"
          @select="emit('select', entry.game.id)"
        />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/vue/24/outline";
import GameTile from "~/components/GameTile.vue";

interface ShelfEntry {
  game: { id: string; mName: string; mCoverObjectId: string | null };
  installed: boolean;
  updateAvailable: boolean;
}

const props = defineProps<{
  title: string;
  entries: ShelfEntry[];
  /** Optional last-played lookup so tiles can show a "Played X ago" line. */
  lastPlayedMap?: Map<string, string>;
  /** Optional route — makes the shelf title a link (e.g. to a collection). */
  to?: string;
}>();

const emit = defineEmits<{ (e: "select", gameId: string): void }>();

// ── Horizontal scroll state + arrow paging (one shelf owns its own row) ──
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

function scroll(direction: -1 | 1) {
  const el = scrollEl.value;
  if (!el) return;
  // Page by ~80% of the visible width so a tile or two of context overlaps.
  el.scrollBy({
    left: direction * Math.round(el.clientWidth * 0.8),
    behavior: "smooth",
  });
}

// Re-measure when the entry set changes so the right arrow enables on first
// render when the row overflows.
watch(
  () => props.entries.length,
  () => nextTick(updateScrollState),
  { immediate: true },
);
onMounted(() => nextTick(updateScrollState));
</script>

<style scoped>
/* The arrow buttons are the canonical control; hide the native bar but keep
   wheel / trackpad swipe working. */
.shelf-scroll-row {
  scrollbar-width: none;
}
.shelf-scroll-row::-webkit-scrollbar {
  display: none;
}
</style>
