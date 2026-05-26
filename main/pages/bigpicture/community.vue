<template>
  <div
    class="flex flex-col h-full"
    :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }"
  >
    <!-- Tab navigation -->
    <div
      class="flex items-center gap-2 px-8 py-4 border-b"
      :style="{ borderColor: 'var(--bpm-border)' }"
    >
      <button
        v-for="tab in tabs"
        :key="tab.value"
        :ref="
          (el: any) =>
            registerTab(el, { onSelect: () => (activeTab = tab.value) })
        "
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
      <div class="h-28 rounded-2xl bg-zinc-800/50 animate-pulse mb-5" />
      <div class="grid grid-cols-2 gap-4 mb-5">
        <div class="h-16 rounded-xl bg-zinc-800/50 animate-pulse" />
        <div class="h-16 rounded-xl bg-zinc-800/50 animate-pulse" />
      </div>
      <div class="space-y-3">
        <div
          v-for="i in 6"
          :key="i"
          class="h-16 rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </div>
    </div>

    <!-- ═══ Activity tab ═══ -->
    <div
      v-else-if="activeTab === 'activity'"
      class="flex-1 overflow-y-auto px-8 py-6 space-y-5"
    >
      <!-- Header stats strip — collapses the old 4-tile grid into a slim
           inline row, freeing the recap card to be the page's only loud
           element. The "Players" tile (which doubled as a tab-switcher
           shortcut) becomes a tappable focusable item so BPM remote /
           keyboard users keep that affordance. -->
      <div class="flex items-center justify-end gap-2 text-sm text-zinc-400 font-medium">
        <button
          :ref="
            (el: any) =>
              registerContent(el, { onSelect: () => (activeTab = 'players') })
          "
          class="flex items-center gap-1.5 rounded-md px-2 py-1 -my-1 hover:bg-zinc-800/60 focus:bg-zinc-800/60 transition-colors"
          @click="activeTab = 'players'"
        >
          <span class="size-1.5 rounded-full bg-green-500 pulse-dot" />
          <span class="text-zinc-200 tabular-nums">{{
            stats.totalUsers.toLocaleString()
          }}</span>
          players
        </button>
        <span class="text-zinc-700">·</span>
        <span>
          <span class="text-zinc-200 tabular-nums">{{
            stats.totalGames.toLocaleString()
          }}</span>
          games
        </span>
        <span class="text-zinc-700">·</span>
        <span>
          <span class="text-zinc-200 tabular-nums"
            >{{ stats.totalPlaytimeHours.toLocaleString() }}h</span
          >
          played
        </span>
        <span class="text-zinc-700">·</span>
        <span>
          <span class="text-zinc-200 tabular-nums">{{
            stats.totalAchievementUnlocks.toLocaleString()
          }}</span>
          unlocks
        </span>
      </div>

      <!-- HERO: weekly recap is the only loud card. Layout mirrors the
           desktop CommunityWeeklyRecap component (thumbnail anchor +
           kicker / headline / meta), just with BPM focus delegate
           wiring and slightly larger touch targets for the 10-foot UI. -->
      <div
        v-if="weeklyRecap.length > 0"
        :ref="(el: any) => registerContent(el, { onSelect: onRecapSelect })"
        class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-indigo-900/40 via-zinc-900/60 to-purple-900/30 ring-1 ring-indigo-500/20 cursor-pointer"
        @click="onRecapSelect"
      >
        <transition name="bpm-slide-fade" mode="out-in">
          <div
            v-if="activeRecapSlide"
            :key="recapIndex"
            class="flex items-center gap-5 px-6 py-5"
          >
            <!-- Thumbnail anchor — cover for game-led slides, avatar for
                 player-led slides, kind emoji as a last resort. -->
            <div
              class="shrink-0 size-24 rounded-xl overflow-hidden ring-1 ring-indigo-400/20 flex items-center justify-center"
              :class="
                activeRecapSlide.coverObjectId ||
                activeRecapSlide.avatarObjectId
                  ? 'bg-zinc-900/80'
                  : 'bg-indigo-500/15'
              "
            >
              <img
                v-if="activeRecapSlide.coverObjectId"
                :src="objectUrl(activeRecapSlide.coverObjectId)"
                :alt="activeRecapSlide.headline"
                class="w-full h-full object-cover"
              />
              <img
                v-else-if="activeRecapSlide.avatarObjectId"
                :src="objectUrl(activeRecapSlide.avatarObjectId)"
                :alt="activeRecapSlide.headline"
                class="w-full h-full object-cover"
              />
              <span v-else class="text-3xl">{{ recapSlideEmoji }}</span>
            </div>

            <div class="flex-1 min-w-0 pr-12">
              <p
                class="text-[10px] tracking-[0.2em] uppercase text-indigo-300/80 font-medium mb-1 truncate"
              >
                {{ activeRecapSlide.title }}
              </p>
              <p
                class="text-2xl font-display font-bold text-zinc-100 leading-tight truncate"
              >
                {{ activeRecapSlide.headline }}
              </p>
              <p class="text-sm text-zinc-400 mt-0.5 truncate">
                {{ activeRecapSlide.meta }}
              </p>
            </div>
          </div>
        </transition>
        <div
          v-if="weeklyRecap.length > 1"
          class="absolute bottom-3 right-4 flex gap-1.5 z-10"
        >
          <button
            v-for="(_, i) in weeklyRecap"
            :key="i"
            class="size-2 rounded-full transition-all"
            :class="i === recapIndex ? 'bg-indigo-300 w-5' : 'bg-indigo-300/30'"
            @click.stop="recapIndex = i"
          />
        </div>
      </div>

      <!-- Around right now strip — hidden on this layout (the data is
           now surfaced inline on the Players / Leaderboard rows as
           "online dot + currently-playing game"). Set
           SHOW_AROUND_NOW_STRIP to true to restore the standalone strip. -->
      <div
        v-if="SHOW_AROUND_NOW_STRIP && nowPlaying.length > 0"
        class="flex items-center gap-3 rounded-xl bg-zinc-900/40 ring-1 ring-zinc-800/60 px-4 py-2.5"
      >
        <div class="flex items-center gap-1.5 shrink-0">
          <span class="size-1.5 rounded-full bg-green-500 pulse-dot" />
          <span
            class="text-[11px] tracking-[0.15em] uppercase text-zinc-400 font-medium"
            >Around now</span
          >
        </div>
        <div class="w-px h-4 bg-zinc-800 shrink-0" />
        <div
          class="flex items-center gap-2 min-w-0 overflow-x-auto"
          style="scrollbar-width: none"
        >
          <template
            v-for="(entry, i) in nowPlaying"
            :key="`${entry.userId}-${entry.startedAt}`"
          >
            <button
              :ref="
                (el: any) =>
                  registerContent(el, { onSelect: () => goToGame(entry.game.id) })
              "
              class="flex items-center gap-1.5 shrink-0 group"
              @click="goToGame(entry.game.id)"
            >
              <img
                v-if="entry.avatarObjectId"
                :src="objectUrl(entry.avatarObjectId)"
                class="size-5 rounded-full object-cover"
              />
              <div
                v-else
                class="size-5 rounded-full bg-emerald-700/60 flex items-center justify-center text-[9px] font-bold text-zinc-100"
              >
                {{ entry.displayName[0]?.toUpperCase() }}
              </div>
              <span class="text-xs font-medium text-zinc-200">{{
                entry.displayName
              }}</span>
              <span class="text-xs text-zinc-500">in</span>
              <span
                class="text-xs text-blue-400 group-hover:text-blue-300 transition-colors truncate max-w-[14rem]"
                >{{ entry.game.name }}</span
              >
            </button>
            <span v-if="i < nowPlaying.length - 1" class="text-zinc-700 text-xs"
              >·</span
            >
          </template>
        </div>
      </div>

      <!-- Weekly Quest card is hidden for now; flip SHOW_WEEKLY_QUEST to
           bring it back. Game Roulette used to share this row but has
           moved to the store's Browse tab where "I want to play
           something" energy belongs. -->
      <div v-if="SHOW_WEEKLY_QUEST && weeklyChallenge" class="space-y-4">
        <div
          class="rounded-xl bg-zinc-800/50 ring-1 ring-zinc-700/40 px-4 py-3 hover:ring-amber-500/40 transition"
        >
          <div class="flex items-center gap-3">
            <div
              class="shrink-0 size-9 rounded-full bg-amber-500/15 flex items-center justify-center text-base"
            >
              {{ weeklyChallengeEmoji }}
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-baseline justify-between gap-2">
                <p class="text-sm font-medium text-zinc-100 truncate">
                  {{ weeklyChallenge.title }}
                  <span class="text-zinc-500 font-normal"
                    >— {{ weeklyChallenge.description }}</span
                  >
                </p>
                <span
                  class="text-[10px] text-zinc-500 shrink-0 tabular-nums"
                  >{{ weeklyChallengeDaysSuffix }}</span
                >
              </div>
              <div class="mt-2 flex items-center gap-3">
                <div
                  class="relative h-1.5 flex-1 rounded-full bg-zinc-900 overflow-hidden"
                >
                  <div
                    class="absolute inset-y-0 left-0 rounded-full bg-amber-500/80 transition-[width] duration-700 ease-out"
                    :style="{ width: `${weeklyChallenge.percentComplete}%` }"
                  />
                </div>
                <p
                  class="text-[11px] font-medium text-zinc-400 tabular-nums shrink-0"
                >
                  {{ formatChallengeValue(weeklyChallenge.currentValue) }} /
                  {{ formatChallengeValue(weeklyChallenge.targetValue) }}
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Activity type filter -->
      <div class="flex gap-2 mb-4">
        <button
          v-for="filter in activityFilters"
          :key="filter.value"
          :ref="
            (el: any) =>
              registerTab(el, {
                onSelect: () => (activityFilter = filter.value),
              })
          "
          class="px-3 py-1 rounded-full text-xs font-medium transition-colors"
          :class="
            activityFilter === filter.value
              ? 'bg-blue-600/20 text-blue-400'
              : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
          "
          @click="activityFilter = filter.value"
        >
          {{ filter.label }}
        </button>
      </div>

      <!-- Activity feed (clustered) -->
      <div class="space-y-2">
        <div
          v-for="cluster in clusteredActivity"
          :key="cluster.key"
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => goToGame(cluster.game?.id),
              })
          "
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4"
        >
          <div
            class="size-10 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden"
          >
            <img
              v-if="cluster.user.profilePictureObjectId"
              :src="objectUrl(cluster.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold"
            >
              {{
                (cluster.user.displayName || cluster.user.username || "?")[0]
              }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <!--
              Same fix as the desktop row: bare `<template v-if>` text was
              getting stripped of surrounding whitespace, mashing words
              together. The verb segment is now a `<span>` wrapper with
              hardcoded `&nbsp;` padding so the spaces sit inside a text
              node (where Vue's compiler always preserves them). Verb copy
              "was in" → "was playing" because "was in <game>" reads like
              a location.
            -->
            <p class="text-sm text-zinc-200">
              <span class="font-medium">{{
                cluster.user.displayName || cluster.user.username
              }}</span>
              <span
                >&nbsp;{{
                  cluster.kind === "request"
                    ? "requested"
                    : cluster.totalDuration && cluster.totalDuration > 0
                      ? "played"
                      : cluster.achievements.length > 0
                        ? "unlocked achievements in"
                        : "was playing"
                }}&nbsp;</span
              >
              <span
                v-if="cluster.kind === 'request' && cluster.request"
                class="text-purple-400"
                >{{ cluster.request.title }}</span
              >
              <span v-else-if="cluster.game" class="text-blue-400">{{
                cluster.game.mName
              }}</span>
              <span
                v-if="
                  cluster.kind !== 'request' &&
                  cluster.totalDuration &&
                  cluster.totalDuration > 0
                "
                class="text-zinc-500"
                >&nbsp;for {{ formatDuration(cluster.totalDuration) }}</span
              >
            </p>
            <p
              v-if="cluster.achievements.length > 0"
              class="text-xs text-zinc-400 mt-1"
            >
              <span class="text-yellow-500 mr-1">unlocked</span>
              <template
                v-for="(ach, i) in cluster.achievements.slice(0, 3)"
                :key="ach.id"
              >
                <span class="text-yellow-300">{{ ach.title }}</span>
                <span
                  v-if="i < Math.min(cluster.achievements.length, 3) - 1"
                  class="text-zinc-500"
                  >,
                </span>
              </template>
              <span v-if="cluster.achievements.length > 3" class="text-zinc-500"
                >, +{{ cluster.achievements.length - 3 }} more</span
              >
            </p>
            <p class="text-xs text-zinc-600 mt-0.5">
              {{ formatTimeAgo(cluster.timestamp) }}
            </p>
          </div>

          <div
            v-if="cluster.game?.mCoverObjectId && cluster.kind !== 'request'"
            class="rounded-lg overflow-hidden flex-shrink-0 bg-zinc-800"
          >
            <img
              :src="objectUrl(cluster.game.mCoverObjectId)"
              class="h-20 w-[3.75rem] object-cover"
              loading="lazy"
            />
          </div>
        </div>
      </div>

      <div v-if="activity.length >= 30" class="flex justify-center py-6">
        <button
          :ref="
            (el: any) => registerContent(el, { onSelect: loadMoreActivity })
          "
          class="px-6 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm font-medium hover:bg-zinc-700 transition-colors"
          @click="loadMoreActivity"
        >
          Load More
        </button>
      </div>
    </div>

    <!-- ═══ Players tab ═══ -->
    <div
      v-else-if="activeTab === 'players'"
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <div class="space-y-2">
        <div
          v-for="entry in leaderboard"
          :key="entry.user.id"
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => viewProfile(entry.user.id),
              })
          "
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4 cursor-pointer"
        >
          <div
            class="size-12 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden"
          >
            <img
              v-if="entry.user.profilePictureObjectId"
              :src="objectUrl(entry.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-500 text-lg font-bold"
            >
              {{ entry.user.displayName[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p
              class="text-sm font-medium text-zinc-200 truncate flex items-center gap-1"
            >
              <span class="truncate">{{ entry.user.displayName }}</span>
              <span
                v-if="mvp && entry.user.id === mvp.userId"
                :title="mvpTooltip"
                class="text-yellow-400 shrink-0"
                aria-label="Today's MVP"
                >👑</span
              >
            </p>
            <!--
              Two states: online (green dot + game name) vs offline
              (@username). We prefer the online line because it's the
              actionable signal — the row tap target already goes to the
              profile, so the in-game name is a soft hint, not a button.
            -->
            <p
              v-if="nowPlayingByUser.get(entry.user.id)"
              class="text-xs text-zinc-400 flex items-center gap-1.5 min-w-0"
            >
              <span class="size-1.5 rounded-full bg-green-500 pulse-dot shrink-0" />
              <span class="truncate">
                {{ nowPlayingByUser.get(entry.user.id)!.game.name }}
              </span>
            </p>
            <p v-else class="text-xs text-zinc-600">@{{ entry.user.username }}</p>
          </div>

          <div class="flex gap-6 text-right">
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.playtimeHours.toLocaleString() }}h
              </p>
              <p class="text-xs text-zinc-600">Playtime</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.gamesPlayed }}
              </p>
              <p class="text-xs text-zinc-600">Games</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.achievements }}
              </p>
              <p class="text-xs text-zinc-600">Achievements</p>
            </div>
          </div>
        </div>

        <p
          v-if="leaderboard.length === 0"
          class="text-zinc-500 text-center py-12 text-sm"
        >
          No players found.
        </p>
      </div>
    </div>

    <!-- ═══ Leaderboard tab ═══ -->
    <div
      v-else-if="activeTab === 'leaderboard'"
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <div class="space-y-2">
        <div
          v-for="entry in leaderboard"
          :key="entry.user.id"
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => viewProfile(entry.user.id),
              })
          "
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4 cursor-pointer"
        >
          <div class="w-8 text-center flex-shrink-0">
            <span
              class="text-lg font-bold"
              :class="
                entry.rank <= 3 ? rankColors[entry.rank - 1] : 'text-zinc-500'
              "
            >
              {{ entry.rank }}
            </span>
          </div>

          <div
            class="size-10 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden"
          >
            <img
              v-if="entry.user.profilePictureObjectId"
              :src="objectUrl(entry.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold"
            >
              {{ entry.user.displayName[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p
              class="text-sm font-medium text-zinc-200 truncate flex items-center gap-1"
            >
              <span class="truncate">{{ entry.user.displayName }}</span>
              <span
                v-if="mvp && entry.user.id === mvp.userId"
                :title="mvpTooltip"
                class="text-yellow-400 shrink-0"
                aria-label="Today's MVP"
                >👑</span
              >
            </p>
            <!--
              Two states: online (green dot + game name) vs offline
              (@username). We prefer the online line because it's the
              actionable signal — the row tap target already goes to the
              profile, so the in-game name is a soft hint, not a button.
            -->
            <p
              v-if="nowPlayingByUser.get(entry.user.id)"
              class="text-xs text-zinc-400 flex items-center gap-1.5 min-w-0"
            >
              <span class="size-1.5 rounded-full bg-green-500 pulse-dot shrink-0" />
              <span class="truncate">
                {{ nowPlayingByUser.get(entry.user.id)!.game.name }}
              </span>
            </p>
            <p v-else class="text-xs text-zinc-600">@{{ entry.user.username }}</p>
          </div>

          <div class="flex gap-6 text-right">
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.playtimeHours.toLocaleString() }}h
              </p>
              <p class="text-xs text-zinc-600">Playtime</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.gamesPlayed }}
              </p>
              <p class="text-xs text-zinc-600">Games</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.achievements }}
              </p>
              <p class="text-xs text-zinc-600">Achievements</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ Shared Shelves tab ═══ -->
    <div
      v-else-if="activeTab === 'shelves'"
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <div v-if="sharedShelves.length > 0" class="space-y-8">
        <div v-for="shelf in sharedShelves" :key="shelf.id">
          <div class="flex items-center gap-3 mb-3">
            <div
              class="size-8 rounded-full bg-zinc-800 overflow-hidden flex-shrink-0"
            >
              <img
                v-if="shelf.user.profilePictureObjectId"
                :src="objectUrl(shelf.user.profilePictureObjectId)"
                class="w-full h-full object-cover"
              />
              <div
                v-else
                class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold"
              >
                {{ shelf.user.displayName[0] }}
              </div>
            </div>
            <div>
              <h3
                class="text-base font-semibold"
                style="color: var(--bpm-text)"
              >
                {{ shelf.name }}
              </h3>
              <p class="text-xs" style="color: var(--bpm-muted)">
                by {{ shelf.user.displayName }} &middot;
                {{ shelf.entries.length }} game{{
                  shelf.entries.length !== 1 ? "s" : ""
                }}
              </p>
            </div>
          </div>
          <div
            class="flex gap-4 overflow-x-auto pb-3 px-1"
            style="scrollbar-width: thin"
          >
            <div
              v-for="entry in shelf.entries"
              :key="entry.gameId"
              class="flex-shrink-0"
              style="width: 9rem"
              :ref="
                (el: any) =>
                  registerContent(el, {
                    onSelect: () => goToGame(entry.gameId),
                  })
              "
            >
              <div
                class="rounded-lg overflow-hidden cursor-pointer transition-transform hover:scale-105"
                style="aspect-ratio: 3/4"
              >
                <img
                  v-if="entry.game.mCoverObjectId"
                  :src="objectUrl(entry.game.mCoverObjectId)"
                  class="w-full h-full object-cover"
                  loading="lazy"
                />
                <div
                  v-else
                  class="w-full h-full flex items-center justify-center text-lg font-bold"
                  style="
                    background-color: var(--bpm-surface);
                    color: var(--bpm-accent-hex);
                  "
                >
                  {{ entry.game.mName[0] }}
                </div>
              </div>
              <p class="text-xs mt-1 truncate" style="color: var(--bpm-text)">
                {{ entry.game.mName }}
              </p>
            </div>
          </div>
        </div>
      </div>
      <div v-else class="flex items-center justify-center py-24">
        <div class="text-center">
          <h3 class="text-xl font-semibold mb-2" style="color: var(--bpm-text)">
            No shared shelves yet
          </h3>
          <p class="text-sm" style="color: var(--bpm-muted)">
            Make a shelf public from the Library &rarr; Shelves tab to share it
            with the community
          </p>
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
  type NowPlayingEntry,
  type WeeklyRecapSlide,
  type MvpToday,
  type WeeklyChallenge,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { clusterActivity } from "~/composables/use-community-clusters";

definePageMeta({ layout: "bigpicture" });

// Layout flags — flip to `true` to bring the hidden cards back. We use
// `const` rather than `v-if="false"` because Vue's template typechecker
// loses narrowing on null-guarded children inside a literal `false`
// branch, but a boolean-typed reference combined with the original
// truthy condition narrows cleanly (`v-if="SHOW_X && weeklyChallenge"`
// still narrows weeklyChallenge to non-null inside the block).
const SHOW_AROUND_NOW_STRIP = false;
const SHOW_WEEKLY_QUEST = false;

const api = useServerApi();
const router = useRouter();
const focusNav = useFocusNavigation();

const registerTab = useBpFocusableGroup("content");
const registerContent = useBpFocusableGroup("content");

const loading = ref(true);
const activeTab = ref("activity");
const activityFilter = ref("all");

const stats = ref<CommunityStats>({
  totalGames: 0,
  totalUsers: 0,
  totalPlaytimeHours: 0,
  totalPlaySessions: 0,
  totalAchievementUnlocks: 0,
  totalRequests: 0,
  pendingRequests: 0,
  totalLeaderboardEntries: 0,
});
const activity = ref<CommunityActivityItem[]>([]);
const leaderboard = ref<LeaderboardUser[]>([]);
const nowPlaying = ref<NowPlayingEntry[]>([]);
const weeklyRecap = ref<WeeklyRecapSlide[]>([]);
const mvp = ref<MvpToday | null>(null);
const weeklyChallenge = ref<WeeklyChallenge | null>(null);
const recapIndex = ref(0);

// userId → currently-playing entry, for the Players / Leaderboard rows.
// The standalone "Around now" strip is hidden on this layout; we recycle
// the same data into each leaderboard row as a green-dot + game-name
// decoration, so a user can see at a glance who's online without a
// separate widget.
const nowPlayingByUser = computed(() => {
  const map = new Map<string, NowPlayingEntry>();
  for (const entry of nowPlaying.value) {
    map.set(entry.userId, entry);
  }
  return map;
});

const weeklyChallengeDaysSuffix = computed(() => {
  if (!weeklyChallenge.value) return "";
  const d = weeklyChallenge.value.daysRemaining;
  if (d <= 0) return "ends today";
  if (d === 1) return "1 day left";
  return `${d} days left`;
});

function formatChallengeValue(n: number): string {
  if (!weeklyChallenge.value) return String(n);
  // `play_hours` and `genre_focus` measure whole hours; the other seven
  // kinds are raw counts (including binary 0/1 quests).
  const isHours =
    weeklyChallenge.value.kind === "play_hours" ||
    weeklyChallenge.value.kind === "genre_focus";
  return isHours ? `${n}h` : n.toLocaleString();
}

// Per-kind emoji for the BPM card — mirrors the heroicon mapping in
// `CommunityWeeklyChallenge.vue` (the desktop variant). Kept inline rather
// than imported so BPM doesn't have to drag in the heroicons set just for
// this card.
const weeklyChallengeEmoji = computed(() => {
  if (!weeklyChallenge.value) return "⏱";
  switch (weeklyChallenge.value.kind) {
    case "play_hours":
      return "⏱";
    case "unlock_count":
      return "🏆";
    case "play_variety":
      return "🎲";
    case "rediscover":
      return "⏳";
    case "marathon":
      return "🏃";
    case "night_owl":
      return "🌙";
    case "new_to_you":
      return "✨";
    case "genre_focus":
      return "🏷";
    case "fresh_drop":
      return "🌱";
    default:
      return "⏱";
  }
});

const mvpTooltip = computed(() => {
  if (!mvp.value) return "";
  const hours = Math.max(1, Math.round(mvp.value.sessionSeconds / 3600));
  const playLabel =
    mvp.value.sessionSeconds === 0 ? "no playtime" : `${hours}h playtime`;
  return `Today's MVP — ${playLabel} · ${mvp.value.achievementsUnlocked} achievement${mvp.value.achievementsUnlocked === 1 ? "" : "s"}`;
});

let recapTimer: ReturnType<typeof setInterval> | null = null;

const rankColors = ["text-yellow-400", "text-zinc-300", "text-amber-600"];

const tabs = [
  { label: "Activity", value: "activity" },
  { label: "Players", value: "players" },
  { label: "Leaderboard", value: "leaderboard" },
  { label: "Shared Shelves", value: "shelves" },
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

const clusteredActivity = computed(() =>
  clusterActivity(filteredActivity.value),
);

const activeRecapSlide = computed(
  () => weeklyRecap.value[recapIndex.value] ?? weeklyRecap.value[0],
);

// Per-kind fallback glyph for the thumbnail slot, used only when the
// slide has neither a game cover nor a user avatar to render. Mirrors
// the same mapping the desktop CommunityWeeklyRecap component uses.
const recapSlideEmoji = computed(() => {
  switch (activeRecapSlide.value?.kind) {
    case "top_game":
      return "🎮";
    case "longest_session":
      return "⏱";
    case "most_unlocks":
      return "🏆";
    case "milestone":
      return "🚀";
    case "new_player":
      return "👋";
    default:
      return "✨";
  }
});

function onRecapSelect() {
  const slide = activeRecapSlide.value;
  if (!slide) return;
  if (slide.gameId) goToGame(slide.gameId);
  else if (slide.userId) viewProfile(slide.userId);
}

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId?: string) {
  if (!gameId) return;
  const target = `/bigpicture/library/${gameId}`;
  focusNav.setRouteState("backTo", "/bigpicture/community", target);
  router.push(target);
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

// Shared shelves data
interface SharedShelf {
  id: string;
  name: string;
  user: {
    id: string;
    username: string;
    displayName: string;
    profilePictureObjectId: string | null;
  };
  entries: Array<{
    gameId: string;
    game: {
      id: string;
      mName: string;
      mCoverObjectId: string | null;
      mIconObjectId: string | null;
    };
  }>;
}
const sharedShelves = ref<SharedShelf[]>([]);

function startRecapTimer() {
  if (recapTimer) clearInterval(recapTimer);
  if (weeklyRecap.value.length > 1) {
    recapTimer = setInterval(() => {
      recapIndex.value = (recapIndex.value + 1) % weeklyRecap.value.length;
    }, 8000);
  }
}

onMounted(async () => {
  try {
    const [
      statsData,
      activityData,
      leaderboardData,
      shelvesData,
      nowPlayingData,
      recapData,
      mvpData,
      weeklyChallengeData,
    ] = await Promise.all([
      api.community.stats().catch(() => stats.value),
      api.community.activity().catch(() => []),
      api.community.leaderboard().catch(() => ({ playtime: [] })),
      fetch(serverUrl("api/v1/community/shelves"))
        .then((r) => (r.ok ? r.json() : []))
        .catch(() => []),
      api.community.nowPlaying().catch(() => []),
      api.community.weeklyRecap().catch(() => []),
      api.community.mvpToday().catch(() => null),
      api.community.weeklyChallenge().catch(() => null),
    ]);
    stats.value = statsData;
    activity.value = activityData;
    leaderboard.value = leaderboardData.playtime;
    sharedShelves.value = shelvesData;
    nowPlaying.value = nowPlayingData;
    weeklyRecap.value = recapData;
    mvp.value = mvpData;
    weeklyChallenge.value = weeklyChallengeData;
    startRecapTimer();
  } catch (e) {
    console.error("Failed to load community data:", e);
  } finally {
    loading.value = false;
    nextTick(() => focusNav.autoFocusContent("content"));
  }
});

onUnmounted(() => {
  if (recapTimer) clearInterval(recapTimer);
});

watch(
  () => weeklyRecap.value.length,
  () => {
    recapIndex.value = 0;
    startRecapTimer();
  },
);
</script>

<style scoped>
.bpm-slide-fade-enter-active,
.bpm-slide-fade-leave-active {
  transition:
    opacity 350ms ease,
    transform 350ms ease;
}
.bpm-slide-fade-enter-from {
  opacity: 0;
  transform: translateY(6px);
}
.bpm-slide-fade-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

/* Pulsing green dot used in the header stats strip + the "Around now"
   presence strip. */
.pulse-dot {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}
@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

/* Hide the horizontal scrollbar on the "Around now" presence strip
   while still allowing wheel/touch scroll on overflow. */
.overflow-x-auto::-webkit-scrollbar {
  display: none;
}
</style>
