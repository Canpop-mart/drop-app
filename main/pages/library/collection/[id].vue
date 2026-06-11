<template>
  <div class="h-full flex flex-col overflow-y-auto">
    <!-- Header: back to library + collection name. -->
    <div
      class="sticky top-0 z-10 bg-zinc-950/80 backdrop-blur-lg px-8 xl:px-12 pt-6 pb-4 border-b border-zinc-800/40"
    >
      <button
        class="inline-flex items-center gap-1.5 text-sm text-zinc-400 hover:text-zinc-100 transition-colors mb-3"
        @click="router.back()"
      >
        <ArrowLeftIcon class="size-4" />
        Library
      </button>
      <h1 class="text-2xl font-display font-bold text-zinc-100">
        {{ collectionName }}
      </h1>
      <p class="mt-1 text-sm text-zinc-500">
        {{ games.length }} game{{ games.length === 1 ? "" : "s" }}
      </p>
    </div>

    <div class="flex-1 px-8 xl:px-12 py-8 pb-16">
      <div
        v-if="loading"
        class="flex items-center justify-center py-20 text-sm text-zinc-500"
      >
        Loading…
      </div>
      <div
        v-else-if="notFound"
        class="flex flex-col items-center justify-center py-20 text-center"
      >
        <p class="text-sm text-zinc-400">Collection not found.</p>
        <NuxtLink
          to="/library"
          class="mt-2 text-sm text-blue-400 hover:text-blue-300"
        >
          Back to library
        </NuxtLink>
      </div>
      <p
        v-else-if="games.length === 0"
        class="py-20 text-center text-sm text-zinc-500"
      >
        This collection is empty.
      </p>
      <GameTileGrid v-else :item-count="games.length">
        <GameTile
          v-for="game in games"
          :key="game.id"
          :cover-url="
            game.mCoverObjectId ? useObject(game.mCoverObjectId) : null
          "
          :name="game.mName"
          :installed="false"
          :update-available="false"
          :last-played="null"
          :hover-action="null"
          @select="goToGame(game.id)"
        />
      </GameTileGrid>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ArrowLeftIcon } from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import { useShelves } from "~/composables/shelves";

const route = useRoute();
const router = useRouter();
const { shelves, fetchShelves } = useShelves();

const collectionId = computed(() => route.params.id as string);
const collectionName = ref("Collection");
const loading = ref(true);
const notFound = ref(false);

type ShelfGame = {
  id: string;
  mName: string;
  mCoverObjectId: string | null;
};
const games = ref<ShelfGame[]>([]);

async function load() {
  loading.value = true;
  notFound.value = false;
  // Shelves are a shared cache; only fetch if it's cold (e.g. deep-link).
  if (shelves.value.length === 0) {
    await fetchShelves();
  }
  const shelf = shelves.value.find((s) => s.id === collectionId.value);
  if (!shelf) {
    notFound.value = true;
    loading.value = false;
    return;
  }
  collectionName.value = shelf.name;
  games.value = shelf.entries.map((e) => e.game);
  loading.value = false;
}

function goToGame(gameId: string) {
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/library/${gameId}`);
}

onMounted(load);
watch(collectionId, load);
</script>
