<template>
  <NuxtLink
    :to="`/bigpicture/library/${game.id}`"
    class="group relative flex flex-col rounded-xl transition-all duration-200 outline-none"
    :class="[
      'ring-2 ring-transparent',
      'focus-visible:ring-blue-500 focus-visible:shadow-xl focus-visible:shadow-blue-500/20',
    ]"
  >
    <!-- Cover image — natural aspect ratio, no forced 3:4 -->
    <div class="bp-focus-ring relative bg-zinc-800 rounded-xl overflow-hidden">
      <img
        v-if="imageObjectId"
        :src="objectUrl(imageObjectId)"
        :alt="game.mName"
        class="w-full block"
        loading="lazy"
      />
      <div
        v-if="!imageObjectId"
        class="w-full aspect-[3/4] flex items-center justify-center"
      >
        <span class="text-2xl font-bold text-zinc-500">
          {{ game.mName[0] }}
        </span>
      </div>

      <!-- Installed indicator -->
      <div
        v-if="isInstalled"
        class="absolute top-2 right-2 size-3 rounded-full bg-green-500 ring-2 ring-zinc-900"
      />

      <!-- Running indicator -->
      <div
        v-if="isRunning"
        class="absolute top-2 right-2 size-3 rounded-full bg-blue-500 ring-2 ring-zinc-900 animate-pulse"
      />

      <!-- Bottom gradient -->
      <div
        class="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-zinc-900/90 to-transparent"
      />
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
import { serverUrl } from "~/composables/use-server-fetch";

/** Load object images via server:// protocol (object:// doesn't work in dev) */
function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

const props = defineProps<{
  game: Game;
  status: GameStatus;
  hideTitles?: boolean;
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
