/**
 * The three-way cloud-save status both Cloud Saves surfaces show, and the
 * display decoding of escaped save names.
 *
 *   node --test main/tests/save-sync-state.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  conflictCloudNote,
  conflictLocalNote,
  displaySaveName,
  heldOnSharedDeviceState,
  legacyRowNote,
  matchLegacyRow,
  saveSyncState,
} from "../composables/save-sync-state.ts";

const cloud = (dataHash: string, id = "row") => ({ id, dataHash });

test("only one side present", () => {
  assert.equal(saveSyncState({ dataHash: "a" }, null), "localOnly");
  assert.equal(saveSyncState(null, cloud("a")), "cloudOnly");
});

test("same bytes are synced whatever the record says", () => {
  assert.equal(
    saveSyncState({ dataHash: "ABC", syncedHash: "zzz", syncedCloudId: "row" }, cloud("abc")),
    "synced",
  );
});

test("only the cloud changed since the last sync", () => {
  assert.equal(
    saveSyncState({ dataHash: "old", syncedHash: "old", syncedCloudId: "row" }, cloud("new")),
    "cloudNewer",
  );
});

test("only this device changed since the last sync", () => {
  assert.equal(
    saveSyncState({ dataHash: "new", syncedHash: "old", syncedCloudId: "row" }, cloud("old")),
    "localNewer",
  );
});

test("another account's copy on this PC is a conflict, not a newer local copy", () => {
  assert.equal(
    saveSyncState(
      {
        dataHash: "new",
        syncedHash: "old",
        syncedCloudId: "row",
        lastSyncedByOtherAccount: "Sam",
      },
      cloud("old"),
    ),
    "conflict",
  );
  assert.equal(
    saveSyncState(
      {
        dataHash: "new",
        syncedHash: "old",
        syncedCloudId: "row",
        lastSyncedByOtherAccount: "",
      },
      cloud("old"),
    ),
    "conflict",
  );
});

test("another account syncing this game here keeps a local change a conflict", () => {
  assert.equal(
    saveSyncState(
      {
        dataHash: "new",
        syncedHash: "old",
        syncedCloudId: "row",
        otherAccountsOnThisDevice: true,
      },
      cloud("old"),
    ),
    "conflict",
  );
  // A download is still offered: the bytes here are this account's own.
  assert.equal(
    saveSyncState(
      {
        dataHash: "old",
        syncedHash: "old",
        syncedCloudId: "row",
        otherAccountsOnThisDevice: true,
      },
      cloud("new"),
    ),
    "cloudNewer",
  );
});

test("both changed is a conflict", () => {
  assert.equal(
    saveSyncState({ dataHash: "mine", syncedHash: "old", syncedCloudId: "row" }, cloud("theirs")),
    "conflict",
  );
});

test("no record, or a record for another row, stays a conflict", () => {
  assert.equal(saveSyncState({ dataHash: "a" }, cloud("b")), "conflict");
  assert.equal(
    saveSyncState({ dataHash: "a", syncedHash: "a", syncedCloudId: null }, cloud("b")),
    "conflict",
  );
  assert.equal(
    saveSyncState({ dataHash: "a", syncedHash: "a", syncedCloudId: "other" }, cloud("b")),
    "conflict",
  );
});

test("display names drop the namespace and undo the escaping", () => {
  assert.equal(displaySaveName("pc__slot1%2Fsave.dat"), "slot1/save.dat");
  assert.equal(displaySaveName("pc/legacy.dat"), "legacy.dat");
  assert.equal(displaySaveName("Zelda%3A Link.srm"), "Zelda: Link.srm");
  assert.equal(displaySaveName("save%2E"), "save.");
  assert.equal(displaySaveName("%63on.srm"), "con.srm");
  assert.equal(displaySaveName("100%252F.sav"), "100%2F.sav");
  // A literal percent sequence that was never an escape is left alone.
  assert.equal(displaySaveName("pc__100%.sav"), "100%.sav");
  assert.equal(displaySaveName("x%41y.sav"), "xAy.sav");
  assert.equal(displaySaveName("c1%C2%85x"), "c1\u0085x");
});

test("an older build's row is matched to the local file it is a copy of", () => {
  const locals = [
    { filename: "Zelda%3A X.srm", dataHash: "mine", legacyCloudName: "Zelda X.srm" },
    { filename: "Other.srm", dataHash: "o" },
  ];
  assert.deepEqual(matchLegacyRow(locals, { filename: "Zelda X.srm", dataHash: "MINE" }), {
    localFilename: "Zelda%3A X.srm",
    same: true,
  });
  assert.deepEqual(matchLegacyRow(locals, { filename: "Zelda X.srm", dataHash: "theirs" }), {
    localFilename: "Zelda%3A X.srm",
    same: false,
  });
  assert.equal(matchLegacyRow(locals, { filename: "Other.srm", dataHash: "x" }), null);
  // An old name that fits two local files writes to neither.
  const two = [
    ...locals,
    { filename: "Zelda%3F X.srm", dataHash: "q", legacyCloudName: "Zelda X.srm" },
  ];
  assert.deepEqual(matchLegacyRow(two, { filename: "Zelda X.srm", dataHash: "x" }), {
    localFilename: null,
    same: false,
  });
  for (const legacy of [
    { localFilename: "a", same: true },
    { localFilename: "a", same: false },
    { localFilename: null, same: false },
  ]) {
    assert.ok(!legacyRowNote(legacy, legacy.localFilename).includes("—"));
  }
});

test("a save only on a shared device is held back from Sync", () => {
  assert.equal(heldOnSharedDeviceState("localOnly", { otherAccountsOnThisDevice: true }), true);
  assert.equal(heldOnSharedDeviceState("localOnly", { otherAccountsOnThisDevice: false }), false);
  assert.equal(heldOnSharedDeviceState("localNewer", { otherAccountsOnThisDevice: true }), false);
  assert.equal(heldOnSharedDeviceState("localOnly", null), false);
});

test("conflict notes name a possible other account and an old cloud name", () => {
  assert.equal(conflictLocalNote({}), "");
  assert.equal(conflictLocalNote({ localLastSyncedBy: "Sam" }), "Last synced by Sam on this device");
  assert.ok(conflictLocalNote({ localMayBeOtherAccount: true }).includes("Drop account also plays"));
  assert.equal(conflictCloudNote({}), "");
  assert.ok(conflictCloudNote({ cloudLegacyName: "Zelda X.srm" }).length > 0);
});
