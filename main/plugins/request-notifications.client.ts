/**
 * Polls the server for game request decisions while signed in, on both
 * surfaces. See `composables/request-notifications.ts`.
 */
import {
  startRequestNotificationPolling,
  stopRequestNotificationPolling,
} from "~/composables/request-notifications";
import { useAppState } from "~/composables/app-state";
import { AppStatus } from "~/types";

export default defineNuxtPlugin(() => {
  if (!import.meta.client) return;

  const state = useAppState();
  // Keyed on the user as well as the status, so another account signing in
  // never sees the previous one's decisions. The user can be missing while
  // signed in (the client started offline); polling still runs for the
  // unread count, but nothing is toasted until the user is known.
  watch(
    () => [state.value?.status, state.value?.user?.id ?? null] as const,
    ([status, userId]) => {
      if (status === AppStatus.SignedIn) startRequestNotificationPolling(userId);
      else stopRequestNotificationPolling();
    },
    { immediate: true },
  );
});
