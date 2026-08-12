/**
 * Telling "this game has saves on the server" apart from "your saves for this
 * game are backed up".
 *
 * `/saves/summary` returns one row per game the caller can READ. PC saves are
 * shared across every account on a Drop server, so on a two-account family
 * server the second user gets a row for every PC game the first one has ever
 * played, with `fileCount > 0` and none of it theirs. Counting those rows is
 * how a tile ends up wearing a cloud badge for a game its owner has never
 * launched, and how a settings page tells someone nine games are backed up when
 * they own none of it.
 *
 * Every surface that asserts a backup belongs to the user goes through here.
 *
 * The fallback exists because a Drop server can be older than its client.
 * `ownCount` is absent there, and `fileCount - sharedCount` is the closest
 * honest answer the old shape can give: it undercounts a save of yours that a
 * housemate's newer copy is currently shadowing, which is the safe direction.
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
