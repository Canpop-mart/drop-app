<template>
  <div class="mx-auto max-w-3xl px-8 py-8">
    <div class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="font-display text-2xl font-bold text-zinc-100">
          Favourite games
        </h1>
        <p class="mt-1 text-sm text-zinc-400">
          Pin up to {{ MAX }} games to your profile. Drag to reorder.
        </p>
      </div>
      <NuxtLink
        to="/profile"
        class="text-sm font-medium text-zinc-400 transition-colors hover:text-zinc-200"
      >
        Cancel
      </NuxtLink>
    </div>

    <div
      v-if="loading"
      class="flex min-h-[30vh] items-center justify-center text-sm text-zinc-500"
    >
      Loading…
    </div>

    <template v-else>
      <draggable
        v-model="ordered"
        item-key="gameId"
        class="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-5"
        :animation="150"
      >
        <template #item="{ element, index }">
          <div class="group relative">
            <div
              class="relative aspect-[3/4] cursor-grab overflow-hidden rounded-lg bg-zinc-800 ring-1 ring-zinc-700/50 active:cursor-grabbing"
            >
              <img
                v-if="element.cover"
                :src="objectUrl(element.cover)"
                class="h-full w-full object-cover"
              />
              <div
                v-else
                class="flex h-full w-full items-center justify-center px-2 text-center text-xs text-zinc-600"
              >
                {{ element.name }}
              </div>
              <button
                class="absolute right-1 top-1 rounded-full bg-black/70 p-1 text-zinc-300 opacity-0 transition-opacity hover:text-white group-hover:opacity-100"
                title="Remove"
                @click="remove(index)"
              >
                <XMarkIcon class="size-4" />
              </button>
            </div>
            <p class="mt-1 truncate text-xs text-zinc-400">{{ element.name }}</p>
          </div>
        </template>
        <template #footer>
          <button
            v-if="ordered.length < MAX"
            class="flex aspect-[3/4] items-center justify-center rounded-lg border-2 border-dashed border-zinc-700 text-zinc-500 transition-colors hover:border-zinc-500 hover:text-zinc-300"
            @click="pickerOpen = true"
          >
            <PlusIcon class="size-7" />
          </button>
        </template>
      </draggable>

      <p class="mt-3 text-xs text-zinc-500 tabular-nums">
        {{ ordered.length }} / {{ MAX }}
      </p>

      <div
        class="sticky bottom-0 -mx-8 mt-8 flex items-center justify-end gap-3 border-t border-zinc-800 bg-zinc-950/90 px-8 py-4 backdrop-blur-sm"
      >
        <p v-if="saveError" class="mr-auto text-sm text-red-400">
          {{ saveError }}
        </p>
        <p v-else-if="saveOk" class="mr-auto text-sm text-green-400">Saved!</p>
        <NuxtLink
          to="/profile"
          class="rounded-md px-4 py-2 text-sm font-medium text-zinc-400 transition-colors hover:text-zinc-200"
        >
          Cancel
        </NuxtLink>
        <button
          class="rounded-md bg-blue-600 px-5 py-2 text-sm font-semibold text-white transition-colors hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="saving"
          @click="save"
        >
          {{ saving ? "Saving…" : "Save" }}
        </button>
      </div>
    </template>

    <GamePickerModal
      :open="pickerOpen"
      placeholder="Search games to add…"
      @close="pickerOpen = false"
      @select="addGame"
    />
  </div>
</template>

<script setup lang="ts">
import { PlusIcon, XMarkIcon } from "@heroicons/vue/24/solid";
import draggable from "vuedraggable";
import GamePickerModal from "~/components/GamePickerModal.vue";
import {
  useServerApi,
  type FavoriteSearchRow,
} from "~/composables/use-server-api";
import { objectImageUrl } from "~/composables/use-object";

useHead({ title: "Favourite games" });

const MAX = 10;
const router = useRouter();
const api = useServerApi();

interface FavItem {
  gameId: string;
  name: string;
  cover: string | null;
}

const ordered = ref<FavItem[]>([]);
const loading = ref(true);
const pickerOpen = ref(false);
const saving = ref(false);
const saveError = ref<string | null>(null);
const saveOk = ref(false);

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

function addGame(g: FavoriteSearchRow) {
  pickerOpen.value = false;
  if (ordered.value.length >= MAX) return;
  if (ordered.value.some((o) => o.gameId === g.id)) return;
  ordered.value.push({ gameId: g.id, name: g.mName, cover: g.mCoverObjectId });
}

function remove(index: number) {
  ordered.value.splice(index, 1);
}

async function save() {
  saving.value = true;
  saveError.value = null;
  saveOk.value = false;
  try {
    await api.profile.favorites.reorder(ordered.value.map((o) => o.gameId));
    saveOk.value = true;
    setTimeout(() => router.push("/profile"), 700);
  } catch (e) {
    saveError.value =
      "Save failed: " + (e instanceof Error ? e.message : String(e));
  } finally {
    saving.value = false;
  }
}

onMounted(async () => {
  try {
    const list = await api.profile.favorites.list();
    ordered.value = list.map((f) => ({
      gameId: f.gameId,
      name: f.game?.mName ?? "Game",
      cover: f.game?.mCoverObjectId ?? null,
    }));
  } catch {
    saveError.value = "Couldn't load favourites.";
  } finally {
    loading.value = false;
  }
});
</script>
