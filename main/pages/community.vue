<template>
  <main class="mx-auto max-w-[1400px] px-8 py-6 space-y-5">
    <!-- Header — title + slim inline stats strip on the right. The fat
         4-card stat grid this page used to lead with has been collapsed
         into this strip so the hero (weekly recap) is the first loud
         element on the page. -->
    <header class="flex items-end justify-between gap-4">
      <div>
        <h1 class="text-2xl font-display font-bold text-zinc-100">Community</h1>
        <p class="text-sm text-zinc-500 mt-0.5">
          What everyone else on this Drop server is up to.
        </p>
      </div>
      <div
        v-if="stats"
        class="flex items-center gap-2 text-sm text-zinc-400 font-medium shrink-0"
      >
        <span class="flex items-center gap-1.5">
          <span class="size-1.5 rounded-full bg-green-500 pulse-dot" />
          <span class="text-zinc-200 tabular-nums">{{
            stats.totalUsers.toLocaleString()
          }}</span>
          players
        </span>
        <span class="text-zinc-700">·</span>
        <span>
          <span class="text-zinc-200 tabular-nums">{{
            stats.totalGames.toLocaleString()
          }}</span>
          games
        </span>
        <span class="text-zinc-700">·</span>
        <span>
          <span class="text-zinc-200 tabular-nums"
            >{{ stats.totalPlaytimeHours.toLocaleString() }}h</span
          >
          played
        </span>
        <span class="text-zinc-700">·</span>
        <span>
          <span class="text-zinc-200 tabular-nums">{{
            stats.totalAchievementUnlocks.toLocaleString()
          }}</span>
          unlocks
        </span>
      </div>
    </header>

    <!-- HERO: weekly recap is the only loud card on the page. -->
    <CommunityWeeklyRecap
      :slides="weeklyRecap"
      @go-to-game="goToGame"
      @go-to-user="goToUser"
    />

    <!-- Around-Now is hidden — its information now lives inline on the
         Top Players leaderboard rows below (online dot +
         currently-playing game name). Set SHOW_AROUND_NOW_STRIP to true
         to bring the standalone strip back alongside the inline rows. -->
    <CommunityNowPlayingStrip
      v-if="SHOW_AROUND_NOW_STRIP"
      :entries="nowPlaying"
      @go-to-game="goToGame"
    />

    <!-- Weekly Quest card is hidden for now. To bring it back, flip
         SHOW_WEEKLY_QUEST to true.  Game Roulette used to live on this
         row alongside the quest; it has moved to the store's Browse tab
         where "I want to play something" energy belongs. -->
    <CommunityWeeklyChallenge
      v-if="SHOW_WEEKLY_QUEST"
      :challenge="weeklyChallenge"
    />

    <!-- Main split: activity (2/3) + top players (1/3). -->
    <section class="grid grid-cols-1 lg:grid-cols-[2fr,1fr] gap-6 pt-2">
      <div class="space-y-3">
        <h2
          class="text-sm font-display font-semibold flex items-center gap-1.5 text-zinc-300"
        >
          <BoltIcon class="size-4 text-blue-400" />
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
            @go-to-user="goToUser"
          />
        </div>
      </div>

      <aside class="space-y-3">
        <h2
          class="text-sm font-display font-semibold flex items-center gap-1.5 text-zinc-300"
        >
          <TrophyIcon class="size-4 text-yellow-500" />
          Top players
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
        <div
          v-else
          class="rounded-xl bg-zinc-800/50 ring-1 ring-zinc-700/40 divide-y divide-zinc-700/40"
        >
          <div
            v-for="entry in leaderboard.slice(0, 10)"
            :key="entry.user.id"
            class="flex items-center gap-3 px-3 py-2.5"
          >
            <span
              class="text-xs font-bold text-zinc-500 tabular-nums w-4 text-center shrink-0"
            >
              {{ entry.rank }}
            </span>
            <button
              class="shrink-0 rounded-full transition-transform hover:scale-110"
              @click="goToUser(entry.user.id)"
            >
              <img
                v-if="entry.user.profilePictureObjectId"
                :src="objectUrl(entry.user.profilePictureObjectId)"
                class="size-7 rounded-full object-cover"
              />
              <div
                v-else
                class="size-7 rounded-full bg-zinc-700 flex items-center justify-center"
              >
                <UserIcon class="size-3.5 text-zinc-500" />
              </div>
            </button>
            <!--
              Two-line cell when the user is currently in a session: name
              + MVP crown on top, pulsing green dot + game name underneath.
              When offline / no live session, the second line collapses
              away so the row stays tight.
            -->
            <div class="flex-1 min-w-0">
              <div class="text-sm flex items-center gap-1 min-w-0">
                <button
                  class="font-medium truncate text-zinc-100 hover:text-blue-400 transition-colors text-left"
                  @click="goToUser(entry.user.id)"
                >
                  {{ entry.user.displayName || entry.user.username }}
                </button>
                <span
                  v-if="mvp && entry.user.id === mvp.userId"
                  :title="mvpTooltip"
                  class="text-amber-300 shrink-0 cursor-help"
                  aria-label="Today's MVP"
                  >👑</span
                >
              </div>
              <button
                v-if="nowPlayingByUser.get(entry.user.id)"
                class="mt-0.5 flex items-center gap-1.5 text-[11px] text-zinc-400 hover:text-zinc-200 transition-colors min-w-0 max-w-full"
                @click="
                  goToGame(nowPlayingByUser.get(entry.user.id)!.game.id)
                "
              >
                <span class="size-1.5 rounded-full bg-green-500 pulse-dot shrink-0" />
                <span class="truncate text-left">
                  {{ nowPlayingByUser.get(entry.user.id)!.game.name }}
                </span>
              </button>
            </div>
            <span
              class="text-xs text-zinc-400 tabular-nums font-medium shrink-0"
            >
              {{ entry.playtimeHours.toLocaleString() }}h
            </span>
          </div>
        </div>
      </aside>
    </section>
  </main>
</template>

<script setup lang="ts">
import { TrophyIcon, UserIcon, BoltIcon } from "@heroicons/vue/24/solid";
import { invoke } from "@tauri-apps/api/core";
import {
  useServerApi,
  type CommunityStats,
  type CommunityActivityItem,
  type LeaderboardUser,
  type NowPlayingEntry,
  type WeeklyRecapSlide,
  type MvpToday,
  type WeeklyChallenge,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { clusterActivity } from "~/composables/use-community-clusters";

useHead({ title: "Community" });

// Layout flags — flip to `true` to bring the hidden cards back. We keep
// the template branches around (rather than deleting them) so the work
// to wire these surfaces in stays accessible. Using script-level consts
// instead of `v-if="false"` so vue-tsc still narrows nullable props
// referenced inside the hidden branch.
const SHOW_AROUND_NOW_STRIP = false;
const SHOW_WEEKLY_QUEST = false;

const router = useRouter();
const api = useServerApi();

const stats = ref<CommunityStats | null>(null);
const activity = ref<CommunityActivityItem[]>([]);
const leaderboard = ref<LeaderboardUser[]>([]);
const nowPlaying = ref<NowPlayingEntry[]>([]);
const weeklyRecap = ref<WeeklyRecapSlide[]>([]);
const mvp = ref<MvpToday | null>(null);
const weeklyChallenge = ref<WeeklyChallenge | null>(null);
const activityLoading = ref(true);
const leaderboardLoading = ref(true);

const clusteredActivity = computed(() => clusterActivity(activity.value));

// userId → currently-playing entry, for the leaderboard rows below. The
// strip rendering of nowPlaying is hidden on this layout (see `v-if="false"`
// on CommunityNowPlayingStrip); the data is recycled into a per-row
// "green dot + game name" decoration on the leaderboard instead.
const nowPlayingByUser = computed(() => {
  const map = new Map<string, NowPlayingEntry>();
  for (const entry of nowPlaying.value) {
    map.set(entry.userId, entry);
  }
  return map;
});

const mvpTooltip = computed(() => {
  if (!mvp.value) return "";
  // Crude session count: server doesn't return it directly, but the
  // weight model (seconds + unlocks*600) means the tooltip is best
  // expressed in human terms — hours played + unlocks.
  const hours = Math.max(1, Math.round(mvp.value.sessionSeconds / 3600));
  const playLabel =
    mvp.value.sessionSeconds === 0 ? "no playtime" : `${hours}h playtime`;
  return `Today's MVP: ${playLabel} · ${mvp.value.achievementsUnlocked} achievement${mvp.value.achievementsUnlocked === 1 ? "" : "s"}`;
});

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

function goToUser(userId: string) {
  // Native read-only profile page (main/pages/profile/[id].vue). Mirrors the
  // BPM surface, which has its own /bigpicture/profile/[id] route.
  router.push(`/profile/${userId}`);
}

// "Now playing" presence is the only datum on this page that is genuinely
// live — the leaderboard row's green dot + game name needs to react when
// someone starts/stops playing. Everything else (stats, activity, recap,
// leaderboard totals) is hour/day-scale and doesn't merit polling.
function refreshNowPlaying() {
  api.community
    .nowPlaying()
    .then((n) => (nowPlaying.value = n))
    .catch((e) => console.warn("[community] now-playing failed:", e));
}

let nowPlayingTimer: ReturnType<typeof setInterval> | null = null;

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

  refreshNowPlaying();
  // 30s cadence — comfortably under the server's 5-minute stale window so a
  // session that just started or just ended flips within roughly one polling
  // cycle. Cleared on unmount to keep the request bus quiet when the user
  // navigates away.
  nowPlayingTimer = setInterval(refreshNowPlaying, 30_000);

  api.community
    .weeklyRecap()
    .then((w) => (weeklyRecap.value = w))
    .catch((e) => console.warn("[community] weekly-recap failed:", e));

  api.community
    .mvpToday()
    .then((m) => (mvp.value = m))
    .catch((e) => console.warn("[community] mvp-today failed:", e));

  api.community
    .weeklyChallenge()
    .then((w) => (weeklyChallenge.value = w))
    .catch((e) => console.warn("[community] weekly-challenge failed:", e));
});

onUnmounted(() => {
  if (nowPlayingTimer) {
    clearInterval(nowPlayingTimer);
    nowPlayingTimer = null;
  }
});
</script>

<style scoped>
/* Subtle 2-second pulse on the green dot in the stats strip — matches
   the same pulse used on the "Around now" presence strip below it. */
.pulse-dot {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}
@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}
</style>
