<template>
  <!-- Failure state for one section of a page. Deliberately distinct from an
       empty state: an unreachable server and a genuinely empty list used to
       render identically, which is why "pages just won't load" looked like
       "there's nothing here". -->
  <div
    class="rounded-xl bg-zinc-800/40 ring-1 ring-zinc-700/40 px-4 py-6 text-center"
  >
    <p class="text-sm text-zinc-300">{{ title }}</p>
    <p v-if="detail" class="mt-1 text-xs text-zinc-500">{{ detail }}</p>
    <button
      class="mt-3 inline-flex items-center gap-1.5 rounded-md bg-zinc-700/60 px-3 py-1.5 text-xs font-semibold text-zinc-100 transition-colors hover:bg-zinc-700"
      @click="emit('retry')"
    >
      <ArrowPathIcon class="size-3.5" />
      Retry
    </button>
  </div>
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
