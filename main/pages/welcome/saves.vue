<template>
  <BpmWizardShell
    step-key="saves"
    :title="title"
    :subtitle="subtitle"
    manual-advance
    :next-disabled="nextDisabled"
    :next-label="nextLabel"
    @next="handleNext"
    @skip="handleSkip"
  >
    <!-- Sub-step 1: what it does, and where the bytes go. -->
    <div v-if="subStep === 'what'" class="max-w-2xl space-y-5">
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
          When you finish playing, Drop copies that game's save files to
          <strong>your own Drop server</strong>. Next time you play, on this
          computer or another one, it puts them back.
        </p>
        <p class="text-sm mt-3" :style="{ color: 'var(--bpm-muted)' }">
          Nothing goes to Drop, to a game publisher, or to any other service.
          The only machine your saves reach is the server you already sign in
          to.
        </p>
      </div>

      <!-- Where a save travels. Four steps, same shape as the RetroAchievements
           step, because the question people actually have is "who sees this". -->
      <div class="flex items-center justify-between gap-3 py-2">
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{
              backgroundColor: 'var(--bpm-surface)',
              border: '1px solid var(--bpm-border)',
            }"
          >
            <span class="text-lg">🎮</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">
            You stop playing
          </p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{
              backgroundColor: 'var(--bpm-surface)',
              border: '1px solid var(--bpm-border)',
            }"
          >
            <span class="text-lg">💾</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">
            Drop finds the save
          </p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{
              backgroundColor:
                'color-mix(in srgb, var(--bpm-accent-hex) 20%, transparent)',
            }"
          >
            <span class="text-lg">🏠</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-accent-hex)' }">
            Your Drop server
          </p>
        </div>
        <div :style="{ color: 'var(--bpm-muted)' }">→</div>
        <div class="flex-1 text-center">
          <div
            class="size-12 mx-auto rounded-lg flex items-center justify-center"
            :style="{
              backgroundColor: 'var(--bpm-surface)',
              border: '1px solid var(--bpm-border)',
            }"
          >
            <span class="text-lg">🖥️</span>
          </div>
          <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">
            Your next device
          </p>
        </div>
      </div>

      <!-- Who can see a save. This is the model, not a setting, and someone
           finding a housemate's progress in their library should have been
           told here first. -->
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-text)' }">
          Who your saves are shared with
        </p>
        <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">
          Emulator saves are backed up to your account alone.
        </p>
        <p class="text-xs mt-1.5" :style="{ color: 'var(--bpm-muted)' }">
          PC game saves are shared with everyone on your Drop server. Drop
          finds them by where the game puts them on this computer, not by who
          is signed in, so one PC save is one save for the whole server.
        </p>
        <p class="text-xs mt-1.5" :style="{ color: 'var(--bpm-muted)' }">
          Switch games are shared on this computer too. Their saves live inside
          the emulator's own system storage, which every account here uses.
        </p>
      </div>
    </div>

    <!-- Sub-step 2: Ludusavi, and what happens if you skip it. -->
    <div v-else-if="subStep === 'ludusavi'" class="max-w-2xl space-y-5">
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: installed
            ? '1px solid var(--bpm-accent-hex)'
            : '1px solid var(--bpm-border)',
        }"
      >
        <div class="flex items-start gap-4">
          <div
            class="size-10 rounded-lg flex items-center justify-center shrink-0"
            :style="{
              backgroundColor: installed
                ? 'color-mix(in srgb, var(--bpm-accent-hex) 20%, transparent)'
                : 'color-mix(in srgb, var(--bpm-muted) 15%, transparent)',
              color: installed ? 'var(--bpm-accent-hex)' : 'var(--bpm-muted)',
            }"
          >
            <svg
              v-if="installed"
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 20 20"
              fill="currentColor"
              class="size-5"
            >
              <path
                fill-rule="evenodd"
                d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z"
                clip-rule="evenodd"
              />
            </svg>
            <svg
              v-else
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 20 20"
              fill="currentColor"
              class="size-5"
            >
              <path
                d="M10 3a1.5 1.5 0 0 1 1.5 1.5V6h3.25A1.75 1.75 0 0 1 16.5 7.75v.5a1.5 1.5 0 1 1-3 0v-.25h-3v6.5h.25a1.5 1.5 0 1 1 0 3h-3.5a1.5 1.5 0 1 1 0-3h.25v-6.5h-3v.25a1.5 1.5 0 1 1-3 0v-.5A1.75 1.75 0 0 1 3.25 6H6.5V4.5A1.5 1.5 0 0 1 8 3h2Z"
              />
            </svg>
          </div>
          <div class="flex-1 min-w-0">
            <p
              class="text-sm font-semibold"
              :style="{ color: 'var(--bpm-text)' }"
            >
              {{ installed ? "Ludusavi is installed" : "Ludusavi is not installed" }}
            </p>
            <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
              {{
                installed
                  ? "Drop can find where your PC games keep their saves."
                  : "Drop has no way to find where your PC games keep their saves."
              }}
            </p>
          </div>
        </div>
      </div>

      <p class="text-sm" :style="{ color: 'var(--bpm-muted)' }">
        Ludusavi is a free, open-source tool that knows where thousands of PC
        games store their save files. Drop asks it, then backs up what it
        points at. It is a separate program and Drop does not bundle it.
      </p>

      <!-- The consequence, said plainly rather than implied. -->
      <div
        v-if="!installed"
        class="rounded-xl px-4 py-3 text-sm"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-muted) 12%, transparent)',
          color: 'var(--bpm-text)',
        }"
      >
        <strong>If you skip this:</strong> PC game saves will not be backed up.
        Emulator saves still will. You can install Ludusavi later from any
        game's Cloud Saves panel.
      </div>

      <div v-if="!installed">
        <button
          :ref="(el: any) => registerContent(el, { onSelect: doInstall })"
          :disabled="installing"
          class="flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{ backgroundColor: 'var(--bpm-accent-hex)', color: 'white' }"
          @click="doInstall"
        >
          <svg
            v-if="!installing"
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 20 20"
            fill="currentColor"
            class="size-4"
          >
            <path
              d="M10.75 2.75a.75.75 0 0 0-1.5 0v8.614L6.295 8.235a.75.75 0 1 0-1.09 1.03l4.25 4.5a.75.75 0 0 0 1.09 0l4.25-4.5a.75.75 0 0 0-1.09-1.03l-2.955 3.129V2.75Z"
            />
            <path
              d="M3.5 12.75a.75.75 0 0 0-1.5 0v2.5A2.75 2.75 0 0 0 4.75 18h10.5A2.75 2.75 0 0 0 18 15.25v-2.5a.75.75 0 0 0-1.5 0v2.5c0 .69-.56 1.25-1.25 1.25H4.75c-.69 0-1.25-.56-1.25-1.25v-2.5Z"
            />
          </svg>
          <svg
            v-else
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            class="size-4 animate-spin"
            stroke="currentColor"
            stroke-width="3"
          >
            <circle cx="12" cy="12" r="10" stroke-opacity="0.25" />
            <path d="M22 12a10 10 0 0 1-10 10" />
          </svg>
          {{ installing ? "Installing…" : "Install Ludusavi" }}
        </button>
        <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">
          Downloads about 15 MB from the official Ludusavi GitHub releases.
        </p>
      </div>

      <div
        v-if="installResult && !installError"
        class="rounded-xl px-4 py-3 text-sm"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 10%, transparent)',
          color: 'var(--bpm-accent-hex)',
        }"
      >
        Installed to
        <span class="font-mono text-xs">{{ installResult }}</span>
      </div>
      <div
        v-if="installError"
        class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
      >
        {{ installError }}
      </div>
    </div>

    <!-- Sub-step 3: turn it on, and where to look afterwards. -->
    <div v-else-if="subStep === 'turnon'" class="max-w-2xl space-y-5">
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: enabled
            ? '1px solid var(--bpm-accent-hex)'
            : '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-text)' }">
          {{ enabled ? "Cloud saves are on" : "Cloud saves are off" }}
        </p>
        <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
          {{
            enabled
              ? "Drop will sync saves before a game starts and after it closes."
              : "Drop will not touch your save files until you turn this on."
          }}
        </p>

        <button
          :ref="(el: any) => registerContent(el, { onSelect: toggleEnabled })"
          :disabled="saving"
          class="mt-4 flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{
            backgroundColor: enabled
              ? 'var(--bpm-surface)'
              : 'var(--bpm-accent-hex)',
            color: enabled ? 'var(--bpm-text)' : 'white',
            border: enabled ? '1px solid var(--bpm-border)' : 'none',
          }"
          @click="toggleEnabled"
        >
          {{ saveLabel }}
        </button>

        <p
          v-if="saveError"
          class="mt-3 text-sm text-red-300"
        >
          {{ saveError }}
        </p>
      </div>

      <!-- What "on" actually covers, so nobody reads it as "every game". -->
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-text)' }">
          What Drop can back up
        </p>
        <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">
          PC games that Ludusavi's list covers, RetroArch games, and Switch
          emulator saves. Other emulators keep their saves in places Drop does
          not know about, and a game's Cloud Saves panel says so when that is
          the case.
        </p>
      </div>

      <div
        class="rounded-xl p-4 text-sm"
        :style="{
          backgroundColor: 'color-mix(in srgb, var(--bpm-muted) 10%, transparent)',
          color: 'var(--bpm-muted)',
        }"
      >
        To see what has been backed up, open
        <strong>Settings, then Cloud Saves</strong>. It lists every game Drop
        is holding saves for, how much space they use, and anything that failed
        to sync.
      </div>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
/**
 * Cloud saves onboarding.
 *
 * Three sub-steps: what the feature does and where the bytes go, the Ludusavi
 * install with the consequence of skipping it, and turning the feature on.
 *
 * Every advance re-reads the real state rather than trusting what this page
 * did a moment ago. The old version of this step installed Ludusavi and
 * nothing else — it never enabled the setting, so people finished the wizard
 * believing cloud saves were set up when the master switch was still off.
 */
import { ref, computed, onMounted, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useOnboarding } from "~/composables/onboarding";
import type { Settings } from "~/types";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");
const focusNav = useFocusNavigation();
const onboarding = useOnboarding();

type SubStep = "what" | "ludusavi" | "turnon";
const subStep = ref<SubStep>("what");

const installed = ref(false);
const installing = ref(false);
const installResult = ref("");
const installError = ref("");

const enabled = ref(false);
const saving = ref(false);
const saveError = ref("");

onMounted(async () => {
  installed.value = await readLudusaviInstalled();
  enabled.value = await readCloudSavesEnabled();
});

// Re-seed focus into the newly visible sub-step, same as the
// RetroAchievements step: the previous target is unmounted and the controller
// would otherwise have no ring until the user presses a direction.
watch(subStep, () => {
  nextTick(() => focusNav.focusGroup("content"));
});

const title = computed(() => {
  if (subStep.value === "what") return "Cloud saves";
  if (subStep.value === "ludusavi") return "Finding your PC game saves";
  return "Turn cloud saves on";
});

const subtitle = computed(() => {
  if (subStep.value === "what")
    return "Your save files, kept on your own Drop server, so a second computer picks up where the first left off.";
  if (subStep.value === "ludusavi")
    return "PC games all store saves somewhere different. Drop uses Ludusavi to know where.";
  // A second run of the wizard must not tell someone the feature is off when
  // they already turned it on.
  return enabled.value
    ? "Already on. You can turn it back off here or in Settings."
    : "Nothing is synced until you turn this on.";
});

const nextLabel = computed(() => {
  if (subStep.value === "what") return "Next";
  if (subStep.value === "ludusavi")
    return installed.value ? "Next" : "Continue without it";
  return "Done";
});

const nextDisabled = computed(() => installing.value || saving.value);

const saveLabel = computed(() => {
  if (saving.value) return "Saving…";
  return enabled.value ? "Turn cloud saves off" : "Turn cloud saves on";
});

/** Ask Rust, not this page's memory of what it did. */
async function readLudusaviInstalled(): Promise<boolean> {
  try {
    return await invoke<boolean>("check_ludusavi");
  } catch {
    return false;
  }
}

async function readCloudSavesEnabled(): Promise<boolean> {
  try {
    const settings = await invoke<Settings>("fetch_settings");
    return settings.cloudSavesEnabled === true;
  } catch {
    return false;
  }
}

async function doInstall() {
  installError.value = "";
  installResult.value = "";
  installing.value = true;
  try {
    installResult.value = await invoke<string>("install_ludusavi");
    installed.value = await readLudusaviInstalled();
    if (!installed.value) {
      installError.value =
        "The download finished but Drop still cannot run Ludusavi. PC game saves will not be backed up.";
    }
  } catch (e: any) {
    installError.value = typeof e === "string" ? e : String(e?.message ?? e);
  } finally {
    installing.value = false;
  }
}

/**
 * Write the setting, then read it back. A toggle that flips in the UI and
 * silently fails to persist is the exact way someone ends up thinking their
 * saves are covered when they are not.
 */
async function toggleEnabled() {
  if (saving.value) return;
  const target = !enabled.value;
  saving.value = true;
  saveError.value = "";
  try {
    await invoke("update_settings", {
      newSettings: { cloudSavesEnabled: target },
    });
    enabled.value = await readCloudSavesEnabled();
    if (enabled.value !== target) {
      saveError.value = target
        ? "Drop could not turn cloud saves on. Try again from Settings, under Cloud Saves."
        : "Drop could not turn cloud saves off. Try again from Settings, under Cloud Saves.";
    }
  } catch (e: any) {
    saveError.value = typeof e === "string" ? e : String(e?.message ?? e);
    enabled.value = await readCloudSavesEnabled();
  } finally {
    saving.value = false;
  }
}

async function handleNext() {
  if (subStep.value === "what") {
    // Re-check on the way in: Ludusavi may already be here from a previous
    // run of the wizard, and offering to install it again would be nonsense.
    installed.value = await readLudusaviInstalled();
    subStep.value = "ludusavi";
    return;
  }
  if (subStep.value === "ludusavi") {
    installed.value = await readLudusaviInstalled();
    enabled.value = await readCloudSavesEnabled();
    subStep.value = "turnon";
    return;
  }
  finish();
}

function handleSkip() {
  finish();
}

function finish() {
  onboarding.markStepSeen("saves");
  navigateTo(onboarding.nextRoute("saves"));
}
</script>
