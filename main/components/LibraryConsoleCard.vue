<template>
  <NuxtLink
    :to="`/library/console/${id}`"
    class="group relative block aspect-video w-64 shrink-0 overflow-hidden rounded-xl ring-1 ring-zinc-800/60 transition-all hover:ring-blue-500/40"
  >
    <!-- Diagonal collage of the console's game covers, same treatment as
         collection cards so the two rows feel of a piece. -->
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
      class="absolute inset-0 bg-zinc-950/55 transition-colors group-hover:bg-zinc-950/35"
    />

    <!-- Console label — name, maker, and the count of games you have. -->
    <div class="absolute inset-0 flex flex-col items-center justify-center p-3">
      <span
        class="rounded-lg bg-white/90 px-4 py-2 text-center text-base font-bold uppercase tracking-wide text-zinc-900 shadow-lg backdrop-blur-sm"
      >
        {{ name }}
      </span>
      <span
        class="mt-2 text-xs font-medium text-zinc-200/90"
        :class="maker ? '' : 'sr-only'"
      >
        {{ maker }}
      </span>
      <span class="mt-0.5 text-xs text-zinc-300/80">
        {{ count }} game{{ count === 1 ? "" : "s" }}
      </span>
    </div>
  </NuxtLink>
</template>

<script setup lang="ts">
const props = defineProps<{
  id: string;
  name: string;
  maker: string;
  count: number;
  /** Object ids of this console's game covers (already filtered non-null). */
  covers: string[];
}>();

// Up to 6 covers; repeat a small set so the collage still fills the card.
const displayCovers = computed(() => {
  const base = props.covers.slice(0, 6);
  if (base.length === 0) return [];
  const out = [...base];
  while (out.length < 6) out.push(...base);
  return out.slice(0, 6);
});
</script>
