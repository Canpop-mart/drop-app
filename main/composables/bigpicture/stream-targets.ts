/**
 * Which of the account's other devices may be offered as a remote target for
 * one game, and what the Play / Install dropdown is made of.
 *
 * Pure on purpose: no Vue, no Tauri, no clock of its own. Everything the rules
 * need is passed in, so `tests/stream-targets.test.ts` can pin the behaviour
 * for every self / installed / reachable combination.
 *
 * Four rules decide what the user is allowed to see:
 *
 *  1. The machine you are sitting at is never a remote target. This is an
 *     identity check, not a guess: the client stores the registration id it
 *     authenticates as (`DatabaseAuth.client_id`, handed back by the handshake
 *     and read through the `fetch_system_data` command), and that id is the
 *     primary key of the very row the device list returns. Comparing the two
 *     is exact. See `isOwnDevice`.
 *  2. "Play on X" requires the game to be installed on X. That is `hasGame`,
 *     which the server derives from the install list X last reported.
 *  3. Both remote actions require X to be reachable. A stream is push-based:
 *     the request sits on the server until X's poller picks it up, and the
 *     server expires it after two minutes. A device that is powered off can
 *     neither host a stream nor start a remote install, so offering either is
 *     offering something that cannot happen. Unreachable devices stay listed
 *     and disabled, so it is clear where they went.
 *  4. A registration nobody has signed in on for a month is dropped entirely.
 *     Re-pairing mints a new registration and abandons the old one, and the
 *     server has no route to delete a client, so those rows accumulate forever
 *     and the account's device list slowly fills with machines that will never
 *     answer. See `DEVICE_FORGET_WINDOW_MS`.
 *
 * `lastConnected` is the liveness signal for rules 3 and 4. Every authenticated
 * request bumps it (debounced to once a minute server-side) and a signed-in
 * client polls for pending stream requests every ten seconds, so a device that
 * is running Drop is always inside the window below and one that is off is not.
 */

/** The device shape this module needs. Matches `ClientDevice`. */
export interface DeviceLike {
  id: string;
  name: string;
  platform: string;
  /** ISO timestamp of the device's last authenticated request. */
  lastConnected: string;
  isSelf: boolean;
  /** Undefined when the server was not asked about a specific game. */
  hasGame?: boolean;
}

/** Who this client is on the server. */
export interface LocalIdentity {
  /**
   * The registration id this client authenticates as, or null when it could
   * not be read (signed out, or the command failed). Same value as the `id`
   * column of the matching row in the device list.
   */
  clientId: string | null;
}

/** A device paired with the reachability verdict used to judge it. */
export interface DeviceTarget {
  device: DeviceLike;
  online: boolean;
}

/**
 * How stale `lastConnected` may get before a device counts as unreachable.
 *
 * The poll is every 10s and the server only writes `lastConnected` once a
 * minute, so a live device is normally under two minutes stale. The poller
 * backs off to five minutes while the server is unhealthy, and this window
 * clears that with room to spare: it is meant to catch "powered off", not
 * "briefly busy".
 */
export const DEVICE_ONLINE_WINDOW_MS = 10 * 60 * 1000;

/**
 * How long a registration survives without being signed in on.
 *
 * Re-pairing a machine creates a new registration and leaves the old one
 * behind, and nothing ever deletes it: the server implements `removeClient`
 * but exposes no route to it, so the client cannot offer to clear one and must
 * not pretend otherwise. Time is the only honest signal it has.
 *
 * A month is deliberately generous. A console booted once a fortnight still
 * appears, greyed out, and any device that comes back reappears the moment it
 * signs in — nothing is lost, only hidden. Past that a registration is far more
 * likely to be an abandoned pairing than a machine the user is about to wake up
 * to receive a game.
 */
export const DEVICE_FORGET_WINDOW_MS = 30 * 24 * 60 * 60 * 1000;

/** Has this device talked to the server recently enough to act on a request? */
export function isDeviceOnline(
  device: DeviceLike,
  nowMs: number,
  windowMs: number = DEVICE_ONLINE_WINDOW_MS,
): boolean {
  const seen = Date.parse(device.lastConnected);
  // A timestamp we cannot read is not evidence of life.
  if (Number.isNaN(seen)) return false;
  return nowMs - seen <= windowMs;
}

/**
 * Has this registration been quiet long enough to stop listing it?
 *
 * The unreadable-timestamp case is the mirror image of `isDeviceOnline`: there
 * it is not evidence of life, here it is not evidence of death. A device whose
 * timestamp the server mangled stays in the list, greyed out, rather than
 * disappearing with no way for the user to find out why.
 */
export function isDeviceForgotten(
  device: DeviceLike,
  nowMs: number,
  forgetMs: number = DEVICE_FORGET_WINDOW_MS,
): boolean {
  const seen = Date.parse(device.lastConnected);
  if (Number.isNaN(seen)) return false;
  return nowMs - seen > forgetMs;
}

/**
 * Is this registration the one we are signed in as?
 *
 * Two spellings of one fact. `local.clientId` is the id this client holds and
 * authenticates with; `device.isSelf` is the server having compared that same
 * id to the row it was building. Either is conclusive, so the check is an
 * equality test and nothing else.
 *
 * Names are not consulted, and must not be: `use-display-name` pushes the
 * account's display name onto whichever device opens a multiplayer screen, so
 * every machine on an account ends up sharing one name. Any rule keyed on the
 * name either hides a real device or offers this one, depending on which way it
 * guesses. On the Steam Deck it guessed wrong in both directions at once.
 *
 * This only ever identifies the *current* registration. An earlier one left
 * behind by a re-pair has a different id and is indistinguishable from a second
 * machine, which is what `DEVICE_FORGET_WINDOW_MS` is for.
 */
export function isOwnDevice(
  device: DeviceLike,
  local: LocalIdentity | null,
): boolean {
  if (device.isSelf) return true;
  return local?.clientId ? device.id === local.clientId : false;
}

export interface PartitionInput {
  local: LocalIdentity | null;
  nowMs: number;
  windowMs?: number;
  forgetMs?: number;
}

export interface DevicePartition {
  /** Devices that have the game installed. Offline ones are kept, flagged. */
  stream: DeviceTarget[];
  /** Devices that definitely do not have the game. */
  install: DeviceTarget[];
}

/**
 * Split the account's devices into the two things the UI can offer.
 *
 * `hasGame === undefined` means the server was never asked about this game for
 * that device, so it lands in neither list: guessing produces either a "Play
 * on X" that fails or an "Install on X" for a game X already has.
 *
 * Order is the server's — most recently connected first — and is kept, so the
 * device you used last is the first one offered.
 *
 * Registrations are no longer collapsed by name. That used to fold repeat
 * pairings of one machine into a single row, but `use-display-name` gives every
 * device on an account the same name, so the key stopped meaning "same machine"
 * and started hiding real ones. Showing a duplicate is a smaller lie than
 * hiding a Steam Deck, and the forget window clears the pairings that dedupe
 * was really aimed at.
 */
export function partitionDevices(
  devices: DeviceLike[],
  {
    local,
    nowMs,
    windowMs = DEVICE_ONLINE_WINDOW_MS,
    forgetMs = DEVICE_FORGET_WINDOW_MS,
  }: PartitionInput,
): DevicePartition {
  const others: DeviceTarget[] = [];
  for (const device of devices) {
    if (isOwnDevice(device, local)) continue;
    if (isDeviceForgotten(device, nowMs, forgetMs)) continue;
    others.push({ device, online: isDeviceOnline(device, nowMs, windowMs) });
  }

  return {
    stream: others.filter((t) => t.device.hasGame === true),
    install: others.filter((t) => t.device.hasGame === false),
  };
}

// ── Menu model ────────────────────────────────────────────────────────────
// The dropdown used to be three template blocks indexed by hand
// (`1 + streamableDevices.length + i`), so any change in list membership
// risked firing the wrong entry's action. One flat array removes the
// arithmetic: the row's index in this array is the only index there is.

export type PlayMenuItem =
  | { kind: "play-local"; key: string; label: string; detail: null; disabled: false }
  | { kind: "install-local"; key: string; label: string; detail: null; disabled: false }
  | {
      kind: "stream";
      key: string;
      label: string;
      detail: string;
      device: DeviceLike;
      disabled: boolean;
    }
  | {
      kind: "install-remote";
      key: string;
      label: string;
      detail: string;
      device: DeviceLike;
      disabled: boolean;
    };

const OFFLINE_DETAIL = "Offline";

function streamItem(target: DeviceTarget): PlayMenuItem {
  return {
    kind: "stream",
    key: `stream-${target.device.id}`,
    label: `Play on ${target.device.name}`,
    detail: target.online ? `Stream · ${target.device.platform}` : OFFLINE_DETAIL,
    device: target.device,
    disabled: !target.online,
  };
}

function installItem(target: DeviceTarget): PlayMenuItem {
  return {
    kind: "install-remote",
    key: `install-${target.device.id}`,
    label: `Install on ${target.device.name}`,
    detail: target.online ? target.device.platform : OFFLINE_DETAIL,
    device: target.device,
    disabled: !target.online,
  };
}

/** Dropdown for a game that is installed here: play, stream, remote install. */
export function buildPlayMenuItems(partition: DevicePartition): PlayMenuItem[] {
  return [
    { kind: "play-local", key: "play-local", label: "Play", detail: null, disabled: false },
    ...partition.stream.map(streamItem),
    ...partition.install.map(installItem),
  ];
}

/** Dropdown for a game that is not installed here: install, remote install. */
export function buildInstallMenuItems(partition: DevicePartition): PlayMenuItem[] {
  return [
    {
      kind: "install-local",
      key: "install-local",
      label: "Install here",
      detail: null,
      disabled: false,
    },
    ...partition.install.map(installItem),
  ];
}

/**
 * Next selectable row for a D-pad press, skipping disabled rows so the stick
 * never parks on something that does nothing. Returns `from` when there is
 * nothing further to move to.
 *
 * `from` is not trusted to be a real row. The list is refetched while the menu
 * is open, so it can shrink out from under the focus index; walking from an
 * index past the end steps straight out of bounds in *both* directions and
 * returns it unchanged, which leaves the menu on screen with a dead D-pad. An
 * out-of-range index is therefore recovered onto the nearest enabled row rather
 * than walked from.
 */
export function nextEnabledIndex(
  items: PlayMenuItem[],
  from: number,
  direction: 1 | -1,
): number {
  if (items.length === 0) return 0;

  const clamped = Math.min(Math.max(Math.trunc(from) || 0, 0), items.length - 1);
  if (clamped !== from) return nearestEnabledIndex(items, clamped);

  for (let i = from + direction; i >= 0 && i < items.length; i += direction) {
    if (!items[i].disabled) return i;
  }
  return from;
}

/**
 * The enabled row closest to `start`, searching towards the top first.
 *
 * Towards the top on purpose: row 0 is the local action in both menus and is
 * never disabled, so this always lands somewhere the user can act.
 */
function nearestEnabledIndex(items: PlayMenuItem[], start: number): number {
  for (let i = start; i >= 0; i--) {
    if (!items[i].disabled) return i;
  }
  for (let i = start + 1; i < items.length; i++) {
    if (!items[i].disabled) return i;
  }
  return start;
}
