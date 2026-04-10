<template>
  <div
    class="flex items-center justify-between px-4 h-8 bg-zinc-950/90 backdrop-blur-sm border-t border-zinc-800/30 shrink-0 text-xs"
  >
    <div class="flex items-center gap-4">
      <BigPictureButtonPrompt button="A" label="Select" size="sm" />
      <BigPictureButtonPrompt button="B" label="Back" size="sm" />
    </div>
    <div class="flex items-center gap-4">
      <BigPictureButtonPrompt
        v-if="showSearch"
        button="Y"
        label="Search"
        size="sm"
      />
      <BigPictureButtonPrompt button="LB" label="" size="sm" />
      <BigPictureButtonPrompt button="RB" label="Tab" size="sm" />
    </div>
  </div>
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";

const route = useRoute();

const showSearch = computed(() => route.path.startsWith("/bigpicture/library"));
</script>
