<template>
  <!-- Stream from another machine -->
  <button
    v-if="availableSession"
    :ref="(el: any) => registerAction?.(el, { onSelect: connectToStream })"
    class="inline-flex items-center px-6 py-4 text-lg gap-3 font-semibold rounded-xl transition-all shadow-lg"
    :class="
      availableSession.status === 'Ready'
        ? 'bg-purple-600 hover:bg-purple-500 text-white shadow-purple-600/20 hover:shadow-purple-500/30 hover:scale-105'
        : 'bg-zinc-800/80 text-zinc-400 cursor-wait'
    "
    :disabled="availableSession.status !== 'Ready'"
    @click="connectToStream"
  >
    <SignalIcon class="size-6" />
    {{
      availableSession.status === "Ready"
        ? `Stream from ${availableSession.hostClient.name}`
        : "Stream starting..."
    }}
  </button>

  <!-- Host stream -->
  <button
    v-else-if="canHost"
    :ref="(el: any) => registerAction?.(el, { onSelect: startHosting })"
    class="inline-flex items-center px-6 py-4 text-lg gap-3 bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
    @click="startHosting"
  >
    <SignalIcon class="size-6 text-purple-400" />
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
  registerAction?: (el: any, opts: { onSelect: () => void }) => void;
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
  pollInterval = setInterval(refreshSessions, 15_000);
});

onUnmounted(() => {
  if (pollInterval) clearInterval(pollInterval);
});

async function refreshSessions() {
  const sessions = await listRemoteSessions();
  availableSession.value =
    sessions.find(
      (s) =>
        s.game?.id === props.gameId &&
        (s.status === "Ready" || s.status === "Starting" || s.status === "Streaming"),
    ) ?? null;
}

function connectToStream() {
  if (availableSession.value) {
    emit("connect", availableSession.value);
  }
}

async function startHosting() {
  hosting.value = true;
  try {
    await registerGame(props.gameId, props.gameName, "drop-launch");
    const { sessionId } = await startStreamingSession(props.gameId);
    await markSessionReady(sessionId);
  } catch (e) {
    console.warn("[STREAMING] Failed to start hosting:", e);
    hosting.value = false;
  }
}
</script>
