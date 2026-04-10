<template>
  <div
    class="flex items-center justify-between px-8 h-14 bg-zinc-950/80 backdrop-blur-sm border-b border-zinc-800/30 shrink-0"
  >
    <!-- Left: page title -->
    <div class="flex items-center gap-3">
      <h2 class="text-lg font-semibold text-zinc-200 font-display">
        {{ pageTitle }}
      </h2>
    </div>

    <!-- Right: status indicators -->
    <div class="flex items-center gap-4">
      <!-- Controller indicator -->
      <div
        v-if="gamepad.connected.value"
        class="flex items-center gap-2 text-xs text-zinc-500"
      >
        <div class="size-2 rounded-full bg-green-500" />
        <span>{{ gamepad.controllerName.value || "Controller" }}</span>
      </div>

      <!-- Clock -->
      <span class="text-sm text-zinc-400 tabular-nums font-medium">
        {{ clock }}
      </span>

      <!-- User avatar -->
      <div v-if="state?.user" class="flex items-center gap-2">
        <img
          v-if="state.user.profilePictureObjectId"
          :src="useObject(state.user.profilePictureObjectId)"
          class="size-8 rounded-full ring-1 ring-zinc-700"
        />
        <div
          v-else
          class="size-8 rounded-full bg-zinc-800 ring-1 ring-zinc-700 flex items-center justify-center"
        >
          <span class="text-xs font-medium text-zinc-400">
            {{ state.user.displayName?.[0]?.toUpperCase() }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useGamepad } from "~/composables/gamepad";
import { useAppState } from "~/composables/app-state";
import { useObject } from "~/composables/use-object";

const route = useRoute();
const gamepad = useGamepad();
const state = useAppState();

// Clock
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

// Page title from route
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
