<template>
  <div
    class="flex items-center justify-between px-8 h-14 border-b shrink-0"
    :class="{ 'backdrop-blur-sm': !reducedMotion }"
    :style="{
      backgroundColor: reducedMotion
        ? 'var(--bpm-bg)'
        : 'color-mix(in srgb, var(--bpm-bg) 80%, transparent)',
      borderColor: 'var(--bpm-border)',
    }"
  >
    <!-- Left: breadcrumbs -->
    <div class="flex items-center gap-1.5">
      <template v-for="(crumb, idx) in breadcrumbs" :key="idx">
        <ChevronRightIcon v-if="idx > 0" class="size-3 text-zinc-600 flex-shrink-0" />
        <span
          class="text-lg font-semibold font-display"
          :class="idx === breadcrumbs.length - 1 ? 'text-zinc-200' : 'text-zinc-500'"
        >
          {{ crumb.label }}
        </span>
      </template>
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

      <!-- User avatar (clickable → profile) -->
      <button
        v-if="state?.user"
        :ref="(el: any) => registerContent(el, { onSelect: () => router.push(`/bigpicture/profile/${state.user.username}`) })"
        class="flex items-center gap-2 rounded-full transition-all hover:ring-2 hover:ring-blue-500/50 focus:ring-2 focus:ring-blue-500/50"
        @click="router.push(`/bigpicture/profile/${state.user.username}`)"
      >
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
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ChevronRightIcon } from "@heroicons/vue/20/solid";
import { useGamepad } from "~/composables/gamepad";
import { useAppState } from "~/composables/app-state";
import { useObject } from "~/composables/use-object";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useReducedMotion } from "~/composables/bp-reduced-motion";

const { reducedMotion } = useReducedMotion();

const route = useRoute();
const router = useRouter();
const gamepad = useGamepad();
const state = useAppState();
const registerContent = useBpFocusableGroup("content");

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

// Game name from API
const gameName = ref("");

watch(
  () => route.path,
  async (path) => {
    const match = path.match(/^\/bigpicture\/library\/([^/]+)$/);
    if (match && match[1] !== "collections") {
      try {
        const response = await fetch(serverUrl(`api/v1/client/game/${match[1]}`));
        if (response.ok) {
          const data = await response.json();
          gameName.value = data.mName || data.name || "Game";
        }
      } catch {
        gameName.value = "Game";
      }
    } else {
      gameName.value = "";
    }
  },
  { immediate: true }
);

// Breadcrumbs from route
const breadcrumbs = computed(() => {
  const path = route.path;
  const crumbs: { label: string }[] = [{ label: "Home" }];

  if (path === "/bigpicture") return crumbs;

  if (path.startsWith("/bigpicture/library")) {
    crumbs.push({ label: "Library" });
    // If on a game detail page, add the game name
    if (path !== "/bigpicture/library" && path !== "/bigpicture/library/collections") {
      crumbs.push({ label: gameName.value || "Game" });
    }
    if (path === "/bigpicture/library/collections") {
      crumbs.push({ label: "Collections" });
    }
  } else if (path.startsWith("/bigpicture/store")) {
    crumbs.push({ label: "Store" });
  } else if (path.startsWith("/bigpicture/community")) {
    crumbs.push({ label: "Community" });
  } else if (path.startsWith("/bigpicture/news")) {
    crumbs.push({ label: "News" });
  } else if (path.startsWith("/bigpicture/downloads")) {
    crumbs.push({ label: "Downloads" });
  } else if (path.startsWith("/bigpicture/profile")) {
    crumbs.push({ label: "Profile" });
    if (path.endsWith("/edit")) {
      crumbs.push({ label: "Edit Profile" });
    }
  } else if (path.startsWith("/bigpicture/settings")) {
    crumbs.push({ label: "Settings" });
  } else if (path.startsWith("/bigpicture/bugreport")) {
    crumbs.push({ label: "Bug Report" });
  }

  return crumbs;
});
</script>
