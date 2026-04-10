<template>
  <div
    class="flex items-center justify-between px-8 h-12 bg-zinc-950/90 backdrop-blur-sm border-t border-zinc-800/30 shrink-0"
  >
    <!-- Left: primary actions -->
    <div class="flex items-center gap-6">
      <BigPictureButtonPrompt button="A" label="Select" />
      <BigPictureButtonPrompt button="B" label="Back" />
    </div>

    <!-- Right: contextual actions -->
    <div class="flex items-center gap-6">
      <BigPictureButtonPrompt v-if="showSearch" button="Y" label="Search" />
      <BigPictureButtonPrompt v-if="showOptions" button="X" label="Options" />
      <BigPictureButtonPrompt button="LB" label="" />
      <BigPictureButtonPrompt button="RB" label="Switch Tab" />
    </div>
  </div>
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";

const route = useRoute();

const showSearch = computed(() => route.path.startsWith("/bigpicture/library"));

const showOptions = computed(() =>
  route.path.startsWith("/bigpicture/library/"),
);
</script>
