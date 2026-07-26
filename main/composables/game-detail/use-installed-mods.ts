/**
 * Installed-mods state for the library game-detail "Mods" tab.
 *
 * The source of truth is the client's `.moddata` ledgers under the parent's
 * install dir, surfaced by the `list_installed_mods` command. Uninstalling a
 * mod removes only that mod's overlay files (via `uninstall_mod`), never the
 * base game.
 *
 * Per-game-detail composable: NOT a singleton — call from a component setup().
 */

import { invoke } from "@tauri-apps/api/core";

export type InstalledMod = {
  gameId: string;
  version: string;
  fileCount: number;
};

export function useInstalledMods(parentGameId: string) {
  const installedMods = ref<InstalledMod[]>([]);
  const loading = ref(false);
  const uninstallingModId = ref<string | null>(null);

  async function refresh() {
    loading.value = true;
    try {
      installedMods.value = await invoke<InstalledMod[]>(
        "list_installed_mods",
        { parentGameId },
      );
      console.log(
        `[installed-mods] ${parentGameId}: ${installedMods.value.length} installed`,
        installedMods.value.map((m) => m.gameId),
      );
    } catch (error) {
      console.error("[installed-mods] list_installed_mods failed:", error);
      installedMods.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function uninstall(modGameId: string) {
    uninstallingModId.value = modGameId;
    try {
      await invoke("uninstall_mod", { modGameId, parentGameId });
      await refresh();
    } catch (error) {
      console.error("[installed-mods] uninstall_mod failed:", error);
    } finally {
      uninstallingModId.value = null;
    }
  }

  return { installedMods, loading, uninstallingModId, refresh, uninstall };
}
