<template>
  <div :class="gridClass" ref="gridEl">
    <slot />
    <!-- Invisible padding cells fill the trailing row so the overall
         grid renders as a perfect rectangle.  Each cell takes the same
         grid track as a real tile but renders nothing, which keeps the
         visible tiles flush-left without leaving an awkward partial
         row at the bottom of a section.  Only emitted when itemCount
         is supplied AND the count isn't already a multiple of cols. -->
    <div
      v-for="i in paddingCount"
      :key="`pad-${i}`"
      aria-hidden="true"
      class="invisible"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * The responsive box-art grid container. Wraps the
 * `grid-cols-3 sm:… md:… lg:… xl:… 2xl:…` class string that was repeated
 * verbatim in the store, library, collections, and shelf views so the
 * column breakpoints stay in one place.
 *
 * `density` picks the gap + column count:
 *   - "default" — the main store/library grids (up to 8 cols, gap-4)
 *   - "compact" — denser contexts like collection shelves (up to 7 cols, gap-3)
 *
 * `itemCount` is optional — when supplied, the grid pads its trailing
 * row with invisible cells so the rendered shape is always a clean
 * rectangle.  Used by the library and store-browse grids to fix the
 * "uneven last row" eyesore on counts that don't divide evenly into
 * the current column count.
 */
const props = withDefaults(
  defineProps<{
    density?: "default" | "compact";
    /** Total number of slotted tiles. Required for the padding feature. */
    itemCount?: number;
  }>(),
  { density: "default", itemCount: 0 },
);

const gridEl = ref<HTMLElement | null>(null);

// Track the current rendered column count by reading
// `grid-template-columns` off the live element. Updates on resize so
// the padding count stays correct across viewport changes.
const cols = ref(1);

function readCols() {
  const el = gridEl.value;
  if (!el) return;
  const tracks = getComputedStyle(el).getPropertyValue(
    "grid-template-columns",
  );
  // grid-template-columns returns space-separated pixel widths
  // (e.g., "234.5px 234.5px 234.5px ..."). Counting tokens gives us
  // the live column count regardless of which Tailwind breakpoint
  // happens to be active.
  const count = tracks.split(/\s+/).filter(Boolean).length;
  if (count > 0) cols.value = count;
}

const paddingCount = computed(() => {
  if (!props.itemCount || cols.value <= 0) return 0;
  const remainder = props.itemCount % cols.value;
  return remainder === 0 ? 0 : cols.value - remainder;
});

const gridClass = computed(() =>
  props.density === "compact"
    ? "grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-7 gap-3"
    : "grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-7 2xl:grid-cols-8 gap-4",
);

onMounted(() => {
  readCols();
  window.addEventListener("resize", readCols);
});

onUnmounted(() => {
  window.removeEventListener("resize", readCols);
});
</script>
