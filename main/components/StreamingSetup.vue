<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div>
        <h3 class="text-base font-semibold text-zinc-100">Remote Play</h3>
        <p class="text-sm text-zinc-400">
          Stream games to other devices using Sunshine + Moonlight.
        </p>
      </div>
      <div
        v-if="status"
        class="flex items-center gap-2 text-xs px-2.5 py-1 rounded-full"
        :class="
          status.running
            ? 'bg-green-500/20 text-green-300'
            : status.installed
              ? 'bg-zinc-700/50 text-zinc-400'
              : 'bg-zinc-800 text-zinc-500'
        "
      >
        <span
          class="size-1.5 rounded-full"
          :class="
            status.running
              ? 'bg-green-400'
              : status.installed
                ? 'bg-zinc-500'
                : 'bg-zinc-600'
          "
        />
        {{
          status.running
            ? "Running"
            : status.installed
              ? "Stopped"
              : "Not Installed"
        }}
      </div>
    </div>

    <!-- Not installed -->
    <div
      v-if="status && !status.installed"
      class="rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-4"
    >
      <p class="text-sm text-zinc-300 mb-3">
        Sunshine is an open-source game streaming server that lets you play
        your games on any device running Moonlight.
      </p>
      <button
        class="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-40"
        :disabled="loading"
        @click="doInstall"
      >
        <ArrowDownTrayIcon class="size-4" />
        {{ loading ? "Installing..." : "Install Sunshine" }}
      </button>
    </div>

    <!-- Installed — controls -->
    <div
      v-else-if="status && status.installed"
      class="rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-4 space-y-4"
    >
      <!-- Admin credentials for starting -->
      <div v-if="!status.running" class="flex items-center gap-2">
        <input
          v-model="adminUsername"
          type="text"
          placeholder="Admin username"
          class="flex-1 rounded-md border border-zinc-600 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-100 placeholder-zinc-500 focus:border-blue-500 focus:outline-none font-mono"
        />
        <input
          v-model="adminPassword"
          type="password"
          placeholder="Admin password"
          class="flex-1 rounded-md border border-zinc-600 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-100 placeholder-zinc-500 focus:border-blue-500 focus:outline-none font-mono"
        />
      </div>

      <div class="flex items-center gap-3">
        <button
          v-if="!status.running"
          class="inline-flex items-center gap-2 rounded-md bg-green-600 px-4 py-2 text-sm font-semibold text-white hover:bg-green-500 disabled:opacity-40"
          :disabled="loading"
          @click="doStart"
        >
          <PlayIcon class="size-4" />
          {{ loading ? "Starting..." : "Start Sunshine" }}
        </button>
        <button
          v-else
          class="inline-flex items-center gap-2 rounded-md bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-500 disabled:opacity-40"
          :disabled="loading"
          @click="doStop"
        >
          <StopIcon class="size-4" />
          {{ loading ? "Stopping..." : "Stop Sunshine" }}
        </button>

        <span v-if="status.version" class="text-xs text-zinc-500">
          v{{ status.version }}
        </span>
      </div>

      <!-- Pairing -->
      <div v-if="status.running" class="space-y-3">
        <div class="text-sm text-zinc-300">
          <span class="font-medium">Paired clients:</span>
          {{ status.paired_clients }}
        </div>

        <div class="flex items-center gap-2">
          <input
            v-model="pairingPin"
            type="text"
            maxlength="4"
            placeholder="PIN"
            class="w-20 rounded-md border border-zinc-600 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-100 placeholder-zinc-500 focus:border-blue-500 focus:outline-none"
          />
          <button
            class="inline-flex items-center gap-1.5 rounded-md bg-zinc-700 px-3 py-1.5 text-sm text-zinc-200 hover:bg-zinc-600 disabled:opacity-40"
            :disabled="pairingPin.length < 4"
            @click="doPair"
          >
            Pair Device
          </button>
        </div>
        <p class="text-xs text-zinc-500">
          Open Moonlight on your remote device, connect to this PC, and enter
          the PIN shown here.
        </p>
      </div>
    </div>

    <!-- Error -->
    <div
      v-if="error"
      class="rounded-lg border border-red-800/50 bg-red-900/20 p-3 text-sm text-red-300"
    >
      {{ error }}
    </div>

    <!-- Loading state -->
    <div
      v-if="!status && !error"
      class="flex items-center gap-2 text-sm text-zinc-500"
    >
      <div
        class="size-4 border-2 border-zinc-600 border-t-zinc-300 rounded-full animate-spin"
      />
      Checking Sunshine status...
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  ArrowDownTrayIcon,
  PlayIcon,
  StopIcon,
} from "@heroicons/vue/20/solid";
import { useStreaming } from "~/composables/useStreaming";

const {
  sunshineStatus: status,
  loading,
  error,
  checkSunshine,
  installSunshine,
  startSunshine,
  stopSunshine,
  sendPin,
} = useStreaming();

const pairingPin = ref("");
const adminUsername = ref("sunshine");
const adminPassword = ref("");

onMounted(() => {
  checkSunshine();
});

async function doInstall() {
  try {
    await installSunshine();
  } catch {
    // error is already set in composable
  }
}

async function doStart() {
  try {
    await startSunshine(adminUsername.value, adminPassword.value);
  } catch {
    // error is already set
  }
}

async function doStop() {
  try {
    await stopSunshine();
  } catch {
    // error is already set
  }
}

async function doPair() {
  try {
    await sendPin(pairingPin.value);
    pairingPin.value = "";
    // Refresh status to update paired client count
    await checkSunshine();
  } catch {
    // error is already set
  }
}
</script>
