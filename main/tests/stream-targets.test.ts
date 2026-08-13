/**
 * Which devices survive as remote targets, and what the dropdown is made of.
 *
 * Runs on the Node test runner with no extra dependency:
 *   node --test main/tests/stream-targets.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DEVICE_FORGET_WINDOW_MS,
  DEVICE_ONLINE_WINDOW_MS,
  buildInstallMenuItems,
  buildPlayMenuItems,
  isDeviceForgotten,
  isDeviceOnline,
  isOwnDevice,
  nextEnabledIndex,
  partitionDevices,
  type DeviceLike,
  type LocalIdentity,
} from "../composables/bigpicture/stream-targets.ts";

const NOW = Date.parse("2026-08-12T12:00:00.000Z");
const seenAgo = (ms: number) => new Date(NOW - ms).toISOString();
const FRESH = seenAgo(30_000);
const STALE = seenAgo(6 * 60 * 60 * 1000);
const ANCIENT = seenAgo(DEVICE_FORGET_WINDOW_MS + 1);

/** The Steam Deck in the bug report: signed in as registration `deck-now`. */
const LOCAL: LocalIdentity = { clientId: "deck-now" };

function device(over: Partial<DeviceLike> = {}): DeviceLike {
  return {
    id: "d1",
    name: "steamdeck (Desktop)",
    platform: "Linux",
    lastConnected: FRESH,
    isSelf: false,
    hasGame: true,
    ...over,
  };
}

// ── Liveness ──────────────────────────────────────────────────────────────

test("a device seen a moment ago is online", () => {
  assert.equal(isDeviceOnline(device({ lastConnected: FRESH }), NOW), true);
});

test("a device seen hours ago is offline", () => {
  assert.equal(isDeviceOnline(device({ lastConnected: STALE }), NOW), false);
});

test("the online window is inclusive at its edge", () => {
  const edge = device({ lastConnected: seenAgo(DEVICE_ONLINE_WINDOW_MS) });
  assert.equal(isDeviceOnline(edge, NOW), true);
  const past = device({ lastConnected: seenAgo(DEVICE_ONLINE_WINDOW_MS + 1) });
  assert.equal(isDeviceOnline(past, NOW), false);
});

test("an unreadable timestamp counts as offline, never as online", () => {
  assert.equal(isDeviceOnline(device({ lastConnected: "" }), NOW), false);
  assert.equal(isDeviceOnline(device({ lastConnected: "whenever" }), NOW), false);
});

test("a registration goes quiet long before it is forgotten", () => {
  assert.equal(isDeviceForgotten(device({ lastConnected: STALE }), NOW), false);
  assert.equal(isDeviceForgotten(device({ lastConnected: ANCIENT }), NOW), true);
});

test("an unreadable timestamp is not evidence of death either", () => {
  // Mirror of the online case: a mangled timestamp leaves the device listed
  // and disabled instead of vanishing with no explanation.
  assert.equal(isDeviceForgotten(device({ lastConnected: "whenever" }), NOW), false);
});

// ── Self detection ────────────────────────────────────────────────────────

test("the server's own isSelf flag is enough", () => {
  const self = device({ isSelf: true, id: "somebody-else", name: "anything" });
  assert.equal(isOwnDevice(self, LOCAL), true);
});

test("the registration we authenticate as is us, whatever it is called", () => {
  const self = device({ id: "deck-now", name: "canpop2", platform: "Linux" });
  assert.equal(isOwnDevice(self, LOCAL), true);
});

test("a different registration is not us, however alike it looks", () => {
  // Same name, same platform, same hardware even — but a different id, so
  // nothing here can prove it is this machine and it is not treated as one.
  const twin = device({ id: "deck-old", name: "canpop2", platform: "Linux" });
  assert.equal(isOwnDevice(twin, LOCAL), false);
});

test("without a local client id only isSelf can rule", () => {
  assert.equal(isOwnDevice(device({ id: "deck-now" }), null), false);
  assert.equal(isOwnDevice(device({ isSelf: true }), null), true);
  assert.equal(isOwnDevice(device({ id: "deck-now" }), { clientId: null }), false);
});

// ── The reported bug ──────────────────────────────────────────────────────

test("the Steam Deck is not offered as a target while sitting at it", () => {
  // Every device on the account carries the account's display name, because
  // opening a multiplayer screen renames whichever device did it. The Deck is
  // signed in and online, so neither the name nor liveness can single it out —
  // only its registration id can.
  const devices = [
    device({ id: "deck-now", name: "canpop2", platform: "Linux", isSelf: true }),
    device({ id: "pc", name: "canpop2", platform: "Windows" }),
    device({ id: "htpc", name: "canpop2", platform: "Linux" }),
  ];

  const { stream } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });

  assert.deepEqual(
    stream.map((t) => t.device.id),
    ["pc", "htpc"],
    "the Deck is gone; the two real machines that share its name are not",
  );
  assert.deepEqual(stream.map((t) => t.online), [true, true]);
});

test("the id rules even when the server forgot to flag the row", () => {
  // isSelf is the server comparing the same two ids, so it should already be
  // set — but the client holds the id first-hand and does not need to be told.
  const devices = [device({ id: "deck-now", name: "canpop2", isSelf: false })];
  const { stream } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream, []);
});

test("devices sharing one name are all offered, never folded together", () => {
  // Collapsing by name used to hide real machines once every device on the
  // account answered to the same one.
  const devices = [
    device({ id: "a", name: "canpop2", platform: "Linux" }),
    device({ id: "b", name: "canpop2", platform: "Linux" }),
  ];
  const { stream } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream.map((t) => t.device.id), ["a", "b"]);
});

test("a registration nobody has signed in on for a month is dropped", () => {
  const devices = [
    device({ id: "deck-old", name: "steamdeck (Desktop)", lastConnected: ANCIENT }),
    device({ id: "laptop", name: "canpop2", hasGame: false, lastConnected: ANCIENT }),
    device({ id: "pc", name: "canpop2", platform: "Windows", lastConnected: STALE }),
  ];
  const { stream, install } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream.map((t) => t.device.id), ["pc"]);
  assert.deepEqual(install, [], "the abandoned laptop is not an install target");
});

test("the forget window is configurable, and inclusive at its edge", () => {
  const edge = device({ lastConnected: seenAgo(DEVICE_FORGET_WINDOW_MS) });
  const { stream } = partitionDevices([edge], { local: LOCAL, nowMs: NOW });
  assert.equal(stream.length, 1, "exactly at the window it is still listed");

  const { stream: tighter } = partitionDevices([device({ lastConnected: STALE })], {
    local: LOCAL,
    nowMs: NOW,
    forgetMs: 60 * 60 * 1000,
  });
  assert.deepEqual(tighter, []);
});

test("the server's ordering survives the partition", () => {
  // The device list arrives most-recently-connected first, which is the order
  // the menu should offer them in.
  const devices = [
    device({ id: "first", lastConnected: FRESH }),
    device({ id: "second", lastConnected: seenAgo(5 * 60 * 1000) }),
    device({ id: "third", lastConnected: STALE }),
  ];
  const { stream } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream.map((t) => t.device.id), ["first", "second", "third"]);
});

test("an offline device with the game is kept, flagged, never silently dropped", () => {
  const { stream } = partitionDevices([device({ lastConnected: STALE })], {
    local: LOCAL,
    nowMs: NOW,
  });
  assert.equal(stream.length, 1);
  assert.equal(stream[0].online, false);
});

test("only devices that definitely lack the game can be installed on", () => {
  const devices = [
    device({ id: "has", name: "deck", hasGame: true }),
    device({ id: "lacks", name: "laptop", hasGame: false }),
    device({ id: "unknown", name: "htpc", hasGame: undefined }),
  ];
  const { stream, install } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream.map((t) => t.device.id), ["has"]);
  assert.deepEqual(install.map((t) => t.device.id), ["lacks"]);
});

test("a repeat registration of one machine stays visible until it is forgotten", () => {
  // Both rows are the same physical PC, paired twice. The client cannot prove
  // that, so it shows both and lets liveness say which one to use. The dead one
  // falls off on its own once a month has passed.
  const devices = [
    device({ id: "old", lastConnected: STALE }),
    device({ id: "live", lastConnected: FRESH }),
  ];
  const { stream } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream.map((t) => t.device.id), ["old", "live"]);
  assert.deepEqual(stream.map((t) => t.online), [false, true]);
});

test("no devices at all leaves both lists empty", () => {
  const empty = partitionDevices([], { local: LOCAL, nowMs: NOW });
  assert.deepEqual(empty.stream, []);
  assert.deepEqual(empty.install, []);
});

// ── Menu model ────────────────────────────────────────────────────────────

const partition = (over: Partial<{ stream: DeviceLike[]; install: DeviceLike[] }> = {}) => ({
  stream: (over.stream ?? []).map((d) => ({ device: d, online: isDeviceOnline(d, NOW) })),
  install: (over.install ?? []).map((d) => ({ device: d, online: isDeviceOnline(d, NOW) })),
});

test("the play menu always starts with the local Play row", () => {
  const items = buildPlayMenuItems(partition());
  assert.deepEqual(items.map((i) => i.kind), ["play-local"]);
  assert.equal(items[0].disabled, false);
});

test("every play menu index resolves to the row it renders", () => {
  const items = buildPlayMenuItems(
    partition({
      stream: [device({ id: "s1" }), device({ id: "s2" })],
      install: [device({ id: "i1", hasGame: false })],
    }),
  );
  assert.deepEqual(
    items.map((i) => [i.kind, "device" in i ? i.device.id : null]),
    [
      ["play-local", null],
      ["stream", "s1"],
      ["stream", "s2"],
      ["install-remote", "i1"],
    ],
  );
});

test("install rows still line up when there are no stream rows", () => {
  const items = buildPlayMenuItems(partition({ install: [device({ id: "i1", hasGame: false })] }));
  assert.deepEqual(
    items.map((i) => ("device" in i ? i.device.id : i.kind)),
    ["play-local", "i1"],
  );
});

test("an offline row is disabled and says why", () => {
  const items = buildPlayMenuItems(partition({ stream: [device({ lastConnected: STALE })] }));
  assert.equal(items[1].disabled, true);
  assert.equal(items[1].detail, "Offline");
});

test("the install menu starts with Install here", () => {
  const items = buildInstallMenuItems(partition({ install: [device({ id: "i1", hasGame: false })] }));
  assert.deepEqual(
    items.map((i) => ("device" in i ? i.device.id : i.kind)),
    ["install-local", "i1"],
  );
});

test("D-pad movement skips disabled rows and stops at the ends", () => {
  const items = buildPlayMenuItems(
    partition({
      stream: [device({ id: "off", lastConnected: STALE }), device({ id: "on" })],
    }),
  );
  assert.equal(nextEnabledIndex(items, 0, 1), 2, "skips the offline row");
  assert.equal(nextEnabledIndex(items, 2, -1), 0, "skips it on the way back");
  assert.equal(nextEnabledIndex(items, 2, 1), 2, "stays put at the bottom");
  assert.equal(nextEnabledIndex(items, 0, -1), 0, "stays put at the top");
});

test("a menu whose remote rows are all offline still reaches the local row", () => {
  const items = buildPlayMenuItems(
    partition({ stream: [device({ id: "off", lastConnected: STALE })] }),
  );
  assert.equal(nextEnabledIndex(items, 0, 1), 0);
});

test("focus past the end of a shrunken list comes back into range", () => {
  // The menu refetches devices while it is open. If that fetch fails the list
  // collapses to the local row under a focus index of 3, and walking from there
  // leaves the D-pad dead in both directions.
  const items = buildPlayMenuItems(partition({}));
  assert.equal(items.length, 1);
  assert.equal(nextEnabledIndex(items, 3, -1), 0);
  assert.equal(nextEnabledIndex(items, 3, 1), 0);
});

test("focus below zero comes back into range too", () => {
  const items = buildPlayMenuItems(partition({ stream: [device({ id: "on" })] }));
  assert.equal(nextEnabledIndex(items, -2, -1), 0);
  assert.equal(nextEnabledIndex(items, -2, 1), 0);
});

test("recovering from an out-of-range index skips disabled rows", () => {
  const items = buildPlayMenuItems(
    partition({ stream: [device({ id: "off", lastConnected: STALE })] }),
  );
  // Row 1 is the offline device, so the nearest row that can be acted on is 0.
  assert.equal(nextEnabledIndex(items, 9, -1), 0);
  assert.equal(nextEnabledIndex(items, 9, 1), 0);
});

test("an empty menu has nowhere to move to", () => {
  assert.equal(nextEnabledIndex([], 0, 1), 0);
  assert.equal(nextEnabledIndex([], 4, -1), 0);
});

// ── End to end, from the Deck's device list ───────────────────────────────

test("the Deck's play menu, built from the list it actually receives", () => {
  // The whole reported case in one go: this Deck is signed in and online, a
  // second Linux box answers to the same account name, an abandoned pairing of
  // this Deck sits under its old hostname, and a fourth machine has a name long
  // enough to have been sliced in half by the old menu width.
  const devices = [
    device({ id: "deck-now", name: "canpop2", platform: "Linux", isSelf: true }),
    device({ id: "htpc", name: "canpop2", platform: "Linux" }),
    device({ id: "deck-old", name: "steamdeck (Desktop)", platform: "Linux", lastConnected: ANCIENT }),
    device({
      id: "pc",
      name: "canpop2-workstation (Desktop)",
      platform: "Windows",
      hasGame: false,
      lastConnected: STALE,
    }),
  ];

  const partition = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  const items = buildPlayMenuItems(partition);

  assert.deepEqual(
    items.map((i) => [i.kind, "device" in i ? i.device.id : null, i.disabled]),
    [
      ["play-local", null, false],
      ["stream", "htpc", false],
      ["install-remote", "pc", true],
    ],
    "no row for this Deck, none for the pairing it abandoned",
  );
  assert.equal(items[1].label, "Play on canpop2");
  assert.equal(items[2].label, "Install on canpop2-workstation (Desktop)");

  // Every index still addresses the row it renders, and the D-pad skips the
  // offline install row rather than parking on it.
  items.forEach((item, i) => assert.equal(items[i], item));
  assert.equal(nextEnabledIndex(items, 0, 1), 1);
  assert.equal(nextEnabledIndex(items, 1, 1), 1, "nothing enabled below row 1");
  assert.equal(nextEnabledIndex(items, 1, -1), 0);
});
