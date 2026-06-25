<template>
  <!-- Stream from another machine — shown when a remote session is available for this game -->
  <button
    v-if="availableSession"
    class="inline-flex items-center gap-x-2 rounded-md px-4 py-3 text-sm font-semibold shadow-sm uppercase font-display transition-colors"
    :class="
      availableSession.status === 'Ready'
        ? 'bg-purple-600 text-white hover:bg-purple-500'
        : 'bg-zinc-700 text-zinc-300 cursor-wait'
    "
    :disabled="availableSession.status !== 'Ready'"
    @click="connectToStream"
  >
    <SignalIcon class="size-5" />
    {{
      availableSession.status === "Ready"
        ? `Stream from ${availableSession.hostClient.name}`
        : "Stream starting..."
    }}
  </button>

  <!-- Host stream — shown when the game is installed locally and Sunshine is running -->
  <button
    v-else-if="canHost"
    class="inline-flex items-center gap-x-2 rounded-md bg-zinc-800/50 px-4 py-3 text-sm font-semibold text-zinc-300 shadow-sm uppercase font-display hover:bg-zinc-700 transition-colors"
    @click="startHosting"
  >
    <SignalIcon class="size-5 text-purple-400" />
    {{ hosting ? "Streaming..." : "Stream" }}
  </button>
</template>

<script setup lang="ts">
import { SignalIcon } from "@heroicons/vue/20/solid";
import {
  useStreaming,
  type StreamingSession,
} from "~/composables/useStreaming";

const props = defineProps<{
  gameId: string;
  gameName: string;
  isInstalled: boolean;
}>();

const emit = defineEmits<{
  connect: [session: StreamingSession];
}>();

const {
  sunshineStatus,
  checkSunshine,
  listRemoteSessions,
  startStreamingSession,
  markSessionReady,
  registerGame,
} = useStreaming();

const availableSession = ref<StreamingSession | null>(null);
const hosting = ref(false);
const canHost = computed(
  () =>
    props.isInstalled &&
    sunshineStatus.value?.installed &&
    sunshineStatus.value?.running &&
    !hosting.value,
);

let pollInterval: ReturnType<typeof setInterval> | null = null;

onMounted(async () => {
  await checkSunshine();
  await refreshSessions();
  // Poll for remote sessions every 15 seconds
  pollInterval = setInterval(refreshSessions, 15_000);
});

onUnmounted(() => {
  if (pollInterval) clearInterval(pollInterval);
});

async function refreshSessions() {
  const sessions = await listRemoteSessions();
  // Find an active session for this game from another machine
  availableSession.value =
    sessions.find(
      (s) =>
        s.game?.id === props.gameId &&
        (s.status === "Ready" || s.status === "Starting" || s.status === "Streaming"),
    ) ?? null;
}

async function connectToStream() {
  if (availableSession.value) {
    emit("connect", availableSession.value);
  }
}

async function startHosting() {
  hosting.value = true;
  try {
    // Register the game with Sunshine so it appears in its app list
    await registerGame(props.gameId, props.gameName, "drop-launch");

    // Tell the server we're hosting a stream
    const { sessionId } = await startStreamingSession(props.gameId);

    // Mark it as ready (Sunshine should already be running)
    await markSessionReady(sessionId);
  } catch (e) {
    console.warn("[STREAMING] Failed to start hosting:", e);
    hosting.value = false;
  }
}
</script>
