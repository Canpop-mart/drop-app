<template>
  <section v-if="challenge" class="mb-8">
    <div class="flex items-baseline gap-2 mb-3">
      <h2 class="text-lg font-display font-semibold text-zinc-100">
        Your weekly quest
      </h2>
      <span class="text-xs text-zinc-500">{{ daysSuffix }}</span>
    </div>

    <div
      class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-amber-900/30 via-zinc-900/70 to-orange-900/20 ring-1 ring-amber-500/25 px-5 py-5"
    >
      <div class="flex items-center gap-5">
        <div
          class="shrink-0 size-12 sm:size-14 rounded-full bg-amber-500/15 ring-1 ring-amber-400/40 flex items-center justify-center"
        >
          <component :is="kindIcon" class="size-6 text-amber-300" />
        </div>

        <div class="flex-1 min-w-0">
          <p
            class="text-base sm:text-lg font-display font-semibold text-zinc-100 leading-snug truncate"
          >
            {{ challenge.title }}
          </p>
          <p class="text-xs text-zinc-400 mt-0.5 line-clamp-2">
            {{ challenge.description }}
          </p>

          <!-- Progress bar — `currentValue` is the CALLER's progress. -->
          <div class="mt-3 flex items-center gap-3">
            <div
              class="relative h-2 flex-1 rounded-full bg-zinc-800/80 overflow-hidden"
            >
              <div
                class="absolute inset-y-0 left-0 rounded-full bg-gradient-to-r from-amber-500 to-orange-400 transition-[width] duration-700 ease-out"
                :style="{ width: `${challenge.percentComplete}%` }"
              />
            </div>
            <p class="text-xs font-medium text-zinc-300 tabular-nums shrink-0">
              {{ formatCurrent }} / {{ formatTarget }}
            </p>
          </div>
        </div>
      </div>
    </div>
  </section>
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
  if (d === 1) return "1 day left";
  return `${d} days left`;
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
