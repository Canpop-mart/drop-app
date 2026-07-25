<template>
  <div>
    <div
      v-if="error"
      class="mb-4 px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
    >
      {{ error }}
    </div>
    <div
      v-if="notice"
      class="mb-4 px-4 py-3 rounded-lg bg-green-900/25 border border-green-500/30 text-green-200 text-sm"
    >
      {{ notice }}
    </div>

    <!-- Session closed by the host -->
    <div
      v-if="sessionEnded"
      class="rounded-xl bg-zinc-900/60 border border-zinc-700 p-6 text-center"
    >
      <p class="text-lg font-medium text-zinc-200 mb-1">Session closed</p>
      <p class="text-sm text-zinc-500 mb-4">
        The host closed this multiworld. You can start or join another anytime.
      </p>
      <button
        v-if="!compact"
        class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600"
        @click="dismissSessionEnded"
      >
        OK
      </button>
    </div>

    <!-- In a session -->
    <div v-else-if="detail" class="space-y-5">
      <div class="rounded-xl bg-zinc-900/60 p-6">
        <p class="text-xs uppercase tracking-wide text-zinc-500 mb-2">
          {{ detail.isHost ? "Session code (share with players)" : "Session code" }}
        </p>
        <button
          v-if="!compact"
          class="group inline-flex items-center gap-3"
          title="Click to copy"
          @click="copyCode"
        >
          <span class="text-3xl font-mono font-bold tracking-widest text-purple-300">
            {{ displayCode || "…" }}
          </span>
          <span
            class="text-xs font-medium"
            :class="codeCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
          >
            {{ codeCopied ? "✓ Copied!" : "Copy" }}
          </span>
        </button>
        <span
          v-else
          class="text-3xl font-mono font-bold tracking-widest text-purple-300"
        >
          {{ displayCode || "…" }}
        </span>
        <p v-if="detail.name" class="text-sm text-zinc-400 mt-2">
          {{ detail.name }}
        </p>
      </div>

      <!-- Connect string: the thing every player needs once the seed is live -->
      <div
        v-if="detail.connectAddress"
        class="rounded-xl bg-zinc-900/60 p-5 border border-purple-500/20"
      >
        <p class="text-xs uppercase tracking-wide text-zinc-500 mb-2">
          Connect address
        </p>
        <button
          v-if="!compact"
          class="group inline-flex items-center gap-3"
          title="Click to copy"
          @click="copyConnect"
        >
          <span class="text-xl font-mono font-bold text-purple-300">
            {{ detail.connectAddress }}
          </span>
          <span
            class="text-xs font-medium"
            :class="connectCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
          >
            {{ connectCopied ? "✓ Copied!" : "Copy" }}
          </span>
        </button>
        <span v-else class="text-xl font-mono font-bold text-purple-300">
          {{ detail.connectAddress }}
        </span>
        <p class="text-xs text-zinc-600 mt-2">
          Enter this in your game's Archipelago client to connect.
        </p>
      </div>

      <!-- Players + who has submitted settings -->
      <div>
        <div class="flex items-center justify-between mb-2">
          <p class="text-sm font-medium text-zinc-400">Players</p>
          <p class="text-xs text-zinc-500">
            {{ detail.readyCount }} of {{ detail.totalCount }} ready
          </p>
        </div>
        <div class="space-y-2">
          <div
            v-for="s in detail.slots"
            :key="s.clientId"
            class="rounded-lg bg-zinc-900/40 px-4 py-3"
          >
            <div class="flex items-center gap-2">
              <span class="text-zinc-200">{{ s.clientName }}</span>
              <span
                v-if="s.isHost"
                class="text-xs px-2 py-0.5 rounded bg-purple-600/20 text-purple-300"
              >
                Host
              </span>
              <span
                class="ml-auto text-xs font-medium"
                :class="s.validationError ? 'text-red-300' : s.hasYaml ? 'text-green-400' : 'text-zinc-500'"
              >
                {{ s.validationError ? "✗ Problem" : s.hasYaml ? "✓ Ready" : "No settings yet" }}
              </span>
            </div>
            <p v-if="s.slotName" class="text-xs text-zinc-500 mt-1">
              {{ s.slotName }}<span v-if="s.game"> · {{ s.game }}</span>
            </p>
            <p v-if="s.validationError" class="text-xs text-red-300/80 mt-1">
              {{ s.validationError }}
            </p>
          </div>
        </div>
      </div>

      <template v-if="!compact">
        <!-- Everyone uploads their own settings file -->
        <div class="rounded-xl bg-zinc-900/60 p-5">
          <h3 class="text-sm font-medium text-zinc-200 mb-1">Your settings</h3>
          <p class="text-sm text-zinc-500 mb-3">
            Export a YAML from the Archipelago Launcher's Options Creator, then
            upload it here. Drop checks it and tells you right away if something
            is wrong.
          </p>
          <button
            :disabled="busy"
            class="px-5 py-2.5 rounded-lg text-sm font-medium bg-purple-600 text-white hover:bg-purple-500 disabled:opacity-50"
            @click="uploadYaml"
          >
            {{ mySlot?.hasYaml ? "Replace my settings" : "Upload my settings" }}
          </button>
        </div>

        <!-- Host-only: gather everything, then publish where to connect -->
        <div v-if="detail.isHost" class="rounded-xl bg-zinc-900/60 p-5 space-y-4">
          <div>
            <h3 class="text-sm font-medium text-zinc-200 mb-1">
              Generate the multiworld
            </h3>
            <p class="text-sm text-zinc-500 mb-3">
              Save everyone's settings as one file, then upload it on the
              Archipelago "Generate" page.
            </p>
            <button
              :disabled="busy || detail.readyCount === 0"
              class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
              @click="saveBundle"
            >
              Save all settings
            </button>
            <p v-if="detail.readyCount === 0" class="text-xs text-zinc-600 mt-2">
              Nobody has uploaded settings yet.
            </p>
            <p
              v-else-if="!detail.allReady"
              class="text-xs text-amber-300/70 mt-2"
            >
              {{ detail.totalCount - detail.readyCount }} player(s) still
              haven't uploaded. You can generate without them, but they won't be
              in the seed.
            </p>
          </div>

          <div class="border-t border-zinc-800 pt-4">
            <h3 class="text-sm font-medium text-zinc-200 mb-1">
              Connect address
            </h3>
            <p class="text-sm text-zinc-500 mb-3">
              After hosting the room, paste its connect line here so everyone
              gets it.
            </p>
            <div class="flex items-center gap-3">
              <input
                v-model="connectInput"
                placeholder="10.243.0.1:38281"
                class="flex-1 px-4 py-2.5 rounded-lg bg-zinc-800 text-zinc-100 font-mono placeholder:text-zinc-600 focus:outline-none focus:ring-2 focus:ring-purple-500"
                @keyup.enter="submitConnect"
              />
              <button
                :disabled="busy || connectInput.trim().length === 0"
                class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
                @click="submitConnect"
              >
                Save
              </button>
            </div>
          </div>
        </div>

        <!--
          Shown until dismissed: Archipelago has to advertise the overlay
          address. Drop can't tell whether it's been set (that's a config file
          in a different container), so the host confirms it by hand. Dismissal
          is keyed by the address, so it stays hidden unless the address ever
          changes — which would mean the setup genuinely has to be redone.
        -->
        <div
          v-if="showSetupBanner"
          class="rounded-xl bg-amber-900/15 border border-amber-500/25 p-5"
        >
          <h3 class="text-sm font-medium text-amber-200 mb-1">
            One-time Archipelago setup
          </h3>
          <p class="text-sm text-amber-100/70 mb-2">
            So players off your network can reach it, set this in Archipelago's
            <span class="font-mono">config.yaml</span> and restart it. You only
            ever do this once.
          </p>
          <code
            class="block px-3 py-2 rounded bg-black/40 text-amber-200 font-mono text-sm"
          >
            HOST_ADDRESS: {{ detail.serverAddress }}
          </code>
          <button
            class="mt-3 px-4 py-1.5 rounded-md text-sm font-medium bg-amber-600/30 text-amber-100 hover:bg-amber-600/50"
            @click="dismissSetup"
          >
            I've done this — hide
          </button>
        </div>

        <div v-if="!confirmingLeave">
          <button
            :disabled="busy"
            class="px-5 py-2.5 rounded-lg text-sm font-medium bg-red-900/40 text-red-200 hover:bg-red-900/60 disabled:opacity-50"
            @click="confirmingLeave = true"
          >
            {{ detail.isHost ? "Close session" : "Leave session" }}
          </button>
        </div>
        <div
          v-else
          class="flex items-center gap-3 rounded-lg bg-zinc-900/60 px-4 py-3"
        >
          <span class="text-sm text-zinc-300">
            {{ detail.isHost ? "Close this session for everyone?" : "Leave this session?" }}
          </span>
          <div class="flex gap-2 ml-auto">
            <button
              :disabled="busy"
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-red-700 text-white hover:bg-red-600 disabled:opacity-50"
              @click="doLeave"
            >
              {{ detail.isHost ? "Close it" : "Leave" }}
            </button>
            <button
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-200 hover:bg-zinc-600"
              @click="confirmingLeave = false"
            >
              Cancel
            </button>
          </div>
        </div>
      </template>

      <p v-else class="text-xs text-zinc-600">
        Manage settings and generation from Drop on your desktop.
      </p>
    </div>

    <!-- Not in a session -->
    <div v-else-if="!compact" class="space-y-6">
      <div class="rounded-xl bg-zinc-900/60 p-6">
        <h2 class="text-lg font-medium text-zinc-200 mb-1">Start a session</h2>
        <p class="text-sm text-zinc-500 mb-4">
          Collect everyone's settings in one place, then generate the multiworld.
        </p>
        <div class="flex items-center gap-3">
          <input
            v-model="newName"
            placeholder="Session name (optional)"
            maxlength="64"
            class="flex-1 px-4 py-2.5 rounded-lg bg-zinc-800 text-zinc-100 placeholder:text-zinc-600 focus:outline-none focus:ring-2 focus:ring-purple-500"
            @keyup.enter="create(newName)"
          />
          <button
            :disabled="busy"
            class="px-5 py-2.5 rounded-lg text-sm font-medium bg-purple-600 text-white hover:bg-purple-500 disabled:opacity-50"
            @click="create(newName)"
          >
            {{ busy ? "Setting up…" : "Start" }}
          </button>
        </div>
      </div>

      <div class="rounded-xl bg-zinc-900/60 p-6">
        <h2 class="text-lg font-medium text-zinc-200 mb-1">Join a session</h2>
        <p class="text-sm text-zinc-500 mb-4">
          Enter the code the host shared with you.
        </p>
        <div class="flex items-center gap-3">
          <input
            v-model="joinCode"
            placeholder="ABC-123"
            maxlength="16"
            class="flex-1 px-4 py-2.5 rounded-lg bg-zinc-800 text-zinc-100 text-lg font-mono tracking-widest uppercase placeholder:text-zinc-600 focus:outline-none focus:ring-2 focus:ring-purple-500"
            @keyup.enter="join(joinCode)"
          />
          <button
            :disabled="busy || joinCode.trim().length === 0"
            class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
            @click="join(joinCode)"
          >
            {{ busy ? "Joining…" : "Join" }}
          </button>
        </div>
      </div>
    </div>

    <p v-else class="text-sm text-zinc-600">
      No active multiworld. Start one from Drop on your desktop.
    </p>
  </div>
</template>

<script setup lang="ts">
/**
 * Archipelago session panel.
 *
 * `compact` renders a read-only view (status + connect string) for Big Picture,
 * where picking a YAML file with a controller isn't practical. Setup happens on
 * the desktop; Big Picture is for seeing where to connect.
 */
import { useArchipelago } from "~/composables/archipelago";

defineProps<{ compact?: boolean }>();

const {
  detail,
  busy,
  error,
  notice,
  sessionEnded,
  codeCopied,
  connectCopied,
  displayCode,
  refresh,
  startPolling,
  stopPolling,
  copyCode,
  copyConnect,
  create,
  join,
  uploadYaml,
  saveBundle,
  setConnect,
  leave,
  restore,
  dismissSessionEnded,
} = useArchipelago();

const joinCode = ref("");
const newName = ref("");
const connectInput = ref("");
const confirmingLeave = ref(false);

const mySlot = computed(() => detail.value?.slots.find((s) => s.isSelf));

// The HOST_ADDRESS setup is a one-time, per-server chore, but Drop can't detect
// that it's been done (it's a file in the Archipelago container). So the host
// dismisses the banner explicitly, and we remember that against the address —
// the banner only returns if the address changes, i.e. the setup really is
// stale. Persisted in localStorage so it survives restarts (this is host-local
// guidance, not shared state).
const setupAckKey = computed(() =>
  detail.value?.serverAddress
    ? `ap-setup-ack:${detail.value.serverAddress}`
    : null,
);
const setupDismissed = ref(false);
watchEffect(() => {
  setupDismissed.value =
    import.meta.client && setupAckKey.value
      ? localStorage.getItem(setupAckKey.value) === "1"
      : false;
});
const showSetupBanner = computed(
  () => !!detail.value?.isHost && !!detail.value?.serverAddress && !setupDismissed.value,
);
function dismissSetup() {
  if (import.meta.client && setupAckKey.value) {
    localStorage.setItem(setupAckKey.value, "1");
    setupDismissed.value = true;
  }
}

async function submitConnect() {
  await setConnect(connectInput.value);
  connectInput.value = "";
}

async function doLeave() {
  confirmingLeave.value = false;
  await leave();
}

onMounted(() => {
  if (detail.value) {
    refresh();
    startPolling();
  } else {
    restore();
  }
});
onUnmounted(() => {
  stopPolling();
});
</script>
