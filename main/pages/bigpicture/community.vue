<template>
  <div class="flex flex-col h-full">
    <!-- Tab navigation -->
    <div class="flex items-center gap-2 px-8 py-4 border-b border-zinc-800/30">
      <button
        v-for="tab in tabs"
        :key="tab.value"
        :ref="(el: any) => registerTab(el, { onSelect: () => (activeTab = tab.value) })"
        class="px-4 py-2 text-sm rounded-lg font-medium transition-colors"
        :class="[
          activeTab === tab.value
            ? 'bg-blue-600/20 text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50',
        ]"
        @click="activeTab = tab.value"
      >
        {{ tab.label }}
      </button>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="grid grid-cols-4 gap-4 mb-8">
        <div v-for="i in 4" :key="i" class="h-20 rounded-xl bg-zinc-800/50 animate-pulse" />
      </div>
      <div class="space-y-3">
        <div v-for="i in 6" :key="i" class="h-16 rounded-xl bg-zinc-800/50 animate-pulse" />
      </div>
    </div>

    <!-- ═══ Activity tab ═══ -->
    <div v-else-if="activeTab === 'activity'" class="flex-1 overflow-y-auto px-8 py-6">
      <!-- Stats cards -->
      <div class="grid grid-cols-4 gap-3 mb-8">
        <div
          :ref="(el: any) => registerContent(el, { onSelect: () => (activeTab = 'players') })"
          class="bg-zinc-900/60 rounded-xl p-4 cursor-pointer hover:bg-zinc-800/60 transition-colors"
          @click="activeTab = 'players'"
        >
          <p class="text-2xl font-bold text-zinc-100">{{ stats.totalUsers.toLocaleString() }}</p>
          <p class="text-xs text-blue-400 mt-1">Players &rarr;</p>
        </div>
        <div class="bg-zinc-900/60 rounded-xl p-4">
          <p class="text-2xl font-bold text-zinc-100">{{ stats.totalGames.toLocaleString() }}</p>
          <p class="text-xs text-zinc-500 mt-1">Games</p>
        </div>
        <div class="bg-zinc-900/60 rounded-xl p-4">
          <p class="text-2xl font-bold text-zinc-100">{{ stats.totalPlaytimeHours.toLocaleString() }}h</p>
          <p class="text-xs text-zinc-500 mt-1">Total Playtime</p>
        </div>
        <div class="bg-zinc-900/60 rounded-xl p-4">
          <p class="text-2xl font-bold text-zinc-100">{{ stats.totalAchievementUnlocks.toLocaleString() }}</p>
          <p class="text-xs text-zinc-500 mt-1">Achievements Unlocked</p>
        </div>
      </div>

      <!-- Activity type filter -->
      <div class="flex gap-2 mb-4">
        <button
          v-for="filter in activityFilters"
          :key="filter.value"
          :ref="(el: any) => registerTab(el, { onSelect: () => (activityFilter = filter.value) })"
          class="px-3 py-1 rounded-full text-xs font-medium transition-colors"
          :class="activityFilter === filter.value
            ? 'bg-blue-600/20 text-blue-400'
            : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'"
          @click="activityFilter = filter.value"
        >
          {{ filter.label }}
        </button>
      </div>

      <!-- Activity feed -->
      <div class="space-y-2">
        <div
          v-for="item in filteredActivity"
          :key="`${item.type}-${item.timestamp}`"
          :ref="(el: any) => registerContent(el, { onSelect: () => goToGame(item.game?.id) })"
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4"
        >
          <div class="size-10 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden">
            <img
              v-if="item.user.profilePictureObjectId"
              :src="objectUrl(item.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div v-else class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold">
              {{ item.user.displayName[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p class="text-sm text-zinc-200">
              <span class="font-medium">{{ item.user.displayName }}</span>
              <template v-if="item.type === 'session'">
                played
                <span class="text-blue-400">{{ item.game.mName }}</span>
                <span v-if="item.data.duration" class="text-zinc-500">
                  for {{ formatDuration(item.data.duration) }}
                </span>
              </template>
              <template v-else-if="item.type === 'achievement'">
                unlocked
                <span class="text-yellow-400">{{ item.data.achievement?.title }}</span>
                in <span class="text-blue-400">{{ item.game.mName }}</span>
              </template>
              <template v-else-if="item.type === 'request'">
                requested
                <span class="text-purple-400">{{ item.data.request?.title }}</span>
              </template>
            </p>
            <p class="text-xs text-zinc-600 mt-0.5">{{ formatTimeAgo(item.timestamp) }}</p>
          </div>

          <div v-if="item.game?.mCoverObjectId" class="size-10 rounded-lg overflow-hidden flex-shrink-0 bg-zinc-800">
            <img :src="objectUrl(item.game.mCoverObjectId)" class="w-full h-full object-cover" loading="lazy" />
          </div>
        </div>
      </div>

      <div v-if="activity.length >= 30" class="flex justify-center py-6">
        <button
          :ref="(el: any) => registerContent(el, { onSelect: loadMoreActivity })"
          class="px-6 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm font-medium hover:bg-zinc-700 transition-colors"
          @click="loadMoreActivity"
        >
          Load More
        </button>
      </div>
    </div>

    <!-- ═══ Players tab ═══ -->
    <div v-else-if="activeTab === 'players'" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="space-y-2">
        <div
          v-for="entry in leaderboard"
          :key="entry.user.id"
          :ref="(el: any) => registerContent(el, { onSelect: () => viewProfile(entry.user.id) })"
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4 cursor-pointer"
        >
          <div class="size-12 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden">
            <img
              v-if="entry.user.profilePictureObjectId"
              :src="objectUrl(entry.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div v-else class="w-full h-full flex items-center justify-center text-zinc-500 text-lg font-bold">
              {{ entry.user.displayName[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-zinc-200 truncate">{{ entry.user.displayName }}</p>
            <p class="text-xs text-zinc-600">@{{ entry.user.username }}</p>
          </div>

          <div class="flex gap-6 text-right">
            <div>
              <p class="text-sm font-medium text-zinc-200">{{ entry.playtimeHours.toLocaleString() }}h</p>
              <p class="text-xs text-zinc-600">Playtime</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">{{ entry.gamesPlayed }}</p>
              <p class="text-xs text-zinc-600">Games</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">{{ entry.achievements }}</p>
              <p class="text-xs text-zinc-600">Achievements</p>
            </div>
          </div>
        </div>

        <p v-if="leaderboard.length === 0" class="text-zinc-500 text-center py-12 text-sm">
          No players found.
        </p>
      </div>
    </div>

    <!-- ═══ Leaderboard tab ═══ -->
    <div v-else-if="activeTab === 'leaderboard'" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="space-y-2">
        <div
          v-for="entry in leaderboard"
          :key="entry.user.id"
          :ref="(el: any) => registerContent(el, { onSelect: () => viewProfile(entry.user.id) })"
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4 cursor-pointer"
        >
          <div class="w-8 text-center flex-shrink-0">
            <span class="text-lg font-bold" :class="entry.rank <= 3 ? rankColors[entry.rank - 1] : 'text-zinc-500'">
              {{ entry.rank }}
            </span>
          </div>

          <div class="size-10 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden">
            <img
              v-if="entry.user.profilePictureObjectId"
              :src="objectUrl(entry.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div v-else class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold">
              {{ entry.user.displayName[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-zinc-200 truncate">{{ entry.user.displayName }}</p>
            <p class="text-xs text-zinc-600">@{{ entry.user.username }}</p>
          </div>

          <div class="flex gap-6 text-right">
            <div>
              <p class="text-sm font-medium text-zinc-200">{{ entry.playtimeHours.toLocaleString() }}h</p>
              <p class="text-xs text-zinc-600">Playtime</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">{{ entry.gamesPlayed }}</p>
              <p class="text-xs text-zinc-600">Games</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">{{ entry.achievements }}</p>
              <p class="text-xs text-zinc-600">Achievements</p>
            </div>
          </div>
        </div>
      </div>
    </div>

  </div>
</template>

<script setup lang="ts">
import {
  useServerApi,
  type CommunityStats,
  type CommunityActivityItem,
  type LeaderboardUser,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

definePageMeta({ layout: "bigpicture" });

const api = useServerApi();
const router = useRouter();
const focusNav = useFocusNavigation();

const registerTab = useBpFocusableGroup("content");
const registerContent = useBpFocusableGroup("content");

const loading = ref(true);
const activeTab = ref("activity");
const activityFilter = ref("all");

const stats = ref<CommunityStats>({
  totalGames: 0, totalUsers: 0, totalPlaytimeHours: 0, totalPlaySessions: 0,
  totalAchievementUnlocks: 0, totalRequests: 0, pendingRequests: 0, totalLeaderboardEntries: 0,
});
const activity = ref<CommunityActivityItem[]>([]);
const leaderboard = ref<LeaderboardUser[]>([]);

const rankColors = ["text-yellow-400", "text-zinc-300", "text-amber-600"];

const tabs = [
  { label: "Activity", value: "activity" },
  { label: "Players", value: "players" },
  { label: "Leaderboard", value: "leaderboard" },
];

const activityFilters = [
  { label: "All", value: "all" },
  { label: "Sessions", value: "session" },
  { label: "Achievements", value: "achievement" },
  { label: "Requests", value: "request" },
];

const filteredActivity = computed(() => {
  if (activityFilter.value === "all") return activity.value;
  return activity.value.filter((a) => a.type === activityFilter.value);
});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId?: string) {
  if (!gameId) return;
  router.push(`/bigpicture/library/${gameId}`);
}

function viewProfile(userId: string) {
  focusNav.saveFocusSnapshot("/bigpicture/community");
  router.push(`/bigpicture/profile/${userId}`);
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.round((seconds % 3600) / 60);
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

function formatTimeAgo(timestamp: string): string {
  const diff = Date.now() - new Date(timestamp).getTime();
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return `${Math.floor(days / 7)}w ago`;
}

async function loadMoreActivity() {
  if (activity.value.length === 0) return;
  const lastTimestamp = activity.value[activity.value.length - 1].timestamp;
  const more = await api.community.activity(30, lastTimestamp);
  activity.value.push(...more);
}

onMounted(async () => {
  try {
    const [statsData, activityData, leaderboardData] = await Promise.all([
      api.community.stats().catch(() => stats.value),
      api.community.activity().catch(() => []),
      api.community.leaderboard().catch(() => ({ playtime: [] })),
    ]);
    stats.value = statsData;
    activity.value = activityData;
    leaderboard.value = leaderboardData.playtime;
  } catch (e) {
    console.error("Failed to load community data:", e);
  } finally {
    loading.value = false;
    nextTick(() => focusNav.autoFocusContent("content"));
  }
});
</script>
