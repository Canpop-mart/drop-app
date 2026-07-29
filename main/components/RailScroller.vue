<!--
  Horizontal scroller for the community rail. Renders a section header (title
  slot + optional count) and, only when the row actually overflows its
  container, a pair of left/right arrow buttons that scroll it — each arrow
  disabling at its end. Used for "Playing now" and "In rotation", which can
  hold more entries than fit the narrow rail.

  Overflow is tracked with a ResizeObserver on both the viewport and the inner
  content, so the arrows appear/disappear as entries stream in (the 30s
  now-playing poll) or the window resizes.
-->
<template>
  <div>
    <div class="mb-2 flex items-center gap-2">
      <h3
        class="flex items-center gap-1.5 text-sm font-display font-semibold text-zinc-300"
      >
        <slot name="title" />
      </h3>
      <div class="ml-auto flex items-center gap-2">
        <span v-if="$slots.count" class="text-xs text-zinc-500">
          <slot name="count" />
        </span>
        <div v-if="overflowing" class="flex items-center gap-1">
          <button
            type="button"
            class="grid size-6 place-items-center rounded-md text-zinc-400 ring-1 ring-zinc-700/50 transition-colors hover:bg-zinc-700/40 hover:text-zinc-200 disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-zinc-400"
            :disabled="!canLeft"
            aria-label="Scroll left"
            @click="scrollByDir(-1)"
          >
            <ChevronLeftIcon class="size-4" />
          </button>
          <button
            type="button"
            class="grid size-6 place-items-center rounded-md text-zinc-400 ring-1 ring-zinc-700/50 transition-colors hover:bg-zinc-700/40 hover:text-zinc-200 disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-zinc-400"
            :disabled="!canRight"
            aria-label="Scroll right"
            @click="scrollByDir(1)"
          >
            <ChevronRightIcon class="size-4" />
          </button>
        </div>
      </div>
    </div>
    <div
      ref="scroller"
      class="overflow-x-auto pb-1"
      style="scrollbar-width: none"
      @scroll="update"
    >
      <div ref="content" class="flex w-max gap-3">
        <slot />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/vue/24/solid";

const scroller = ref<HTMLElement | null>(null);
const content = ref<HTMLElement | null>(null);
const canLeft = ref(false);
const canRight = ref(false);
const overflowing = computed(() => canLeft.value || canRight.value);

function update() {
  const el = scroller.value;
  if (!el) return;
  canLeft.value = el.scrollLeft > 1;
  canRight.value = Math.ceil(el.scrollLeft + el.clientWidth) < el.scrollWidth - 1;
}

function scrollByDir(dir: number) {
  const el = scroller.value;
  if (!el) return;
  el.scrollBy({ left: dir * el.clientWidth * 0.85, behavior: "smooth" });
}

let ro: ResizeObserver | null = null;
onMounted(() => {
  update();
  ro = new ResizeObserver(() => update());
  if (scroller.value) ro.observe(scroller.value);
  if (content.value) ro.observe(content.value);
});

onUnmounted(() => {
  ro?.disconnect();
  ro = null;
});
</script>
