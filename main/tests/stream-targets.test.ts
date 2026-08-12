/**
 * Which devices survive as remote targets, and what the dropdown is made of.
 *
 * Runs on the Node test runner with no extra dependency:
 *   node --test main/tests/stream-targets.test.ts
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DEVICE_ONLINE_WINDOW_MS,
  buildInstallMenuItems,
  buildPlayMenuItems,
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

/** The machine in the bug report: Windows, hostname canpop2, renamed. */
const LOCAL: LocalIdentity = {
  platform: "windows",
  hostname: "canpop2",
  displayName: "canpop2",
};

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

// ── Self detection ────────────────────────────────────────────────────────

test("the server's own isSelf flag is enough", () => {
  const self = device({ isSelf: true, name: "anything", platform: "Windows" });
  assert.equal(isOwnDevice(self, LOCAL, true), true);
});

test("a registration under this machine's hostname is us even while live", () => {
  const stale = device({ name: "canpop2 (Desktop)", platform: "Windows" });
  assert.equal(isOwnDevice(stale, LOCAL, true), true);
});

test("a dead registration under our display name is our own leftover", () => {
  const leftover = device({ name: "canpop2", platform: "Windows" });
  assert.equal(isOwnDevice(leftover, LOCAL, false), true);
});

test("a live device sharing our display name is someone else's machine", () => {
  // Renaming pushes the account name to every device, so the name alone
  // cannot be trusted — a running machine is not us.
  const other = device({ name: "canpop2", platform: "Windows" });
  assert.equal(isOwnDevice(other, LOCAL, true), false);
});

test("the other half of a dual boot is a different device, not us", () => {
  const linuxTwin = device({ name: "canpop2", platform: "Linux" });
  assert.equal(isOwnDevice(linuxTwin, LOCAL, false), false);
});

test("without a local identity only isSelf can rule", () => {
  assert.equal(isOwnDevice(device({ name: "canpop2" }), null, false), false);
  assert.equal(isOwnDevice(device({ isSelf: true }), null, false), true);
});

// ── The reported bug ──────────────────────────────────────────────────────

test("the machine you are sitting at is never offered as a stream target", () => {
  const devices = [
    // The current registration.
    device({ id: "win-now", name: "canpop2", platform: "Windows", isSelf: true }),
    // A dead earlier registration of the same Windows install.
    device({ id: "win-old", name: "canpop2", platform: "Windows", lastConnected: STALE }),
    // The Linux half of the same box: it cannot be up while Windows is.
    device({ id: "linux", name: "canpop2", platform: "Linux", lastConnected: STALE }),
    // A real second machine that is switched on.
    device({ id: "deck", name: "steamdeck (Desktop)", platform: "Linux" }),
  ];

  const { stream, install } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });

  assert.deepEqual(
    stream.map((t) => t.device.id),
    ["linux", "deck"],
    "our own Windows rows are gone, the dual-boot twin stays but is not live",
  );
  assert.deepEqual(stream.map((t) => t.online), [false, true]);
  assert.deepEqual(install, []);
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

test("repeat registrations collapse to the one worth offering", () => {
  const devices = [
    device({ id: "old", lastConnected: STALE, hasGame: true }),
    device({ id: "live", lastConnected: FRESH, hasGame: true }),
  ];
  const { stream } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream.map((t) => t.device.id), ["live"]);
});

test("between two dead registrations the most recent one survives", () => {
  const devices = [
    device({ id: "older", lastConnected: seenAgo(48 * 60 * 60 * 1000) }),
    device({ id: "newer", lastConnected: STALE }),
  ];
  const { stream } = partitionDevices(devices, { local: LOCAL, nowMs: NOW });
  assert.deepEqual(stream.map((t) => t.device.id), ["newer"]);
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
