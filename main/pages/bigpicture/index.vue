<template>
  <div class="flex flex-col h-full">
    <!-- Main scrollable content -->
    <div
      ref="scrollContainer"
      class="flex-1 overflow-y-auto px-8 py-6 space-y-8"
      data-bp-scroll
    >
      <!-- Continue Playing Section -->
      <section v-if="recentGames.length > 0">
        <h2 class="text-lg font-semibold text-zinc-200 font-display mb-4">
          Continue Playing
        </h2>
        <div class="flex gap-4 overflow-x-auto pb-2">
          <div
            v-for="(entry, index) in recentGames"
            :key="entry.game.id"
            class="flex-shrink-0"
            :style="{ width: '10rem' }"
            :ref="
              (el: any) =>
                registerTile(el, {
                  onSelect: () => {
                    focusNav.saveFocusSnapshot(route.path);
                    $router.push(`/bigpicture/library/${entry.game.id}`).catch((e: any) => {
                      console.error(`[BPM:HOME] Navigation FAILED for ${entry.game.id}:`, e);
                    });
                  },
                  onFocus: () => prefetchGame(entry.game.id),
                })
            "
          >
            <BigPictureGameTile
              :game="entry.game"
              :status="entry.status"
            />
          </div>
        </div>
      </section>

      <!-- Empty state for Continue Playing -->
      <section v-else class="py-12">
        <div class="text-center">
          <PlayIcon class="size-12 mx-auto mb-3 text-zinc-600" />
          <p class="text-zinc-500 text-sm">Start playing to see your recent games here.</p>
        </div>
      </section>

      <!-- Downloads Section (only if active downloads) -->
      <section v-if="activeDownloads.length > 0">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold text-zinc-200 font-display">
            Downloads
          </h2>
          <NuxtLink
            :ref="(el: any) => registerQuickLink(el, { onSelect: () => {} })"
            to="/bigpicture/downloads"
            class="text-xs font-medium text-blue-400 hover:text-blue-300 transition-colors"
          >
            View all
          </NuxtLink>
        </div>
        <div class="space-y-3">
          <div
            v-for="item in activeDownloads.slice(0, 3)"
            :key="item.meta.id"
            class="flex items-center gap-4 bg-zinc-900/50 rounded-lg p-4"
          >
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium text-zinc-200">
                {{ gameNames[item.meta.id]?.name || item.meta.id }}
              </p>
              <div class="mt-2 h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  class="h-full bg-blue-500 rounded-full transition-all duration-300"
                  :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                />
              </div>
            </div>
            <div class="text-xs font-medium text-zinc-400 flex-shrink-0 w-10 text-right">
              {{ (item.dl_progress * 100).toFixed(0) }}%
            </div>
          </div>
        </div>
      </section>

      <!-- Quick Links Section -->
      <section>
        <h2 class="text-lg font-semibold text-zinc-200 font-display mb-4">
          Quick Links
        </h2>
        <div class="grid grid-cols-3 gap-4">
          <!-- Store -->
          <NuxtLink
            to="/bigpicture/store"
            :ref="(el: any) => registerQuickLink(el, { onSelect: () => {} })"
            class="bg-zinc-800/50 hover:bg-zinc-700/50 rounded-2xl p-6 flex flex-col items-center gap-3 transition-colors ring-2 ring-transparent focus-visible:ring-blue-500"
          >
            <ShoppingBagIcon class="size-10 text-blue-400" />
            <span class="text-sm font-medium text-zinc-300">Store</span>
          </NuxtLink>

          <!-- Library -->
          <NuxtLink
            to="/bigpicture/library"
            :ref="(el: any) => registerQuickLink(el, { onSelect: () => {} })"
            class="bg-zinc-800/50 hover:bg-zinc-700/50 rounded-2xl p-6 flex flex-col items-center gap-3 transition-colors ring-2 ring-transparent focus-visible:ring-blue-500"
          >
            <Square3Stack3DIcon class="size-10 text-blue-400" />
            <span class="text-sm font-medium text-zinc-300">Library</span>
          </NuxtLink>

          <!-- Community -->
          <NuxtLink
            to="/bigpicture/community"
            :ref="(el: any) => registerQuickLink(el, { onSelect: () => {} })"
            class="bg-zinc-800/50 hover:bg-zinc-700/50 rounded-2xl p-6 flex flex-col items-center gap-3 transition-colors ring-2 ring-transparent focus-visible:ring-blue-500"
          >
            <ChatBubbleLeftRightIcon class="size-10 text-blue-400" />
            <span class="text-sm font-medium text-zinc-300">Community</span>
          </NuxtLink>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import {
  ShoppingBagIcon,
  Square3Stack3DIcon,
  ChatBubbleLeftRightIcon,
  PlayIcon,
} from "@heroicons/vue/24/outline";
import BigPictureGameTile from "~/components/bigpicture/BigPictureGameTile.vue";
import { parseStatus, deduplicatedInvoke } from "~/composables/game";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useQueueState } from "~/composables/downloads";
import { serverUrl } from "~/composables/use-server-fetch";
import { useGame } from "~/composables/game";
import type { Game, GameStatus, RawGameStatus } from "~/types";

definePageMeta({ layout: "bigpicture" });

interface RecentGameEntry {
  game: Game;
  status: GameStatus;
}

const recentGames = ref<RecentGameEntry[]>([]);
const scrollContainer = ref<HTMLElement | null>(null);
const focusNav = useFocusNavigation();
const registerTile = useBpFocusableGroup("content");
const registerQuickLink = useBpFocusableGroup("content");
const route = useRoute();
const gameNames = ref<Record<string, { name: string; coverUrl?: string }>>({});

// Download state
const queueState = useQueueState();
const queue = computed(() => queueState.value?.queue ?? []);
const activeDownloads = computed(() => queue.value.filter((item) => item.status !== "Completed"));

function prefetchGame(gameId: string) {
  deduplicatedInvoke("fetch_game", { gameId }).catch(() => {});
}

async function loadRecentGames() {
  try {
    const recentData = await fetch(serverUrl("api/v1/client/playtime/recent")).then((r) =>
      r.json(),
    ) as { recent_games: string[] };

    if (!Array.isArray(recentData.recent_games)) {
      recentGames.value = [];
      return;
    }

    const gameIds = recentData.recent_games.slice(0, 5);
    const entries: RecentGameEntry[] = [];

    for (const gameId of gameIds) {
      try {
        const gameData = await useGame(gameId);
        const statusData: RawGameStatus = await invoke("fetch_game_status", { id: gameId });
        entries.push({
          game: gameData.game,
          status: parseStatus(statusData),
        });
      } catch (e) {
        console.error(`Failed to load recent game ${gameId}:`, e);
      }
    }

    recentGames.value = entries;

    // Load game names for download items
    for (const item of queue.value) {
      if (!gameNames.value[item.meta.id]) {
        try {
          const gameData = await useGame(item.meta.id);
          gameNames.value[item.meta.id] = {
            name: gameData.game.mName,
            coverUrl: gameData.game.mCoverObjectId
              ? serverUrl(`api/v1/object/${gameData.game.mCoverObjectId}`)
              : undefined,
          };
        } catch {
          // Game data not available
        }
      }
    }
  } catch (e) {
    console.error("Failed to fetch recent games:", e);
  }
}

onMounted(async () => {
  await loadRecentGames();
  if (!focusNav.restoreFocusSnapshot(route.path)) {
    focusNav.autoFocusContent("content");
  }
});</script>
