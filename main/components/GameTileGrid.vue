<template>
  <div :class="gridClass">
    <slot />
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
 */
const props = withDefaults(
  defineProps<{ density?: "default" | "compact" }>(),
  { density: "default" },
);

const gridClass = computed(() =>
  props.density === "compact"
    ? "grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-7 gap-3"
    : "grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-7 2xl:grid-cols-8 gap-4",
);
</script>
