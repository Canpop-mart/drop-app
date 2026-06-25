<template>
  <CollapsibleSection v-if="entries.length" title="HowLongToBeat">
    <div class="flex overflow-hidden rounded-lg ring-2 ring-zinc-950/80">
      <div
        v-for="(entry, i) in entries"
        :key="entry.label"
        class="flex-1 px-2 py-2.5 text-center"
        :class="i > 0 ? 'border-l-2 border-zinc-950/80' : ''"
        :style="{ backgroundColor: entry.color }"
      >
        <div class="text-[10px] font-medium leading-tight text-white/90">
          {{ entry.label }}
        </div>
        <div class="mt-0.5 text-sm font-bold leading-tight text-white">
          {{ entry.hours }}
        </div>
      </div>
    </div>
  </CollapsibleSection>
</template>

<script setup lang="ts">
import CollapsibleSection from "~/components/CollapsibleSection.vue";
import type { Game } from "~/types";

const props = defineProps<{ game: Game }>();

// HowLongToBeat times come in minutes; the segment labels read "3½ Hours".
function formatHltbHours(minutes: number): string {
  const rounded = Math.round((minutes / 60) * 2) / 2; // nearest half hour
  const whole = Math.floor(rounded);
  const half = rounded - whole >= 0.5;
  const value = whole === 0 ? (half ? "½" : "0") : `${whole}${half ? "½" : ""}`;
  return `${value} ${value === "1" ? "Hour" : "Hours"}`;
}

// HowLongToBeat's segment palette: Main Story blue, the longer tiers crimson.
const entries = computed(() => {
  const out: Array<{ label: string; hours: string; color: string }> = [];
  const add = (
    label: string,
    minutes: number | null | undefined,
    color: string,
  ) => {
    if (minutes && minutes > 0)
      out.push({ label, hours: formatHltbHours(minutes), color });
  };
  add("Main Story", props.game.mHltbMain, "#3b82f6");
  add("Main + Sides", props.game.mHltbMainSides, "#d6455f");
  add("Completionist", props.game.mHltbCompletionist, "#d6455f");
  return out;
});
</script>
