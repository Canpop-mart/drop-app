<template>
  <div class="h-full overflow-y-auto bg-zinc-950">
    <!-- Loading skeleton -->
    <div
      v-if="!collection"
      class="flex h-full flex-col items-center justify-center text-zinc-500"
    >
      <ArrowPathIcon class="size-6 animate-spin mb-2" />
      <p class="text-sm">Loading...</p>
    </div>

    <template v-else>
      <!-- Hero — the collection cover as a soft banner with the title block
           and the bulk "add to library" CTA overlaid. -->
      <div class="relative w-full overflow-hidden">
        <div aria-hidden="true" class="absolute inset-0">
          <img
            v-if="collection.coverObjectId"
            :src="objectUrl(collection.coverObjectId)"
            alt=""
            class="h-full w-full object-cover"
          />
          <div class="absolute inset-0 bg-zinc-950/80" />
          <div
            class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent"
          />
        </div>

        <div class="relative max-w-5xl px-10 pb-8 pt-28 xl:px-14">
          <!-- Back button — matches the store detail page's position. -->
          <button
            class="absolute left-4 top-4 z-10 rounded-lg bg-zinc-900/60 p-2 text-zinc-100 backdrop-blur-sm transition-colors hover:bg-zinc-900/80"
            @click="router.back()"
          >
            <ArrowLeftIcon class="size-5" />
          </button>

          <p
            class="mb-2 text-xs font-semibold uppercase tracking-widest text-blue-400"
          >
            Collection
          </p>
          <h1
            class="mb-2 font-display text-4xl font-bold text-zinc-100 drop-shadow-lg"
          >
            {{ collection.name }}
          </h1>
          <p
            v-if="collection.description"
            class="mb-5 line-clamp-3 max-w-3xl text-base text-zinc-300"
          >
            {{ collection.description }}
          </p>

          <button
            v-if="collection.games.length > 0"
            :disabled="adding || added"
            class="inline-flex items-center gap-2 rounded-md bg-blue-600 px-5 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-60"
            @click="addToLibrary"
          >
            <CheckIcon v-if="added" class="size-4" />
            <PlusIcon v-else class="size-4" />
            <span v-if="added"
              >Added {{ collection.games.length }} game{{
                collection.games.length === 1 ? "" : "s"
              }}
              to your library</span
            >
            <span v-else-if="adding">Adding…</span>
            <span v-else>Add entire collection to my library</span>
          </button>
          <p v-if="addError" class="mt-2 text-xs text-red-400">
            {{ addError }}
          </p>
        </div>
      </div>

      <!-- Games grid — click-through to each game's store page so the user
           can browse + add individually ("go through it normally"). -->
      <div class="px-10 py-8 xl:px-14">
        <GameTileGrid
          v-if="collection.games.length > 0"
          :item-count="collection.games.length"
        >
          <GameTile
            v-for="game in collection.games"
            :key="game.id"
            :cover-url="
              game.mCoverObjectId ? objectUrl(game.mCoverObjectId) : null
            "
            :name="game.mName"
            @select="goToGame(game.id)"
          />
        </GameTileGrid>
        <div v-else class="py-20 text-center text-sm text-zinc-500">
          This collection doesn't have any games yet.
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * Store collection landing page — native desktop view of a curated store
 * collection. Mirrors the web `/store/collection/[id]` page: a hero with the
 * collection's cover + an "add the whole thing to my library" action, and a
 * box-art grid that links through to each game's store page so the user can
 * browse and add games individually.
 */
import {
  ArrowLeftIcon,
  ArrowPathIcon,
  CheckIcon,
  PlusIcon,
} from "@heroicons/vue/24/outline";
import {
  useServerApi,
  type StoreCollectionDetail,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { deduplicatedInvoke } from "~/composables/game";

const route = useRoute();
const router = useRouter();
const api = useServerApi();

const collectionId = computed(() => route.params.id?.toString() ?? "");
const collection = ref<StoreCollectionDetail | null>(null);
const adding = ref(false);
const added = ref(false);
const addError = ref<string | null>(null);

const pageTitle = computed(() => collection.value?.name ?? "Collection");
useHead({ title: pageTitle });

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

// Click-through to a game's store presentation page (same destination the
// store grid uses), prefetching the Tauri-side game so the page mounts fast.
function goToGame(gameId: string) {
  deduplicatedInvoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/store/${gameId}`);
}

/** Add every game in the collection to the user's library and save a personal
 *  copy as a shelf (server does both in one call). */
async function addToLibrary() {
  if (adding.value || added.value || !collectionId.value) return;
  adding.value = true;
  addError.value = null;
  try {
    await api.store.addCollectionToLibrary(collectionId.value);
    added.value = true;
  } catch (e) {
    console.error("[store/collection] add to library failed:", e);
    addError.value = "Couldn't add the collection. Please try again.";
  } finally {
    adding.value = false;
  }
}

async function load() {
  if (!collectionId.value) return;
  try {
    collection.value = await api.store.collection(collectionId.value);
  } catch (e) {
    console.warn("[store/collection] failed to load collection:", e);
  }
}

onMounted(load);
</script>
