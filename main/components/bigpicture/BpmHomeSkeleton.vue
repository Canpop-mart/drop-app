<!--
  First-paint placeholder for the BPM home screen.

  The home screen used to render nothing at all while its data was in flight,
  so on a cold start the TV showed an empty coloured field until it landed.
  This skeleton is drawn entirely from the `--bpm-*` tokens, so it takes on
  whichever of the ten themes is active instead of imposing a palette, and it
  mirrors the shape the merged home shares (a hero, then a row of tiles).
-->
<template>
  <div class="min-h-full" style="background-color: var(--bpm-bg)">
    <div style="padding: 4vh 3vw">
      <!-- Hero -->
      <div
        class="bpm-skeleton-block"
        style="width: 100%; height: 40vh; border-radius: 1vh"
      />

      <!-- Shelf label -->
      <div
        class="bpm-skeleton-block"
        style="width: 14vw; height: 2.2vh; border-radius: 0.4vh; margin: 4vh 0 1.5vh"
      />

      <!-- Tile row -->
      <div class="flex" style="gap: 1.5vw">
        <div
          v-for="i in 7"
          :key="i"
          class="bpm-skeleton-block"
          style="width: 11vw; height: 22vh; border-radius: 0.8vh"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.bpm-skeleton-block {
  background-color: var(--bpm-muted);
  opacity: 0.12;
  animation: bpm-skeleton-pulse 1.6s ease-in-out infinite;
}

@keyframes bpm-skeleton-pulse {
  0%,
  100% {
    opacity: 0.08;
  }
  50% {
    opacity: 0.18;
  }
}

@media (prefers-reduced-motion: reduce) {
  .bpm-skeleton-block {
    animation: none;
  }
}
</style>
