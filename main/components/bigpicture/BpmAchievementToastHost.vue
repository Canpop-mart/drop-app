<template>
  <!-- Themed unlock toast for Big Picture. One at a time; later unlocks
       queue behind it. Auto-dismisses, so nothing here needs focus: a
       controller never has to reach it. -->
  <BpmAchievementToast
    :theme-id="themeId"
    :achievement="current"
    @dismissed="showNext"
  />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, computed } from "vue";
import BpmAchievementToast from "~/components/bigpicture/BpmAchievementToast.vue";
import { useBpmTheme } from "~/composables/bp-theme";
import { useListen } from "~/composables/useListen";
import {
  acceptUnlock,
  bpmToastHosts,
  type AchievementUnlockedPayload,
} from "~/composables/achievements/toast";

const DEDUP_WINDOW_MS = 10_000;
// A burst (the exit check after a long offline session) shouldn't keep the
// screen busy for minutes; the achievement list has the rest.
const MAX_QUEUED = 5;

const theme = useBpmTheme();
const themeId = computed(() => theme.themeId.value);

type ToastItem = { title: string; game: string; icon?: string };
const current = ref<ToastItem | null>(null);
const queue: ToastItem[] = [];
const seen = new Map<string, number>();

function showNext() {
  current.value = queue.shift() ?? null;
}

useListen<AchievementUnlockedPayload>("achievement_unlocked", (event) => {
  const data = event.payload;
  if (!data?.id || !acceptUnlock(seen, data.id, Date.now(), DEDUP_WINDOW_MS)) return;
  const item: ToastItem = {
    title: data.title,
    // The theme's third line is the game; fall back to the description when
    // the game name isn't known.
    game: data.gameName || data.description || "",
    icon: data.iconUrl || undefined,
  };
  if (current.value === null) {
    current.value = item;
  } else if (queue.length < MAX_QUEUED) {
    queue.push(item);
  }
});

onMounted(() => {
  bpmToastHosts.value++;
});
onBeforeUnmount(() => {
  bpmToastHosts.value = Math.max(0, bpmToastHosts.value - 1);
});
</script>
