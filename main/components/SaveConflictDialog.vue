<template>
  <ModalTemplate v-model="open" size-class="max-w-2xl">
    <div>
      <div class="flex items-center gap-3 mb-1">
        <ExclamationTriangleIcon class="size-6 text-amber-400 shrink-0" />
        <h3 class="text-base font-semibold text-zinc-100">
          Cloud Save Conflict
        </h3>
      </div>
      <p class="text-sm text-zinc-400 mt-1">
        Some saves were modified both locally and in the cloud. Choose which
        version to keep for each file.
      </p>
    </div>

    <!-- Conflict list -->
    <div class="space-y-3 mt-4 max-h-80 overflow-y-auto pr-1">
      <div
        v-for="(conflict, i) in conflicts"
        :key="conflict.filename"
        class="rounded-lg border border-zinc-700/50 bg-zinc-800/50 p-4"
      >
        <div class="flex items-center justify-between mb-3">
          <span class="text-sm font-medium text-zinc-200 truncate">
            {{ conflict.filename }}
          </span>
          <span
            class="text-xs px-2 py-0.5 rounded-full"
            :class="
              conflict.saveType === 'save'
                ? 'bg-blue-500/20 text-blue-300'
                : conflict.saveType === 'state'
                  ? 'bg-purple-500/20 text-purple-300'
                  : 'bg-green-500/20 text-green-300'
            "
          >
            {{ conflict.saveType }}
          </span>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <!-- Local option -->
          <button
            class="rounded-lg border p-3 text-left transition-all"
            :class="
              choices[i] === 'keep_local'
                ? 'border-blue-500 bg-blue-500/10 ring-1 ring-blue-500/50'
                : 'border-zinc-700 bg-zinc-800 hover:border-zinc-500'
            "
            @click="choices[i] = 'keep_local'"
          >
            <div class="flex items-center gap-2 mb-2">
              <ComputerDesktopIcon class="size-4 text-blue-400" />
              <span class="text-sm font-medium text-zinc-200">This PC</span>
            </div>
            <div class="space-y-1 text-xs text-zinc-400">
              <div>{{ formatSize(conflict.localSize) }}</div>
              <div>{{ formatDate(conflict.localModifiedAt * 1000) }}</div>
            </div>
          </button>

          <!-- Cloud option -->
          <button
            class="rounded-lg border p-3 text-left transition-all"
            :class="
              choices[i] === 'keep_cloud'
                ? 'border-blue-500 bg-blue-500/10 ring-1 ring-blue-500/50'
                : 'border-zinc-700 bg-zinc-800 hover:border-zinc-500'
            "
            @click="choices[i] = 'keep_cloud'"
          >
            <div class="flex items-center gap-2 mb-2">
              <CloudIcon class="size-4 text-cyan-400" />
              <span class="text-sm font-medium text-zinc-200">Cloud</span>
            </div>
            <div class="space-y-1 text-xs text-zinc-400">
              <div>{{ formatSize(conflict.cloudSize) }}</div>
              <div>{{ formatDate(conflict.cloudModifiedAt) }}</div>
              <div v-if="conflict.cloudUploadedFrom" class="text-zinc-500">
                from {{ conflict.cloudUploadedFrom }}
              </div>
            </div>
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="submitError"
      class="mt-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300"
    >
      {{ submitError }}
    </div>

    <template #buttons="{ close }">
      <button
        class="inline-flex justify-center rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed"
        :disabled="!allResolved || submitting"
        @click="submit"
      >
        {{ submitting ? "Applying…" : "Continue Launch" }}
      </button>
      <button
        class="inline-flex justify-center rounded-md bg-zinc-700 px-4 py-2 text-sm font-semibold text-zinc-200 shadow-sm hover:bg-zinc-600 disabled:opacity-40 disabled:cursor-not-allowed"
        :disabled="submitting"
        @click="keepAllLocal"
      >
        Keep All Local
      </button>
    </template>
  </ModalTemplate>
</template>

<script setup lang="ts">
import {
  ExclamationTriangleIcon,
  ComputerDesktopIcon,
  CloudIcon,
} from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import type { SaveConflict } from "~/types/save-sync";

const props = defineProps<{
  gameId: string;
  conflicts: SaveConflict[];
}>();

const open = defineModel<boolean>();

const choices = ref<string[]>([]);
const submitting = ref(false);
const submitError = ref<string | null>(null);

watch(
  () => props.conflicts,
  (val) => {
    // Default all to keep_local
    choices.value = val.map(() => "keep_local");
    submitError.value = null;
  },
  { immediate: true },
);

// Clear the error whenever the user changes a selection so stale text
// doesn't hang around after they've picked a different choice.
watch(choices, () => {
  if (submitError.value) submitError.value = null;
}, { deep: true });

const allResolved = computed(() =>
  choices.value.every((c) => c === "keep_local" || c === "keep_cloud"),
);

function keepAllLocal() {
  choices.value = props.conflicts.map(() => "keep_local");
  submit();
}

async function submit() {
  if (submitting.value) return;
  submitting.value = true;
  submitError.value = null;
  const resolutions = props.conflicts.map((c, i) => ({
    filename: c.filename,
    choice: choices.value[i],
  }));

  try {
    await invoke("resolve_save_conflicts", {
      payload: {
        gameId: props.gameId,
        resolutions,
      },
    });
    open.value = false;
  } catch (e) {
    console.warn("[SAVE-SYNC] Failed to send conflict resolutions:", e);
    submitError.value =
      e instanceof Error
        ? `Couldn't apply choices: ${e.message}`
        : `Couldn't apply choices: ${String(e)}`;
    // Deliberately leave the dialog open — losing the user's conflict
    // resolution choices on a transient failure would be worse than asking
    // them to retry.
  } finally {
    submitting.value = false;
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(input: string | number): string {
  try {
    const d = new Date(input);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return String(input);
  }
}
</script>
