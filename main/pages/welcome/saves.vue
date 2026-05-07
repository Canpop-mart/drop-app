<template>
  <BpmWizardShell
    step-key="saves"
    title="Cloud saves"
    subtitle="Drop uses Ludusavi to find and back up game saves. It's a separate, free tool — Drop can install it for you."
    :next-disabled="installing"
    :next-label="installed ? 'Continue' : installing ? 'Installing…' : 'Skip for now'"
  >
    <div class="max-w-2xl space-y-5">
      <!-- Status card -->
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: installed ? '1px solid var(--bpm-accent-hex)' : '1px solid var(--bpm-border)',
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
            <svg v-if="installed" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5">
              <path fill-rule="evenodd" d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z" clip-rule="evenodd" />
            </svg>
            <svg v-else xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5">
              <path d="M10 3a1.5 1.5 0 0 1 1.5 1.5V6h3.25A1.75 1.75 0 0 1 16.5 7.75v.5a1.5 1.5 0 1 1-3 0v-.25h-3v6.5h.25a1.5 1.5 0 1 1 0 3h-3.5a1.5 1.5 0 1 1 0-3h.25v-6.5h-3v.25a1.5 1.5 0 1 1-3 0v-.5A1.75 1.75 0 0 1 3.25 6H6.5V4.5A1.5 1.5 0 0 1 8 3h2Z" />
            </svg>
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-text)' }">
              {{ installed ? "Ludusavi installed" : "Ludusavi not installed" }}
            </p>
            <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
              {{
                installed
                  ? "Cloud save sync is ready to use."
                  : "Without it, Drop can still manage emulator saves, but PC save backup/restore won't work."
              }}
            </p>
          </div>
        </div>
      </div>

      <!-- Install button -->
      <div v-if="!installed">
        <button
          :ref="(el: any) => registerContent(el, { onSelect: doInstall })"
          :disabled="installing"
          class="flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{
            backgroundColor: 'var(--bpm-accent-hex)',
            color: 'white',
          }"
          @click="doInstall"
        >
          <svg v-if="!installing" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-4">
            <path d="M10.75 2.75a.75.75 0 0 0-1.5 0v8.614L6.295 8.235a.75.75 0 1 0-1.09 1.03l4.25 4.5a.75.75 0 0 0 1.09 0l4.25-4.5a.75.75 0 0 0-1.09-1.03l-2.955 3.129V2.75Z" />
            <path d="M3.5 12.75a.75.75 0 0 0-1.5 0v2.5A2.75 2.75 0 0 0 4.75 18h10.5A2.75 2.75 0 0 0 18 15.25v-2.5a.75.75 0 0 0-1.5 0v2.5c0 .69-.56 1.25-1.25 1.25H4.75c-.69 0-1.25-.56-1.25-1.25v-2.5Z" />
          </svg>
          <svg v-else xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" class="size-4 animate-spin" stroke="currentColor" stroke-width="3">
            <circle cx="12" cy="12" r="10" stroke-opacity="0.25" />
            <path d="M22 12a10 10 0 0 1-10 10" />
          </svg>
          {{ installing ? "Installing…" : "Install Ludusavi" }}
        </button>
        <p class="text-xs mt-2" :style="{ color: 'var(--bpm-muted)' }">
          Downloads ~15 MB from the official Ludusavi GitHub releases.
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
        Installed to: <span class="font-mono text-xs">{{ installResult }}</span>
      </div>
      <div
        v-if="installError"
        class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
      >
        {{ installError }}
      </div>

      <!-- What is Ludusavi? -->
      <details
        class="rounded-xl p-4 text-sm"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <summary class="cursor-pointer font-medium" :style="{ color: 'var(--bpm-text)' }">
          What is Ludusavi?
        </summary>
        <p class="mt-3" :style="{ color: 'var(--bpm-muted)' }">
          Ludusavi is an open-source tool that knows where thousands of PC games store their save
          files. Drop calls into it to back up and restore saves between devices. Your saves never
          leave your Drop server.
        </p>
      </details>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");

// The saves step is hidden from the wizard sequence when dev mode is off,
// but a direct URL hit (bookmarked link, manual navigation) could still
// land here. Bounce back to the wizard start in that case so the user
// doesn't see an unfinished page.
const devMode = useDevMode();
if (!devMode.enabled.value) {
  await navigateTo("/welcome", { replace: true });
}

const installed = ref(false);
const installing = ref(false);
const installResult = ref("");
const installError = ref("");

onMounted(async () => {
  installed.value = await refreshStatus();
});

async function refreshStatus(): Promise<boolean> {
  try {
    return await invoke<boolean>("check_ludusavi");
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
    installed.value = await refreshStatus();
  } catch (e: any) {
    installError.value = typeof e === "string" ? e : String(e?.message ?? e);
  } finally {
    installing.value = false;
  }
}
</script>
