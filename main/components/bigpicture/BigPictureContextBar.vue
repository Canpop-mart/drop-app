<template>
  <div class="shrink-0">
    <!-- Persistent download strip — visible on every page except /downloads
         so the user always knows a download is running and can jump to it. -->
    <button
      v-if="showDownloadStrip"
      type="button"
      :ref="
        (el: any) =>
          registerChrome(el, {
            onSelect: () => navigateTo('/bigpicture/downloads'),
          })
      "
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

      <!-- Right: contextual actions. Paired triggers/bumpers share one label,
           so the two glyphs sit together instead of one showing a blank. -->
      <div class="flex items-center gap-6">
        <BigPictureButtonPrompt v-if="showSearch" button="Y" label="Search" />
        <BigPictureButtonPrompt v-if="showSort" button="X" label="Sort" />
        <BigPictureButtonPrompt v-if="startLabel" button="Start" :label="startLabel" />
        <div class="flex items-center gap-1.5">
          <BigPictureButtonPrompt button="LT" label="" />
          <BigPictureButtonPrompt button="RT" label="Scroll" />
        </div>
        <div class="flex items-center gap-1.5">
          <BigPictureButtonPrompt button="LB" label="" />
          <BigPictureButtonPrompt button="RB" label="Prev/Next Section" />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import { useReducedMotion } from "~/composables/bp-reduced-motion";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useQueueState, useStatsState, formatKilobytes } from "~/composables/downloads";

const { reducedMotion } = useReducedMotion();
const route = useRoute();
// Persistent chrome, same group as the top-bar avatar: reachable with RB
// without ever being a page's default focus.
const registerChrome = useBpFocusableGroup("chrome");

// Search is only wired up on list pages that have a searchable input —
// not on the library detail page (/bigpicture/library/[id]), downloads,
// settings, profile, etc. The home screen is the merged library, so it is
// where the library's search and sort now live.
const showSearch = computed(
  () => route.path === "/bigpicture" || route.path === "/bigpicture/store",
);

// Start does two different things depending on the page, so it can't carry one
// label: the game page opens the Game Options modal, the store toggles
// bulk-select on the Browse tab. It used to say "Options" on both.
const startLabel = computed(() => {
  if (route.path.startsWith("/bigpicture/library/")) return "Options";
  if (route.path === "/bigpicture/store") return "Select Multiple";
  return "";
});

const showSort = computed(
  () => route.path === "/bigpicture" || route.path === "/bigpicture/store",
);

// ── Download indicator strip ────────────────────────────────────────────
const queue = useQueueState();
const stats = useStatsState();
const downloadCount = computed(() => queue.value.queue.length);
// The downloads page is itself the canonical view, so the strip stays off
// there. It used to be off on the home page too, because each of the ten
// themes drew its own downloads list; those are gone and this strip is the
// single tracker again.
const showDownloadStrip = computed(
  () => downloadCount.value > 0 && route.path !== "/bigpicture/downloads",
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
