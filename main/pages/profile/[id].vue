<template>
  <div class="min-h-full bg-zinc-950" :style="vars">
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
      <ProfileHero
        :profile="profile"
        :stats="stats"
        :presence="livePresence"
        @go-to-game="goToGame"
      />

      <div class="mx-auto max-w-5xl space-y-10 px-8 pb-16 pt-10">
        <FavoriteShelf :favorites="favorites" @select-game="goToGame" />

        <!-- Showcase -->
        <section v-if="showcase.length > 0">
          <div class="mb-4 flex items-center gap-2">
            <span
              class="h-4 w-1 rounded-full"
              :style="{ background: 'var(--accent)' }"
            />
            <h2 class="font-display text-lg font-semibold text-zinc-100">
              Showcase
            </h2>
          </div>
          <div
            class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5"
          >
            <ShowcaseCard
              v-for="item in showcase"
              :key="item.id"
              :item="item"
              @select-game="goToGame"
            />
          </div>
        </section>

        <!-- Recent sessions — with a per-game "compare achievements" jump. -->
        <section v-if="stats && stats.recentSessions.length > 0">
          <div class="mb-4 flex items-center gap-2">
            <span
              class="h-4 w-1 rounded-full"
              :style="{ background: 'var(--accent)' }"
            />
            <h2 class="font-display text-lg font-semibold text-zinc-100">
              Recent sessions
            </h2>
          </div>
          <div class="space-y-2">
            <div
              v-for="session in stats.recentSessions.slice(0, 8)"
              :key="session.id"
              class="srow flex items-center gap-x-4 rounded-xl bg-zinc-800/50 p-3 ring-1 ring-zinc-700/40 transition-colors"
            >
              <button
                class="flex min-w-0 flex-1 items-center gap-x-4 text-left"
                @click="session.game && goToGame(session.game.id)"
              >
                <img
                  v-if="session.game?.mCoverObjectId"
                  :src="objectUrl(session.game.mCoverObjectId)"
                  class="size-12 shrink-0 rounded object-cover"
                />
                <div
                  v-else
                  class="flex size-12 shrink-0 items-center justify-center rounded bg-zinc-700"
                >
                  <PlayIcon class="size-5 text-zinc-500" />
                </div>
                <div class="min-w-0 flex-1">
                  <p class="truncate text-sm font-medium text-zinc-200">
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
              <button
                v-if="session.game"
                class="shrink-0 rounded-md px-2.5 py-1 text-xs font-medium text-zinc-300 ring-1 ring-zinc-700 transition-colors hover:bg-zinc-700/50 hover:text-zinc-100"
                title="Compare achievements on this game"
                @click="compareOn(session.game.id)"
              >
                Compare
              </button>
            </div>
          </div>
        </section>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { PlayIcon } from "@heroicons/vue/24/solid";
import { invoke } from "@tauri-apps/api/core";
import {
  useServerApi,
  type UserProfile,
  type UserStats,
  type ShowcaseItem,
  type FavoriteEntry,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  formatPlaytime,
  formatLastPlayed,
} from "~/composables/use-recent-games";
import { useProfileTheme } from "~/composables/use-profile-theme";

useHead({ title: "Profile" });

const route = useRoute();
const router = useRouter();
const api = useServerApi();

const profileId = route.params.id as string;

const profile = ref<UserProfile | null>(null);
const stats = ref<UserStats | null>(null);
const showcase = ref<ShowcaseItem[]>([]);
const favorites = ref<FavoriteEntry[]>([]);
const livePresence = ref<{ gameId: string; gameName: string } | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);

const { vars } = useProfileTheme(() => profile.value?.profileTheme);

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId: string) {
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/store/${gameId}`);
}

// Jump to the game's library page in achievement-compare mode against this user.
function compareOn(gameId: string) {
  router.push(`/library/${gameId}?compare=${profileId}`);
}

onMounted(async () => {
  try {
    profile.value = await api.profile.get(profileId);
    // Stats / showcase / favourites / presence all soft-fail independently so
    // one broken endpoint can't blank the profile.
    const [statsRes, showcaseRes, favRes] = await Promise.allSettled([
      api.profile.stats(profileId),
      api.profile.showcase(profileId),
      api.profile.favorites.forUser(profileId),
    ]);
    if (statsRes.status === "fulfilled") stats.value = statsRes.value;
    if (showcaseRes.status === "fulfilled")
      showcase.value = showcaseRes.value.items;
    if (favRes.status === "fulfilled") favorites.value = favRes.value;

    // Presence — is this user in a live session right now?
    api.community
      .nowPlaying()
      .then((entries) => {
        const live = entries.find((e) => e.userId === profileId);
        livePresence.value = live
          ? { gameId: live.game.id, gameName: live.game.name }
          : null;
      })
      .catch(() => {});
  } catch (e) {
    error.value =
      "Couldn't load this profile. " +
      (e instanceof Error ? e.message : String(e));
  } finally {
    loading.value = false;
  }
});
</script>

<style scoped>
.srow:hover {
  --tw-ring-color: var(--accent-border);
}
</style>
