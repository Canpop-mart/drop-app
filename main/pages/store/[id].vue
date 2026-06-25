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
      <!-- Hero banner — slimmer (21:9, matches the store-index hero
           ratio) so the body content sits above the fold on standard
           laptop displays. Falls back to a flat gradient when no
           banner is provided. -->
      <div class="relative w-full aspect-[21/9] overflow-hidden">
        <img
          v-if="gameRef.mBannerObjectId"
          :src="useObject(gameRef.mBannerObjectId)"
          :alt="gameRef.mName"
          class="absolute inset-0 w-full h-full object-cover"
        />
        <div v-else class="absolute inset-0">
          <BannerFallback :name="gameRef.mName" text-size="text-9xl" />
        </div>
        <!-- Softer gradient masks. The bottom drop is the heaviest
             (so the title is always readable), the side drop is
             lighter so banner art shows through on the right. -->
        <div
          class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/40 to-transparent"
        />
        <div
          class="absolute inset-0 bg-gradient-to-r from-zinc-950/60 via-transparent to-transparent"
        />

        <!-- Back button — top-left, matches the gear icon position on
             the library detail page so the muscle-memory transfer
             is clean. -->
        <button
          class="absolute top-4 left-4 z-10 rounded-lg p-2 bg-zinc-900/60 backdrop-blur-sm text-zinc-100 hover:bg-zinc-900/80 transition-colors"
          @click="router.back()"
        >
          <ArrowLeftIcon class="size-5" />
        </button>

        <!-- Title block. Dropped from 5xl → 4xl so it has more
             breathing room beside the short description. Padded
             right so it doesn't fight any future top-right hero
             chrome. -->
        <div class="absolute bottom-0 inset-x-0 p-8 sm:p-10 max-w-5xl">
          <h1
            class="text-4xl font-display font-bold text-zinc-100 drop-shadow-lg mb-2"
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

          <!-- Achievements — sidebar variant. The previous full-width
               version dominated the page; here a denser 4-col grid sits
               beside the cover/CTA stack, leaving the main column for
               description + gallery. -->
          <div
            v-if="achievementsLoading || achievements.length > 0"
            class="rounded-xl bg-zinc-900/50 ring-1 ring-zinc-800/60 p-4 space-y-3"
          >
            <div class="flex items-baseline justify-between">
              <p class="text-zinc-500 uppercase tracking-widest text-[10px]">
                Achievements
              </p>
              <span
                v-if="achievements.length > 0"
                class="text-[11px] text-zinc-400 tabular-nums"
              >
                {{ unlockedCount }} / {{ achievements.length }}
              </span>
            </div>

            <!-- Progress bar — slimmer than the main-column variant so
                 it sits cleanly under the count line. -->
            <div
              v-if="achievements.length > 0"
              class="h-1 bg-zinc-800 rounded-full overflow-hidden"
            >
              <div
                class="h-full bg-yellow-500/80 rounded-full transition-all duration-500"
                :style="{ width: `${unlockedPercent}%` }"
              />
            </div>

            <!-- Loading skeleton — 4×4 grid of pulse tiles so the
                 layout doesn't jump when the data arrives. -->
            <div
              v-if="achievementsLoading"
              class="grid grid-cols-4 gap-1.5"
            >
              <div
                v-for="i in 12"
                :key="i"
                class="aspect-square rounded bg-zinc-800/60 animate-pulse"
              />
            </div>

            <!-- Icon grid. 4 cols at ~60px each fits the 320px sidebar
                 with the padding budget. Locked icons fall back to a
                 grayscale unlocked icon when no locked variant ships. -->
            <div
              v-else
              class="grid grid-cols-4 gap-1.5"
            >
              <div
                v-for="ach in visibleAchievements"
                :key="ach.id"
                class="group relative aspect-square rounded overflow-hidden bg-zinc-800/60 ring-1 ring-zinc-700/40"
                :title="achievementTooltip(ach)"
              >
                <img
                  v-if="ach.unlocked && ach.iconUrl"
                  :src="ach.iconUrl"
                  :alt="ach.title"
                  class="w-full h-full object-cover"
                />
                <img
                  v-else-if="!ach.unlocked && ach.iconLockedUrl"
                  :src="ach.iconLockedUrl"
                  :alt="ach.title"
                  class="w-full h-full object-cover"
                />
                <img
                  v-else-if="ach.iconUrl"
                  :src="ach.iconUrl"
                  :alt="ach.title"
                  class="w-full h-full object-cover grayscale opacity-40"
                />
                <div
                  v-else
                  class="w-full h-full flex items-center justify-center"
                >
                  <TrophyIcon class="size-3.5 text-zinc-600" />
                </div>

                <!-- Unlocked dot — smaller in the sidebar variant to
                     match the smaller tile. -->
                <div
                  v-if="ach.unlocked"
                  class="absolute top-0.5 right-0.5 size-3 rounded-full bg-yellow-500 flex items-center justify-center shadow"
                >
                  <CheckIcon class="size-2 text-zinc-950" />
                </div>
              </div>
            </div>

            <!-- Disclosure toggle — chevron flips on expand. Keeps the
                 same affordance as the rest of the page's collapsible
                 sections instead of mixing a one-way 'Show all' link
                 with the closeable-elsewhere pattern. -->
            <button
              v-if="
                !achievementsLoading &&
                achievements.length > ACHIEVEMENTS_PREVIEW
              "
              class="w-full flex items-center justify-between text-xs text-zinc-400 hover:text-zinc-200 transition-colors py-1"
              @click="achievementsExpanded = !achievementsExpanded"
            >
              <span>{{
                achievementsExpanded
                  ? "Show fewer"
                  : `Show all ${achievements.length}`
              }}</span>
              <ChevronDownIcon
                class="size-3.5 transition-transform"
                :class="{ 'rotate-180': achievementsExpanded }"
              />
            </button>
          </div>

          <GameDetailHowLongToBeat :game="gameRef" />

          <GameDetailControllerSupport
            :game="gameRef"
            :emulated="storeMeta?.isEmulated"
          />
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
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  PlusIcon,
} from "@heroicons/vue/24/outline";
import { TrophyIcon } from "@heroicons/vue/24/solid";
import { renderMarkdown } from "~/composables/render-markdown";
import BannerFallback from "~/components/BannerFallback.vue";
import { useGame } from "~/composables/game";
import {
  useServerApi,
  type StoreAchievement,
  type StoreGame,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
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

// Achievements — preview-only fetch via the games.achievements binding.
// Each row already carries the caller's unlock state so we don't have
// to round-trip a second time for membership checks. Preview cap is
// tuned for the sidebar grid (4 cols × 4 rows of small tiles); the
// "Show all" link expands to the full list inline.
const ACHIEVEMENTS_PREVIEW = 16;
const achievements = ref<StoreAchievement[]>([]);
const achievementsLoading = ref(false);
const achievementsExpanded = ref(false);

const unlockedCount = computed(
  () => achievements.value.filter((a) => a.unlocked).length,
);
const unlockedPercent = computed(() =>
  achievements.value.length > 0
    ? (unlockedCount.value / achievements.value.length) * 100
    : 0,
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

/** Per-game achievement list with the caller's unlock state baked in.
 *  Soft-fails to an empty list so the section hides cleanly when the
 *  endpoint isn't available (no provider linked, network down, etc.). */
async function loadAchievements() {
  if (!gameId.value) return;
  achievementsLoading.value = true;
  try {
    achievements.value = await api.games.achievements(gameId.value);
  } catch (e) {
    console.warn("[store/[id]] failed to load achievements:", e);
    achievements.value = [];
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

onMounted(async () => {
  await load();
  // Kick off the remaining fetches in parallel — none of them block
  // first paint of the hero + about block, so we keep them independent.
  loadStoreMeta();
  checkInLibrary();
  loadAchievements();
});

// Watch gameId for navigation between different store/[id] pages without
// a full remount (e.g. clicking a related game tile in the future).
watch(gameId, async () => {
  gameRef.value = null;
  storeMeta.value = null;
  currentImage.value = 0;
  inLibrary.value = null;
  libraryError.value = null;
  achievements.value = [];
  achievementsExpanded.value = false;
  await load();
  loadStoreMeta();
  checkInLibrary();
  loadAchievements();
});
</script>
