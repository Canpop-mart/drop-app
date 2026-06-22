<template>
  <div class="min-h-full bg-zinc-950">
    <!-- Loading -->
    <div
      v-if="loading"
      class="flex items-center justify-center min-h-[60vh] text-zinc-500 text-sm gap-x-3"
    >
      <div
        class="size-4 rounded-full border-2 border-zinc-700 border-t-zinc-300 animate-spin"
      />
      Loading profile...
    </div>

    <!-- Error -->
    <div v-else-if="error" class="mx-auto max-w-2xl px-8 py-20 text-center">
      <p class="text-sm text-red-400">{{ error }}</p>
    </div>

    <template v-else-if="profile">
      <!-- Banner section -->
      <div class="relative h-56">
        <img
          v-if="profile.bannerObjectId"
          :src="objectUrl(profile.bannerObjectId)"
          alt=""
          class="w-full h-full object-cover"
        />
        <div
          v-else
          class="w-full h-full"
          :style="{
            background: `linear-gradient(135deg, ${themeColors.from}, ${themeColors.to})`,
          }"
        />
        <div
          class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent"
        />
      </div>

      <div class="mx-auto max-w-5xl px-8 -mt-16 relative">
        <!-- Avatar + name overlay -->
        <div class="flex items-end gap-5 mb-6">
          <img
            v-if="profile.profilePictureObjectId"
            :src="objectUrl(profile.profilePictureObjectId)"
            class="size-32 rounded-full border-4 border-zinc-950 object-cover shadow-2xl"
          />
          <div
            v-else
            class="size-32 rounded-full border-4 border-zinc-950 bg-zinc-800 flex items-center justify-center shadow-2xl"
          >
            <UserIcon class="size-14 text-zinc-500" />
          </div>
          <div class="pb-2 flex-1 min-w-0">
            <h1 class="text-3xl font-display font-bold text-zinc-100 truncate">
              {{ profile.displayName || profile.username }}
            </h1>
            <p class="text-sm text-zinc-400">@{{ profile.username }}</p>
          </div>
        </div>

        <p v-if="profile.bio" class="text-sm text-zinc-300 max-w-2xl mb-8">
          {{ profile.bio }}
        </p>

        <!-- Stat cards -->
        <div v-if="stats" class="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-10">
          <div
            class="rounded-xl bg-zinc-800/50 backdrop-blur-sm p-5 ring-1 ring-zinc-700/40"
          >
            <ClockIcon class="size-5 text-blue-400 mb-2" />
            <p class="text-2xl font-bold text-zinc-100">
              {{ formatPlaytime(stats.totalPlaytimeSeconds) }}
            </p>
            <p class="text-xs text-zinc-500 uppercase tracking-wider mt-1">
              Total playtime
            </p>
          </div>
          <div
            class="rounded-xl bg-zinc-800/50 backdrop-blur-sm p-5 ring-1 ring-zinc-700/40"
          >
            <PlayIcon class="size-5 text-purple-400 mb-2" />
            <p class="text-2xl font-bold text-zinc-100">
              {{ stats.gamesPlayed.toLocaleString() }}
            </p>
            <p class="text-xs text-zinc-500 uppercase tracking-wider mt-1">
              Games played
            </p>
          </div>
          <div
            class="rounded-xl bg-zinc-800/50 backdrop-blur-sm p-5 ring-1 ring-zinc-700/40"
          >
            <TrophyIcon class="size-5 text-yellow-400 mb-2" />
            <p class="text-2xl font-bold text-zinc-100">
              {{ stats.achievementsUnlocked.toLocaleString() }}
            </p>
            <p class="text-xs text-zinc-500 uppercase tracking-wider mt-1">
              Achievements unlocked
            </p>
          </div>
        </div>

        <!-- Showcase (favorite games / pinned items) -->
        <section v-if="showcase.length > 0" class="mb-10">
          <h2 class="text-lg font-display font-semibold text-zinc-100 mb-4">
            Showcase
          </h2>
          <div
            class="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-6 lg:grid-cols-8 gap-3"
          >
            <button
              v-for="item in showcase"
              :key="item.id"
              class="group flex flex-col text-left transition-transform duration-200 hover:-translate-y-1"
              @click="item.game && goToGame(item.game.id)"
            >
              <div
                class="relative aspect-[3/4] rounded-lg overflow-hidden bg-zinc-800 ring-1 ring-zinc-700/50 group-hover:ring-blue-500/50 transition-colors"
              >
                <img
                  v-if="item.game?.mCoverObjectId"
                  :src="objectUrl(item.game.mCoverObjectId)"
                  :alt="item.title"
                  class="w-full h-full object-cover"
                  loading="lazy"
                />
                <div
                  v-else
                  class="w-full h-full flex items-center justify-center text-zinc-600 text-xs px-2 text-center"
                >
                  {{ item.title }}
                </div>
              </div>
              <p
                class="mt-1.5 text-xs text-zinc-400 truncate group-hover:text-zinc-200 transition-colors"
              >
                {{ item.game?.mName || item.title }}
              </p>
            </button>
          </div>
        </section>

        <!-- Recent sessions -->
        <section v-if="stats && stats.recentSessions.length > 0" class="mb-10">
          <h2 class="text-lg font-display font-semibold text-zinc-100 mb-4">
            Recent sessions
          </h2>
          <div class="space-y-2">
            <button
              v-for="session in stats.recentSessions.slice(0, 8)"
              :key="session.id"
              class="w-full flex items-center gap-x-4 rounded-xl bg-zinc-800/50 backdrop-blur-sm p-3 ring-1 ring-zinc-700/40 hover:ring-blue-500/40 transition-colors text-left"
              @click="session.game && goToGame(session.game.id)"
            >
              <img
                v-if="session.game?.mCoverObjectId"
                :src="objectUrl(session.game.mCoverObjectId)"
                class="size-12 rounded object-cover shrink-0"
              />
              <div
                v-else
                class="size-12 rounded bg-zinc-700 shrink-0 flex items-center justify-center"
              >
                <PlayIcon class="size-5 text-zinc-500" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-zinc-200 truncate">
                  {{ session.game?.mName || "Unknown game" }}
                </p>
                <p class="text-xs text-zinc-500">
                  {{ formatLastPlayed(session.startedAt) }}
                  <template v-if="session.durationSeconds">
                    · {{ formatPlaytime(session.durationSeconds) }}
                  </template>
                </p>
              </div>
            </button>
          </div>
        </section>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import {
  ClockIcon,
  PlayIcon,
  TrophyIcon,
  UserIcon,
} from "@heroicons/vue/24/solid";
import { invoke } from "@tauri-apps/api/core";
import {
  useServerApi,
  type UserProfile,
  type UserStats,
  type ShowcaseItem,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  formatPlaytime,
  formatLastPlayed,
} from "~/composables/use-recent-games";

useHead({ title: "Profile" });

const route = useRoute();
const router = useRouter();
const api = useServerApi();

const profile = ref<UserProfile | null>(null);
const stats = ref<UserStats | null>(null);
const showcase = ref<ShowcaseItem[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

// Theme accent for the banner fallback gradient, mapped from the user's
// `profileTheme` enum so the page works without a server-rendered theme.
const themeColors = computed(() => {
  switch (profile.value?.profileTheme) {
    case "ocean":
      return { from: "#0ea5e9", to: "#1e3a8a" };
    case "sunset":
      return { from: "#f97316", to: "#7c2d12" };
    case "forest":
      return { from: "#22c55e", to: "#14532d" };
    case "purple":
      return { from: "#a855f7", to: "#581c87" };
    case "rose":
      return { from: "#f43f5e", to: "#881337" };
    default:
      return { from: "#3b82f6", to: "#1e3a8a" };
  }
});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId: string) {
  // Viewing someone else's profile is a discovery surface — land on the
  // store presentation rather than the management UI.
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/store/${gameId}`);
}

onMounted(async () => {
  const id = route.params.id as string;
  try {
    profile.value = await api.profile.get(id);
    // Stats / showcase can soft-fail without blocking the profile header.
    const [statsRes, showcaseRes] = await Promise.allSettled([
      api.profile.stats(id),
      api.profile.showcase(id),
    ]);
    if (statsRes.status === "fulfilled") stats.value = statsRes.value;
    if (showcaseRes.status === "fulfilled")
      showcase.value = showcaseRes.value.items;
  } catch (e) {
    error.value =
      "Couldn't load this profile. " +
      (e instanceof Error ? e.message : String(e));
  } finally {
    loading.value = false;
  }
});
</script>
