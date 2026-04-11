<template>
  <div class="flex h-screen w-screen bg-zinc-950 overflow-hidden">
    <!-- Navigation Rail (left edge) -->
    <BigPictureNavRail />

    <!-- Main content area -->
    <div class="flex-1 flex flex-col min-w-0">
      <BigPictureTopBar />
      <div class="flex-1 overflow-y-auto" data-bp-scroll>
        <Transition name="bp-page">
          <slot />
        </Transition>
      </div>
      <BigPictureContextBar />
    </div>
  </div>
</template>

<script setup lang="ts">
import BigPictureNavRail from "~/components/bigpicture/BigPictureNavRail.vue";
import BigPictureTopBar from "~/components/bigpicture/BigPictureTopBar.vue";
import BigPictureContextBar from "~/components/bigpicture/BigPictureContextBar.vue";
import { useFocusNavigation } from "~/composables/focus-navigation";

const focusNav = useFocusNavigation();

// Catch Vue rendering errors in BPM so they get logged instead of
// triggering Nuxt's error page (which breaks out of BPM layout)
onErrorCaptured((err, instance, info) => {
  console.error("[BPM:LAYOUT] Vue error captured:", err, "\nInfo:", info, "\nComponent:", instance?.$options?.name ?? instance?.$options?.__name);
  return false; // prevent propagation to Nuxt error handler
});

onMounted(() => {
  focusNav.enabled.value = true;
});

onUnmounted(() => {
  focusNav.enabled.value = false;
});
</script>

<style scoped>
.bp-page-enter-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}

.bp-page-leave-active {
  transition:
    opacity 0.1s ease,
    transform 0.1s ease;
  position: absolute;
  inset: 0;
}

.bp-page-enter-from {
  opacity: 0;
  transform: translateY(4px);
}

.bp-page-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>

<!-- Global BPM focus & press styles (unscoped so child components inherit) -->
<style>
/* Focus indicator ring — applied by focus-navigation.ts */
[data-focusable].bp-focused {
  outline: 3px solid rgba(59, 130, 246, 0.8);
  outline-offset: 2px;
  border-radius: 0.75rem;
  transition:
    outline-color 0.12s ease,
    transform 0.08s ease;
}

/* Press feedback — brief scale-down on A button */
[data-focusable].bp-pressed {
  transform: scale(0.97);
  transition: transform 0.08s ease;
}
</style>
