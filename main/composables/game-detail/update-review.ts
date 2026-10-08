/**
 * Pure rules behind the in-place update review, shared by the desktop game
 * page and the Big Picture game page.
 *
 * The engine (`plan_game_update` / `apply_game_update`, src-tauri/src/updates.rs)
 * does the real work. What lives here is everything the two surfaces have to
 * agree on: which install an Update acts on, when the review counts as "up to
 * date", what the player's choices turn into, how a long conflict list is
 * paged, when Play should ask first, and how the download queue labels an
 * update. No Vue, no Tauri: `node --test main/tests/update-review.test.ts`.
 *
 * Player-facing strings are placeholders for the owner to rewrite. They are
 * kept here so both surfaces read the same words.
 */

export type ConflictKind =
  | "added_exists"
  | "changed_both"
  | "removed_edited"
  /** A file in a folder the pack mirrors that the pack does not ship (the
   * player's own mod, say). Take update moves it to the recovery folder,
   * keep mine keeps it and Drop never asks about it again. */
  | "extra_file";
export type Resolution = "take_update" | "keep_mine";
export type UpdateConflict = { path: string; kind: ConflictKind };

/** `plan_game_update`'s result (camelCase from serde). */
export type UpdatePlan = {
  gameId: string;
  fromVersionId: string;
  fromRevision: number | null;
  toVersionId: string;
  toRevision: number;
  addCount: number;
  updateCount: number;
  removeCount: number;
  /** Upper bound: assumes every conflict takes the update. */
  downloadBytes: number;
  conflicts: UpdateConflict[];
  /**
   * Files replaced or removed with the player's copy kept as `.bak`, because
   * Drop can't tell whether the player changed them (also files a mod had
   * replaced that the game takes back). Not conflicts: no choice needed, and
   * already counted in `updateCount` / `removeCount`.
   */
  backupPaths: string[];
  /**
   * The `backupPaths` in folders the pack mirrors: the player's copy goes to
   * the recovery folder (`RECOVERY_FOLDER` in the game folder) instead of
   * `.bak`. Optional: older builds don't send it.
   */
  recoveryPaths?: string[];
  baselineSource: "local" | "server" | "none";
  /**
   * Player-data folders (install-relative) that are links, often to an SD
   * card, where this update has files to change but leaves them as they are.
   * The rest of the update applies. Optional: older builds don't send it.
   */
  skippedLinkedFolders?: string[];
  /**
   * The folders this review mirrored, as the engine checked them. Passed
   * back to `apply_game_update`, which refuses (asking for a new review)
   * when the server's list changed since. Optional: older builds don't send
   * it.
   */
  mirrorFolders?: string[];
};

/** `recover_game_update`'s result. */
export type RecoverOutcome =
  | { result: "nothing" }
  | { result: "rolledBack" }
  | { result: "rolledForward" }
  | { result: "setAside"; folder: string };

/** One row of `fetch_game_installs`. */
export type InstallSummary = {
  versionId: string;
  /** "Installed" | "SetupRequired" | "PartiallyInstalled". */
  installType: string;
  updateAvailable: boolean;
};

export type Choices = Record<string, Resolution>;

// ── Which install, and whether there is anything to do ──────────────────────

/**
 * The install an Update acts on: the one Play is pointed at when it has an
 * update, else the first finished install that has one. Null when none does.
 * A partial install is never updated in place (the engine refuses it); it
 * resumes instead.
 */
export function pickUpdateInstall(
  installs: readonly InstallSummary[],
  preferredVersionId: string | null | undefined,
): string | null {
  const updatable = installs.filter(
    (i) => i.updateAvailable && i.installType !== "PartiallyInstalled",
  );
  const preferred = updatable.find((i) => i.versionId === preferredVersionId);
  return (preferred ?? updatable[0])?.versionId ?? null;
}

/** Nothing changes at all: same version, same revision, no files. */
export function isUpToDate(plan: UpdatePlan): boolean {
  return (
    plan.fromVersionId === plan.toVersionId &&
    plan.fromRevision === plan.toRevision &&
    plan.addCount === 0 &&
    plan.updateCount === 0 &&
    plan.removeCount === 0 &&
    plan.conflicts.length === 0
  );
}

// ── The player's choices ─────────────────────────────────────────────────────

/** A file in a mirrored folder that the pack does not ship. */
export function isExtra(c: { kind: ConflictKind }): boolean {
  return c.kind === "extra_file";
}

/**
 * A press on a conflict row: unset goes to "take update", then it flips. A
 * file that is not part of the pack starts at "keep" instead, so a stray
 * press never moves a player's own file away.
 */
export function nextChoice(
  current: Resolution | undefined,
  kind?: ConflictKind,
): Resolution {
  if (!current) return kind === "extra_file" ? "keep_mine" : "take_update";
  return current === "take_update" ? "keep_mine" : "take_update";
}

/**
 * Every conflict except the extra files set to the same choice ("Take
 * update for all" / "Keep mine for all"). Extra files are left out: one
 * press must never move all of a player's own mods away. They have their
 * own bulk actions (`chooseExtras`).
 */
export function chooseAll(
  conflicts: readonly UpdateConflict[],
  choice: Resolution,
): Choices {
  const out: Choices = {};
  for (const c of conflicts) if (!isExtra(c)) out[c.path] = choice;
  return out;
}

/** Every extra file set to the same choice ("Keep all" / "Remove all"). */
export function chooseExtras(
  conflicts: readonly UpdateConflict[],
  choice: Resolution,
): Choices {
  const out: Choices = {};
  for (const c of conflicts) if (isExtra(c)) out[c.path] = choice;
  return out;
}

/** Conflicts the player has not decided yet. */
export function unresolvedPaths(
  conflicts: readonly UpdateConflict[],
  choices: Choices,
): string[] {
  return conflicts.filter((c) => !choices[c.path]).map((c) => c.path);
}

/**
 * The `resolutions` argument for `apply_game_update`: one entry per conflict
 * of THIS plan and nothing else (a choice left over from an earlier plan for
 * a path that is no longer a conflict is dropped). `missing` lists the
 * conflicts with no choice; the engine would refuse the apply anyway.
 */
export function buildResolutions(
  conflicts: readonly UpdateConflict[],
  choices: Choices,
):
  | { ok: true; resolutions: Choices }
  | { ok: false; missing: string[] } {
  const missing = unresolvedPaths(conflicts, choices);
  if (missing.length > 0) return { ok: false, missing };
  const resolutions: Choices = {};
  for (const c of conflicts) resolutions[c.path] = choices[c.path];
  return { ok: true, resolutions };
}

/**
 * The arguments of `apply_game_update` for a reviewed plan. The revision and
 * the mirrored folders go back as the review saw them: the engine refuses
 * the apply (asking for a new review) when either changed on the server
 * since, so a file the player chose to keep is never replaced unasked.
 */
export function applyArgs(
  gameId: string,
  installVersionId: string,
  plan: UpdatePlan,
  resolutions: Choices,
) {
  return {
    gameId,
    installVersionId,
    toVersionId: plan.toVersionId,
    toRevision: plan.toRevision,
    mirrorFolders: plan.mirrorFolders ?? [],
    resolutions,
  };
}

/**
 * Keep the player's choices across a re-check. A choice survives only when
 * the same path is still a conflict of the same kind: "take update" on a file
 * the player edited means something different once the file is, say, removed
 * by the update instead.
 */
export function carryChoices(
  previous: readonly UpdateConflict[],
  choices: Choices,
  next: readonly UpdateConflict[],
): Choices {
  const kindBefore = new Map(previous.map((c) => [c.path, c.kind]));
  const out: Choices = {};
  for (const c of next) {
    const choice = choices[c.path];
    if (choice && kindBefore.get(c.path) === c.kind) out[c.path] = choice;
  }
  return out;
}

// ── Paging the conflict list (Big Picture) ──────────────────────────────────

/** Conflicts per page in Big Picture: two columns of four. */
export const CONFLICT_PAGE_SIZE = 8;

export function pageCount(total: number, size = CONFLICT_PAGE_SIZE): number {
  return Math.max(1, Math.ceil(total / size));
}

export function clampPage(
  page: number,
  total: number,
  size = CONFLICT_PAGE_SIZE,
): number {
  return Math.min(Math.max(0, Math.floor(page)), pageCount(total, size) - 1);
}

export function pageSlice<T>(
  list: readonly T[],
  page: number,
  size = CONFLICT_PAGE_SIZE,
): T[] {
  const p = clampPage(page, list.length, size);
  return list.slice(p * size, p * size + size);
}

/**
 * The page a conflict is on, so a re-check keeps showing the row the player
 * was looking at (by path, not by position). Falls back to `fallback`
 * (clamped) when the path is no longer a conflict.
 */
export function pageForPath(
  conflicts: readonly UpdateConflict[],
  path: string | null | undefined,
  fallback: number,
  size = CONFLICT_PAGE_SIZE,
): number {
  const idx = path ? conflicts.findIndex((c) => c.path === path) : -1;
  if (idx >= 0) return Math.floor(idx / size);
  return clampPage(fallback, conflicts.length, size);
}

// ── Play with an update pending ──────────────────────────────────────────────

/**
 * Whether Play should ask "Update first / Play anyway". Only when the install
 * that would launch is known to have an update. Unknown installs (the list
 * failed to load) never block: a failed update check must not stop a launch.
 */
export function shouldAskBeforePlay(
  installs: readonly InstallSummary[] | null,
  launchVersionId: string | null | undefined,
): boolean {
  if (!installs || !launchVersionId) return false;
  const install = installs.find((i) => i.versionId === launchVersionId);
  return !!install && install.updateAvailable && install.installType === "Installed";
}

// ── Installing an older version ──────────────────────────────────────────────

/**
 * Whether `versionId` is the newest version for its platform. Version options
 * arrive latest first (the server orders them by `versionIndex`). Only the
 * newest gets updates enabled: an older one would otherwise be flagged
 * "update available" the moment it finished installing.
 */
export function isLatestForPlatform(
  options: readonly { versionId: string; platform: string }[],
  versionId: string,
): boolean {
  const chosen = options.find((o) => o.versionId === versionId);
  if (!chosen) return false;
  const first = options.find((o) => o.platform === chosen.platform);
  return first?.versionId === versionId;
}

// ── Download queue labels ────────────────────────────────────────────────────

/**
 * What the queue shows for an item. An in-place update runs through the same
 * queue as an install and the queue's own status says "Downloading", so the
 * game's status ("Updating", set by the update agent) or the page's own
 * knowledge that it queued an update decides the wording.
 */
export function queueItemLabel(
  queueStatus: string,
  opts: { gameStatusType?: string | null; updating?: boolean },
): string {
  const updating = opts.updating || opts.gameStatusType === "Updating";
  if (!updating) return queueStatus;
  switch (queueStatus) {
    case "Queued":
      return "Update queued";
    case "Downloading":
      return "Updating";
    case "Validating":
      return "Applying update";
    default:
      return queueStatus;
  }
}

/** The badge on a finished queue entry. */
export function completionLabel(kind: "install" | "update" | undefined): string {
  return kind === "update" ? "Updated" : "Installed";
}

// ── Copy (placeholders) ──────────────────────────────────────────────────────

export const CONFLICT_KIND_LABEL: Record<ConflictKind, string> = {
  added_exists: "You added a file the update also adds",
  changed_both: "You changed a file the update changes",
  removed_edited: "You changed a file the update removes",
  extra_file: "Not part of the pack",
};

export const RESOLUTION_LABEL: Record<Resolution, string> = {
  take_update: "Take update",
  keep_mine: "Keep mine",
};

/** The labels an extra file's choices read as, on both surfaces. */
export const EXTRA_RESOLUTION_LABEL: Record<Resolution, string> = {
  take_update: "Remove",
  keep_mine: "Keep",
};

/** What a choice is called on a row of this kind. */
export function choiceLabel(kind: ConflictKind, choice: Resolution): string {
  return kind === "extra_file"
    ? EXTRA_RESOLUTION_LABEL[choice]
    : RESOLUTION_LABEL[choice];
}

/** A row's choices in the order they are shown: "Keep" first for an extra file. */
export function choicesFor(kind: ConflictKind): Resolution[] {
  return kind === "extra_file"
    ? ["keep_mine", "take_update"]
    : ["take_update", "keep_mine"];
}

/** What a choice does to this file, for the row's detail line. */
export function resolutionDetail(
  kind: ConflictKind,
  choice: Resolution | undefined,
): string {
  if (!choice) return "Choose what to do with this file";
  if (kind === "extra_file") {
    return choice === "keep_mine"
      ? "Kept, and not asked again"
      : "Moved to the recovery folder";
  }
  if (choice === "keep_mine") return "Your copy stays as it is";
  return kind === "removed_edited"
    ? "The file is removed. Your copy is kept as .bak"
    : "The update's copy is used. Your copy is kept as .bak";
}

export const BASELINE_NONE_NOTE =
  "Drop has no record of what this install looked like when it was installed, so it can't tell which files you changed. Every file that differs is listed below.";

/** Where the update goes: the same version republished, or another one. */
export function targetLine(
  plan: UpdatePlan,
  versionName: (versionId: string) => string,
): string {
  return plan.fromVersionId === plan.toVersionId
    ? `Files in ${versionName(plan.toVersionId)} were changed on the server`
    : `From ${versionName(plan.fromVersionId)} to ${versionName(plan.toVersionId)}`;
}

/**
 * The folder, inside the game folder, where an update moves the player's
 * files out of a mirrored folder (`RECOVERY_DIR`,
 * games/src/downloads/update/mod.rs). Drop never deletes anything in it.
 */
export const RECOVERY_FOLDER = ".drop-removed";

/** Where the player's copy of a `backupPaths` entry goes. */
export function backupDestination(plan: UpdatePlan, path: string): string {
  return (plan.recoveryPaths ?? []).includes(path) ? RECOVERY_FOLDER : ".bak";
}

/** The informational line for `backupPaths`, or null when there are none. */
export function backupLine(plan: UpdatePlan): string | null {
  const all = plan.backupPaths ?? [];
  if (all.length === 0) return null;
  const recovered = new Set(plan.recoveryPaths ?? []);
  const r = all.filter((p) => recovered.has(p)).length;
  const n = all.length - r;
  const parts: string[] = [];
  if (n > 0) {
    parts.push(
      n === 1
        ? "1 file will be replaced or removed. Drop can't tell whether you changed it, so your copy is kept as .bak"
        : `${n} files will be replaced or removed. Drop can't tell whether you changed them, so your copies are kept as .bak`,
    );
  }
  if (r > 0) {
    parts.push(
      r === 1
        ? `1 file in a folder that matches the pack will be replaced or removed. Your copy is moved to the ${RECOVERY_FOLDER} folder in the game folder`
        : `${r} files in folders that match the pack will be replaced or removed. Your copies are moved to the ${RECOVERY_FOLDER} folder in the game folder`,
    );
  }
  return parts.join(". ");
}

/**
 * The line above the extra files' bulk actions, or null when there are
 * none. Names the two choices and where a removed file goes.
 */
export function extrasLine(conflicts: readonly UpdateConflict[]): string | null {
  const n = conflicts.filter(isExtra).length;
  if (n === 0) return null;
  const lead =
    n === 1
      ? "1 file in a folder that matches the pack is not part of it."
      : `${n} files in folders that match the pack are not part of it.`;
  return `${lead} Remove moves a file to the ${RECOVERY_FOLDER} folder in the game folder. Keep leaves it, and Drop won't ask about it again.`;
}

/** Big Picture's hint for what A does on a conflict row. */
export function toggleHint(conflicts: readonly UpdateConflict[]): string {
  const extras = conflicts.some(isExtra);
  const others = conflicts.some((c) => !isExtra(c));
  if (extras && others) {
    return "A switches between Take update and Keep mine, or Keep and Remove.";
  }
  return extras
    ? "A switches between Keep and Remove."
    : "A switches between Take update and Keep mine.";
}

/** The informational line for `skippedLinkedFolders`, or null when there are none. */
export function skippedLinkedLine(plan: UpdatePlan): string | null {
  const folders = plan.skippedLinkedFolders ?? [];
  if (folders.length === 0) return null;
  const names = folders.join(", ");
  return folders.length === 1
    ? `Files in ${names} won't be updated because that folder is a link to another location. The rest of the update still applies.`
    : `Files in ${names} won't be updated because those folders are links to another location. The rest of the update still applies.`;
}

/**
 * The line above the conflict list, on both surfaces. Says nothing about
 * who changed the files: an `extra_file` is not a change at all.
 */
export function conflictsLine(count: number): string {
  return count === 1
    ? "1 file needs a decision before this update."
    : `${count} files need a decision before this update.`;
}

/** "3 added, 12 updated, 1 removed". */
export function countsLine(plan: UpdatePlan): string {
  return `${plan.addCount} added, ${plan.updateCount} updated, ${plan.removeCount} removed`;
}

/** Download size; "up to" when conflicts may still skip some of it. */
export function downloadLine(plan: UpdatePlan): string {
  const size = formatBytes(plan.downloadBytes);
  return plan.conflicts.length > 0 ? `Download: up to ${size}` : `Download: ${size}`;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let i = 0;
  while (value >= 1000 && i < units.length - 1) {
    value /= 1000;
    i++;
  }
  return i === 0 ? `${value} B` : `${value.toFixed(1)} ${units[i]}`;
}

/** Result line for Big Picture's "Check for Updates". */
export type CheckOutcome =
  | { kind: "checking" }
  | { kind: "error"; message: string }
  | { kind: "up_to_date" }
  | { kind: "available" };

export function checkOutcome(
  error: string | null,
  installs: readonly InstallSummary[] | null,
): CheckOutcome {
  if (error) return { kind: "error", message: error };
  if (installs && installs.some((i) => i.updateAvailable)) return { kind: "available" };
  return { kind: "up_to_date" };
}

export function checkOutcomeText(outcome: CheckOutcome): string {
  switch (outcome.kind) {
    case "checking":
      return "Checking for updates...";
    case "error":
      return `Could not check for updates: ${outcome.message}`;
    case "up_to_date":
      return "No updates for this game";
    case "available":
      return "An update is available";
  }
}

// ── A stuck update ───────────────────────────────────────────────────────────

/**
 * The launch refusal for an update that was interrupted and could be neither
 * finished nor undone (`ProcessError::UpdateInProgress`, process/src/error.rs).
 * Matched on its text: launch errors reach the UI as plain strings.
 */
export function isStuckUpdateError(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  return msg.includes("last update did not finish");
}

/**
 * Prefix the engine puts on a `plan_game_update` / `apply_game_update` error
 * when an earlier update can be neither finished nor undone and has to be
 * repaired first (`NEEDS_RECOVERY_MARKER`, games/src/downloads/update/mod.rs).
 */
export const NEEDS_RECOVERY_MARKER = "[needs-recovery] ";

/** Whether an update error says "repair the earlier update first". */
export function needsRecovery(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  return msg.startsWith(NEEDS_RECOVERY_MARKER);
}

/** An update error as the player should read it: the marker removed. */
export function updateErrorText(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  return msg.startsWith(NEEDS_RECOVERY_MARKER)
    ? msg.slice(NEEDS_RECOVERY_MARKER.length)
    : msg;
}

/** What "Repair update" did, in plain words. */
export function recoverOutcomeText(outcome: RecoverOutcome): string {
  switch (outcome.result) {
    case "rolledBack":
      return "The unfinished update was undone. The game is back as it was before the update.";
    case "rolledForward":
      return "The update was finished. The game is up to date.";
    case "setAside":
      return `The game can be launched again. Your files from before the update, and any copies Drop kept, were moved to ${outcome.folder} inside the game folder. Some game files may be from the update and some from before it. Uninstalling the game deletes that folder, so copy anything you need out of it first.`;
    case "nothing":
      return "There was no unfinished update to repair.";
  }
}
