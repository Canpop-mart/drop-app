<template>
  <section
    v-if="entries.length > 0"
    class="flex items-center gap-3 rounded-xl bg-zinc-900/40 ring-1 ring-zinc-800/60 px-4 py-2.5"
  >
    <!-- Label cluster: pulsing dot + small uppercase eyebrow. The
         standalone "X players" count from the old card has been dropped —
         each player is now visible inline so the headcount is implicit. -->
    <div class="flex items-center gap-1.5 shrink-0">
      <span class="size-1.5 rounded-full bg-green-500 pulse-dot" />
      <span
        class="text-[11px] tracking-[0.15em] uppercase text-zinc-400 font-medium"
        >Around now</span
      >
    </div>
    <div class="w-px h-4 bg-zinc-800 shrink-0" />

    <!-- Player list — horizontal scroll on overflow so a busy server
         doesn't push the page wider. -->
    <div
      class="flex items-center gap-2 min-w-0 overflow-x-auto"
      style="scrollbar-width: none"
    >
      <template v-for="(entry, i) in entries" :key="`${entry.userId}-${entry.startedAt}`">
        <button
          class="flex items-center gap-1.5 shrink-0 group"
          @click="$emit('go-to-game', entry.game.id)"
        >
          <img
            v-if="entry.avatarObjectId"
            :src="objectUrl(entry.avatarObjectId)"
            class="size-5 rounded-full object-cover"
          />
          <div
            v-else
            class="size-5 rounded-full bg-emerald-700/60 flex items-center justify-center text-[9px] font-bold text-zinc-100"
          >
            {{ entry.displayName[0]?.toUpperCase() }}
          </div>
          <span class="text-xs font-medium text-zinc-200">{{
            entry.displayName
          }}</span>
          <span class="text-xs text-zinc-500">in</span>
          <span
            class="text-xs text-blue-400 group-hover:text-blue-300 transition-colors truncate max-w-[14rem]"
            >{{ entry.game.name }}</span
          >
        </button>
        <span v-if="i < entries.length - 1" class="text-zinc-700 text-xs">·</span>
      </template>
    </div>
  </section>
</template>

<script setup lang="ts">
import { serverUrl } from "~/composables/use-server-fetch";
import type { NowPlayingEntry } from "~/composables/use-server-api";

defineProps<{
  entries: NowPlayingEntry[];
}>();

defineEmits<{
  (e: "go-to-game", gameId: string): void;
}>();

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}
</script>

<style scoped>
.pulse-dot {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}
@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

/* Hide horizontal scrollbar but keep the scroll behavior */
.overflow-x-auto::-webkit-scrollbar {
  display: none;
}
</style>
