import { invoke } from "@tauri-apps/api/core";
import type { VersionOption } from "~/composables/game";
import { quickInstallArgs } from "./quick-install-logic";

/** Placeholder copy for the owner to rewrite. */
export const NO_DOWNLOADABLE_VERSIONS =
  "No downloadable versions available for this game.";

/**
 * Queue the default version of a game for install (see quick-install-logic).
 * Pass `options` when the caller already fetched them. Throws with a
 * readable message on failure, so callers can show it.
 */
export async function quickInstallGame(
  gameId: string,
  options?: readonly VersionOption[] | null,
): Promise<void> {
  const list =
    options && options.length > 0
      ? options
      : await invoke<VersionOption[]>("fetch_game_version_options", { gameId });
  const args = quickInstallArgs(list ?? []);
  if (!args) throw new Error(NO_DOWNLOADABLE_VERSIONS);
  await invoke("download_game", { gameId, ...args });
}
