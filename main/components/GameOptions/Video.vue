<template>
  <div class="space-y-6">
    <p class="text-sm text-zinc-400">
      These apply the next time you launch the game.
    </p>

    <!-- Controller layout -->
    <div>
      <label class="block text-sm font-medium text-zinc-100 mb-2">
        Controller Layout
      </label>
      <div class="grid grid-cols-2 gap-2">
        <button
          v-for="opt in CONTROLLER_OPTIONS"
          :key="String(opt.value)"
          type="button"
          :class="segClass(model.controllerType === opt.value)"
          @click="model.controllerType = opt.value"
        >
          {{ opt.label }}
        </button>
      </div>
      <p class="mt-2 text-xs text-zinc-500">
        Auto detects your controller. Pick one here if the face buttons come out
        in the wrong order or the in-game shortcuts do nothing.
      </p>
    </div>

    <!-- Quality preset -->
    <div>
      <label class="block text-sm font-medium text-zinc-100 mb-2">
        Quality Preset
      </label>
      <div class="grid grid-cols-5 gap-2">
        <button
          v-for="opt in QUALITY_OPTIONS"
          :key="String(opt.value)"
          type="button"
          :class="segClass(model.qualityPreset === opt.value)"
          @click="model.qualityPreset = opt.value"
        >
          {{ opt.label }}
        </button>
      </div>
      <p class="mt-2 text-xs text-zinc-500">
        Higher presets raise internal resolution for 3D cores. Auto leaves each
        core at its default.
      </p>
    </div>

    <!-- Aspect ratio -->
    <div>
      <label class="block text-sm font-medium text-zinc-100 mb-2">
        Aspect Ratio
      </label>
      <div class="grid grid-cols-3 gap-2">
        <button
          v-for="opt in ASPECT_CYCLE"
          :key="opt"
          type="button"
          :class="segClass(model.widescreen === opt)"
          @click="model.widescreen = opt"
        >
          {{ aspectLabel(opt) }}
        </button>
      </div>
    </div>

    <!-- Fullscreen -->
    <div class="flex items-center justify-between">
      <div>
        <p class="text-sm font-medium text-zinc-100">Fullscreen</p>
        <p class="text-xs text-zinc-500">Launch the emulator in fullscreen.</p>
      </div>
      <button
        type="button"
        :class="toggleClass(isFullscreen)"
        @click="model.fullscreen = !isFullscreen"
      >
        {{ isFullscreen ? "On" : "Off" }}
      </button>
    </div>

    <!-- CRT filter -->
    <div class="flex items-center justify-between">
      <div>
        <p class="text-sm font-medium text-zinc-100">CRT Filter</p>
        <p class="text-xs text-zinc-500">
          A scanline shader for a retro-TV look. Best on 2D games.
        </p>
      </div>
      <button
        type="button"
        :class="toggleClass(model.crtShader)"
        @click="model.crtShader = !model.crtShader"
      >
        {{ model.crtShader ? "On" : "Off" }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * "Video & Controls" tab of the Configure modal, shown for emulated (RetroArch)
 * games. Edits the shared userConfiguration object directly (the modal owns the
 * save). These used to be cramped, clipping button-rows inside the cog dropdown;
 * here they get full width so nothing truncates.
 */
import type { GameVersion } from "~/types";
import {
  ASPECT_CYCLE,
  CONTROLLER_OPTIONS,
  QUALITY_OPTIONS,
  aspectLabel,
} from "~/composables/game-detail/emulator-options";

const model = defineModel<GameVersion["userConfiguration"]>({ required: true });

// Backend stores Option<bool>; null means "no preference" = RetroArch default
// (fullscreen on). Collapse null/true into "on" for the toggle.
const isFullscreen = computed(() => model.value.fullscreen ?? true);

const segBase =
  "px-2 py-1.5 rounded text-xs font-medium transition-colors truncate";
function segClass(active: boolean) {
  return [
    segBase,
    active
      ? "bg-blue-600 text-white"
      : "bg-zinc-700 text-zinc-300 hover:bg-zinc-600 hover:text-zinc-100",
  ];
}
function toggleClass(active: boolean) {
  return [
    "px-3 py-1 rounded-md text-xs font-medium transition-colors",
    active ? "bg-green-600 text-white" : "bg-zinc-700 text-zinc-300",
  ];
}
</script>
