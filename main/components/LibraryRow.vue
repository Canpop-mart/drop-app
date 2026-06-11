<template>
  <section>
    <div class="mb-4 flex items-baseline justify-between">
      <div class="flex items-baseline gap-3">
        <h2 class="font-display text-lg font-semibold text-zinc-100">
          {{ title }}
        </h2>
        <span
          v-if="count !== undefined"
          class="text-xs tabular-nums text-zinc-500"
          >{{ count }}</span
        >
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
      class="library-row -mx-1 flex gap-4 overflow-x-auto px-1 pb-2"
      @scroll="updateScrollState"
    >
      <slot />
    </div>
  </section>
</template>

<script setup lang="ts">
import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/vue/24/outline";

const props = defineProps<{
  title: string;
  /** Optional item count shown next to the title. */
  count?: number;
}>();

// ── Horizontal scroll state + arrow paging (mirrors LibraryShelf). ──
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
  el.scrollBy({
    left: direction * Math.round(el.clientWidth * 0.8),
    behavior: "smooth",
  });
}

// Re-measure when the item count changes so the arrows enable correctly.
watch(
  () => props.count,
  () => nextTick(updateScrollState),
  { immediate: true },
);
onMounted(() => nextTick(updateScrollState));
</script>

<style scoped>
/* Arrow buttons are the control; hide the native bar, keep wheel/swipe. */
.library-row {
  scrollbar-width: none;
}
.library-row::-webkit-scrollbar {
  display: none;
}
</style>
