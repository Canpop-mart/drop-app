<template>
  <div class="shrink-0">
    <!-- Persistent download strip — visible on every page except /downloads
         so the user always knows a download is running and can jump to it. -->
    <button
      v-if="showDownloadStrip"
      type="button"
      class="w-full flex items-center gap-3 px-8 h-8 text-xs text-zinc-300 border-t transition-colors hover:bg-zinc-800/40"
      :class="{ 'backdrop-blur-sm': !reducedMotion }"
      :style="{
        backgroundColor: reducedMotion
          ? 'var(--bpm-surface)'
          : 'color-mix(in srgb, var(--bpm-surface) 85%, transparent)',
        borderColor: 'var(--bpm-border)',
      }"
      @click="navigateTo('/bigpicture/downloads')"
    >
      <span class="shrink-0 size-2 rounded-full bg-blue-500 animate-pulse" />
      <span class="truncate flex-1 text-left">
        Downloading {{ downloadCount }}{{ downloadCount === 1 ? "" : " items" }}
        <span v-if="downloadSpeed" class="text-zinc-500">· {{ downloadSpeed }}/s</span>
      </span>
      <div class="h-1 w-40 rounded bg-zinc-700/60 overflow-hidden shrink-0">
        <div
          class="h-full bg-blue-500 transition-[width] duration-300"
          :style="{ width: `${combinedProgress}%` }"
        />
      </div>
      <span class="shrink-0 text-zinc-500 font-mono w-10 text-right">
        {{ combinedProgress }}%
      </span>
    </button>

    <div
      class="flex items-center justify-between px-8 h-12 border-t"
      :class="{ 'backdrop-blur-sm': !reducedMotion }"
      :style="{
        backgroundColor: reducedMotion
          ? 'var(--bpm-bg)'
          : 'color-mix(in srgb, var(--bpm-bg) 90%, transparent)',
        borderColor: 'var(--bpm-border)',
      }"
    >
      <!-- Left: primary actions -->
      <div class="flex items-center gap-6">
        <BigPictureButtonPrompt button="A" label="Select" />
        <BigPictureButtonPrompt button="B" label="Back" />
      </div>

      <!-- Right: contextual actions -->
      <div class="flex items-center gap-6">
        <BigPictureButtonPrompt v-if="showSearch" button="Y" label="Search" />
        <BigPictureButtonPrompt v-if="showSort" button="X" label="Sort" />
        <BigPictureButtonPrompt v-if="showOptions" button="Start" label="Options" />
        <BigPictureButtonPrompt button="LT" label="" />
        <BigPictureButtonPrompt button="RT" label="Scroll" />
        <BigPictureButtonPrompt button="LB" label="" />
        <BigPictureButtonPrompt button="RB" label="Switch Tab" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import { useReducedMotion } from "~/composables/bp-reduced-motion";
import { useQueueState, useStatsState, formatKilobytes } from "~/composables/downloads";

const { reducedMotion } = useReducedMotion();
const route = useRoute();

// Search is only wired up on list pages that have a searchable input —
// not on the library detail page (/bigpicture/library/[id]), downloads,
// settings, profile, etc.
const showSearch = computed(() =>
  route.path === "/bigpicture/library" ||
  route.path === "/bigpicture/library/collections" ||
  route.path === "/bigpicture/store",
);

const showOptions = computed(() =>
  (route.path.startsWith("/bigpicture/library/") &&
    route.path !== "/bigpicture/library") ||
  // Store also uses Start — toggles bulk-select mode on the Browse tab.
  route.path === "/bigpicture/store",
);

const showSort = computed(() =>
  route.path === "/bigpicture/library" ||
  route.path === "/bigpicture/store",
);

// ── Download indicator strip ────────────────────────────────────────────
const queue = useQueueState();
const stats = useStatsState();
const downloadCount = computed(() => queue.value.queue.length);
// The home page renders its own per-theme Downloads section, and the
// downloads page is itself the canonical view. Hiding the strip there
// avoids a duplicate tracker in either surface.
const showDownloadStrip = computed(
  () =>
    downloadCount.value > 0 &&
    route.path !== "/bigpicture/downloads" &&
    route.path !== "/bigpicture",
);
const downloadSpeed = computed(() =>
  stats.value.speed > 0 ? formatKilobytes(stats.value.speed) : "",
);
const combinedProgress = computed(() => {
  const items = queue.value.queue;
  if (!items.length) return 0;
  let current = 0;
  let max = 0;
  for (const it of items) {
    current += it.dl_current;
    max += it.dl_max;
  }
  if (max <= 0) return 0;
  return Math.min(100, Math.round((current / max) * 100));
});
</script>
