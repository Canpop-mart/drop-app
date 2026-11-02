/**
 * A console's emulator is listed before its ROMs.
 *
 *   node --test main/tests/console-emulators.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { withEmulatorsFirst } from "../composables/console-emulators.ts";

const e = (id: string) => ({ game: { id } });
const ids = (list: { game: { id: string } }[]) => list.map((x) => x.game.id);

test("emulators go first, in the server's order", () => {
  const emulators = new Map([
    ["ryu", e("ryu")],
    ["ra", e("ra")],
  ]);
  assert.deepEqual(
    ids(withEmulatorsFirst([e("zelda"), e("mario")], ["ra", "ryu"], emulators)),
    ["ra", "ryu", "zelda", "mario"],
  );
});

test("an emulator that didn't load is left out", () => {
  assert.deepEqual(
    ids(withEmulatorsFirst([e("zelda")], ["ra"], new Map())),
    ["zelda"],
  );
});

test("an emulator also in the ROM list appears once, at the front", () => {
  const emulators = new Map([["ra", e("ra")]]);
  assert.deepEqual(
    ids(withEmulatorsFirst([e("zelda"), e("ra")], ["ra", "ra"], emulators)),
    ["ra", "zelda"],
  );
});

test("an old server without emulator ids changes nothing", () => {
  assert.deepEqual(ids(withEmulatorsFirst([e("a")], undefined, new Map())), ["a"]);
});
