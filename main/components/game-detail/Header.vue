<template>
  <div>
    <!-- Blurred banner backdrop. When the game has no real banner
         (mBannerObjectId is null), render the BannerFallback gradient
         instead of stretching the cover-style icon across the full
         hero — looks intentional rather than broken. -->
    <div class="absolute inset-0 z-0">
      <img
        v-if="game.mBannerObjectId"
        :src="bannerUrl"
        class="w-full h-[24rem] object-cover blur-sm scale-105"
      />
      <div v-else class="w-full h-[24rem] blur-sm scale-105">
        <BannerFallback :name="game.mName" text-size="text-9xl" />
      </div>
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
            :disabled="launchInFlight"
            @install="$emit('install')"
            @launch="$emit('launch')"
            @launch-incognito="$emit('launch-incognito')"
            @queue="$emit('queue')"
            @kill="$emit('kill')"
            @resume="$emit('resume')"
          />
          <!-- Launch status line — sits beside the Play button while a launch
               is in flight. Shows the precise prep message when the backend is
               doing slow one-time prefix setup, otherwise a generic
               "Launching…". Renders nothing when idle, so other states are
               visually unchanged. -->
          <div
            v-if="prepStatus || launchInFlight"
            class="inline-flex items-center gap-x-2 rounded-md bg-zinc-800/60 backdrop-blur-sm px-4 text-sm font-medium text-zinc-200 shadow-xl"
          >
            <span
              class="size-4 rounded-full border-2 border-zinc-500/40 border-t-blue-400 animate-spin"
            />
            {{ prepStatus ?? "Launching..." }}
          </div>
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
        <div v-if="controllerLabel" class="w-px h-4 bg-zinc-600" />
        <div v-if="controllerLabel" class="flex items-center gap-2 text-sm">
          <GamepadIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">Controller</span>
          <span
            :class="
              controllerLabel === 'Full'
                ? 'text-green-400 font-medium'
                : 'text-amber-400 font-medium'
            "
            >{{ controllerLabel }}</span
          >
        </div>

        <!-- HowLongToBeat — Main Story figure inline, full breakdown on hover. -->
        <div v-if="hltb" class="w-px h-4 bg-zinc-600" />
        <div
          v-if="hltb"
          class="flex items-center gap-2 text-sm"
          :title="hltb.breakdown"
        >
          <FlagIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">To Beat</span>
          <span class="text-zinc-100 font-medium">{{ hltb.primary }}</span>
        </div>

        <!-- Friends — server-mates who've played this game. Lives inline
             with the other stats now (previously sat in its own row
             below the stat bar, which made the page feel cluttered).
             Clicking it jumps the user to the Community tab where the
             full leaderboard lives. -->
        <div
          v-if="players && players.length > 0"
          class="w-px h-4 bg-zinc-600"
        />
        <button
          v-if="players"
          type="button"
          class="group flex items-center gap-2 text-sm rounded-md px-1.5 py-1 -mx-1.5 -my-1 hover:bg-zinc-700/40 transition-colors"
          @click="$emit('open-community')"
        >
          <UsersIcon class="size-4 text-zinc-400 shrink-0" />
          <!-- "Friends Played:" disambiguates the stat — the bare
               "Friends 0" on a solo game read like "you have no
               friends" rather than "no friends have played this yet".
               Colon makes the label-value relationship explicit (no
               more "Friends Played 3" run-on). -->
          <span class="text-zinc-400">Friends Played:</span>
          <span class="text-zinc-100 font-medium">
            {{ players.length }}
          </span>
          <div
            v-if="players.length > 0"
            class="flex -space-x-1.5 ml-0.5"
          >
            <template v-for="p in players.slice(0, 3)" :key="p.userId">
              <!-- Avatar URLs go through the Tauri `server://` protocol
                   so the desktop webview can resolve them (a bare
                   `/api/v1/object/...` path 404s in the file:// origin
                   the webview runs in). The web build's serverUrl just
                   returns the same path back. -->
              <img
                v-if="p.avatarObjectId"
                :src="serverUrl(`api/v1/object/${p.avatarObjectId}`)"
                class="size-5 rounded-full ring-2 ring-zinc-800 object-cover bg-zinc-700"
                referrerpolicy="no-referrer"
              />
              <div
                v-else
                class="size-5 rounded-full ring-2 ring-zinc-800 bg-zinc-700 flex items-center justify-center text-[10px] font-semibold text-zinc-300"
              >
                {{ (p.displayName || "?")[0] }}
              </div>
            </template>
            <div
              v-if="players.length > 3"
              class="size-5 rounded-full ring-2 ring-zinc-800 bg-zinc-700 flex items-center justify-center text-[10px] font-semibold text-zinc-300"
            >
              +{{ players.length - 3 }}
            </div>
          </div>
        </button>
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
  FlagIcon,
  TrophyIcon,
  UsersIcon,
  WrenchIcon,
} from "@heroicons/vue/24/solid";
import { InstalledType } from "~/types";
import type { Game, GameStatus, GameVersion } from "~/types";
import BannerFallback from "~/components/BannerFallback.vue";
import GamepadIcon from "~/components/Icons/GamepadIcon.vue";
import type { GamePlayerEntry } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
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
  // Server-mates who've played this game (any playtime > 0 or any
  // achievement unlocked). `null` while loading; empty array == nobody.
  // The header renders an inline Friends stat + click target only when
  // this is non-empty — for solo games the slot stays hidden so the
  // stat bar doesn't gain a permanently-zero row.
  players?: GamePlayerEntry[] | null;
  // Launch progress (optional; both default to "off" so other callers are
  // unaffected). `launchInFlight` is true between the Play click and the
  // backend accepting the launch; `prepStatus` carries a precise message
  // while a slow one-time prefix-prep step runs (e.g. installing the VC++
  // runtime). When either is set the header shows a spinner + label beside
  // the Play button and disables the launch action.
  launchInFlight?: boolean;
  prepStatus?: string;
}>();

defineEmits<{
  (e: "install"): void;
  (e: "launch"): void;
  // Hidden Shift+click activation on the Play button — re-emitted up
  // so the page can route it to launchCtl.launchIncognito().
  (e: "launch-incognito"): void;
  (e: "queue"): void;
  (e: "kill"): void;
  (e: "resume"): void;
  (e: "compat-result", outcome: unknown): void;
  // Emitted when the inline Friends stat is clicked — the page should
  // switch to the Community tab where the full leaderboard lives.
  (e: "open-community"): void;
}>();

const achievementPercent = computed(() => {
  const { achievementsUnlocked, achievementsTotal } = props.gameStats;
  if (achievementsTotal <= 0) return 0;
  return Math.round((achievementsUnlocked / achievementsTotal) * 100);
});

// Gamepad support, from Steam's controller_support signal. Hidden when None.
const controllerLabel = computed(() => {
  const c = props.game.mControllerSupport;
  return c === "Full" ? "Full" : c === "Partial" ? "Partial" : null;
});

// HowLongToBeat completion times. Minutes -> a compact "9½ h" label. The
// stat bar shows the Main Story figure with the full three-tier breakdown
// in the hover title, so it stays one slot wide.
function formatHltbHours(minutes: number): string {
  const rounded = Math.round((minutes / 60) * 2) / 2; // nearest half hour
  const whole = Math.floor(rounded);
  const half = rounded - whole >= 0.5;
  if (whole === 0) return half ? "½ h" : "0 h";
  return `${whole}${half ? "½" : ""} h`;
}

const hltb = computed(() => {
  const fmt = (m?: number | null) => (m && m > 0 ? formatHltbHours(m) : null);
  const main = fmt(props.game.mHltbMain);
  const mainSides = fmt(props.game.mHltbMainSides);
  const completionist = fmt(props.game.mHltbCompletionist);
  if (!main && !mainSides && !completionist) return null;

  const breakdown: string[] = [];
  if (main) breakdown.push(`Main Story ${main}`);
  if (mainSides) breakdown.push(`Main + Extras ${mainSides}`);
  if (completionist) breakdown.push(`Completionist ${completionist}`);

  return {
    primary: main ?? mainSides ?? completionist!,
    breakdown: breakdown.join(" · "),
  };
});
</script>
