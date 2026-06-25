<template>
  <div class="h-full flex flex-col">
    <div
      class="px-8 py-4 flex items-center gap-4 shrink-0"
      :style="{ borderBottom: '1px solid var(--bpm-border)' }"
    >
      <button
        :ref="(el: any) => registerContent(el, { onSelect: goBack })"
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors"
        :style="{
          backgroundColor: 'var(--bpm-surface)',
          color: 'var(--bpm-text)',
          border: '1px solid var(--bpm-border)',
        }"
        @click="goBack"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-4">
          <path fill-rule="evenodd" d="M11.78 5.22a.75.75 0 0 1 0 1.06L8.06 10l3.72 3.72a.75.75 0 1 1-1.06 1.06l-4.25-4.25a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0Z" clip-rule="evenodd" />
        </svg>
        Back
      </button>
      <h1 class="text-xl font-display font-semibold" :style="{ color: 'var(--bpm-text)' }">
        Error reference
      </h1>
      <span class="ml-auto hidden sm:inline-flex items-center gap-1.5 text-[11px]" :style="{ color: 'var(--bpm-muted)' }">
        <kbd class="px-1.5 py-0.5 rounded border" :style="{ borderColor: 'var(--bpm-border)' }">B</kbd>
        back
      </span>
    </div>

    <div data-bp-scroll class="flex-1 min-h-0 overflow-y-auto bp-scroll-hint">
      <div class="px-8 py-6 max-w-3xl">
        <p class="text-sm mb-6" :style="{ color: 'var(--bpm-muted)' }">
          A quick lookup for common warnings and errors you might hit while using Drop. Use the D-pad to scroll through entries.
        </p>
        <BpmErrorReference />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, nextTick } from "vue";
import BpmErrorReference from "~/components/bigpicture/BpmErrorReference.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";

definePageMeta({ layout: "bpm-wizard" });

const registerContent = useBpFocusableGroup("content");
const focusNav = useFocusNavigation();
const gamepad = useGamepad();
const unsubs: (() => void)[] = [];

function goBack() {
  navigateTo("/bigpicture/settings");
}

onMounted(() => {
  unsubs.push(gamepad.onButton(GamepadButton.East, goBack));
  nextTick(() => focusNav.focusGroup("content"));
});
onUnmounted(() => {
  for (const u of unsubs) u();
  unsubs.length = 0;
});
</script>
