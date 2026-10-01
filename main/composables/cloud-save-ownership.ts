/**
 * Telling "this game has saves on the server" apart from "your saves for this
 * game are backed up".
 *
 * A current Drop server reads saves strictly per account, so the two are the
 * same and `ownCount` equals `fileCount`. An older server read PC saves across
 * every account, and on a family server the second user got a row for every
 * PC game the first had played, none of it theirs. Every surface that asserts
 * a backup belongs to the user still goes through here, so it stays right
 * against either.
 *
 * The fallback exists because a Drop server can be older than its client.
 * `ownCount` is absent there, and `fileCount - sharedCount` is the closest
 * honest answer the old shape can give.
 */
import type { CloudSaveGameSummary } from "~/composables/use-server-api";

/** How many of this game's cloud files are the signed-in user's own. */
export function ownSaveCount(summary: CloudSaveGameSummary): number {
  if (typeof summary.ownCount === "number") return summary.ownCount;
  return Math.max(summary.fileCount - summary.sharedCount, 0);
}

/**
 * How many bytes of this game's cloud files are the signed-in user's own.
 *
 * With no `ownBytes` there is no way to split a mixed game's total, so a mixed
 * game contributes nothing rather than a guess.
 */
export function ownSaveBytes(summary: CloudSaveGameSummary): number {
  if (typeof summary.ownBytes === "number") return summary.ownBytes;
  return summary.sharedCount === 0 ? summary.totalBytes : 0;
}

/** Whether the user has anything of their own backed up for this game. */
export function hasOwnSaves(summary: CloudSaveGameSummary): boolean {
  return ownSaveCount(summary) > 0;
}
