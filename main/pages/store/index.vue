<template>
  <div class="flex min-h-full flex-col bg-zinc-950">
    <div class="flex w-full flex-1 flex-col">
      <iframe
        :src="convertedStoreUrl"
        title="Drop store"
        class="h-full min-h-full w-full border-0 flex-1"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";

const route = useRoute();
const gameId = route.query.gameId?.toString();

const convertedStoreUrl = convertFileSrc(`dummyvalue`, "server").replace(
  "dummyvalue",
  gameId ? `store/${gameId}` : "store",
);

useHead({
  title: "Store",
});
</script>
