/**
 * Mods state for the Big Picture game-detail page.
 *
 * All of the actual work is the shared game-detail layer — `fetch_game_mods`
 * for the server listing, `useInstalledMods` for the on-disk ledgers,
 * `useModInstall` for the queue-with-prerequisites flow. This composable only
 * adds the two things Big Picture needs on top: the merged card list the grid
 * renders, and a prerequisite confirmation that a controller can answer (the
 * desktop default routes through the app-wide modal stack, which has no
 * gamepad focus at all).
 *
 * Per-page composable: NOT a singleton — call from a component setup().
 */

import { invoke } from "@tauri-apps/api/core";
import { useInstalledMods } from "~/composables/game-detail/use-installed-mods";
import { useModInstall } from "~/composables/game-detail/use-mod-install";
import {
  buildModCards,
  modDependentNames,
  modDisplayName,
  type AvailableMod,
  type ModCard,
} from "~/composables/game-detail/mods-tab";

export function useBpmMods(parentGameId: string) {
  const available = ref<AvailableMod[]>([]);
  // Distinguishes "no mods" from "haven't asked yet", which is what decides
  // whether the Mods tab is hidden or still showing.
  const loaded = ref(false);

  const installedCtl = useInstalledMods(parentGameId);

  // ── Prerequisite confirmation ───────────────────────────────────────────
  // `installMod` awaits an answer, so the dialog is modelled as a promise the
  // component resolves. A pending promise pins `installingModId`, which is why
  // scope teardown below has to answer it.
  // `prereqNames` holds the copy and outlives the close, so the text doesn't
  // blank out during the dialog's fade; `prereqOpen` is what drives visibility.
  const prereqNames = ref<string[]>([]);
  const prereqOpen = ref(false);
  let prereqResolve: ((proceed: boolean) => void) | null = null;

  function confirmPrereqs(names: string[]): Promise<boolean> {
    prereqNames.value = names;
    prereqOpen.value = true;
    return new Promise<boolean>((resolve) => {
      prereqResolve = resolve;
    });
  }

  /** Answer the open prerequisite dialog. Safe to call when none is open. */
  function answerPrereqs(proceed: boolean) {
    prereqOpen.value = false;
    const resolve = prereqResolve;
    prereqResolve = null;
    resolve?.(proceed);
  }

  const installCtl = useModInstall(parentGameId, { confirmPrereqs });

  // ── Uninstall confirmation ──────────────────────────────────────────────
  // The dialog's copy is snapshotted when it opens rather than derived from
  // `modToUninstall`, which clears the moment the user confirms — deriving it
  // would blank the text out mid fade-out.
  const modToUninstall = ref<string | null>(null);
  const uninstallName = ref("");
  /** Names of installed mods that would break if the pending uninstall goes
   *  ahead — shown in the confirmation, not discovered afterwards. */
  const uninstallDependents = ref<string[]>([]);

  function askUninstall(modId: string) {
    modToUninstall.value = modId;
    uninstallName.value = modDisplayName(available.value, modId);
    uninstallDependents.value = modDependentNames(
      available.value,
      installedCtl.installedMods.value,
      modId,
    );
  }

  function cancelUninstall() {
    modToUninstall.value = null;
  }

  async function confirmUninstall() {
    const modId = modToUninstall.value;
    modToUninstall.value = null;
    if (!modId) return;
    await installedCtl.uninstall(modId);
  }

  // ── Data ────────────────────────────────────────────────────────────────
  async function loadAvailable() {
    try {
      // Must go through a Tauri command: the /client/* endpoints need the JWT
      // client auth header, which a raw server:// fetch doesn't carry (→ 403).
      available.value = await invoke<AvailableMod[]>("fetch_game_mods", {
        gameId: parentGameId,
      });
    } catch (e) {
      console.warn("[bpm-mods] failed to load available mods:", e);
      available.value = [];
    } finally {
      loaded.value = true;
    }
  }

  /** Re-read the on-disk ledgers. Only meaningful once the base game is
   *  installed — there is no install dir to scan before that. */
  function refreshInstalled() {
    return installedCtl.refresh();
  }

  /** Reload both halves. `installed` is false when the base game isn't
   *  present, in which case there is nothing on disk to read. */
  async function refresh(installed: boolean) {
    await Promise.all([
      loadAvailable(),
      installed ? installedCtl.refresh() : Promise.resolve(),
    ]);
  }

  const cards = computed<ModCard[]>(() =>
    buildModCards(available.value, installedCtl.installedMods.value),
  );

  // Leaving a pending prerequisite promise unresolved would hang `installMod`
  // forever with a mod id pinned in `installingModId`.
  onScopeDispose(() => answerPrereqs(false));

  return {
    available,
    loaded,
    cards,
    installedMods: installedCtl.installedMods,
    installingModId: installCtl.installingModId,
    uninstallingModId: installedCtl.uninstallingModId,
    modError: installCtl.modError,
    installMod: (modId: string) => installCtl.installMod({ id: modId }),
    prereqNames,
    prereqOpen,
    answerPrereqs,
    modToUninstall,
    uninstallName,
    uninstallDependents,
    askUninstall,
    cancelUninstall,
    confirmUninstall,
    refresh,
    refreshInstalled,
  };
}
