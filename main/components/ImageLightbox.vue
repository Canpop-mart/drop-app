<template>
  <Transition
    enter-active-class="transition ease-out duration-300"
    enter-from-class="opacity-0"
    enter-to-class="opacity-100"
    leave-active-class="transition ease-in duration-200"
    leave-from-class="opacity-100"
    leave-to-class="opacity-0"
  >
    <div
      v-if="open && srcs.length > 0"
      class="fixed inset-0 z-[210] bg-black/95 flex items-center justify-center"
      @click="close()"
    >
      <div
        class="relative w-full h-full flex items-center justify-center"
        @click.stop
      >
        <button
          class="absolute top-4 right-4 p-2 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
          aria-label="Close"
          @click.stop="close()"
        >
          <XMarkIcon class="size-6" />
        </button>

        <button
          v-if="srcs.length > 1"
          class="absolute left-4 p-3 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
          aria-label="Previous image"
          @click.stop="previous()"
        >
          <ChevronLeftIcon class="size-6" />
        </button>
        <button
          v-if="srcs.length > 1"
          class="absolute right-4 p-3 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
          aria-label="Next image"
          @click.stop="next()"
        >
          <ChevronRightIcon class="size-6" />
        </button>

        <TransitionGroup
          name="lightbox-slide"
          tag="div"
          class="w-full h-full flex items-center justify-center"
          @click.stop
        >
          <img
            v-for="(src, i) in srcs"
            v-show="i === index"
            :key="i"
            :src="src"
            class="max-h-[90vh] max-w-[90vw] object-contain"
            :alt="`${altPrefix} ${i + 1}`"
          />
        </TransitionGroup>

        <div
          v-if="srcs.length > 1"
          class="absolute bottom-4 left-1/2 -translate-x-1/2 px-4 py-2 rounded-full bg-zinc-900/50 backdrop-blur-sm"
        >
          <p class="text-zinc-100 text-sm font-medium">
            {{ index + 1 }} / {{ srcs.length }}
          </p>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
/**
 * Fullscreen image viewer shared by every surface. Lifted out of
 * `game-detail/Gallery.vue` so the gallery carousel and the images inside a
 * game description open the SAME viewer instead of each growing their own.
 *
 * `gamepad` is opt-in and defaults off: `useGamepad()` / `useFocusNavigation()`
 * start a requestAnimationFrame poll on first call, and the desktop surface has
 * no controller focus system to feed, so it must not pay for one.
 */
import { onUnmounted, ref, watch } from "vue";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  XMarkIcon,
} from "@heroicons/vue/20/solid";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useFocusNavigation } from "~/composables/focus-navigation";

const props = withDefaults(
  defineProps<{
    open: boolean;
    /** Resolved image URLs, already through `object://` where applicable. */
    srcs: string[];
    /** Which image to show when the viewer opens. */
    startIndex?: number;
    /** Prefix for each image's alt text, e.g. the game name. */
    altPrefix?: string;
    /** Big Picture only: controller nav + the modal input lock. */
    gamepad?: boolean;
  }>(),
  { startIndex: 0, altPrefix: "Image", gamepad: false },
);

const emit = defineEmits<{
  close: [];
  /** Fires on every move so an owning carousel can stay on the same image. */
  navigate: [index: number];
}>();

const index = ref(props.startIndex);

function close() {
  emit("close");
}

function goTo(i: number) {
  index.value = i;
  emit("navigate", i);
}

function next() {
  if (props.srcs.length === 0) return;
  goTo((index.value + 1) % props.srcs.length);
}

function previous() {
  if (props.srcs.length === 0) return;
  goTo((index.value - 1 + props.srcs.length) % props.srcs.length);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") close();
  else if (e.key === "ArrowRight") next();
  else if (e.key === "ArrowLeft") previous();
  else return;
  e.preventDefault();
}

// ── Big Picture controller wiring ────────────────────────────────────────
// Same shape as `BpmRetroArchCheatsheet`: take the input lock so the page's
// focus navigation stops competing, subscribe with `bypassInputLock` so our
// own handlers still fire, and give the lock back on close AND on unmount.
let lockId = 0;
const unsubs: (() => void)[] = [];

function wireGamepad() {
  unwireGamepad();
  const focusNav = useFocusNavigation();
  const gamepad = useGamepad();
  lockId = focusNav.acquireInputLock();
  const bypass = { bypassInputLock: true };
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadLeft, () => previous(), bypass),
    gamepad.onButton(GamepadButton.DPadRight, () => next(), bypass),
    gamepad.onButton(GamepadButton.East, () => close(), bypass),
  );
}

function unwireGamepad() {
  for (const u of unsubs) u();
  unsubs.length = 0;
  if (lockId) {
    useFocusNavigation().releaseInputLock(lockId);
    lockId = 0;
  }
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      index.value = Math.min(
        Math.max(props.startIndex, 0),
        Math.max(props.srcs.length - 1, 0),
      );
      window.addEventListener("keydown", onKeydown);
      if (props.gamepad) wireGamepad();
    } else {
      window.removeEventListener("keydown", onKeydown);
      unwireGamepad();
    }
  },
  { immediate: true },
);

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  unwireGamepad();
});
</script>

<style scoped>
.lightbox-slide-enter-active,
.lightbox-slide-leave-active {
  transition: all 0.3s ease;
  position: absolute;
}

.lightbox-slide-enter-from {
  opacity: 0;
  transform: translateX(100%);
}

.lightbox-slide-leave-to {
  opacity: 0;
  transform: translateX(-100%);
}
</style>
