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
      <!-- Compare mode: one compact summary line — small avatars, counts,
           percents (you gold, them blue), then the shared / only-you /
           only-them breakdown pushed right. The per-row ownership bars
           below carry the achievement-by-achievement detail. -->
      <div
        v-if="compare"
        class="mb-3 rounded-lg bg-zinc-800/40 px-3 py-2 ring-1 ring-zinc-700/40"
      >
        <div class="flex flex-wrap items-center gap-x-4 gap-y-1.5 text-xs">
          <span class="flex min-w-0 items-center gap-1.5">
            <Avatar :object-id="youAvatarObjectId" :name="youLabel" :size="18" />
            <span class="truncate font-medium text-zinc-300">{{ youLabel }}</span>
            <span class="font-semibold tabular-nums text-zinc-100">{{
              unlockedCount
            }}</span>
            <span class="tabular-nums text-yellow-400"
              >· {{ Math.round(unlockedPercent) }}%</span
            >
          </span>
          <span class="flex min-w-0 items-center gap-1.5">
            <Avatar
              :object-id="compare.avatarObjectId"
              :name="compare.name"
              :size="18"
            />
            <span
              class="truncate font-medium text-zinc-300"
              :title="compare.name"
              >{{ compare.name }}</span
            >
            <span class="font-semibold tabular-nums text-zinc-100">{{
              compare.unlockedCount
            }}</span>
            <span class="tabular-nums text-blue-400"
              >· {{ Math.round(comparePercent) }}%</span
            >
          </span>
          <span
            class="ml-auto flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-zinc-400"
          >
            <span class="flex items-center gap-1.5">
              <span class="size-1.5 rounded-full bg-emerald-400" />
              <span class="tabular-nums text-zinc-200">{{ bothCount }}</span>
              shared
            </span>
            <span class="flex items-center gap-1.5">
              <span class="size-1.5 rounded-full bg-yellow-500" />
              <span class="tabular-nums text-zinc-200">{{ onlyYouCount }}</span>
              only you
            </span>
            <span class="flex items-center gap-1.5">
              <span class="size-1.5 rounded-full bg-blue-400" />
              <span class="tabular-nums text-zinc-200">{{ onlyThemCount }}</span>
              only them
            </span>
          </span>
        </div>
      </div>
      <!-- Solo mode: single progress bar. -->
      <div v-else class="flex items-center justify-between mb-2">
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
        class="flex items-center gap-3 py-2 px-2 rounded-lg transition-colors hover:bg-zinc-700/30"
      >
        <img
          v-if="ach.iconUrl && !iconErrors[ach.id]"
          :src="ach.iconUrl"
          :class="[
            'size-9 rounded shrink-0',
            ach.unlocked ? '' : 'grayscale opacity-50',
            firsts[ach.id] ? 'ring-2 ring-yellow-500/70' : '',
          ]"
          @error="iconErrors[ach.id] = true"
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
          <div class="flex items-center gap-1 min-w-0">
            <p
              :class="[
                'text-sm font-medium truncate',
                ach.unlocked ? 'text-zinc-100' : 'text-zinc-500',
              ]"
            >
              {{ ach.title }}
            </p>
            <!-- Server-first marker — small gold trophy inline after the
                 title; who/when live in its tooltip. -->
            <GameAchievementFirstBadge
              v-if="firsts[ach.id]"
              :first="firsts[ach.id]"
              class="shrink-0"
            />
          </div>
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
          <!-- Compare: ownership bar. Left half gold = you have it, right
               half blue = they do; the two meet in the middle when you both
               do, and the muted track shows through where a side is missing. -->
          <div
            v-if="compare"
            class="mt-1.5 flex h-1.5 w-full overflow-hidden rounded-full bg-zinc-700/50"
            :title="`${youLabel}: ${ach.unlocked ? 'unlocked' : 'locked'} · ${compare.name}: ${compareSet.has(ach.id) ? 'unlocked' : 'locked'}`"
          >
            <div
              class="h-full w-1/2"
              :class="ach.unlocked ? 'bg-yellow-500' : 'bg-transparent'"
            />
            <div
              class="h-full w-1/2"
              :class="compareSet.has(ach.id) ? 'bg-blue-500' : 'bg-transparent'"
            />
          </div>
        </div>
        <!-- Solo mode: single unlock check. Compare mode carries the
             you-vs-them signal in the per-row ownership bar instead. -->
        <div v-if="!compare && ach.unlocked" class="shrink-0">
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
import Avatar from "~/components/Avatar.vue";
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
  /** When set, renders a Steam-style side-by-side comparison. `unlockedIds`
   *  are the OTHER user's unlocked achievement ids (same ids as this list —
   *  both sides come from the shared server computation). */
  compare?: {
    name: string;
    unlockedIds: string[];
    unlockedCount: number;
    avatarObjectId?: string | null;
  } | null;
  /** Label + avatar for the "You" side of the compare panel. */
  youName?: string;
  youAvatarObjectId?: string | null;
}>();

// Icon error tracking — swap to the trophy fallback when a URL 404s (e.g.
// Goldberg stores a crack-local icon path that doesn't resolve on the web).
const iconErrors = reactive<Record<string, boolean>>({});

const youLabel = computed(() => props.youName ?? "You");

const firsts = computed<Record<string, GameAchievementFirst>>(
  () => props.firstsMap ?? {},
);

const compareSet = computed(() => new Set(props.compare?.unlockedIds ?? []));

// Compare breakdown: how the two sets overlap. `unlocked` is our side, the
// compareSet is theirs — computed here so the panel can show shared / gap
// counts and each row can tint toward whoever's ahead on it.
const bothCount = computed(() =>
  props.compare
    ? props.achievements.filter(
        (a) => a.unlocked && compareSet.value.has(a.id),
      ).length
    : 0,
);
const onlyYouCount = computed(() =>
  props.compare
    ? props.achievements.filter(
        (a) => a.unlocked && !compareSet.value.has(a.id),
      ).length
    : 0,
);
const onlyThemCount = computed(() =>
  props.compare
    ? props.achievements.filter(
        (a) => !a.unlocked && compareSet.value.has(a.id),
      ).length
    : 0,
);

const unlockedPercent = computed(() =>
  props.achievements.length > 0
    ? (props.unlockedCount / props.achievements.length) * 100
    : 0,
);
const comparePercent = computed(() =>
  props.compare && props.achievements.length > 0
    ? (props.compare.unlockedCount / props.achievements.length) * 100
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
