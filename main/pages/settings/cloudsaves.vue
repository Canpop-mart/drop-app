<template>
  <div>
    <div class="border-b border-zinc-700 py-5">
      <h3 class="text-base font-semibold font-display leading-6 text-zinc-100">
        Cloud Saves
      </h3>
      <p class="mt-1 text-sm text-zinc-400 max-w-xl">
        Configure how Drop syncs save files with your server and identifies
        this machine in conflict prompts.
      </p>
    </div>

    <div class="mt-5 space-y-10 max-w-xl">
      <!-- Sync toggle -->
      <div class="flex flex-row items-center justify-between">
        <div class="pr-4">
          <h3 class="text-sm font-medium leading-6 text-zinc-100">
            Sync saves to the cloud
          </h3>
          <p class="mt-1 text-sm leading-6 text-zinc-400">
            When enabled, Drop uploads and restores save files between devices
            using your Drop server. Disable to keep all saves local.
          </p>
        </div>
        <Switch
          v-model="cloudSavesEnabled"
          :class="[
            cloudSavesEnabled ? 'bg-blue-600' : 'bg-zinc-700',
            'relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out',
          ]"
        >
          <span
            :class="[
              cloudSavesEnabled ? 'translate-x-5' : 'translate-x-0',
              'pointer-events-none relative inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
            ]"
          />
        </Switch>
      </div>

      <!-- Device name -->
      <div>
        <label for="deviceName" class="block text-sm font-medium text-zinc-100">
          This device's name
        </label>
        <div class="mt-2">
          <input
            id="deviceName"
            v-model="deviceName"
            type="text"
            maxlength="64"
            :placeholder="hostnamePlaceholder"
            class="block w-full rounded-md border-0 py-1.5 text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 bg-zinc-800 placeholder:text-zinc-500 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:text-sm sm:leading-6"
          />
        </div>
        <p class="mt-2 text-sm text-zinc-400">
          Shown in the cloud save conflict dialog as the machine a save was
          uploaded from. Leave blank to use your computer's name.
        </p>
      </div>

      <!-- Save button -->
      <div>
        <button
          type="button"
          :disabled="saveState.loading"
          :class="[
            'inline-flex items-center rounded-md px-3 py-2 text-sm font-semibold text-white shadow-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 transition-colors duration-300',
            saveState.success
              ? 'bg-green-600 hover:bg-green-500 focus-visible:outline-green-600'
              : 'bg-blue-600 hover:bg-blue-500 focus-visible:outline-blue-600',
            'disabled:bg-blue-600/50 disabled:cursor-not-allowed',
          ]"
          @click="saveSettings"
        >
          {{ saveState.success ? "Saved" : "Save Changes" }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Switch } from "@headlessui/vue";
import { invoke } from "@tauri-apps/api/core";
import { hostname } from "@tauri-apps/plugin-os";
import { type Settings } from "~/types";

// Cloud saves is dev-gated. The settings nav hides the link outside dev mode;
// this guards a direct URL visit too.
definePageMeta({
  middleware() {
    const devMode = useDevMode();
    if (!devMode.enabled.value) return navigateTo("/settings");
  },
});

const settings = await invoke<Settings>("fetch_settings");

const cloudSavesEnabled = ref<boolean>(settings.cloudSavesEnabled ?? true);
// Treat null / undefined / "" all as "use default" — show as empty input,
// the backend will fall back to the OS hostname.
const deviceName = ref<string>(settings.deviceName ?? "");

// Try to read the OS hostname for the placeholder so the user knows what
// they'd inherit by leaving the field blank. Falls back to a generic
// hint if the plugin call fails (some sandboxes / older platforms).
const hostnamePlaceholder = ref<string>("Auto-detected");
onMounted(async () => {
  try {
    const h = await hostname();
    if (h && h.trim().length > 0) {
      hostnamePlaceholder.value = h;
    }
  } catch {
    // Keep "Auto-detected" placeholder.
  }
});

const saveState = reactive({
  loading: false,
  success: false,
});

async function saveSettings() {
  try {
    saveState.loading = true;
    const trimmed = deviceName.value.trim();
    await invoke("update_settings", {
      newSettings: {
        cloudSavesEnabled: cloudSavesEnabled.value,
        // Send null when blank so the backend stores `None` and falls
        // back to the OS hostname in the conflict UI.
        deviceName: trimmed.length === 0 ? null : trimmed,
      },
    });
    saveState.success = true;
    setTimeout(() => {
      saveState.success = false;
    }, 2000);
  } catch (error) {
    console.error("Failed to save cloud save settings:", error);
  } finally {
    saveState.loading = false;
  }
}

// Persist the moment the sync toggle flips. The Switch reads as an instant
// control, but previously only the "Save Changes" button called
// update_settings — so toggling sync OFF appeared to take effect when it
// hadn't (the launch path kept reading the old value). Auto-saving on change
// closes that trap; the button still covers the device-name field.
watch(cloudSavesEnabled, () => {
  saveSettings();
});
</script>
