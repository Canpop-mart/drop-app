<template>
  <div>
    <div class="border-b border-zinc-700 py-5">
      <h3
        class="text-base font-semibold font-display leading-6 text-zinc-100"
      >
        Remote play
      </h3>
      <p class="mt-1 text-sm text-zinc-400">
        Play the games on this PC from another device, like a Steam Deck or a
        laptop. Both devices need to be on the same home network.
      </p>
      <p class="mt-1 text-sm text-zinc-500">
        To play a game that lives on another PC on this one instead, open Big
        Picture and pick the game there.
      </p>
    </div>

    <div class="mt-5">
      <StreamingSetup @changed="onSetupChanged" />
    </div>

    <!-- Network access. Nothing outside this PC can reach remote play until
         Windows is told to allow it, and the only place that could be fixed used
         to be the first-run wizard: anyone who declined the administrator prompt,
         or who never ran an install because their copy was already healthy, was
         refused every stream with no button anywhere to put it right. -->
    <div v-if="firewall?.supported" class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-1">Network Access</h4>
      <p class="text-sm text-zinc-400 mb-3">
        Windows has to let other devices on your network reach this PC.
      </p>
      <div
        v-if="firewall.allowed"
        class="flex items-center gap-2 text-sm text-green-400"
      >
        <span class="size-2 rounded-full bg-green-400" />
        Other devices can reach this PC.
      </div>
      <div v-else class="space-y-3">
        <div class="flex items-center gap-2 text-sm text-amber-300">
          <span class="size-2 rounded-full bg-amber-400" />
          Windows is blocking other devices, so streams from this PC are turned
          away.
        </div>
        <p class="text-xs text-zinc-500">
          Drop asks for administrator rights once to add the permission, then
          never again.
        </p>
        <button
          class="rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-40"
          :disabled="fixingFirewall"
          @click="doFirewall"
        >
          {{
            fixingFirewall
              ? "Waiting for Windows..."
              : firewallError
                ? "Try again"
                : "Allow it"
          }}
        </button>
        <p v-if="firewallError" class="text-xs text-amber-400">
          {{ firewallError }}
        </p>
      </div>
    </div>

    <!-- Stream quality — used when THIS PC watches a game streamed from another
         PC (the profile + HDR + auto-resolution Moonlight requests). -->
    <div class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-1">Stream Quality</h4>
      <p class="text-sm text-zinc-400 mb-3">
        Profile used when this PC watches a game streamed from another PC.
        Higher looks sharper but needs more bandwidth.
      </p>
      <select
        v-model="streamingQuality"
        class="w-full max-w-xs rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
        @change="saveStreamingQuality"
      >
        <option value="performance">Performance (60fps · 18 Mbps)</option>
        <option value="balanced">Balanced (60fps · 30 Mbps)</option>
        <option value="quality">Quality (60fps · 50 Mbps)</option>
        <option value="ultra">Ultra (120fps · 80 Mbps)</option>
      </select>
      <label class="mt-3 flex items-center gap-2 text-sm text-zinc-300">
        <input
          v-model="streamingHdr"
          type="checkbox"
          class="rounded border-zinc-600 bg-zinc-800 text-blue-600 focus:ring-blue-500"
          @change="saveStreamingToggles"
        />
        HDR (10-bit) — best on an HDR display
      </label>
      <label class="mt-2 flex items-center gap-2 text-sm text-zinc-300">
        <input
          v-model="streamingAutoResolution"
          type="checkbox"
          class="rounded border-zinc-600 bg-zinc-800 text-blue-600 focus:ring-blue-500"
          @change="saveStreamingToggles"
        />
        Auto resolution — match this device's screen when watching
      </label>
    </div>

    <!-- Host resolution: the display mode this PC switches to while HOSTING a
         stream (Sunshine does the switching, via dd_resolution_option). Match it
         to the device you stream to (small for a handheld, 1080p/4K for a docked
         TV); "Don't change" leaves your desktop alone. With auto resolution on
         the host is never switched, but this value is still the fallback for
         what to ask the client for, so the control stays enabled. -->
    <div class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-1">Host Resolution</h4>
      <p class="text-sm text-zinc-400 mb-3">
        The resolution this PC switches to while streaming a game to another
        device. Match it to the device you stream to.
      </p>
      <select
        v-model="streamingResolution"
        class="w-full max-w-xs rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
        @change="saveStreamingResolution"
      >
        <option value="1280x800">Handheld (1280×800)</option>
        <option value="1920x1080">1080p (1920×1080)</option>
        <option value="2560x1440">1440p (2560×1440)</option>
        <option value="3840x2160">4K (3840×2160)</option>
        <option value="native">Don't change my resolution</option>
      </select>
      <p v-if="streamingAutoResolution" class="mt-2 text-xs text-zinc-500">
        Auto resolution is on, so this PC keeps its own resolution and only uses
        the value above if the other device does not report its screen size.
      </p>
      <p v-else-if="resolutionSaved" class="mt-2 text-xs text-green-400">
        Saved.
      </p>
    </div>

    <!-- Settings below only reach Sunshine through the config file Drop writes
         for the Sunshine it starts itself, so an instance started outside Drop
         ignores every one of them. Saying nothing here is how the pickers end up
         looking like they work. -->
    <div
      v-if="hostDevicesAvailable && sunshineIsForeign"
      class="mt-8 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-sm text-amber-200"
    >
      Another copy of Sunshine is already running on this PC and Drop did not
      start it. It uses its own configuration, so the display and audio settings
      below are not in effect. Close that Sunshine and let Drop start its own to
      apply them.
    </div>

    <!-- Which monitor gets captured. The whole point: without this Sunshine
         picks one by chance, and a multi-monitor PC streams the wrong screen. -->
    <div v-if="hostDevicesAvailable" class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-1">Capture Display</h4>
      <p class="text-sm text-zinc-400 mb-3">
        The monitor other devices see when they stream from this PC. Drop
        switches it on if it is off, and leaves your other monitors alone.
      </p>
      <select
        v-model="streamingDisplay"
        class="w-full max-w-md rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
        @change="saveStreamingDisplay"
      >
        <option value="">Automatic (let Sunshine choose)</option>
        <option v-for="d in displays" :key="d.deviceId" :value="d.deviceId">
          {{ displayLabel(d) }}
        </option>
      </select>
      <p v-if="displaysError" class="mt-2 text-xs text-amber-400">
        {{ displaysError }}
      </p>
      <p v-else-if="displaySaved" class="mt-2 text-xs text-green-400">Saved.</p>
    </div>

    <!-- Audio routing. virtual_sink is what actually silences the PC. -->
    <div v-if="hostDevicesAvailable" class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-1">Stream Audio</h4>
      <p class="text-sm text-zinc-400 mb-3">
        Where the sound goes while another device is streaming from this PC.
      </p>

      <label class="block text-xs text-zinc-400 mb-1">Capture from</label>
      <select
        v-model="streamingAudioSink"
        class="w-full max-w-md rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
        @change="saveStreamingAudio"
      >
        <option value="">Automatic (current default device)</option>
        <option v-for="s in audioSinks" :key="s.name" :value="s.name">
          {{ sinkLabel(s) }}
        </option>
      </select>

      <label class="block text-xs text-zinc-400 mb-1 mt-4">
        Silence this PC using
      </label>
      <select
        v-model="streamingVirtualSink"
        class="w-full max-w-md rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
        @change="saveStreamingAudio"
      >
        <option value="">Automatic (find Steam's streaming speakers)</option>
        <option v-for="s in audioSinks" :key="s.name" :value="s.name">
          {{ sinkLabel(s) }}
        </option>
      </select>
      <p class="mt-2 text-xs text-zinc-500">
        Sound normally keeps playing out of this PC while you stream. Routing it
        through a virtual device sends it to the device you are playing on
        instead, and puts your speakers back when the stream ends.
      </p>
      <p v-if="audioError" class="mt-2 text-xs text-amber-400">
        {{ audioError }}
      </p>
      <p v-else-if="audioSaved" class="mt-2 text-xs text-green-400">Saved.</p>
    </div>

    <!-- Paired devices. Sunshine only answers this call while it is running,
         so the section says why it is empty instead of showing nothing. -->
    <div class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-1">Paired Devices</h4>
      <p class="text-sm text-zinc-400 mb-3">
        Devices that are allowed to play games from this PC without entering a
        code again.
      </p>
      <div v-if="!hostRunning" class="text-sm text-zinc-500">
        Turn on remote play to see which devices are paired.
      </div>
      <div v-else-if="clientsLoading" class="text-sm text-zinc-500">
        Loading paired devices...
      </div>
      <div v-else-if="clientsError" class="text-sm text-amber-400">
        {{ clientsError }}
      </div>
      <div v-else-if="pairedClients.length === 0" class="text-sm text-zinc-500">
        No devices are paired yet.
      </div>
      <div v-else class="space-y-2">
        <div
          v-for="client in pairedClients"
          :key="client.uuid"
          class="flex items-center justify-between rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-3"
        >
          <div class="text-sm text-zinc-200">{{ client.name }}</div>
          <button
            class="rounded-md bg-zinc-700 px-3 py-1.5 text-xs text-zinc-200 hover:bg-zinc-600 disabled:opacity-40"
            :disabled="unpairing === client.uuid"
            @click="doUnpair(client)"
          >
            {{ unpairing === client.uuid ? "Removing..." : "Remove" }}
          </button>
        </div>
      </div>
    </div>

    <!-- Active sessions -->
    <div class="mt-8">
      <h4 class="text-sm font-semibold text-zinc-200 mb-3">
        Active Sessions
      </h4>
      <div v-if="sessionsLoading" class="text-sm text-zinc-500">
        Loading sessions...
      </div>
      <div
        v-else-if="sessions.length === 0"
        class="text-sm text-zinc-500"
      >
        Nothing is streaming right now.
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
          <div class="flex items-center gap-3">
            <span class="text-xs text-zinc-500">
              {{ formatSessionTime(session.createdAt) }}
            </span>
            <!-- Until now a session that hung on the host could only be
                 cleared by quitting Drop on both machines. -->
            <button
              class="rounded-md bg-zinc-700 px-3 py-1.5 text-xs text-zinc-200 hover:bg-zinc-600 disabled:opacity-40"
              :disabled="stoppingSession === session.id"
              @click="doStopSession(session)"
            >
              {{ stoppingSession === session.id ? "Stopping..." : "Stop" }}
            </button>
          </div>
        </div>
      </div>
      <p v-if="sessionsError" class="mt-2 text-xs text-amber-400">
        {{ sessionsError }}
      </p>
    </div>

    <!-- The real product names live here and nowhere else. Nobody needs them to
         use the feature, but every useful search result and log line uses them,
         and the web page is where the advanced knobs are. -->
    <details class="mt-8 rounded-lg border border-zinc-700/50 bg-zinc-800/30 p-3">
      <summary class="cursor-pointer text-sm text-zinc-300">
        Troubleshooting and technical details
      </summary>
      <div class="mt-2 space-y-2 text-xs text-zinc-400">
        <p>
          Drop uses Sunshine on this PC and Moonlight on the other device. Both
          are open source, so searching for either name turns up help that
          applies here.
        </p>
        <p v-if="hostVersion">Sunshine version {{ hostVersion }}.</p>
        <button
          class="rounded-md bg-zinc-700 px-3 py-1.5 text-xs text-zinc-200 hover:bg-zinc-600 disabled:opacity-40"
          :disabled="!hostRunning"
          @click="openWebUi"
        >
          Open Sunshine settings
        </button>
        <p v-if="!hostRunning" class="text-zinc-500">
          Available while remote play is on.
        </p>
        <p v-else class="text-zinc-500">
          Opens {{ webUiUrl }} in your browser. Your browser will warn about the
          certificate; that is expected for a page served by your own PC.
        </p>
        <p v-if="webUiError" class="text-amber-400">{{ webUiError }}</p>

        <!-- The way out of "Drop could not sign in to remote play", which is
             what the other device is told when Drop's stored password and the
             one Sunshine hashed have drifted apart. The wording of that message
             names this button, so the two have to stay in step. -->
        <div class="pt-2">
          <button
            class="rounded-md bg-zinc-700 px-3 py-1.5 text-xs text-zinc-200 hover:bg-zinc-600 disabled:opacity-40"
            :disabled="resettingCredentials"
            @click="doResetCredentials"
          >
            {{ resettingCredentials ? "Resetting..." : "Reset remote play sign-in" }}
          </button>
          <p class="mt-1 text-zinc-500">
            Turns remote play off and gives it a new password the next time it
            starts. Use this when another device says Drop could not sign in.
          </p>
          <p v-if="credentialsError" class="mt-1 text-amber-400">
            {{ credentialsError }}
          </p>
          <p v-else-if="credentialsReset" class="mt-1 text-green-400">
            Done. Turn remote play back on when you are ready.
          </p>
        </div>
      </div>
    </details>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import {
  useStreaming,
  type StreamingSession,
  type SunshineStatusResult,
} from "~/composables/useStreaming";

const { listRemoteSessions, stopStreamingSession } = useStreaming();

/** Mirrors `SunshinePairedClient` in src-tauri/src/streaming.rs. */
type PairedClient = { uuid: string; name: string };

/** Mirrors `FirewallStatus` in src-tauri/src/streaming.rs. */
type FirewallStatus = {
  supported: boolean;
  configured: boolean;
  allowed: boolean;
};

type HostDisplay = {
  deviceId: string;
  display: string;
  adapter: string;
  resolution: string;
  friendlyName: string;
  primary: boolean;
};

type HostAudioSink = {
  name: string;
  active: boolean;
  default: boolean;
  virtualSink: boolean;
};

const sessions = ref<StreamingSession[]>([]);
const sessionsLoading = ref(true);
const sessionsError = ref("");
const stoppingSession = ref<string | null>(null);

// Host-side state, kept next to the setup card so the paired list and the web
// UI link know whether Sunshine is actually answering.
const hostRunning = ref(false);
const hostVersion = ref("");
const webUiPort = ref(47990);
const webUiError = ref("");
const webUiUrl = computed(() => `https://localhost:${webUiPort.value}`);

const pairedClients = ref<PairedClient[]>([]);
const clientsLoading = ref(false);
const clientsError = ref("");
const unpairing = ref<string | null>(null);

// Null until the first query answers, and again if it ever fails: the section
// only renders once Drop knows there is something here to report.
const firewall = ref<FirewallStatus | null>(null);
const fixingFirewall = ref(false);
const firewallError = ref("");

const resettingCredentials = ref(false);
const credentialsError = ref("");
const credentialsReset = ref(false);

// Host display resolution while streaming (read by streaming.rs at launch).
const streamingResolution = ref("1280x800");
const resolutionSaved = ref(false);

// Client-side stream quality (used when this PC watches a stream).
const streamingQuality = ref("balanced");
const streamingHdr = ref(false);
const streamingAutoResolution = ref(true);

// Host capture devices. `streamingDisplay` holds Sunshine's own display id
// rather than \\.\DISPLAYn, which renumbers across reboots and would drift onto
// the wrong monitor. The whole section hides when the host cannot enumerate
// devices (anything but Windows).
const displays = ref<HostDisplay[]>([]);
const audioSinks = ref<HostAudioSink[]>([]);
const hostDevicesAvailable = ref(false);
const displaysError = ref("");
const audioError = ref("");
const streamingDisplay = ref("");
const streamingAudioSink = ref("");
const streamingVirtualSink = ref("");
const displaySaved = ref(false);
const audioSaved = ref(false);
const sunshineIsForeign = ref(false);

function displayLabel(d: HostDisplay): string {
  const bits = [d.friendlyName];
  if (d.resolution) bits.push(d.resolution);
  if (d.adapter) bits.push(d.adapter);
  if (d.primary) bits.push("main display");
  return bits.join(" · ");
}

// Some machines report every endpoint as inactive, in which case the flag is
// noise and tagging working speakers "not available" is worse than saying
// nothing.
const activeFlagIsMeaningful = computed(() =>
  audioSinks.value.some((s) => s.active),
);

function sinkLabel(s: HostAudioSink): string {
  if (s.default) return `${s.name} (current default)`;
  if (!s.active && activeFlagIsMeaningful.value) {
    return `${s.name} (not available right now)`;
  }
  return s.name;
}

function flash(flag: Ref<boolean>) {
  flag.value = true;
  setTimeout(() => {
    flag.value = false;
  }, 1500);
}

async function saveStreamingDisplay() {
  // The adapter travels with the display: Sunshine needs the GPU the chosen
  // monitor actually hangs off, and pairing a display with the wrong one makes
  // capture fail outright.
  const chosen = displays.value.find((d) => d.deviceId === streamingDisplay.value);
  try {
    await invoke("update_settings", {
      newSettings: {
        streamingDisplay: streamingDisplay.value,
        streamingAdapter: chosen?.adapter ?? "",
      },
    });
    displaysError.value = "";
    flash(displaySaved);
  } catch (e) {
    console.error("[SETTINGS] Failed to save capture display:", e);
    displaysError.value = "Could not save the capture display.";
  }
}

async function saveStreamingAudio() {
  try {
    await invoke("update_settings", {
      newSettings: {
        streamingAudioSink: streamingAudioSink.value,
        streamingVirtualSink: streamingVirtualSink.value,
      },
    });
    audioError.value = "";
    flash(audioSaved);
  } catch (e) {
    console.error("[SETTINGS] Failed to save stream audio devices:", e);
    audioError.value = "Could not save the audio devices.";
  }
}

async function loadHostDevices() {
  try {
    displays.value = await invoke<HostDisplay[]>("sunshine_list_displays");
    hostDevicesAvailable.value = true;
  } catch (e) {
    // Not a Windows host, or the display query failed. Leave the section out
    // rather than showing an empty picker.
    console.warn("[SETTINGS] Could not list host displays:", e);
    return;
  }
  try {
    audioSinks.value = await invoke<HostAudioSink[]>("sunshine_list_audio_sinks");
  } catch (e) {
    console.warn("[SETTINGS] Could not list host audio devices:", e);
    audioError.value = "Could not read this PC's audio devices.";
  }
  try {
    sunshineIsForeign.value = await invoke<boolean>("sunshine_is_foreign");
  } catch (e) {
    console.warn("[SETTINGS] Could not check for a foreign Sunshine:", e);
  }
}

async function loadPairedClients() {
  if (!hostRunning.value) {
    pairedClients.value = [];
    return;
  }
  clientsLoading.value = true;
  clientsError.value = "";
  try {
    const list = await invoke<{ count: number; clients: PairedClient[] }>(
      "sunshine_list_clients",
    );
    pairedClients.value = list.clients;
  } catch (e) {
    console.warn("[SETTINGS] Could not list paired devices:", e);
    clientsError.value = "Could not read the paired devices from this PC.";
  } finally {
    clientsLoading.value = false;
  }
}

async function doUnpair(client: PairedClient) {
  unpairing.value = client.uuid;
  clientsError.value = "";
  try {
    await invoke("sunshine_unpair_client", { uuid: client.uuid });
    await loadPairedClients();
  } catch (e) {
    clientsError.value = typeof e === "string" ? e : String(e);
  } finally {
    unpairing.value = null;
  }
}

async function refreshFirewall() {
  try {
    firewall.value = await invoke<FirewallStatus>("sunshine_firewall_status");
  } catch (e) {
    // An unanswerable query is not evidence of a problem, and a red row nobody
    // can act on is worse than no row: hide the section instead.
    console.warn("[SETTINGS] Could not read the firewall status:", e);
    firewall.value = null;
  }
}

async function doFirewall() {
  fixingFirewall.value = true;
  firewallError.value = "";
  try {
    await invoke("sunshine_configure_firewall");
  } catch (e) {
    firewallError.value = typeof e === "string" ? e : String(e);
  } finally {
    // Ask Windows again rather than trusting the call: a declined administrator
    // prompt and a silent no-op look identical from here.
    await refreshFirewall();
    fixingFirewall.value = false;
    if (!firewallError.value && firewall.value?.supported && !firewall.value.allowed) {
      firewallError.value =
        "Windows still is not letting other devices through. Try again, or add the permission yourself in Windows Defender Firewall.";
    }
  }
}

async function doResetCredentials() {
  resettingCredentials.value = true;
  credentialsError.value = "";
  credentialsReset.value = false;
  try {
    await invoke("sunshine_reset_credentials");
    credentialsReset.value = true;
    await refreshHostState();
  } catch (e) {
    credentialsError.value = typeof e === "string" ? e : String(e);
  } finally {
    resettingCredentials.value = false;
  }
}

/** Re-read host status and everything that depends on it. */
async function refreshHostState() {
  try {
    const status = await invoke<SunshineStatusResult>("sunshine_status");
    hostRunning.value = status.running;
    hostVersion.value = status.version;
    webUiPort.value = status.webUiPort;
  } catch (e) {
    console.warn("[SETTINGS] Could not read remote play status:", e);
    hostRunning.value = false;
  }
  await loadPairedClients();
}

/**
 * Setting up remote play is also what opens the firewall, and the prompt that
 * does it can be declined. Re-read both, or the row above keeps saying "Ready"
 * for a PC that will turn every stream away.
 */
async function onSetupChanged() {
  await refreshHostState();
  await refreshFirewall();
}

async function loadSessions() {
  sessionsError.value = "";
  try {
    sessions.value = await listRemoteSessions();
  } finally {
    sessionsLoading.value = false;
  }
}

async function doStopSession(session: StreamingSession) {
  stoppingSession.value = session.id;
  sessionsError.value = "";
  try {
    await stopStreamingSession(session.id);
    await loadSessions();
  } catch (e) {
    sessionsError.value = typeof e === "string" ? e : String(e);
  } finally {
    stoppingSession.value = null;
  }
}

async function openWebUi() {
  webUiError.value = "";
  try {
    await openExternal(webUiUrl.value);
  } catch (e) {
    console.warn("[SETTINGS] Could not open the Sunshine web UI:", e);
    webUiError.value = `Could not open ${webUiUrl.value}. Paste it into your browser instead.`;
  }
}

async function saveStreamingQuality() {
  try {
    await invoke("update_settings", {
      newSettings: { streamingQuality: streamingQuality.value },
    });
  } catch (e) {
    console.error("[SETTINGS] Failed to save stream quality:", e);
  }
}

async function saveStreamingToggles() {
  try {
    await invoke("update_settings", {
      newSettings: {
        streamingHdr: streamingHdr.value,
        streamingAutoResolution: streamingAutoResolution.value,
      },
    });
  } catch (e) {
    console.error("[SETTINGS] Failed to save streaming toggles:", e);
  }
}

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
    if (typeof settings.streamingQuality === "string") {
      streamingQuality.value = settings.streamingQuality;
    }
    if (typeof settings.streamingHdr === "boolean") {
      streamingHdr.value = settings.streamingHdr;
    }
    if (typeof settings.streamingAutoResolution === "boolean") {
      streamingAutoResolution.value = settings.streamingAutoResolution;
    }
    if (typeof settings.streamingDisplay === "string") {
      streamingDisplay.value = settings.streamingDisplay;
    }
    if (typeof settings.streamingAudioSink === "string") {
      streamingAudioSink.value = settings.streamingAudioSink;
    }
    if (typeof settings.streamingVirtualSink === "string") {
      streamingVirtualSink.value = settings.streamingVirtualSink;
    }
  } catch {
    // keep default
  }
  await loadHostDevices();
  await refreshHostState();
  await refreshFirewall();
  await loadSessions();
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
