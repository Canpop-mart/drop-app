<template>
  <div class="space-y-4">
    <div
      v-if="error"
      class="px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
    >
      {{ error }}
    </div>
    <div
      v-if="notice"
      class="px-4 py-3 rounded-lg bg-green-900/25 border border-green-500/30 text-green-200 text-sm"
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
    <template v-else-if="detail">
      <!-- Session: name + code + readiness in one strip -->
      <div class="flex items-center gap-4 rounded-xl bg-zinc-900/60 p-4">
        <div class="min-w-0 flex-1">
          <p class="text-xs text-zinc-500 mb-1">
            {{ detail.name || "Session" }}
          </p>
          <button
            v-if="!compact"
            class="group inline-flex items-center gap-2"
            title="Click to copy"
            @click="copyCode"
          >
            <span class="font-mono text-2xl font-bold tracking-widest text-purple-300">
              {{ displayCode || "…" }}
            </span>
            <span
              class="text-xs font-medium"
              :class="codeCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
            >
              {{ codeCopied ? "Copied" : "Copy" }}
            </span>
          </button>
          <span
            v-else
            class="font-mono text-2xl font-bold tracking-widest text-purple-300"
          >
            {{ displayCode || "…" }}
          </span>
        </div>
        <span
          class="shrink-0 inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-medium"
          :class="detail.allReady ? 'bg-green-900/25 text-green-300' : 'bg-zinc-800 text-zinc-400'"
        >
          {{ detail.readyCount }} / {{ detail.totalCount }} ready
        </span>
      </div>

      <!-- Connect address: the payoff, prominent for everyone once it's set -->
      <div
        v-if="detail.connectAddress"
        class="rounded-xl bg-zinc-900/60 p-4 border border-purple-500/20"
      >
        <p class="text-xs uppercase tracking-wide text-zinc-500 mb-1.5">
          Connect address
        </p>
        <button
          v-if="!compact"
          class="group inline-flex items-center gap-2"
          title="Click to copy"
          @click="copyConnect"
        >
          <span class="font-mono text-lg font-bold text-purple-300">
            {{ detail.connectAddress }}
          </span>
          <span
            class="text-xs font-medium"
            :class="connectCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
          >
            {{ connectCopied ? "Copied" : "Copy" }}
          </span>
        </button>
        <span v-else class="font-mono text-lg font-bold text-purple-300">
          {{ detail.connectAddress }}
        </span>
        <p class="text-xs text-zinc-600 mt-1.5">
          Enter this in your game's Archipelago client.
        </p>
      </div>

      <!-- Players -->
      <div class="rounded-xl bg-zinc-900/60 p-2">
        <div
          v-for="s in detail.slots"
          :key="s.clientId"
          class="flex items-center gap-3 px-3 py-2.5"
        >
          <span
            class="size-2 shrink-0 rounded-full"
            :class="s.validationError ? 'bg-red-400' : s.hasYaml ? 'bg-green-400' : 'bg-zinc-600'"
          />
          <div class="min-w-0 flex-1">
            <!-- Inline rename for your own device -->
            <div
              v-if="editingName && s.isSelf"
              class="flex items-center gap-2"
            >
              <input
                v-model="nameDraft"
                maxlength="64"
                class="min-w-0 flex-1 px-2 py-1 rounded bg-zinc-800 text-sm text-zinc-100 ring-1 ring-inset ring-zinc-700 focus:outline-none focus:ring-2 focus:ring-purple-500"
                @keyup.enter="saveName"
                @keyup.esc="editingName = false"
              />
              <button
                class="shrink-0 text-xs font-medium text-purple-300 hover:text-purple-200"
                @click="saveName"
              >
                Save
              </button>
              <button
                class="shrink-0 text-xs text-zinc-500 hover:text-zinc-300"
                @click="editingName = false"
              >
                Cancel
              </button>
            </div>
            <div v-else class="flex items-center gap-2">
              <span class="text-sm text-zinc-200 truncate">
                {{ s.clientName }}
              </span>
              <button
                v-if="s.isSelf && !compact"
                title="Rename this device"
                class="shrink-0 text-zinc-500 hover:text-zinc-300"
                @click="startRename(s.clientName)"
              >
                <PencilIcon class="size-3.5" />
              </button>
              <span
                v-if="s.isHost"
                class="shrink-0 text-[11px] px-2 py-0.5 rounded-full bg-purple-600/20 text-purple-300"
              >
                Host
              </span>
            </div>
            <p
              v-if="s.slotName || s.game"
              class="text-xs text-zinc-500 truncate"
            >
              {{ s.slotName }}<span v-if="s.game"> · {{ s.game }}</span>
            </p>
            <p v-if="s.validationError" class="text-xs text-red-300/80">
              {{ s.validationError }}
            </p>
          </div>
          <span
            class="shrink-0 text-xs font-medium"
            :class="s.validationError ? 'text-red-300' : s.hasYaml ? 'text-green-400' : 'text-zinc-500'"
          >
            {{ s.validationError ? "Problem" : s.hasYaml ? "Ready" : "No settings" }}
          </span>
        </div>
      </div>

      <!-- Steps (desktop only) -->
      <template v-if="!compact">
        <div class="rounded-xl bg-zinc-900/60 p-4 space-y-5">
          <!-- 1. Make your settings -->
          <div class="flex gap-3">
            <div class="flex size-6 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-xs font-medium text-purple-300">
              1
            </div>
            <div class="min-w-0 flex-1">
              <p class="text-sm font-medium text-zinc-200 mb-2">
                Make your settings
              </p>
              <div v-if="webHostUrl" class="flex items-center gap-2">
                <button
                  class="shrink-0 px-3 py-2 rounded-lg text-sm font-medium bg-purple-600 text-white hover:bg-purple-500"
                  @click="openWebHost"
                >
                  Open Archipelago Web
                </button>
                <div class="flex flex-1 items-center gap-2">
                  <input
                    v-model="gameQuery"
                    list="ap-supported-games"
                    placeholder="Search a game…"
                    class="min-w-0 flex-1 px-3 py-2 rounded-lg bg-zinc-800 text-sm text-zinc-100 placeholder:text-zinc-600 focus:outline-none focus:ring-2 focus:ring-purple-500"
                    @keyup.enter="openGame"
                  />
                  <datalist id="ap-supported-games">
                    <option v-for="g in supportedGames" :key="g" :value="g" />
                  </datalist>
                  <button
                    :disabled="gameQuery.trim().length === 0"
                    class="shrink-0 px-3 py-2 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
                    @click="openGame"
                  >
                    Open
                  </button>
                </div>
              </div>
              <p v-else class="text-sm text-zinc-500">
                Create a YAML with the Archipelago Launcher's Options Creator.
              </p>
            </div>
          </div>

          <!-- 2. Upload your YAML -->
          <div class="flex gap-3">
            <div class="flex size-6 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-xs font-medium text-purple-300">
              2
            </div>
            <div class="min-w-0 flex-1">
              <p class="text-sm font-medium text-zinc-200 mb-2">
                Upload your YAML
              </p>
              <div class="flex items-center gap-3">
                <button
                  :disabled="busy"
                  class="px-3 py-2 rounded-lg text-sm font-medium bg-purple-600 text-white hover:bg-purple-500 disabled:opacity-50"
                  @click="uploadYaml"
                >
                  {{ mySlot?.hasYaml ? "Replace my settings" : "Upload my settings" }}
                </button>
                <span
                  v-if="mySlot?.hasYaml && !mySlot?.validationError"
                  class="text-xs font-medium text-green-400"
                >
                  Uploaded
                </span>
              </div>
            </div>
          </div>

          <!-- 3. Host generates; everyone else waits -->
          <div class="flex gap-3">
            <div class="flex size-6 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-xs font-medium text-purple-300">
              3
            </div>
            <div class="min-w-0 flex-1">
              <template v-if="detail.isHost">
                <p class="text-sm font-medium text-zinc-200 mb-1">
                  Generate, then share the connect address
                </p>
                <p class="text-xs text-zinc-500 mb-3">
                  Save everyone's settings, generate on the web, then paste the
                  connect line so everyone gets it.
                </p>
                <button
                  :disabled="busy || detail.readyCount === 0"
                  class="px-3 py-2 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
                  @click="saveBundle"
                >
                  Save all settings
                </button>
                <p
                  v-if="detail.readyCount === 0"
                  class="text-xs text-zinc-600 mt-2"
                >
                  Nobody has uploaded settings yet.
                </p>
                <p
                  v-else-if="!detail.allReady"
                  class="text-xs text-amber-300/70 mt-2"
                >
                  {{ detail.totalCount - detail.readyCount }} player(s) still
                  haven't uploaded.
                </p>
                <div class="mt-3 flex items-center gap-2">
                  <input
                    v-model="connectInput"
                    placeholder="10.243.0.1:38281"
                    class="min-w-0 flex-1 px-3 py-2 rounded-lg bg-zinc-800 text-zinc-100 font-mono text-sm placeholder:text-zinc-600 focus:outline-none focus:ring-2 focus:ring-purple-500"
                    @keyup.enter="submitConnect"
                  />
                  <button
                    :disabled="busy || connectInput.trim().length === 0"
                    class="shrink-0 px-4 py-2 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
                    @click="submitConnect"
                  >
                    Save
                  </button>
                </div>
              </template>
              <template v-else>
                <p class="text-sm font-medium text-zinc-200 mb-1">
                  Wait for the host
                </p>
                <p class="text-xs text-zinc-500">
                  {{
                    detail.connectAddress
                      ? "Use the connect address above in your game."
                      : "The host will generate the multiworld and share the connect address here."
                  }}
                </p>
              </template>
            </div>
          </div>
        </div>

        <!--
          One-time HOST_ADDRESS setup, collapsed. Drop can't detect whether it's
          been done (it's a file in the Archipelago container), so the host
          confirms it by hand; dismissal is keyed to the address so it only
          returns if the address changes.
        -->
        <details
          v-if="showSetupBanner"
          class="rounded-xl bg-amber-900/15 border border-amber-500/25 px-4 py-3"
        >
          <summary class="cursor-pointer text-sm font-medium text-amber-200">
            One-time server setup
          </summary>
          <p class="text-xs text-amber-100/70 mt-2 mb-2">
            So players off your network can reach it, set this in Archipelago's
            <span class="font-mono">config.yaml</span> and restart it.
          </p>
          <code class="block px-3 py-2 rounded bg-black/40 text-amber-200 font-mono text-xs">
            HOST_ADDRESS: {{ detail.serverAddress }}
          </code>
          <button
            class="mt-2 px-3 py-1.5 rounded-md text-xs font-medium bg-amber-600/30 text-amber-100 hover:bg-amber-600/50"
            @click="dismissSetup"
          >
            I've done this
          </button>
        </details>

        <!-- Detailed how-to, tucked away -->
        <details class="rounded-xl bg-zinc-900/40 px-4 py-3">
          <summary class="cursor-pointer text-sm text-zinc-400">
            How multiworlds work
          </summary>
          <div class="mt-2 space-y-2 text-xs text-zinc-500 leading-relaxed">
            <p>
              Everyone makes a YAML on the Archipelago web (the game search
              above, or the Launcher's Options Creator), then uploads it here.
              Drop checks it and flags problems right away.
            </p>
            <p>
              The host saves everyone's settings as one file, uploads it on the
              web's Generate page, hosts the room, then pastes the connect line
              here so everyone gets it.
            </p>
          </div>
        </details>

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
    </template>

    <!-- Not in a session -->
    <template v-else-if="!compact">
      <div class="rounded-xl bg-zinc-900/60 p-6">
        <h2 class="text-lg font-medium text-zinc-200 mb-1">Start a session</h2>
        <p class="text-sm text-zinc-500 mb-4">
          Collect everyone's settings, then generate the multiworld together.
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

      <button
        v-if="webHostUrl"
        class="text-sm text-zinc-400 hover:text-zinc-200 inline-flex items-center gap-1.5"
        @click="openWebHost"
      >
        Open Archipelago Web
      </button>
    </template>

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
 * where picking a YAML file or driving a browser with a controller isn't
 * practical. Setup happens on the desktop; Big Picture is for seeing where to
 * connect.
 */
import { PencilIcon } from "@heroicons/vue/16/solid";
import { useArchipelago } from "~/composables/archipelago";
import { useDisplayName } from "~/composables/use-display-name";

const props = defineProps<{ compact?: boolean }>();

const {
  detail,
  busy,
  error,
  notice,
  sessionEnded,
  codeCopied,
  connectCopied,
  webHostUrl,
  supportedGames,
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
  loadConfig,
  openWebHost,
  openGameOptions,
} = useArchipelago();

const { setName } = useDisplayName();

const joinCode = ref("");
const newName = ref("");
const connectInput = ref("");
const confirmingLeave = ref(false);
const gameQuery = ref("");
const editingName = ref(false);
const nameDraft = ref("");

const mySlot = computed(() => detail.value?.slots.find((s) => s.isSelf));

function openGame() {
  const g = gameQuery.value.trim();
  if (g) openGameOptions(g);
}

function startRename(current: string) {
  nameDraft.value = current;
  editingName.value = true;
}

async function saveName() {
  const n = nameDraft.value.trim();
  if (!n) return;
  await setName(n);
  editingName.value = false;
  await refresh();
}

// The HOST_ADDRESS setup is a one-time, per-server chore, but Drop can't detect
// that it's been done (it's a file in the Archipelago container). So the host
// dismisses it explicitly, and we remember that against the address — it only
// returns if the address changes, i.e. the setup really is stale. Persisted in
// localStorage so it survives restarts (host-local guidance, not shared state).
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
  // WebHost tools are desktop-only, so don't bother fetching config in compact.
  if (!props.compact) loadConfig();
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
