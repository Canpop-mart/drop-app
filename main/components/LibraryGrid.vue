<template>
  <!-- Compact (list) mode renders rows in a single column so each title
       reads top-to-bottom; default (cover) mode keeps the box-art tile
       grid the page has historically used. -->
  <div
    v-if="compact"
    class="flex flex-col gap-0.5 rounded-xl bg-zinc-800/30 ring-1 ring-zinc-700/40 p-1"
  >
    <GameTile
      v-for="entry in entries"
      :key="entry.game.id"
      compact
      :cover-url="entry.game.mCoverObjectId ? useObject(entry.game.mCoverObjectId) : null"
      :name="entry.game.mName"
      :installed="entry.installed"
      :update-available="entry.updateAvailable"
      :last-played="lastPlayedMap?.get(entry.game.id) ?? null"
      :hover-action="hoverActionFor(entry)"
      @select="$emit('select', entry.game.id)"
    />
  </div>
  <GameTileGrid v-else :item-count="entries.length">
    <GameTile
      v-for="entry in entries"
      :key="entry.game.id"
      :cover-url="entry.game.mCoverObjectId ? useObject(entry.game.mCoverObjectId) : null"
      :name="entry.game.mName"
      :installed="entry.installed"
      :update-available="entry.updateAvailable"
      :last-played="lastPlayedMap?.get(entry.game.id) ?? null"
      :hover-action="hoverActionFor(entry)"
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

const props = defineProps<{
  entries: LibraryGridEntry[];
  /**
   * Switch the grid into compact list mode — single-column rows with a
   * small cover thumb. Hooks up to the library page's density toggle.
   */
  compact?: boolean;
  /**
   * Optional map of gameId → ISO timestamp for the last play session.
   * When present, each tile shows a "Played X ago" meta line so the
   * user can prioritise recently-touched games at a glance.
   */
  lastPlayedMap?: Map<string, string>;
  /**
   * When true, every tile renders a hover overlay — "Play" for installed
   * games, "Install" for not-yet-installed ones — so the user can act
   * without drilling into the detail page. Disabled by default (the
   * collections-management and search-result variants want plain
   * navigation).
   */
  showHoverAction?: boolean;
}>();

defineEmits<{
  (e: "select", gameId: string): void;
}>();

function hoverActionFor(
  entry: LibraryGridEntry,
): "play" | "install" | null {
  if (!props.showHoverAction) return null;
  return entry.installed ? "play" : "install";
}
</script>
