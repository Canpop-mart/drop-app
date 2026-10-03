/**
 * Which games have an in-place update in the download queue.
 *
 * The queue carries an update exactly like an install (the item's own status
 * says "Queued" / "Downloading"), so labels that should say "Updating" or
 * "Updated" need to know. Two sources mark a game:
 *   - this client queued it (`apply_game_update` returned Ok), and
 *   - the game's status event showed the update agent's `Updating` state
 *     (game.ts), which also covers an update queued from the other surface.
 * A mark is cleared when the queue finishes it (`download_complete`, which
 * also records the entry as an update), when it fails (`download_error`
 * while it is at the front of the queue), or when the game later shows a
 * plain `Downloading` state, which only a fresh install produces.
 *
 * Module-level state like the game registry in game.ts: no Nuxt context is
 * needed, so the Tauri event callbacks can use it.
 */
import { ref } from "vue";

const updating = ref<Set<string>>(new Set());

export function markUpdating(gameId: string) {
  if (updating.value.has(gameId)) return;
  const next = new Set(updating.value);
  next.add(gameId);
  updating.value = next;
}

export function clearUpdating(gameId: string) {
  if (!updating.value.has(gameId)) return;
  const next = new Set(updating.value);
  next.delete(gameId);
  updating.value = next;
}

export function isUpdating(gameId: string): boolean {
  return updating.value.has(gameId);
}
