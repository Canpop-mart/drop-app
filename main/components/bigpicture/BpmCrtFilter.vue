<template>
  <div v-if="enabled" class="crt-filter" :style="filterStyle"></div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  enabled: boolean
  intensity?: number
}

const props = withDefaults(defineProps<Props>(), {
  intensity: 0.5,
})

const filterStyle = computed(() => ({
  opacity: props.intensity,
}))
</script>

<style scoped>
.crt-filter {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 9997;
  pointer-events: none;
  background: repeating-linear-gradient(
    0deg,
    rgba(0, 0, 0, 0.15) 0px,
    rgba(0, 0, 0, 0.15) 1px,
    transparent 1px,
    transparent 2px
  );
  box-shadow: inset 0 0 80px rgba(0, 0, 0, 0.3);
  border-radius: 0.3% / 0.5%;
}
</style>
