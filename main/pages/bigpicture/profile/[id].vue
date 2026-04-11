<template>
  <div class="flex flex-col h-full overflow-y-auto">
    <!-- Loading state -->
    <div v-if="loading" class="flex-1 flex items-center justify-center">
      <div class="size-12 border-4 border-blue-500/30 border-t-blue-500 rounded-full animate-spin" />
    </div>

    <!-- Error state -->
    <div v-else-if="!profile" class="flex-1 flex items-center justify-center">
      <div class="text-center">
        <UserIcon class="size-16 mx-auto mb-4 text-zinc-600" />
        <h3 class="text-2xl font-semibold text-zinc-400 mb-2">User not found</h3>
        <p class="text-zinc-600">This profile doesn't exist or couldn't be loaded.</p>
      </div>
    </div>

    <template v-else>
      <!-- Banner -->
      <div class="relative shrink-0 h-72">
        <img
          v-if="profile.bannerObjectId"
          :src="objectUrl(profile.bannerObjectId)"
          class="w-full h-full object-cover"
        />
        <div
          v-else
          class="w-full h-full"
          :style="{ background: `linear-gradient(135deg, ${themeColors.from}, ${themeColors.to})` }"
        />
        <!-- Gradient overlays -->
        <div
          class="absolute inset-0"
          :style="{ background: `linear-gradient(to top, ${themeColors.from}ee, ${themeColors.to}44, transparent)` }"
        />
        <div class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent" />

        <!-- Profile header overlay -->
        <div class="absolute bottom-0 left-0 right-0 p-8 flex items-end gap-6">
          <img
            v-if="profile.profilePictureObjectId"
            :src="objectUrl(profile.profilePictureObjectId)"
            class="size-28 rounded-full border-4 border-zinc-900 object-cover shadow-2xl shrink-0"
          />
          <div
            v-else
            class="size-28 rounded-full bg-zinc-700 border-4 border-zinc-900 flex items-center justify-center shrink-0"
          >
            <UserIcon class="size-12 text-zinc-500" />
          </div>
          <div class="flex-1 min-w-0 pb-1">
            <h1 class="text-4xl font-bold font-display text-zinc-100 truncate">
              {{ profile.displayName ?? profile.username }}
            </h1>
            <p class="text-lg text-zinc-400">@{{ profile.username }}</p>
            <p v-if="profile.bio" class="text-sm text-zinc-400 mt-1 line-clamp-2 max-w-2xl">
              {{ profile.bio }}
            </p>
          </div>
        </div>
      </div>

      <!-- Stats row -->
      <div class="px-8 py-6">
        <div class="grid grid-cols-4 gap-4">
          <div class="bg-zinc-800/50 rounded-xl p-4 text-center ring-1 ring-white/5">
            <p class="text-3xl font-bold text-blue-400">
              {{ stats ? Math.round(stats.totalPlaytimeSeconds / 3600) : 0 }}
            </p>
            <p class="text-sm text-zinc-500 mt-1">Hours Played</p>
          </div>
          <div class="bg-zinc-800/50 rounded-xl p-4 text-center ring-1 ring-white/5">
            <p class="text-3xl font-bold text-blue-400">{{ stats?.gamesPlayed ?? 0 }}</p>
            <p class="text-sm text-zinc-500 mt-1">Games Played</p>
          </div>
          <div class="bg-zinc-800/50 rounded-xl p-4 text-center ring-1 ring-white/5">
            <p class="text-3xl font-bold text-yellow-400">{{ stats?.achievementsUnlocked ?? 0 }}</p>
            <p class="text-sm text-zinc-500 mt-1">Achievements</p>
          </div>
          <div class="bg-zinc-800/50 rounded-xl p-4 text-center ring-1 ring-white/5">
            <p class="text-3xl font-bold text-green-400">{{ recentSessionCount }}</p>
            <p class="text-sm text-zinc-500 mt-1">Recent Sessions</p>
          </div>
        </div>
      </div>

      <!-- Game Showcase -->
      <div v-if="gameShowcaseItems.length" class="px-8 pb-6">
        <h2 class="text-xl font-bold font-display text-zinc-100 mb-4">Game Showcase</h2>
        <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-4">
          <button
            v-for="item in gameShowcaseItems"
            :key="item.id"
            :ref="(el: any) => registerContent(el, {
              onSelect: () => item.game && navigateToGame(item.game.id),
            })"
            class="group relative rounded-xl overflow-hidden bg-zinc-800/50 ring-1 ring-white/5 focus:ring-blue-500/60 focus:ring-2 transition-all duration-200"
            @click="item.game && navigateToGame(item.game.id)"
          >
            <div class="aspect-[2/3]">
              <img
                v-if="item.game?.mCoverObjectId"
                :src="objectUrl(item.game.mCoverObjectId)"
                :alt="item.game.mName"
                class="size-full object-cover transition-transform duration-300 group-hover:scale-105"
              />
              <div v-else class="size-full flex items-center justify-center text-zinc-600">
                <SparklesIcon class="size-8" />
              </div>
            </div>
            <!-- Game name overlay -->
            <div class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-zinc-950/90 to-transparent p-3">
              <p class="text-sm font-medium text-zinc-200 truncate">
                {{ item.game?.mName || item.title }}
              </p>
            </div>
            <!-- Completion badge -->
            <div
              v-if="(item.gameStats?.achievementsTotal ?? 0) > 0"
              class="absolute top-2 right-2 px-2 py-0.5 rounded-full text-xs font-bold"
              :class="
                (item.gameStats?.achievementsUnlocked ?? 0) >= (item.gameStats?.achievementsTotal ?? 1)
                  ? 'bg-yellow-500/90 text-yellow-950'
                  : 'bg-zinc-900/80 text-zinc-300'
              "
            >
              {{ Math.round(((item.gameStats?.achievementsUnlocked ?? 0) / (item.gameStats?.achievementsTotal ?? 1)) * 100) }}%
            </div>
          </button>
        </div>
      </div>

      <!-- Achievement Showcase -->
      <div v-if="achievementShowcaseItems.length" class="px-8 pb-6">
        <h2 class="text-xl font-bold font-display text-zinc-100 mb-4">Achievement Showcase</h2>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div
            v-for="item in achievementShowcaseItems"
            :key="item.id"
            class="flex items-center gap-4 rounded-xl bg-zinc-800/50 ring-1 ring-white/5 p-4 transition-all duration-200"
          >
            <div class="shrink-0 size-14 rounded-lg overflow-hidden bg-zinc-700/50 flex items-center justify-center">
              <img
                v-if="item.achievement?.iconUrl && !achievementIconErrors[item.id]"
                :src="item.achievement.iconUrl"
                class="size-full object-cover"
                @error="achievementIconErrors[item.id] = true"
              />
              <TrophyIcon v-else class="size-7 text-yellow-500" />
            </div>
            <div class="min-w-0 flex-1">
              <p class="text-sm font-semibold text-zinc-100 truncate">
                {{ item.achievement?.title || item.title }}
              </p>
              <p v-if="item.achievement?.description" class="text-xs text-zinc-500 truncate mt-0.5">
                {{ item.achievement.description }}
              </p>
              <p class="text-xs text-zinc-400 mt-0.5">{{ item.game?.mName }}</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Recent Activity -->
      <div class="px-8 pb-8">
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <!-- Recent Sessions -->
          <div>
            <h2 class="text-xl font-bold font-display text-zinc-100 mb-4">Recent Activity</h2>
            <div v-if="stats?.recentSessions?.length" class="space-y-2">
              <button
                v-for="session in stats.recentSessions.slice(0, 5)"
                :key="session.id"
                :ref="(el: any) => registerContent(el, {
                  onSelect: () => session.game && navigateToGame(session.gameId),
                })"
                class="w-full flex items-center gap-4 p-4 bg-zinc-800/30 rounded-xl ring-1 ring-white/5 focus:ring-blue-500/60 focus:ring-2 transition-all text-left"
                @click="session.game && navigateToGame(session.gameId)"
              >
                <img
                  v-if="session.game?.mIconObjectId"
                  :src="objectUrl(session.game.mIconObjectId)"
                  class="size-10 rounded-lg object-cover shrink-0"
                />
                <div v-else class="size-10 rounded-lg bg-zinc-700/50 shrink-0" />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-zinc-100 truncate">{{ session.game?.mName ?? 'Unknown Game' }}</p>
                  <p class="text-xs text-zinc-500">{{ timeAgo(session.startedAt) }}</p>
                </div>
                <div v-if="session.durationSeconds" class="text-xs text-zinc-500 shrink-0">
                  {{ formatDuration(session.durationSeconds) }}
                </div>
              </button>
            </div>
            <div v-else class="p-6 bg-zinc-800/20 rounded-xl text-center">
              <p class="text-sm text-zinc-500">No recent activity</p>
            </div>
          </div>

          <!-- Recent Achievements -->
          <div>
            <h2 class="text-xl font-bold font-display text-zinc-100 mb-4">Recent Achievements</h2>
            <div v-if="recentAchievements.length" class="space-y-2">
              <div
                v-for="ach in recentAchievements"
                :key="ach.id"
                class="flex items-center gap-4 p-4 bg-zinc-800/30 rounded-xl ring-1 ring-white/5 transition-all"
              >
                <div class="shrink-0 size-10 rounded-lg overflow-hidden bg-zinc-700/50 flex items-center justify-center">
                  <img
                    v-if="ach.achievement?.iconUrl && !achievementIconErrors[ach.id]"
                    :src="ach.achievement.iconUrl"
                    class="size-full object-cover"
                    @error="achievementIconErrors[ach.id] = true"
                  />
                  <TrophyIcon v-else class="size-5 text-yellow-500" />
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-zinc-100 truncate">{{ ach.achievement?.title }}</p>
                  <p class="text-xs text-zinc-500">{{ ach.game?.mName }}</p>
                </div>
                <p class="text-xs text-zinc-500 shrink-0">{{ timeAgo(ach.unlockedAt) }}</p>
              </div>
            </div>
            <div v-else class="p-6 bg-zinc-800/20 rounded-xl text-center">
              <p class="text-sm text-zinc-500">No achievements yet</p>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import {
  UserIcon,
  SparklesIcon,
} from "@heroicons/vue/24/outline";
import { TrophyIcon } from "@heroicons/vue/24/solid";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  useServerApi,
  type UserProfile,
  type UserStats,
  type UserActivity,
  type UserShowcase,
  type ShowcaseItem,
} from "~/composables/use-server-api";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

definePageMeta({ layout: "bigpicture" });

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

const THEME_MAP: Record<string, { from: string; to: string }> = {
  default: { from: "#1e3a5f", to: "#581c87" },
  ocean: { from: "#0c4a6e", to: "#164e63" },
  sunset: { from: "#9a3412", to: "#831843" },
  forest: { from: "#14532d", to: "#1a2e05" },
  ember: { from: "#7c2d12", to: "#451a03" },
  arctic: { from: "#0e7490", to: "#1e40af" },
  midnight: { from: "#1e1b4b", to: "#0f172a" },
  rose: { from: "#9f1239", to: "#4c0519" },
};

function timeAgo(dateStr: string | Date): string {
  const ms = Date.now() - new Date(dateStr).getTime();
  const sec = Math.floor(ms / 1000);
  if (sec < 60) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hrs = Math.floor(min / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  return `${months}mo ago`;
}

function formatDuration(seconds: number): string {
  const hrs = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  if (hrs > 0) return `${hrs}h ${mins}m`;
  return `${mins}m`;
}

const route = useRoute();
const router = useRouter();
const api = useServerApi();
const registerContent = useBpFocusableGroup("content");
const focusNav = useFocusNavigation();

const userId = computed(() => route.params.id as string);
const loading = ref(true);
const profile = ref<UserProfile | null>(null);
const stats = ref<UserStats | null>(null);
const activity = ref<UserActivity | null>(null);
const showcase = ref<UserShowcase | null>(null);
const achievementIconErrors = reactive<Record<string, boolean>>({});

const themeColors = computed(
  () => THEME_MAP[profile.value?.profileTheme ?? "default"] ?? THEME_MAP.default,
);

const recentSessionCount = computed(
  () => stats.value?.recentSessions?.length ?? 0,
);

const gameShowcaseItems = computed(() =>
  (showcase.value?.items ?? []).filter((i) => i.type === "FavoriteGame"),
);

const achievementShowcaseItems = computed(() =>
  (showcase.value?.items ?? []).filter(
    (i) => i.type === "Achievement" && i.achievement,
  ),
);

const recentAchievements = computed(() =>
  (activity.value?.achievements ?? []).slice(0, 10),
);

function navigateToGame(gameId: string) {
  focusNav.saveFocusSnapshot(route.path);
  router.push(`/bigpicture/library/${gameId}`);
}

async function loadProfile() {
  loading.value = true;
  try {
    const id = userId.value;
    const [profileData, statsData, activityData, showcaseData] =
      await Promise.all([
        api.profile.get(id).catch(() => null),
        api.profile.stats(id).catch(() => null),
        api.profile.activity(id).catch(() => null),
        api.profile.showcase(id).catch(() => null),
      ]);

    profile.value = profileData;
    stats.value = statsData;
    activity.value = activityData;
    showcase.value = showcaseData;
  } catch (e) {
    console.error("Failed to load profile:", e);
  } finally {
    loading.value = false;
    nextTick(() => {
      if (!focusNav.restoreFocusSnapshot(route.path)) {
        focusNav.autoFocusContent("content");
      }
    });
  }
}

onMounted(() => loadProfile());
</script>
