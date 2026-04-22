<template>
  <div
    :class="['h-screen w-screen overflow-hidden flex flex-col', themeClass, modeClass, { 'bpm-reduced-motion': reducedMotion }]"
    :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import { useBpmTheme } from "~/composables/bp-theme";
import { useReducedMotion } from "~/composables/bp-reduced-motion";
import { useUiZoom } from "~/composables/ui-zoom";
import { useFocusNavigation } from "~/composables/focus-navigation";

const bpmTheme = useBpmTheme();
const themeClass = computed(() => bpmTheme.themeData.value.cssClass);
const modeClass = computed(() => `bpm-${bpmTheme.mode.value}`);
const { reducedMotion } = useReducedMotion();
useUiZoom();

const focusNav = useFocusNavigation();

onMounted(() => {
  // Re-assert on the next tick so we win the race against the previous
  // layout's onUnmounted (Vue mounts new layout before unmounting old).
  focusNav.enabled.value = true;
  focusNav.setGroupOrder(["content", "wizard-chrome"]);
  nextTick(() => {
    focusNav.enabled.value = true;
  });
});

// Deliberately no onUnmounted: focus-nav is torn down by bigPicture.exit()
// on actual BPM exit. Clearing `enabled` here would break route transitions
// into another BPM layout (e.g. wizard → /bigpicture/settings).
</script>
