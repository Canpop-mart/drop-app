<template>
  <section v-if="games.length > 0" class="mb-10">
    <h3 class="text-lg font-display font-semibold text-zinc-100 mb-4">
      {{ title }}
    </h3>
    <GameTileGrid>
      <GameTile
        v-for="game in games.slice(0, max)"
        :key="game.id"
        :cover-url="game.mCoverObjectId ? objectUrl(game.mCoverObjectId) : null"
        :name="game.mName"
        :rom="game.isEmulated"
        :update-available="game.updateAvailable ?? false"
        @select="$emit('select', game.id)"
      />
    </GameTileGrid>
  </section>
</template>

<script setup lang="ts">
/**
 * One horizontal "shelf" of store tiles — used for Featured-tab sections
 * like "Most Played This Week" and "Recently Added". Composes the shared
 * `GameTile` so the tile shape stays identical to the Browse grid.
 */
import type { StoreGame, TrendingGame } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

defineProps<{
  title: string;
  games: (StoreGame | TrendingGame)[];
  /** Cap to keep the shelf to exactly one or two rows wide. */
  max?: number;
}>();

defineEmits<{
  (e: "select", gameId: string): void;
}>();

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}
</script>
