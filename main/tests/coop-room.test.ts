/**
 * Co-op room flow helpers: restart-recovery offer, code handling, and which
 * platform hint is shown.
 *
 * Runs on the Node test runner with no extra dependency:
 *   node --test main/tests/coop-room.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  elevationHint,
  formatRoomCode,
  isRoomGoneError,
  normaliseRoomCode,
  rejoinOffer,
  unavailableReason,
  type MyRoom,
} from "../composables/coop-room-logic.ts";

const MINE: MyRoom = {
  roomId: "r1",
  shortCode: "ABC234",
  networkId: "abcdef0123456789",
  isHost: false,
};

test("offers a rejoin only for a room this session isn't already in", () => {
  assert.equal(rejoinOffer(MINE, null), MINE);
  assert.equal(rejoinOffer({ ...MINE, active: true }, null), null);
  assert.equal(rejoinOffer(MINE, "r2"), null, "already in another room");
  assert.equal(rejoinOffer(null, null), null);
  assert.equal(rejoinOffer(undefined, undefined), null);
});

test("room codes are normalised however they're typed", () => {
  assert.equal(normaliseRoomCode(" abc-234 "), "ABC234");
  assert.equal(normaliseRoomCode("a b c 2 3 4"), "ABC234");
  assert.equal(formatRoomCode("ABC234"), "ABC-234");
  assert.equal(formatRoomCode("ABCD"), "ABCD");
});

test("only the room_not_found marker counts as the room being gone", () => {
  assert.equal(isRoomGoneError("room_not_found"), true);
  assert.equal(isRoomGoneError("Couldn't fetch the room: HTTP 500"), false);
});

test("the unavailable reason is worded per platform", () => {
  assert.equal(
    unavailableReason({ installed: true, running: false, capsReady: false, platform: "linux" }),
    null,
  );
  assert.match(
    unavailableReason({ installed: false, running: false, capsReady: true, platform: "windows" }) ?? "",
    /zerotier\.com/,
  );
  const linux =
    unavailableReason({ installed: false, running: false, capsReady: false, platform: "linux" }) ?? "";
  assert.match(linux, /AppImage/);
  assert.doesNotMatch(linux, /zerotier\.com/);
  assert.match(
    unavailableReason({
      installed: true,
      running: false,
      capsReady: false,
      platform: "linux",
      needsDesktopSetup: true,
    }) ?? "",
    /Desktop Mode/,
  );
  assert.equal(unavailableReason(null), null);
});

test("the permission hint matches what will actually be asked", () => {
  assert.match(
    elevationHint({ installed: true, running: false, capsReady: true, platform: "windows" }) ?? "",
    /Windows/,
  );
  assert.match(
    elevationHint({ installed: true, running: false, capsReady: false, platform: "linux" }) ?? "",
    /password/,
  );
  // Already set up, or a daemon is already running: nothing will be asked.
  assert.equal(
    elevationHint({ installed: true, running: false, capsReady: true, platform: "linux" }),
    null,
  );
  assert.equal(
    elevationHint({ installed: true, running: true, capsReady: false, platform: "linux" }),
    null,
  );
  assert.equal(
    elevationHint({
      installed: true,
      running: false,
      capsReady: false,
      platform: "linux",
      needsDesktopSetup: true,
    }),
    null,
  );
});

test("no player-facing string uses an em-dash", () => {
  const strings = [
    unavailableReason({ installed: false, running: false, capsReady: true, platform: "windows" }),
    unavailableReason({ installed: false, running: false, capsReady: true, platform: "linux" }),
    unavailableReason({ installed: false, running: false, capsReady: true, platform: "other" }),
    unavailableReason({ installed: true, running: false, capsReady: false, needsDesktopSetup: true }),
    elevationHint({ installed: true, running: false, capsReady: true, platform: "windows" }),
    elevationHint({ installed: true, running: false, capsReady: false, platform: "linux" }),
  ];
  for (const s of strings) assert.ok(s && !s.includes("—"), s ?? "null");
});
