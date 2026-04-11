<template>
  <div />
</template>

<script setup lang="ts">
// No standalone /bigpicture/profile page — redirect back to community
definePageMeta({ layout: "bigpicture" });
const router = useRouter();
onMounted(() => {
  router.replace("/bigpicture/community");
});
</script>
