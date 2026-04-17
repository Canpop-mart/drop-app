<template>
  <div>
    <div class="border-b border-zinc-700 py-5">
      <h3
        class="text-base font-semibold font-display leading-6 text-zinc-100"
      >
        Streaming
      </h3>
      <p class="mt-1 text-sm text-zinc-400">
        Stream your games to other devices using Sunshine and Moonlight.
      </p>
    </div>

    <div class="mt-5">
      <StreamingSetup />
    </div>

    <!-- Active sessions -->
    <div class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-3">
        Active Streaming Sessions
      </h4>
      <div v-if="sessionsLoading" class="text-sm text-zinc-500">
        Loading sessions...
      </div>
      <div
        v-else-if="sessions.length === 0"
        class="text-sm text-zinc-500"
      >
        No active streaming sessions.
      </div>
      <div v-else class="space-y-2">
        <div
          v-for="session in sessions"
          :key="session.id"
          class="flex items-center justify-between rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-3"
        >
          <div class="flex items-center gap-3">
            <span
              class="size-2 rounded-full"
              :class="
                session.status === 'Ready'
                  ? 'bg-green-400'
                  : session.status === 'Streaming'
                    ? 'bg-purple-400 animate-pulse'
                    : 'bg-yellow-400'
              "
            />
            <div>
              <div class="text-sm text-zinc-200">
                {{ session.game?.mName ?? "Desktop" }}
              </div>
              <div class="text-xs text-zinc-500">
                {{ session.hostClient.name }} &middot; {{ session.status }}
              </div>
            </div>
          </div>
          <div class="text-xs text-zinc-500">
            {{ formatSessionTime(session.createdAt) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  useStreaming,
  type StreamingSession,
} from "~/composables/useStreaming";

const { listRemoteSessions } = useStreaming();

const sessions = ref<StreamingSession[]>([]);
const sessionsLoading = ref(true);

onMounted(async () => {
  try {
    sessions.value = await listRemoteSessions();
  } finally {
    sessionsLoading.value = false;
  }
});

function formatSessionTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}
</script>
