<!--
  Favourite games shelf — a themed cover-art rail (wraps RailScroller so it
  scrolls with arrows when there are more than fit). Used on both the own and
  other-user profile; pass `editHref` (own profile only) for a "Manage" link.
  Accent tinting comes from the inherited profile CSS vars.
-->
<template>
  <RailScroller v-if="favorites.length > 0">
    <template #title>
      <StarIcon class="size-4" :style="{ color: 'var(--accent)' }" />
      Favourites
    </template>
    <template v-if="editHref" #count>
      <NuxtLink
        :to="editHref"
        class="font-medium hover:underline"
        :style="{ color: 'var(--accent)' }"
      >
        Manage
      </NuxtLink>
    </template>
    <button
      v-for="fav in favorites"
      :key="fav.id"
      class="favtile group w-28 shrink-0 text-left"
      @click="fav.game && $emit('select-game', fav.game.id)"
    >
      <div
        class="cov relative aspect-[3/4] overflow-hidden rounded-xl bg-zinc-800 ring-1 ring-zinc-700/50 transition-all"
      >
        <img
          v-if="fav.game?.mCoverObjectId"
          :src="objectUrl(fav.game.mCoverObjectId)"
          :alt="fav.game?.mName"
          class="h-full w-full object-cover"
          loading="lazy"
        />
        <div
          v-else
          class="flex h-full w-full items-center justify-center px-2 text-center text-xs text-zinc-600"
        >
          {{ fav.game?.mName }}
        </div>
      </div>
      <p
        class="mt-1.5 truncate text-xs text-zinc-400 transition-colors group-hover:text-zinc-200"
      >
        {{ fav.game?.mName }}
      </p>
    </button>
  </RailScroller>
</template>

<script setup lang="ts">
import { StarIcon } from "@heroicons/vue/24/solid";
import type { FavoriteEntry } from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";

defineProps<{
  favorites: FavoriteEntry[];
  editHref?: string;
}>();

defineEmits<{ (e: "select-game", gameId: string): void }>();

function objectUrl(id: string): string {
  return objectImageUrl(id);
}
</script>

<style scoped>
/* Hover lift + re-tint the ring to the profile accent (overrides the Tailwind
   ring colour var on hover). */
.favtile:hover .cov {
  transform: translateY(-3px);
  --tw-ring-color: var(--accent-border);
}
</style>
