/**
 * Adding an install folder that already holds games says what it found.
 *
 *   node --test main/tests/install-dir-scan.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  describeInstallDirScan,
  parseInstallDirScan,
} from "../composables/install-dir-scan.ts";

test("parses the backend's report", () => {
  assert.deepEqual(parseInstallDirScan({ installed: 3, partial: 1 }), {
    installed: 3,
    partial: 1,
  });
});

test("a missing or malformed report counts as nothing found", () => {
  for (const raw of [null, undefined, "x", { installed: -2, partial: NaN }, { installed: "4" }]) {
    assert.deepEqual(parseInstallDirScan(raw), { installed: 0, partial: 0 });
  }
});

test("an empty folder says nothing was found", () => {
  assert.match(describeInstallDirScan({ installed: 0, partial: 0 }), /No Drop games/);
});

test("counts complete and unfinished installs together", () => {
  const one = describeInstallDirScan({ installed: 1, partial: 0 });
  assert.match(one, /Found 1 game already/);
  assert.doesNotMatch(one, /finish/);

  const mixed = describeInstallDirScan({ installed: 2, partial: 1 });
  assert.match(mixed, /Found 3 games/);
  assert.match(mixed, /1 needs to finish downloading/);

  assert.match(describeInstallDirScan({ installed: 0, partial: 2 }), /2 need to finish/);
});

test("no em-dashes in player-facing text", () => {
  for (const s of [
    describeInstallDirScan({ installed: 0, partial: 0 }),
    describeInstallDirScan({ installed: 2, partial: 3 }),
  ]) {
    assert.ok(!s.includes("\u2014"), s);
  }
});
