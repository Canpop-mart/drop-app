<template>
  <div>
    <!-- Blurred banner backdrop. When the game has no real banner
         (mBannerObjectId is null), render the BannerFallback gradient
         instead of stretching the cover-style icon across the full
         hero — looks intentional rather than broken. -->
    <div class="absolute inset-x-0 top-0 z-0 h-[26rem] overflow-hidden">
      <img
        v-if="game.mBannerObjectId"
        :src="bannerUrl"
        class="h-full w-full scale-105 object-cover blur-[2px]"
      />
      <div v-else class="h-full w-full scale-105 blur-sm">
        <BannerFallback :name="game.mName" text-size="text-9xl" />
      </div>
      <div
        class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/65 to-zinc-950/10"
      />
      <div
        class="absolute inset-0 bg-gradient-to-r from-zinc-950/95 via-zinc-950/65 to-zinc-950/10"
      />
    </div>

    <div class="game-hero-layout relative z-10">
      <div class="min-w-0">
      <div class="max-w-6xl px-8">
        <h1
          class="max-w-4xl text-5xl font-bold font-display leading-none text-zinc-50 drop-shadow-2xl"
        >
          {{ game.mName }}
        </h1>

        <!-- Version / update status line. -->
        <div
          v-if="
            status.type === 'Installed' &&
            status.install_type.type != InstalledType.PartiallyInstalled
          "
          class="mt-2"
        >
          <!-- A republish of a pinned version is still an update, so the
               flag is checked before "pinned". -->
          <div
            v-if="updateAvailable"
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Update available <ArrowDownTrayIcon class="size-3 text-blue-600" />
          </div>
          <div
            v-else-if="!version?.userConfiguration?.enableUpdates"
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Version pinned
            <WrenchIcon class="size-3 text-blue-600" />
          </div>
          <div
            v-else
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Up to date <CheckCircleIcon class="size-3 text-green-600" />
          </div>
        </div>

        <p
          v-if="game.mShortDescription"
          class="mt-3 max-w-3xl text-[15px] leading-6 text-zinc-300/85 drop-shadow"
        >
          {{ game.mShortDescription }}
        </p>

        <!-- Action button row. -->
        <div class="mt-5 flex flex-row flex-wrap items-stretch gap-3">
          <!-- Do not add scale animations to this: https://stackoverflow.com/a/35683068 -->
          <GameStatusButton
            class="!rounded-xl !px-6 !py-3.5"
            :status="status"
            :disabled="launchInFlight"
            @install="$emit('install')"
            @launch="$emit('launch')"
            @launch-incognito="$emit('launch-incognito')"
            @queue="$emit('queue')"
            @kill="$emit('kill')"
            @resume="$emit('resume')"
          />
          <button
            v-if="showSettings"
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-white/10 bg-zinc-950/55 px-5 py-3 text-sm font-semibold text-zinc-200 shadow-lg backdrop-blur-xl transition-colors hover:bg-zinc-800 hover:text-white"
            @click="$emit('settings')"
          >
            <Cog6ToothIcon class="size-5" />
            Settings
          </button>
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
          <!-- No stream button here. The one that used to sit in this slot did
               nothing at all: its "Stream from {host}" branch emitted an event
               with no listener, and its "Stream" branch created a session on the
               server without launching a game, Moonlight, or anything else, with
               no way to stop it again. Requesting a stream needs a device to
               request it from, and that picker only exists in Big Picture, which
               is where remote play is driven from. -->
          <!-- Update opens the change review and updates the existing
               install in place. Side-by-side versions live in Settings. -->
          <button
            v-if="status.type === 'Installed' && updateAvailable"
            class="inline-flex items-center gap-x-2 rounded-xl bg-blue-600 px-6 font-semibold text-white shadow-xl shadow-blue-950/30 transition-colors hover:bg-blue-500 uppercase font-display"
            @click="$emit('update')"
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
        </div>
      </div>

      <!-- Stat bar — a stable summary, kept outside the tabs. -->
      <div
        v-if="!statsLoading"
        class="mx-8 mt-5 flex max-w-5xl flex-wrap items-center gap-x-5 gap-y-2 text-zinc-400"
      >
        <div
          v-if="gameStats.lastPlayedAt"
          class="flex items-center gap-2 text-xs"
        >
          <CalendarIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">Last Played</span>
          <span class="text-zinc-100 font-medium">{{
            formatLastPlayed(gameStats.lastPlayedAt)
          }}</span>
        </div>
        <div v-if="gameStats.lastPlayedAt" class="h-3 w-px bg-zinc-700" />
        <div class="flex items-center gap-2 text-xs">
          <ClockIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">Play Time</span>
          <span class="text-zinc-100 font-medium">{{
            formatPlaytime(gameStats.playtimeSeconds)
          }}</span>
        </div>
        <div
          v-if="gameStats.achievementsTotal > 0"
          class="h-3 w-px bg-zinc-700"
        />
        <div
          v-if="gameStats.achievementsTotal > 0"
          class="flex items-center gap-2 text-xs"
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

        <!-- Friends — server-mates who've played this game. Lives inline
             with the other stats now (previously sat in its own row
             below the stat bar, which made the page feel cluttered).
             Clicking it jumps the user to the Community tab where the
             full leaderboard lives. -->
        <div
          v-if="players && players.length > 0"
          class="h-3 w-px bg-zinc-700"
        />
        <button
          v-if="players"
          type="button"
          class="group -mx-1.5 -my-1 flex items-center gap-2 rounded-md px-1.5 py-1 text-xs transition-colors hover:bg-zinc-800/60"
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
              <!-- Avatar URLs go through a Tauri protocol so the desktop
                   webview can resolve them (a bare `/api/v1/object/...`
                   path 404s in the file:// origin the webview runs in). -->
              <img
                v-if="p.avatarObjectId"
                :src="objectImageUrl(p.avatarObjectId)"
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
      <aside class="game-hero-aside px-8 pt-6">
        <slot name="aside" />
      </aside>
    </div>
  </div>
</template>

<style scoped>
.game-hero-aside:not(:has([data-league-widget])) { display: none; }
@media (min-width: 1200px) {
  .game-hero-layout:has([data-league-widget]) {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(360px, 440px);
    gap: 40px;
    align-items: end;
  }
  .game-hero-aside {
    padding-top: 0;
    padding-left: 0;
    margin-top: -96px;
  }
}
</style>

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
  CalendarIcon,
  CheckCircleIcon,
  ClockIcon,
  TrophyIcon,
  UsersIcon,
  WrenchIcon,
} from "@heroicons/vue/24/solid";
import { Cog6ToothIcon } from "@heroicons/vue/24/outline";
import { InstalledType } from "~/types";
import type { Game, GameStatus, GameVersion } from "~/types";
import BannerFallback from "~/components/BannerFallback.vue";
import type { GamePlayerEntry } from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";
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
  // Some install of this game has an update (per-install flags from
  // `fetch_game_installs`, or the current install's status flag).
  updateAvailable?: boolean;
  showSettings?: boolean;
}>();

defineEmits<{
  (e: "install"): void;
  // The Update button: the page opens the in-place update review.
  (e: "update"): void;
  (e: "launch"): void;
  // Hidden Shift+click activation on the Play button — re-emitted up
  // so the page can route it to launchCtl.launchIncognito().
  (e: "launch-incognito"): void;
  (e: "queue"): void;
  (e: "kill"): void;
  (e: "resume"): void;
  (e: "compat-result", outcome: unknown): void;
  (e: "settings"): void;
  // Emitted when the inline Friends stat is clicked — the page should
  // switch to the Community tab where the full leaderboard lives.
  (e: "open-community"): void;
}>();

const achievementPercent = computed(() => {
  const { achievementsUnlocked, achievementsTotal } = props.gameStats;
  if (achievementsTotal <= 0) return 0;
  return Math.round((achievementsUnlocked / achievementsTotal) * 100);
});
</script>
