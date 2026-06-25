<template>
  <BpmWizardShell
    step-key="storage"
    title="Where should games install?"
    subtitle="Pick a folder with plenty of free space. On Steam Deck you can point this at an SD card."
    :next-disabled="loading || installDirs.length === 0"
    :next-label="installDirs.length === 0 ? 'Add a folder first' : 'Continue'"
  >
    <div class="max-w-2xl space-y-5">
      <!-- Current install dirs -->
      <div>
        <p class="text-xs uppercase tracking-wide font-medium mb-2" :style="{ color: 'var(--bpm-muted)' }">
          Configured folders
        </p>
        <div v-if="installDirs.length > 0" class="space-y-2">
          <div
            v-for="dir in installDirs"
            :key="dir"
            class="flex items-center gap-3 rounded-xl px-4 py-3"
            :style="{
              backgroundColor: 'var(--bpm-surface)',
              border: '1px solid var(--bpm-border)',
            }"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5 shrink-0" :style="{ color: 'var(--bpm-accent-hex)' }">
              <path d="M3.75 3A1.75 1.75 0 0 0 2 4.75v3.26a3.235 3.235 0 0 1 1.75-.51h12.5c.644 0 1.245.188 1.75.51V6.75A1.75 1.75 0 0 0 16.25 5h-4.836a.25.25 0 0 1-.177-.073L9.823 3.513A1.75 1.75 0 0 0 8.586 3H3.75ZM3.75 9A1.75 1.75 0 0 0 2 10.75v4.5c0 .966.784 1.75 1.75 1.75h12.5A1.75 1.75 0 0 0 18 15.25v-4.5A1.75 1.75 0 0 0 16.25 9H3.75Z" />
            </svg>
            <span class="text-sm truncate" :style="{ color: 'var(--bpm-text)' }">
              {{ dir }}
            </span>
          </div>
        </div>
        <p v-else class="text-sm italic" :style="{ color: 'var(--bpm-muted)' }">
          No folders configured yet.
        </p>
      </div>

      <!-- Actions -->
      <div class="flex flex-wrap gap-2">
        <button
          :ref="(el: any) => registerContent(el, { onSelect: addFolder })"
          :disabled="loading"
          class="flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50"
          :style="{
            backgroundColor: 'var(--bpm-accent-hex)',
            color: 'white',
          }"
          @click="addFolder"
        >
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-4">
            <path d="M10.75 4.75a.75.75 0 0 0-1.5 0v4.5h-4.5a.75.75 0 0 0 0 1.5h4.5v4.5a.75.75 0 0 0 1.5 0v-4.5h4.5a.75.75 0 0 0 0-1.5h-4.5v-4.5Z" />
          </svg>
          {{ loading ? "Working…" : "Add a folder" }}
        </button>
      </div>

      <!-- Detected removable storage (Linux / Deck) -->
      <div v-if="removableStorage.length > 0" class="space-y-2">
        <p class="text-xs uppercase tracking-wide font-medium" :style="{ color: 'var(--bpm-muted)' }">
          Detected drives
        </p>
        <button
          v-for="drive in removableStorage"
          :key="drive.path"
          :ref="(el: any) => registerContent(el, { onSelect: () => addFromDetected(drive.path) })"
          class="w-full flex items-center justify-between gap-3 rounded-xl px-4 py-3 text-left transition-colors"
          :style="{
            backgroundColor: 'var(--bpm-surface)',
            border: '1px solid var(--bpm-border)',
          }"
          @click="addFromDetected(drive.path)"
        >
          <div class="flex items-center gap-3 min-w-0">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5 shrink-0" :style="{ color: 'var(--bpm-muted)' }">
              <path fill-rule="evenodd" d="M5.5 17a4.5 4.5 0 0 1-1.44-8.765 4.5 4.5 0 0 1 8.302-3.046 3.5 3.5 0 0 1 4.504 4.272A4 4 0 0 1 15 17H5.5Z" clip-rule="evenodd" />
            </svg>
            <div class="min-w-0">
              <p class="text-sm font-medium truncate" :style="{ color: 'var(--bpm-text)' }">
                {{ drive.label || drive.path }}
              </p>
              <p class="text-xs truncate" :style="{ color: 'var(--bpm-muted)' }">
                {{ drive.path }}{{ drive.free_bytes ? ` · ${formatBytes(drive.free_bytes)} free` : "" }}
              </p>
            </div>
          </div>
          <span class="text-xs shrink-0" :style="{ color: 'var(--bpm-accent-hex)' }">Use</span>
        </button>
      </div>

      <div
        v-if="error"
        class="rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-300"
      >
        {{ error }}
      </div>
    </div>
  </BpmWizardShell>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import BpmWizardShell from "~/components/bigpicture/BpmWizardShell.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");

interface DetectedDrive {
  path: string;
  label?: string;
  free_bytes?: number;
}

const installDirs = ref<string[]>([]);
const removableStorage = ref<DetectedDrive[]>([]);
const loading = ref(false);
const error = ref("");

onMounted(async () => {
  await refreshInstallDirs();
  try {
    const raw = await invoke<any[]>("detect_removable_storage");
    removableStorage.value = Array.isArray(raw)
      ? raw.map((d) => ({
          path: d.path ?? d.mount ?? d,
          label: d.label ?? d.name,
          free_bytes: d.free_bytes ?? d.freeBytes,
        }))
      : [];
  } catch {
    // Not supported on this platform — silently ignore.
  }
});

async function refreshInstallDirs() {
  try {
    installDirs.value = await invoke<string[]>("fetch_download_dir_stats");
  } catch (e: any) {
    error.value = typeof e === "string" ? e : String(e?.message ?? e);
  }
}

async function addFolder() {
  error.value = "";
  loading.value = true;
  try {
    const chosen = await open({
      multiple: false,
      directory: true,
      canCreateDirectories: true,
      title: "Choose a folder for game installs",
    });
    if (!chosen || Array.isArray(chosen)) return;
    await invoke("add_download_dir", { newDir: chosen });
    await refreshInstallDirs();
  } catch (e: any) {
    error.value = typeof e === "string" ? e : String(e?.message ?? e);
  } finally {
    loading.value = false;
  }
}

async function addFromDetected(path: string) {
  error.value = "";
  loading.value = true;
  try {
    const gameDir = path.endsWith("/") ? `${path}drop-games` : `${path}/drop-games`;
    await invoke("add_download_dir", { newDir: gameDir });
    await refreshInstallDirs();
  } catch (e: any) {
    error.value = typeof e === "string" ? e : String(e?.message ?? e);
  } finally {
    loading.value = false;
  }
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes)) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let u = 0;
  let v = bytes;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[u]}`;
}
</script>
