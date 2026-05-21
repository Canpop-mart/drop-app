<template>
  <GameTileGrid>
    <GameTile
      v-for="entry in entries"
      :key="entry.game.id"
      :cover-url="entry.game.mCoverObjectId ? useObject(entry.game.mCoverObjectId) : null"
      :name="entry.game.mName"
      :installed="entry.installed"
      :update-available="entry.updateAvailable"
      @select="$emit('select', entry.game.id)"
    />
  </GameTileGrid>
</template>

<script setup lang="ts">
/**
 * Reusable tile grid for library views. Composes the shared `GameTile` +
 * `GameTileGrid` so the same look can be dropped into multiple sections
 * (Installed / Not installed / search results / "Recently played")
 * without duplication.
 *
 * Kept deliberately presentational — fetching, sorting, and filtering
 * live in the parent. Click events bubble up via `select` so the parent
 * decides what "open this game" means.
 */
import type { Game, GameStatus } from "~/types";

export interface LibraryGridEntry {
  game: Game;
  status: GameStatus | null;
  installed: boolean;
  updateAvailable: boolean;
}

defineProps<{
  entries: LibraryGridEntry[];
}>();

defineEmits<{
  (e: "select", gameId: string): void;
}>();
</script>
