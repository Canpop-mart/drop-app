<!--
  Wrapped deck chrome — a click-through of story cards with a header directory,
  prev/next arrows (+ arrow keys), story-style progress bars and a window
  toggle. Themed from a profile accent (via use-profile-theme). The cards
  themselves come from the page as a scoped slot, keyed by the active card, so
  each Wrapped screen (personal / community) owns its own card layouts.
-->
<template>
  <div class="min-h-full bg-zinc-950 text-zinc-100" :style="vars">
    <div class="mx-auto max-w-5xl px-6 py-6">
      <div class="mb-6 flex flex-wrap items-center gap-3">
        <button
          class="inline-flex items-center gap-1 rounded-lg px-3 py-2 text-sm font-medium text-zinc-400 ring-1 ring-zinc-800 transition-colors hover:bg-zinc-900 hover:text-zinc-100"
          @click="$emit('close')"
        >
          <ChevronLeftIcon class="size-4" />
          Back
        </button>
        <div class="min-w-0">
          <div
            class="text-[11px] font-semibold uppercase tracking-[0.18em]"
            :style="{ color: 'var(--accent)' }"
          >
            Wrapped
          </div>
          <h1 class="truncate font-display text-xl font-bold">{{ title }}</h1>
        </div>
        <div
          class="ml-auto flex rounded-lg bg-zinc-900 p-1 text-xs font-semibold"
        >
          <button
            v-for="w in WINDOWS"
            :key="w.key"
            class="rounded-md px-3 py-1.5 transition-colors"
            :class="window === w.key ? '' : 'text-zinc-400 hover:text-zinc-200'"
            :style="
              window === w.key
                ? { background: 'var(--accent)', color: 'var(--accent-contrast)' }
                : {}
            "
            @click="$emit('update:window', w.key)"
          >
            {{ w.label }}
          </button>
        </div>
      </div>

      <div
        v-if="loading"
        class="flex min-h-[50vh] items-center justify-center"
      >
        <div
          class="size-5 animate-spin rounded-full border-2 border-zinc-700 border-t-zinc-300"
        />
      </div>
      <div
        v-else-if="cards.length === 0"
        class="flex min-h-[40vh] flex-col items-center justify-center text-center"
      >
        <p class="text-sm text-zinc-400">Nothing to wrap for this window yet.</p>
        <p class="mt-1 text-xs text-zinc-600">
          Try a wider window, or come back after some more playtime.
        </p>
      </div>

      <div v-else class="grid items-start gap-6 md:grid-cols-[196px_1fr]">
        <aside class="hidden md:block">
          <div
            class="mb-2 text-[10px] font-bold uppercase tracking-[0.14em] text-zinc-500"
          >
            Cards
          </div>
          <div class="flex flex-col gap-0.5">
            <button
              v-for="(c, i) in cards"
              :key="c.key"
              class="flex items-center gap-2 rounded-lg px-3 py-2 text-left text-sm transition-colors"
              :class="
                i === index
                  ? 'font-semibold text-zinc-100'
                  : 'text-zinc-400 hover:bg-zinc-900'
              "
              :style="i === index ? { background: 'var(--accent-soft)' } : {}"
              @click="index = i"
            >
              <span
                class="w-4 text-xs tabular-nums"
                :style="{ color: i === index ? 'var(--accent)' : undefined }"
                >{{ i + 1 }}</span
              >
              <span class="truncate">{{ c.title }}</span>
            </button>
          </div>
        </aside>

        <div class="min-w-0">
          <div class="mb-3 flex gap-1.5">
            <i
              v-for="(c, i) in cards"
              :key="c.key"
              class="h-[3px] flex-1 rounded-full transition-colors"
              :style="{ background: i <= index ? 'var(--accent)' : '#27272a' }"
            />
          </div>
          <div class="relative">
            <button
              v-if="index > 0"
              class="deck-nav"
              style="left: -14px"
              aria-label="Previous"
              @click="index--"
            >
              <ChevronLeftIcon class="size-5" />
            </button>
            <button
              v-if="index < cards.length - 1"
              class="deck-nav"
              style="right: -14px"
              aria-label="Next"
              @click="index++"
            >
              <ChevronRightIcon class="size-5" />
            </button>
            <div v-if="active" :key="active.key" class="deck-fade">
              <slot :card="active" :index="index" />
            </div>
          </div>
          <div class="mt-3 text-center text-xs tabular-nums text-zinc-500">
            {{ index + 1 }} / {{ cards.length }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/vue/24/solid";
import { useProfileTheme } from "~/composables/use-profile-theme";

const props = defineProps<{
  cards: { key: string; title: string }[];
  window: string;
  theme?: string | null;
  title: string;
  loading?: boolean;
}>();

defineEmits<{ (e: "close"): void; (e: "update:window", w: string): void }>();

const WINDOWS = [
  { key: "all", label: "All" },
  { key: "year", label: "Year" },
  { key: "month", label: "Month" },
  { key: "week", label: "Week" },
];

const { vars } = useProfileTheme(() => props.theme);
const index = ref(0);
const active = computed(() => props.cards[index.value] ?? props.cards[0]);

// Reset to the first card when the set changes (e.g. a window switch drops or
// reorders cards).
watch(
  () => props.cards.map((c) => c.key).join("|"),
  () => {
    index.value = 0;
  },
);

function onKey(e: KeyboardEvent) {
  if (e.key === "ArrowRight" && index.value < props.cards.length - 1)
    index.value++;
  else if (e.key === "ArrowLeft" && index.value > 0) index.value--;
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<style scoped>
.deck-nav {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 9999px;
  color: #fafafa;
  cursor: pointer;
  background: rgba(10, 10, 12, 0.7);
  border: 1px solid rgba(63, 63, 70, 0.5);
  backdrop-filter: blur(6px);
}
.deck-nav:hover {
  background: #18181b;
}
.deck-fade {
  animation: deckfade 0.32s ease;
}
@keyframes deckfade {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
</style>
