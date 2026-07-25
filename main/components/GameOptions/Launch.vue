<template>
  <div>
    <label for="launch" class="block text-sm/6 font-medium text-zinc-100"
      >Launch string template</label
    >
    <div class="mt-2">
      <input
        type="text"
        name="launch"
        id="launch"
        class="block w-full rounded-md bg-zinc-800 px-3 py-1.5 text-base text-zinc-100 outline-1 -outline-offset-1 outline-zinc-800 placeholder:text-zinc-400 focus:outline-2 focus:-outline-offset-2 focus:outline-blue-600 sm:text-sm/6"
        placeholder="{}"
        aria-describedby="launch-description"
        v-model="model.launchTemplate"
      />
    </div>
    <p class="mt-2 text-sm text-zinc-400" id="launch-description">
      Override the launch string. Passed to system's default shell, and replaces
      "{}" with the command to start the game.
      <span class="font-semibold text-zinc-200"
        >Leaving it blank will cause the game not to start.</span
      >
    </p>

    <ProtonSelector v-model="model" v-if="$props.protonEnabled" />

    <!-- MangoHud is a Linux performance overlay for any launched game, so it
         lives here with the launch/Proton settings rather than in the
         emulator-only Video tab. -->
    <div v-if="isLinux" class="mt-6">
      <label class="block text-sm font-medium text-zinc-100 mb-2">
        MangoHud overlay
      </label>
      <div class="grid grid-cols-4 gap-2">
        <button
          v-for="opt in MANGOHUD_OPTIONS"
          :key="opt.value"
          type="button"
          :class="[
            'px-2 py-1.5 rounded text-xs font-medium transition-colors truncate',
            (model.mangohud ?? 'off') === opt.value
              ? 'bg-blue-600 text-white'
              : 'bg-zinc-700 text-zinc-300 hover:bg-zinc-600 hover:text-zinc-100',
          ]"
          @click="model.mangohud = opt.value"
        >
          {{ opt.label }}
        </button>
      </div>
      <p class="mt-2 text-xs text-zinc-500">
        An on-screen performance overlay (FPS, frametimes) while the game runs.
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { platform } from "@tauri-apps/plugin-os";
import type { GameVersion } from "~/types";
import ProtonSelector from "./ProtonSelector.vue";
import { MANGOHUD_OPTIONS } from "~/composables/game-detail/emulator-options";

const model = defineModel<GameVersion["userConfiguration"]>({ required: true });

defineProps<{
  protonEnabled: boolean;
}>();

const isLinux = platform() === "linux";
</script>
