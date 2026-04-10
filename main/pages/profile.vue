<template>
  <div class="flex min-h-full flex-col bg-zinc-950">
    <div class="flex w-full flex-1 flex-col">
      <iframe
        :src="profileUrl"
        title="Profile"
        class="h-full min-h-full w-full border-0 flex-1"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";

const state = useAppState();
const username = state.value?.user?.username ?? "me";

const profileUrl = convertFileSrc("dummyvalue", "server").replace(
  "dummyvalue",
  `user/${username}`,
);

useHead({
  title: "Profile",
});
</script>
