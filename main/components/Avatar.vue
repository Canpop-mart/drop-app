<!--
  Shared user avatar. Replaces the img-or-fallback block that was copy-pasted
  across every community / profile / leaderboard surface.

  - Shows the profile picture when `objectId` is set, otherwise the name's
    first initial on a deterministic per-name gradient (warmer than the old
    flat-grey UserIcon fallback).
  - `presence` renders a live green pulse dot (bottom-right), so "playing now"
    can decorate an avatar anywhere.

  Sized via inline styles (not `size-[Npx]`) because Tailwind's JIT can't see
  class names built from a dynamic value.
-->
<template>
  <span
    class="relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full"
    :style="{ width: px, height: px }"
  >
    <img
      v-if="objectId"
      :src="src"
      :alt="name ?? ''"
      class="h-full w-full object-cover"
    />
    <span
      v-else
      class="flex h-full w-full select-none items-center justify-center font-semibold text-white"
      :style="{ background: gradient, fontSize: fontPx }"
      >{{ initial }}</span
    >
    <span
      v-if="presence"
      class="pulse-dot absolute bottom-0 right-0 rounded-full bg-green-500 ring-2 ring-zinc-900"
      :style="{ width: dotPx, height: dotPx }"
      aria-label="Playing now"
    />
  </span>
</template>

<script setup lang="ts">
import { serverUrl } from "~/composables/use-server-fetch";

const props = withDefaults(
  defineProps<{
    objectId?: string | null;
    name?: string | null;
    /** Pixel diameter. */
    size?: number;
    presence?: boolean;
  }>(),
  { size: 32, presence: false },
);

const px = computed(() => `${props.size}px`);
const fontPx = computed(() => `${Math.round(props.size * 0.42)}px`);
const dotPx = computed(() => `${Math.max(8, Math.round(props.size * 0.28))}px`);

const src = computed(() =>
  props.objectId ? serverUrl(`api/v1/object/${props.objectId}`) : "",
);

const initial = computed(() => {
  const n = (props.name ?? "").trim();
  return n ? n[0]!.toUpperCase() : "?";
});

// Stable gradient derived from the name so fallbacks aren't a wall of grey.
const gradient = computed(() => {
  const n = props.name ?? "";
  let h = 0;
  for (let i = 0; i < n.length; i++) h = (h * 31 + n.charCodeAt(i)) & 0xffff;
  const hue = h % 360;
  return `linear-gradient(135deg, hsl(${hue} 45% 42%), hsl(${(hue + 40) % 360} 45% 32%))`;
});
</script>

<style scoped>
.pulse-dot {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}
@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}
</style>
