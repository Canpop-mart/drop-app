<template>
  <div class="game-friends-tile">
    <!-- The clickable pill — styled to match the existing stat-bar tiles. -->
    <button
      type="button"
      class="group inline-flex items-center gap-3 rounded-lg bg-zinc-800/60 backdrop-blur-sm px-5 py-3 border border-zinc-700/50 hover:border-zinc-600 transition-colors"
      :class="{ 'border-blue-500/50': expanded }"
      :disabled="players.length === 0 && !loading"
      :aria-expanded="expanded"
      @click="toggle"
    >
      <UsersIcon class="size-4 text-zinc-400 shrink-0" />
      <span class="text-sm text-zinc-400">Friends</span>
      <span class="text-sm text-zinc-100 font-medium">
        <span v-if="loading">Loading…</span>
        <span v-else>{{ visibleCount }} of {{ players.length }}</span>
      </span>

      <!-- Stacked avatar circles — up to 3, overlapping. -->
      <div v-if="players.length > 0" class="flex -space-x-2 items-center">
        <template v-for="(p, i) in visibleAvatars" :key="p.userId">
          <img
            v-if="p.avatarObjectId"
            :src="avatarUrl(p.avatarObjectId)"
            :alt="p.displayName"
            class="size-6 rounded-full ring-2 ring-zinc-900 object-cover bg-zinc-700"
            :style="{ zIndex: 3 - i }"
            referrerpolicy="no-referrer"
          />
          <div
            v-else
            class="size-6 rounded-full ring-2 ring-zinc-900 bg-zinc-700 flex items-center justify-center"
            :style="{ zIndex: 3 - i }"
          >
            <span class="text-[10px] font-semibold text-zinc-300 uppercase">
              {{ initial(p.displayName) }}
            </span>
          </div>
        </template>
      </div>

      <ChevronDownIcon
        v-if="players.length > 0"
        class="size-4 text-zinc-400 transition-transform"
        :class="{ 'rotate-180': expanded }"
      />
    </button>

    <!-- Inline expanded panel — compact list of players. Slides down. -->
    <Transition
      enter-active-class="ease-out duration-200 origin-top"
      enter-from-class="opacity-0 -translate-y-2 scale-y-95"
      enter-to-class="opacity-100 translate-y-0 scale-y-100"
      leave-active-class="ease-in duration-150 origin-top"
      leave-from-class="opacity-100 translate-y-0 scale-y-100"
      leave-to-class="opacity-0 -translate-y-2 scale-y-95"
    >
      <div
        v-if="expanded && players.length > 0"
        class="mt-3 rounded-lg bg-zinc-800/60 backdrop-blur-sm border border-zinc-700/50 overflow-hidden"
      >
        <div
          v-for="p in players"
          :key="p.userId"
          class="flex items-center gap-3 px-4 py-2.5 hover:bg-zinc-700/30 transition-colors"
        >
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
            <p class="text-sm text-zinc-100 font-medium truncate">
              {{ p.displayName }}
            </p>
            <p class="text-xs text-zinc-500">
              {{ formatPlaytime(p.playtimeSeconds) }}
              <span v-if="p.achievementsTotal > 0">
                · {{ p.achievementsUnlocked }}/{{ p.achievementsTotal }}
                <TrophyIcon class="inline size-3 text-yellow-500 -mt-0.5" />
              </span>
            </p>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
/**
 * "Friends · X of Y" stat tile for the library game-detail page.
 *
 * Shows the count of server users with any playtime on this game, a row of
 * up to three stacked avatar circles, and an inline expand-on-click list
 * with name + playtime + achievement count per player.
 *
 * Data comes from `community.gamePlayers(gameId)` (Agent C's endpoint). If
 * the endpoint hasn't shipped yet the component renders an empty "0 of 0"
 * state and the button becomes inert — no error toast.
 */
import { ChevronDownIcon } from "@heroicons/vue/20/solid";
import { UsersIcon, TrophyIcon } from "@heroicons/vue/24/solid";
import { useServerApi, type GamePlayerEntry } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { formatPlaytime } from "~/composables/game-detail/use-game-stats";

const props = defineProps<{
  gameId: string;
  /** Optional preloaded list, so the page can fetch once and pass to both
   *  this tile and the Community tab. */
  players?: GamePlayerEntry[];
}>();

const api = useServerApi();

const players = ref<GamePlayerEntry[]>(props.players ?? []);
const loading = ref(props.players === undefined);
const expanded = ref(false);

watch(
  () => props.players,
  (v) => {
    if (v) {
      players.value = v;
      loading.value = false;
    }
  },
);

onMounted(async () => {
  if (props.players !== undefined) return;
  try {
    players.value = await api.community.gamePlayers(props.gameId);
  } catch {
    players.value = [];
  } finally {
    loading.value = false;
  }
});

const visibleAvatars = computed(() => players.value.slice(0, 3));

/**
 * The tile reads "Friends · X of 6". Per spec, the right-hand number is the
 * count of server users with any playtime; the left-hand number is the
 * count of those whose avatars we render in the stack. We cap the stack
 * at 3 but advertise the full count.
 */
const visibleCount = computed(() => Math.min(3, players.value.length));

function avatarUrl(objectId: string): string {
  return serverUrl(`api/v1/object/${objectId}`);
}

function initial(name: string): string {
  return (name || "?").trim().charAt(0) || "?";
}

function toggle() {
  if (players.value.length === 0) return;
  expanded.value = !expanded.value;
}
</script>
