<template>
  <!-- This component is now rendered inside a CollapsibleSection
       on the library detail page, which already carries the
       "Achievements" header + progress count.  Stripping the duplicate
       header + outer card wrapper keeps the column tidy and lets the
       parent own the surrounding chrome. -->
  <div>
    <!-- ROM hash status banner (RetroAchievements). -->
    <div
      v-if="romHashResult?.status === 'Mismatch'"
      class="mb-4 rounded-lg bg-amber-500/10 p-3 outline outline-1 outline-amber-500/20"
    >
      <p class="text-sm font-medium text-amber-400 mb-1">
        ROM not recognised by RetroAchievements
      </p>
      <p class="text-xs text-zinc-400 mb-2">
        Your ROM hash
        (<code class="text-zinc-300"
          >{{ romHashResult.rom_hash?.slice(0, 12) }}…</code
        >) doesn't match any known hash. Achievements won't track until the
        ROM is patched or replaced.
      </p>
      <div
        v-if="romHashResult.expected_hashes?.some((h) => h.patchUrl)"
        class="flex flex-wrap gap-2"
      >
        <a
          v-for="h in romHashResult.expected_hashes?.filter((h) => h.patchUrl)"
          :key="h.hash"
          :href="h.patchUrl"
          target="_blank"
          class="inline-flex items-center gap-1 rounded bg-amber-500/20 px-2 py-0.5 text-xs text-amber-300 hover:bg-amber-500/30 transition-colors"
        >
          Patch: {{ h.label || h.hash.slice(0, 8) }}
        </a>
      </div>
    </div>
    <div
      v-else-if="romHashResult?.status === 'Match'"
      class="mb-4 rounded-lg bg-emerald-500/10 p-2 outline outline-1 outline-emerald-500/20"
    >
      <p class="text-xs text-emerald-400">
        ROM verified — matches RetroAchievements
        <span v-if="romHashResult.matched_label" class="text-zinc-400">
          ({{ romHashResult.matched_label }})
        </span>
      </p>
    </div>
    <div
      v-else-if="romHashResult?.status === 'Error'"
      class="mb-4 rounded-lg bg-red-500/10 p-2 outline outline-1 outline-red-500/20"
    >
      <p class="text-xs text-red-400">
        Hash check failed: {{ romHashResult.message }}
      </p>
    </div>

    <!-- Loading / empty / list. -->
    <div v-if="loading" class="flex justify-center py-4">
      <div
        class="w-5 h-5 border-2 border-zinc-600 border-t-zinc-100 rounded-full animate-spin"
      />
    </div>
    <div
      v-else-if="achievements.length === 0"
      class="flex flex-col items-center justify-center text-center py-4"
    >
      <TrophyIcon class="size-10 text-zinc-600 mb-2" />
      <p class="text-zinc-500 text-sm">No achievements available</p>
    </div>
    <!-- No inner max-height — the CollapsibleSection wrapper now
         provides the show/hide affordance, so capping list height
         here only adds a redundant inner scrollbar. The full list
         expands naturally and users collapse the whole section if
         it gets long. -->
    <div v-else class="space-y-1">
      <div class="flex items-center justify-between mb-2">
        <span class="text-xs text-zinc-400">
          {{ unlockedCount }} / {{ achievements.length }} unlocked
          <span v-if="totalPoints > 0" class="text-amber-400/80"
            >· {{ earnedPoints }} / {{ totalPoints }} pts</span
          >
        </span>
        <div
          class="flex-1 ml-3 h-1.5 bg-zinc-700 rounded-full overflow-hidden"
        >
          <div
            class="h-full bg-yellow-500 rounded-full transition-all"
            :style="{ width: `${unlockedPercent}%` }"
          />
        </div>
      </div>
      <div
        v-for="ach in achievements"
        :key="ach.id"
        class="flex items-center gap-3 py-2 px-2 rounded-lg hover:bg-zinc-700/30 transition-colors"
      >
        <img
          v-if="ach.iconUrl"
          :src="ach.iconUrl"
          :class="[
            'size-9 rounded shrink-0',
            ach.unlocked ? '' : 'grayscale opacity-50',
            firsts[ach.id] ? 'ring-2 ring-yellow-500/70' : '',
          ]"
        />
        <div
          v-else
          :class="[
            'size-9 rounded shrink-0 bg-zinc-700/50 flex items-center justify-center',
            ach.unlocked ? '' : 'opacity-50',
            firsts[ach.id] ? 'ring-2 ring-yellow-500/70' : '',
          ]"
        >
          <TrophyIcon class="size-5 text-zinc-500" />
        </div>
        <div class="flex-1 min-w-0">
          <p
            :class="[
              'text-sm font-medium truncate',
              ach.unlocked ? 'text-zinc-100' : 'text-zinc-500',
            ]"
          >
            {{ ach.title }}
          </p>
          <p class="text-xs text-zinc-500 truncate">
            {{ ach.description }}
          </p>
          <p
            v-if="rarityLabel(ach) || ach.points"
            class="text-[11px] text-zinc-600 flex items-center gap-2 mt-0.5"
          >
            <span v-if="rarityLabel(ach)">{{ rarityLabel(ach) }} of players</span>
            <span v-if="ach.points" class="text-amber-400/80"
              >{{ ach.points }} pts</span
            >
          </p>
          <!-- Server-first badge — only renders when this achievement
               appears in the firsts map. Uses the shared component. -->
          <GameAchievementFirstBadge
            v-if="firsts[ach.id]"
            :first="firsts[ach.id]"
            class="mt-0.5"
          />
        </div>
        <div v-if="ach.unlocked" class="shrink-0">
          <CheckCircleIcon class="size-4 text-yellow-500" />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Achievements tab for the library game-detail page: the RetroAchievements
 * ROM-hash status banner and the achievement list. Purely presentational;
 * data + reset action come from `useGameStats` on the parent.
 */
import { CheckCircleIcon, TrophyIcon } from "@heroicons/vue/24/solid";
import GameAchievementFirstBadge from "~/components/GameAchievementFirstBadge.vue";
import type {
  AchievementData,
  RomHashResult,
} from "~/composables/game-detail/use-game-stats";
import type { GameAchievementFirst } from "~/composables/use-server-api";

const props = defineProps<{
  achievements: AchievementData[];
  loading: boolean;
  unlockedCount: number;
  romHashResult: RomHashResult | null;
  /** Map of achievementId -> "first to unlock" record. Provided by the
   *  page-level fetch of `community.gameFirsts(gameId)`. Defaults to {} so
   *  the badge logic is a no-op when the endpoint hasn't shipped. */
  firstsMap?: Record<string, GameAchievementFirst>;
}>();

const firsts = computed<Record<string, GameAchievementFirst>>(
  () => props.firstsMap ?? {},
);

const unlockedPercent = computed(() =>
  props.achievements.length > 0
    ? (props.unlockedCount / props.achievements.length) * 100
    : 0,
);

// Gamerscore-style points (RetroAchievements). Summed from the list, which
// already carries the max points across provider variants. Hidden when 0
// (Steam/Goldberg-only games carry no points).
const totalPoints = computed(() =>
  props.achievements.reduce((sum, a) => sum + (a.points ?? 0), 0),
);
const earnedPoints = computed(() =>
  props.achievements.reduce(
    (sum, a) => sum + (a.unlocked ? (a.points ?? 0) : 0),
    0,
  ),
);

/** Preferred rarity label: global (RA/Steam) % if known, else this server's. */
function rarityLabel(ach: AchievementData): string | null {
  const pct = ach.globalPercent ?? ach.rarity ?? null;
  if (pct === null || pct === undefined) return null;
  return `${Math.round(pct * 10) / 10}%`;
}
</script>

<style scoped>
.custom-scrollbar {
  scrollbar-width: thin;
  scrollbar-color: rgb(82 82 91) transparent;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: rgb(82 82 91);
  border-radius: 3px;
}
</style>
