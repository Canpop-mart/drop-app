<template>
  <div class="flex min-h-full flex-col bg-zinc-950">
    <div class="flex w-full flex-1 flex-col">
      <iframe
        ref="frame"
        :key="requestsUrl"
        :src="requestsUrl"
        title="Game Requests"
        class="h-full min-h-full w-full border-0 flex-1"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * The server's request board in an iframe. `?view=mine` (what the header
 * bell opens) is passed through so the board opens on the user's own
 * requests, and opening that view marks the request decisions read.
 *
 * Switching to My requests inside the iframe never reaches this route. For
 * that, the board marks the decisions read itself and posts a message to
 * this window, which `composables/request-notifications.ts` answers with a
 * fresh poll; the frame is registered so only its message is trusted. A
 * server without that support leaves them unread until the bell is used.
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  markRequestDecisionsRead,
  registerRequestBoardFrame,
} from "~/composables/request-notifications";

const route = useRoute();
const frame = ref<HTMLIFrameElement | null>(null);

// Read lazily: the iframe is re-created whenever its URL changes.
const unregisterFrame = registerRequestBoardFrame(
  () => frame.value?.contentWindow ?? null,
);
onUnmounted(unregisterFrame);

const showMine = computed(() => route.query.view === "mine");

const requestsUrl = computed(() =>
  convertFileSrc("dummyvalue", "server").replace(
    "dummyvalue",
    showMine.value ? "requests?view=mine" : "requests",
  ),
);

watch(
  showMine,
  (mine) => {
    if (mine) void markRequestDecisionsRead();
  },
  { immediate: true },
);

useHead({
  title: "Requests",
});
</script>
