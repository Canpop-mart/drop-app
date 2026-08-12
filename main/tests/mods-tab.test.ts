/**
 * Tab-visibility rules for the game-detail Mods tab.
 *
 * Runs on the Node test runner with no extra dependency:
 *   node --test main/tests/mods-tab.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  shouldShowModsTab,
  resolveActiveTab,
  buildModCards,
  modDisplayName,
  modDependentNames,
  type AvailableMod,
} from "../composables/game-detail/mods-tab.ts";

/** SMAPI is the loader; the Archipelago mod needs it. */
const smapi: AvailableMod = {
  id: "smapi",
  mName: "SMAPI",
  mShortDescription: "Mod loader",
  mIconObjectId: "icon-smapi",
};
const archipelago: AvailableMod = {
  id: "ap",
  mName: "StardewArchipelago",
  mShortDescription: "Randomiser client",
  mIconObjectId: "",
  requiredMods: [{ gameId: "smapi", name: "SMAPI" }],
};

test("mods tab stays hidden for a game that isn't installed", () => {
  assert.equal(
    shouldShowModsTab({
      installed: false,
      loaded: true,
      availableCount: 3,
      installedCount: 0,
    }),
    false,
  );
});

test("mods tab stays visible while the list is still loading", () => {
  // Hiding here and re-showing on arrival is the flicker we're avoiding.
  assert.equal(
    shouldShowModsTab({
      installed: true,
      loaded: false,
      availableCount: 0,
      installedCount: 0,
    }),
    true,
  );
});

test("mods tab hides once the list has loaded empty", () => {
  assert.equal(
    shouldShowModsTab({
      installed: true,
      loaded: true,
      availableCount: 0,
      installedCount: 0,
    }),
    false,
  );
});

test("mods tab shows when mods are available", () => {
  assert.equal(
    shouldShowModsTab({
      installed: true,
      loaded: true,
      availableCount: 2,
      installedCount: 0,
    }),
    true,
  );
});

test("mods tab shows for an installed mod the server no longer lists", () => {
  assert.equal(
    shouldShowModsTab({
      installed: true,
      loaded: true,
      availableCount: 0,
      installedCount: 1,
    }),
    true,
  );
});

test("selected tab is kept when it's still visible", () => {
  assert.equal(
    resolveActiveTab(["about", "community", "mods"], "mods", "about"),
    "mods",
  );
});

test("selected tab falls back when it disappears", () => {
  assert.equal(
    resolveActiveTab(["about", "community"], "mods", "about"),
    "about",
  );
});

test("cards merge the server listing with the on-disk ledger", () => {
  const cards = buildModCards(
    [smapi, archipelago],
    [{ gameId: "smapi", fileCount: 12 }],
  );
  assert.deepEqual(
    cards.map((c) => [c.id, c.installed, c.fileCount]),
    [
      ["smapi", true, 12],
      ["ap", false, null],
    ],
  );
  assert.deepEqual(cards[1].requires, ["SMAPI"]);
  // An empty icon id must not become an <img src="">.
  assert.equal(cards[1].iconObjectId, null);
});

test("card order follows the server listing, not install state", () => {
  // A card that jumped to the front on install would move under the cursor.
  const before = buildModCards([smapi, archipelago], []).map((c) => c.id);
  const after = buildModCards(
    [smapi, archipelago],
    [{ gameId: "ap", fileCount: 3 }],
  ).map((c) => c.id);
  assert.deepEqual(before, after);
});

test("an installed mod the server no longer lists still gets a card", () => {
  // Otherwise there is no way left to uninstall it.
  const cards = buildModCards([smapi], [{ gameId: "ghost", fileCount: 4 }]);
  const ghost = cards.find((c) => c.id === "ghost");
  assert.ok(ghost);
  assert.equal(ghost.unlisted, true);
  assert.equal(ghost.installed, true);
  assert.equal(ghost.name, "ghost");
});

test("an unlisted installed mod is not duplicated when the server lists it", () => {
  const cards = buildModCards([smapi], [{ gameId: "smapi", fileCount: 12 }]);
  assert.equal(cards.length, 1);
  assert.equal(cards[0].unlisted, false);
});

test("display name falls back to the id for an unlisted mod", () => {
  assert.equal(modDisplayName([smapi], "smapi"), "SMAPI");
  assert.equal(modDisplayName([smapi], "ghost"), "ghost");
});

test("uninstalling a prerequisite names the installed mods that need it", () => {
  assert.deepEqual(
    modDependentNames(
      [smapi, archipelago],
      [
        { gameId: "smapi", fileCount: 12 },
        { gameId: "ap", fileCount: 3 },
      ],
      "smapi",
    ),
    ["StardewArchipelago"],
  );
});

test("a mod nothing depends on reports no dependents", () => {
  assert.deepEqual(
    modDependentNames(
      [smapi, archipelago],
      [
        { gameId: "smapi", fileCount: 12 },
        { gameId: "ap", fileCount: 3 },
      ],
      "ap",
    ),
    [],
  );
  assert.deepEqual(modDependentNames([smapi], [], null), []);
});

test("a dependent that isn't installed is not warned about", () => {
  // The listing knows AP needs SMAPI, but AP isn't on disk, so removing SMAPI
  // breaks nothing the user actually has.
  assert.deepEqual(
    modDependentNames([smapi, archipelago], [{ gameId: "smapi" }], "smapi"),
    [],
  );
});
