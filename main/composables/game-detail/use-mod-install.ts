/**
 * Install-flow state for mods on the store/library game-detail pages.
 *
 * A mod is a Game (type=Mod) with its own versions, so we reuse
 * `fetch_game_version_options` to find its latest build, then queue it through
 * the `download_mod` command, which overlays the mod's files into the parent
 * game's install dir. Installing a mod also pulls in its prerequisite mods
 * (e.g. StardewArchipelago needs SMAPI): required mods are resolved
 * recursively, any that are missing get installed onto the same parent first,
 * then the target mod is installed.
 *
 * Per-game-detail composable: NOT a singleton — call from a component setup().
 */

import { invoke } from "@tauri-apps/api/core";
import type { VersionOption } from "~/composables/game";

export type InstallableMod = {
  id: string;
};

/** One mod resolved to its latest version, ready to queue. */
type PlannedMod = { id: string; name: string; version: VersionOption };

/**
 * Asks the user whether to pull in prerequisite mods. Injectable because the
 * default implementation is the desktop `createModal` stack, which Big Picture
 * cannot navigate with a controller — BPM passes its own dialog instead.
 */
export type PrereqConfirm = (prereqNames: string[]) => Promise<boolean>;

export function useModInstall(
  parentGameId: string,
  opts?: { confirmPrereqs?: PrereqConfirm },
) {
  // The mod id currently being queued (drives per-row spinner), or null.
  const installingModId = ref<string | null>(null);
  const modError = ref<string | undefined>();

  /**
   * Resolve a mod and its required mods into an ordered install plan,
   * prerequisites first (so a loader like SMAPI is queued before the mod that
   * needs it). Skips mods already installed on this parent and guards against
   * dependency cycles.
   */
  async function buildInstallPlan(rootModId: string): Promise<PlannedMod[]> {
    const installed = await invoke<Array<{ gameId: string }>>(
      "list_installed_mods",
      { parentGameId },
    );
    const installedIds = new Set(installed.map((m) => m.gameId));

    const plan: PlannedMod[] = [];
    const planned = new Set<string>();
    const visiting = new Set<string>();

    async function visit(modId: string, displayName?: string) {
      if (planned.has(modId) || installedIds.has(modId) || visiting.has(modId))
        return;
      visiting.add(modId);

      const versions = await invoke<VersionOption[]>(
        "fetch_game_version_options",
        { gameId: modId },
      );
      if (!versions || versions.length === 0) {
        throw new Error(
          `"${displayName ?? modId}" has no downloadable version for this platform.`,
        );
      }
      // Index 0 is the latest version. Resolve prerequisites depth-first so they
      // land in the plan ahead of this mod.
      const latest = versions[0];
      for (const req of latest.requiredMods ?? []) {
        await visit(req.gameId, req.name);
      }

      visiting.delete(modId);
      planned.add(modId);
      plan.push({ id: modId, name: displayName ?? modId, version: latest });
    }

    await visit(rootModId);
    return plan;
  }

  /** Desktop default: ask through the app-wide modal stack. */
  function confirmPrereqsViaModal(prereqNames: string[]): Promise<boolean> {
    const single = prereqNames.length === 1;
    return new Promise((resolve) =>
      createModal(
        ModalType.Confirmation,
        {
          title: "Install required mods?",
          description:
            `This mod also needs ${single ? "another mod" : "other mods"}: ` +
            `${prereqNames.join(", ")}. Drop will install ${single ? "it" : "them"} ` +
            `onto this game too.`,
          buttonText: "Install all",
        },
        (event, close) => {
          close();
          resolve(event === "confirm");
        },
      ),
    );
  }

  const confirmPrereqs: PrereqConfirm =
    opts?.confirmPrereqs ?? confirmPrereqsViaModal;

  /**
   * Queue a mod (and anything it needs) for download.
   *
   * Returns how many downloads were queued — 0 when there was nothing to do,
   * the user backed out of the prerequisites, or it failed. Callers use it to
   * tell "it's on its way" apart from "nothing happened", which otherwise look
   * identical: the install itself completes later, through the download queue.
   */
  async function installMod(mod: InstallableMod): Promise<number> {
    modError.value = undefined;
    installingModId.value = mod.id;
    try {
      const plan = await buildInstallPlan(mod.id);
      if (plan.length === 0) return 0; // target + prerequisites already installed

      const prereqs = plan.filter((p) => p.id !== mod.id);
      if (prereqs.length > 0) {
        const proceed = await confirmPrereqs(prereqs.map((p) => p.name));
        if (!proceed) return 0;
      }

      // Queue prerequisites first, then the target. Placement (overlay dir +
      // launch override) is declared on each mod version.
      for (const step of plan) {
        await invoke("download_mod", {
          modGameId: step.id,
          parentGameId,
          versionId: step.version.versionId,
          targetPlatform: step.version.platform,
          modInstallDir: step.version.modInstallDir ?? "",
          launchOverride: step.version.launchOverride ?? null,
        });
      }
      return plan.length;
    } catch (error) {
      console.error("[mod-install] installMod failed:", error);
      modError.value = String(error);
      return 0;
    } finally {
      installingModId.value = null;
    }
  }

  return { installingModId, modError, installMod };
}
