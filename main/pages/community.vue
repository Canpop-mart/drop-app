<template>
  <div class="mx-auto max-w-6xl px-8 py-6">
    <div class="mb-6">
      <h1 class="text-2xl font-display font-bold text-zinc-100">Community</h1>
      <p class="mt-1 text-sm text-zinc-400">
        What everyone else on this Drop server is up to.
      </p>
    </div>

    <!-- Weekly recap carousel — hidden when server returns no slides -->
    <CommunityWeeklyRecap
      :slides="weeklyRecap"
      @go-to-game="goToGame"
      @go-to-user="goToUser"
    />

    <!-- Drop Time Machine — anniversary card, sibling of weekly recap. Hidden when null. -->
    <CommunityTimeMachine :event="timeMachine" />

    <!-- Personal weekly quest — hidden if endpoint returns null -->
    <CommunityWeeklyChallenge :challenge="weeklyChallenge" />

    <!-- Stats row -->
    <section v-if="stats" class="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-8">
      <div
        v-for="stat in statCards"
        :key="stat.label"
        class="rounded-xl bg-zinc-800/50 backdrop-blur-sm p-4 ring-1 ring-zinc-700/40"
      >
        <component :is="stat.icon" :class="['size-5 mb-2', stat.color]" />
        <p class="text-xl font-bold text-zinc-100 tabular-nums">
          {{ stat.value.toLocaleString() }}
        </p>
        <p class="text-xs text-zinc-500 uppercase tracking-wider mt-1">
          {{ stat.label }}
        </p>
      </div>
    </section>

    <!-- Around right now — hidden when no one's playing -->
    <CommunityNowPlayingStrip :entries="nowPlaying" @go-to-game="goToGame" />

    <!-- Game roulette — always rendered; component handles its own empty state. -->
    <CommunityRoulette
      :cover-pool="rouletteCoverPool"
      @select="onRouletteSelect"
    />

    <div class="grid grid-cols-1 lg:grid-cols-[2fr,1fr] gap-8">
      <!-- Activity feed -->
      <section>
        <h2 class="text-lg font-display font-semibold text-zinc-100 mb-4">
          Recent activity
        </h2>
        <div
          v-if="activityLoading"
          class="text-sm text-zinc-500 py-10 text-center"
        >
          Loading activity...
        </div>
        <div
          v-else-if="clusteredActivity.length === 0"
          class="text-sm text-zinc-500 py-10 text-center"
        >
          No recent activity to show.
        </div>
        <div v-else class="space-y-2">
          <CommunityActivityRow
            v-for="cluster in clusteredActivity"
            :key="cluster.key"
            :cluster="cluster"
            @go-to-game="goToGame"
          />
        </div>
      </section>

      <!-- Leaderboard -->
      <aside>
        <h2 class="text-lg font-display font-semibold text-zinc-100 mb-4">
          Top players (playtime)
        </h2>
        <div
          v-if="leaderboardLoading"
          class="text-sm text-zinc-500 py-10 text-center"
        >
          Loading leaderboard...
        </div>
        <div
          v-else-if="leaderboard.length === 0"
          class="text-sm text-zinc-500 py-10 text-center"
        >
          No players yet.
        </div>
        <div v-else class="space-y-2">
          <div
            v-for="entry in leaderboard.slice(0, 10)"
            :key="entry.user.id"
            class="flex items-center gap-x-3 rounded-lg bg-zinc-800/50 backdrop-blur-sm p-2.5 ring-1 ring-zinc-700/40"
          >
            <span
              class="size-6 rounded-full flex items-center justify-center text-xs font-bold shrink-0 tabular-nums"
              :class="rankColor(entry.rank)"
            >
              {{ entry.rank }}
            </span>
            <img
              v-if="entry.user.profilePictureObjectId"
              :src="objectUrl(entry.user.profilePictureObjectId)"
              class="size-7 rounded-full object-cover shrink-0"
            />
            <div
              v-else
              class="size-7 rounded-full bg-zinc-700 flex items-center justify-center shrink-0"
            >
              <UserIcon class="size-3.5 text-zinc-500" />
            </div>
            <div class="flex-1 min-w-0">
              <p
                class="text-sm font-medium text-zinc-200 truncate flex items-center gap-1"
              >
                <span class="truncate">{{
                  entry.user.displayName || entry.user.username
                }}</span>
                <span
                  v-if="mvp && entry.user.id === mvp.userId"
                  :title="mvpTooltip"
                  class="text-yellow-400 shrink-0 cursor-help"
                  aria-label="Today's MVP"
                  >👑</span
                >
              </p>
              <p class="text-[10px] text-zinc-500">
                {{ entry.playtimeHours.toLocaleString() }}h ·
                {{ entry.gamesPlayed }} games
              </p>
            </div>
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  UserGroupIcon,
  ClockIcon,
  CubeIcon,
  TrophyIcon,
  UserIcon,
} from "@heroicons/vue/24/solid";
import { invoke } from "@tauri-apps/api/core";
import {
  useServerApi,
  type CommunityStats,
  type CommunityActivityItem,
  type LeaderboardUser,
  type NowPlayingEntry,
  type WeeklyRecapSlide,
  type MvpToday,
  type TimeMachineEvent,
  type WeeklyChallenge,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { clusterActivity } from "~/composables/use-community-clusters";

useHead({ title: "Community" });

const router = useRouter();
const api = useServerApi();

const stats = ref<CommunityStats | null>(null);
const activity = ref<CommunityActivityItem[]>([]);
const leaderboard = ref<LeaderboardUser[]>([]);
const nowPlaying = ref<NowPlayingEntry[]>([]);
const weeklyRecap = ref<WeeklyRecapSlide[]>([]);
const mvp = ref<MvpToday | null>(null);
const timeMachine = ref<TimeMachineEvent | null>(null);
const weeklyChallenge = ref<WeeklyChallenge | null>(null);
const activityLoading = ref(true);
const leaderboardLoading = ref(true);

const clusteredActivity = computed(() => clusterActivity(activity.value));

// Cover pool for the roulette spin animation — covers from anything we
// already loaded for other surfaces, so we don't pay an extra fetch just
// to make the wheel look populated. De-duped + capped at 40.
const rouletteCoverPool = computed(() => {
  const pool = new Set<string>();
  for (const a of activity.value) {
    if (a.game?.mCoverObjectId) pool.add(a.game.mCoverObjectId);
  }
  for (const n of nowPlaying.value) {
    if (n.game?.coverObjectId) pool.add(n.game.coverObjectId);
  }
  return [...pool].slice(0, 40);
});

function onRouletteSelect(payload: { gameId: string; owned: boolean }) {
  // The roulette card has its own router.push; we only need to side-
  // effect the metadata prefetch so the destination page hydrates fast.
  // (Mirrors goToGame's invoke() pattern; safe to no-op on failure.)
  invoke("fetch_game", { gameId: payload.gameId }).catch(() => {});
}

const mvpTooltip = computed(() => {
  if (!mvp.value) return "";
  // Crude session count: server doesn't return it directly, but the
  // weight model (seconds + unlocks*600) means the tooltip is best
  // expressed in human terms — hours played + unlocks.
  const hours = Math.max(1, Math.round(mvp.value.sessionSeconds / 3600));
  const playLabel =
    mvp.value.sessionSeconds === 0 ? "no playtime" : `${hours}h playtime`;
  return `Today's MVP — ${playLabel} · ${mvp.value.achievementsUnlocked} achievement${mvp.value.achievementsUnlocked === 1 ? "" : "s"}`;
});

const statCards = computed(() => [
  {
    label: "Players",
    value: stats.value?.totalUsers ?? 0,
    icon: UserGroupIcon,
    color: "text-blue-400",
  },
  {
    label: "Games",
    value: stats.value?.totalGames ?? 0,
    icon: CubeIcon,
    color: "text-purple-400",
  },
  {
    label: "Hours played",
    value: stats.value?.totalPlaytimeHours ?? 0,
    icon: ClockIcon,
    color: "text-emerald-400",
  },
  {
    label: "Achievements unlocked",
    value: stats.value?.totalAchievementUnlocks ?? 0,
    icon: TrophyIcon,
    color: "text-yellow-400",
  },
]);

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId: string) {
  // Community is a discovery surface — clicking a game card from
  // someone else's activity feed should land on the store presentation,
  // not the management UI. Users who already own the game still get
  // there via the page's "Open in library" CTA.
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/store/${gameId}`);
}

function goToUser(_userId: string) {
  // The native desktop client doesn't currently surface per-user profile
  // pages; we just no-op here so weekly-recap slides that link to a user
  // don't blow up. The BPM surface and the server-rendered iframe
  // surface both have their own profile routes.
}

function rankColor(rank: number): string {
  if (rank === 1)
    return "bg-yellow-500/20 text-yellow-300 ring-1 ring-yellow-500/40";
  if (rank === 2) return "bg-zinc-400/20 text-zinc-200 ring-1 ring-zinc-400/40";
  if (rank === 3)
    return "bg-orange-700/30 text-orange-300 ring-1 ring-orange-700/40";
  return "bg-zinc-700/50 text-zinc-400";
}

onMounted(() => {
  // Independent fetches in parallel — each soft-fails so a single broken
  // endpoint can't blank the whole page.
  api.community
    .stats()
    .then((s) => (stats.value = s))
    .catch((e) => console.warn("[community] stats failed:", e));

  api.community
    .activity(30)
    .then((a) => (activity.value = a))
    .catch((e) => console.warn("[community] activity failed:", e))
    .finally(() => (activityLoading.value = false));

  api.community
    .leaderboard()
    .then((d) => (leaderboard.value = d.playtime))
    .catch((e) => console.warn("[community] leaderboard failed:", e))
    .finally(() => (leaderboardLoading.value = false));

  api.community
    .nowPlaying()
    .then((n) => (nowPlaying.value = n))
    .catch((e) => console.warn("[community] now-playing failed:", e));

  api.community
    .weeklyRecap()
    .then((w) => (weeklyRecap.value = w))
    .catch((e) => console.warn("[community] weekly-recap failed:", e));

  // MVP + Time Machine — both soft-fail to no UI render.
  api.community
    .mvpToday()
    .then((m) => (mvp.value = m))
    .catch((e) => console.warn("[community] mvp-today failed:", e));

  api.community
    .timeMachine()
    .then((t) => (timeMachine.value = t))
    .catch((e) => console.warn("[community] time-machine failed:", e));

  api.community
    .weeklyChallenge()
    .then((w) => (weeklyChallenge.value = w))
    .catch((e) => console.warn("[community] weekly-challenge failed:", e));
});
</script>
