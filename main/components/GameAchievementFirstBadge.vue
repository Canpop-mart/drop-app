<template>
  <!-- A purely presentational subtitle: "First on this server · X · 3d ago".
       Pair with the gold ring around the achievement icon, which is applied
       by the parent (we only need the parent to read `first` from the
       firsts map and conditionally swap classes on the <img>). -->
  <span
    v-if="first"
    class="inline-flex items-center gap-1 text-[11px] font-medium text-yellow-400/90"
    :title="fullTimestamp"
  >
    <TrophyIcon class="size-3 shrink-0" />
    First on this server
    <span class="text-yellow-200/80">· {{ first.displayName }}</span>
    <span class="text-zinc-500">· {{ relativeTime }}</span>
  </span>
</template>

<script setup lang="ts">
/**
 * Subtitle badge used inline on the existing Achievements tab to mark
 * achievements that someone on this Drop server was first to unlock.
 *
 * The gold-ring border around the achievement icon is applied by the
 * parent template (one extra class on the existing <img>) — keeping the
 * badge here purely about the caption keeps the integration footprint
 * minimal: no DOM gymnastics in the existing list rows.
 */
import { TrophyIcon } from "@heroicons/vue/24/solid";
import type { GameAchievementFirst } from "~/composables/use-server-api";

const props = defineProps<{
  first: GameAchievementFirst | null | undefined;
}>();

/**
 * Relative time, matching the `formatTimeAgo` style elsewhere in the app
 * ("3d ago", "2h ago"). Stays in this component to avoid a cross-cutting
 * helper import for a single use.
 */
const relativeTime = computed(() => {
  if (!props.first) return "";
  const diff = Math.floor(
    (Date.now() - new Date(props.first.unlockedAt).getTime()) / 1000,
  );
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  if (diff < 31536000) return `${Math.floor(diff / 604800)}w ago`;
  return `${Math.floor(diff / 31536000)}y ago`;
});

const fullTimestamp = computed(() => {
  if (!props.first) return "";
  try {
    return new Date(props.first.unlockedAt).toLocaleString();
  } catch {
    return props.first.unlockedAt;
  }
});
</script>
