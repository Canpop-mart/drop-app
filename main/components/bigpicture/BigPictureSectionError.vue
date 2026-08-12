<template>
  <!-- Big Picture failure state for one section. The whole card is the retry
       target so a controller only has to land on it and press A. -->
  <button
    class="w-full rounded-xl px-6 py-8 text-center transition-colors"
    style="background-color: var(--bpm-surface)"
    @click="emit('retry')"
  >
    <p class="text-base font-medium" style="color: var(--bpm-text)">
      {{ title }}
    </p>
    <p v-if="detail" class="mt-1 text-sm" style="color: var(--bpm-muted)">
      {{ detail }}
    </p>
    <span
      class="mt-4 inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium"
      style="
        background-color: var(--bpm-accent-hex);
        color: var(--bpm-accent-text);
      "
    >
      <ArrowPathIcon class="size-4" />
      Retry
    </span>
  </button>
</template>

<script setup lang="ts">
import { ArrowPathIcon } from "@heroicons/vue/24/outline";

withDefaults(
  defineProps<{
    title?: string;
    detail?: string;
  }>(),
  { title: "Couldn't reach the server", detail: undefined },
);

const emit = defineEmits<{ (e: "retry"): void }>();
</script>
