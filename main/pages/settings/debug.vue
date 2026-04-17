<template>
  <div class="divide-y divide-zinc-700">
    <div class="py-6">
      <h2 class="text-base font-semibold font-display leading-7 text-zinc-100">
        Debug Information
      </h2>
      <p class="mt-1 text-sm leading-6 text-zinc-400">
        Technical information about your Drop client installation, helpful for
        debugging.
      </p>

      <div class="mt-10 space-y-8">
        <div>
          <div class="flex items-center gap-x-3">
            <FingerPrintIcon class="h-5 w-5 text-zinc-400" />
            <h3 class="text-sm font-medium leading-6 text-zinc-100">
              Client ID
            </h3>
          </div>
          <p class="mt-2 text-sm text-zinc-400 font-mono ml-8">
            {{ clientId || "Not signed in" }}
          </p>
        </div>

        <div>
          <div class="flex items-center gap-x-3">
            <ComputerDesktopIcon class="h-5 w-5 text-zinc-400" />
            <h3 class="text-sm font-medium leading-6 text-zinc-100">
              Platform
            </h3>
          </div>
          <p class="mt-2 text-sm text-zinc-400 font-mono ml-8">
            {{ platformInfo }}
          </p>
        </div>

        <div>
          <div class="flex items-center gap-x-3">
            <ServerIcon class="h-5 w-5 text-zinc-400" />
            <h3 class="text-sm font-medium leading-6 text-zinc-100">
              Server URL
            </h3>
          </div>
          <p class="mt-2 text-sm text-zinc-400 font-mono ml-8">
            {{ baseUrl || "Not connected" }}
          </p>
        </div>

        <div>
          <div class="flex items-center gap-x-3">
            <FolderIcon class="h-5 w-5 text-zinc-400" />
            <h3 class="text-sm font-medium leading-6 text-zinc-100">
              Data Directory
            </h3>
          </div>
          <p class="mt-2 text-sm text-zinc-400 font-mono ml-8">
            {{ dataDir || "Unknown" }}
          </p>
        </div>

        <div class="pt-6 flex gap-x-4">
          <button
            @click="() => openDataDir()"
            type="button"
            class="inline-flex items-center gap-x-2 rounded-md bg-blue-600 px-3.5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600"
          >
            <FolderIcon class="h-5 w-5" aria-hidden="true" />
            Open Data Directory
          </button>
          <button
            @click="() => openLogFile()"
            type="button"
            class="inline-flex items-center gap-x-2 rounded-md bg-blue-600 px-3.5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600"
          >
            <DocumentTextIcon class="h-5 w-5" aria-hidden="true" />
            Open Log File
          </button>
        </div>
      </div>
    </div>

    <!-- Test Harnesses -->
    <div class="py-6">
      <h2 class="text-base font-semibold font-display leading-7 text-zinc-100">
        Test Harnesses
      </h2>
      <p class="mt-1 text-sm leading-6 text-zinc-400">
        Trigger specific features for testing without needing a real
        multi-machine setup.
      </p>

      <div class="mt-6 space-y-6">
        <!-- Save Conflict Test -->
        <div
          class="rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-4 space-y-3"
        >
          <div class="flex items-center gap-2">
            <ExclamationTriangleIcon class="size-5 text-amber-400" />
            <h3 class="text-sm font-semibold text-zinc-100">
              Save Conflict Dialog
            </h3>
          </div>
          <p class="text-xs text-zinc-400">
            Emits a fake save_sync_conflict event for a game. Navigate to the
            game page first, then trigger — the conflict dialog should appear.
          </p>
          <div class="flex items-center gap-3">
            <input
              v-model="conflictGameId"
              type="text"
              placeholder="Game ID"
              class="flex-1 rounded-md border border-zinc-600 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-100 placeholder-zinc-500 focus:border-blue-500 focus:outline-none font-mono"
            />
            <button
              class="inline-flex items-center gap-1.5 rounded-md bg-amber-600 px-3 py-1.5 text-sm font-semibold text-white hover:bg-amber-500"
              @click="emitFakeConflict"
            >
              Emit Conflict
            </button>
          </div>
          <div
            v-if="conflictEmitted"
            class="text-xs text-green-400"
          >
            Emitted save_sync_conflict/{{ conflictGameId }} — switch to the
            game page to see the dialog.
          </div>
        </div>

        <!-- Streaming Test -->
        <div
          class="rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-4 space-y-3"
        >
          <div class="flex items-center gap-2">
            <SignalIcon class="size-5 text-purple-400" />
            <h3 class="text-sm font-semibold text-zinc-100">
              Sunshine Streaming
            </h3>
          </div>
          <p class="text-xs text-zinc-400">
            Check Sunshine status, test install, and verify the streaming
            lifecycle without leaving the debug page.
          </p>

          <!-- Admin credentials for starting Sunshine -->
          <div
            v-if="sunshineStatus?.installed && !sunshineStatus?.running"
            class="flex items-center gap-2"
          >
            <input
              v-model="sunshineAdminUser"
              type="text"
              placeholder="Admin username"
              class="flex-1 rounded-md border border-zinc-600 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-100 placeholder-zinc-500 focus:border-blue-500 focus:outline-none font-mono"
            />
            <input
              v-model="sunshineAdminPass"
              type="password"
              placeholder="Admin password"
              class="flex-1 rounded-md border border-zinc-600 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-100 placeholder-zinc-500 focus:border-blue-500 focus:outline-none font-mono"
            />
          </div>

          <div class="flex items-center gap-3 flex-wrap">
            <button
              class="inline-flex items-center gap-1.5 rounded-md bg-zinc-700 px-3 py-1.5 text-sm text-zinc-200 hover:bg-zinc-600"
              :disabled="streamingLoading"
              @click="doCheckSunshine"
            >
              Check Status
            </button>
            <button
              v-if="sunshineStatus && !sunshineStatus.installed"
              class="inline-flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-500"
              :disabled="streamingLoading"
              @click="doInstallSunshine"
            >
              Install
            </button>
            <button
              v-if="sunshineStatus?.installed && !sunshineStatus?.running"
              class="inline-flex items-center gap-1.5 rounded-md bg-green-600 px-3 py-1.5 text-sm text-white hover:bg-green-500"
              :disabled="streamingLoading"
              @click="doStartSunshine"
            >
              Start
            </button>
            <button
              v-if="sunshineStatus?.running"
              class="inline-flex items-center gap-1.5 rounded-md bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-500"
              :disabled="streamingLoading"
              @click="doStopSunshine"
            >
              Stop
            </button>
          </div>

          <!-- Status display -->
          <div
            v-if="sunshineStatus"
            class="rounded-md bg-zinc-900 p-3 text-xs font-mono text-zinc-300 space-y-1"
          >
            <div>
              installed:
              <span :class="sunshineStatus.installed ? 'text-green-400' : 'text-red-400'">
                {{ sunshineStatus.installed }}
              </span>
            </div>
            <div>
              running:
              <span :class="sunshineStatus.running ? 'text-green-400' : 'text-zinc-500'">
                {{ sunshineStatus.running }}
              </span>
            </div>
            <div v-if="sunshineStatus.version">
              version: {{ sunshineStatus.version }}
            </div>
            <div v-if="sunshineStatus.web_ui_url">
              web_ui: {{ sunshineStatus.web_ui_url }}
            </div>
            <div>paired_clients: {{ sunshineStatus.paired_clients }}</div>
          </div>

          <!-- Streaming sessions from server -->
          <div class="pt-2 space-y-2">
            <div class="flex items-center gap-2 flex-wrap">
              <button
                class="inline-flex items-center gap-1.5 rounded-md bg-zinc-700 px-3 py-1.5 text-sm text-zinc-200 hover:bg-zinc-600"
                @click="doFetchSessions"
              >
                Fetch Server Sessions
              </button>
              <button
                class="inline-flex items-center gap-1.5 rounded-md bg-purple-600 px-3 py-1.5 text-sm text-white hover:bg-purple-500"
                @click="doCreateTestSession"
              >
                Create Test Session
              </button>
              <button
                v-if="streamingSessions.length > 0"
                class="inline-flex items-center gap-1.5 rounded-md bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-500"
                @click="doStopAllSessions"
              >
                Stop All
              </button>
            </div>
            <div
              v-if="streamingSessions.length > 0"
              class="mt-2 rounded-md bg-zinc-900 p-3 text-xs font-mono text-zinc-300 max-h-40 overflow-y-auto"
            >
              <div
                v-for="s in streamingSessions"
                :key="s.id"
                class="mb-2 pb-2 border-b border-zinc-800 last:border-0"
              >
                <div>id: {{ s.id }}</div>
                <div>status: <span :class="s.status === 'Ready' ? 'text-green-400' : s.status === 'Streaming' ? 'text-purple-400' : 'text-yellow-400'">{{ s.status }}</span></div>
                <div>host: {{ s.hostClient.name }} ({{ s.hostClient.platform }})</div>
                <div>game: {{ s.game?.mName ?? 'Desktop' }}</div>
                <div>ip: {{ s.hostLocalIp ?? 'unknown' }}</div>
              </div>
            </div>
            <div
              v-else-if="sessionsFetched"
              class="mt-2 text-xs text-zinc-500"
            >
              No active streaming sessions.
            </div>
          </div>

          <div
            v-if="streamingError"
            class="text-xs text-red-400"
          >
            {{ streamingError }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import {
  FingerPrintIcon,
  ComputerDesktopIcon,
  ServerIcon,
  FolderIcon,
  DocumentTextIcon,
  ExclamationTriangleIcon,
  SignalIcon,
} from "@heroicons/vue/24/outline";
import { open } from "@tauri-apps/plugin-shell";
import {
  useStreaming,
  type StreamingSession,
  type SunshineStatusResult,
} from "~/composables/useStreaming";

// ── System info ────────────────────────────────────────────────────────

const clientId = ref<string | null>(null);
const platformInfo = ref("Loading...");
const baseUrl = ref<string | null>(null);
const dataDir = ref<string | null>(null);

const systemData = await invoke<{
  clientId: string;
  baseUrl: string;
  dataDir: string;
}>("fetch_system_data");

clientId.value = systemData.clientId;
baseUrl.value = systemData.baseUrl;
dataDir.value = systemData.dataDir;

const currentPlatform = await platform();
platformInfo.value = currentPlatform;

async function openDataDir() {
  if (!dataDir.value) return;
  try {
    await invoke("open_fs", { path: dataDir.value });
  } catch (error) {
    console.error("Failed to open data dir:", error);
  }
}

async function openLogFile() {
  if (!dataDir.value) return;
  try {
    const logPath = `${dataDir.value}/drop.log`;
    await invoke("open_fs", { path: logPath });
  } catch (error) {
    console.error("Failed to open log file:", error);
  }
}

// ── Save Conflict Test ─────────────────────────────────────────────────

const conflictGameId = ref("");
const conflictEmitted = ref(false);

async function emitFakeConflict() {
  if (!conflictGameId.value) return;

  const fakeConflicts = {
    gameId: conflictGameId.value,
    conflicts: [
      {
        filename: "test-save-01.srm",
        saveType: "save",
        localHash: "abc123local",
        localSize: 32768,
        localModifiedAt: Math.floor(Date.now() / 1000) - 3600,
        cloudId: "cloud-save-uuid-1",
        cloudHash: "def456cloud",
        cloudSize: 32512,
        cloudModifiedAt: new Date(Date.now() - 7200_000).toISOString(),
        cloudUploadedFrom: "Gaming-PC",
      },
      {
        filename: "test-save-02.state",
        saveType: "state",
        localHash: "ghi789local",
        localSize: 1048576,
        localModifiedAt: Math.floor(Date.now() / 1000) - 600,
        cloudId: "cloud-save-uuid-2",
        cloudHash: "jkl012cloud",
        cloudSize: 1048320,
        cloudModifiedAt: new Date(Date.now() - 1800_000).toISOString(),
        cloudUploadedFrom: "Living-Room-PC",
      },
      {
        filename: "pc/AppData/Saves/slot1.sav",
        saveType: "pc",
        localHash: "mno345local",
        localSize: 524288,
        localModifiedAt: Math.floor(Date.now() / 1000) - 120,
        cloudId: "cloud-save-uuid-3",
        cloudHash: "pqr678cloud",
        cloudSize: 524000,
        cloudModifiedAt: new Date(Date.now() - 300_000).toISOString(),
        cloudUploadedFrom: "Laptop",
      },
    ],
  };

  await emit(
    `save_sync_conflict/${conflictGameId.value}`,
    fakeConflicts,
  );

  conflictEmitted.value = true;
  setTimeout(() => {
    conflictEmitted.value = false;
  }, 5000);
}

// ── Streaming Test ─────────────────────────────────────────────────────

const {
  sunshineStatus,
  loading: streamingLoading,
  error: streamingError,
  checkSunshine,
  installSunshine,
  startSunshine,
  stopSunshine,
  listRemoteSessions,
  startStreamingSession,
  markSessionReady,
  stopStreamingSession,
} = useStreaming();

const streamingSessions = ref<StreamingSession[]>([]);
const sessionsFetched = ref(false);
const sunshineAdminUser = ref("sunshine");
const sunshineAdminPass = ref("");

async function doCheckSunshine() {
  try {
    await checkSunshine();
  } catch {
    // error shown in template
  }
}

async function doInstallSunshine() {
  try {
    await installSunshine();
  } catch {
    // error shown in template
  }
}

async function doStartSunshine() {
  try {
    await startSunshine(sunshineAdminUser.value, sunshineAdminPass.value);
  } catch {
    // error shown in template
  }
}

async function doStopSunshine() {
  try {
    await stopSunshine();
  } catch {
    // error shown in template
  }
}

async function doFetchSessions() {
  streamingSessions.value = await listRemoteSessions();
  sessionsFetched.value = true;
}

async function doCreateTestSession() {
  try {
    const { sessionId } = await startStreamingSession();
    await markSessionReady(sessionId);
    await doFetchSessions();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    console.error("[DEBUG] Failed to create test session:", msg);
    streamingError.value = msg;
  }
}

async function doStopAllSessions() {
  try {
    for (const s of streamingSessions.value) {
      await stopStreamingSession(s.id);
    }
    await doFetchSessions();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    console.error("[DEBUG] Failed to stop sessions:", msg);
    streamingError.value = msg;
  }
}
</script>
