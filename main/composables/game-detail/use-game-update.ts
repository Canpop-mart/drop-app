/**
 * State behind the in-place update review, used by the desktop game page
 * (UpdateReviewModal) and the Big Picture game page (inline rows).
 *
 * Flow: `review(installVersionId)` asks the engine what updating that install
 * would change (`plan_game_update`, read-only). The player decides every
 * conflict, then `apply()` queues it (`apply_game_update`). The engine plans
 * again before queueing and refuses, with a message, when the update changed
 * on the server, a file changed since the review, or a conflict has no
 * choice; nothing is queued then, and the phase becomes an error the page
 * offers to re-check from. Progress is the download queue's.
 *
 * Per-page composable: NOT a singleton.
 */

import { invoke } from "@tauri-apps/api/core";
import {
  applyArgs,
  buildResolutions,
  carryChoices,
  chooseAll,
  chooseExtras,
  isUpToDate,
  needsRecovery,
  nextChoice,
  recoverOutcomeText,
  unresolvedPaths,
  updateErrorText,
  type Choices,
  type RecoverOutcome,
  type Resolution,
  type UpdatePlan,
} from "./update-review";
import { markUpdating } from "~/composables/update-tracking";

export type ReviewPhase =
  | { kind: "idle" }
  | { kind: "loading" }
  | {
      kind: "error";
      /** Shown as-is (the engine's needs-recovery marker is removed). */
      message: string;
      during: "plan" | "apply" | "repair";
      /** An earlier update has to be repaired first: offer "Repair update". */
      needsRecovery: boolean;
    }
  | { kind: "repairing" }
  | { kind: "up_to_date" }
  | { kind: "ready" }
  | { kind: "applying" }
  | { kind: "queued" };

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function updateError(e: unknown, during: "plan" | "apply"): ReviewPhase {
  return {
    kind: "error",
    message: updateErrorText(e),
    during,
    needsRecovery: needsRecovery(e),
  };
}

export function useGameUpdate(gameId: string) {
  const phase = ref<ReviewPhase>({ kind: "idle" });
  /** The last plan the engine returned; kept through an apply error. */
  const plan = ref<UpdatePlan | null>(null);
  const choices = ref<Choices>({});
  /** The install being reviewed. */
  const installVersionId = ref<string | null>(null);
  /** What "Repair update" did, shown above the re-checked review. */
  const repairNote = ref<string | null>(null);
  // A re-check can be pressed while one is in flight; only the newest answer
  // may land, or an older plan would replace the one on screen.
  let requestSeq = 0;

  const conflicts = computed(() => plan.value?.conflicts ?? []);
  const unresolved = computed(() =>
    unresolvedPaths(conflicts.value, choices.value),
  );
  const canApply = computed(
    () =>
      phase.value.kind === "ready" &&
      plan.value !== null &&
      unresolved.value.length === 0,
  );

  async function review(versionId: string, keepRepairNote = false) {
    const seq = ++requestSeq;
    if (!keepRepairNote) repairNote.value = null;
    installVersionId.value = versionId;
    phase.value = { kind: "loading" };
    try {
      const next = await invoke<UpdatePlan>("plan_game_update", {
        gameId,
        installVersionId: versionId,
      });
      if (seq !== requestSeq) return;
      choices.value = carryChoices(
        plan.value?.conflicts ?? [],
        choices.value,
        next.conflicts,
      );
      plan.value = next;
      phase.value = isUpToDate(next) ? { kind: "up_to_date" } : { kind: "ready" };
    } catch (e) {
      if (seq !== requestSeq) return;
      console.warn(`[update] plan_game_update failed for ${gameId}:`, e);
      phase.value = updateError(e, "plan");
    }
  }

  /** Check again with the same install (after an error or an apply refusal). */
  function recheck() {
    if (installVersionId.value) void review(installVersionId.value);
  }

  /**
   * The review was refused because an earlier update is stuck: repair it
   * (`recover_game_update`), then check the update again.
   */
  async function repairThenRecheck() {
    const v = installVersionId.value;
    if (!v || phase.value.kind === "repairing") return;
    const seq = ++requestSeq;
    phase.value = { kind: "repairing" };
    try {
      const outcome = await invoke<RecoverOutcome>("recover_game_update", {
        gameId,
        installVersionId: v,
      });
      if (seq !== requestSeq) return;
      repairNote.value = recoverOutcomeText(outcome);
      await review(v, true);
    } catch (e) {
      if (seq !== requestSeq) return;
      console.warn(`[update] recover_game_update failed for ${gameId}:`, e);
      phase.value = {
        kind: "error",
        message: updateErrorText(e),
        during: "repair",
        needsRecovery: true,
      };
    }
  }

  function choose(path: string, choice: Resolution) {
    choices.value = { ...choices.value, [path]: choice };
  }

  function toggle(path: string) {
    const kind = conflicts.value.find((c) => c.path === path)?.kind;
    choose(path, nextChoice(choices.value[path], kind));
  }

  /** "Take update for all" / "Keep mine for all": every conflict except the
   * extra files, whose choices stay as they are. */
  function chooseEvery(choice: Resolution) {
    choices.value = { ...choices.value, ...chooseAll(conflicts.value, choice) };
  }

  /** "Keep all" / "Remove all": the extra files only. */
  function chooseEveryExtra(choice: Resolution) {
    choices.value = { ...choices.value, ...chooseExtras(conflicts.value, choice) };
  }

  /** Queue the reviewed update. True when it was queued. */
  async function apply(): Promise<boolean> {
    const p = plan.value;
    const from = installVersionId.value;
    if (!p || !from || phase.value.kind !== "ready") return false;
    const built = buildResolutions(p.conflicts, choices.value);
    if (!built.ok) return false;
    const seq = ++requestSeq;
    phase.value = { kind: "applying" };
    try {
      await invoke(
        "apply_game_update",
        applyArgs(gameId, from, p, built.resolutions),
      );
      markUpdating(gameId);
      if (seq === requestSeq) phase.value = { kind: "queued" };
      return true;
    } catch (e) {
      console.warn(`[update] apply_game_update failed for ${gameId}:`, e);
      if (seq === requestSeq) {
        phase.value = updateError(e, "apply");
      }
      return false;
    }
  }

  function close() {
    requestSeq++;
    phase.value = { kind: "idle" };
    plan.value = null;
    choices.value = {};
    installVersionId.value = null;
    repairNote.value = null;
  }

  return {
    phase,
    plan,
    choices,
    conflicts,
    unresolved,
    canApply,
    installVersionId,
    repairNote,
    review,
    recheck,
    repairThenRecheck,
    choose,
    toggle,
    chooseEvery,
    chooseEveryExtra,
    apply,
    close,
  };
}

export type GameUpdateController = ReturnType<typeof useGameUpdate>;

/**
 * "Repair update": the way out when a launch is refused because an update was
 * interrupted and could be neither finished nor undone. Calls
 * `recover_game_update`, which tries once more and otherwise sets the update's
 * folder aside (nothing is deleted), and reports what happened in words.
 *
 * `target` is the install the failed launch was for; null means closed.
 */
export type RepairState =
  | { kind: "offer"; message: string }
  | { kind: "running" }
  | { kind: "done"; text: string }
  | { kind: "error"; message: string };

export function useUpdateRepair(gameId: string) {
  const target = ref<string | null>(null);
  const state = ref<RepairState | null>(null);

  /** Offer the repair for `versionId`, showing the launch error's text. */
  function offer(versionId: string, launchMessage: string) {
    target.value = versionId;
    state.value = { kind: "offer", message: launchMessage };
  }

  async function repair() {
    const v = target.value;
    if (!v || state.value?.kind === "running") return;
    state.value = { kind: "running" };
    try {
      const outcome = await invoke<RecoverOutcome>("recover_game_update", {
        gameId,
        installVersionId: v,
      });
      state.value = { kind: "done", text: recoverOutcomeText(outcome) };
    } catch (e) {
      console.warn(`[update] recover_game_update failed for ${gameId}:`, e);
      state.value = { kind: "error", message: updateErrorText(e) };
    }
  }

  function close() {
    target.value = null;
    state.value = null;
  }

  return { target, state, offer, repair, close };
}

export type UpdateRepairController = ReturnType<typeof useUpdateRepair>;
