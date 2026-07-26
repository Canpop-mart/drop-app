/**
 * Install-flow state for mods on the store/library game-detail pages.
 *
 * A mod is a Game (type=Mod) with its own versions, so we reuse
 * `fetch_game_version_options` to find its latest build, then queue it through
 * the `download_mod` command, which overlays the mod's files into the parent
 * game's install dir. Unlike a normal game there is no version picker or
 * install-dir choice: mods always install latest onto the (single) parent
 * install.
 *
 * Per-game-detail composable: NOT a singleton — call from a component setup().
 */

import { invoke } from "@tauri-apps/api/core";
import type { VersionOption } from "~/composables/game";

export type InstallableMod = {
  id: string;
};

export function useModInstall(parentGameId: string) {
  // The mod id currently being queued (drives per-row spinner), or null.
  const installingModId = ref<string | null>(null);
  const modError = ref<string | undefined>();

  async function installMod(mod: InstallableMod) {
    modError.value = undefined;
    installingModId.value = mod.id;
    try {
      const versions = await invoke<VersionOption[]>(
        "fetch_game_version_options",
        { gameId: mod.id },
      );
      if (!versions || versions.length === 0) {
        throw new Error("This mod has no downloadable version yet.");
      }
      // Index 0 is the latest version. Placement (where files overlay + the
      // launch override) is declared on the version itself.
      const latest = versions[0];
      await invoke("download_mod", {
        modGameId: mod.id,
        parentGameId,
        versionId: latest.versionId,
        targetPlatform: latest.platform,
        modInstallDir: latest.modInstallDir ?? "",
        launchOverride: latest.launchOverride ?? null,
      });
    } catch (error) {
      console.error("[mod-install] installMod failed:", error);
      modError.value = String(error);
    } finally {
      installingModId.value = null;
    }
  }

  return { installingModId, modError, installMod };
}
