/**
 * App-wide co-op room upkeep, on both surfaces.
 *
 * - Member polling runs whenever a room is active, not only while a
 *   multiplayer page is open. That poll is what notices the host ending the
 *   room (joiners then run the full leave cleanup and get a toast), and each
 *   poll also pushes the current peer list into any launched co-op game's
 *   `custom_broadcasts.txt` on the Rust side.
 * - Once signed in, it asks the server whether this device is still in a room
 *   from before a restart or crash, and if so shows a toast pointing at the
 *   multiplayer page, where Rejoin / Leave are offered.
 */
import { useCoopRoom } from "~/composables/coop-room";
import { useAppState } from "~/composables/app-state";
import { AppStatus } from "~/types";

export default defineNuxtPlugin(() => {
  if (!import.meta.client) return;

  const coop = useCoopRoom();
  const state = useAppState();

  watch(
    () => coop.room.value?.roomId ?? null,
    (roomId) => {
      if (roomId) coop.startPolling();
      else coop.stopPolling();
    },
    { immediate: true },
  );

  let checked = false;
  watch(
    () => state.value?.status,
    async (status) => {
      if (checked || status !== AppStatus.SignedIn) return;
      checked = true;
      const pending = await coop.checkMine();
      if (!pending) return;
      // The toast host picks the wording for whichever surface is showing
      // (BPM is usually entered a moment after startup).
      coop.pushToast(
        "You're still in a co-op room. Open Co-op Rooms from the people icon in the top bar to rejoin or leave it.",
        "You're still in a co-op room. Open Multiplayer to rejoin or leave it.",
      );
    },
    { immediate: true },
  );
});
