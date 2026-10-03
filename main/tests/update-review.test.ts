/**
 * Rules behind the in-place update review (desktop and Big Picture).
 *
 * Runs on the Node test runner with no extra dependency:
 *   node --test main/tests/update-review.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  backupLine,
  buildResolutions,
  carryChoices,
  checkOutcome,
  chooseAll,
  clampPage,
  completionLabel,
  countsLine,
  downloadLine,
  formatBytes,
  isLatestForPlatform,
  isStuckUpdateError,
  needsRecovery,
  updateErrorText,
  isUpToDate,
  nextChoice,
  pageCount,
  pageForPath,
  pageSlice,
  pickUpdateInstall,
  queueItemLabel,
  recoverOutcomeText,
  resolutionDetail,
  shouldAskBeforePlay,
  targetLine,
  unresolvedPaths,
  type InstallSummary,
  type UpdateConflict,
  type UpdatePlan,
} from "../composables/game-detail/update-review.ts";

function plan(over: Partial<UpdatePlan> = {}): UpdatePlan {
  return {
    gameId: "g",
    fromVersionId: "v1",
    fromRevision: 1,
    toVersionId: "v1",
    toRevision: 1,
    addCount: 0,
    updateCount: 0,
    removeCount: 0,
    downloadBytes: 0,
    conflicts: [],
    backupPaths: [],
    baselineSource: "local",
    ...over,
  };
}

const conflicts: UpdateConflict[] = [
  { path: "config/a.cfg", kind: "changed_both" },
  { path: "mods/b.jar", kind: "added_exists" },
  { path: "old.txt", kind: "removed_edited" },
];

const inst = (
  versionId: string,
  updateAvailable: boolean,
  installType = "Installed",
): InstallSummary => ({ versionId, updateAvailable, installType });

test("the update acts on the selected install when it has an update", () => {
  const installs = [inst("v1", true), inst("v2", true)];
  assert.equal(pickUpdateInstall(installs, "v2"), "v2");
  assert.equal(pickUpdateInstall(installs, null), "v1");
});

test("the selected install without an update falls back to one that has it", () => {
  assert.equal(pickUpdateInstall([inst("v1", false), inst("v2", true)], "v1"), "v2");
  assert.equal(pickUpdateInstall([inst("v1", false)], "v1"), null);
});

test("a partial install is never offered for an in-place update", () => {
  assert.equal(pickUpdateInstall([inst("v1", true, "PartiallyInstalled")], "v1"), null);
});

test("up to date only when nothing at all changes", () => {
  assert.equal(isUpToDate(plan()), true);
  assert.equal(isUpToDate(plan({ updateCount: 1 })), false);
  assert.equal(isUpToDate(plan({ conflicts: [conflicts[0]] })), false);
  // A new version with identical files still moves the install: not "up to date".
  assert.equal(isUpToDate(plan({ toVersionId: "v2" })), false);
  assert.equal(isUpToDate(plan({ toRevision: 2 })), false);
  // An install from before revisions (fromRevision null) is not up to date
  // even with no file changes: applying records the baseline.
  assert.equal(isUpToDate(plan({ fromRevision: null })), false);
});

test("a press on a conflict starts at take update, then flips", () => {
  assert.equal(nextChoice(undefined), "take_update");
  assert.equal(nextChoice("take_update"), "keep_mine");
  assert.equal(nextChoice("keep_mine"), "take_update");
});

test("apply needs a choice for every conflict", () => {
  const partial = { "config/a.cfg": "keep_mine" as const };
  assert.deepEqual(unresolvedPaths(conflicts, partial), ["mods/b.jar", "old.txt"]);
  const r = buildResolutions(conflicts, partial);
  assert.equal(r.ok, false);
  if (!r.ok) assert.deepEqual(r.missing, ["mods/b.jar", "old.txt"]);
});

test("resolutions carry exactly this plan's conflicts", () => {
  const choices = {
    ...chooseAll(conflicts, "take_update"),
    "stale/path.cfg": "keep_mine" as const,
  };
  const r = buildResolutions(conflicts, choices);
  assert.equal(r.ok, true);
  if (r.ok) {
    assert.deepEqual(Object.keys(r.resolutions).sort(), [
      "config/a.cfg",
      "mods/b.jar",
      "old.txt",
    ]);
    assert.equal(r.resolutions["old.txt"], "take_update");
  }
});

test("no conflicts means an empty resolutions object, which applies", () => {
  const r = buildResolutions([], {});
  assert.deepEqual(r, { ok: true, resolutions: {} });
});

test("choose all sets every conflict", () => {
  assert.deepEqual(chooseAll(conflicts, "keep_mine"), {
    "config/a.cfg": "keep_mine",
    "mods/b.jar": "keep_mine",
    "old.txt": "keep_mine",
  });
});

test("a re-check keeps choices only for the same path and kind", () => {
  const choices = {
    "config/a.cfg": "keep_mine" as const,
    "mods/b.jar": "take_update" as const,
    "old.txt": "keep_mine" as const,
  };
  const next: UpdateConflict[] = [
    { path: "config/a.cfg", kind: "changed_both" }, // same: kept
    { path: "mods/b.jar", kind: "changed_both" }, // kind changed: dropped
    { path: "new.cfg", kind: "added_exists" }, // new: unset
  ];
  assert.deepEqual(carryChoices(conflicts, choices, next), {
    "config/a.cfg": "keep_mine",
  });
});

test("paging: two columns of four, clamped", () => {
  assert.equal(pageCount(0), 1);
  assert.equal(pageCount(8), 1);
  assert.equal(pageCount(9), 2);
  assert.equal(clampPage(5, 9), 1);
  assert.equal(clampPage(-1, 9), 0);
  const list = Array.from({ length: 10 }, (_, i) => i);
  assert.deepEqual(pageSlice(list, 1), [8, 9]);
  assert.deepEqual(pageSlice(list, 7), [8, 9]);
});

test("a re-check stays on the page of the row the player was on", () => {
  const many: UpdateConflict[] = Array.from({ length: 20 }, (_, i) => ({
    path: `f${String(i).padStart(2, "0")}`,
    kind: "changed_both" as const,
  }));
  assert.equal(pageForPath(many, "f17", 0), 2);
  // Rows above it went away: the same path is now on an earlier page.
  assert.equal(pageForPath(many.slice(10), "f17", 2), 0);
  // Gone entirely: keep the old page, clamped to what exists.
  assert.equal(pageForPath(many.slice(0, 5), "f17", 2), 0);
  assert.equal(pageForPath(many, null, 1), 1);
});

test("play asks first only when the install it launches has an update", () => {
  const installs = [inst("v1", true), inst("v2", false)];
  assert.equal(shouldAskBeforePlay(installs, "v1"), true);
  assert.equal(shouldAskBeforePlay(installs, "v2"), false);
  assert.equal(shouldAskBeforePlay([inst("v1", true, "SetupRequired")], "v1"), false);
});

test("play never blocks when the installs or the version are unknown", () => {
  assert.equal(shouldAskBeforePlay(null, "v1"), false);
  assert.equal(shouldAskBeforePlay([inst("v1", true)], null), false);
  assert.equal(shouldAskBeforePlay([inst("v1", true)], "gone"), false);
});

test("only the newest version for its platform counts as latest", () => {
  const options = [
    { versionId: "w3", platform: "Windows" },
    { versionId: "l2", platform: "Linux" },
    { versionId: "w2", platform: "Windows" },
    { versionId: "l1", platform: "Linux" },
  ];
  assert.equal(isLatestForPlatform(options, "w3"), true);
  assert.equal(isLatestForPlatform(options, "l2"), true);
  assert.equal(isLatestForPlatform(options, "w2"), false);
  assert.equal(isLatestForPlatform(options, "l1"), false);
  assert.equal(isLatestForPlatform(options, "missing"), false);
});

test("queue labels read as an update when the game is updating", () => {
  assert.equal(queueItemLabel("Downloading", {}), "Downloading");
  assert.equal(queueItemLabel("Downloading", { gameStatusType: "Updating" }), "Updating");
  assert.equal(queueItemLabel("Queued", { updating: true }), "Update queued");
  assert.equal(queueItemLabel("Validating", { updating: true }), "Applying update");
  assert.equal(queueItemLabel("Paused", { updating: true }), "Paused");
  assert.equal(queueItemLabel("Queued", { gameStatusType: "Queued" }), "Queued");
});

test("a finished update is not called an install", () => {
  assert.equal(completionLabel("update"), "Updated");
  assert.equal(completionLabel("install"), "Installed");
  assert.equal(completionLabel(undefined), "Installed");
});

test("the detail line says what the choice does", () => {
  assert.match(resolutionDetail("changed_both", "take_update"), /\.bak/);
  assert.match(resolutionDetail("removed_edited", "take_update"), /removed/);
  assert.match(resolutionDetail("removed_edited", "keep_mine"), /stays/);
  assert.match(resolutionDetail("added_exists", undefined), /Choose/);
});

test("summary lines", () => {
  assert.equal(
    countsLine(plan({ addCount: 3, updateCount: 12, removeCount: 1 })),
    "3 added, 12 updated, 1 removed",
  );
  assert.equal(downloadLine(plan({ downloadBytes: 1_500_000 })), "Download: 1.5 MB");
  assert.equal(
    downloadLine(plan({ downloadBytes: 2_000, conflicts: [conflicts[0]] })),
    "Download: up to 2.0 KB",
  );
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(999), "999 B");
  assert.equal(formatBytes(3_200_000_000), "3.2 GB");
});

test("check outcome: an error is never reported as up to date", () => {
  assert.deepEqual(checkOutcome("offline", [inst("v1", false)]), {
    kind: "error",
    message: "offline",
  });
  assert.deepEqual(checkOutcome(null, [inst("v1", true)]), { kind: "available" });
  assert.deepEqual(checkOutcome(null, [inst("v1", false)]), { kind: "up_to_date" });
});

test("no player-facing copy uses an em dash", async () => {
  const mod = await import("../composables/game-detail/update-review.ts");
  const strings: string[] = [
    ...Object.values(mod.CONFLICT_KIND_LABEL),
    ...Object.values(mod.RESOLUTION_LABEL),
    mod.BASELINE_NONE_NOTE,
    mod.checkOutcomeText({ kind: "checking" }),
    mod.checkOutcomeText({ kind: "up_to_date" }),
    mod.checkOutcomeText({ kind: "available" }),
    mod.checkOutcomeText({ kind: "error", message: "x" }),
    resolutionDetail("changed_both", "take_update"),
    resolutionDetail("removed_edited", "take_update"),
    resolutionDetail("changed_both", "keep_mine"),
    resolutionDetail("changed_both", undefined),
    backupLine(plan({ backupPaths: ["a"] }))!,
    backupLine(plan({ backupPaths: ["a", "b"] }))!,
    targetLine(plan(), (v) => v),
    targetLine(plan({ toVersionId: "v2" }), (v) => v),
    recoverOutcomeText({ result: "nothing" }),
    recoverOutcomeText({ result: "rolledBack" }),
    recoverOutcomeText({ result: "rolledForward" }),
    recoverOutcomeText({ result: "setAside", folder: "/g/.drop-update-set-aside-1" }),
  ];
  for (const s of strings) assert.ok(!s.includes("—"), s);
});

test("backed-up files are information, never choices", () => {
  const p = plan({ updateCount: 2, backupPaths: ["options.txt", "config/x.cfg"] });
  assert.equal(backupLine(plan()), null);
  assert.match(backupLine(p)!, /^2 files .*\.bak$/);
  assert.match(backupLine(plan({ backupPaths: ["a"] }))!, /^1 file .*your copy/);
  // They need no resolution: apply is not held up by them.
  assert.deepEqual(buildResolutions(p.conflicts, {}), { ok: true, resolutions: {} });
  assert.equal(isUpToDate(p), false);
});

test("backed-up files page like the conflicts", () => {
  const paths = Array.from({ length: 11 }, (_, i) => `f${i}`);
  assert.equal(pageCount(paths.length), 2);
  assert.deepEqual(pageSlice(paths, 1), ["f8", "f9", "f10"]);
});

test("a republished pinned version reads as the same version", () => {
  const name = (v: string) => (v === "v1" ? "1.0" : "2.0");
  const same = plan({ fromRevision: 1, toRevision: 2 });
  assert.equal(isUpToDate(same), false);
  assert.equal(targetLine(same, name), "Files in 1.0 were changed on the server");
  assert.equal(targetLine(plan({ toVersionId: "v2" }), name), "From 1.0 to 2.0");
});

test("the stuck-update launch error is recognised by its text", () => {
  const msg =
    "This game's last update did not finish, and Drop could not finish or undo it. Restart Drop to try again.";
  assert.equal(isStuckUpdateError(msg), true);
  assert.equal(isStuckUpdateError(new Error(msg)), true);
  assert.equal(isStuckUpdateError("Launch file is missing: 'x'"), false);
});

test("every repair outcome says what happened", () => {
  assert.match(recoverOutcomeText({ result: "rolledBack" }), /back as it was/);
  assert.match(recoverOutcomeText({ result: "rolledForward" }), /finished/);
  assert.match(recoverOutcomeText({ result: "nothing" }), /no unfinished update/);
  const aside = recoverOutcomeText({ result: "setAside", folder: "/games/x/.drop-update-set-aside-9" });
  assert.match(aside, /launched again/);
  assert.ok(aside.includes("/games/x/.drop-update-set-aside-9"));
  // The set-aside folder lives inside the install, and uninstalling deletes
  // it: the text must say to copy things out first, never just "reinstall".
  assert.match(aside, /deletes that folder, so copy anything you need out of it first/);
  assert.doesNotMatch(aside, /uninstall it and install it again/i);
});

test("a needs-recovery error is recognised and shown without its marker", () => {
  const raw =
    "[needs-recovery] The last update of this game did not finish and has to be repaired before it can be updated again (journal unreadable).";
  assert.equal(needsRecovery(raw), true);
  assert.equal(needsRecovery(new Error(raw)), true);
  assert.equal(
    updateErrorText(raw),
    "The last update of this game did not finish and has to be repaired before it can be updated again (journal unreadable).",
  );
  // Only a leading marker counts.
  assert.equal(needsRecovery("Could not queue the update: [needs-recovery] x"), false);
  assert.equal(updateErrorText("Not enough free space for this update."), "Not enough free space for this update.");
  assert.equal(needsRecovery("The Drop server needs updating before games can be updated in place."), false);
});
