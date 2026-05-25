<template>
  <section class="mb-8">
    <div class="flex items-baseline gap-2 mb-3">
      <h2 class="text-lg font-display font-semibold text-zinc-100">
        Game roulette
      </h2>
      <span class="text-xs text-zinc-500">Can't decide? Let fate pick.</span>
    </div>

    <div
      class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-fuchsia-900/30 via-zinc-900/60 to-cyan-900/30 ring-1 ring-fuchsia-500/20 p-4 sm:p-5"
    >
      <div class="flex items-center gap-5">
        <!-- Cover slot ─ either spinning preview or settled pick. -->
        <button
          type="button"
          class="group relative shrink-0 size-28 sm:size-32 rounded-xl overflow-hidden bg-zinc-800 ring-1 ring-fuchsia-500/30 transition-transform"
          :class="{
            'cursor-default': spinning || (!result && !empty),
            'cursor-pointer hover:scale-[1.02] hover:ring-fuchsia-400/60':
              !spinning && result,
            'roulette-tada': settled,
          }"
          :disabled="spinning || (!result && !empty)"
          @click="onCoverClick"
        >
          <img
            v-if="currentCoverId"
            :src="objectUrl(currentCoverId)"
            class="w-full h-full object-cover"
            :class="{ 'roulette-blur': spinning }"
            loading="lazy"
          />
          <div
            v-else
            class="w-full h-full flex items-center justify-center text-zinc-600"
          >
            <SparklesIcon class="size-10" />
          </div>
        </button>

        <!-- Caption + spin button -->
        <div class="flex-1 min-w-0">
          <p
            v-if="!hasInteracted"
            class="text-sm text-zinc-300 leading-snug max-w-md"
          >
            Spin to land on something to play right now.
          </p>
          <template v-else-if="result">
            <p
              class="text-base sm:text-lg font-display font-semibold text-zinc-100 leading-snug truncate"
            >
              {{ result.game.name }}
            </p>
            <p class="text-xs text-fuchsia-300 mt-0.5">
              {{ caption }}
            </p>
          </template>
          <p v-else-if="empty" class="text-sm text-zinc-400 leading-snug">
            Nothing to spin up yet. Install a game or wait for someone else to
            play one — we'll have a pick for you soon.
          </p>
          <p v-else class="text-sm text-zinc-400 leading-snug">Spinning...</p>

          <div class="mt-3 flex gap-2">
            <button
              type="button"
              class="inline-flex items-center gap-2 rounded-lg bg-fuchsia-500/20 px-4 py-2 text-sm font-medium text-fuchsia-200 ring-1 ring-fuchsia-500/40 transition-colors hover:bg-fuchsia-500/30 disabled:cursor-not-allowed disabled:opacity-50"
              :disabled="spinning"
              @click="spin"
            >
              <ArrowPathIcon
                class="size-4"
                :class="{ 'animate-spin': spinning }"
              />
              {{ spinning ? "Spinning" : hasInteracted ? "Spin again" : "Spin" }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ArrowPathIcon, SparklesIcon } from "@heroicons/vue/24/solid";
import {
  useServerApi,
  type RouletteResult,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

const props = defineProps<{
  /** Optional pool of cover IDs to cycle through during the spin animation. */
  coverPool?: string[];
}>();

const emit = defineEmits<{
  (e: "select", payload: { gameId: string; owned: boolean }): void;
}>();

const api = useServerApi();
const router = useRouter();

const spinning = ref(false);
const result = ref<RouletteResult | null>(null);
const empty = ref(false);
const hasInteracted = ref(false);
const settled = ref(false);
const previewCoverId = ref<string | null>(null);

let cycleTimer: ReturnType<typeof setInterval> | null = null;
let tadaTimer: ReturnType<typeof setTimeout> | null = null;

const currentCoverId = computed(() => {
  if (spinning.value) return previewCoverId.value;
  return result.value?.game.coverObjectId ?? null;
});

const caption = computed(() => {
  if (!result.value) return "";
  switch (result.value.source) {
    case "rediscovery":
      return "You haven't played in a while";
    case "library":
      return "From your library";
    case "social": {
      const n = result.value.alsoPlayedBy?.length ?? 0;
      if (n === 0) return "Played by others on this server";
      if (n === 1)
        return `${result.value.alsoPlayedBy![0].displayName} has played this`;
      return `${n} friends have played this`;
    }
    default:
      return "";
  }
});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function pickRandomPreviewCover(): string | null {
  const pool = props.coverPool ?? [];
  if (pool.length === 0) return previewCoverId.value;
  return pool[Math.floor(Math.random() * pool.length)] ?? null;
}

async function spin() {
  if (spinning.value) return;
  hasInteracted.value = true;
  empty.value = false;
  settled.value = false;
  spinning.value = true;

  // ── Animation: cycle 8–12 covers at ~80ms intervals while the fetch
  // races. We always run the animation for at least its minimum duration
  // so a sub-100ms fetch doesn't make the spin feel fake.
  const cycleMs = 80;
  const minCycles = 8;
  const maxCycles = 12;
  const targetCycles =
    minCycles + Math.floor(Math.random() * (maxCycles - minCycles + 1));
  const minDurationMs = targetCycles * cycleMs;

  if (cycleTimer) clearInterval(cycleTimer);
  cycleTimer = setInterval(() => {
    previewCoverId.value = pickRandomPreviewCover();
  }, cycleMs);

  const [fetched] = await Promise.all([
    api.community.roulette().catch((e) => {
      console.warn("[roulette] fetch failed:", e);
      return null;
    }),
    new Promise((r) => setTimeout(r, minDurationMs)),
  ]);

  if (cycleTimer) {
    clearInterval(cycleTimer);
    cycleTimer = null;
  }

  spinning.value = false;
  if (fetched) {
    result.value = fetched;
    empty.value = false;
    settled.value = true;
    if (tadaTimer) clearTimeout(tadaTimer);
    tadaTimer = setTimeout(() => {
      settled.value = false;
    }, 700);
  } else {
    result.value = null;
    empty.value = true;
  }
}

function onCoverClick() {
  if (!result.value || spinning.value) return;
  const owned =
    result.value.source === "rediscovery" || result.value.source === "library";
  const gameId = result.value.game.id;
  emit("select", { gameId, owned });
  // Default behavior — page hosts can override by handling @select and
  // calling preventDefault before navigation. Keeping it inline here so
  // the component works standalone.
  router.push(owned ? `/library/${gameId}` : `/store/${gameId}`);
}

onUnmounted(() => {
  if (cycleTimer) clearInterval(cycleTimer);
  if (tadaTimer) clearTimeout(tadaTimer);
});
</script>

<style scoped>
.roulette-blur {
  filter: brightness(0.85) saturate(1.1);
  transition: filter 80ms ease-out;
}

@keyframes roulette-tada {
  0% {
    transform: scale(1);
  }
  30% {
    transform: scale(1.08);
  }
  60% {
    transform: scale(0.97);
  }
  100% {
    transform: scale(1);
  }
}

.roulette-tada {
  animation: roulette-tada 700ms cubic-bezier(0.34, 1.56, 0.64, 1);
}
</style>
