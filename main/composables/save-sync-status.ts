/**
 * The last cloud-save failure recorded per game, so "any error" is answerable
 * later rather than only in the two seconds a modal is on screen.
 *
 * `save_sync_error` fires from a launch or an exit, at whatever moment the
 * sync gave up. The plugin that listens for it shows a modal and the modal is
 * dismissed, and after that the only remaining record was a line in drop.log.
 * A settings page whose whole job is answering "are my saves backed up" cannot
 * be built on an event that has already been and gone, so the plugin also
 * writes here.
 *
 * Persisted to localStorage on purpose: a failure that happened before the app
 * was restarted is still a save that is not backed up.
 *
 * Module-level singletons, the same pattern as `library-filters.ts`.
 */
import type { SaveSyncError } from "~/types/save-sync";

const STORAGE_KEY = "drop:saveSync:failures";

/**
 * Cap on remembered failures. A user with a hundred broken games has one
 * problem, not a hundred, and an unbounded map in localStorage is a slow leak.
 * Oldest goes first.
 */
const MAX_REMEMBERED = 50;

export interface SaveSyncFailure {
  gameId: string;
  /** "check" | "upload" | "download" | "write" | "conflict" */
  phase: string;
  message: string;
  /** Epoch ms, so the page can say how long ago it happened. */
  at: number;
}

function read(): Record<string, SaveSyncFailure> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return raw === null ? {} : (JSON.parse(raw) as Record<string, SaveSyncFailure>);
  } catch {
    return {};
  }
}

const failures = ref<Record<string, SaveSyncFailure>>(read());

function persist() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(failures.value));
  } catch {
    // A full or blocked localStorage must not break the sync path itself.
  }
}

export function useSaveSyncStatus() {
  /** Remember a failure. One per game: the newest is the one that still applies. */
  function recordFailure(payload: SaveSyncError) {
    const message = payload?.message?.trim();
    if (!payload?.gameId || !message) return;

    const next = { ...failures.value };
    next[payload.gameId] = {
      gameId: payload.gameId,
      phase: payload.phase,
      message,
      at: Date.now(),
    };

    const ids = Object.keys(next);
    if (ids.length > MAX_REMEMBERED) {
      ids
        .sort((a, b) => next[a].at - next[b].at)
        .slice(0, ids.length - MAX_REMEMBERED)
        .forEach((id) => delete next[id]);
    }

    failures.value = next;
    persist();
  }

  /**
   * Forget this game's failure. Called when saves for it actually move, so a
   * problem the user has already fixed stops being reported as current.
   *
   * `phase` matters. A single launch runs the upload leg and then the download
   * leg, and both report against the same gameId. Clearing on any success meant
   * a session that failed to push a local-only save and then restored a
   * different cloud-only one recorded the upload failure and deleted it a
   * moment later, so this page went back to reporting the game as fine. Pass
   * the phase that succeeded and only a failure from that phase is cleared.
   *
   * A stored `check` failure clears on any success: the check runs before both
   * legs, so a completion of either one proves it passed this time. Omit
   * `phase` entirely for the user's own Dismiss button, which means "I have
   * read this", not "this is fixed".
   */
  function clearFailure(gameId: string, phase?: string) {
    const held = failures.value[gameId];
    if (!held) return;
    if (phase !== undefined && held.phase !== phase && held.phase !== "check") {
      return;
    }
    const next = { ...failures.value };
    delete next[gameId];
    failures.value = next;
    persist();
  }

  function clearAll() {
    failures.value = {};
    persist();
  }

  return { failures, recordFailure, clearFailure, clearAll };
}
