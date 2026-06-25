<template>
  <section v-if="slides.length > 0" class="mb-8">
    <div
      class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-blue-900/40 via-zinc-900/60 to-blue-900/30 ring-1 ring-blue-500/20"
    >
      <!-- Slide stack. Each slide is its own absolutely-positioned card so
           the carousel fade can swap them; the kicker label and dot nav
           sit outside the transition so they don't flicker on rotate. -->
      <div class="relative h-32">
        <transition name="slide-fade" mode="out-in">
          <div
            v-if="activeSlide"
            :key="activeIndex"
            class="absolute inset-0 flex items-center gap-4 sm:gap-5 px-5 sm:px-6 py-4 cursor-pointer"
            @click="onSlideClick"
          >
            <!-- Thumbnail anchor — cover takes priority (game-led slides
                 like top_game / longest_session), avatar fallback when no
                 game is in scope (most_unlocks / milestone / new_player).
                 If neither is available we render an emoji glyph that
                 hints at the slide kind so the layout still has a visual
                 weight on the left edge. -->
            <div
              class="shrink-0 size-20 sm:size-24 rounded-xl overflow-hidden ring-1 ring-blue-400/20 flex items-center justify-center"
              :class="
                activeSlide.coverObjectId || activeSlide.avatarObjectId
                  ? 'bg-zinc-900/80'
                  : 'bg-blue-500/15'
              "
            >
              <img
                v-if="activeSlide.coverObjectId"
                :src="objectUrl(activeSlide.coverObjectId)"
                :alt="activeSlide.headline"
                class="w-full h-full object-cover"
              />
              <img
                v-else-if="activeSlide.avatarObjectId"
                :src="objectUrl(activeSlide.avatarObjectId)"
                :alt="activeSlide.headline"
                class="w-full h-full object-cover"
              />
              <component :is="slideIcon" v-else class="size-8 text-zinc-300" />
            </div>

            <!-- Text column: kicker → headline → meta. The kicker carries
                 the slide kind ("MOST PLAYED"), the headline is the thing
                 the slide is *about* (game name or player name) in display
                 type, and meta is the quieter supporting line. -->
            <div class="flex-1 min-w-0 pr-12">
              <p
                class="text-[10px] tracking-[0.2em] uppercase text-blue-300/80 font-medium mb-1 truncate"
              >
                {{ activeSlide.title }}
              </p>
              <p
                class="text-xl sm:text-2xl font-display font-bold text-zinc-100 leading-tight truncate"
              >
                {{ activeSlide.headline }}
              </p>
              <p
                class="text-xs sm:text-sm text-zinc-400 mt-0.5 truncate"
              >
                {{ activeSlide.meta }}
              </p>
            </div>
          </div>
        </transition>
      </div>

      <!-- Dot navigation — pinned to the bottom-right, sits outside the
           transition wrapper so a slide swap doesn't blink the dots. -->
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
              ? 'bg-blue-300 w-5'
              : 'bg-blue-300/30 hover:bg-blue-300/60'
          "
          :aria-label="`Slide ${i + 1}`"
          @click.stop="goTo(i)"
        />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import {
  FireIcon,
  ClockIcon,
  TrophyIcon,
  FlagIcon,
  UserPlusIcon,
  SparklesIcon,
} from "@heroicons/vue/24/solid";
import type { WeeklyRecapSlide } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

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

// Fallback icon per slide kind, shown only when the slide has neither a
// game cover nor a user avatar. Uses the same heroicon vocabulary as
// CommunityWeeklyChallenge so the two community cards stay consistent, and
// avoids emoji, which render differently across SteamOS and Windows.
const slideIcon = computed(() => {
  switch (activeSlide.value?.kind) {
    case "top_game":
      return FireIcon;
    case "longest_session":
      return ClockIcon;
    case "most_unlocks":
      return TrophyIcon;
    case "milestone":
      return FlagIcon;
    case "new_player":
      return UserPlusIcon;
    default:
      return SparklesIcon;
  }
});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

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
  // Game-anchored slides drill into the game; user-anchored slides emit
  // a go-to-user event the parent decides what to do with (desktop no-
  // ops, BPM can route to a profile).
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
