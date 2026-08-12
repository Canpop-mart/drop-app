<template>
  <Teleport to="body">
    <TransitionGroup
      tag="div"
      class="fixed bottom-6 left-6 z-[9998] flex flex-col gap-3 pointer-events-none"
      enter-active-class="transition-all duration-500 ease-out"
      enter-from-class="-translate-x-full opacity-0"
      enter-to-class="translate-x-0 opacity-100"
      leave-active-class="transition-all duration-300 ease-in"
      leave-from-class="translate-x-0 opacity-100"
      leave-to-class="-translate-x-full opacity-0"
    >
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="pointer-events-auto flex items-center gap-3 px-4 py-3 bg-zinc-900 ring-1 ring-cyan-500/30 rounded-xl shadow-2xl shadow-cyan-500/10 max-w-sm"
      >
        <div
          class="size-10 rounded-lg shrink-0 bg-cyan-500/10 flex items-center justify-center"
        >
          <component
            :is="toast.phase === 'download' ? CloudArrowDownIcon : CloudArrowUpIcon"
            class="size-5 text-cyan-400"
          />
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-xs font-medium text-cyan-400 uppercase tracking-wide">
            Cloud saves
          </p>
          <p class="text-sm font-semibold text-zinc-100">
            {{ toast.message }}
          </p>
          <p v-if="toast.gameName" class="text-xs text-zinc-400 truncate">
            {{ toast.gameName }}
          </p>
        </div>
      </div>
    </TransitionGroup>
  </Teleport>
</template>

<script setup lang="ts">
/**
 * "Backed up 3 saves for Tony Hawk's Pro Skater 3+4" — on screen, once, at the
 * moment it happens.
 *
 * This is the reassuring half of the pair. `save-sync-errors.client.ts` owns
 * the failures and puts them in a modal, because a save that never reached the
 * cloud is worth stopping someone for. A backup that worked is not, so it gets
 * the same treatment `AchievementToast` gives an unlock: a corner, six seconds,
 * no button.
 *
 * Mounted from `app.vue` alongside the achievement toasts rather than on a
 * page. The post-exit upload finishes long after whichever page launched the
 * game has been navigated away from, and the pre-launch restore can fire from
 * the library, the Big Picture detail page, or Big Picture quick-launch.
 *
 * Bottom LEFT on purpose: `AchievementToast` owns bottom right, and a game
 * exiting can produce both at once.
 */
import {
  CloudArrowDownIcon,
  CloudArrowUpIcon,
} from "@heroicons/vue/24/outline";
import { useSaveSyncStatus } from "~/composables/save-sync-status";
import { useListen } from "~/composables/useListen";
import type { SaveSyncComplete } from "~/types/save-sync";

interface SaveSyncToast {
  id: string;
  message: string;
  gameName: string | null;
  phase: string;
}

/** Matches the achievement toast so a burst of both behaves consistently. */
const TOAST_LIFETIME_MS = 6_000;
const MAX_VISIBLE_TOASTS = 3;

/**
 * Same suppression window as the error listener. A launch that restores saves
 * and an exit that backs them up are different phases, so the key includes it
 * and both still show.
 */
const REPEAT_WINDOW_MS = 10_000;

const toasts = ref<SaveSyncToast[]>([]);
const lastShown = new Map<string, number>();
const { clearFailure } = useSaveSyncStatus();

useListen<SaveSyncComplete>("save_sync_complete", (event) => {
  const payload = event.payload;
  const message = payload?.message?.trim();
  if (!message || !payload.count) return;

  // Saves moved, so whatever the settings page is still reporting for THIS
  // PHASE is a problem the user has already fixed. Keyed on the phase because a
  // launch runs both legs: a successful download must not erase the record of
  // the upload that failed minutes earlier, which is the only thing keeping the
  // settings page honest after the modal is gone.
  clearFailure(payload.gameId, payload.phase);

  const key = `${payload.gameId}|${payload.phase}|${message}`;
  const now = Date.now();
  for (const [k, ts] of lastShown) {
    if (now - ts > REPEAT_WINDOW_MS) lastShown.delete(k);
  }
  if (lastShown.has(key)) return;
  lastShown.set(key, now);

  const toast: SaveSyncToast = {
    id: `${key}-${now}`,
    message,
    gameName: payload.gameName || null,
    phase: payload.phase,
  };
  toasts.value.push(toast);
  if (toasts.value.length > MAX_VISIBLE_TOASTS) {
    toasts.value = toasts.value.slice(-MAX_VISIBLE_TOASTS);
  }

  setTimeout(() => {
    toasts.value = toasts.value.filter((t) => t.id !== toast.id);
  }, TOAST_LIFETIME_MS);
});
</script>
