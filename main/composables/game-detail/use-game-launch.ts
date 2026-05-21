/**
 * Launch / kill / uninstall / resume actions for the library game-detail
 * page, plus the "pick a launch option" and "dependency required" modal
 * state those actions drive.
 *
 * Extracted from `pages/library/[id]/index.vue`. The page used to inline
 * all of this; pulling it out keeps the install flow (`useGameInstall`),
 * the read data (`useGameStats`), and these write actions cleanly
 * separated.
 *
 * Per-game-detail composable: NOT a singleton — call from a component
 * `setup()`. `createModal` is auto-imported from drop-base.
 */

import { invoke } from "@tauri-apps/api/core";
import { InstalledType } from "~/types";
import type { Game, GameStatus } from "~/types";
import type { LaunchResult } from "~/composables/game";

export function useGameLaunch(game: Game, status: Ref<GameStatus>) {
  // ── Launch-options modal ────────────────────────────────────────────────
  // `launchOptions` doubles as the modal's open state: `undefined` = closed.
  const launchOptions = ref<Array<{ name: string }> | undefined>(undefined);
  const launchOptionsOpen = computed(() => launchOptions.value !== undefined);

  // ── Dependency-required modal ───────────────────────────────────────────
  const dependencyRequiredModal = ref<
    { gameId: string; versionId: string } | undefined
  >(undefined);

  // Guards against duplicate `launch_game` invocations from double-clicks /
  // repeated keyboard activations — the backend rejects the second call with
  // `AlreadyRunning`, which would otherwise show an error over a game that's
  // actually starting fine.
  const launchInFlight = ref(false);

  function notifyLaunchFailure(action: "run" | "stop", err: unknown) {
    const errMsg = err instanceof Error ? err.message : String(err);
    createModal(
      ModalType.Notification,
      {
        title: `Couldn't ${action} "${game.mName}"`,
        description: `Drop failed to ${action} "${game.mName}": ${errMsg}`,
        buttonText: "Close",
      },
      (_e, c) => c(),
    );
  }

  async function launch() {
    // SetupRequired installs launch straight into their (single) setup step.
    if (
      status.value.type === "Installed" &&
      status.value.install_type.type === InstalledType.SetupRequired
    ) {
      await launchIndex(0);
      return;
    }
    try {
      const fetchedLaunchOptions = await invoke<Array<{ name: string }>>(
        "get_launch_options",
        { id: game.id },
      );
      if (fetchedLaunchOptions.length === 1) {
        await launchIndex(0);
        return;
      }
      launchOptions.value = fetchedLaunchOptions;
    } catch (e) {
      notifyLaunchFailure("run", e);
      console.error(e);
    }
  }

  async function launchIndex(index: number) {
    if (launchInFlight.value) return;
    launchInFlight.value = true;
    launchOptions.value = undefined;
    try {
      const result = await invoke<LaunchResult>("launch_game", {
        id: game.id,
        index,
      });
      if (result.result === "InstallRequired") {
        dependencyRequiredModal.value = {
          gameId: result.data[0],
          versionId: result.data[1],
        };
      }
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      if (
        errMsg.includes("AlreadyRunning") ||
        errMsg.includes("already running")
      ) {
        // Benign — the first invoke already started the game.
        return;
      }
      notifyLaunchFailure("run", e);
    } finally {
      launchInFlight.value = false;
    }
  }

  async function kill() {
    try {
      await invoke("kill_game", { gameId: game.id });
    } catch (e) {
      notifyLaunchFailure("stop", e);
      console.error(e);
    }
  }

  async function uninstall() {
    await invoke("uninstall_game", { gameId: game.id });
  }

  async function resumeDownload() {
    try {
      await invoke("resume_download", { gameId: game.id });
    } catch (e) {
      console.error(e);
    }
  }

  return {
    launchOptions,
    launchOptionsOpen,
    dependencyRequiredModal,
    launch,
    launchIndex,
    kill,
    uninstall,
    resumeDownload,
  };
}
