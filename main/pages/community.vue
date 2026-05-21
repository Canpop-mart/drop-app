<template>
  <div class="mx-auto max-w-6xl px-8 py-6">
    <div class="mb-6">
      <h1 class="text-2xl font-display font-bold text-zinc-100">Community</h1>
      <p class="mt-1 text-sm text-zinc-400">
        What everyone else on this Drop server is up to.
      </p>
    </div>

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
          v-else-if="activity.length === 0"
          class="text-sm text-zinc-500 py-10 text-center"
        >
          No recent activity to show.
        </div>
        <div v-else class="space-y-2">
          <div
            v-for="(item, i) in activity"
            :key="`${item.type}-${item.timestamp}-${i}`"
            class="flex items-start gap-x-3 rounded-xl bg-zinc-800/50 backdrop-blur-sm p-3 ring-1 ring-zinc-700/40 hover:ring-blue-500/40 transition-colors"
          >
            <img
              v-if="item.user.profilePictureObjectId"
              :src="objectUrl(item.user.profilePictureObjectId)"
              class="size-10 rounded-full object-cover shrink-0"
            />
            <div
              v-else
              class="size-10 rounded-full bg-zinc-700 flex items-center justify-center shrink-0"
            >
              <UserIcon class="size-5 text-zinc-500" />
            </div>
            <div class="flex-1 min-w-0">
              <p class="text-sm text-zinc-300">
                <span class="font-medium text-zinc-100">{{
                  item.user.displayName || item.user.username
                }}</span>
                <template v-if="item.type === 'session'">
                  played
                  <button
                    class="font-medium text-blue-400 hover:text-blue-300 transition-colors"
                    @click="goToGame(item.game.id)"
                  >
                    {{ item.game.mName }}
                  </button>
                  <template v-if="item.data.duration">
                    for {{ formatPlaytime(item.data.duration) }}
                  </template>
                </template>
                <template v-else-if="item.type === 'achievement'">
                  unlocked
                  <span class="font-medium text-yellow-400">
                    {{ item.data.achievement?.title }}
                  </span>
                  in
                  <button
                    class="font-medium text-blue-400 hover:text-blue-300 transition-colors"
                    @click="goToGame(item.game.id)"
                  >
                    {{ item.game.mName }}
                  </button>
                </template>
                <template v-else-if="item.type === 'request'">
                  requested
                  <span class="font-medium text-purple-400">{{
                    item.data.request?.title
                  }}</span>
                </template>
              </p>
              <p class="text-xs text-zinc-500 mt-0.5">
                {{ formatLastPlayed(item.timestamp) }}
              </p>
            </div>
            <img
              v-if="item.game.mCoverObjectId && item.type !== 'request'"
              :src="objectUrl(item.game.mCoverObjectId)"
              class="h-14 w-10 rounded object-cover shrink-0 hidden sm:block"
            />
          </div>
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
              <p class="text-sm font-medium text-zinc-200 truncate">
                {{ entry.user.displayName || entry.user.username }}
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
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  formatPlaytime,
  formatLastPlayed,
} from "~/composables/use-recent-games";

useHead({ title: "Community" });

const router = useRouter();
const api = useServerApi();

const stats = ref<CommunityStats | null>(null);
const activity = ref<CommunityActivityItem[]>([]);
const leaderboard = ref<LeaderboardUser[]>([]);
const activityLoading = ref(true);
const leaderboardLoading = ref(true);

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

function rankColor(rank: number): string {
  if (rank === 1)
    return "bg-yellow-500/20 text-yellow-300 ring-1 ring-yellow-500/40";
  if (rank === 2)
    return "bg-zinc-400/20 text-zinc-200 ring-1 ring-zinc-400/40";
  if (rank === 3)
    return "bg-orange-700/30 text-orange-300 ring-1 ring-orange-700/40";
  return "bg-zinc-700/50 text-zinc-400";
}

onMounted(() => {
  // Three independent fetches in parallel — none of them block the others.
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
});
</script>
