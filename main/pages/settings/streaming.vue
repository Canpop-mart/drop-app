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

    <!-- Stream resolution: the display mode this PC switches to while hosting a
         stream. Set it to match the streaming device (small for a handheld,
         1080p for a docked TV); "Don't change" leaves your desktop alone. -->
    <div class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-1">Stream Resolution</h4>
      <p class="text-sm text-zinc-400 mb-3">
        The resolution this PC switches to while streaming a game. Lower matches
        a handheld screen; higher suits a docked TV. Set the same value on the
        device you stream to.
      </p>
      <select
        v-model="streamingResolution"
        class="w-full max-w-xs rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
        @change="saveStreamingResolution"
      >
        <option value="1280x800">Handheld (1280×800)</option>
        <option value="1920x1080">1080p (1920×1080)</option>
        <option value="2560x1440">1440p (2560×1440)</option>
        <option value="native">Don't change my resolution</option>
      </select>
      <p v-if="resolutionSaved" class="mt-2 text-xs text-green-400">Saved.</p>
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
import { invoke } from "@tauri-apps/api/core";
import {
  useStreaming,
  type StreamingSession,
} from "~/composables/useStreaming";

const { listRemoteSessions } = useStreaming();

const sessions = ref<StreamingSession[]>([]);
const sessionsLoading = ref(true);

// Host display resolution while streaming (read by streaming.rs at launch).
const streamingResolution = ref("1280x800");
const resolutionSaved = ref(false);

async function saveStreamingResolution() {
  try {
    await invoke("update_settings", {
      newSettings: { streamingResolution: streamingResolution.value },
    });
    resolutionSaved.value = true;
    setTimeout(() => {
      resolutionSaved.value = false;
    }, 1500);
  } catch (e) {
    console.error("[SETTINGS] Failed to save streaming resolution:", e);
  }
}

onMounted(async () => {
  try {
    const settings = await invoke<Record<string, unknown>>("fetch_settings");
    if (typeof settings.streamingResolution === "string") {
      streamingResolution.value = settings.streamingResolution;
    }
  } catch {
    // keep default
  }
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
