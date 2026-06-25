<template>
  <div
    class="flex items-center gap-x-3 rounded-xl bg-zinc-800/50 backdrop-blur-sm p-3 ring-1 ring-zinc-700/40 hover:ring-blue-500/40 transition-colors"
  >
    <button
      class="shrink-0 rounded-full transition-transform hover:scale-105"
      @click="$emit('go-to-user', cluster.user.id)"
    >
      <img
        v-if="cluster.user.profilePictureObjectId"
        :src="objectUrl(cluster.user.profilePictureObjectId)"
        class="size-10 rounded-full object-cover"
      />
      <div
        v-else
        class="size-10 rounded-full bg-zinc-700 flex items-center justify-center"
      >
        <UserIcon class="size-5 text-zinc-500" />
      </div>
    </button>

    <div class="flex-1 min-w-0">
      <!--
        The verb segment used to be inline `<template v-if>` blocks.  Vue's
        compiler strips whitespace AROUND template branches even in 'condense'
        mode, which rendered as "Girlchriszwas inRune Dice" — words mashed
        together.  Computing the verb as a single string and rendering it
        inside a `<span>` with hard-coded leading/trailing spaces keeps the
        whitespace inside a text node, where it always survives.  The verb
        copy here also moved from "was in" to "was playing" — reads more
        naturally for sessions that have no recorded duration yet.
      -->
      <p class="text-sm text-zinc-300 leading-snug">
        <button
          class="font-medium text-zinc-100 hover:text-blue-400 transition-colors"
          @click="$emit('go-to-user', cluster.user.id)"
        >{{ userLabel }}</button>
        <span>&nbsp;{{ verb }}&nbsp;</span>
        <button
          v-if="cluster.kind !== 'request' && cluster.game"
          class="font-medium text-blue-400 hover:text-blue-300 transition-colors"
          @click="$emit('go-to-game', cluster.game.id)"
        >{{ cluster.game.mName }}</button>
        <span
          v-else-if="cluster.kind === 'request' && cluster.request"
          class="font-medium text-blue-400"
        >{{ cluster.request.title }}</span>
        <span v-if="suffix">&nbsp;{{ suffix }}</span>
      </p>

      <!-- Achievement chips inline below the headline -->
      <p
        v-if="cluster.achievements.length > 0"
        class="text-xs text-zinc-400 mt-1 leading-snug"
      >
        <span class="text-yellow-500 mr-1">unlocked</span>
        <template
          v-for="(ach, i) in displayedAchievements"
          :key="ach.id"
        >
          <span class="text-yellow-300">{{ ach.title }}</span>
          <span
            v-if="i < displayedAchievements.length - 1"
            class="text-zinc-500"
            >,
          </span>
        </template>
        <span
          v-if="cluster.achievements.length > displayedAchievements.length"
          class="text-zinc-500"
          >, +{{
            cluster.achievements.length - displayedAchievements.length
          }}
          more</span
        >
      </p>

      <p class="text-xs text-zinc-500 mt-0.5">
        {{ formatLastPlayed(cluster.timestamp) }}
      </p>
    </div>

    <!-- Bigger cover art (anchors the row) -->
    <button
      v-if="cluster.game?.mCoverObjectId && cluster.kind !== 'request'"
      class="shrink-0 rounded-md overflow-hidden ring-1 ring-zinc-700/60 hover:ring-blue-500/50 transition-all"
      @click="$emit('go-to-game', cluster.game.id)"
    >
      <img
        :src="objectUrl(cluster.game.mCoverObjectId)"
        class="h-20 w-[3.75rem] object-cover hidden sm:block"
        loading="lazy"
      />
    </button>
  </div>
</template>

<script setup lang="ts">
import { UserIcon } from "@heroicons/vue/24/solid";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  formatPlaytime,
  formatLastPlayed,
} from "~/composables/use-recent-games";
import type { ActivityCluster } from "~/composables/use-community-clusters";

const props = defineProps<{
  cluster: ActivityCluster;
}>();

defineEmits<{
  (e: "go-to-game", gameId: string): void;
  (e: "go-to-user", userId: string): void;
}>();

const hasPlaytime = computed(
  () => (props.cluster.totalDuration ?? 0) > 0,
);

const userLabel = computed(
  () => props.cluster.user.displayName || props.cluster.user.username,
);

// Verb chosen per cluster kind. "was playing" replaces the earlier "was in"
// which read oddly — "was in Rune Dice" sounds like a location, not an
// activity.
const verb = computed(() => {
  if (props.cluster.kind === "request") return "requested";
  return hasPlaytime.value ? "played" : "was playing";
});

const suffix = computed(() => {
  if (props.cluster.kind === "request") return null;
  if (!hasPlaytime.value) return null;
  return `for ${formatPlaytime(props.cluster.totalDuration ?? 0)}`;
});

const displayedAchievements = computed(() =>
  props.cluster.achievements.slice(0, 3),
);

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}
</script>
