<template>
  <div
    v-if="challenge"
    class="rounded-xl bg-zinc-800/50 ring-1 ring-zinc-700/40 px-4 py-3 hover:ring-amber-500/40 transition"
  >
    <div class="flex items-center gap-3">
      <div
        class="shrink-0 size-9 rounded-full bg-amber-500/15 flex items-center justify-center"
      >
        <component :is="kindIcon" class="size-4 text-amber-300" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="flex items-baseline justify-between gap-2">
          <p class="text-sm font-medium text-zinc-100 truncate">
            {{ challenge.title }}
            <span class="text-zinc-500 font-normal"
              >— {{ challenge.description }}</span
            >
          </p>
          <span class="text-[10px] text-zinc-500 shrink-0 tabular-nums">{{
            daysSuffix
          }}</span>
        </div>
        <!-- Progress bar — amber is the only accent. Surface itself stays
             neutral so the card visually matches the activity rows. -->
        <div class="mt-2 flex items-center gap-3">
          <div
            class="relative h-1.5 flex-1 rounded-full bg-zinc-900 overflow-hidden"
          >
            <div
              class="absolute inset-y-0 left-0 rounded-full bg-amber-500/80 transition-[width] duration-700 ease-out"
              :style="{ width: `${challenge.percentComplete}%` }"
            />
          </div>
          <p
            class="text-[11px] font-medium text-zinc-400 tabular-nums shrink-0"
          >
            {{ formatCurrent }} / {{ formatTarget }}
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  ClockIcon,
  TrophyIcon,
  TagIcon,
  Squares2X2Icon,
  ArrowUturnLeftIcon,
  BoltIcon,
  MoonIcon,
  SparklesIcon,
} from "@heroicons/vue/24/solid";
import type { WeeklyChallenge } from "~/composables/use-server-api";

const props = defineProps<{
  challenge: WeeklyChallenge | null;
}>();

// Per-kind icon — kept close to the per-kind copy in
// drop-server/server/api/v1/community/weekly-challenge.get.ts. The two
// "play-a-different-kind-of-game" kinds (`play_variety` and `fresh_drop`)
// share the Squares2X2 icon by design.
const kindIcon = computed(() => {
  if (!props.challenge) return ClockIcon;
  switch (props.challenge.kind) {
    case "play_hours":
      return ClockIcon;
    case "unlock_count":
      return TrophyIcon;
    case "play_variety":
      return Squares2X2Icon;
    case "rediscover":
      return ArrowUturnLeftIcon;
    case "marathon":
      return BoltIcon;
    case "night_owl":
      return MoonIcon;
    case "new_to_you":
      return SparklesIcon;
    case "genre_focus":
      return TagIcon;
    case "fresh_drop":
      return Squares2X2Icon;
    default:
      return ClockIcon;
  }
});

const daysSuffix = computed(() => {
  if (!props.challenge) return "";
  const d = props.challenge.daysRemaining;
  if (d <= 0) return "ends today";
  if (d === 1) return "1d left";
  return `${d}d left`;
});

// `play_hours` and `genre_focus` measure in whole hours; everything else
// is a raw count (including binary 0/1 quests, which render as "0 / 1" or
// "1 / 1").
const isHours = computed(
  () =>
    props.challenge?.kind === "play_hours" ||
    props.challenge?.kind === "genre_focus",
);

const formatCurrent = computed(() => {
  if (!props.challenge) return "";
  return isHours.value
    ? `${props.challenge.currentValue}h`
    : props.challenge.currentValue.toLocaleString();
});

const formatTarget = computed(() => {
  if (!props.challenge) return "";
  return isHours.value
    ? `${props.challenge.targetValue}h`
    : props.challenge.targetValue.toLocaleString();
});
</script>
