<template>
  <div class="flex items-center" :class="sizeClasses.gap">
    <!-- Face buttons (A/B/X/Y) render as a 4-button diamond glyph
         with the active button filled and the rest as hollow rings -->
    <svg
      v-if="isFaceButton"
      :class="sizeClasses.glyph"
      viewBox="0 0 40 40"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <!-- South / bottom (A) -->
      <circle cx="20" cy="32" r="6" :fill="button === 'A' ? 'currentColor' : 'none'" stroke="currentColor" :stroke-width="button === 'A' ? 0 : 2" />
      <!-- North / top (Y) -->
      <circle cx="20" cy="8" r="6" :fill="button === 'Y' ? 'currentColor' : 'none'" stroke="currentColor" :stroke-width="button === 'Y' ? 0 : 2" />
      <!-- West / left (X) -->
      <circle cx="8" cy="20" r="6" :fill="button === 'X' ? 'currentColor' : 'none'" stroke="currentColor" :stroke-width="button === 'X' ? 0 : 2" />
      <!-- East / right (B) -->
      <circle cx="32" cy="20" r="6" :fill="button === 'B' ? 'currentColor' : 'none'" stroke="currentColor" :stroke-width="button === 'B' ? 0 : 2" />
    </svg>

    <!-- Non-face buttons use text labels -->
    <span
      v-else
      class="inline-flex items-center justify-center rounded-md font-bold uppercase tracking-wide bg-zinc-700/50 text-zinc-300 ring-1 ring-zinc-600/50"
      :class="sizeClasses.badge"
    >
      {{ buttonLabel }}
    </span>

    <span v-if="label" :class="['text-zinc-400', sizeClasses.label]">{{
      label
    }}</span>
  </div>
</template>

<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    button: string;
    label?: string;
    size?: "sm" | "md" | "lg";
  }>(),
  { size: "md" },
);

const FACE_BUTTONS = new Set(["A", "B", "X", "Y"]);
const isFaceButton = computed(() => FACE_BUTTONS.has(props.button));

const sizeClasses = computed(() => {
  switch (props.size) {
    case "sm":
      return {
        gap: "gap-1",
        badge: "min-w-[1.25rem] h-5 px-1 text-[10px]",
        label: "text-[10px]",
        glyph: "size-5 text-zinc-300",
      };
    case "lg":
      return {
        gap: "gap-2",
        badge: "min-w-[2rem] h-8 px-2 text-sm",
        label: "text-sm",
        glyph: "size-8 text-zinc-300",
      };
    default:
      return {
        gap: "gap-1.5",
        badge: "min-w-[1.75rem] h-7 px-1.5 text-xs",
        label: "text-xs",
        glyph: "size-7 text-zinc-300",
      };
  }
});

const buttonLabel = computed(() => {
  const map: Record<string, string> = {
    LB: "LB",
    RB: "RB",
    LT: "LT",
    RT: "RT",
    Start: "\u2630", // hamburger ☰
    Select: "\u25A1", // square □
    Guide: "\u229A", // circled ring ⊚
  };
  return map[props.button] ?? props.button;
});
</script>
