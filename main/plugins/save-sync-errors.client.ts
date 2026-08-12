/**
 * Surfaces cloud-save sync failures.
 *
 * Every failure in the sync used to be a `warn!` into drop.log. That is the
 * one place it must not be: a save that never reached the cloud looks exactly
 * like a save that did, right up until the device it was on dies. The Rust
 * side now emits `save_sync_error` at each failing step and this turns it into
 * something on screen.
 *
 * Registered as a plugin, not on a page, for the same reason as
 * `streaming-host-errors.client.ts`: the sync runs at launch and at exit, from
 * whichever surface pressed Play, and the post-exit upload can fail long after
 * every game page has been navigated away from.
 */
import { listen } from "@tauri-apps/api/event";
import { useSaveSyncStatus } from "~/composables/save-sync-status";
import type { SaveSyncError } from "~/types/save-sync";

/**
 * How long the same failure is suppressed for. One launch can fail its
 * sync-check, its download and its upload off a single dead connection, and
 * three identical modals in a row is worse than one.
 */
const REPEAT_WINDOW_MS = 60_000;

export default defineNuxtPlugin(() => {
  if (!import.meta.client) return;

  const lastShown = new Map<string, number>();
  const { recordFailure } = useSaveSyncStatus();

  listen<SaveSyncError>("save_sync_error", (event) => {
    const payload = event.payload;
    const message = payload?.message?.trim();
    if (!message) return;

    // Recorded before the dedup check, and regardless of whether a modal is
    // shown: the Cloud Saves settings page answers "any error" off this, and a
    // failure suppressed as a repeat is still a save that is not backed up.
    recordFailure(payload);

    const key = `${payload.gameId}|${payload.phase}|${message}`;
    const now = Date.now();
    const previous = lastShown.get(key);
    if (previous !== undefined && now - previous < REPEAT_WINDOW_MS) return;
    lastShown.set(key, now);

    createModal(
      ModalType.Notification,
      {
        // The Rust side writes the whole message: it knows which file, which
        // step and whether retrying helps, and splitting the copy across two
        // languages is how it drifts.
        title: "Cloud saves didn't sync",
        description: message,
        buttonText: "Close",
      },
      (_e, c) => c(),
    );
  }).catch((e) => {
    console.warn("[save-sync] failed to register save_sync_error listener:", e);
  });
});
