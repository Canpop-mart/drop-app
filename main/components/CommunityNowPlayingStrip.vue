<template>
  <section v-if="entries.length > 0" class="mb-8">
    <div class="flex items-baseline gap-2 mb-3">
      <h2 class="text-lg font-display font-semibold text-zinc-100">
        Around right now
      </h2>
      <span class="relative flex size-2.5">
        <span
          class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"
        />
        <span class="relative inline-flex rounded-full size-2.5 bg-emerald-500" />
      </span>
      <span class="text-xs text-zinc-500">
        {{ entries.length }} {{ entries.length === 1 ? "player" : "players" }}
      </span>
    </div>

    <div
      class="flex gap-3 overflow-x-auto pb-2 -mx-1 px-1"
      style="scrollbar-width: thin"
    >
      <button
        v-for="entry in entries"
        :key="`${entry.userId}-${entry.startedAt}`"
        class="shrink-0 group flex items-center gap-3 rounded-xl bg-emerald-500/5 ring-1 ring-emerald-500/20 hover:ring-emerald-400/50 hover:bg-emerald-500/10 transition-all p-2.5 pr-4"
        @click="$emit('go-to-game', entry.game.id)"
      >
        <div class="relative shrink-0">
          <img
            v-if="entry.avatarObjectId"
            :src="objectUrl(entry.avatarObjectId)"
            class="size-10 rounded-full object-cover ring-2 ring-emerald-400/60"
          />
          <div
            v-else
            class="size-10 rounded-full bg-zinc-700 flex items-center justify-center ring-2 ring-emerald-400/60"
          >
            <span class="text-xs font-bold text-zinc-400">
              {{ entry.displayName[0]?.toUpperCase() }}
            </span>
          </div>
          <span
            class="absolute -bottom-0.5 -right-0.5 size-3 rounded-full bg-emerald-500 ring-2 ring-zinc-950"
          />
        </div>

        <img
          v-if="entry.game.coverObjectId"
          :src="objectUrl(entry.game.coverObjectId)"
          class="h-12 w-9 rounded object-cover shrink-0"
          loading="lazy"
        />

        <div class="text-left min-w-0 max-w-[10rem]">
          <p class="text-xs text-zinc-400 truncate">
            {{ entry.displayName }}
          </p>
          <p
            class="text-sm font-medium text-zinc-100 truncate group-hover:text-emerald-300 transition-colors"
          >
            {{ entry.game.name }}
          </p>
        </div>
      </button>
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
