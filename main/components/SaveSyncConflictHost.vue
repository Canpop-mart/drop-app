<template>
  <BigpictureBpmSaveConflictDialog
    v-if="current && bigPictureActive"
    :visible="dialogVisible"
    :game-id="current.gameId"
    :conflicts="current.conflicts"
    :seconds-left="secondsLeft"
    @resolved="close"
    @dismissed="dismiss"
  />
  <SaveConflictDialog
    v-else-if="current"
    :visible="dialogVisible"
    :game-id="current.gameId"
    :conflicts="current.conflicts"
    :seconds-left="secondsLeft"
    @resolved="close"
    @dismissed="dismiss"
  />
</template>

<script setup lang="ts">
/**
 * App-level host for the cloud-save conflict dialog.
 *
 * The listener used to live on the two game pages, which meant it only fired
 * if the page for that exact game happened to be mounted. Big Picture's grid
 * quick-launch had no listener at all, so a conflict started there was
 * invisible: the launch simply sat there until the backend gave up. Mounted
 * from `app.vue` alongside ModalStack, this hears every conflict wherever Play
 * was pressed, and picks the dialog that matches the current mode.
 */
import { invoke } from "@tauri-apps/api/core";
import { useListen } from "~/composables/useListen";
import { useBigPictureMode } from "~/composables/big-picture";
import type { SaveConflictEvent } from "~/types/save-sync";

const { isActive } = useBigPictureMode();
const bigPictureActive = isActive;

const current = ref<SaveConflictEvent | null>(null);
/**
 * Mounting and showing are separate ticks on purpose. Both dialogs act on a
 * false to true transition of `visible` — the Big Picture one grabs the
 * gamepad input lock there — and a component mounted already visible never
 * sees that transition.
 */
const dialogVisible = ref(false);
/** Conflicts that arrived while another dialog was up (two games launching). */
const queue = ref<SaveConflictEvent[]>([]);
const secondsLeft = ref(0);

let ticker: ReturnType<typeof setInterval> | undefined;

useListen<SaveConflictEvent>("save_sync_conflict", (event) => {
  if (current.value) {
    queue.value.push(event.payload);
    return;
  }
  show(event.payload);
});

function show(payload: SaveConflictEvent) {
  current.value = payload;
  nextTick(() => {
    dialogVisible.value = true;
  });
  // Count the real deadline down, not a number the UI invented: the backend
  // sends its own resolve timeout so the two can never drift apart.
  secondsLeft.value = payload.timeoutSecs;
  stopTicker();
  ticker = setInterval(() => {
    secondsLeft.value = Math.max(0, secondsLeft.value - 1);
    // The backend has stopped waiting and synced nothing. It emits its own
    // `save_sync_error` for that, so this just clears the dead dialog.
    if (secondsLeft.value === 0) close();
  }, 1000);
}

function stopTicker() {
  if (ticker !== undefined) clearInterval(ticker);
  ticker = undefined;
}

function close() {
  stopTicker();
  // Hide first, unmount after the watchers have seen it: the Big Picture
  // dialog releases the gamepad input lock on that same transition, and
  // unmounting in the same tick would strand the whole UI locked.
  dialogVisible.value = false;
  nextTick(() => {
    current.value = null;
    const next = queue.value.shift();
    if (next) show(next);
  });
}

/**
 * Closed without a choice. Answer explicitly with `skip` for every file rather
 * than leaving the channel silent: the launch continues immediately instead of
 * stalling for the rest of the timeout, and the backend treats a skip exactly
 * like the timeout, so neither copy of the save is touched.
 */
async function dismiss() {
  const payload = current.value;
  close();
  if (!payload) return;
  try {
    await invoke("resolve_save_conflicts", {
      payload: {
        gameId: payload.gameId,
        resolutions: payload.conflicts.map((c) => ({
          filename: c.filename,
          choice: "skip",
        })),
      },
    });
  } catch (e) {
    // The launch falls back to its own timeout, which has the same effect.
    console.warn("[SAVE-SYNC] Failed to defer conflict resolution:", e);
  }
}

onUnmounted(stopTicker);
</script>
