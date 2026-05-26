<template>
  <!-- Three stacked sections — leaderboard, first-to-unlock, recent
       activity. Designed to slot inside a CollapsibleSection sidebar
       on the library detail page, so the parent owns the section
       header.  Padding + sizing are tuned to read cleanly in a 360px
       sidebar without feeling smushed. -->
  <div class="space-y-5">
    <!-- ── Leaderboard ────────────────────────────────────────────────── -->
    <div>
      <div class="flex items-center gap-2 mb-2.5">
        <TrophyIcon class="size-4 text-yellow-500" />
        <h4 class="text-xs font-display font-semibold text-zinc-200 uppercase tracking-wider">
          Leaderboard
        </h4>
        <span class="text-[10px] text-zinc-500 ml-auto tabular-nums">
          {{ players.length }} {{ players.length === 1 ? "player" : "players" }}
        </span>
      </div>
      <div v-if="playersLoading" class="text-sm text-zinc-500 py-3">
        Loading…
      </div>
      <div
        v-else-if="players.length === 0"
        class="text-sm text-zinc-500 py-3"
      >
        No data yet
      </div>
      <ol
        v-else
        class="divide-y divide-zinc-700/40 rounded-lg bg-zinc-900/40 ring-1 ring-zinc-800/40 overflow-hidden"
      >
        <li
          v-for="(p, idx) in players"
          :key="p.userId"
          class="flex items-center gap-3 px-3.5 py-3"
        >
          <span
            class="size-6 rounded-full flex items-center justify-center text-[11px] font-bold shrink-0 tabular-nums"
            :class="rankColor(idx + 1)"
          >
            {{ idx + 1 }}
          </span>
          <img
            v-if="p.avatarObjectId"
            :src="avatarUrl(p.avatarObjectId)"
            :alt="p.displayName"
            class="size-8 rounded-full object-cover bg-zinc-700 shrink-0"
            referrerpolicy="no-referrer"
          />
          <div
            v-else
            class="size-8 rounded-full bg-zinc-700 flex items-center justify-center shrink-0"
          >
            <span class="text-xs font-semibold text-zinc-300 uppercase">
              {{ initial(p.displayName) }}
            </span>
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-zinc-100 truncate">
              {{ p.displayName }}
            </p>
            <p class="text-xs text-zinc-500 mt-0.5">
              {{ formatPlaytime(p.playtimeSeconds) }}
              <span v-if="p.achievementsTotal > 0">
                · {{ p.achievementsUnlocked }}/{{ p.achievementsTotal }}
              </span>
            </p>
          </div>
        </li>
      </ol>
    </div>

    <!-- ── First-to-unlock achievements ─────────────────────────────── -->
    <div>
      <div class="flex items-center gap-2 mb-2.5">
        <SparklesIcon class="size-4 text-yellow-400" />
        <h4 class="text-xs font-display font-semibold text-zinc-200 uppercase tracking-wider">
          First to unlock
        </h4>
        <span class="text-[10px] text-zinc-500 ml-auto">on this server</span>
      </div>
      <div v-if="firstsLoading" class="text-sm text-zinc-500 py-3">
        Loading…
      </div>
      <div
        v-else-if="firsts.length === 0"
        class="text-sm text-zinc-500 py-3"
      >
        No data yet
      </div>
      <div
        v-else
        class="flex gap-2.5 overflow-x-auto pb-2 firsts-scroll"
      >
        <div
          v-for="f in firsts"
          :key="f.achievementId"
          class="shrink-0 w-36 rounded-lg bg-zinc-900/50 p-3 ring-1 ring-yellow-500/25"
        >
          <img
            v-if="f.achievementIconUrl"
            :src="f.achievementIconUrl"
            class="size-10 rounded ring-2 ring-yellow-500/60 mb-2"
            referrerpolicy="no-referrer"
            :alt="f.achievementName"
          />
          <div
            v-else
            class="size-10 rounded ring-2 ring-yellow-500/60 bg-zinc-800 flex items-center justify-center mb-2"
          >
            <TrophyIcon class="size-5 text-yellow-500" />
          </div>
          <p
            class="text-xs font-medium text-zinc-100 leading-tight line-clamp-2 mb-1.5"
          >
            {{ f.achievementName }}
          </p>
          <p class="text-[11px] text-yellow-400/90 truncate font-medium">
            {{ f.displayName }}
          </p>
          <p
            class="text-[10px] text-zinc-500 mt-0.5"
            :title="fullTime(f.unlockedAt)"
          >
            {{ relativeTime(f.unlockedAt) }}
          </p>
        </div>
      </div>
    </div>

    <!-- ── Activity for this game ───────────────────────────────────── -->
    <div>
      <div class="flex items-center gap-2 mb-2.5">
        <BoltIcon class="size-4 text-blue-400" />
        <h4 class="text-xs font-display font-semibold text-zinc-200 uppercase tracking-wider">
          Recent activity
        </h4>
      </div>
      <div v-if="activityLoading" class="text-sm text-zinc-500 py-3">
        Loading…
      </div>
      <div
        v-else-if="activity.length === 0"
        class="text-sm text-zinc-500 py-3"
      >
        No data yet
      </div>
      <ul
        v-else
        class="divide-y divide-zinc-700/40 rounded-lg bg-zinc-900/40 ring-1 ring-zinc-800/40 overflow-hidden"
      >
        <li
          v-for="(item, i) in activity"
          :key="`${item.type}-${item.timestamp}-${i}`"
          class="flex items-start gap-3 px-3.5 py-3"
        >
          <img
            v-if="item.user?.profilePictureObjectId"
            :src="avatarUrl(item.user.profilePictureObjectId)"
            class="size-8 rounded-full object-cover bg-zinc-700 shrink-0"
            referrerpolicy="no-referrer"
          />
          <div
            v-else
            class="size-8 rounded-full bg-zinc-700 flex items-center justify-center shrink-0"
          >
            <UserIcon class="size-4 text-zinc-500" />
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm text-zinc-300 leading-snug">
              <span class="font-medium text-zinc-100">{{
                item.user?.displayName || item.user?.username || "Someone"
              }}</span>
              <template v-if="item.type === 'session'">
                played
                <template v-if="item.data?.duration">
                  for
                  <span class="text-zinc-200">{{
                    formatPlaytime(item.data.duration)
                  }}</span>
                </template>
              </template>
              <template v-else-if="item.type === 'achievement'">
                unlocked
                <span class="font-medium text-yellow-400">
                  {{ item.data?.achievement?.title }}
                </span>
              </template>
            </p>
            <p class="text-[11px] text-zinc-500 mt-1">
              {{ relativeTime(item.timestamp) }}
            </p>
          </div>
        </li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Per-game Community tab body. Shows three stacked sections:
 *   • Leaderboard — all server users with any playtime, ranked desc.
 *   • First-to-unlock — horizontal scroll of achievement icons that
 *     someone on the server unlocked first, with their name + when.
 *   • Recent activity — sessions + achievement unlocks for this game,
 *     across all users on this Drop instance.
 *
 * Three independent fetches; each soft-fails to a "No data yet" empty
 * state. Owned endpoints live in `community.gamePlayers / gameActivity /
 * gameFirsts` (Agent C). The parent page passes the players + firsts
 * lists in via props so they can be fetched once and reused by the
 * Friends tile and the Achievements tab.
 */
import {
  TrophyIcon,
  SparklesIcon,
  BoltIcon,
  UserIcon,
} from "@heroicons/vue/24/solid";
import {
  useServerApi,
  type GamePlayerEntry,
  type GameAchievementFirst,
  type CommunityActivityItem,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { formatPlaytime } from "~/composables/game-detail/use-game-stats";

const props = defineProps<{
  gameId: string;
  /** Page-level fetch results, passed in to avoid duplicate calls. */
  players?: GamePlayerEntry[];
  firsts?: GameAchievementFirst[];
}>();

const api = useServerApi();

const players = ref<GamePlayerEntry[]>(props.players ?? []);
const playersLoading = ref(props.players === undefined);

const firsts = ref<GameAchievementFirst[]>(props.firsts ?? []);
const firstsLoading = ref(props.firsts === undefined);

const activity = ref<CommunityActivityItem[]>([]);
const activityLoading = ref(true);

watch(
  () => props.players,
  (v) => {
    if (v) {
      players.value = v;
      playersLoading.value = false;
    }
  },
);
watch(
  () => props.firsts,
  (v) => {
    if (v) {
      firsts.value = v;
      firstsLoading.value = false;
    }
  },
);

onMounted(async () => {
  // Players + firsts are only fetched here as a fallback for cases where
  // the parent hasn't seeded them (e.g. the tab is mounted independently).
  if (props.players === undefined) {
    api.community
      .gamePlayers(props.gameId)
      .then((p) => (players.value = p))
      .catch(() => (players.value = []))
      .finally(() => (playersLoading.value = false));
  }
  if (props.firsts === undefined) {
    api.community
      .gameFirsts(props.gameId)
      .then((f) => (firsts.value = f))
      .catch(() => (firsts.value = []))
      .finally(() => (firstsLoading.value = false));
  }

  // Activity is always fetched here — the tab is the only consumer.
  api.community
    .gameActivity(props.gameId, 20)
    .then((a) => (activity.value = a))
    .catch(() => (activity.value = []))
    .finally(() => (activityLoading.value = false));
});

function avatarUrl(objectId: string): string {
  return serverUrl(`api/v1/object/${objectId}`);
}

function initial(name: string): string {
  return (name || "?").trim().charAt(0) || "?";
}

function rankColor(rank: number): string {
  if (rank === 1)
    return "bg-yellow-500/20 text-yellow-300 ring-1 ring-yellow-500/40";
  if (rank === 2)
    return "bg-zinc-400/20 text-zinc-200 ring-1 ring-zinc-400/40";
  if (rank === 3)
    return "bg-orange-700/30 text-orange-300 ring-1 ring-orange-700/40";
  return "bg-zinc-700/50 text-zinc-400";
}

function relativeTime(dateStr: string): string {
  const diff = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  if (diff < 31536000) return `${Math.floor(diff / 604800)}w ago`;
  return `${Math.floor(diff / 31536000)}y ago`;
}

function fullTime(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleString();
  } catch {
    return dateStr;
  }
}
</script>

<style scoped>
.firsts-scroll {
  scrollbar-width: thin;
  scrollbar-color: rgb(82 82 91) transparent;
}
.firsts-scroll::-webkit-scrollbar {
  height: 6px;
}
.firsts-scroll::-webkit-scrollbar-thumb {
  background-color: rgb(82 82 91);
  border-radius: 3px;
}

.line-clamp-2 {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
