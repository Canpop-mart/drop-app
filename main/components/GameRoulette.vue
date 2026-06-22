<template>
  <!--
    Expanded layout: the roulette used to share a row with the weekly
    quest as a small inline strip. With the quest hidden it now takes
    the full row, so we lean into a bigger cover thumbnail (h-24 w-16
    instead of size-9) and slightly more breathing room. The base
    surface still matches activity rows; only the cover and spin
    button carry colour.
  -->
  <div
    class="rounded-xl bg-zinc-800/50 ring-1 ring-zinc-700/40 px-5 py-5 hover:ring-purple-500/40 transition"
    :class="{ 'roulette-spinning-surface': spinning }"
  >
    <div class="flex items-center gap-5">
      <!-- Cover slot — rectangular thumbnail so a real cover reads
           better than a circular crop. Static state shows a sparkle;
           spinning state cycles covers from the pool; settled state
           keeps the chosen cover. -->
      <button
        type="button"
        class="group relative shrink-0 h-24 w-16 rounded-md overflow-hidden flex items-center justify-center transition-transform"
        :class="[
          spinning || result
            ? 'bg-zinc-900 ring-1 ring-purple-500/30'
            : 'bg-purple-500/15 ring-1 ring-purple-400/30',
          {
            'cursor-default': spinning || (!result && !empty),
            'cursor-pointer hover:scale-[1.03] hover:ring-purple-400/60':
              !spinning && result,
            'roulette-tada': settled,
          },
        ]"
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
        <SparklesIcon v-else class="size-7 text-purple-300" />
      </button>

      <div class="flex-1 min-w-0">
        <template v-if="!hasInteracted">
          <p class="text-base font-display font-semibold text-zinc-100">
            Can't decide?
          </p>
          <p class="text-sm text-zinc-500 mt-0.5">
            Spin to land on a random game from across the Drop catalog —
            your library, your backlog, or something brand new.
          </p>
        </template>
        <template v-else-if="result">
          <p class="text-base font-display font-semibold text-zinc-100 truncate">
            {{ result.game.name }}
          </p>
          <p class="text-xs text-purple-300/80 truncate mt-0.5">
            {{ caption }}
          </p>
        </template>
        <template v-else-if="empty">
          <p class="text-base font-display font-semibold text-zinc-100">
            Nothing to spin yet
          </p>
          <p class="text-sm text-zinc-500 mt-0.5">
            No games on the Drop catalog yet.
          </p>
        </template>
        <template v-else>
          <p class="text-base font-display font-semibold text-zinc-100">
            Spinning...
          </p>
          <p class="text-sm text-zinc-500 mt-0.5">
            Picking something for you.
          </p>
        </template>
      </div>

      <button
        type="button"
        class="shrink-0 inline-flex items-center gap-1.5 rounded-md bg-purple-500/20 ring-1 ring-purple-400/40 px-4 py-2 text-sm font-semibold text-purple-100 hover:bg-purple-500/30 transition disabled:cursor-not-allowed disabled:opacity-50"
        :disabled="spinning"
        @click="spin"
      >
        <ArrowPathIcon class="size-4" :class="{ 'animate-spin': spinning }" />
        {{ spinning ? "Spinning" : hasInteracted ? "Spin again" : "Spin" }}
      </button>
    </div>
  </div>
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
    case "social":
    case "discover": {
      // Both branches mean "the caller doesn't own this" — the only
      // difference is provenance (social = ≥2 server users; discover =
      // any catalog game). The caption is identical and reads off the
      // same alsoPlayedBy enrichment when present.
      const n = result.value.alsoPlayedBy?.length ?? 0;
      if (n === 0) return "From the Drop catalog, give it a try";
      if (n === 1)
        return `${result.value.alsoPlayedBy![0].displayName} has played this`;
      return `${n} on this server have played this`;
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
  // `rediscovery` and `library` are caller-owned (always installed);
  // `social` and `discover` are not — route the latter to the store
  // presentation so the user lands on Install rather than Play.
  const owned =
    result.value.source === "rediscovery" || result.value.source === "library";
  const gameId = result.value.game.id;
  emit("select", { gameId, owned });
  router.push(owned ? `/library/${gameId}` : `/store/${gameId}`);
}

onUnmounted(() => {
  if (cycleTimer) clearInterval(cycleTimer);
  if (tadaTimer) clearTimeout(tadaTimer);
});
</script>

<style scoped>
/* While the wheel is spinning we ghost in a soft purple tint so the
   neutral surface has a hint of motion. Static + settled states stay
   purely neutral, matching the activity rows. */
.roulette-spinning-surface {
  background-color: rgb(76 29 149 / 0.08);
}

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
