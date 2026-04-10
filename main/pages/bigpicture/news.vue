<template>
  <div class="flex flex-col flex-1 min-h-0 h-full">
    <iframe
      ref="iframeRef"
      :src="pageUrl"
      class="w-full border-0 flex-1 min-h-0"
    />
  </div>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { useIframeController } from "~/composables/iframe-controller";

definePageMeta({ layout: "bigpicture" });

const pageUrl = convertFileSrc("dummyvalue", "server").replace(
  "dummyvalue",
  "news",
);

const iframeRef = ref<HTMLIFrameElement | null>(null);
useIframeController(iframeRef);
</script>
