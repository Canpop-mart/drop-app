<template>
  <!-- ═══ Desktop Big Picture (wide screen) ═══ -->
  <div
    v-if="!deck.isDeckMode.value"
    class="flex h-screen w-screen bg-zinc-950 overflow-hidden"
  >
    <!-- Navigation Rail (left edge) -->
    <BigPictureNavRail />

    <!-- Main content area -->
    <div class="flex-1 flex flex-col min-w-0">
      <BigPictureTopBar />
      <div class="flex-1 overflow-y-auto" data-bp-scroll>
        <Transition name="bp-page" mode="out-in">
          <slot />
        </Transition>
      </div>
      <BigPictureContextBar />
    </div>
  </div>

  <!-- ═══ Steam Deck / Handheld layout ═══ -->
  <div
    v-else
    class="flex flex-col h-screen w-screen bg-zinc-950 overflow-hidden deck-mode"
  >
    <!-- Compact top bar -->
    <DeckTopBar />

    <!-- Page content (fills remaining space between top bar and bottom nav) -->
    <div class="flex-1 overflow-y-auto min-h-0" data-bp-scroll>
      <Transition name="bp-page" mode="out-in">
        <slot />
      </Transition>
    </div>

    <!-- Controller prompt strip -->
    <DeckContextBar />

    <!-- Bottom tab navigation (SteamOS-style) -->
    <DeckNavBar />
  </div>
</template>

<script setup lang="ts">
import BigPictureNavRail from "~/components/bigpicture/BigPictureNavRail.vue";
import BigPictureTopBar from "~/components/bigpicture/BigPictureTopBar.vue";
import BigPictureContextBar from "~/components/bigpicture/BigPictureContextBar.vue";
import DeckNavBar from "~/components/bigpicture/DeckNavBar.vue";
import DeckTopBar from "~/components/bigpicture/DeckTopBar.vue";
import DeckContextBar from "~/components/bigpicture/DeckContextBar.vue";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useBigPictureMode } from "~/composables/big-picture";
import { useDeckMode } from "~/composables/deck-mode";

const focusNav = useFocusNavigation();
const bigPicture = useBigPictureMode();
const deck = useDeckMode();

// Ensure focus system is enabled when this layout mounts
onMounted(() => {
  focusNav.enabled.value = true;
});

onUnmounted(() => {
  focusNav.enabled.value = false;
});
</script>

<style scoped>
.bp-page-enter-active,
.bp-page-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.bp-page-enter-from {
  opacity: 0;
  transform: translateY(8px);
}

.bp-page-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
