<template>
  <div class="max-w-2xl">
    <div class="mb-6">
      <h1 class="text-xl font-semibold font-display text-zinc-100">
        Interface
      </h1>
      <p class="mt-1 text-sm text-zinc-400">
        Customise how Drop looks and behaves.
      </p>
    </div>

    <div class="rounded-xl bg-zinc-800/50 border border-zinc-700/50 p-5">
      <div class="mb-4">
        <h2 class="text-sm font-semibold text-zinc-100">Interface zoom</h2>
        <p class="mt-1 text-xs text-zinc-400">
          Rescale the entire Drop interface. Lower this if text is cut off,
          raise it if everything looks too small — particularly useful when
          the app is rendered under Steam's Game Mode (gamescope) at a lower
          effective resolution.
        </p>
      </div>
      <div class="flex items-center gap-3">
        <button
          :disabled="zoom <= minZoom"
          class="size-9 inline-flex items-center justify-center rounded-lg bg-zinc-700 text-zinc-200 hover:bg-zinc-600 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
          aria-label="Decrease UI zoom"
          @click="bump(-0.05)"
        >
          <MinusIcon class="size-4" />
        </button>
        <div class="flex-1 text-center text-sm tabular-nums text-zinc-100">
          {{ Math.round(zoom * 100) }}%
        </div>
        <button
          :disabled="zoom >= maxZoom"
          class="size-9 inline-flex items-center justify-center rounded-lg bg-zinc-700 text-zinc-200 hover:bg-zinc-600 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
          aria-label="Increase UI zoom"
          @click="bump(0.05)"
        >
          <PlusIcon class="size-4" />
        </button>
        <button
          class="px-3 h-9 inline-flex items-center rounded-lg text-xs font-medium bg-zinc-700 text-zinc-200 hover:bg-zinc-600 transition-colors"
          @click="reset"
        >
          Reset
        </button>
      </div>
      <p class="mt-3 text-xs text-zinc-500">
        Range: {{ Math.round(minZoom * 100) }}% &ndash;
        {{ Math.round(maxZoom * 100) }}%. Setting persists across restarts.
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { MinusIcon, PlusIcon } from "@heroicons/vue/20/solid";
import { useUiZoom } from "~/composables/ui-zoom";

const { zoom, minZoom, maxZoom, reset } = useUiZoom();

function bump(delta: number) {
  zoom.value = Math.round((zoom.value + delta) * 100) / 100;
}
</script>
