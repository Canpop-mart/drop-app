<template>
  <NuxtLink
    :to="`/bigpicture/library/${game.id}`"
    class="group relative flex flex-col rounded-xl overflow-hidden transition-all duration-200 outline-none"
    :class="[
      'ring-2 ring-transparent',
      'focus-visible:ring-blue-500 focus-visible:scale-105 focus-visible:shadow-xl focus-visible:shadow-blue-500/20',
      'bp-focused:ring-blue-500 bp-focused:scale-105 bp-focused:shadow-xl bp-focused:shadow-blue-500/20',
    ]"
  >
    <!-- Cover image -->
    <div class="relative aspect-[3/4] bg-zinc-800">
      <img
        v-if="imageObjectId"
        :src="objectUrl(imageObjectId)"
        :alt="game.mName"
        class="w-full h-full object-cover"
        loading="lazy"
      />
      <div
        v-if="!imageObjectId"
        class="w-full h-full flex items-center justify-center"
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
    <div class="px-2 py-2 bg-zinc-900/80">
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
}>();

// Prefer cover art, fall back to icon if cover is empty
const imageObjectId = computed(
  () => props.game.mCoverObjectId || props.game.mIconObjectId || "",
);

const isInstalled = computed(() => props.status.type === "Installed");

const isRunning = computed(() => props.status.type === "Running");
</script>

<style scoped>
/* Scoped style for the bp-focused class applied by focus-navigation */
:deep(.bp-focused) {
  --tw-ring-color: rgb(59 130 246);
  --tw-ring-offset-shadow: var(--tw-ring-inset) 0 0 0
    var(--tw-ring-offset-width) var(--tw-ring-offset-color);
  --tw-ring-shadow: var(--tw-ring-inset) 0 0 0
    calc(2px + var(--tw-ring-offset-width)) var(--tw-ring-color);
  box-shadow:
    var(--tw-ring-offset-shadow), var(--tw-ring-shadow),
    var(--tw-shadow, 0 0 #0000);
  transform: scale(1.05);
}
</style>
