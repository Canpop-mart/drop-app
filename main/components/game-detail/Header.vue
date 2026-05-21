<template>
  <div>
    <!-- Blurred banner backdrop. -->
    <div class="absolute inset-0 z-0">
      <img
        :src="bannerUrl"
        class="w-full h-[24rem] object-cover blur-sm scale-105"
      />
      <div
        class="absolute inset-0 bg-gradient-to-t from-zinc-900 via-zinc-900/80 to-transparent opacity-90"
      />
      <div
        class="absolute inset-0 bg-gradient-to-r from-zinc-900/95 via-zinc-900/80 to-transparent opacity-90"
      />
    </div>

    <div class="relative z-10">
      <div class="px-8">
        <h1
          class="text-5xl text-zinc-100 font-bold font-display drop-shadow-lg"
        >
          {{ game.mName }}
        </h1>

        <!-- Version / update status line. -->
        <div
          v-if="
            status.type === 'Installed' &&
            status.install_type.type != InstalledType.PartiallyInstalled
          "
          class="mt-1"
        >
          <div
            v-if="!version?.userConfiguration?.enableUpdates"
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Version pinned
            <WrenchIcon class="size-3 text-blue-600" />
          </div>
          <div
            v-else-if="!status.update_available"
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Up to date <CheckCircleIcon class="size-3 text-green-600" />
          </div>
          <div
            v-else
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Update available <ArrowDownTrayIcon class="size-3 text-blue-600" />
          </div>
        </div>

        <!-- Action button row. -->
        <div class="mt-3 flex flex-row gap-x-4 items-stretch">
          <!-- Do not add scale animations to this: https://stackoverflow.com/a/35683068 -->
          <GameStatusButton
            :status="status"
            @install="$emit('install')"
            @launch="$emit('launch')"
            @queue="$emit('queue')"
            @uninstall="$emit('uninstall')"
            @kill="$emit('kill')"
            @options="$emit('options')"
            @resume="$emit('resume')"
          />
          <!-- Streaming is gated behind dev mode while the Sunshine/Moonlight
               flow is hardened. The button polls the server every 15s for
               available remote sessions, so hiding it also avoids the
               background traffic for users who can't use the feature. -->
          <StreamButton
            v-if="devMode.enabled.value"
            :game-id="game.id"
            :game-name="game.mName"
            :is-installed="status.type === 'Installed'"
          />
          <button
            v-if="status.type === 'Installed' && status.update_available"
            class="transition-transform duration-300 hover:scale-105 active:scale-95 inline-flex gap-x-2 items-center rounded-md bg-blue-600 px-6 font-semibold text-white shadow-xl backdrop-blur-sm hover:bg-blue-700 uppercase font-display"
            @click="$emit('install')"
          >
            Update <ArrowDownTrayIcon class="size-5" />
          </button>
          <!-- Compat testing is a power-user feature — gated behind dev
               mode so casual users don't see a button whose effect they
               don't understand. -->
          <CompatTestButton
            v-if="devMode.enabled.value"
            :game-id="game.id"
            :is-installed="status.type === 'Installed'"
            @result="$emit('compat-result', $event)"
          />
          <NuxtLink
            class="transition-transform duration-300 hover:scale-105 active:scale-95 inline-flex items-center rounded-md bg-zinc-800/50 px-6 font-semibold text-white shadow-xl backdrop-blur-sm hover:bg-zinc-800/80 uppercase font-display"
            :to="{ path: '/store', query: { gameId: game.id } }"
          >
            <BuildingStorefrontIcon class="mr-2 size-5" aria-hidden="true" />
            Store
          </NuxtLink>
        </div>
      </div>

      <!-- Stat bar — a stable summary, kept outside the tabs. -->
      <div
        v-if="!statsLoading"
        class="mt-6 mx-8 flex items-center gap-6 rounded-lg bg-zinc-800/60 backdrop-blur-sm px-6 py-3 border border-zinc-700/50"
      >
        <div
          v-if="gameStats.lastPlayedAt"
          class="flex items-center gap-2 text-sm"
        >
          <CalendarIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">Last Played</span>
          <span class="text-zinc-100 font-medium">{{
            formatLastPlayed(gameStats.lastPlayedAt)
          }}</span>
        </div>
        <div v-if="gameStats.lastPlayedAt" class="w-px h-4 bg-zinc-600" />
        <div class="flex items-center gap-2 text-sm">
          <ClockIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">Play Time</span>
          <span class="text-zinc-100 font-medium">{{
            formatPlaytime(gameStats.playtimeSeconds)
          }}</span>
        </div>
        <div
          v-if="gameStats.achievementsTotal > 0"
          class="w-px h-4 bg-zinc-600"
        />
        <div
          v-if="gameStats.achievementsTotal > 0"
          class="flex items-center gap-2 text-sm"
        >
          <TrophyIcon class="size-4 text-yellow-500 shrink-0" />
          <span class="text-zinc-400">Achievements</span>
          <span class="text-zinc-100 font-medium">
            {{ gameStats.achievementsUnlocked }}/{{
              gameStats.achievementsTotal
            }}
          </span>
          <div class="w-24 h-1.5 bg-zinc-700 rounded-full overflow-hidden">
            <div
              class="h-full bg-blue-500 rounded-full transition-all duration-500"
              :style="{ width: achievementPercent + '%' }"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Library game-detail header: blurred banner, title, version/update
 * status line, the primary action-button row, and the stat bar.
 *
 * Purely presentational — every action button re-emits upward so the
 * page (which owns the install/launch composables) decides what happens.
 */
import {
  ArrowDownTrayIcon,
  BuildingStorefrontIcon,
  CalendarIcon,
  CheckCircleIcon,
  ClockIcon,
  TrophyIcon,
  WrenchIcon,
} from "@heroicons/vue/24/solid";
import { InstalledType } from "~/types";
import type { Game, GameStatus, GameVersion } from "~/types";
import {
  formatLastPlayed,
  formatPlaytime,
  type GameStatsData,
} from "~/composables/game-detail/use-game-stats";

const props = defineProps<{
  game: Game;
  status: GameStatus;
  version: GameVersion | undefined;
  bannerUrl: string;
  statsLoading: boolean;
  gameStats: GameStatsData;
  devMode: ReturnType<typeof useDevMode>;
}>();

defineEmits<{
  (e: "install"): void;
  (e: "launch"): void;
  (e: "queue"): void;
  (e: "uninstall"): void;
  (e: "kill"): void;
  (e: "options"): void;
  (e: "resume"): void;
  (e: "compat-result", outcome: unknown): void;
}>();

const achievementPercent = computed(() => {
  const { achievementsUnlocked, achievementsTotal } = props.gameStats;
  if (achievementsTotal <= 0) return 0;
  return Math.round((achievementsUnlocked / achievementsTotal) * 100);
});
</script>
