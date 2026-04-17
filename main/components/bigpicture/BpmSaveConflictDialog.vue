<template>
  <BigPictureDialog
    :visible="visible"
    title="Cloud Save Conflict"
    confirm-label="Continue"
    cancel-label="Keep All Local"
    :show-cancel="true"
    @confirm="submit"
    @cancel="keepAllLocal"
  >
    <p class="text-zinc-400 text-sm mb-4">
      Some saves were modified both locally and in the cloud.
    </p>

    <div class="space-y-3 max-h-72 overflow-y-auto pr-1">
      <div
        v-for="(conflict, i) in conflicts"
        :key="conflict.filename"
        class="rounded-xl border border-zinc-700/50 bg-zinc-800/40 p-4"
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
          <!-- Local -->
          <button
            class="rounded-xl border p-3 text-left transition-all focus:outline-none"
            :class="
              choices[i] === 'keep_local'
                ? 'border-blue-500 bg-blue-500/10 ring-2 ring-blue-500/50'
                : 'border-zinc-700 bg-zinc-800 hover:border-zinc-500'
            "
            @click="choices[i] = 'keep_local'"
          >
            <div class="text-sm font-medium text-zinc-200 mb-1">This PC</div>
            <div class="text-xs text-zinc-400">
              {{ formatSize(conflict.localSize) }} &middot;
              {{ formatDate(conflict.localModifiedAt * 1000) }}
            </div>
          </button>

          <!-- Cloud -->
          <button
            class="rounded-xl border p-3 text-left transition-all focus:outline-none"
            :class="
              choices[i] === 'keep_cloud'
                ? 'border-blue-500 bg-blue-500/10 ring-2 ring-blue-500/50'
                : 'border-zinc-700 bg-zinc-800 hover:border-zinc-500'
            "
            @click="choices[i] = 'keep_cloud'"
          >
            <div class="text-sm font-medium text-zinc-200 mb-1">Cloud</div>
            <div class="text-xs text-zinc-400">
              {{ formatSize(conflict.cloudSize) }} &middot;
              {{ formatDate(conflict.cloudModifiedAt) }}
            </div>
            <div
              v-if="conflict.cloudUploadedFrom"
              class="text-xs text-zinc-500 mt-0.5"
            >
              from {{ conflict.cloudUploadedFrom }}
            </div>
          </button>
        </div>
      </div>
    </div>
  </BigPictureDialog>
</template>

<script setup lang="ts">
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
import { invoke } from "@tauri-apps/api/core";
import type { SaveConflict } from "~/types/save-sync";

const props = defineProps<{
  visible: boolean;
  gameId: string;
  conflicts: SaveConflict[];
}>();

const emit = defineEmits<{
  resolved: [];
}>();

const choices = ref<string[]>([]);

watch(
  () => props.conflicts,
  (val) => {
    choices.value = val.map(() => "keep_local");
  },
  { immediate: true },
);

function keepAllLocal() {
  choices.value = props.conflicts.map(() => "keep_local");
  submit();
}

async function submit() {
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
  } catch (e) {
    console.warn("[SAVE-SYNC] Failed to send conflict resolutions:", e);
  }

  emit("resolved");
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
