<template>
  <NuxtLink
    :to="`/library/console/${id}`"
    class="group relative flex h-28 w-52 shrink-0 flex-col items-center justify-center overflow-hidden rounded-xl bg-gradient-to-b from-zinc-900 to-zinc-950 px-4 py-3 ring-1 ring-zinc-800 transition-all hover:-translate-y-0.5 hover:ring-blue-500/40"
  >
    <!-- Console render. Pixel-art renders stay crisp when scaled. -->
    <img
      v-if="art"
      :src="art.render"
      :alt="art.name"
      class="mb-2 w-auto max-w-[80%] object-contain drop-shadow-md transition-transform duration-300 group-hover:scale-105"
      :class="art.big ? 'h-10' : 'h-12'"
      :style="art.pixel ? 'image-rendering: pixelated' : ''"
    />

    <!-- Fallback for consoles we have no render for: a rotated cover collage. -->
    <template v-else>
      <div
        class="pointer-events-none absolute -inset-8 flex items-center justify-center gap-2 opacity-50 transition-transform duration-700 group-hover:scale-105"
        style="transform: rotate(-18deg)"
      >
        <img
          v-for="(cover, i) in displayCovers"
          :key="i"
          :src="useObject(cover)"
          alt=""
          class="h-32 w-20 shrink-0 rounded object-cover"
        />
      </div>
      <div class="absolute inset-0 bg-zinc-950/55" />
    </template>

    <div class="relative text-center">
      <!-- Official logo, in its brand colours, straight on the dark card. The
           few dark / monochrome logos (whiten) get inverted to white so they
           read; text fallback when there's no logo. -->
      <img
        v-if="art?.logo"
        :src="art.logo"
        :alt="art.name"
        class="mx-auto mb-1 w-auto max-w-[9rem] object-contain"
        :class="art.big ? 'h-8' : 'h-6'"
        :style="
          art.whiten
            ? 'filter: brightness(0) invert(1) drop-shadow(0 1px 1px rgba(0,0,0,0.4))'
            : 'filter: drop-shadow(0 1px 2px rgba(0,0,0,0.5))'
        "
      />
      <div
        v-else
        class="font-display text-sm font-bold uppercase tracking-wide text-zinc-100"
      >
        {{ displayName }}
      </div>
      <div class="text-[11px] text-zinc-400">
        {{ count }} game{{ count === 1 ? "" : "s" }}
      </div>
    </div>
  </NuxtLink>
</template>

<script setup lang="ts">
import { consoleArt } from "~/composables/console-art";

const props = defineProps<{
  id: string;
  name: string;
  count: number;
  /** Object ids of this console's game covers — only used for the fallback. */
  covers: string[];
}>();

const art = computed(() => consoleArt(props.name));
const displayName = computed(() => art.value?.name ?? props.name);

// Fallback collage: up to 4 covers, repeated to fill if the console is small.
const displayCovers = computed(() => {
  const base = props.covers.slice(0, 4);
  if (base.length === 0) return [];
  const out = [...base];
  while (out.length < 4) out.push(...base);
  return out.slice(0, 4);
});
</script>
