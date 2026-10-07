/**
 * Big Picture store: Back from a game must return to the Browse page, search,
 * sort, filter and collection the player left, with focus on the game they
 * opened.
 *
 *   node --test main/tests/store-browse-state.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_SNAPSHOT,
  clampPage,
  needsMoreResults,
  parseSnapshot,
  planReturnFocus,
  readStoreSnapshot,
  shouldRestoreFocus,
  writeStoreSnapshot,
  totalPages,
  type ReturnFocus,
  type StoreBrowseSnapshot,
} from "../composables/bigpicture/store-browse-state.ts";

const full: StoreBrowseSnapshot = {
  activeTab: "browse",
  searchQuery: "zelda",
  browseSort: "name",
  browseLibraryFilter: "lib-1",
  browsePage: 3,
  collectionId: null,
  returnFocus: { kind: "tile", gameId: "g42", scrollTop: 812 },
};

test("a written snapshot round-trips unchanged", () => {
  assert.deepEqual(parseSnapshot(JSON.stringify(full)), full);
});

test("missing, corrupt or non-object entries parse to null", () => {
  assert.equal(parseSnapshot(null), null);
  assert.equal(parseSnapshot(""), null);
  assert.equal(parseSnapshot("{not json"), null);
  assert.equal(parseSnapshot("42"), null);
  assert.equal(parseSnapshot("null"), null);
});

test("bad fields fall back to defaults instead of breaking the page", () => {
  const s = parseSnapshot(
    JSON.stringify({
      activeTab: "settings",
      searchQuery: 5,
      browseSort: "popularity",
      browseLibraryFilter: null,
      browsePage: -4,
      collectionId: "",
      returnFocus: { kind: "tile" },
    }),
  );
  assert.deepEqual(s, DEFAULT_SNAPSHOT);
});

test("fractional and NaN pages are made whole", () => {
  assert.equal(parseSnapshot('{"browsePage":2.7}')!.browsePage, 2);
  assert.equal(parseSnapshot('{"browsePage":"3"}')!.browsePage, 0);
});

test("an older entry without the new fields still parses", () => {
  const s = parseSnapshot('{"activeTab":"collections","collectionId":"c1"}')!;
  assert.equal(s.activeTab, "collections");
  assert.equal(s.collectionId, "c1");
  assert.equal(s.returnFocus, null);
  assert.equal(s.browsePage, 0);
});

test("loads continue until the restored page is full or the catalog ends", () => {
  // page 3 at 24 per page needs 96 results.
  assert.equal(needsMoreResults(55, 500, 3, 24), true);
  assert.equal(needsMoreResults(110, 500, 3, 24), false);
  // Catalog only has 80: stop once all 80 are loaded.
  assert.equal(needsMoreResults(55, 80, 3, 24), true);
  assert.equal(needsMoreResults(80, 80, 3, 24), false);
  assert.equal(needsMoreResults(0, 0, 0, 24), false);
});

test("totalPages", () => {
  assert.equal(totalPages(0, 24), 0);
  assert.equal(totalPages(24, 24), 1);
  assert.equal(totalPages(25, 24), 2);
});

test("a restored page is clamped when the catalog shrank", () => {
  assert.equal(clampPage(3, 500, 110, 24), 3);
  assert.equal(clampPage(3, 50, 50, 24), 2); // only 3 pages now
  assert.equal(clampPage(3, 0, 0, 24), 0);
  assert.equal(clampPage(-1, 100, 100, 24), 0);
});

test("a fetch that stopped early clamps to the last loaded page", () => {
  // Server says 500 but only 55 arrived (later batch failed).
  assert.equal(clampPage(5, 500, 55, 24), 2);
});

const rf: ReturnFocus = { kind: "tile", gameId: "g42", scrollTop: 0 };

test("focus is restored only when coming back from that game's page", () => {
  assert.equal(shouldRestoreFocus(rf, "/bigpicture/library/g42"), true);
  assert.equal(shouldRestoreFocus(rf, "/bigpicture/library/g42/"), true);
  assert.equal(shouldRestoreFocus(rf, "/bigpicture/library/g42?x=1"), true);
  assert.equal(shouldRestoreFocus(rf, "/bigpicture/library/g4"), false);
  assert.equal(shouldRestoreFocus(rf, "/bigpicture"), false);
  assert.equal(shouldRestoreFocus(rf, null), false);
  assert.equal(shouldRestoreFocus(null, "/bigpicture/library/g42"), false);
});

test("focus goes to the opened game by id, wherever it sits on the page", () => {
  assert.deepEqual(planReturnFocus(rf, ["a", "b", "g42"]), {
    kind: "tile",
    gameId: "g42",
  });
});

test("a game no longer on the page falls back to the first tile", () => {
  assert.deepEqual(planReturnFocus(rf, ["a", "b"]), { kind: "first-tile" });
  assert.deepEqual(planReturnFocus(rf, []), { kind: "first-tile" });
});

test("the roulette card gets focus back when it was used", () => {
  assert.deepEqual(
    planReturnFocus({ kind: "roulette", gameId: "g42", scrollTop: 0 }, []),
    { kind: "roulette" },
  );
});

test("the snapshot is read from sessionStorage and patches merge", () => {
  const store = new Map<string, string>();
  store.set("drop.bpm.store.browseSnapshot", JSON.stringify(full));
  (globalThis as { sessionStorage?: unknown }).sessionStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
  };
  try {
    assert.deepEqual(readStoreSnapshot(), full);
    writeStoreSnapshot({ returnFocus: null, browsePage: 1 });
    const saved = parseSnapshot(store.get("drop.bpm.store.browseSnapshot")!)!;
    assert.equal(saved.returnFocus, null);
    assert.equal(saved.browsePage, 1);
    assert.equal(saved.searchQuery, "zelda");
    assert.equal(readStoreSnapshot().browsePage, 1);
  } finally {
    delete (globalThis as { sessionStorage?: unknown }).sessionStorage;
  }
});
