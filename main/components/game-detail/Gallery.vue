<template>
  <div class="bg-zinc-800/50 rounded-xl p-6 backdrop-blur-sm">
    <div class="relative">
      <div v-if="imageIds.length > 0">
        <div
          :ref="(el: any) => registerAction?.(el, { onSelect: () => (fullscreenOpen = true) })"
          class="relative aspect-video rounded-lg overflow-hidden cursor-pointer group"
        >
          <div
            class="absolute inset-0"
            @click="fullscreenOpen = true"
          >
            <TransitionGroup name="slide" tag="div" class="h-full">
              <img
                v-for="(url, index) in imageIds"
                :key="url"
                :src="useObject(url)"
                class="absolute inset-0 w-full h-full object-cover"
                v-show="index === currentImageIndex"
              />
            </TransitionGroup>
          </div>

          <div
            class="absolute inset-0 flex items-center justify-between px-4 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"
          >
            <div class="pointer-events-auto">
              <button
                v-if="imageIds.length > 1"
                @click.stop="previousImage()"
                class="p-2 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900/80 transition-all duration-300 hover:scale-110"
              >
                <ChevronLeftIcon class="size-5" />
              </button>
            </div>
            <div class="pointer-events-auto">
              <button
                v-if="imageIds.length > 1"
                @click.stop="nextImage()"
                class="p-2 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900/80 transition-all duration-300 hover:scale-110"
              >
                <ChevronRightIcon class="size-5" />
              </button>
            </div>
          </div>

          <div
            class="absolute inset-0 bg-gradient-to-t from-black/50 to-transparent opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"
          />
          <div
            class="absolute bottom-4 right-4 flex items-center gap-x-2 text-white opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"
          >
            <ArrowsPointingOutIcon class="size-5" />
            <span class="text-sm font-medium">View Fullscreen</span>
          </div>
        </div>

        <div class="mt-3 flex justify-center gap-x-2">
          <button
            v-for="(_, index) in imageIds"
            :key="index"
            @click.stop="currentImageIndex = index"
            class="w-2 h-2 rounded-full transition-all"
            :class="[
              currentImageIndex === index
                ? 'bg-zinc-100 scale-125'
                : 'bg-zinc-600 hover:bg-zinc-500',
            ]"
          />
        </div>
      </div>

      <div
        v-else
        class="aspect-video rounded-lg overflow-hidden bg-zinc-900/50 flex flex-col items-center justify-center text-center px-4"
      >
        <PhotoIcon class="size-12 text-zinc-500 mb-2" />
        <p class="text-zinc-400 font-medium">No images available</p>
        <p class="text-zinc-500 text-sm">
          Game screenshots will appear here when available
        </p>
      </div>
    </div>
  </div>

  <!-- Fullscreen viewer — shared with the description images on both surfaces. -->
  <ImageLightbox
    :open="fullscreenOpen"
    :srcs="imageSrcs"
    :start-index="currentImageIndex"
    :alt-prefix="`${gameName} screenshot`"
    :gamepad="!!registerAction"
    @navigate="currentImageIndex = $event"
    @close="fullscreenOpen = false"
  />
</template>

<script setup lang="ts">
/**
 * Gallery for the library game-detail page — the image carousel, shared by the
 * desktop About tab and Big Picture. Self-contained: it owns the current index
 * and the fullscreen open state, so the parent just passes the image IDs. The
 * fullscreen viewer itself is `ImageLightbox`, shared with description images.
 */
import {
  ArrowsPointingOutIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  PhotoIcon,
} from "@heroicons/vue/20/solid";

const props = defineProps<{
  imageIds: string[];
  gameName: string;
  /**
   * Optional — pass the Big Picture page's focus-nav registrar (the "content"
   * group from `useBpFocusableGroup`) so a controller can reach the carousel
   * and open it fullscreen. Absent on desktop, which is mouse-driven.
   */
  registerAction?: (
    el: any,
    opts: { onSelect: () => void; onContext?: () => void },
  ) => void;
}>();

const currentImageIndex = ref(0);
const fullscreenOpen = ref(false);

// `useObject` (object://) rather than the server:// proxy: it is the only
// image path with a cache in front of it.
const imageSrcs = computed(() => props.imageIds.map((id) => useObject(id)));

function nextImage() {
  if (props.imageIds.length === 0) return;
  currentImageIndex.value =
    (currentImageIndex.value + 1) % props.imageIds.length;
}

function previousImage() {
  if (props.imageIds.length === 0) return;
  currentImageIndex.value =
    (currentImageIndex.value - 1 + props.imageIds.length) %
    props.imageIds.length;
}
</script>

<style scoped>
.slide-enter-active,
.slide-leave-active {
  transition: all 0.3s ease;
  position: absolute;
}

.slide-enter-from {
  opacity: 0;
  transform: translateX(100%);
}

.slide-leave-to {
  opacity: 0;
  transform: translateX(-100%);
}
</style>
