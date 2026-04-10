<script setup lang="ts">
// Big Picture root redirects to library — library is the main landing page
definePageMeta({ layout: "bigpicture" });

const router = useRouter();
onMounted(() => {
  router.replace("/bigpicture/library");
});
</script>

<template>
  <div />
</template>
