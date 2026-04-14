<template>
  <div />
</template>

<script setup lang="ts">
import { useAppState } from "~/composables/app-state";

definePageMeta({ layout: "bigpicture" });

const router = useRouter();
const state = useAppState();

onMounted(() => {
  const username = state.value?.user?.username;
  if (username) {
    router.replace(`/bigpicture/profile/${username}`);
  } else {
    router.replace("/bigpicture");
  }
});
</script>
