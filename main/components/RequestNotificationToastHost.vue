<template>
  <Teleport to="body">
    <TransitionGroup
      tag="div"
      class="fixed top-20 left-1/2 z-[9998] flex -translate-x-1/2 flex-col items-center gap-3 pointer-events-none"
      enter-active-class="transition-all duration-300 ease-out"
      enter-from-class="-translate-y-4 opacity-0"
      enter-to-class="translate-y-0 opacity-100"
      leave-active-class="transition-all duration-200 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-for="toast in toasts"
        :key="toast.id"
        role="status"
        class="flex items-center gap-3 px-4 py-3 bg-zinc-900 rounded-xl shadow-2xl max-w-md ring-1"
        :class="toast.kind === 'denied' ? 'ring-red-500/30' : 'ring-green-500/30'"
      >
        <div
          class="size-10 rounded-lg shrink-0 flex items-center justify-center"
          :class="toast.kind === 'denied' ? 'bg-red-500/10' : 'bg-green-500/10'"
        >
          <XCircleIcon
            v-if="toast.kind === 'denied'"
            class="size-5 text-red-400"
          />
          <CheckCircleIcon v-else class="size-5 text-green-400" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-zinc-100">{{ toast.title }}</p>
          <p class="text-xs text-zinc-400 line-clamp-2">
            {{ toast.description }}
          </p>
          <p class="text-xs text-zinc-500 mt-0.5">
            {{
              bigPicture.isActive.value
                ? "Open Requests, then My requests, to see it."
                : "Click the bell in the top bar to see your requests."
            }}
          </p>
        </div>
      </div>
    </TransitionGroup>
  </Teleport>
</template>

<script setup lang="ts">
/**
 * Toasts for game request decisions (approved, denied, added to the
 * library), pushed by `composables/request-notifications.ts`. Mounted from
 * `app.vue` so it shows on both surfaces, like `CoopRoomToastHost`.
 *
 * Display only, with no buttons, so there is nothing for the Big Picture
 * focus system to register and nothing that could hold the input lock. Top
 * centre because the achievement, cloud-save and co-op toasts own the bottom
 * of the screen.
 */
import { CheckCircleIcon, XCircleIcon } from "@heroicons/vue/24/outline";
import { useRequestNotifications } from "~/composables/request-notifications";
import { useBigPictureMode } from "~/composables/big-picture";

const { toasts } = useRequestNotifications();
const bigPicture = useBigPictureMode();
</script>
