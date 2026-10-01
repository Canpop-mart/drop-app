/**
 * Big Picture profile editor: saving the slots must not delete showcase items
 * the slots don't show.
 *
 *   node --test main/tests/showcase-merge.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mergeShowcase,
  slotItems,
  untouchedCount,
  type ShowcaseEntry,
} from "../composables/bigpicture/showcase-merge.ts";

const g = (id: string): ShowcaseEntry => ({ type: "FavoriteGame", gameId: id });
const a = (id: string): ShowcaseEntry => ({ type: "Achievement", itemId: id });
const stats = (id: string): ShowcaseEntry => ({ type: "GameStats", gameId: id });
const custom = (t: string): ShowcaseEntry => ({ type: "Custom", title: t });

const ids = (items: ShowcaseEntry[]) =>
  items.map((i) => `${i.type}:${i.gameId ?? i.itemId ?? i.title}`);

test("unchanged slots reproduce the stored list exactly", () => {
  const loaded = [g("1"), stats("s"), a("x"), custom("hi"), g("2")];
  const { games, achievements } = slotItems(loaded, 6, 6);
  assert.deepEqual(
    ids(mergeShowcase(loaded, games, achievements, 6, 6)),
    ids(loaded),
  );
});

test("GameStats and Custom items survive a save", () => {
  const loaded = [stats("s"), g("1"), custom("hi")];
  const out = mergeShowcase(loaded, [g("9")], [], 6, 6);
  assert.deepEqual(ids(out), ["GameStats:s", "FavoriteGame:9", "Custom:hi"]);
});

test("items past the slot limit are kept, not dropped", () => {
  const loaded = [g("1"), g("2"), g("3")];
  // Two slots: g1 and g2 are editable, g3 is not shown and must stay.
  const { games } = slotItems(loaded, 2, 6);
  assert.equal(games.length, 2);
  assert.equal(untouchedCount(loaded, 2, 6), 1);
  const out = mergeShowcase(loaded, [g("2")], [], 2, 6);
  assert.deepEqual(ids(out), ["FavoriteGame:2", "FavoriteGame:3"]);
});

test("removing a slot item removes only that item", () => {
  const loaded = [g("1"), custom("c"), a("x"), a("y")];
  const out = mergeShowcase(loaded, [g("1")], [a("y")], 6, 6);
  assert.deepEqual(ids(out), ["FavoriteGame:1", "Custom:c", "Achievement:y"]);
});

test("new slot items are appended after the stored list", () => {
  const loaded = [custom("c")];
  const out = mergeShowcase(loaded, [g("1")], [a("x")], 6, 6);
  assert.deepEqual(ids(out), ["Custom:c", "FavoriteGame:1", "Achievement:x"]);
});

test("empty stored list and empty slots save nothing", () => {
  assert.deepEqual(mergeShowcase([], [], [], 6, 6), []);
});
