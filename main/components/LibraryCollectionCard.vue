<template>
  <NuxtLink
    :to="`/library/collection/${id}`"
    class="group relative block aspect-video w-64 shrink-0 overflow-hidden rounded-xl ring-1 ring-zinc-800/60 transition-all hover:ring-blue-500/40"
  >
    <!-- Diagonal collage of the collection's covers — oversized + rotated so it
         fills the card; the card clips the overflow. -->
    <div
      v-if="displayCovers.length > 0"
      class="pointer-events-none absolute -inset-10 flex items-center justify-center gap-2 transition-transform duration-700 group-hover:scale-105"
      style="transform: rotate(-20deg)"
    >
      <img
        v-for="(cover, i) in displayCovers"
        :key="i"
        :src="useObject(cover)"
        alt=""
        class="h-48 w-28 shrink-0 rounded-md object-cover shadow-lg"
      />
    </div>
    <div
      v-else
      class="absolute inset-0 bg-gradient-to-br from-zinc-800 to-zinc-900"
    />

    <!-- Darkening so the label reads; lifts a touch on hover. -->
    <div
      class="absolute inset-0 bg-zinc-950/45 transition-colors group-hover:bg-zinc-950/25"
    />

    <!-- Centered frosted label. -->
    <div class="absolute inset-0 flex items-center justify-center p-3">
      <span
        class="rounded-lg bg-white/90 px-4 py-2 text-center text-sm font-bold uppercase tracking-wide text-zinc-900 shadow-lg backdrop-blur-sm"
      >
        {{ name }}
      </span>
    </div>
  </NuxtLink>
</template>

<script setup lang="ts">
const props = defineProps<{
  id: string;
  name: string;
  /** Object ids of the collection's game covers (already filtered non-null). */
  covers: string[];
}>();

// Show up to 6 covers; if the collection is small, repeat them so the collage
// still fills the card rather than leaving a big empty wedge.
const displayCovers = computed(() => {
  const base = props.covers.slice(0, 6);
  if (base.length === 0) return [];
  const out = [...base];
  while (out.length < 5) out.push(...base);
  return out.slice(0, 6);
});
</script>
