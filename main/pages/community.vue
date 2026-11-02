<template>
  <main class="mx-auto max-w-[1900px] px-8 py-8">
    <!-- Header — title + slim inline stats strip on the right. -->
    <header class="flex items-end justify-between gap-8 mb-7">
      <div>
        <h1 class="text-2xl font-display font-bold text-zinc-100">Community</h1>
        <p class="text-sm text-zinc-500 mt-0.5">
          What everyone's playing on this Drop server.
        </p>
      </div>
      <div class="flex shrink-0 flex-col items-end gap-2.5">
        <button
          v-if="!stats && statsError"
          class="flex items-center gap-1.5 text-sm text-zinc-500 font-medium hover:text-zinc-300 transition-colors"
          @click="loadStats"
        >
          <ArrowPathIcon class="size-3.5" />
          Server stats unavailable · Retry
        </button>
        <div
          v-else-if="stats"
          class="grid grid-cols-4 overflow-hidden rounded-xl bg-zinc-900/70 ring-1 ring-white/5"
        >
          <div
            v-for="item in statItems"
            :key="item.label"
            class="px-5 py-3 border-l border-white/5 first:border-l-0"
          >
            <div class="text-base font-semibold tabular-nums text-zinc-100">
              {{ item.value }}
            </div>
            <div class="text-[11px] uppercase tracking-wide text-zinc-500">
              {{ item.label }}
            </div>
          </div>
        </div>
      </div>
    </header>

    <!-- Command deck: a wide main column (weekly-recap hero + activity feed)
         beside a rail that gathers presence, top players and rotation into
         compact cards — so nothing sits alone in an empty full-width band. -->
    <div
      class="grid grid-cols-1 xl:grid-cols-[minmax(0,1.65fr)_minmax(360px,0.85fr)] gap-6 items-stretch mb-8"
    >
      <CommunityWeeklyRecap
        class="[&>section]:mb-0 [&>section]:h-full [&>section>div]:h-full"
        :slides="weeklyRecap"
        :failed="recapError"
        @go-to-game="goToGame"
        @go-to-user="goToUser"
        @retry="loadWeeklyRecap"
      />

      <section
        class="rounded-2xl bg-zinc-900/65 ring-1 ring-white/5 p-5 min-h-32"
      >
        <div class="mb-4 flex items-center justify-between gap-3">
          <h2
            class="font-display font-semibold text-zinc-100 flex items-center gap-2"
          >
            <span
              class="size-2 rounded-full"
              :class="
                nowPlaying.length ? 'bg-green-500 pulse-dot' : 'bg-zinc-600'
              "
            />
            Playing now
          </h2>
          <span class="text-xs text-zinc-500"
            >{{ nowPlaying.length || recentFaces.length }}
            {{ nowPlaying.length === 1 ? "player" : "players" }}</span
          >
        </div>
        <div
          v-if="nowPlaying.length"
          class="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-1 2xl:grid-cols-2 gap-2"
        >
          <button
            v-for="entry in nowPlaying.slice(0, 4)"
            :key="`${entry.userId}-${entry.startedAt}`"
            class="group flex items-center gap-3 rounded-xl bg-black/20 p-2.5 text-left ring-1 ring-white/5 hover:ring-blue-500/40"
            @click="goToGame(entry.game.id)"
          >
            <img
              v-if="entry.game.coverObjectId"
              :src="objectUrl(entry.game.coverObjectId)"
              class="h-14 w-11 rounded-md object-cover"
            />
            <div v-else class="h-14 w-11 rounded-md bg-zinc-800" />
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium text-zinc-100">
                {{ entry.displayName }}
              </div>
              <div
                class="truncate text-xs text-zinc-500 group-hover:text-blue-300"
              >
                {{ entry.game.name }}
              </div>
            </div>
          </button>
        </div>
        <div
          v-else-if="recentFaces.length"
          class="grid grid-cols-2 gap-x-5 gap-y-3"
        >
          <button
            v-for="face in recentFaces.slice(0, 4)"
            :key="face.id"
            class="flex min-w-0 items-center gap-2.5 text-left"
            @click="goToUser(face.id)"
          >
            <Avatar :object-id="face.avatar" :name="face.name" :size="32" />
            <div class="min-w-0">
              <div class="truncate text-sm text-zinc-200">{{ face.name }}</div>
              <div class="truncate text-xs text-zinc-600">
                {{ face.game ?? "Recently active" }}
              </div>
            </div>
          </button>
        </div>
        <p v-else class="py-5 text-center text-sm text-zinc-600">
          Nobody is playing right now.
        </p>
      </section>
    </div>

    <div
      class="grid grid-cols-1 xl:grid-cols-[minmax(0,1.65fr)_minmax(360px,0.85fr)] gap-6 items-start"
    >
      <!-- MAIN COLUMN -->
      <div class="min-w-0">
        <CommunityWeeklyChallenge
          v-if="SHOW_WEEKLY_QUEST"
          :challenge="weeklyChallenge"
        />

        <section>
          <h2
            class="text-sm font-display font-semibold flex items-center gap-1.5 text-zinc-300 mb-3"
          >
            <BoltIcon class="size-4 text-blue-400" />
            Recent activity
          </h2>
          <!-- The outer div holds the slot in the chain for the whole load;
               the skeleton inside it only appears once the load has run past
               the ~180ms threshold, so a fast feed paints straight in. -->
          <div v-if="activityLoading">
            <div v-if="showActivitySkeleton" class="space-y-2">
              <div
                v-for="i in 5"
                :key="i"
                class="flex items-center gap-3 rounded-lg bg-zinc-800/30 p-3"
              >
                <div
                  class="size-9 shrink-0 rounded-full bg-zinc-800/60 animate-pulse"
                />
                <div class="flex-1 space-y-2">
                  <div class="h-3 w-2/5 rounded bg-zinc-800/60 animate-pulse" />
                  <div class="h-3 w-1/4 rounded bg-zinc-800/60 animate-pulse" />
                </div>
              </div>
            </div>
          </div>
          <SectionError
            v-else-if="activityError"
            detail="The activity feed didn't load."
            @retry="loadActivity"
          />
          <div
            v-else-if="clusteredActivity.length === 0"
            class="text-sm text-zinc-500 py-10 text-center"
          >
            No recent activity to show.
          </div>
          <div v-else class="space-y-2">
            <CommunityActivityRow
              v-for="cluster in displayedActivity"
              :key="cluster.key"
              :cluster="cluster"
              @go-to-game="goToGame"
              @go-to-user="goToUser"
            />
            <p
              v-if="hiddenActivityCount > 0"
              class="pt-1 text-center text-xs text-zinc-600"
            >
              and {{ hiddenActivityCount }} more this week
            </p>
          </div>
        </section>
      </div>

      <!-- RAIL -->
      <aside class="space-y-5">
        <!-- Top players -->
        <div>
          <h3
            class="mb-2 flex items-center gap-1.5 text-sm font-display font-semibold text-zinc-300"
          >
            <TrophyIcon class="size-4 text-yellow-500" />
            Top players
          </h3>
          <div v-if="leaderboardLoading">
            <div
              v-if="showLeaderboardSkeleton"
              class="rounded-xl bg-zinc-800/50 ring-1 ring-zinc-700/40 divide-y divide-zinc-700/40"
            >
              <div v-for="i in 5" :key="i" class="flex items-center gap-3 p-3">
                <div class="size-4 rounded bg-zinc-700/50 animate-pulse" />
                <div
                  class="size-8 shrink-0 rounded-full bg-zinc-700/50 animate-pulse"
                />
                <div class="h-3 flex-1 rounded bg-zinc-700/50 animate-pulse" />
              </div>
            </div>
          </div>
          <SectionError
            v-else-if="leaderboardError"
            detail="The leaderboard didn't load."
            @retry="loadLeaderboard"
          />
          <div
            v-else-if="leaderboard.length === 0"
            class="text-sm text-zinc-500 py-8 text-center"
          >
            No players yet.
          </div>
          <div
            v-else
            class="rounded-2xl bg-zinc-900/65 ring-1 ring-white/5 divide-y divide-white/5 overflow-hidden"
          >
            <PlayerProgressRow
              v-for="entry in leaderboard.slice(0, 10)"
              :key="entry.user.id"
              :rank="entry.rank"
              :name="entry.user.displayName || entry.user.username"
              :avatar-object-id="entry.user.profilePictureObjectId"
              :crown="!!mvp && entry.user.id === mvp.userId"
              :crown-title="mvpTooltip"
              :now-playing="
                nowPlayingByUser.get(entry.user.id)?.game.name ?? null
              "
              :stat="`${entry.playtimeHours.toLocaleString()}h`"
              @select-user="goToUser(entry.user.id)"
              @select-game="goToUserGame(entry.user.id)"
            />
          </div>
        </div>
      </aside>
    </div>
  </main>
</template>

<script setup lang="ts">
import { TrophyIcon, BoltIcon } from "@heroicons/vue/24/solid";
import { ArrowPathIcon } from "@heroicons/vue/24/outline";
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
import { objectImageUrl } from "~/composables/use-object";
import { clusterActivity } from "~/composables/use-community-clusters";

useHead({ title: "Community" });

// Flip to `true` to bring the hidden weekly-quest card back. Kept as a
// script const (not `v-if="false"`) so vue-tsc still narrows the nullable prop.
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
const showActivitySkeleton = useDeferredLoading(() => activityLoading.value);
const showLeaderboardSkeleton = useDeferredLoading(
  () => leaderboardLoading.value,
);

// Each section owns its own failure flag. Without these a dead server and an
// empty server rendered the same "No recent activity to show." / "No players
// yet." copy, so there was no way to tell a broken page from a quiet one.
const statsError = ref(false);
const activityError = ref(false);
const leaderboardError = ref(false);
const recapError = ref(false);

const statItems = computed(() => {
  if (!stats.value) return [];
  return [
    { label: "Players", value: stats.value.totalUsers.toLocaleString() },
    { label: "Games", value: stats.value.totalGames.toLocaleString() },
    { label: "Hours", value: stats.value.totalPlaytimeHours.toLocaleString() },
    {
      label: "Unlocks",
      value: stats.value.totalAchievementUnlocks.toLocaleString(),
    },
  ];
});

// Recent activity: keep only the last week of the fetched events, then cap how
// many rows render — so a slow week still populates but a busy one doesn't turn
// into a giant wall. Both numbers are easy to tune.
const ACTIVITY_WINDOW_MS = 7 * 24 * 60 * 60 * 1000;
const MAX_ACTIVITY_ROWS = 15;

const clusteredActivity = computed(() => {
  const cutoff = Date.now() - ACTIVITY_WINDOW_MS;
  const recent = activity.value.filter((a) => {
    const t = new Date(a.timestamp).getTime();
    return Number.isNaN(t) || t >= cutoff;
  });
  return clusterActivity(recent);
});

// The feed renders at most MAX_ACTIVITY_ROWS; the full windowed set still feeds
// the "recently around" faces and the "+N more" count below the list.
const displayedActivity = computed(() =>
  clusteredActivity.value.slice(0, MAX_ACTIVITY_ROWS),
);
const hiddenActivityCount = computed(() =>
  Math.max(0, clusteredActivity.value.length - MAX_ACTIVITY_ROWS),
);

// userId → currently-playing entry, so the leaderboard rows can show a live
// green dot + game name (via PlayerProgressRow's `nowPlaying` prop) and the
// "Playing now" rail is the same data rendered as cover cards.
const nowPlayingByUser = computed(() => {
  const map = new Map<string, NowPlayingEntry>();
  for (const entry of nowPlaying.value) {
    map.set(entry.userId, entry);
  }
  return map;
});

// Recently-active faces for the quiet-state "Right now" strip: the most recent
// distinct people from the activity feed, with what they last touched.
const recentFaces = computed(() => {
  const seen = new Set<string>();
  const out: {
    id: string;
    name: string;
    avatar: string | null;
    game: string | null;
  }[] = [];
  for (const c of clusteredActivity.value) {
    if (seen.has(c.user.id)) continue;
    seen.add(c.user.id);
    out.push({
      id: c.user.id,
      name: c.user.displayName || c.user.username,
      avatar: c.user.profilePictureObjectId ?? null,
      game: c.kind === "request" ? null : (c.game?.mName ?? null),
    });
    if (out.length >= 8) break;
  }
  return out;
});

const mvpTooltip = computed(() => {
  if (!mvp.value) return "";
  const hours = Math.max(1, Math.round(mvp.value.sessionSeconds / 3600));
  const playLabel =
    mvp.value.sessionSeconds === 0 ? "no playtime" : `${hours}h playtime`;
  return `Today's MVP: ${playLabel} · ${mvp.value.achievementsUnlocked} achievement${mvp.value.achievementsUnlocked === 1 ? "" : "s"}`;
});

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

function goToGame(gameId: string) {
  // Community is a discovery surface, so land on the Store presentation.
  router.push(`/store/${gameId}`);
}

function goToUser(userId: string) {
  router.push(`/profile/${userId}`);
}

// A leaderboard row's live "now playing" line links to that game.
function goToUserGame(userId: string) {
  const entry = nowPlayingByUser.value.get(userId);
  if (entry) goToGame(entry.game.id);
}

// "Now playing" is the only genuinely live datum — poll it; everything else is
// hour/day-scale and fetched once.
function refreshNowPlaying() {
  api.community
    .nowPlaying()
    .then((n) => (nowPlaying.value = n))
    .catch((e) => console.warn("[community] now-playing failed:", e));
}

let nowPlayingTimer: ReturnType<typeof setInterval> | null = null;

function loadStats() {
  statsError.value = false;
  api.community
    .stats()
    .then((s) => (stats.value = s))
    .catch((e) => {
      console.warn("[community] stats failed:", e);
      statsError.value = true;
    });
}

function loadActivity() {
  activityLoading.value = true;
  activityError.value = false;
  // 30, not 100. Each row paints two images, so the old limit asked for 200
  // pictures in one go and was comfortably the heaviest paint in the app. Big
  // Picture already uses the server default.
  api.community
    .activity(30)
    .then((a) => (activity.value = a))
    .catch((e) => {
      console.warn("[community] activity failed:", e);
      activityError.value = true;
    })
    .finally(() => (activityLoading.value = false));
}

function loadLeaderboard() {
  leaderboardLoading.value = true;
  leaderboardError.value = false;
  api.community
    .leaderboard()
    .then((d) => (leaderboard.value = d.playtime))
    .catch((e) => {
      console.warn("[community] leaderboard failed:", e);
      leaderboardError.value = true;
    })
    .finally(() => (leaderboardLoading.value = false));
}

function loadWeeklyRecap() {
  recapError.value = false;
  api.community
    .weeklyRecap()
    .then((w) => (weeklyRecap.value = w))
    .catch((e) => {
      console.warn("[community] weekly-recap failed:", e);
      recapError.value = true;
    });
}

onMounted(() => {
  loadStats();
  loadActivity();
  loadLeaderboard();
  loadWeeklyRecap();

  refreshNowPlaying();
  nowPlayingTimer = setInterval(refreshNowPlaying, 30_000);

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
