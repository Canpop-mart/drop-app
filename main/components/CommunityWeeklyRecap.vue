<template>
  <section v-if="slides.length > 0" class="mb-8">
    <div
      class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-indigo-900/40 via-zinc-900/60 to-purple-900/30 ring-1 ring-indigo-500/20"
    >
      <!-- Decorative corner tag -->
      <div
        class="absolute top-3 left-4 text-[10px] tracking-[0.2em] uppercase text-indigo-300/80 font-medium z-10"
      >
        This week
      </div>

      <!-- Slide stack -->
      <div class="relative h-32 sm:h-28">
        <transition name="slide-fade" mode="out-in">
          <div
            :key="activeIndex"
            class="absolute inset-0 px-6 py-7 sm:py-6 pl-6 sm:pl-8 pr-12 flex flex-col justify-center cursor-pointer"
            @click="onSlideClick"
          >
            <p class="text-xs font-medium text-indigo-300 mb-1">
              {{ activeSlide?.title }}
            </p>
            <p
              class="text-base sm:text-lg font-display font-semibold text-zinc-100 leading-snug truncate"
            >
              {{ activeSlide?.subtitle }}
            </p>
          </div>
        </transition>
      </div>

      <!-- Dot navigation -->
      <div
        v-if="slides.length > 1"
        class="absolute bottom-3 right-4 flex gap-1.5 z-10"
      >
        <button
          v-for="(_, i) in slides"
          :key="i"
          class="size-2 rounded-full transition-all"
          :class="
            i === activeIndex
              ? 'bg-indigo-300 w-5'
              : 'bg-indigo-300/30 hover:bg-indigo-300/60'
          "
          :aria-label="`Slide ${i + 1}`"
          @click.stop="goTo(i)"
        />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { WeeklyRecapSlide } from "~/composables/use-server-api";

const props = defineProps<{
  slides: WeeklyRecapSlide[];
}>();

const emit = defineEmits<{
  (e: "go-to-game", gameId: string): void;
  (e: "go-to-user", userId: string): void;
}>();

const activeIndex = ref(0);
const activeSlide = computed(
  () => props.slides[activeIndex.value] ?? props.slides[0],
);

let timer: ReturnType<typeof setInterval> | null = null;

function rotate() {
  if (props.slides.length <= 1) return;
  activeIndex.value = (activeIndex.value + 1) % props.slides.length;
}

function startTimer() {
  stopTimer();
  if (props.slides.length > 1) {
    timer = setInterval(rotate, 8000);
  }
}

function stopTimer() {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
}

function goTo(i: number) {
  activeIndex.value = i;
  startTimer(); // reset cadence after manual nav
}

function onSlideClick() {
  const slide = activeSlide.value;
  if (!slide) return;
  if (slide.gameId) emit("go-to-game", slide.gameId);
  else if (slide.userId) emit("go-to-user", slide.userId);
}

onMounted(startTimer);
onUnmounted(stopTimer);

watch(
  () => props.slides.length,
  () => {
    activeIndex.value = 0;
    startTimer();
  },
);
</script>

<style scoped>
.slide-fade-enter-active,
.slide-fade-leave-active {
  transition:
    opacity 350ms ease,
    transform 350ms ease;
}
.slide-fade-enter-from {
  opacity: 0;
  transform: translateY(6px);
}
.slide-fade-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}
</style>
