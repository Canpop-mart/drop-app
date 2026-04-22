<template>
  <BpmWizardShell
    step-key="platform"
    :title="title"
    :subtitle="subtitle"
  >
    <!-- Linux / Steam Deck: show Proton strategy picker -->
    <div v-if="isLinux" class="max-w-2xl space-y-4">
      <!-- Auto option (recommended) -->
      <button
        :ref="(el: any) => registerContent(el, { onSelect: () => choose(null) })"
        class="w-full rounded-xl p-4 text-left transition-all"
        :style="{
          backgroundColor: selected === null ? 'color-mix(in srgb, var(--bpm-accent-hex) 15%, transparent)' : 'var(--bpm-surface)',
          border: selected === null ? '1px solid var(--bpm-accent-hex)' : '1px solid var(--bpm-border)',
        }"
        @click="choose(null)"
      >
        <div class="flex items-start justify-between gap-3">
          <div class="flex-1">
            <div class="flex items-center gap-2">
              <p class="text-sm font-semibold" :style="{ color: 'var(--bpm-text)' }">
                Auto (GE-Proton)
              </p>
              <span
                class="text-[10px] uppercase tracking-wide px-2 py-0.5 rounded-full font-medium"
                :style="{
                  backgroundColor: 'color-mix(in srgb, var(--bpm-accent-hex) 20%, transparent)',
                  color: 'var(--bpm-accent-hex)',
                }"
              >
                Recommended
              </span>
            </div>
            <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
              Drop fetches GE-Proton on first launch — the community build with the most game fixes.
              Nothing to install or manage yourself.
            </p>
          </div>
          <div
            v-if="selected === null"
            class="size-5 rounded-full shrink-0 flex items-center justify-center"
            :style="{ backgroundColor: 'var(--bpm-accent-hex)' }"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="white" class="size-3.5">
              <path fill-rule="evenodd" d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z" clip-rule="evenodd" />
            </svg>
          </div>
        </div>
      </button>

      <!-- Installed Proton builds -->
      <div v-if="protonPaths.length > 0">
        <p class="text-xs uppercase tracking-wide font-medium mt-6 mb-2" :style="{ color: 'var(--bpm-muted)' }">
          Or pin a specific build
        </p>
        <div class="space-y-2">
          <button
            v-for="proton in protonPaths"
            :key="proton.path"
            :ref="(el: any) => registerContent(el, { onSelect: () => choose(proton.path) })"
            class="w-full rounded-xl p-3 text-left transition-all"
            :style="{
              backgroundColor: selected === proton.path ? 'color-mix(in srgb, var(--bpm-accent-hex) 15%, transparent)' : 'var(--bpm-surface)',
              border: selected === proton.path ? '1px solid var(--bpm-accent-hex)' : '1px solid var(--bpm-border)',
            }"
            @click="choose(proton.path)"
          >
            <p class="text-sm font-medium truncate" :style="{ color: 'var(--bpm-text)' }">
              {{ proton.name }}
            </p>
            <p class="text-xs truncate" :style="{ color: 'var(--bpm-muted)' }">
              {{ proton.path }}
            </p>
          </button>
        </div>
      </div>
      <p v-else-if="!loading" class="text-xs" :style="{ color: 'var(--bpm-muted)' }">
        No Proton builds detected yet — Auto will fetch one when you launch a Windows game.
      </p>

      <div
        v-if="error"
        class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
      >
        {{ error }}
      </div>
    </div>

    <!-- Windows / macOS: nothing to configure -->
    <div v-else class="max-w-2xl">
      <div
        class="rounded-xl p-5"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          border: '1px solid var(--bpm-border)',
        }"
      >
        <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
          Nothing to configure on this platform — Windows games run directly. Press Next to continue.
        </p>
      </div>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");

interface ProtonPath {
  path: string;
  name: string;
}

const currentPlatform = platform();
const isLinux = currentPlatform === "linux";

const protonPaths = ref<ProtonPath[]>([]);
const selected = ref<string | null>(null);
const loading = ref(true);
const error = ref("");

const title = computed(() =>
  isLinux ? "How should Windows games run?" : "Platform defaults",
);
const subtitle = computed(() =>
  isLinux
    ? "Proton lets Drop run Windows games on Linux. Most people should keep the recommended default."
    : "Running on " + currentPlatform + " — no compatibility layer needed.",
);

onMounted(async () => {
  if (!isLinux) {
    loading.value = false;
    return;
  }
  try {
    const result = await invoke<{
      autodiscovered: ProtonPath[];
      custom: ProtonPath[];
      default: string | null;
    }>("fetch_proton_paths");
    protonPaths.value = [...result.autodiscovered, ...result.custom];
    selected.value = result.default;
  } catch (e: any) {
    error.value = typeof e === "string" ? e : String(e?.message ?? e);
  } finally {
    loading.value = false;
  }
});

async function choose(path: string | null) {
  error.value = "";
  try {
    if (path === null) {
      await invoke("clear_default_proton");
    } else {
      await invoke("set_default", { path });
    }
    selected.value = path;
  } catch (e: any) {
    error.value = typeof e === "string" ? e : String(e?.message ?? e);
  }
}
</script>
