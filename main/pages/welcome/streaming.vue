<template>
  <BpmWizardShell
    step-key="streaming"
    :title="title"
    :subtitle="subtitle"
    manual-advance
    :next-disabled="nextDisabled"
    :next-label="nextLabel"
    @next="handleNext"
    @skip="handleSkip"
  >
    <!-- Sub-step 1: what this is -->
    <div v-if="subStep === 'intro'" class="max-w-2xl space-y-5">
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
          Play the games on this PC from another device, like a Steam Deck or a
          laptop. Both devices need to be on the same home network.
        </p>
        <p class="text-sm mt-3" :style="{ color: 'var(--bpm-muted)' }">
          The game keeps running here. The other device only sends your button
          presses and shows the picture.
        </p>
      </div>

      <!-- Simple flow diagram -->
      <div class="flex items-center justify-between gap-3 py-4">
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: 'var(--bpm-surface)', border: '1px solid var(--bpm-border)' }"
          >
            <span class="text-lg">🖥️</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">This PC</p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: 'var(--bpm-surface)', border: '1px solid var(--bpm-border)' }"
          >
            <span class="text-lg">📶</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">Home network</p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 20%, transparent)' }"
          >
            <span class="text-lg">🎮</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-accent-hex)' }">Other device</p>
        </div>
      </div>

      <div class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
        Setting this up takes a couple of minutes and a one-time download. You
        can skip it and come back from Settings later.
      </div>
    </div>

    <!-- Sub-step 2: download and check the files -->
    <div v-else-if="subStep === 'install'" class="max-w-xl space-y-4">
      <div
        v-if="hostReady"
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 10%, transparent)',
          border: '1px solid var(--bpm-accent-hex)',
        }"
      >
        <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-accent-hex)' }">
          Remote play is set up on this PC
        </p>
        <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
          Everything it needs is on disk and passed the check.
        </p>
      </div>

      <template v-else>
        <div
          class="rounded-xl p-4 text-sm"
          :style="{
            backgroundColor: 'var(--bpm-surface)',
            border: '1px solid var(--bpm-border)',
            color: 'var(--bpm-muted)',
          }"
        >
          {{
            status?.installed
              ? "Some files are missing or damaged. Setting up again replaces them."
              : "Drop downloads about 70 MB the first time. It only happens once."
          }}
        </div>

        <div
          v-if="installError"
          class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {{ installError }}
        </div>

        <button
          :ref="(el: any) => registerContent(el, { onSelect: doInstall })"
          :disabled="installing"
          class="flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{ backgroundColor: 'var(--bpm-accent-hex)', color: 'white' }"
          @click="doInstall"
        >
          <span
            v-if="installing"
            class="size-4 rounded-full border-2 border-white/30 border-t-white animate-spin"
          />
          {{ installLabel }}
        </button>

        <p v-if="installing" class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
          This can take a minute on a slow connection. Leave the screen open.
        </p>
      </template>
    </div>

    <!-- Sub-step 3: let other devices reach this PC -->
    <div v-else-if="subStep === 'permissions'" class="max-w-xl space-y-4">
      <div
        v-if="!firewall?.supported"
        class="rounded-xl p-4 text-sm"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
          color: 'var(--bpm-muted)',
        }"
      >
        There is nothing to open on this device. Continue.
      </div>

      <div
        v-else-if="firewall.allowed"
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 10%, transparent)',
          border: '1px solid var(--bpm-accent-hex)',
        }"
      >
        <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-accent-hex)' }">
          Other devices can reach this PC
        </p>
        <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
          Windows is letting remote play through.
        </p>
      </div>

      <template v-else>
        <div
          class="rounded-xl p-4 text-sm"
          :style="{
            backgroundColor: 'var(--bpm-surface)',
            border: '1px solid var(--bpm-border)',
            color: 'var(--bpm-muted)',
          }"
        >
          Windows blocks other devices from reaching this PC until you allow it.
          Drop asks for administrator rights once to add the permission, then
          never again.
        </div>

        <div
          v-if="firewallError"
          class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {{ firewallError }}
        </div>

        <button
          :ref="(el: any) => registerContent(el, { onSelect: doFirewall })"
          :disabled="fixingFirewall"
          class="flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{ backgroundColor: 'var(--bpm-accent-hex)', color: 'white' }"
          @click="doFirewall"
        >
          <span
            v-if="fixingFirewall"
            class="size-4 rounded-full border-2 border-white/30 border-t-white animate-spin"
          />
          {{ fixingFirewall ? "Waiting for Windows..." : firewallError ? "Try again" : "Allow it" }}
        </button>

        <!-- The way past a prompt the user cannot answer. Continue unlocks once
             they have tried, so this step is no longer a wall for anyone on a
             standard Windows account. -->
        <p
          v-if="firewallAttempted && !fixingFirewall"
          class="text-xs"
          :style="{ color: 'var(--bpm-muted)' }"
        >
          You can carry on without this. Other devices will not reach this PC
          until Windows allows it, and the same button is waiting in Settings,
          under Remote play.
        </p>
      </template>
    </div>

    <!-- Sub-step 4: which screen and which speakers -->
    <div v-else-if="subStep === 'screen'" class="max-w-2xl space-y-4">
      <div
        v-if="!hostDevicesAvailable"
        class="rounded-xl p-4 text-sm"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
          color: 'var(--bpm-muted)',
        }"
      >
        Drop cannot list this device's screens, so it will use whichever one
        Windows reports first. Continue.
      </div>

      <template v-else>
        <p class="text-sm" :style="{ color: 'var(--bpm-muted)' }">
          Pick the screen the other device should see. On a PC with more than
          one monitor this is the difference between playing your game and
          watching an empty desktop.
        </p>

        <div class="space-y-2">
          <button
            v-for="d in displays"
            :key="d.deviceId"
            :ref="(el: any) => registerContent(el, { onSelect: () => pickDisplay(d) })"
            type="button"
            class="w-full text-left px-4 py-3 rounded-xl text-sm transition-colors"
            :style="{
              backgroundColor:
                chosenDisplay === d.deviceId
                  ? 'var(--bpm-accent-hex)'
                  : 'var(--bpm-surface)',
              color: chosenDisplay === d.deviceId ? 'white' : 'var(--bpm-text)',
              border: '1px solid var(--bpm-border)',
            }"
            @click="pickDisplay(d)"
          >
            <span class="block font-medium">{{ d.friendlyName }}</span>
            <span class="block text-xs opacity-70 mt-0.5">
              {{ displayDetail(d) }}
            </span>
          </button>
        </div>

        <div v-if="audioSinks.length > 0" class="pt-2 space-y-2">
          <p class="text-sm font-medium" :style="{ color: 'var(--bpm-text)' }">
            Sound
          </p>
          <p class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
            Sound normally keeps playing out of this PC while you play
            elsewhere. Sending it to the other device instead is optional.
          </p>
          <div class="grid grid-cols-2 gap-2">
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => pickVirtualSink('') })"
              type="button"
              class="px-3 py-2 rounded-lg text-sm transition-colors text-left"
              :style="{
                backgroundColor:
                  chosenVirtualSink === ''
                    ? 'var(--bpm-accent-hex)'
                    : 'var(--bpm-surface)',
                color: chosenVirtualSink === '' ? 'white' : 'var(--bpm-text)',
                border: '1px solid var(--bpm-border)',
              }"
              @click="pickVirtualSink('')"
            >
              Leave the sound here
            </button>
            <button
              v-for="s in virtualSinks"
              :key="s.name"
              :ref="(el: any) => registerContent(el, { onSelect: () => pickVirtualSink(s.name) })"
              type="button"
              class="px-3 py-2 rounded-lg text-sm transition-colors text-left"
              :style="{
                backgroundColor:
                  chosenVirtualSink === s.name
                    ? 'var(--bpm-accent-hex)'
                    : 'var(--bpm-surface)',
                color: chosenVirtualSink === s.name ? 'white' : 'var(--bpm-text)',
                border: '1px solid var(--bpm-border)',
              }"
              @click="pickVirtualSink(s.name)"
            >
              Send it to the other device
              <span class="block text-xs opacity-70 mt-0.5">{{ s.name }}</span>
            </button>
          </div>
        </div>

        <div
          v-if="pickError"
          class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {{ pickError }}
        </div>
      </template>
    </div>

    <!-- Sub-step 5: prove it actually works -->
    <div v-else class="max-w-xl space-y-4">
      <div
        v-if="testPassed"
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 10%, transparent)',
          border: '1px solid var(--bpm-accent-hex)',
        }"
      >
        <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-accent-hex)' }">
          Remote play is ready
        </p>
        <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
          On the other device, open Drop, find a game that lives on this PC and
          choose "Play on {{ hostLabel }}".
        </p>
        <!-- The check runs over loopback, so it passes on a PC nothing outside
             can reach. Anyone who came past the permissions step without the
             firewall now lands here; saying "ready" and stopping would be a
             promise this cannot keep. -->
        <p
          v-if="firewall?.supported && !firewall.allowed"
          class="text-xs mt-2 text-amber-300"
        >
          Windows is still blocking other devices, so this only works from this
          PC until that is allowed. The button for it is in Settings, under
          Remote play.
        </p>
      </div>

      <template v-else>
        <div
          class="rounded-xl p-4 text-sm"
          :style="{
            backgroundColor: 'var(--bpm-surface)',
            border: '1px solid var(--bpm-border)',
            color: 'var(--bpm-muted)',
          }"
        >
          Last step: Drop turns remote play on and checks that it answers. No
          game is launched.
        </div>

        <div
          v-if="testError"
          class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {{ testError }}
        </div>

        <button
          :ref="(el: any) => registerContent(el, { onSelect: doTest })"
          :disabled="testing"
          class="flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{ backgroundColor: 'var(--bpm-accent-hex)', color: 'white' }"
          @click="doTest"
        >
          <span
            v-if="testing"
            class="size-4 rounded-full border-2 border-white/30 border-t-white animate-spin"
          />
          {{ testing ? testStage : testError ? "Try again" : "Run the check" }}
        </button>
      </template>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useOnboarding } from "~/composables/onboarding";
import type { SunshineStatusResult } from "~/composables/useStreaming";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");
const focusNav = useFocusNavigation();
const onboarding = useOnboarding();

type SubStep = "intro" | "install" | "permissions" | "screen" | "test";
const subStep = ref<SubStep>("intro");

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

const status = ref<SunshineStatusResult | null>(null);
const installing = ref(false);
const installError = ref("");

const firewall = ref<FirewallStatus | null>(null);
const fixingFirewall = ref(false);
const firewallError = ref("");
// Whether the user has had a go at the permission. A standard Windows account
// cannot answer the administrator prompt at all, so "did it work" is the wrong
// gate for this step: it would leave those users with Skip as their only way
// out, abandoning the screen choice and the check along with it.
const firewallAttempted = ref(false);

const displays = ref<HostDisplay[]>([]);
const audioSinks = ref<HostAudioSink[]>([]);
const hostDevicesAvailable = ref(false);
const chosenDisplay = ref("");
const chosenVirtualSink = ref("");
const pickError = ref("");

const testing = ref(false);
const testStage = ref("Checking...");
const testPassed = ref(false);
const testError = ref("");
const hostLabel = ref("this PC");

// Installed, complete, and verified — the same health probe the settings page
// uses. `installed` alone still covers a half-extracted copy.
const hostReady = computed(() => status.value?.healthy === true);

// Only the endpoints that can actually take the sound off this PC's speakers.
const virtualSinks = computed(() => audioSinks.value.filter((s) => s.virtualSink));

onMounted(async () => {
  await refreshStatus();
  try {
    const settings = await invoke<Record<string, any>>("fetch_settings");
    if (typeof settings.streamingDisplay === "string") {
      chosenDisplay.value = settings.streamingDisplay;
    }
    if (typeof settings.streamingVirtualSink === "string") {
      chosenVirtualSink.value = settings.streamingVirtualSink;
    }
    if (typeof settings.deviceName === "string" && settings.deviceName) {
      hostLabel.value = settings.deviceName;
    }
  } catch {
    // Settings unavailable — the defaults are fine.
  }
});

// Re-seed focus into the newly visible sub-step, same as the other wizard
// pages: the previous sub-step's target is unmounted and the controller would
// otherwise have no ring until a D-pad press.
watch(subStep, () => {
  nextTick(() => focusNav.focusGroup("content"));
});

async function refreshStatus() {
  try {
    status.value = await invoke<SunshineStatusResult>("sunshine_status");
  } catch (e) {
    console.warn("[WIZARD] Could not read remote play status:", e);
    status.value = null;
  }
}

async function refreshFirewall() {
  try {
    firewall.value = await invoke<FirewallStatus>("sunshine_firewall_status");
  } catch (e) {
    console.warn("[WIZARD] Could not read the firewall status:", e);
    // Unknown counts as fine: blocking setup over a query that failed would be
    // worse than letting the test step find the real problem.
    firewall.value = { supported: false, configured: false, allowed: true };
  }
}

async function loadHostDevices() {
  try {
    displays.value = await invoke<HostDisplay[]>("sunshine_list_displays");
    hostDevicesAvailable.value = displays.value.length > 0;
  } catch (e) {
    console.warn("[WIZARD] Could not list screens:", e);
    hostDevicesAvailable.value = false;
    return;
  }
  try {
    audioSinks.value = await invoke<HostAudioSink[]>("sunshine_list_audio_sinks");
  } catch (e) {
    console.warn("[WIZARD] Could not list audio devices:", e);
  }
}

function displayDetail(d: HostDisplay): string {
  const bits: string[] = [];
  if (d.resolution) bits.push(d.resolution);
  if (d.adapter) bits.push(d.adapter);
  if (d.primary) bits.push("main display");
  return bits.join(" · ");
}

const installLabel = computed(() => {
  if (installing.value) {
    return status.value?.installed ? "Repairing..." : "Setting up...";
  }
  if (installError.value) return "Try again";
  return status.value?.installed ? "Repair remote play" : "Set up remote play";
});

const title = computed(() => {
  switch (subStep.value) {
    case "intro":
      return "Play on another device";
    case "install":
      return "Set up remote play";
    case "permissions":
      return "Let other devices in";
    case "screen":
      return "Pick the screen to send";
    default:
      return "Check that it works";
  }
});

const subtitle = computed(() => {
  switch (subStep.value) {
    case "intro":
      return "Your games run here and appear on the device in your hands.";
    case "install":
      return "A one-time download, then Drop checks the files are all there.";
    case "permissions":
      return "Windows has to be told that other devices on your network may connect.";
    case "screen":
      return "Which monitor the other device sees, and where the sound goes.";
    default:
      return "Drop turns it on and makes sure it answers.";
  }
});

const nextLabel = computed(() => {
  if (subStep.value === "intro") return "Set it up";
  if (subStep.value === "test") return "Done";
  return "Continue";
});

const nextDisabled = computed(() => {
  switch (subStep.value) {
    case "intro":
      return false;
    case "install":
      // Nothing past here works without the files, so the check is the gate.
      return !hostReady.value || installing.value;
    case "permissions":
      if (fixingFirewall.value) return true;
      if (!firewall.value) return true;
      if (!firewall.value.supported || firewall.value.allowed) return false;
      // Still blocked, but they have tried. Carrying on is worth more than the
      // firewall: the screen picker two steps along is the whole reason this
      // page exists, and Settings > Remote play has the same button waiting.
      return !firewallAttempted.value;
    case "screen":
      // A blank display is exactly the misconfiguration this step exists to
      // prevent, so it does not count as a choice.
      return hostDevicesAvailable.value && !chosenDisplay.value;
    default:
      return !testPassed.value || testing.value;
  }
});

async function doInstall() {
  installing.value = true;
  installError.value = "";
  try {
    // A damaged copy needs the wipe-and-replace path; a missing one doesn't.
    if (status.value?.installed) {
      await invoke<string>("repair_sunshine");
    } else {
      await invoke<string>("install_sunshine");
    }
    await refreshStatus();
    if (!hostReady.value) {
      installError.value =
        "The files were downloaded but the check still fails. Try again, or set it up from Settings.";
    }
  } catch (e) {
    installError.value = typeof e === "string" ? e : String((e as any)?.message ?? e);
  } finally {
    installing.value = false;
  }
}

async function doFirewall() {
  fixingFirewall.value = true;
  firewallError.value = "";
  try {
    await invoke("sunshine_configure_firewall");
  } catch (e) {
    firewallError.value = typeof e === "string" ? e : String((e as any)?.message ?? e);
  } finally {
    // Trust the re-query, not the call: a declined prompt and a silent no-op
    // look the same from here.
    await refreshFirewall();
    fixingFirewall.value = false;
    firewallAttempted.value = true;
    if (!firewallError.value && firewall.value?.supported && !firewall.value.allowed) {
      firewallError.value = "Windows still is not letting other devices through.";
    }
  }
}

async function pickDisplay(d: HostDisplay) {
  chosenDisplay.value = d.deviceId;
  pickError.value = "";
  try {
    // The adapter travels with the display: pairing a screen with the wrong
    // GPU makes capture fail outright.
    await invoke("update_settings", {
      newSettings: {
        streamingDisplay: d.deviceId,
        streamingAdapter: d.adapter ?? "",
      },
    });
  } catch (e) {
    console.warn("[WIZARD] Could not save the screen choice:", e);
    chosenDisplay.value = "";
    pickError.value = "Could not save that choice. Try again.";
  }
}

async function pickVirtualSink(name: string) {
  chosenVirtualSink.value = name;
  try {
    await invoke("update_settings", {
      newSettings: { streamingVirtualSink: name },
    });
  } catch (e) {
    console.warn("[WIZARD] Could not save the sound choice:", e);
    pickError.value = "Could not save the sound choice. Try again.";
  }
}

async function doTest() {
  testing.value = true;
  testError.value = "";
  testPassed.value = false;
  try {
    testStage.value = "Turning it on...";
    await invoke<string>("start_sunshine");

    testStage.value = "Checking it answers...";
    await refreshStatus();
    if (!status.value?.running) {
      testError.value = "Remote play started but stopped again straight away.";
      return;
    }

    // An open port only proves something is listening. The paired-device call
    // is authenticated, so it also proves Drop can talk to it.
    testStage.value = "Signing in...";
    await invoke<{ count: number }>("sunshine_list_clients");
    // Re-read rather than trust what the permissions step saw: the user may
    // have allowed it in the meantime, and the panel below reports on it.
    await refreshFirewall();
    testPassed.value = true;
  } catch (e) {
    testError.value = typeof e === "string" ? e : String((e as any)?.message ?? e);
  } finally {
    testing.value = false;
    testStage.value = "Checking...";
  }
}

async function handleNext() {
  if (subStep.value === "intro") {
    subStep.value = "install";
    await refreshStatus();
    return;
  }
  if (subStep.value === "install") {
    subStep.value = "permissions";
    await refreshFirewall();
    return;
  }
  if (subStep.value === "permissions") {
    subStep.value = "screen";
    await loadHostDevices();
    return;
  }
  if (subStep.value === "screen") {
    subStep.value = "test";
    return;
  }
  onboarding.markStepSeen("streaming");
  navigateTo(onboarding.nextRoute("streaming"));
}

function handleSkip() {
  // Skip the whole step, like every other wizard page.
  onboarding.markStepSeen("streaming");
  navigateTo(onboarding.nextRoute("streaming"));
}
</script>
