<template>
  <div class="min-h-full bg-zinc-950 px-8 pb-16 pt-5 xl:px-12">
    <div v-if="loading" class="flex min-h-[70vh] flex-col items-center justify-center text-zinc-500">
      <ArrowPathIcon class="mb-2 size-6 animate-spin" />
      <p class="text-sm">Loading...</p>
    </div>
    <SectionError v-else-if="loadError" title="Couldn't load this game" :detail="loadError" @retry="loadPage" />

    <template v-else-if="gameRef">
      <section class="relative mx-auto h-[500px] max-w-[1900px] overflow-hidden rounded-2xl ring-1 ring-zinc-800/80">
        <img v-if="gameRef.mBannerObjectId" :src="objectUrl(gameRef.mBannerObjectId)" :alt="gameRef.mName" class="absolute inset-0 h-full w-full object-cover opacity-80" />
        <BannerFallback v-else :name="gameRef.mName" text-size="text-9xl" />
        <div class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/35 to-zinc-950/10" />
        <div class="absolute inset-0 bg-gradient-to-r from-zinc-950 via-zinc-950/75 to-transparent" />

        <button class="absolute left-5 top-5 z-20 rounded-xl bg-zinc-950/60 p-2.5 text-zinc-100 backdrop-blur hover:bg-zinc-900" aria-label="Back" @click="router.back()">
          <ArrowLeftIcon class="size-5" />
        </button>

        <div class="absolute inset-x-0 bottom-0 z-10 grid items-end gap-8 p-8 lg:grid-cols-[minmax(0,1fr)_220px] lg:p-10">
          <div class="max-w-4xl">
            <h1 class="font-display text-4xl font-bold text-white drop-shadow-lg lg:text-5xl">{{ gameRef.mName }}</h1>
            <p v-if="gameRef.mShortDescription" class="mt-3 max-w-3xl text-base leading-7 text-zinc-200">{{ gameRef.mShortDescription }}</p>
            <div v-if="storeMeta?.tags?.length" class="mt-4 flex flex-wrap gap-2">
              <span v-for="tag in storeMeta.tags.slice(0, 6)" :key="tag.id" class="rounded-full bg-white/10 px-3 py-1 text-xs font-medium text-zinc-100 backdrop-blur ring-1 ring-white/10">{{ tag.name }}</span>
            </div>

            <div class="mt-6 flex flex-wrap items-center gap-3">
              <button v-if="inLibrary === false" class="inline-flex items-center gap-2 rounded-xl bg-blue-500 px-5 py-3 text-sm font-semibold text-white shadow-lg hover:bg-blue-400 disabled:opacity-60" :disabled="libraryActionLoading" @click="addToLibrary">
                <PlusIcon class="size-4" />
                {{ libraryActionLoading ? "Adding..." : "Add to Library" }}
              </button>
              <NuxtLink v-else-if="inLibrary === true" :to="`/library/${gameId}`" class="inline-flex items-center gap-2 rounded-xl bg-emerald-500 px-5 py-3 text-sm font-semibold text-white shadow-lg hover:bg-emerald-400">
                <CheckIcon class="size-4" /> View in Library
              </NuxtLink>
              <div v-else class="rounded-xl bg-zinc-800/80 px-5 py-3 text-sm text-zinc-400">Checking library...</div>
              <button v-if="membershipError" class="text-sm font-medium text-red-300 hover:text-red-200" @click="checkInLibrary">Retry library check</button>
            </div>
            <p v-if="libraryError" class="mt-3 text-sm text-red-300">{{ libraryError }}</p>
          </div>

          <div class="hidden aspect-[3/4] overflow-hidden rounded-xl bg-zinc-900 shadow-2xl ring-1 ring-white/15 lg:block">
            <img v-if="gameRef.mCoverObjectId" :src="objectUrl(gameRef.mCoverObjectId)" :alt="gameRef.mName" class="h-full w-full object-cover" />
            <div v-else class="flex h-full items-center justify-center p-4 text-center text-sm text-zinc-500">{{ gameRef.mName }}</div>
          </div>
        </div>
      </section>

      <main class="mx-auto mt-8 max-w-[1900px] space-y-10">
        <section v-if="gameRef.mImageCarouselObjectIds.length > 0" class="overflow-hidden rounded-2xl bg-zinc-900 ring-1 ring-zinc-800/70">
          <div class="relative aspect-[21/8] min-h-[420px]">
            <img :src="objectUrl(gameRef.mImageCarouselObjectIds[currentImage]!)" :alt="`${gameRef.mName} screenshot ${currentImage + 1}`" class="h-full w-full object-cover" />
            <button v-if="gameRef.mImageCarouselObjectIds.length > 1" class="absolute left-5 top-1/2 -translate-y-1/2 rounded-full bg-zinc-950/65 p-3 text-white backdrop-blur hover:bg-zinc-950" aria-label="Previous image" @click="prevImage"><ChevronLeftIcon class="size-5" /></button>
            <button v-if="gameRef.mImageCarouselObjectIds.length > 1" class="absolute right-5 top-1/2 -translate-y-1/2 rounded-full bg-zinc-950/65 p-3 text-white backdrop-blur hover:bg-zinc-950" aria-label="Next image" @click="nextImage"><ChevronRightIcon class="size-5" /></button>
            <div v-if="gameRef.mImageCarouselObjectIds.length > 1" class="absolute inset-x-0 bottom-5 flex justify-center gap-2">
              <button v-for="(_, i) in gameRef.mImageCarouselObjectIds" :key="i" class="h-1.5 rounded-full transition-all" :class="i === currentImage ? 'w-7 bg-blue-400' : 'w-1.5 bg-white/40 hover:bg-white/70'" :aria-label="`Show image ${i + 1}`" @click="currentImage = i" />
            </div>
          </div>
        </section>

        <div class="grid items-start gap-10 xl:grid-cols-[minmax(0,1fr)_360px]">
          <div class="min-w-0 space-y-10">
            <section v-if="gameRef.mDescription">
              <h2 class="mb-5 font-display text-2xl font-semibold text-zinc-100">About</h2>
              <div class="prose prose-invert prose-blue max-w-none text-zinc-300" v-html="htmlDescription" />
            </section>

            <section v-if="availableMods.length > 0">
              <h2 class="mb-4 font-display text-2xl font-semibold text-zinc-100">Available Mods</h2>
              <div class="grid gap-3 sm:grid-cols-2">
                <div v-for="mod in availableMods" :key="mod.id" class="flex items-center gap-3 rounded-xl bg-zinc-900 p-4 ring-1 ring-zinc-800/70">
                  <img v-if="mod.mIconObjectId" :src="objectUrl(mod.mIconObjectId)" class="size-12 shrink-0 rounded-lg object-cover" />
                  <div class="min-w-0"><p class="truncate text-sm font-medium text-zinc-100">{{ mod.mName }}</p><p v-if="mod.mShortDescription" class="mt-1 truncate text-xs text-zinc-400">{{ mod.mShortDescription }}</p></div>
                </div>
              </div>
              <p class="mt-3 text-xs text-zinc-500">{{ inLibrary ? "Install the game from your library to add mods." : "Add the game to your library to use mods." }}</p>
            </section>
          </div>

          <aside class="space-y-4 xl:sticky xl:top-6">
            <div class="rounded-2xl bg-zinc-900/70 p-5 ring-1 ring-zinc-800/70">
              <div v-if="gameRef.type !== 'Game'" class="mb-4"><p class="text-xs text-zinc-500">Type</p><p class="mt-1 text-sm text-zinc-200">{{ gameRef.type }}</p></div>
              <div v-if="storeMeta?.isEmulated"><p class="text-xs text-zinc-500">Platform</p><p class="mt-1 text-sm text-zinc-200">Emulated<span v-if="storeMeta.launchPlatform" class="text-zinc-500"> · {{ storeMeta.launchPlatform }}</span></p></div>
              <p v-if="storeMetaError" class="text-sm text-red-300">Game details could not be loaded. <button class="font-semibold underline" @click="loadStoreMeta">Retry</button></p>
            </div>
            <GameDetailHowLongToBeat :game="gameRef" />
            <GameDetailControllerSupport :game="gameRef" :emulated="storeMeta?.isEmulated" />
          </aside>
        </div>

        <section v-if="achievementsLoading || achievements.length > 0 || achievementsError" class="rounded-2xl bg-zinc-900/50 p-6 ring-1 ring-zinc-800/70">
          <div class="mb-4 flex items-center justify-between gap-4"><h2 class="font-display text-xl font-semibold text-zinc-100">Achievements</h2><span v-if="achievements.length" class="text-sm tabular-nums text-zinc-400">{{ unlockedCount }} / {{ achievements.length }}</span></div>
          <SectionError v-if="achievementsError" title="Couldn't load achievements" :detail="achievementsError" @retry="loadAchievements" />
          <div v-else-if="achievementsLoading" class="grid grid-cols-8 gap-3"><div v-for="i in 8" :key="i" class="aspect-square animate-pulse rounded-xl bg-zinc-800" /></div>
          <div v-else class="grid grid-cols-6 gap-3 sm:grid-cols-8 lg:grid-cols-12">
            <div v-for="ach in visibleAchievements" :key="ach.id" class="relative aspect-square overflow-hidden rounded-xl bg-zinc-800 ring-1 ring-zinc-700/50" :title="achievementTooltip(ach)">
              <img v-if="ach.unlocked && ach.iconUrl" :src="ach.iconUrl" :alt="ach.title" class="h-full w-full object-cover" />
              <img v-else-if="!ach.unlocked && ach.iconLockedUrl" :src="ach.iconLockedUrl" :alt="ach.title" class="h-full w-full object-cover" />
              <img v-else-if="ach.iconUrl" :src="ach.iconUrl" :alt="ach.title" class="h-full w-full object-cover grayscale opacity-40" />
              <div v-else class="flex h-full items-center justify-center"><TrophyIcon class="size-5 text-zinc-600" /></div>
              <div v-if="ach.unlocked" class="absolute right-1 top-1 flex size-4 items-center justify-center rounded-full bg-yellow-400"><CheckIcon class="size-2.5 text-zinc-950" /></div>
            </div>
          </div>
          <button v-if="!achievementsLoading && achievements.length > ACHIEVEMENTS_PREVIEW" class="mt-4 flex items-center gap-2 text-sm text-zinc-400 hover:text-zinc-200" @click="achievementsExpanded = !achievementsExpanded">{{ achievementsExpanded ? "Show fewer" : `Show all ${achievements.length}` }}<ChevronDownIcon class="size-4 transition-transform" :class="{ 'rotate-180': achievementsExpanded }" /></button>
        </section>
      </main>
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
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  PlusIcon,
} from "@heroicons/vue/24/outline";
import { TrophyIcon } from "@heroicons/vue/24/solid";
import { renderMarkdown } from "~/composables/render-markdown";
import { objectImageUrl } from "~/composables/use-object";
import BannerFallback from "~/components/BannerFallback.vue";
import {
  useServerApi,
  type StoreAchievement,
  type StoreGame,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { invoke } from "@tauri-apps/api/core";
import type { Game } from "~/types";

const route = useRoute();
const router = useRouter();
const api = useServerApi();

const gameId = computed(() => route.params.id?.toString() ?? "");

useHead({ title: "Store" });

const gameRef = ref<Game | null>(null);
const storeMeta = ref<StoreGame | null>(null);
const currentImage = ref(0);
const loading = ref(true);
const loadError = ref<string | null>(null);
const storeMetaError = ref<string | null>(null);

// Available mods for this game — discovery only. Installing a mod needs the base
// game installed, which happens on the library page, so the store just surfaces
// that mods exist and points the user there.
type AvailableMod = {
  id: string;
  mName: string;
  mShortDescription: string;
  mIconObjectId: string;
};
const availableMods = ref<AvailableMod[]>([]);

// Library membership — `null` while still checking, then `true` / `false`.
const inLibrary = ref<boolean | null>(null);
const libraryActionLoading = ref(false);
const libraryError = ref<string | null>(null);
const membershipError = ref<string | null>(null);

// Achievements — preview-only fetch via the games.achievements binding.
// Each row already carries the caller's unlock state so we don't have
// to round-trip a second time for membership checks. Preview cap is
// tuned for the sidebar grid (4 cols × 4 rows of small tiles); the
// "Show all" link expands to the full list inline.
const ACHIEVEMENTS_PREVIEW = 16;
const achievements = ref<StoreAchievement[]>([]);
const achievementsLoading = ref(false);
const achievementsExpanded = ref(false);
const achievementsError = ref<string | null>(null);

const unlockedCount = computed(
  () => achievements.value.filter((a) => a.unlocked).length,
);
const visibleAchievements = computed(() =>
  achievementsExpanded.value
    ? achievements.value
    : achievements.value.slice(0, ACHIEVEMENTS_PREVIEW),
);

/** Tooltip string for an achievement tile.  Composes title +
 *  description + unlocked timestamp / rarity so a quick hover gives
 *  the user the whole picture without an inline panel. */
function achievementTooltip(ach: StoreAchievement): string {
  const parts: string[] = [ach.title];
  if (ach.description) parts.push(ach.description);
  if (ach.unlocked && ach.unlockedAt) {
    const d = new Date(ach.unlockedAt);
    if (!Number.isNaN(d.getTime())) {
      parts.push(`Unlocked ${d.toLocaleDateString()}`);
    }
  } else if (ach.rarity > 0) {
    parts.push(`${ach.rarity.toFixed(1)}% of players have this`);
  }
  return parts.join(" · ");
}

// Game descriptions are authored in Markdown — rendered through the shared,
// sanitizing renderMarkdown (micromark → DOMPurify → image rewrite), the same
// pipeline the library and big-picture pages use. Empty string until loaded.
const htmlDescription = computed(() =>
  gameRef.value?.mDescription
    ? renderMarkdown(gameRef.value.mDescription)
    : "",
);

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

/** Pull the Tauri-side game data. */
async function load() {
  if (!gameId.value) return;
  loadError.value = null;
  try {
    gameRef.value = await api.store.game(gameId.value);
  } catch (e) {
    console.warn("[store/[id]] failed to load game:", e);
    loadError.value = e instanceof Error ? e.message : String(e);
  }
}

/** Best-effort store metadata (tags, isEmulated). The server has no
 *  per-id store endpoint, so we name-search and pick the match. Failures
 *  are non-fatal — the page still works without tags. */
async function loadStoreMeta() {
  if (!gameRef.value) return;
  storeMetaError.value = null;
  try {
    const res = await api.store.browse({ q: gameRef.value.mName, take: 5 });
    storeMeta.value = res.results.find((g) => g.id === gameId.value) ?? null;
  } catch (e) {
    console.warn("[store/[id]] failed to load store meta:", e);
    storeMetaError.value = e instanceof Error ? e.message : String(e);
  }
}

/** Mods available for this game (discovery only — install happens in the
 *  library). Soft-fails to an empty list so the section hides cleanly. */
async function loadMods() {
  if (!gameId.value) return;
  try {
    // Via a Tauri command so the JWT client auth header is attached — a raw
    // server:// fetch to /client/* arrives unauthenticated (403).
    availableMods.value = await invoke<AvailableMod[]>("fetch_game_mods", {
      gameId: gameId.value,
    });
  } catch (e) {
    console.warn("[store/[id]] failed to load mods:", e);
    availableMods.value = [];
  }
}

/** Per-game achievement list with the caller's unlock state baked in.
 *  Soft-fails to an empty list so the section hides cleanly when the
 *  endpoint isn't available (no provider linked, network down, etc.). */
async function loadAchievements() {
  if (!gameId.value) return;
  achievementsLoading.value = true;
  achievementsError.value = null;
  try {
    achievements.value = await api.games.achievements(gameId.value);
  } catch (e) {
    console.warn("[store/[id]] failed to load achievements:", e);
    achievements.value = [];
    achievementsError.value = e instanceof Error ? e.message : String(e);
  } finally {
    achievementsLoading.value = false;
  }
}

/**
 * Determine whether this game is already in the user's library — i.e. in
 * their default collection. Mirrors the BPM game page's membership check.
 */
async function checkInLibrary() {
  if (!gameId.value) return;
  inLibrary.value = null;
  membershipError.value = null;
  try {
    const res = await fetch(serverUrl("api/v1/collection/default"));
    if (!res.ok) throw new Error(`status ${res.status}`);
    const collection = await res.json();
    const entries: Array<{ gameId: string }> = collection.entries ?? [];
    inLibrary.value = entries.some((e) => e.gameId === gameId.value);
  } catch (e) {
    console.warn("[store/[id]] failed to check library membership:", e);
    membershipError.value = e instanceof Error ? e.message : String(e);
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
    libraryError.value = "Couldn't add to library. Please try again.";
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

async function loadPage() {
  loading.value = true;
  gameRef.value = null;
  loadError.value = null;
  await load();
  loading.value = false;
  if (!gameRef.value) return;
  void Promise.all([
    loadStoreMeta(),
    checkInLibrary(),
    loadAchievements(),
    loadMods(),
  ]);
}

onMounted(loadPage);

// Watch gameId for navigation between different store/[id] pages without
// a full remount (e.g. clicking a related game tile in the future).
watch(gameId, async () => {
  storeMeta.value = null;
  storeMetaError.value = null;
  currentImage.value = 0;
  inLibrary.value = null;
  membershipError.value = null;
  libraryError.value = null;
  achievements.value = [];
  achievementsError.value = null;
  achievementsExpanded.value = false;
  availableMods.value = [];
  await loadPage();
});
</script>
