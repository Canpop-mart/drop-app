<template>
  <HeaderButton v-if="showMinimise" @click="() => minimise()">
    <MinusIcon />
  </HeaderButton>
  <HeaderButton @click="() => close()">
    <XMarkIcon />
  </HeaderButton>
</template>

<script setup lang="ts">
import { MinusIcon, XMarkIcon } from "@heroicons/vue/16/solid";
import { getCurrentWindow } from "@tauri-apps/api/window";

const window = getCurrentWindow();
// Avoid top-level await — it makes this an async component which can block
// the entire layout rendering during transitions (e.g., BPM exit → default).
const showMinimise = ref(true);
window.isMinimizable().then((v) => { showMinimise.value = v; }).catch(() => {});

async function close() {
  await window.close();
}

async function minimise() {
  await window.minimize();
}
</script>
