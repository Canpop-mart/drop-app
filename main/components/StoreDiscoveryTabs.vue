<template>
  <!-- Width is owned by the parent now (the Store page caps the
       whole Featured column at 1600px), so this section just fills
       whatever it's given.  Earlier iterations capped here directly,
       which made the table look slim against a wider hero + cards
       above it. -->
  <section class="mb-12">
    <!-- Tab strip — slightly bigger and bolder than before so the
         tabs themselves feel like real navigation, not breadcrumbs. -->
    <div class="flex items-center gap-1 border-b border-zinc-700/50 mb-0">
      <button
        v-for="t in tabs"
        :key="t.value"
        class="relative px-6 py-3.5 text-sm font-semibold transition-colors"
        :class="
          activeTab === t.value
            ? 'text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200'
        "
        @click="setTab(t.value)"
      >
        {{ t.label }}
        <span
          v-if="activeTab === t.value"
          class="absolute bottom-0 left-3 right-3 h-0.5 bg-blue-500 rounded-full"
        />
      </button>
    </div>

    <!-- Two-column body: vertical list on the left, sticky preview on
         the right.  No `overflow-hidden` on this grid wrapper — that
         would establish a non-scrolling ancestor for the sticky
         preview inside, which silently broke window-scroll stickiness
         and let the preview's top scroll off the screen.  Each column
         clips its own bottom corner instead. -->
    <div
      class="grid lg:grid-cols-[1fr_400px] gap-0 bg-zinc-900/40 rounded-b-xl ring-1 ring-zinc-700/40"
    >
      <!-- List column -->
      <div
        class="divide-y divide-zinc-800/60 overflow-hidden rounded-bl-xl"
      >
        <div
          v-if="loading"
          class="p-8 text-sm text-zinc-500 text-center"
        >
          Loading…
        </div>
        <div
          v-else-if="games.length === 0"
          class="p-8 text-sm text-zinc-500 text-center"
        >
          Nothing here yet.
        </div>
        <button
          v-for="g in games"
          :key="g.id"
          class="w-full flex items-center gap-4 px-5 py-4 text-left transition-colors"
          :class="{
            'bg-blue-500/10 ring-1 ring-inset ring-blue-500/40':
              hoveredId === g.id,
            'hover:bg-zinc-800/50': hoveredId !== g.id,
          }"
          @mouseenter="hoveredId = g.id"
          @focus="hoveredId = g.id"
          @click="$emit('select', g.id)"
        >
          <!-- Capsule thumbnail — wider + taller so it actually reads
               as game banner art instead of a tiny chip. Steam uses
               ~184x69; we use 160x80 for the slightly less wide
               viewport budget. -->
          <div
            class="shrink-0 w-40 h-20 rounded-md overflow-hidden bg-zinc-800 ring-1 ring-zinc-700/50"
          >
            <img
              v-if="g.mBannerObjectId"
              :src="objectUrl(g.mBannerObjectId)"
              :alt="g.mName"
              class="w-full h-full object-cover"
              loading="lazy"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-700 font-display font-bold text-2xl"
            >
              {{ g.mName.charAt(0).toUpperCase() }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p class="text-sm font-semibold text-zinc-100 truncate">
              {{ g.mName }}
            </p>
            <!-- Tag chips inline — Steam-style, small but actual
                 chip-shaped not comma-separated text. -->
            <div
              v-if="g.tags && g.tags.length > 0"
              class="flex flex-wrap gap-1 mt-1.5"
            >
              <span
                v-for="tag in g.tags.slice(0, 4)"
                :key="tag.id"
                class="px-1.5 py-0.5 rounded text-[10px] text-zinc-300 bg-zinc-800/80 ring-1 ring-zinc-700/40"
              >
                {{ tag.name }}
              </span>
            </div>
          </div>

          <div class="shrink-0 text-right">
            <p class="text-xs text-zinc-400 tabular-nums">
              {{ rowMeta(g) }}
            </p>
            <p
              v-if="libraryGameIds?.has(g.id)"
              class="text-[10px] font-bold uppercase tracking-wider text-blue-400 mt-1"
            >
              In library
            </p>
          </div>
        </button>

        <!-- "See more" footer -->
        <div class="px-5 py-4 flex justify-end">
          <button
            class="text-xs font-semibold text-zinc-300 hover:text-zinc-100 px-3 py-1.5 rounded-md bg-zinc-800/60 ring-1 ring-zinc-700/40 hover:ring-zinc-600/60 transition-colors uppercase tracking-wider"
            @click="$emit('see-more', activeTab)"
          >
            See more →
          </button>
        </div>
      </div>

      <!-- Preview column — sticky on lg+ so it follows the user's
           scroll. `max-h-[calc(100vh-5rem)] overflow-y-auto` keeps the
           title visible even when the preview content is taller than
           the viewport — without it, scrolling down to a bottom row
           clipped the preview header above the visible area. The 5rem
           buffer accounts for the top nav so the preview never sits
           behind it. Rounded bottom-right corner here instead of on
           the outer grid (see comment on grid wrapper). -->
      <aside
        class="hidden lg:block bg-zinc-900/60 border-l border-zinc-700/40 rounded-br-xl"
      >
        <div
          v-if="preview"
          class="sticky top-0 max-h-[calc(100vh-5rem)] overflow-y-auto p-6 space-y-4 preview-scroll"
        >
          <!-- Cover art header — bigger now so it reads as the
               headline of the panel. -->
          <div
            class="aspect-[16/9] rounded-lg overflow-hidden bg-zinc-800 ring-1 ring-zinc-700/40 shadow-lg"
          >
            <img
              v-if="preview.mBannerObjectId"
              :src="objectUrl(preview.mBannerObjectId)"
              :alt="preview.mName"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-700 text-5xl font-display font-bold"
            >
              {{ preview.mName.charAt(0).toUpperCase() }}
            </div>
          </div>

          <h4 class="text-lg font-display font-semibold text-zinc-100 leading-tight">
            {{ preview.mName }}
          </h4>

          <!-- Description — capped at 4 lines now that we have a wider
               column. -->
          <p
            v-if="preview.mShortDescription"
            class="text-xs text-zinc-400 leading-relaxed line-clamp-4"
          >
            {{ preview.mShortDescription }}
          </p>

          <!-- Tag pills — bigger, more pill-like to match Steam. -->
          <div
            v-if="preview.tags && preview.tags.length > 0"
            class="flex flex-wrap gap-1.5"
          >
            <span
              v-for="tag in preview.tags.slice(0, 8)"
              :key="tag.id"
              class="px-2 py-1 rounded text-[11px] text-zinc-200 bg-zinc-800 ring-1 ring-zinc-700/50"
            >
              {{ tag.name }}
            </span>
          </div>

          <!-- Screenshot strip — pulls from the game's image carousel.
               2 stacked screenshots at a slightly larger aspect read
               cleaner than the previous 3 cramped ones. -->
          <div
            v-if="previewScreenshots.length > 0"
            class="space-y-2 pt-1"
          >
            <div
              v-for="(id, i) in previewScreenshots"
              :key="id"
              class="aspect-video rounded-md overflow-hidden bg-zinc-800 ring-1 ring-zinc-700/40"
            >
              <img
                :src="objectUrl(id)"
                :alt="`${preview.mName} screenshot ${i + 1}`"
                class="w-full h-full object-cover"
                loading="lazy"
              />
            </div>
          </div>
        </div>

        <!-- No game hovered yet — render a static prompt. -->
        <div
          v-else
          class="p-8 text-center text-xs text-zinc-500 sticky top-0"
        >
          Hover a game to preview
        </div>
      </aside>
    </div>
  </section>
</template>

<script setup lang="ts">
/**
 * Steam-style "New & Trending" widget: a tab strip across the top
 * (New / Most Played / Random) with a two-column body — vertical game
 * list on the left, sticky preview panel on the right.
 *
 * The parent owns data fetching; we just render whatever's passed in.
 * Tab state lives here so switching tabs is instantaneous, and we emit
 * a `tab` event so the parent can lazy-fetch the corresponding list.
 */
import type { StoreGame } from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";

type TabValue = "new" | "popular" | "random";

const props = withDefaults(
  defineProps<{
    /** Currently rendered game list. Parent provides per-tab data. */
    games: StoreGame[];
    loading?: boolean;
    /** Active tab — supports v-model:tab so parent can lift state if
     *  needed for fetch coordination. */
    tab?: TabValue;
    /** Optional library-ownership set for the "In library" tag. */
    libraryGameIds?: Set<string>;
    /** Sometimes Drop's StoreGame carries an extra `playerCount` /
     *  `totalPlaytimeHours` field (the popular endpoint adds them).
     *  We read off them dynamically — they're optional. */
  }>(),
  { loading: false, tab: "new" },
);

const emit = defineEmits<{
  (e: "select", gameId: string): void;
  (e: "tab", value: TabValue): void;
  (e: "see-more", value: TabValue): void;
}>();

const tabs: Array<{ label: string; value: TabValue }> = [
  { label: "New", value: "new" },
  { label: "Most Played", value: "popular" },
  { label: "Random", value: "random" },
];

const activeTab = ref<TabValue>(props.tab);
watch(
  () => props.tab,
  (v) => {
    if (v !== activeTab.value) activeTab.value = v;
  },
);
function setTab(v: TabValue) {
  if (activeTab.value === v) return;
  activeTab.value = v;
  hoveredId.value = null;
  emit("tab", v);
}

// Hover preview: tracks which row the cursor / focus is on.  The
// right-side panel always reflects this, defaulting to the first
// item once data lands so users see *something* on tab change before
// they hover.
const hoveredId = ref<string | null>(null);
watch(
  () => props.games,
  (list) => {
    if (list.length === 0) {
      hoveredId.value = null;
    } else if (!hoveredId.value || !list.find((g) => g.id === hoveredId.value)) {
      hoveredId.value = list[0]?.id ?? null;
    }
  },
  { immediate: true },
);

const preview = computed(() =>
  props.games.find((g) => g.id === hoveredId.value) ?? null,
);

// Strip the first 3 screenshots from the previewed game's carousel
// images.  Steam shows ~4 stacked thumbs; 3 keeps it tight and
// matches the panel's vertical budget on most displays.
const previewScreenshots = computed(() => {
  const ids = (preview.value as StoreGame & {
    mImageCarouselObjectIds?: string[];
  })?.mImageCarouselObjectIds;
  return Array.isArray(ids) ? ids.slice(0, 3) : [];
});

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

// Per-row meta — varies by tab. New shows release/added date, Most
// Played shows total playtime hours, Random shows a generic tag.
function rowMeta(g: StoreGame): string {
  if (activeTab.value === "popular") {
    const hrs = (g as StoreGame & { totalPlaytimeHours?: number })
      .totalPlaytimeHours;
    if (typeof hrs === "number" && hrs > 0) {
      return `${hrs.toLocaleString()}h played`;
    }
    return "";
  }
  if (activeTab.value === "new") {
    const created = (g as StoreGame & { created?: string }).created;
    return created ? new Date(created).toLocaleDateString() : "";
  }
  // Random tab: show a tag or count of tags as a low-key meta line.
  return g.tags && g.tags.length > 0 ? `${g.tags.length} tags` : "";
}
</script>

<style scoped>
/* Thin, low-contrast internal scrollbar for the preview when its
   content overflows the viewport. Visible enough to hint that more
   content is below, quiet enough not to compete with the panel. */
.preview-scroll {
  scrollbar-width: thin;
  scrollbar-color: rgb(82 82 91 / 0.5) transparent;
}
.preview-scroll::-webkit-scrollbar {
  width: 6px;
}
.preview-scroll::-webkit-scrollbar-thumb {
  background-color: rgb(82 82 91 / 0.5);
  border-radius: 3px;
}
</style>
