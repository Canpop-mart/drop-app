/**
 * Install-flow state for the library game-detail page: the version picker,
 * install-directory selection, optional-dependency toggles, and the
 * `download_game` invocations that queue everything.
 *
 * Extracted from `pages/library/[id]/index.vue`. The install modal markup
 * lives in `components/game-detail/GameInstallModal.vue`; this composable
 * is the state + behaviour behind it.
 *
 * Per-game-detail composable: NOT a singleton — call from a component
 * `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import type { Game } from "~/types";
import type { VersionOption } from "~/composables/game";

export function useGameInstall(game: Game) {
  const installFlowOpen = ref(false);
  // `undefined` = still loading; `null` / `[]` = none available (error UI).
  const versionOptions = ref<Array<VersionOption> | undefined | null>();
  const installDirs = ref<Array<string> | undefined>();
  const installLoading = ref(false);
  const installError = ref<string | undefined>();
  const installVersionIndex = ref(-1);
  const installDir = ref(0);
  const installDepsDisabled = ref<Record<string, boolean>>({});

  /** The version the user currently has selected (`-1` → "Latest" → index 0). */
  const currentVersionOption = computed(
    () => versionOptions.value?.[Math.max(installVersionIndex.value, 0)],
  );

  /** Open the modal and load version options + install dirs. */
  async function openInstallFlow() {
    installFlowOpen.value = true;
    versionOptions.value = undefined;
    installDirs.value = undefined;
    installError.value = undefined;

    try {
      versionOptions.value = await invoke("fetch_game_version_options", {
        gameId: game.id,
      });
      installDirs.value = await invoke("fetch_download_dir_stats");
    } catch (error) {
      installError.value = String(error);
      versionOptions.value = null;
    }
  }

  /** Human-readable label for a version option (used by the picker). */
  function formatVersionOptionText(index: number): string | undefined {
    if (!versionOptions.value) return undefined;
    const versionOption =
      versionOptions.value[Math.max(index, 0)];
    const template = `${
      versionOption.displayName || versionOption.versionPath
    } on ${versionOption.platform}, ${formatKilobytes(
      versionOption.size.installSize / 1024,
    )}B`;
    return index === -1 ? `Latest (${template})` : template;
  }

  /** Queue the selected version (and any enabled dependencies) for download. */
  async function install() {
    try {
      if (!versionOptions.value) {
        throw new Error("Versions have not been loaded");
      }
      installLoading.value = true;
      const versionOption =
        versionOptions.value[Math.max(installVersionIndex.value, 0)];
      const isLatest = installVersionIndex.value === -1;

      const downloads = [
        { gameId: game.id, versionId: versionOption.versionId },
        ...versionOption.requiredContent
          .filter((v) => !installDepsDisabled.value[v.versionId])
          .map((v) => ({ gameId: v.gameId, versionId: v.versionId })),
      ];

      for (const dl of downloads) {
        await invoke("download_game", {
          gameId: dl.gameId,
          versionId: dl.versionId,
          installDir: installDir.value,
          targetPlatform: versionOption.platform,
          enableUpdates: isLatest,
        });
      }

      installFlowOpen.value = false;
    } catch (error) {
      installError.value = String(error);
    } finally {
      installLoading.value = false;
    }
  }

  return {
    installFlowOpen,
    versionOptions,
    installDirs,
    installLoading,
    installError,
    installVersionIndex,
    installDir,
    installDepsDisabled,
    currentVersionOption,
    openInstallFlow,
    formatVersionOptionText,
    install,
  };
}
