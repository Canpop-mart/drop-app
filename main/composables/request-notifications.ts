/**
 * Game request decisions (approved, denied, added to the library) from the
 * server's notification list, for both surfaces.
 *
 * The desktop client has no notification UI of its own: the server's bell
 * lives in its web header, which is hidden inside the client's iframes. So
 * the client polls `/api/v1/notifications` while signed in (started by
 * `plugins/request-notifications.client.ts`) and:
 *  - toasts each new decision once (`RequestNotificationToastHost`, mounted
 *    in app.vue so it shows on both surfaces),
 *  - exposes the unread count for the desktop header bell and the Big Picture
 *    nav rail,
 *  - marks decisions read when the user opens their requests (Big Picture's
 *    My requests tab, or the desktop bell). The server's own request board,
 *    shown in the desktop iframe, marks them read itself when its My
 *    requests view opens and posts REQUEST_DECISIONS_READ_MESSAGE to this
 *    window, which triggers a re-poll. Only a message from the request board
 *    iframe the desktop page registered counts (`registerRequestBoardFrame`),
 *    and such polls are spaced at least a few seconds apart.
 *
 * Only request decisions are handled here; other notification types are
 * left alone. The poll needs the server to grant client tokens
 * `notifications:read`. A failed poll, including the 403 an older server
 * answers, is retried with a growing delay (`nextPollDelay`), never given
 * up on.
 *
 * Everything held here belongs to the signed-in user: signing out clears it,
 * and the remembered toast ids are stored per user. While the user is not
 * known (the client started offline) the count is kept but nothing is
 * toasted, since there is no list to check against.
 */
import { serverUrl } from "./use-server-fetch";
import {
  isRequestDecisionsReadMessage,
  LEGACY_TOASTED_STORAGE_KEY,
  mergeLegacyToasted,
  messagePollDelay,
  nextPollDelay,
  pickNewToasts,
  pruneToastedIds,
  requestDecisionKind,
  toastedStorageKey,
  unreadRequestDecisions,
  type RequestDecisionKind,
  type ServerNotification,
} from "./request-notifications-logic";

const TOAST_DURATION_MS = 8_000;

export type RequestToast = {
  id: string;
  kind: RequestDecisionKind;
  title: string;
  description: string;
};

const notifications = ref<ServerNotification[]>([]);
const toasts = ref<RequestToast[]>([]);
let timer: ReturnType<typeof setTimeout> | undefined;
let failures = 0;
// The user being polled for; null while signed out.
let activeUserId: string | null = null;
let active = false;
// Bumped on every start and stop. A poll or mark that was in flight across a
// sign-out or account switch compares its captured value and throws its
// result away instead of writing the previous user's data back.
let generation = 0;
// The generation whose poll is in flight, so polls never overlap within one
// session but a new session is never blocked by an old one's request.
let inFlightGeneration = -1;
// Set when a poll is asked for while one is in flight: that one may have
// been sent before the change it should pick up, so poll again after it.
let pollAgain = false;
// Message-triggered polls: when the last one ran, and the one waiting to.
let lastMessagePollAt: number | null = null;
let messagePollTimer: ReturnType<typeof setTimeout> | undefined;
// The desktop request board iframe's window, the only sender whose
// REQUEST_DECISIONS_READ_MESSAGE is trusted.
let requestBoardWindow: (() => Window | null) | null = null;

function loadToasted(userId: string): string[] {
  try {
    const raw = localStorage.getItem(toastedStorageKey(userId));
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed.filter((x): x is string => typeof x === "string")
      : [];
  } catch (e) {
    // Unreadable storage only means a decision may be toasted twice.
    console.warn("[REQUESTS] could not read toasted notification ids:", e);
    return [];
  }
}

function saveToasted(userId: string, ids: string[]) {
  try {
    localStorage.setItem(toastedStorageKey(userId), JSON.stringify(ids));
  } catch (e) {
    console.warn("[REQUESTS] could not save toasted notification ids:", e);
  }
}

/**
 * Moves the ids toasted before they were stored per user into this user's
 * list, once, so upgrading does not toast old decisions again. They belong
 * to whoever used this machine before the upgrade, most likely this user;
 * for anyone else they are ids of notifications they do not have, which
 * pruning drops on the next poll.
 */
function migrateLegacyToasted(userId: string) {
  let raw: string | null;
  try {
    raw = localStorage.getItem(LEGACY_TOASTED_STORAGE_KEY);
  } catch (e) {
    // Storage unreadable: nothing to migrate, at worst a repeated toast.
    console.warn("[REQUESTS] could not read old toasted ids:", e);
    return;
  }
  if (raw === null) return;
  let legacy: string[] = [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed))
      legacy = parsed.filter((x): x is string => typeof x === "string");
  } catch (e) {
    // Corrupt old list: dropping it only risks a repeated toast.
    console.warn("[REQUESTS] old toasted ids were unreadable:", e);
  }
  saveToasted(userId, mergeLegacyToasted(loadToasted(userId), legacy));
  try {
    localStorage.removeItem(LEGACY_TOASTED_STORAGE_KEY);
  } catch (e) {
    // Harmless: the merge above is idempotent, so a retry next start is safe.
    console.warn("[REQUESTS] could not remove old toasted ids:", e);
  }
}

function showToast(n: ServerNotification) {
  const kind = requestDecisionKind(n);
  if (!kind) return;
  toasts.value = [
    ...toasts.value,
    { id: n.id, kind, title: n.title, description: n.description },
  ];
  setTimeout(() => {
    toasts.value = toasts.value.filter((t) => t.id !== n.id);
  }, TOAST_DURATION_MS);
}

function schedule(delayMs: number) {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void poll(), delayMs);
}

async function poll() {
  if (!active) return;
  if (inFlightGeneration === generation) {
    pollAgain = true;
    return;
  }
  const gen = generation;
  const userId = activeUserId;
  inFlightGeneration = gen;
  let ok = false;
  try {
    const resp = await fetch(serverUrl("api/v1/notifications"));
    if (gen !== generation) return;
    if (resp.status === 403) {
      console.warn(
        "[REQUESTS] server does not let this client read notifications (403); retrying later. An older server needs updating for request decision toasts.",
      );
      return;
    }
    if (!resp.ok) {
      console.warn("[REQUESTS] notification poll failed:", resp.status);
      return;
    }
    const list = (await resp.json()) as ServerNotification[];
    if (gen !== generation) return;
    notifications.value = list;
    ok = true;

    // No user yet: no list to check toasts against, so none are shown.
    // They come once the user is known and polling restarts for them.
    if (userId === null) return;
    const toasted = loadToasted(userId);
    const fresh = pickNewToasts(list, new Set(toasted));
    for (const n of fresh) showToast(n);
    saveToasted(
      userId,
      pruneToastedIds(
        [...toasted, ...fresh.map((n) => n.id)],
        new Set(list.map((n) => n.id)),
      ),
    );
  } catch (e) {
    // Offline or server unreachable: retried after the backoff delay.
    console.warn("[REQUESTS] notification poll error:", e);
  } finally {
    if (gen === generation) {
      inFlightGeneration = -1;
      failures = ok ? 0 : failures + 1;
      schedule(pollAgain ? 0 : nextPollDelay(failures));
      pollAgain = false;
    }
  }
}

function onWindowMessage(ev: MessageEvent) {
  if (!isRequestDecisionsReadMessage(ev.data)) return;
  // Trusted only from the registered request board iframe. The window
  // object is compared rather than ev.origin: the iframe is served over the
  // server:// custom protocol, whose URL differs by platform (Tauri's
  // convertFileSrc gives http://server.localhost/ on Windows and
  // server://localhost/ on Linux) and whose origin as reported in a message
  // event has not been checked on either. A window reference does not
  // depend on that and cannot be forged by another frame.
  const board = requestBoardWindow?.();
  if (!board || ev.source !== board) return;
  if (!active || messagePollTimer) return;
  messagePollTimer = setTimeout(
    () => {
      messagePollTimer = undefined;
      lastMessagePollAt = Date.now();
      requestNotificationsPollNow();
    },
    messagePollDelay(Date.now(), lastMessagePollAt),
  );
}

/**
 * The desktop request board page registers its iframe here so its "read"
 * message is trusted. Returns the unregister function.
 */
export function registerRequestBoardFrame(getWindow: () => Window | null) {
  requestBoardWindow = getWindow;
  return () => {
    if (requestBoardWindow === getWindow) requestBoardWindow = null;
  };
}

/**
 * Starts polling for `userId`. Calling it again for the same user is a
 * no-op; for a different user it drops everything held for the previous one
 * first.
 */
export function startRequestNotificationPolling(userId: string | null) {
  if (active && userId === activeUserId) return;
  stopRequestNotificationPolling();
  active = true;
  activeUserId = userId;
  generation++;
  failures = 0;
  if (userId !== null) migrateLegacyToasted(userId);
  window.addEventListener("message", onWindowMessage);
  void poll();
}

/** Stops polling and forgets the signed-in user's notifications. */
export function stopRequestNotificationPolling() {
  generation++;
  inFlightGeneration = -1;
  pollAgain = false;
  active = false;
  activeUserId = null;
  failures = 0;
  if (timer) clearTimeout(timer);
  timer = undefined;
  if (messagePollTimer) clearTimeout(messagePollTimer);
  messagePollTimer = undefined;
  window.removeEventListener("message", onWindowMessage);
  notifications.value = [];
  toasts.value = [];
}

/**
 * Polls now instead of waiting for the timer (and resets the timer). Used
 * after something else changed the read state on the server.
 */
export function requestNotificationsPollNow() {
  if (!active) return;
  if (timer) clearTimeout(timer);
  timer = undefined;
  void poll();
}

/**
 * Marks every unread request decision read on the server. Called when the
 * user opens their requests on either surface. A failed mark leaves the
 * notification unread, so the count stays honest and the next visit retries.
 */
export async function markRequestDecisionsRead() {
  const gen = generation;
  const unread = unreadRequestDecisions(notifications.value);
  await Promise.all(
    unread.map(async (n) => {
      try {
        const resp = await fetch(
          serverUrl(`api/v1/notifications/${encodeURIComponent(n.id)}/read`),
          { method: "POST" },
        );
        if (!resp.ok) {
          console.warn("[REQUESTS] mark read failed:", n.id, resp.status);
          return;
        }
        // Signed out or switched account meanwhile: not this list any more.
        if (gen !== generation) return;
        notifications.value = notifications.value.map((x) =>
          x.id === n.id ? { ...x, read: true } : x,
        );
      } catch (e) {
        console.warn("[REQUESTS] mark read error:", n.id, e);
      }
    }),
  );
  // A poll sent before these marks landed would put them back as unread
  // until the next interval; fetch the list again so it settles now.
  if (unread.length > 0 && gen === generation) requestNotificationsPollNow();
}

export function useRequestNotifications() {
  const unreadCount = computed(
    () => unreadRequestDecisions(notifications.value).length,
  );
  return { toasts, unreadCount };
}
