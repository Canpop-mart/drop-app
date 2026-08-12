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
      Loading your profile...
    </div>

    <!-- Error -->
    <div v-else-if="error" class="mx-auto max-w-2xl px-8 py-20 text-center">
      <p class="text-sm text-red-400">{{ error }}</p>
    </div>

    <template v-else-if="profile">
      <ProfileHero
        :profile="profile"
        :stats="stats"
        editable
        @go-to-game="goToGame"
      />

      <div class="mx-auto max-w-5xl space-y-10 px-8 pb-16 pt-10">
        <!-- Favourites — shelf when set, an invite to add when empty. -->
        <div>
          <FavoriteShelf
            v-if="favorites.length > 0"
            :favorites="favorites"
            edit-href="/profile/favorites"
            @select-game="goToGame"
          />
          <template v-else>
            <div class="mb-3 flex items-center gap-2">
              <StarIcon class="size-4" :style="{ color: 'var(--accent)' }" />
              <h2 class="font-display text-lg font-semibold text-zinc-100">
                Favourites
              </h2>
            </div>
            <NuxtLink
              to="/profile/favorites"
              class="flex items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-700/70 py-8 text-sm font-medium text-zinc-400 transition-colors hover:border-zinc-500 hover:text-zinc-200"
            >
              <PlusIcon class="size-5" />
              Pin your favourite games
            </NuxtLink>
          </template>
        </div>

        <!-- Showcase — always shown on your own profile so it's addable. -->
        <section>
          <div class="mb-4 flex items-center gap-2">
            <span
              class="h-4 w-1 rounded-full"
              :style="{ background: 'var(--accent)' }"
            />
            <h2 class="font-display text-lg font-semibold text-zinc-100">
              Showcase
            </h2>
            <NuxtLink
              to="/profile/showcase"
              class="ml-auto text-xs font-medium hover:underline"
              :style="{ color: 'var(--accent)' }"
            >
              Edit
            </NuxtLink>
          </div>
          <div
            v-if="showcase.length > 0"
            class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5"
          >
            <ShowcaseCard
              v-for="item in showcase"
              :key="item.id"
              :item="item"
              @select-game="goToGame"
            />
          </div>
          <NuxtLink
            v-else
            to="/profile/showcase"
            class="flex items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-700/70 py-8 text-sm font-medium text-zinc-400 transition-colors hover:border-zinc-500 hover:text-zinc-200"
          >
            <PlusIcon class="size-5" />
            Feature games, achievements or a note
          </NuxtLink>
        </section>

        <!-- Recent sessions -->
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
            <button
              v-for="session in stats.recentSessions.slice(0, 8)"
              :key="session.id"
              class="srow flex w-full items-center gap-x-4 rounded-xl bg-zinc-800/50 p-3 text-left ring-1 ring-zinc-700/40 transition-colors"
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
          </div>
        </section>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { PlayIcon, StarIcon, PlusIcon } from "@heroicons/vue/24/solid";
import { invoke } from "@tauri-apps/api/core";
import {
  useServerApi,
  type UserProfile,
  type UserStats,
  type ShowcaseItem,
  type FavoriteEntry,
} from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";
import {
  formatPlaytime,
  formatLastPlayed,
} from "~/composables/use-recent-games";
import { useProfileTheme } from "~/composables/use-profile-theme";

useHead({ title: "Profile" });

const router = useRouter();
const api = useServerApi();

const profile = ref<UserProfile | null>(null);
const stats = ref<UserStats | null>(null);
const showcase = ref<ShowcaseItem[]>([]);
const favorites = ref<FavoriteEntry[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

// Accent theme resolved from the user's `profileTheme` (preset key or #hex).
// `vars` is bound on the page root so children reference the CSS custom
// properties (--accent, --accent-soft, --accent-border, --profile-banner, …).
const { vars } = useProfileTheme(() => profile.value?.profileTheme);

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

function goToGame(gameId: string) {
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/library/${gameId}`);
}

onMounted(async () => {
  try {
    const me = await api.profile.me();
    profile.value = me;
    // Stats / showcase / favourites soft-fail independently so one broken
    // endpoint can't blank the profile.
    const [statsRes, showcaseRes, favRes] = await Promise.allSettled([
      api.profile.stats(me.id),
      api.profile.showcase(me.id),
      api.profile.favorites.list(),
    ]);
    if (statsRes.status === "fulfilled") stats.value = statsRes.value;
    if (showcaseRes.status === "fulfilled")
      showcase.value = showcaseRes.value.items;
    if (favRes.status === "fulfilled") favorites.value = favRes.value;
  } catch (e) {
    error.value =
      "Couldn't load your profile. " +
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
