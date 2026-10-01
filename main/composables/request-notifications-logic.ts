/**
 * Pure helpers for game request notifications (approved, denied, added to
 * the library). The server sends these with a nonce of
 * `request-<kind>-<requestId>`; see drop-server server/internal/requests.
 *
 * Kept free of Vue and Tauri so `main/tests/request-notifications.test.ts`
 * can run them on the plain Node test runner.
 */

/** The notification fields this module reads. */
export type ServerNotification = {
  id: string;
  nonce: string | null;
  title: string;
  description: string;
  read: boolean;
  created: string;
};

export type RequestDecisionKind = "approved" | "denied" | "fulfilled";

const NONCE_PATTERN = /^request-(approved|denied|fulfilled)-/;

/** What a notification says about a request, or null if it is not one. */
export function requestDecisionKind(
  n: Pick<ServerNotification, "nonce">,
): RequestDecisionKind | null {
  const match = n.nonce ? NONCE_PATTERN.exec(n.nonce) : null;
  return match ? (match[1] as RequestDecisionKind) : null;
}

/** Unread request decisions, oldest first. */
export function unreadRequestDecisions<T extends ServerNotification>(
  list: T[],
): T[] {
  return list
    .filter((n) => !n.read && requestDecisionKind(n) !== null)
    .sort((a, b) => Date.parse(a.created) - Date.parse(b.created));
}

/**
 * Which unread decisions still need a toast: ones not toasted before, oldest
 * first, at most `max` (the rest stay in the unread count rather than
 * flooding the screen after a long time offline).
 */
export function pickNewToasts<T extends ServerNotification>(
  list: T[],
  alreadyToasted: ReadonlySet<string>,
  max = 3,
): T[] {
  return unreadRequestDecisions(list)
    .filter((n) => !alreadyToasted.has(n.id))
    .slice(-max);
}

/**
 * Keeps the remembered toast ids bounded. Ids of notifications that no
 * longer exist on the server are dropped first; then the newest `max` win.
 */
export function pruneToastedIds(
  toasted: readonly string[],
  stillPresent: ReadonlySet<string>,
  max = 200,
): string[] {
  return toasted.filter((id) => stillPresent.has(id)).slice(-max);
}

/**
 * How long to wait before the next poll after `failures` failed polls in a
 * row: the normal interval, doubling per failure, capped at `max`. A server
 * that refuses the poll (an older one answers 403) is retried at the capped
 * rate rather than given up on, so updating the server brings the
 * notifications back without restarting the client.
 */
export function nextPollDelay(
  failures: number,
  base = 60_000,
  max = 30 * 60_000,
): number {
  if (failures <= 0) return base;
  // Cap the exponent too, so a long outage never overflows to Infinity.
  return Math.min(base * 2 ** Math.min(failures, 20), max);
}

/**
 * localStorage key for the ids already toasted, per user, so a different
 * account signing in on the same machine starts from its own list. There is
 * no key for an unknown user: nothing is toasted until the user is known.
 */
export function toastedStorageKey(userId: string): string {
  return `drop:requestNotificationsToasted:${userId}`;
}

/** Where every account's toasted ids were kept before they were per user. */
export const LEGACY_TOASTED_STORAGE_KEY = "drop:requestNotificationsToasted";

/**
 * One-time move of the shared, pre-per-user toasted ids into a user's list,
 * so upgrading does not toast decisions again. Ids already in the user's
 * list stay first; the result has no duplicates.
 */
export function mergeLegacyToasted(
  current: readonly string[],
  legacy: readonly string[],
): string[] {
  return [...new Set([...current, ...legacy])];
}

/**
 * Message-triggered polls are spaced at least `minGapMs` apart. Returns how
 * long to wait before the next one (0 = now).
 */
export function messagePollDelay(
  now: number,
  lastMessagePollAt: number | null,
  minGapMs = 5_000,
): number {
  if (lastMessagePollAt === null) return 0;
  return Math.max(0, lastMessagePollAt + minGapMs - now);
}

/**
 * The message drop-server's request board (pages/requests.vue) posts to the
 * window embedding it after it marks request decisions read, so the client
 * can re-poll instead of showing a stale unread count.
 */
export const REQUEST_DECISIONS_READ_MESSAGE = "drop:request-decisions-read";

export function isRequestDecisionsReadMessage(data: unknown): boolean {
  return (
    typeof data === "object" &&
    data !== null &&
    (data as { type?: unknown }).type === REQUEST_DECISIONS_READ_MESSAGE
  );
}
