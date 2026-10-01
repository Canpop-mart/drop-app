/**
 * How one save compares between this device and the cloud, shared by the
 * desktop Cloud Saves panel and Big Picture's Saves tab.
 *
 * Two hashes are not enough to tell "only the cloud changed" from "both
 * changed": the panels used to call every difference a conflict, and Big
 * Picture compared modification times instead, which disagree across
 * machines. The third point is the hash this account's sync manifest recorded
 * at the last sync (`syncedHash`, with the cloud row it was recorded against).
 * The client only sends it from manifest entries the current build wrote:
 * older builds recorded declined conflicts as synced, and trusting those
 * would offer a download over the copy the user chose to keep.
 *
 * This is the same rule as `three_way_verdict` in the client's
 * `remote/src/save_sync/decide.rs`, plus the launch sync's
 * `keep_other_accounts_changes_as_conflicts`. Keep them in step.
 *
 * Pure, so it runs under `node --test main/tests/save-sync-state.test.ts`.
 */

export type SaveSyncState =
  /** Same bytes on both sides. */
  | "synced"
  /** Only on this device. */
  | "localOnly"
  /** Only in the cloud. */
  | "cloudOnly"
  /** Only the cloud changed since the last sync. */
  | "cloudNewer"
  /** Only this device changed since the last sync. */
  | "localNewer"
  /** Both changed, or there is no trustworthy record of the last sync. */
  | "conflict";

export interface LocalSide {
  dataHash: string;
  /** Hash recorded at the last sync, if this account has synced the file. */
  syncedHash?: string | null;
  /** The cloud row that last sync was recorded against. */
  syncedCloudId?: string | null;
  /**
   * Set when these bytes are what another Drop account on this PC last
   * synced. Two accounts on one PC share its save files, so "changed here"
   * can mean "the other person played". That is shown as a conflict, never
   * offered as this account's newer copy.
   */
  lastSyncedByOtherAccount?: string | null;
  /**
   * Another Drop account also syncs this game on this device. A local change
   * may be that person's progress even when their own sync failed and left
   * no record of the bytes, so it is never offered as this account's newer
   * copy.
   */
  otherAccountsOnThisDevice?: boolean;
}

export interface CloudSide {
  id: string;
  dataHash: string;
}

function sameHash(a: string | null | undefined, b: string | null | undefined) {
  return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}

export function saveSyncState(
  local: LocalSide | null,
  cloud: CloudSide | null,
): SaveSyncState {
  if (local && !cloud) return "localOnly";
  if (cloud && !local) return "cloudOnly";
  if (!local || !cloud) return "conflict";
  if (sameHash(local.dataHash, cloud.dataHash)) return "synced";

  // Only trust a record that names this very cloud row. Old manifests
  // recorded files that never reached the cloud, always without an id, and
  // trusting one would offer a download over real local progress.
  const base = local.syncedHash;
  if (!base || local.syncedCloudId !== cloud.id) return "conflict";
  const localChanged = !sameHash(local.dataHash, base);
  const cloudChanged = !sameHash(cloud.dataHash, base);
  if (!localChanged && cloudChanged) return "cloudNewer";
  if (localChanged && !cloudChanged) {
    const othersCopy =
      local.otherAccountsOnThisDevice === true ||
      (local.lastSyncedByOtherAccount !== null &&
        local.lastSyncedByOtherAccount !== undefined);
    return othersCopy ? "conflict" : "localNewer";
  }
  return "conflict";
}

/**
 * Bytes a save name may carry escaped (see `escape_relpath` in the client's
 * `save_sync/scan.rs`): `%`, `/`, `.`, space, the letters a Windows device
 * name starts with, `? < > : * | "`, and control characters.
 */
function isEscapableByte(b: number): boolean {
  if (b < 0x20) return true;
  return '%/. cCpPaAnNlL?<>:*|"'.includes(String.fromCharCode(b));
}

/**
 * Turn a cloud save name into something readable: drop the `pc__` / `pc/` /
 * `switch__` namespace and undo the escaping. Display only; every action
 * stays keyed on the wire name.
 */
export function displaySaveName(filename: string): string {
  let name = filename;
  if (name.startsWith("pc__")) name = name.slice(4);
  else if (name.startsWith("pc/")) name = name.slice(3);
  else if (name.startsWith("switch__")) name = name.slice(8);

  const out: number[] = [];
  const bytes = new TextEncoder().encode(name);
  const hex = (i: number) => {
    const s = String.fromCharCode(bytes[i] ?? 0, bytes[i + 1] ?? 0);
    return /^[0-9A-F]{2}$/.test(s) ? parseInt(s, 16) : null;
  };
  for (let i = 0; i < bytes.length; i++) {
    if (bytes[i] === 0x25) {
      const first = hex(i + 1);
      const second = bytes[i + 3] === 0x25 ? hex(i + 4) : null;
      if (first === 0xc2 && second !== null && second >= 0x80 && second <= 0x9f) {
        out.push(0xc2, second);
        i += 5;
        continue;
      }
      if (first !== null && isEscapableByte(first)) {
        out.push(first);
        i += 2;
        continue;
      }
    }
    out.push(bytes[i]);
  }
  return new TextDecoder().decode(new Uint8Array(out));
}

/** The parts of a launch conflict the notes below read. */
export interface ConflictNotes {
  localLastSyncedBy?: string | null;
  localMayBeOtherAccount?: boolean;
  cloudLegacyName?: string | null;
}

/**
 * Whose progress the file on this device may be, for the conflict prompt on
 * both surfaces. "" when nothing suggests it is not this account's.
 */
export function conflictLocalNote(conflict: ConflictNotes): string {
  const by = conflict.localLastSyncedBy;
  if (by !== null && by !== undefined) {
    return by
      ? `Last synced by ${by} on this device`
      : "Last synced by another Drop account on this device";
  }
  if (conflict.localMayBeOtherAccount) {
    return "Another Drop account also plays this game here, so this may be their progress";
  }
  return "";
}

/** A note on the cloud side of a conflict, or "". */
export function conflictCloudNote(conflict: ConflictNotes): string {
  return conflict.cloudLegacyName
    ? "Saved by an older version of Drop under a different name"
    : "";
}

/** What a cloud row stored under an older build's name is to this device. */
export interface LegacyRow {
  /** The local file it is an older copy of, or null when the old name fits
   * more than one local file (nothing is written for such a row). */
  localFilename: string | null;
  /** Same bytes as that local file: nothing to do. */
  same: boolean;
}

/**
 * Match a cloud-only row against the names older builds gave local files
 * (`legacyCloudName`, from the client's `legacy_cloud_name`). Same rule as the
 * launch sync's `resolve_legacy_rows`: a matching row is never pulled by
 * Sync; with different bytes it is the user's choice, and a restore writes
 * into the local file rather than next to it under the old name.
 */
export function matchLegacyRow(
  locals: { filename: string; dataHash: string; legacyCloudName?: string | null }[],
  cloud: { filename: string; dataHash?: string | null },
): LegacyRow | null {
  // A row whose own name is on disk is that file's row, not an old copy.
  if (locals.some((l) => l.filename === cloud.filename)) return null;
  const matches = locals.filter((l) => l.legacyCloudName === cloud.filename);
  if (matches.length === 0) return null;
  if (matches.length > 1) return { localFilename: null, same: false };
  const local = matches[0]!;
  return {
    localFilename: local.filename,
    same: sameHash(local.dataHash, cloud.dataHash),
  };
}

/**
 * The note under an older build's cloud row, on both surfaces. `localName` is
 * the display name of the local file it belongs to (null when ambiguous).
 */
export function legacyRowNote(legacy: LegacyRow, localName: string | null): string {
  if (!localName)
    return "Saved by an older version of Drop under a name that fits more than one save here, so Drop does not restore it.";
  if (legacy.same)
    return `Same as ${localName}, saved by an older version of Drop under a different name. Sync leaves it alone.`;
  return `An older version of Drop saved this copy of ${localName} under a different name, and it differs from the file on this device. Sync leaves it alone. Restore puts it in place of that file.`;
}

/**
 * A save that is only on this device, on a device where another Drop account
 * also syncs this game (`otherAccountsOnThisDevice`, set only for saves every
 * account here shares: PC saves, the Switch NAND, and emulator saves still in
 * the old shared folder). It may be that person's,
 * so it is never backed up automatically: the launch sync skips it, and so
 * does the panel's Sync. Backing it up by hand still works.
 */
export function heldOnSharedDeviceState(
  state: string,
  local: { otherAccountsOnThisDevice?: boolean } | null,
): boolean {
  return state === "localOnly" && local?.otherAccountsOnThisDevice === true;
}
