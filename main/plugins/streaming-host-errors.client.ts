/**
 * Surfaces host-side remote play failures.
 *
 * A stream is requested by the *other* device: you press Play on the Deck, and
 * this PC's background poller picks the request up and tries to host it. When
 * that fails there is no page waiting on a promise to reject, so the backend
 * emits `stream-host-error` and the failure would otherwise only reach
 * drop.log. That is exactly how the "Sunshine is already running from another
 * install" and credential failures stayed invisible: the remote device just
 * showed the session ending for no stated reason.
 *
 * Registered as a plugin rather than inside `useStreaming` because the poller
 * runs whether or not any streaming page is mounted.
 */
import { listen } from "@tauri-apps/api/event";

type StreamHostError = {
  sessionId: string;
  gameId: string;
  reason: string;
};

export default defineNuxtPlugin(() => {
  if (!import.meta.client) return;

  listen<StreamHostError>("stream-host-error", (event) => {
    const reason = event.payload?.reason?.trim();
    createModal(
      ModalType.Notification,
      {
        title: "Remote play couldn't start",
        description: reason
          ? `This PC couldn't host the stream. ${reason}`
          : "This PC couldn't host the stream. Check the remote play settings and try again.",
        buttonText: "Close",
      },
      (_e, c) => c(),
    );
  }).catch((e) => {
    console.warn("[streaming] failed to register stream-host-error listener:", e);
  });
});
