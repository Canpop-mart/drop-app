<template />

<script setup lang="ts">
// `/` redirects to /store. Drop's desktop landing page is the store —
// the previous standalone home dashboard didn't carry enough beyond what
// the store's Featured tab already shows, and shipping two near-identical
// landings split user attention. Anyone wanting the old "continue
// playing" shelf can find it on the library page.
definePageMeta({
  layout: false,
  middleware: () => navigateTo("/store", { replace: true }),
});
</script>
