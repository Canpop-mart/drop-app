<template>
  <div
    class="flex items-center justify-between px-4 h-10 bg-zinc-950/80 backdrop-blur-sm border-b border-zinc-800/30 shrink-0"
  >
    <!-- Left: page title -->
    <div class="flex items-center gap-2">
      <h2 class="text-base font-semibold text-zinc-200 font-display">
        {{ pageTitle }}
      </h2>
    </div>

    <!-- Right: compact status row -->
    <div class="flex items-center gap-3">
      <!-- Controller indicator (dot only) -->
      <div
        v-if="gamepad.connected.value"
        class="size-2 rounded-full bg-green-500"
        :title="gamepad.controllerName.value || 'Controller'"
      />

      <!-- Clock -->
      <span class="text-xs text-zinc-400 tabular-nums font-medium">
        {{ clock }}
      </span>

      <!-- Battery placeholder (Steam Deck shows battery) -->
      <div class="flex items-center gap-1 text-xs text-zinc-500">
        <BoltIcon class="size-3" />
      </div>

      <!-- User avatar (small) -->
      <div v-if="state?.user">
        <img
          v-if="state.user.profilePictureObjectId"
          :src="useObject(state.user.profilePictureObjectId)"
          class="size-6 rounded-full ring-1 ring-zinc-700"
        />
        <div
          v-else
          class="size-6 rounded-full bg-zinc-800 ring-1 ring-zinc-700 flex items-center justify-center"
        >
          <span class="text-[10px] font-medium text-zinc-400">
            {{ state.user.displayName?.[0]?.toUpperCase() }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { BoltIcon } from "@heroicons/vue/24/solid";
import { useGamepad } from "~/composables/gamepad";
import { useAppState } from "~/composables/app-state";
import { useObject } from "~/composables/use-object";

const route = useRoute();
const gamepad = useGamepad();
const state = useAppState();

const clock = ref("");
function updateClock() {
  const now = new Date();
  clock.value = now.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}
updateClock();
const clockInterval = setInterval(updateClock, 30_000);
onUnmounted(() => clearInterval(clockInterval));

const pageTitle = computed(() => {
  const path = route.path;
  if (path.startsWith("/bigpicture/library")) return "Library";
  if (path.startsWith("/bigpicture/store")) return "Store";
  if (path.startsWith("/bigpicture/community")) return "Community";
  if (path.startsWith("/bigpicture/news")) return "News";
  if (path.startsWith("/bigpicture/downloads")) return "Downloads";
  if (path.startsWith("/bigpicture/settings")) return "Settings";
  return "Drop";
});
</script>
