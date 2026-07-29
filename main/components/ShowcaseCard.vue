<!--
  A single showcase card. Switches on the item type (game cover / achievement /
  game stats / custom text) and tints from the inherited profile accent vars.
  Game + achievement cards are clickable (emit `select-game`); stat + custom are
  static. Uniform aspect so a mixed set grids/rails cleanly.
-->
<template>
  <!-- Favourite / pinned game -->
  <button
    v-if="item.type === 'FavoriteGame' && item.game"
    class="scard group relative aspect-[3/4] overflow-hidden rounded-xl bg-zinc-800 text-left ring-1 ring-zinc-700/50 transition-all"
    @click="item.game && $emit('select-game', item.game.id)"
  >
    <img
      v-if="item.game.mCoverObjectId"
      :src="objectUrl(item.game.mCoverObjectId)"
      class="h-full w-full object-cover"
      loading="lazy"
    />
    <div
      v-else
      class="flex h-full w-full items-center justify-center px-2 text-center text-xs text-zinc-500"
    >
      {{ item.game.mName }}
    </div>
    <div
      class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 to-transparent p-2.5"
    >
      <p class="truncate text-xs font-semibold text-white">
        {{ item.game.mName }}
      </p>
    </div>
  </button>

  <!-- Achievement -->
  <button
    v-else-if="item.type === 'Achievement' && item.achievement"
    class="scard flex aspect-[3/4] flex-col rounded-xl p-3 text-left"
    :style="tintStyle"
    @click="item.game && $emit('select-game', item.game.id)"
  >
    <span
      class="text-[9px] font-semibold uppercase tracking-[0.1em]"
      :style="{ color: 'var(--accent)' }"
    >
      Achievement
    </span>
    <img
      v-if="item.achievement.iconUrl"
      :src="item.achievement.iconUrl"
      class="mt-3 size-12 rounded-lg"
      referrerpolicy="no-referrer"
    />
    <p class="mt-auto truncate text-sm font-bold text-zinc-100">
      {{ item.achievement.title }}
    </p>
    <p v-if="item.game" class="truncate text-[11px] text-zinc-400">
      {{ item.game.mName }}
    </p>
  </button>

  <!-- Game stats (computed server-side) -->
  <div
    v-else-if="item.type === 'GameStats' && item.gameStats"
    class="scard flex aspect-[3/4] flex-col rounded-xl p-3.5"
    :style="tintStyle"
  >
    <span
      class="truncate text-[9px] font-semibold uppercase tracking-[0.1em]"
      :style="{ color: 'var(--accent)' }"
    >
      {{ item.game ? item.game.mName : "Stats" }}
    </span>
    <div
      class="mt-3 text-2xl font-extrabold leading-none tabular-nums"
      :style="{ color: 'var(--accent)' }"
    >
      {{ formatPlaytime(item.gameStats.playtimeSeconds) }}
    </div>
    <p class="mt-1 text-[11px] text-zinc-400">playtime</p>
    <div class="mt-auto text-lg font-bold tabular-nums text-zinc-100">
      {{ item.gameStats.achievementsUnlocked }} /
      {{ item.gameStats.achievementsTotal }}
    </div>
    <p class="text-[11px] text-zinc-400">achievements</p>
  </div>

  <!-- Custom text card -->
  <div
    v-else-if="item.type === 'Custom'"
    class="scard flex aspect-[3/4] flex-col rounded-xl p-3.5"
    :style="{
      background: 'linear-gradient(150deg, var(--accent-soft), transparent)',
      border: '1px solid var(--accent-border)',
    }"
  >
    <span
      class="text-[9px] font-semibold uppercase tracking-[0.1em]"
      :style="{ color: 'var(--accent)' }"
    >
      About
    </span>
    <p class="mt-auto text-sm font-semibold leading-snug text-zinc-100">
      {{ item.title }}
    </p>
  </div>
</template>

<script setup lang="ts">
import type { ShowcaseItem } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { formatPlaytime } from "~/composables/use-recent-games";

defineProps<{ item: ShowcaseItem }>();
defineEmits<{ (e: "select-game", gameId: string): void }>();

const tintStyle = {
  background: "var(--accent-soft)",
  border: "1px solid var(--accent-border)",
};

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}
</script>

<style scoped>
button.scard {
  transition: transform 0.15s ease;
}
button.scard:hover {
  transform: translateY(-3px);
  --tw-ring-color: var(--accent-border);
}
</style>
