<template>
  <ModalTemplate :model-value="open">
    <template #default>
      <div class="sm:flex sm:items-start">
        <div class="mt-3 text-center sm:mt-0 sm:text-left">
          <h3 class="text-base font-semibold text-zinc-100">
            Launch {{ gameName }}
          </h3>
          <div class="mt-2">
            <p class="text-sm text-zinc-400">
              The instance admin has configured multiple ways to start this
              game. Select an option to start.
            </p>
          </div>
        </div>
      </div>

      <ol class="space-y-2">
        <li v-for="(launchData, launchIdx) in options" :key="launchIdx">
          <button
            class="transition w-full rounded-sm bg-zinc-800 inline-flex items-center text-sm py-2 px-3 gap-x-2 text-zinc-100 hover:text-zinc-300 hover:bg-zinc-700"
            @click="$emit('select', launchIdx)"
          >
            <PlayIcon class="size-4" />
            <span>{{ launchData.name }}</span>
          </button>
        </li>
      </ol>
    </template>
    <template #buttons>
      <button
        type="button"
        class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
        @click="$emit('cancel')"
      >
        Cancel
      </button>
    </template>
  </ModalTemplate>
</template>

<script setup lang="ts">
/**
 * The "pick how to launch this game" modal, shown when a game exposes more
 * than one launch option. Stateless — `useGameLaunch` on the parent owns
 * the option list and decides what `select` does.
 */
import { PlayIcon } from "@heroicons/vue/20/solid";

defineProps<{
  open: boolean;
  gameName: string;
  options: Array<{ name: string }>;
}>();

defineEmits<{
  (e: "select", index: number): void;
  (e: "cancel"): void;
}>();
</script>
