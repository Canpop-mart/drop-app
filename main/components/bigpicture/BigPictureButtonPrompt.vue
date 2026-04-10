<template>
  <div class="flex items-center" :class="sizeClasses.gap">
    <span
      class="inline-flex items-center justify-center rounded-md font-bold uppercase tracking-wide"
      :class="[buttonStyle, sizeClasses.badge]"
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
    // H9 fix: size prop for layout consistency
    size?: "sm" | "md" | "lg";
  }>(),
  { size: "md" },
);

// H9 fix: responsive size classes
const sizeClasses = computed(() => {
  switch (props.size) {
    case "sm":
      return {
        gap: "gap-1",
        badge: "min-w-[1.25rem] h-5 px-1 text-[10px]",
        label: "text-[10px]",
      };
    case "lg":
      return {
        gap: "gap-2",
        badge: "min-w-[2rem] h-8 px-2 text-sm",
        label: "text-sm",
      };
    default:
      return {
        gap: "gap-1.5",
        badge: "min-w-[1.75rem] h-7 px-1.5 text-xs",
        label: "text-xs",
      };
  }
});

// Map button codes to display labels and styles
const buttonLabel = computed(() => {
  const map: Record<string, string> = {
    A: "A",
    B: "B",
    X: "X",
    Y: "Y",
    LB: "LB",
    RB: "RB",
    LT: "LT",
    RT: "RT",
    Start: "\u2630", // hamburger
    Select: "\u25A1", // square
    Guide: "\u229A", // xbox-ish
  };
  return map[props.button] ?? props.button;
});

const buttonStyle = computed(() => {
  const styles: Record<string, string> = {
    A: "bg-green-600/30 text-green-400 ring-1 ring-green-500/40",
    B: "bg-red-600/30 text-red-400 ring-1 ring-red-500/40",
    X: "bg-blue-600/30 text-blue-400 ring-1 ring-blue-500/40",
    Y: "bg-yellow-600/30 text-yellow-400 ring-1 ring-yellow-500/40",
  };
  return (
    styles[props.button] ??
    "bg-zinc-700/50 text-zinc-300 ring-1 ring-zinc-600/50"
  );
});
</script>
