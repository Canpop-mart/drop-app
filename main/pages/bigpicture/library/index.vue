<template />

<script setup lang="ts">
// `/bigpicture/library` redirects to `/bigpicture`. Big Picture's home screen
// and its library were the same data twice — the home screen's recent list is
// the playtime feed truncated to 20, a strict subset of the library grid — so
// they were merged into one page. The themed spotlight hero, the rails and the
// full grid all live at `/bigpicture` now. Game pages at
// `/bigpicture/library/<id>` are unaffected.
definePageMeta({
  layout: false,
  middleware: () => navigateTo("/bigpicture", { replace: true }),
});
</script>
