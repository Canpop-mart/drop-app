/**
 * One-press Install rule (Big Picture home and game page).
 *   node --test main/tests/quick-install.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { quickInstallArgs } from "../composables/game-detail/quick-install-logic.ts";

test("no versions gives nothing to install", () => {
  assert.equal(quickInstallArgs([]), null);
});

test("picks the first option and sends every key download_game requires", () => {
  const args = quickInstallArgs([
    { versionId: "w2", platform: "Windows" },
    { versionId: "l1", platform: "Linux" },
    { versionId: "w1", platform: "Windows" },
  ]);
  assert.deepEqual(args, {
    versionId: "w2",
    targetPlatform: "Windows",
    installDir: 0,
    enableUpdates: true,
  });
});
