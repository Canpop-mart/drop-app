/**
 * Installed-mods state for the library game-detail "Mods" tab.
 *
 * The source of truth is the client's `.moddata` ledgers under the parent's
 * install dir, surfaced by the `list_installed_mods` command. That includes
 * downloads that never finished (`complete: false`), which can be resumed or
 * removed. Uninstalling a mod (`uninstall_mod`) deletes the files it added and
 * puts back any base-game files it replaced.
 *
 * The backend emits `update_mods/<parent id>` whenever a mod's state on disk
 * changes (download started, finished, failed, cancelled, removed), so the
 * list follows along without a page reload.
 *
 * Per-game-detail composable: NOT a singleton — call from a component setup().
 */

import { invoke } from "@tauri-apps/api/core";
import { useListen } from "~/composables/useListen";

export type InstalledMod = {
  gameId: string;
  version: string;
  fileCount: number;
  complete: boolean;
  downloading: boolean;
  platform: string;
  modInstallDir: string;
  launchOverride: string | null;
  /** See `InstalledModEntry` in mods-tab.ts. */
  launchOverrideSkipped: string | null;
  launchOverrideWinner: string | null;
};

export function useInstalledMods(parentGameId: string) {
  const installedMods = ref<InstalledMod[]>([]);
  const loading = ref(false);
  /** Set when the on-disk list could not be read. The list then keeps what it
   *  last showed rather than pretending nothing is installed. */
  const error = ref<string | null>(null);
  const uninstallingModId = ref<string | null>(null);
  /** Why the last uninstall or resume failed, for the tab to show. */
  const actionError = ref<string | null>(null);

  async function refresh() {
    loading.value = true;
    try {
      installedMods.value = await invoke<InstalledMod[]>(
        "list_installed_mods",
        { parentGameId },
      );
      error.value = null;
    } catch (e) {
      console.error("[installed-mods] list_installed_mods failed:", e);
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  /** Returns true when the mod is gone. On failure `actionError` says why and
   *  the mod stays listed, so the user can try again. */
  async function uninstall(modGameId: string): Promise<boolean> {
    uninstallingModId.value = modGameId;
    actionError.value = null;
    try {
      await invoke("uninstall_mod", { modGameId, parentGameId });
      return true;
    } catch (e) {
      console.error("[installed-mods] uninstall_mod failed:", e);
      actionError.value = String(e);
      return false;
    } finally {
      uninstallingModId.value = null;
      await refresh();
    }
  }

  /** Queue an unfinished install again, exactly as it was started: same
   *  version, platform and placement. It picks up from its ledger. */
  async function resume(modGameId: string): Promise<boolean> {
    const entry = installedMods.value.find((m) => m.gameId === modGameId);
    if (!entry) return false;
    actionError.value = null;
    try {
      await invoke("download_mod", {
        modGameId,
        parentGameId,
        versionId: entry.version,
        targetPlatform: entry.platform,
        modInstallDir: entry.modInstallDir,
        launchOverride: entry.launchOverride,
      });
      await refresh();
      return true;
    } catch (e) {
      console.error("[installed-mods] resume failed:", e);
      actionError.value = String(e);
      return false;
    }
  }

  useListen(`update_mods/${parentGameId}`, () => {
    refresh();
  });

  return {
    installedMods,
    loading,
    error,
    uninstallingModId,
    actionError,
    refresh,
    uninstall,
    resume,
  };
}
