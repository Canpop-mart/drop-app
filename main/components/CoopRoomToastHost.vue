<template>
  <Teleport to="body">
    <TransitionGroup
      tag="div"
      class="fixed bottom-24 left-1/2 z-[9998] flex -translate-x-1/2 flex-col items-center gap-3 pointer-events-none"
      enter-active-class="transition-all duration-300 ease-out"
      enter-from-class="translate-y-4 opacity-0"
      enter-to-class="translate-y-0 opacity-100"
      leave-active-class="transition-all duration-200 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-for="toast in toasts"
        :key="toast.id"
        role="status"
        class="flex items-center gap-3 px-4 py-3 bg-zinc-900 ring-1 ring-blue-500/30 rounded-xl shadow-2xl max-w-md"
      >
        <div
          class="size-10 rounded-lg shrink-0 bg-blue-500/10 flex items-center justify-center"
        >
          <UserGroupIcon class="size-5 text-blue-400" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-xs font-medium text-blue-400 uppercase tracking-wide">
            Co-op
          </p>
          <p class="text-sm font-semibold text-zinc-100">
            {{ bigPicture.isActive.value && toast.bpmMessage ? toast.bpmMessage : toast.message }}
          </p>
        </div>
      </div>
    </TransitionGroup>
  </Teleport>
</template>

<script setup lang="ts">
/**
 * Co-op room notices ("the host ended the room", "you're still in a room"),
 * pushed by `useCoopRoom().pushToast`. Mounted from `app.vue` so it shows on
 * both surfaces and whatever page is open, like `SaveSyncToastHost`.
 *
 * Display only, with no buttons, so there is nothing for the Big Picture focus
 * system to register and nothing that could hold the input lock. Bottom centre
 * because the achievement and cloud-save toasts own the bottom corners; high
 * enough to clear the Big Picture context bar.
 */
import { UserGroupIcon } from "@heroicons/vue/24/outline";
import { useCoopRoom } from "~/composables/coop-room";
import { useBigPictureMode } from "~/composables/big-picture";

const { toasts } = useCoopRoom();
const bigPicture = useBigPictureMode();
</script>
