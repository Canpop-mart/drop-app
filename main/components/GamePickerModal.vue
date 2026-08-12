<!--
  Shared game-search modal used by the favourites + showcase editors. Emits
  `select` with the chosen game row. Debounced search over the existing
  favourites/game search endpoint.
-->
<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-start justify-center bg-black/60 p-4 pt-24"
    @click.self="$emit('close')"
  >
    <div
      class="w-full max-w-lg overflow-hidden rounded-2xl bg-zinc-900 shadow-2xl ring-1 ring-zinc-700/60"
    >
      <div class="flex items-center gap-2 border-b border-zinc-800 p-3">
        <MagnifyingGlassIcon class="size-5 shrink-0 text-zinc-500" />
        <input
          ref="inputEl"
          v-model="q"
          type="text"
          :placeholder="placeholder"
          class="flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          @input="scheduleSearch"
        />
        <button
          class="shrink-0 text-zinc-500 hover:text-zinc-300"
          @click="$emit('close')"
        >
          <XMarkIcon class="size-5" />
        </button>
      </div>
      <div class="max-h-80 overflow-y-auto p-2">
        <div v-if="loading" class="py-8 text-center text-sm text-zinc-500">
          Searching…
        </div>
        <div
          v-else-if="q.trim() && results.length === 0"
          class="py-8 text-center text-sm text-zinc-500"
        >
          No games found.
        </div>
        <div
          v-else-if="!q.trim()"
          class="py-8 text-center text-sm text-zinc-600"
        >
          Type to search games.
        </div>
        <button
          v-for="g in results"
          :key="g.id"
          class="flex w-full items-center gap-3 rounded-lg p-2 text-left transition-colors hover:bg-zinc-800"
          @click="$emit('select', g)"
        >
          <img
            v-if="g.mCoverObjectId"
            :src="objectUrl(g.mCoverObjectId)"
            class="h-12 w-9 shrink-0 rounded object-cover"
          />
          <div v-else class="h-12 w-9 shrink-0 rounded bg-zinc-800" />
          <span class="flex-1 truncate text-sm text-zinc-200">{{
            g.mName
          }}</span>
          <CheckCircleIcon
            v-if="g.isFavorite"
            class="size-4 shrink-0 text-zinc-500"
            title="Already a favourite"
          />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  MagnifyingGlassIcon,
  XMarkIcon,
  CheckCircleIcon,
} from "@heroicons/vue/24/solid";
import {
  useServerApi,
  type FavoriteSearchRow,
} from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";

const props = withDefaults(
  defineProps<{ open: boolean; placeholder?: string }>(),
  { placeholder: "Search games…" },
);

defineEmits<{
  (e: "close"): void;
  (e: "select", game: FavoriteSearchRow): void;
}>();

const api = useServerApi();
const q = ref("");
const results = ref<FavoriteSearchRow[]>([]);
const loading = ref(false);
const inputEl = ref<HTMLInputElement | null>(null);
let timer: ReturnType<typeof setTimeout> | null = null;

function scheduleSearch() {
  if (timer) clearTimeout(timer);
  const term = q.value.trim();
  if (!term) {
    results.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  timer = setTimeout(async () => {
    try {
      results.value = await api.profile.favorites.search(term);
    } catch {
      results.value = [];
    } finally {
      loading.value = false;
    }
  }, 250);
}

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

watch(
  () => props.open,
  (o) => {
    if (o) {
      q.value = "";
      results.value = [];
      nextTick(() => inputEl.value?.focus());
    }
  },
);
</script>
