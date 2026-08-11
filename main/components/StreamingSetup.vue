<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div>
        <h3 class="text-base font-semibold text-zinc-100">This PC</h3>
        <p class="text-sm text-zinc-400">
          Games on this PC can be played from another device on your network.
        </p>
      </div>
      <div
        v-if="status"
        class="flex items-center gap-2 text-xs px-2.5 py-1 rounded-full"
        :class="
          status.running
            ? 'bg-green-500/20 text-green-300'
            : status.healthy
              ? 'bg-zinc-700/50 text-zinc-400'
              : 'bg-zinc-800 text-zinc-500'
        "
      >
        <span
          class="size-1.5 rounded-full"
          :class="
            status.running
              ? 'bg-green-400'
              : status.healthy
                ? 'bg-zinc-500'
                : 'bg-zinc-600'
          "
        />
        {{ statusLabel }}
      </div>
    </div>

    <!-- Not installed, or installed but damaged. A flattened extract leaves
         sunshine.exe in place with none of its support files, which used to
         show up as a fully working install with no way to fix it. -->
    <div
      v-if="status && !status.healthy"
      class="rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-4"
    >
      <p class="text-sm text-zinc-300 mb-3">
        {{
          status.installed
            ? "Some remote play files are missing or damaged. Set it up again to replace them."
            : "Drop downloads what it needs the first time you set this up. It takes about a minute."
        }}
      </p>
      <button
        class="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-40"
        :disabled="loading"
        @click="doInstall"
      >
        <ArrowDownTrayIcon class="size-4" />
        {{ installButtonLabel }}
      </button>
    </div>

    <!-- Installed — controls -->
    <div
      v-else-if="status && status.healthy"
      class="rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-4 space-y-4"
    >
      <div class="flex items-center gap-3">
        <button
          v-if="!status.running"
          class="inline-flex items-center gap-2 rounded-md bg-green-600 px-4 py-2 text-sm font-semibold text-white hover:bg-green-500 disabled:opacity-40"
          :disabled="loading"
          @click="doStart"
        >
          <PlayIcon class="size-4" />
          {{ loading ? "Turning on..." : "Turn on remote play" }}
        </button>
        <button
          v-else
          class="inline-flex items-center gap-2 rounded-md bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-500 disabled:opacity-40"
          :disabled="loading"
          @click="doStop"
        >
          <StopIcon class="size-4" />
          {{ loading ? "Turning off..." : "Turn off remote play" }}
        </button>
      </div>

      <!-- Pairing. The PIN is shown by the OTHER device when it asks to
           connect; this field is where it gets typed in. -->
      <div v-if="status.running" class="space-y-3">
        <p class="text-xs text-zinc-400">
          The first time another device connects it shows a four-digit code.
          Type that code here to let it in.
        </p>
        <div class="flex items-center gap-2">
          <input
            v-model="pairingPin"
            type="text"
            maxlength="4"
            placeholder="Code"
            class="w-20 rounded-md border border-zinc-600 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-100 placeholder-zinc-500 focus:border-blue-500 focus:outline-none"
          />
          <button
            class="inline-flex items-center gap-1.5 rounded-md bg-zinc-700 px-3 py-1.5 text-sm text-zinc-200 hover:bg-zinc-600 disabled:opacity-40"
            :disabled="pairingPin.length < 4"
            @click="doPair"
          >
            Pair device
          </button>
        </div>
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
      Checking remote play...
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
  repairSunshine,
  startSunshine,
  stopSunshine,
  sendPin,
} = useStreaming();

const emit = defineEmits<{ (e: "changed"): void }>();

const pairingPin = ref("");

const statusLabel = computed(() => {
  if (!status.value) return "";
  if (status.value.running) return "Ready";
  if (!status.value.installed) return "Not set up";
  return status.value.healthy ? "Off" : "Needs repair";
});

const installButtonLabel = computed(() => {
  const damaged = status.value?.installed === true;
  if (loading.value) return damaged ? "Repairing..." : "Setting up...";
  return damaged ? "Repair remote play" : "Set up remote play";
});

onMounted(() => {
  checkSunshine();
});

async function doInstall() {
  try {
    // A damaged install needs the wipe-and-replace path; a missing one doesn't.
    if (status.value?.installed) {
      await repairSunshine();
    } else {
      await installSunshine();
    }
    emit("changed");
  } catch {
    // error is already set in composable
  }
}

async function doStart() {
  try {
    await startSunshine();
    emit("changed");
  } catch {
    // error is already set
  }
}

async function doStop() {
  try {
    await stopSunshine();
    emit("changed");
  } catch {
    // error is already set
  }
}

async function doPair() {
  try {
    await sendPin(pairingPin.value, "Drop Client");
    pairingPin.value = "";
    // Refresh status to update paired client count
    await checkSunshine();
    emit("changed");
  } catch {
    // error is already set
  }
}
</script>
