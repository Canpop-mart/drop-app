<template>
  <div class="h-full overflow-y-auto bg-zinc-950">
    <!-- Loading skeleton — keeps the page from flashing empty while we
         hydrate the Tauri-side fetch. -->
    <div
      v-if="!gameRef"
      class="flex flex-col items-center justify-center h-full text-zinc-500"
    >
      <ArrowPathIcon class="size-6 animate-spin mb-2" />
      <p class="text-sm">Loading...</p>
    </div>

    <template v-else>
      <!-- Hero banner — falls back to cover-on-gradient when no banner. -->
      <div class="relative w-full aspect-[21/8] overflow-hidden">
        <img
          v-if="gameRef.mBannerObjectId"
          :src="useObject(gameRef.mBannerObjectId)"
          :alt="gameRef.mName"
          class="absolute inset-0 w-full h-full object-cover"
        />
        <div
          v-else
          class="absolute inset-0 bg-gradient-to-br from-zinc-800 to-zinc-900"
        />
        <div
          class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/60 to-transparent"
        />
        <div
          class="absolute inset-0 bg-gradient-to-r from-zinc-950/80 via-transparent to-transparent"
        />

        <!-- Back button — top-left, matches the gear icon position on the
             library detail page so the muscle-memory transfer is clean. -->
        <button
          class="absolute top-4 left-4 z-10 rounded-lg p-2 bg-zinc-900/60 backdrop-blur-sm text-zinc-100 hover:bg-zinc-900/80 transition-colors"
          @click="router.back()"
        >
          <ArrowLeftIcon class="size-5" />
        </button>

        <!-- Title block over the banner. -->
        <div class="absolute bottom-0 inset-x-0 p-10 max-w-5xl">
          <h1
            class="text-5xl font-display font-bold text-zinc-100 drop-shadow-lg mb-2"
          >
            {{ gameRef.mName }}
          </h1>
          <p
            v-if="gameRef.mShortDescription"
            class="text-base text-zinc-200/90 max-w-3xl line-clamp-2 drop-shadow"
          >
            {{ gameRef.mShortDescription }}
          </p>
        </div>
      </div>

      <!-- Body — two-column layout: left for description + gallery, right
           for the action panel + metadata. Sticky right sidebar keeps the
           CTA visible while the user scrolls through the description. -->
      <div class="px-10 xl:px-14 py-8 grid lg:grid-cols-[1fr_320px] gap-8">
        <!-- Left column -->
        <div class="space-y-8 min-w-0">
          <!-- Gallery — uses the carousel image set if present. -->
          <section
            v-if="gameRef.mImageCarouselObjectIds.length > 0"
            class="rounded-xl bg-zinc-900/50 ring-1 ring-zinc-800/60 overflow-hidden"
          >
            <div class="relative aspect-video">
              <img
                :src="useObject(gameRef.mImageCarouselObjectIds[currentImage]!)"
                class="w-full h-full object-cover"
              />
              <button
                v-if="gameRef.mImageCarouselObjectIds.length > 1"
                class="absolute left-3 top-1/2 -translate-y-1/2 rounded-full p-2 bg-zinc-900/70 text-zinc-100 hover:bg-zinc-900 transition-colors"
                @click="prevImage"
              >
                <ChevronLeftIcon class="size-5" />
              </button>
              <button
                v-if="gameRef.mImageCarouselObjectIds.length > 1"
                class="absolute right-3 top-1/2 -translate-y-1/2 rounded-full p-2 bg-zinc-900/70 text-zinc-100 hover:bg-zinc-900 transition-colors"
                @click="nextImage"
              >
                <ChevronRightIcon class="size-5" />
              </button>
            </div>
            <div
              v-if="gameRef.mImageCarouselObjectIds.length > 1"
              class="px-3 py-2 flex gap-1.5 justify-center bg-zinc-900/50"
            >
              <button
                v-for="(_, i) in gameRef.mImageCarouselObjectIds"
                :key="i"
                class="size-1.5 rounded-full transition-colors"
                :class="
                  i === currentImage
                    ? 'bg-blue-500'
                    : 'bg-zinc-600 hover:bg-zinc-400'
                "
                @click="currentImage = i"
              />
            </div>
          </section>

          <!-- About — Markdown description rendered to HTML with image
               URLs rewritten, matching the library detail page's pipeline. -->
          <section
            v-if="gameRef.mDescription"
            class="rounded-xl bg-zinc-900/50 ring-1 ring-zinc-800/60 p-6"
          >
            <h2 class="text-lg font-display font-semibold text-zinc-100 mb-4">
              About
            </h2>
            <div
              class="prose prose-invert prose-blue max-w-none"
              v-html="htmlDescription"
            />
          </section>
        </div>

        <!-- Right column — action panel + metadata. Sticky on lg+ so it
             follows the user down the description scroll. -->
        <aside class="lg:sticky lg:top-6 lg:self-start space-y-4">
          <!-- Cover thumbnail (small) — gives the side a visual anchor. -->
          <div
            class="aspect-[3/4] rounded-xl overflow-hidden bg-zinc-900 ring-1 ring-zinc-800/60"
          >
            <img
              v-if="gameRef.mCoverObjectId"
              :src="useObject(gameRef.mCoverObjectId)"
              :alt="gameRef.mName"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-600 text-sm px-3 text-center"
            >
              {{ gameRef.mName }}
            </div>
          </div>

          <!-- Primary CTA — the store is a discovery surface, so the action
               here is "Add to Library", never "Install". Installing happens
               on /library/[id] once the game is in the user's library. -->
          <button
            v-if="inLibrary === false"
            class="w-full inline-flex items-center justify-center gap-2 rounded-md bg-blue-600 px-4 py-3 text-sm font-semibold text-white transition-colors hover:bg-blue-500 disabled:opacity-60"
            :disabled="libraryActionLoading"
            @click="addToLibrary"
          >
            <PlusIcon class="size-4" />
            {{ libraryActionLoading ? "Adding…" : "Add to Library" }}
          </button>
          <NuxtLink
            v-else-if="inLibrary === true"
            :to="`/library/${gameId}`"
            class="w-full inline-flex items-center justify-center gap-2 rounded-md bg-green-600 px-4 py-3 text-sm font-semibold text-white transition-colors hover:bg-green-500"
          >
            <CheckIcon class="size-4" />
            View in Library
          </NuxtLink>
          <div
            v-else
            class="w-full rounded-md bg-zinc-800 px-4 py-3 text-center text-sm text-zinc-500"
          >
            Checking library…
          </div>

          <p
            v-if="inLibrary === true"
            class="-mt-2 text-center text-xs text-zinc-500"
          >
            In your library — open it to install.
          </p>
          <p v-if="libraryError" class="-mt-2 text-center text-xs text-red-400">
            {{ libraryError }}
          </p>

          <!-- Metadata block — short description, tags, type, etc. The
               server's browse endpoint surfaces tags so we pull them on
               mount via a name-search. Other fields (developers/pubs)
               aren't returned by any current endpoint, so they're not
               shown here — adding a /store/[id] server endpoint would
               unlock them. -->
          <div
            class="rounded-xl bg-zinc-900/50 ring-1 ring-zinc-800/60 p-4 space-y-3 text-xs"
          >
            <div v-if="storeMeta?.tags?.length" class="space-y-1.5">
              <p class="text-zinc-500 uppercase tracking-widest text-[10px]">
                Tags
              </p>
              <div class="flex flex-wrap gap-1.5">
                <span
                  v-for="tag in storeMeta.tags"
                  :key="tag.id"
                  class="px-2 py-0.5 rounded-full bg-blue-500/15 text-blue-300 ring-1 ring-blue-500/30 text-[11px]"
                >
                  {{ tag.name }}
                </span>
              </div>
            </div>

            <div v-if="gameRef.type !== 'Game'" class="space-y-1.5">
              <p class="text-zinc-500 uppercase tracking-widest text-[10px]">
                Type
              </p>
              <p class="text-zinc-300">{{ gameRef.type }}</p>
            </div>

            <div v-if="storeMeta?.isEmulated" class="space-y-1.5">
              <p class="text-zinc-500 uppercase tracking-widest text-[10px]">
                Platform
              </p>
              <p class="text-zinc-300">
                Emulated
                <span v-if="storeMeta?.launchPlatform" class="text-zinc-500">
                  · {{ storeMeta.launchPlatform }}
                </span>
              </p>
            </div>
          </div>
        </aside>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * Store detail page — marketing-flavored view of a single game, distinct
 * from /library/[id] which is the install/launch management surface.
 *
 * Click flow:
 *   - Click a tile on /store  → /store/[id] (this page)
 *   - "Add to Library" here   → adds the game to the user's default
 *                                collection (their library)
 *   - "View in Library" here  → /library/[id], where install/launch lives
 *
 * The store is a discovery surface: you ADD a game to your library here,
 * then install it from the library page. Installing never happens on the
 * store page.
 */
import {
  ArrowLeftIcon,
  ArrowPathIcon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  PlusIcon,
} from "@heroicons/vue/24/outline";
import { micromark } from "micromark";
import { useGame } from "~/composables/game";
import { useServerApi, type StoreGame } from "~/composables/use-server-api";
import {
  rewriteDescriptionImages,
  serverUrl,
} from "~/composables/use-server-fetch";
import type { Game } from "~/types";

const route = useRoute();
const router = useRouter();
const api = useServerApi();

const gameId = computed(() => route.params.id?.toString() ?? "");

useHead({ title: "Store" });

const gameRef = ref<Game | null>(null);
const storeMeta = ref<StoreGame | null>(null);
const currentImage = ref(0);

// Library membership — `null` while still checking, then `true` / `false`.
const inLibrary = ref<boolean | null>(null);
const libraryActionLoading = ref(false);
const libraryError = ref<string | null>(null);

// Game descriptions are authored in Markdown and may embed server-relative
// image URLs. Render to HTML via micromark, then rewrite image `src`
// attributes to absolute server:// URLs — the same pipeline the library
// detail page uses. Empty string until the game loads.
const htmlDescription = computed(() =>
  gameRef.value?.mDescription
    ? rewriteDescriptionImages(micromark(gameRef.value.mDescription))
    : "",
);

/** Pull the Tauri-side game data. */
async function load() {
  if (!gameId.value) return;
  try {
    const data = await useGame(gameId.value);
    gameRef.value = data.game;
  } catch (e) {
    console.warn("[store/[id]] failed to load game:", e);
  }
}

/** Best-effort store metadata (tags, isEmulated). The server has no
 *  per-id store endpoint, so we name-search and pick the match. Failures
 *  are non-fatal — the page still works without tags. */
async function loadStoreMeta() {
  if (!gameRef.value) return;
  try {
    const res = await api.store.browse({ q: gameRef.value.mName, take: 5 });
    storeMeta.value = res.results.find((g) => g.id === gameId.value) ?? null;
  } catch (e) {
    console.warn("[store/[id]] failed to load store meta:", e);
  }
}

/**
 * Determine whether this game is already in the user's library — i.e. in
 * their default collection. Mirrors the BPM game page's membership check.
 */
async function checkInLibrary() {
  if (!gameId.value) return;
  try {
    const res = await fetch(serverUrl("api/v1/collection/default"));
    if (!res.ok) throw new Error(`status ${res.status}`);
    const collection = await res.json();
    const entries: Array<{ gameId: string }> = collection.entries ?? [];
    inLibrary.value = entries.some((e) => e.gameId === gameId.value);
  } catch (e) {
    console.warn("[store/[id]] failed to check library membership:", e);
    // Couldn't confirm — assume not added so the user can still try.
    inLibrary.value = false;
  }
}

/** Add this game to the user's library (their default collection). */
async function addToLibrary() {
  if (libraryActionLoading.value || inLibrary.value) return;
  libraryActionLoading.value = true;
  libraryError.value = null;
  try {
    const res = await fetch(serverUrl("api/v1/collection/default/entry"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: gameId.value }),
    });
    if (!res.ok) throw new Error(`Server returned ${res.status}`);
    inLibrary.value = true;
  } catch (e) {
    console.error("[store/[id]] add to library failed:", e);
    libraryError.value = "Couldn't add to library — please try again.";
  } finally {
    libraryActionLoading.value = false;
  }
}

function nextImage() {
  if (!gameRef.value) return;
  const len = gameRef.value.mImageCarouselObjectIds.length;
  if (len === 0) return;
  currentImage.value = (currentImage.value + 1) % len;
}

function prevImage() {
  if (!gameRef.value) return;
  const len = gameRef.value.mImageCarouselObjectIds.length;
  if (len === 0) return;
  currentImage.value = (currentImage.value - 1 + len) % len;
}

onMounted(async () => {
  await load();
  // Run the store metadata + library-membership fetches in parallel so
  // the page paints fast.
  loadStoreMeta();
  checkInLibrary();
});

// Watch gameId for navigation between different store/[id] pages without
// a full remount (e.g. clicking a related game tile in the future).
watch(gameId, async () => {
  gameRef.value = null;
  storeMeta.value = null;
  currentImage.value = 0;
  inLibrary.value = null;
  libraryError.value = null;
  await load();
  loadStoreMeta();
  checkInLibrary();
});
</script>
