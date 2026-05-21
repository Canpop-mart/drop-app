<template>
  <div class="bg-zinc-800/50 rounded-xl p-6 backdrop-blur-sm">
    <div class="relative">
      <div v-if="imageIds.length > 0">
        <div
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

  <!-- Fullscreen viewer. -->
  <Transition
    enter="transition ease-out duration-300"
    enter-from="opacity-0"
    enter-to="opacity-100"
    leave="transition ease-in duration-200"
    leave-from="opacity-100"
    leave-to="opacity-0"
  >
    <div
      v-if="fullscreenOpen"
      class="fixed inset-0 z-50 bg-black/95 flex items-center justify-center"
      @click="fullscreenOpen = false"
    >
      <div
        class="relative w-full h-full flex items-center justify-center"
        @click.stop
      >
        <button
          class="absolute top-4 right-4 p-2 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
          @click.stop="fullscreenOpen = false"
        >
          <XMarkIcon class="size-6" />
        </button>

        <button
          v-if="imageIds.length > 1"
          @click.stop="previousImage()"
          class="absolute left-4 p-3 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
        >
          <ChevronLeftIcon class="size-6" />
        </button>
        <button
          v-if="imageIds.length > 1"
          @click.stop="nextImage()"
          class="absolute right-4 p-3 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
        >
          <ChevronRightIcon class="size-6" />
        </button>

        <TransitionGroup
          name="slide"
          tag="div"
          class="w-full h-full flex items-center justify-center"
          @click.stop
        >
          <img
            v-for="(url, index) in imageIds"
            v-show="currentImageIndex === index"
            :key="index"
            :src="useObject(url)"
            class="max-h-[90vh] max-w-[90vw] object-contain"
            :alt="`${gameName} screenshot ${index + 1}`"
          />
        </TransitionGroup>

        <div
          class="absolute bottom-4 left-1/2 -translate-x-1/2 px-4 py-2 rounded-full bg-zinc-900/50 backdrop-blur-sm"
        >
          <p class="text-zinc-100 text-sm font-medium">
            {{ currentImageIndex + 1 }} / {{ imageIds.length }}
          </p>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
/**
 * Gallery tab for the library game-detail page — the image carousel and
 * its fullscreen viewer. Self-contained: it owns the current index and
 * the fullscreen open state, so the parent just passes the image IDs.
 */
import {
  ArrowsPointingOutIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  PhotoIcon,
  XMarkIcon,
} from "@heroicons/vue/20/solid";

const props = defineProps<{
  imageIds: string[];
  gameName: string;
}>();

const currentImageIndex = ref(0);
const fullscreenOpen = ref(false);

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
