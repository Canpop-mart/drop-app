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
import { listen } from "@tauri-apps/api/event";
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

  // Precise "Preparing…" message emitted by the backend during a slow,
  // blocking one-time prefix-prep step (e.g. installing the VC++ runtime via
  // winetricks before a umu/Proton launch). The first `launch_game` invoke
  // doesn't return until prep finishes, so without this the window looks
  // frozen. `undefined` = no prep in flight. Linux-only by nature — the
  // backend only ever emits this on Proton launches.
  const prepStatus = ref<string | undefined>(undefined);

  // Listen for backend prep-status events, scoped to this game. The payload
  // carries the game id (so a sibling page doesn't react), an `active` flag,
  // and the message to show while active.
  const unlistenPrepStatus = listen<{
    gameId: string;
    active: boolean;
    message: string;
  }>("game_prep_status", (event) => {
    if (event.payload.gameId !== game.id) return;
    prepStatus.value = event.payload.active ? event.payload.message : undefined;
  });
  onScopeDispose(() => {
    unlistenPrepStatus.then((unlisten) => unlisten());
  });

  // Incognito mode for the *next* launch. When true, the backend won't
  // open a PlaySession, won't heartbeat, won't update Playtime, and won't
  // poll achievements. Cleared back to `false` after the launch_game call
  // returns so a normal Play click after an incognito launch isn't sticky.
  // Pure client-side state — the server doesn't see this flag.
  const incognitoNext = ref(false);
  // Latches `true` while an incognito session is actually running so a UI
  // overlay (purple badge) can confirm to the user that no session data is
  // being reported. Cleared by the page's existing process-exit watcher.
  const incognitoActive = ref(false);

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

  /**
   * Convenience entry: launch the game with incognito set for *this*
   * invocation only. Used by the gear menu's "Play incognito" action so a
   * follow-up plain Play click stays normal.
   */
  async function launchIncognito() {
    incognitoNext.value = true;
    try {
      await launch();
    } finally {
      // `launch` may queue the launch-options modal; if it does we want
      // incognito to still be set when the user picks an option. The flag
      // is cleared inside `launchIndex` after the actual invoke.
    }
  }

  async function launchIndex(index: number) {
    if (launchInFlight.value) return;
    launchInFlight.value = true;
    launchOptions.value = undefined;
    const useIncognito = incognitoNext.value;
    incognitoNext.value = false;
    try {
      const result = await invoke<LaunchResult>("launch_game", {
        id: game.id,
        index,
        incognito: useIncognito,
      });
      if (result.result === "InstallRequired") {
        dependencyRequiredModal.value = {
          gameId: result.data[0],
          versionId: result.data[1],
        };
      } else if (useIncognito) {
        // Latch the overlay only once the backend has accepted the launch
        // (the Success arm). InstallRequired never actually spawns the
        // child, so we don't claim incognito is active in that case.
        incognitoActive.value = true;
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
      // The invoke has returned, so any prep step the backend ran is done —
      // clear the indicator in case a final "clear" event was missed.
      prepStatus.value = undefined;
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
    launchIncognito,
    launchIndex,
    kill,
    uninstall,
    resumeDownload,
    incognitoActive,
    launchInFlight,
    prepStatus,
  };
}
