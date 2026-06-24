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

    <div
      class="mt-6 rounded-xl bg-zinc-800/50 border border-zinc-700/50 p-5"
    >
      <div class="flex items-start justify-between gap-4">
        <div>
          <h2 class="text-sm font-semibold text-zinc-100">
            Organize emulated games by console
          </h2>
          <p class="mt-1 text-xs text-zinc-400">
            Group emulated games into per-console rows on your library home,
            each opening a console view. PC games keep the normal layout. When
            off, emulated games sit in the regular grid and sort like anything
            else.
          </p>
        </div>
        <button
          type="button"
          role="switch"
          :aria-checked="consoleSections.enabled.value"
          class="relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors mt-0.5"
          :class="
            consoleSections.enabled.value ? 'bg-blue-600' : 'bg-zinc-600'
          "
          @click="consoleSections.toggle()"
        >
          <span
            class="inline-block size-5 transform rounded-full bg-white transition-transform"
            :class="
              consoleSections.enabled.value ? 'translate-x-5' : 'translate-x-0.5'
            "
          />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { MinusIcon, PlusIcon } from "@heroicons/vue/20/solid";
import { useUiZoom } from "~/composables/ui-zoom";
import { useConsoleSections } from "~/composables/console-sections";

const { zoom, minZoom, maxZoom, reset } = useUiZoom();
const consoleSections = useConsoleSections();

function bump(delta: number) {
  zoom.value = Math.round((zoom.value + delta) * 100) / 100;
}
</script>
