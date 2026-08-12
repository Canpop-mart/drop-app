<template>
  <NuxtLink
    :to="`/bigpicture/library/${game.id}`"
    class="group relative flex flex-col rounded-xl transition-all duration-200 outline-none"
    :class="[
      'ring-2 ring-transparent',
      'focus-visible:ring-blue-500 focus-visible:shadow-xl focus-visible:shadow-blue-500/20',
    ]"
  >
    <!-- Cover slot is a fixed 3:4 box so every tile in the grid is the same
         height. Sources disagree wildly (Steam writes 2:3, IGDB 264x374,
         manual import falls back to the 512x512 square icon), so the image
         is cropped rather than stretched: `object-top` keeps the title
         artwork, which sits at the top of nearly every box art. -->
    <div
      class="bp-focus-ring relative bg-zinc-800 rounded-xl overflow-hidden aspect-[3/4]"
    >
      <img
        v-if="imageObjectId"
        :src="objectUrl(imageObjectId)"
        :alt="game.mName"
        class="w-full h-full object-cover object-top"
        loading="lazy"
        decoding="async"
      />
      <div
        v-if="!imageObjectId"
        class="absolute inset-0 flex items-center justify-center"
      >
        <span class="text-2xl font-bold text-zinc-500">
          {{ game.mName[0] }}
        </span>
      </div>

      <!-- Installed / running indicators. `z-10` keeps them above the box-art
           frame below (which paints at z-index 2 and is full-bleed): the
           cartridge templates are opaque at this corner, so without it an
           installed game reads as not installed. -->
      <div
        v-if="isInstalled"
        class="absolute top-2 right-2 z-10 size-3 rounded-full bg-green-500 ring-2 ring-zinc-900"
      />

      <div
        v-if="isRunning"
        class="absolute top-2 right-2 z-10 size-3 rounded-full bg-blue-500 ring-2 ring-zinc-900 animate-pulse"
      />

      <!-- Bottom gradient -->
      <div
        class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent"
      />

      <!-- Console box-art frame (cartridge / DVD case template). The ten BPM
           home themes used to draw this themselves on their own grids; the
           merged home has one grid, so the frame rides on the tile instead.
           Off unless the caller passes a theme, so nothing else changes. -->
      <BpmBoxArtOverlay v-if="overlayThemeId" :theme-id="overlayThemeId" />
    </div>

    <!-- Title -->
    <div v-if="!hideTitles" class="px-2 py-1.5">
      <p class="text-sm font-medium text-zinc-200 truncate">
        {{ game.mName }}
      </p>
    </div>
  </NuxtLink>
</template>

<script setup lang="ts">
import type { Game, GameStatus } from "~/types";
import { objectImageUrl } from "~/composables/use-object";
import BpmBoxArtOverlay from "~/components/bigpicture/BpmBoxArtOverlay.vue";

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

const props = defineProps<{
  game: Game;
  status: GameStatus;
  hideTitles?: boolean;
  /** BPM theme id to draw a box-art frame for. Omit for a bare cover. */
  overlayThemeId?: string;
}>();

// Prefer cover art, fall back to icon if cover is empty
const imageObjectId = computed(
  () => props.game.mCoverObjectId || props.game.mIconObjectId || "",
);

const isInstalled = computed(() => props.status.type === "Installed");

const isRunning = computed(() => props.status.type === "Running");
</script>

<style scoped>
/* Focus glow is now handled by bp-focus-delegate / bp-focus-ring in main.scss */
</style>
