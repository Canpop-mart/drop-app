<template>
  <div class="space-y-4">
    <div>
      <p class="text-sm font-medium text-zinc-100 mb-1">Install location</p>
      <p
        v-if="installDir"
        class="text-sm text-zinc-300 break-all font-mono bg-zinc-800/60 rounded px-3 py-2"
      >
        {{ installDir }}
      </p>
      <p v-else class="text-sm text-zinc-500">This game isn't installed.</p>
    </div>

    <button
      v-if="installDir && gameId"
      type="button"
      class="inline-flex items-center gap-2 rounded-md bg-zinc-800 px-3 py-2 text-sm font-medium text-zinc-100 ring-1 ring-inset ring-zinc-700 hover:bg-zinc-700"
      @click="openFolder"
    >
      <FolderOpenIcon class="size-5 text-amber-400" />
      Open folder
    </button>
  </div>
</template>

<script setup lang="ts">
/**
 * "Storage" tab of the Configure modal. Shows where the game is installed and
 * opens that folder in the OS file manager. Reads no config (inheritAttrs:false
 * so the modal's shared v-model / protonEnabled don't leave stray attrs on it).
 */
import { invoke } from "@tauri-apps/api/core";
import { FolderOpenIcon } from "@heroicons/vue/24/solid";

defineOptions({ inheritAttrs: false });

const props = defineProps<{ gameId?: string; installDir?: string | null }>();

async function openFolder() {
  if (!props.gameId) return;
  try {
    await invoke("open_game_install_dir", { gameId: props.gameId });
  } catch (e) {
    console.error("[storage] open folder failed:", e);
  }
}
</script>
