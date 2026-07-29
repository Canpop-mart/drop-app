<!--
  Shared stat card: a labelled number with an optional icon. Used for the
  Community header stats and the profile stat cards, which were previously
  inlined separately on each surface.
-->
<template>
  <div class="rounded-xl bg-zinc-800/50 px-4 py-3 ring-1 ring-zinc-700/40">
    <div class="flex items-center gap-1.5 text-zinc-500">
      <component :is="icon" v-if="icon" class="size-4" />
      <span class="text-xs font-medium uppercase tracking-wide">{{ label }}</span>
    </div>
    <div
      class="mt-1 font-display text-2xl font-bold tabular-nums text-zinc-100"
    >
      {{ value }}
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Component } from "vue";

defineProps<{
  label: string;
  value: string | number;
  icon?: Component;
}>();
</script>
