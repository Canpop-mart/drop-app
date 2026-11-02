<template>
  <NuxtLink
    :to="`/library/emulator/${id}`"
    class="group relative flex h-28 w-52 shrink-0 flex-col items-center justify-center overflow-hidden rounded-xl bg-gradient-to-b from-zinc-900 to-zinc-950 px-4 py-3 ring-1 ring-zinc-800 transition-all hover:-translate-y-0.5 hover:ring-blue-500/40"
  >
    <!-- The emulator's own icon, or a chip glyph when it has none. -->
    <img
      v-if="iconObjectId"
      :src="useObject(iconObjectId)"
      :alt="name"
      class="mb-1.5 size-12 rounded-lg object-cover shadow-md transition-transform duration-300 group-hover:scale-105"
    />
    <div
      v-else
      class="mb-1.5 flex size-12 items-center justify-center rounded-lg bg-zinc-800 transition-transform duration-300 group-hover:scale-105"
    >
      <CpuChipIcon class="size-7 text-zinc-500" />
    </div>

    <div class="relative text-center">
      <div class="font-display text-sm font-bold text-zinc-100">
        {{ name }}
      </div>
      <div class="text-[11px] text-zinc-400">
        {{ subtitle }}
      </div>
    </div>
  </NuxtLink>
</template>

<script setup lang="ts">
import { CpuChipIcon } from "@heroicons/vue/24/outline";

const props = defineProps<{
  id: string;
  name: string;
  /** Icon object id; empty string falls back to the chip glyph. */
  iconObjectId: string;
  /** RetroArch host — shows a core count instead of the generic label. */
  retroarch: boolean;
  coreCount: number;
}>();

// RetroArch hosts show how many cores they carry; other emulators just read
// "Emulator" (the cores concept doesn't apply to them).
const subtitle = computed(() =>
  props.retroarch
    ? `${props.coreCount} core${props.coreCount === 1 ? "" : "s"}`
    : "Emulator",
);
</script>
