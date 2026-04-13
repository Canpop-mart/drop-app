<template>
  <div class="flex flex-col h-full">
    <!-- Back button + title -->
    <div class="flex items-center gap-4 px-8 py-4 border-b border-zinc-800/30">
      <button
        :ref="
          (el: any) =>
            registerButton(el, {
              onSelect: () => $router.push('/bigpicture/library'),
            })
        "
        class="flex items-center gap-2 px-3 py-2 text-sm rounded-lg font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50 transition-colors"
      >
        <ChevronLeftIcon class="size-4" />
        <span>Back to Library</span>
      </button>

      <div class="flex-1" />

      <h1 class="text-lg font-semibold text-zinc-200">Collections</h1>
    </div>

    <!-- Loading state with skeleton grid -->
    <div v-if="loading" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="grid gap-4 grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
        <div
          v-for="i in 8"
          :key="i"
          class="bg-zinc-800/50 rounded-2xl p-4 aspect-[3/4] animate-pulse"
        />
      </div>
    </div>

    <!-- Collections grid -->
    <div
      v-else
      ref="scrollContainer"
      class="flex-1 overflow-y-auto px-8 py-6"
      data-bp-scroll
    >
      <!-- Empty state -->
      <div v-if="collections.length === 0" class="flex items-center justify-center py-24">
        <div class="text-center">
          <FolderIcon class="size-16 mx-auto mb-4 text-zinc-600" />
          <h3 class="text-2xl font-semibold text-zinc-400 mb-2">No collections yet</h3>
          <p class="text-zinc-600">Create one to organize your games.</p>
        </div>
      </div>

      <!-- Collections and new collection card -->
      <div v-else class="grid gap-4 grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
        <div
          v-for="collection in collections"
          :key="collection.id"
          :ref="
            (el: any) =>
              registerCard(el, {
                onSelect: () => {
                  focusNav.saveFocusSnapshot(route.path);
                  $router.push(`/bigpicture/library?collection=${collection.id}`);
                },
              })
          "
          class="bg-zinc-800/50 hover:bg-zinc-700/50 rounded-2xl p-4 transition-colors cursor-pointer"
        >
          <!-- Mini grid of first 4 game covers -->
          <div class="grid grid-cols-2 gap-1 rounded-lg overflow-hidden aspect-square mb-3">
            <div
              v-for="(entry, idx) in collection.entries.slice(0, 4)"
              :key="idx"
              class="bg-zinc-900 flex items-center justify-center"
            >
              <img
                :src="useObject(entry.game.mCoverObjectId)"
                :alt="entry.game.mName"
                class="w-full h-full object-cover"
              />
            </div>
            <!-- Fill empty slots if less than 4 games -->
            <div
              v-for="idx in Math.max(0, 4 - collection.entries.length)"
              :key="`empty-${idx}`"
              class="bg-zinc-900"
            />
          </div>

          <!-- Collection name -->
          <h3 class="text-sm font-semibold text-zinc-200 mt-3 truncate">
            {{ collection.name }}
          </h3>

          <!-- Game count -->
          <p class="text-xs text-zinc-500">
            {{ collection.entries.length }} game{{ collection.entries.length !== 1 ? 's' : '' }}
          </p>
        </div>

        <!-- New collection card -->
        <div
          :ref="
            (el: any) =>
              registerCard(el, {
                onSelect: () => {
                  showKeyboard = true;
                  keyboardInput = '';
                },
              })
          "
          class="border-2 border-dashed border-zinc-700 rounded-2xl p-4 flex flex-col items-center justify-center gap-2 cursor-pointer hover:border-zinc-600 transition-colors aspect-square"
        >
          <PlusIcon class="size-6 text-zinc-500" />
          <span class="text-sm font-medium text-zinc-400">New Collection</span>
        </div>
      </div>
    </div>

    <!-- On-screen keyboard for new collection -->
    <BigPictureKeyboard
      :visible="showKeyboard"
      :model-value="keyboardInput"
      placeholder="Collection name..."
      @update:model-value="keyboardInput = $event"
      @close="showKeyboard = false"
      @submit="createCollection"
    />
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronLeftIcon,
  FolderIcon,
  PlusIcon,
} from "@heroicons/vue/24/outline";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useObject } from "~/composables/use-object";
import { serverUrl } from "~/composables/use-server-fetch";
import type { Collection, Game } from "~/types";

definePageMeta({ layout: "bigpicture" });

interface FetchLibraryResponse {
  library: Game[];
  collections: Collection[];
  other: Game[];
  missing: Game[];
}

const collections: Ref<Collection[]> = ref([]);
const loading = ref(true);
const showKeyboard = ref(false);
const keyboardInput = ref("");
const scrollContainer = ref<HTMLElement | null>(null);
const focusNav = useFocusNavigation();
const registerButton = useBpFocusableGroup("content");
const registerCard = useBpFocusableGroup("content");
const route = useRoute();

async function loadCollections() {
  try {
    const data = await invoke<FetchLibraryResponse>("fetch_library", {
      hardRefresh: false,
    });
    collections.value = data.collections;
  } catch (e) {
    console.error("Failed to fetch collections:", e);
  } finally {
    loading.value = false;
    nextTick(() => {
      focusNav.autoFocusContent("content");
    });
  }
}

async function createCollection() {
  const collectionName = keyboardInput.value.trim();
  if (!collectionName) {
    showKeyboard.value = false;
    return;
  }

  try {
    const response = await fetch(
      serverUrl("api/v1/client/collection"),
      {
        method: "POST",
        body: JSON.stringify({ name: collectionName }),
        headers: { "Content-Type": "application/json" },
      }
    );

    if (!response.ok) {
      throw new Error(`Failed to create collection: ${response.status}`);
    }

    showKeyboard.value = false;
    keyboardInput.value = "";

    // Reload collections to show the new one
    await loadCollections();
  } catch (e) {
    console.error("Failed to create collection:", e);
  }
}

onMounted(async () => {
  await loadCollections();
  if (!focusNav.restoreFocusSnapshot(route.path)) {
    focusNav.autoFocusContent("content");
  }
});
</script>
