<template>
  <div
    class="flex items-center justify-between px-8 h-12 backdrop-blur-sm border-t shrink-0"
    :style="{ backgroundColor: 'color-mix(in srgb, var(--bpm-bg) 90%, transparent)', borderColor: 'var(--bpm-border)' }"
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
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";

const route = useRoute();

const showSearch = computed(() =>
  route.path.startsWith("/bigpicture/library") ||
  route.path.startsWith("/bigpicture/store"),
);

const showOptions = computed(() =>
  route.path.startsWith("/bigpicture/library/") &&
  route.path !== "/bigpicture/library",
);

const showSort = computed(() =>
  route.path === "/bigpicture/library" ||
  route.path.startsWith("/bigpicture/store"),
);
</script>
