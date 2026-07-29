<template>
  <div class="mx-auto max-w-3xl px-8 py-8" :style="vars">
    <div class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="font-display text-2xl font-bold text-zinc-100">Showcase</h1>
        <p class="mt-1 text-sm text-zinc-400">
          Feature up to {{ MAX }} things on your profile. Drag to reorder.
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
        v-model="slots"
        item-key="_k"
        class="grid grid-cols-2 gap-3 sm:grid-cols-3"
        :animation="150"
      >
        <template #item="{ element, index }">
          <div class="group relative">
            <div
              class="relative aspect-[3/4] overflow-hidden rounded-xl bg-zinc-800/60 ring-1 ring-zinc-700/50"
            >
              <!-- Game / achievement: cover -->
              <template
                v-if="
                  element.type === 'FavoriteGame' ||
                  element.type === 'Achievement'
                "
              >
                <img
                  v-if="element.cover"
                  :src="objectUrl(element.cover)"
                  class="h-full w-full cursor-grab object-cover active:cursor-grabbing"
                />
                <div
                  v-else
                  class="flex h-full w-full cursor-grab items-center justify-center px-2 text-center text-xs text-zinc-500"
                >
                  {{ element.gameName || element.title }}
                </div>
                <div
                  class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 to-transparent p-2"
                >
                  <p class="truncate text-xs font-semibold text-white">
                    {{
                      element.type === "Achievement"
                        ? element.title
                        : element.gameName
                    }}
                  </p>
                  <p
                    class="text-[9px] font-semibold uppercase tracking-wide"
                    :style="{ color: 'var(--accent)' }"
                  >
                    {{ element.type === "Achievement" ? "Achievement" : "Game" }}
                  </p>
                </div>
              </template>

              <!-- Stat card -->
              <div
                v-else-if="element.type === 'GameStats'"
                class="flex h-full cursor-grab flex-col p-3 active:cursor-grabbing"
                :style="{ background: 'var(--accent-soft)' }"
              >
                <span
                  class="text-[9px] font-semibold uppercase tracking-wide"
                  :style="{ color: 'var(--accent)' }"
                >
                  Stat card
                </span>
                <p class="mt-2 truncate text-sm font-bold text-zinc-100">
                  {{ element.gameName }}
                </p>
                <p class="mt-auto text-[11px] text-zinc-400">
                  Playtime and achievements are filled in on your profile.
                </p>
              </div>

              <!-- Custom text -->
              <div
                v-else
                class="flex h-full flex-col p-3"
                :style="{ background: 'var(--accent-soft)' }"
              >
                <span
                  class="text-[9px] font-semibold uppercase tracking-wide"
                  :style="{ color: 'var(--accent)' }"
                >
                  Custom
                </span>
                <textarea
                  v-model="element.title"
                  maxlength="120"
                  placeholder="Say something…"
                  class="mt-2 flex-1 resize-none bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
                />
              </div>
            </div>
            <button
              class="absolute -right-1.5 -top-1.5 rounded-full bg-zinc-700 p-1 text-zinc-300 opacity-0 ring-2 ring-zinc-950 transition-opacity hover:bg-zinc-600 hover:text-white group-hover:opacity-100"
              title="Remove"
              @click="remove(index)"
            >
              <XMarkIcon class="size-3.5" />
            </button>
          </div>
        </template>
      </draggable>

      <div v-if="slots.length < MAX" class="mt-4 flex flex-wrap gap-2">
        <button
          v-for="add in addButtons"
          :key="add.type"
          class="inline-flex items-center gap-1.5 rounded-md bg-zinc-800/60 px-3 py-1.5 text-sm font-medium text-zinc-200 ring-1 ring-zinc-700/60 transition-colors hover:bg-zinc-800"
          @click="add.run()"
        >
          <PlusIcon class="size-4" />
          {{ add.label }}
        </button>
      </div>
      <p class="mt-3 text-xs text-zinc-500 tabular-nums">
        {{ slots.length }} / {{ MAX }}
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
      :placeholder="pickerPlaceholder"
      @close="pickerOpen = false"
      @select="onPick"
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
import { serverUrl } from "~/composables/use-server-fetch";
import { useProfileTheme } from "~/composables/use-profile-theme";

useHead({ title: "Showcase" });

const MAX = 12;
const router = useRouter();
const api = useServerApi();

type SlotType = "FavoriteGame" | "GameStats" | "Custom" | "Achievement";
interface Slot {
  _k: number;
  type: SlotType;
  gameId: string | null;
  itemId: string | null;
  title: string;
  cover: string | null;
  gameName: string | null;
}

let uid = 0;
const theme = ref<string | undefined>(undefined);
const { vars } = useProfileTheme(() => theme.value);

const slots = ref<Slot[]>([]);
const loading = ref(true);
const pickerOpen = ref(false);
const pendingType = ref<SlotType>("FavoriteGame");
const saving = ref(false);
const saveError = ref<string | null>(null);
const saveOk = ref(false);

const pickerPlaceholder = computed(() =>
  pendingType.value === "GameStats"
    ? "Search a game to show stats for…"
    : "Search games to feature…",
);

const addButtons = [
  { type: "FavoriteGame", label: "Game", run: () => openPicker("FavoriteGame") },
  { type: "GameStats", label: "Stat card", run: () => openPicker("GameStats") },
  { type: "Custom", label: "Custom text", run: addCustom },
];

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function openPicker(type: SlotType) {
  pendingType.value = type;
  pickerOpen.value = true;
}

function onPick(g: FavoriteSearchRow) {
  pickerOpen.value = false;
  if (slots.value.length >= MAX) return;
  slots.value.push({
    _k: uid++,
    type: pendingType.value,
    gameId: g.id,
    itemId: null,
    title: "",
    cover: g.mCoverObjectId,
    gameName: g.mName,
  });
}

function addCustom() {
  if (slots.value.length >= MAX) return;
  slots.value.push({
    _k: uid++,
    type: "Custom",
    gameId: null,
    itemId: null,
    title: "",
    cover: null,
    gameName: null,
  });
}

function remove(index: number) {
  slots.value.splice(index, 1);
}

async function save() {
  saving.value = true;
  saveError.value = null;
  saveOk.value = false;
  try {
    await api.profile.updateShowcase(
      slots.value.map((s) => ({
        type: s.type,
        gameId: s.gameId,
        itemId: s.itemId,
        title: s.title,
        data: null,
      })),
    );
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
    const me = await api.profile.me();
    theme.value = me.profileTheme;
    const res = await api.profile.showcase(me.id);
    slots.value = res.items.map((it) => ({
      _k: uid++,
      type: it.type,
      gameId: it.gameId,
      itemId: it.itemId,
      title: it.type === "Achievement" ? (it.achievement?.title ?? "") : it.title,
      cover: it.game?.mCoverObjectId ?? null,
      gameName: it.game?.mName ?? null,
    }));
  } catch {
    saveError.value = "Couldn't load your showcase.";
  } finally {
    loading.value = false;
  }
});
</script>
