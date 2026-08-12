/**
 * Which of the account's other devices may be offered as a remote target for
 * one game, and what the Play / Install dropdown is made of.
 *
 * Pure on purpose: no Vue, no Tauri, no clock of its own. Everything the rules
 * need is passed in, so `tests/stream-targets.test.ts` can pin the behaviour
 * for every self / installed / reachable combination.
 *
 * Three rules decide what the user is allowed to see:
 *
 *  1. The machine you are sitting at is never a remote target. The server's
 *     `isSelf` only marks the registration you are authenticated as, so a
 *     second registration of the same physical machine (a re-pair makes a new
 *     client row, the old one stays) is not covered by it. `isOwnDevice` adds
 *     the local machine's own identity as a second check.
 *  2. "Play on X" requires the game to be installed on X. That is `hasGame`,
 *     which the server derives from the install list X last reported.
 *  3. Both remote actions require X to be reachable. A stream is push-based:
 *     the request sits on the server until X's poller picks it up, and the
 *     server expires it after two minutes. A device that is powered off can
 *     neither host a stream nor start a remote install, so offering either is
 *     offering something that cannot happen.
 *
 * `lastConnected` is the liveness signal. Every authenticated request bumps it
 * (debounced to once a minute server-side) and a signed-in client polls for
 * pending stream requests every ten seconds, so a device that is running Drop
 * is always inside the window below and one that is off is not.
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

/** What this client knows about the machine it is running on. */
export interface LocalIdentity {
  /** `platform()` from the OS plugin: "windows", "linux", "macos". */
  platform: string;
  /** OS hostname, or null when it could not be read. */
  hostname: string | null;
  /** The name this client pushes to the server for itself, if known. */
  displayName: string | null;
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

/** Registration names Drop gives a machine before anything renames it. */
function hostnameDeviceName(hostname: string): string {
  return `${hostname} (Desktop)`;
}

function normalise(value: string | null | undefined): string {
  return (value ?? "").trim().toLowerCase();
}

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
 * Is this registration the machine we are running on?
 *
 * `isSelf` is authoritative when true but misses stale registrations of this
 * same machine, so two local names are checked as well:
 *
 *  - `{hostname} (Desktop)` is what auth registers, and it names one machine,
 *    so it is trusted on its own.
 *  - The display name is not: renaming a device pushes the same account name
 *    to every device the user owns, so matching on it alone would hide a real
 *    Steam Deck. It only counts as us when the entry is also unreachable,
 *    which is exactly the shape of a dead registration this machine left
 *    behind.
 *
 * Both name checks require the platform to match, so the Linux half of a
 * dual-boot box is never mistaken for the Windows half. Being the same
 * hardware, it cannot be running at the same time, and rule 3 covers it.
 */
export function isOwnDevice(
  device: DeviceLike,
  local: LocalIdentity | null,
  online: boolean,
): boolean {
  if (device.isSelf) return true;
  if (!local) return false;
  if (normalise(device.platform) !== normalise(local.platform)) return false;

  const name = normalise(device.name);
  if (local.hostname && name === normalise(hostnameDeviceName(local.hostname))) {
    return true;
  }
  if (local.displayName && name === normalise(local.displayName) && !online) {
    return true;
  }
  return false;
}

/**
 * Collapse repeat registrations of one machine.
 *
 * Same name and platform means the same device registered more than once.
 * The survivor is the one most worth offering: reachable first, then the one
 * that has the game, then the most recently seen.
 */
function dedupe(targets: DeviceTarget[]): DeviceTarget[] {
  const byKey = new Map<string, DeviceTarget>();
  for (const target of targets) {
    const key = `${normalise(target.device.name)}::${normalise(target.device.platform)}`;
    const existing = byKey.get(key);
    if (!existing || beatsExisting(target, existing)) byKey.set(key, target);
  }
  return [...byKey.values()];
}

function beatsExisting(candidate: DeviceTarget, existing: DeviceTarget): boolean {
  if (candidate.online !== existing.online) return candidate.online;
  const candidateHas = candidate.device.hasGame === true;
  const existingHas = existing.device.hasGame === true;
  if (candidateHas !== existingHas) return candidateHas;
  return candidate.device.lastConnected > existing.device.lastConnected;
}

export interface PartitionInput {
  local: LocalIdentity | null;
  nowMs: number;
  windowMs?: number;
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
 */
export function partitionDevices(
  devices: DeviceLike[],
  { local, nowMs, windowMs = DEVICE_ONLINE_WINDOW_MS }: PartitionInput,
): DevicePartition {
  const others: DeviceTarget[] = [];
  for (const device of devices) {
    const online = isDeviceOnline(device, nowMs, windowMs);
    if (isOwnDevice(device, local, online)) continue;
    others.push({ device, online });
  }

  const unique = dedupe(others);
  return {
    stream: unique.filter((t) => t.device.hasGame === true),
    install: unique.filter((t) => t.device.hasGame === false),
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
