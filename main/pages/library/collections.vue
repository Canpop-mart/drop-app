<template>
  <div class="h-full flex flex-col px-8 py-6 overflow-y-auto">
    <div class="flex items-center justify-between mb-6">
      <div>
        <h1 class="text-2xl font-display font-bold text-zinc-100">
          Collections
        </h1>
        <p class="mt-1 text-sm text-zinc-400">
          Group games into shelves you can navigate to from the sidebar.
        </p>
      </div>
      <button
        class="inline-flex items-center gap-x-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 transition-colors"
        @click="newShelfModalOpen = true"
      >
        <PlusIcon class="size-4" />
        New collection
      </button>
    </div>

    <!-- Loading / empty / list states -->
    <div
      v-if="loading"
      class="flex-1 flex items-center justify-center text-sm text-zinc-500"
    >
      Loading collections...
    </div>

    <div
      v-else-if="shelves.length === 0"
      class="flex-1 flex flex-col items-center justify-center text-center"
    >
      <div class="rounded-2xl bg-zinc-800/50 p-6 mb-4">
        <Square3Stack3DIcon class="size-10 text-zinc-500" />
      </div>
      <p class="text-sm text-zinc-400 max-w-md mb-4">
        No collections yet. Create one to organize favourites, in-progress
        games, or any other grouping that makes sense for your library.
      </p>
      <button
        class="inline-flex items-center gap-x-2 rounded-md bg-blue-600 px-5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 transition-colors"
        @click="newShelfModalOpen = true"
      >
        <PlusIcon class="size-4" />
        Create your first collection
      </button>
    </div>

    <div v-else class="space-y-8 pb-8">
      <section
        v-for="shelf in shelves"
        :key="shelf.id"
        class="rounded-xl bg-zinc-800/40 ring-1 ring-zinc-700/40 p-5"
      >
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-2 min-w-0">
            <Square3Stack3DIcon class="size-5 text-zinc-500 shrink-0" />
            <h2 class="text-lg font-display font-semibold text-zinc-100 truncate">
              {{ shelf.name }}
            </h2>
            <span
              class="text-xs text-zinc-500 ml-2 tabular-nums shrink-0"
            >
              ({{ shelf.entries.length }})
            </span>
            <span
              v-if="shelf.isPublic"
              class="text-[10px] font-bold uppercase bg-green-500/20 text-green-300 px-1.5 py-0.5 rounded ml-2"
              title="Visible on your public profile"
            >
              Public
            </span>
          </div>
          <Menu as="div" class="relative">
            <MenuButton
              class="rounded-md p-1.5 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700/50 transition-colors"
            >
              <EllipsisHorizontalIcon class="size-5" />
            </MenuButton>
            <Transition
              enter-active-class="transition ease-out duration-100"
              enter-from-class="opacity-0 scale-95"
              enter-to-class="opacity-100 scale-100"
              leave-active-class="transition ease-in duration-75"
              leave-from-class="opacity-100 scale-100"
              leave-to-class="opacity-0 scale-95"
            >
              <MenuItems
                class="absolute right-0 z-10 mt-1 w-48 origin-top-right rounded-md bg-zinc-900 shadow-lg ring-1 ring-zinc-700/50 focus:outline-none overflow-hidden"
              >
                <MenuItem v-slot="{ active }">
                  <button
                    class="w-full px-4 py-2 text-sm text-left flex items-center justify-between"
                    :class="
                      active
                        ? 'bg-zinc-800 text-zinc-100'
                        : 'text-zinc-400'
                    "
                    @click="toggleVisibility(shelf)"
                  >
                    {{ shelf.isPublic ? "Make private" : "Make public" }}
                    <EyeIcon class="size-4" />
                  </button>
                </MenuItem>
                <MenuItem v-slot="{ active }">
                  <button
                    class="w-full px-4 py-2 text-sm text-left flex items-center justify-between text-red-400"
                    :class="active ? 'bg-zinc-800' : ''"
                    @click="confirmDelete(shelf)"
                  >
                    Delete collection
                    <TrashIcon class="size-4" />
                  </button>
                </MenuItem>
              </MenuItems>
            </Transition>
          </Menu>
        </div>

        <!-- Game tiles -->
        <div
          v-if="shelf.entries.length === 0"
          class="text-sm text-zinc-500 italic py-4"
        >
          Empty. Add games from any game's detail page.
        </div>
        <GameTileGrid v-else density="compact">
          <GameTile
            v-for="entry in shelf.entries"
            :key="entry.gameId"
            :cover-url="entry.game.mCoverObjectId ? useObject(entry.game.mCoverObjectId) : null"
            :name="entry.game.mName"
            rounded="lg"
            @select="goToGame(entry.gameId)"
          />
        </GameTileGrid>
      </section>
    </div>

    <!-- New collection modal -->
    <ModalTemplate v-model="newShelfModalOpen">
      <template #default>
        <h3 class="text-base font-semibold text-zinc-100 mb-3">
          New collection
        </h3>
        <p class="text-sm text-zinc-400 mb-4">
          A name to group games under. You can rename or delete it later.
        </p>
        <input
          v-model="newShelfName"
          type="text"
          maxlength="64"
          placeholder="e.g. Currently playing"
          class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:bg-zinc-800 focus:border-blue-500 focus:ring-2 focus:ring-blue-500/30 outline-none transition-colors"
          @keydown.enter="createNewShelf"
        />
      </template>
      <template #buttons>
        <button
          type="button"
          class="ml-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          :disabled="!newShelfName.trim() || creating"
          @click="createNewShelf"
        >
          {{ creating ? "Creating..." : "Create" }}
        </button>
        <button
          type="button"
          class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 ring-1 ring-inset ring-zinc-700 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
          @click="newShelfModalOpen = false"
        >
          Cancel
        </button>
      </template>
    </ModalTemplate>
  </div>
</template>

<script setup lang="ts">
import {
  PlusIcon,
  Square3Stack3DIcon,
  EllipsisHorizontalIcon,
  TrashIcon,
  EyeIcon,
} from "@heroicons/vue/24/outline";
import { Menu, MenuButton, MenuItem, MenuItems } from "@headlessui/vue";
import { invoke } from "@tauri-apps/api/core";
import { useShelves, type Shelf } from "~/composables/shelves";

useHead({ title: "Collections" });

const router = useRouter();
const {
  shelves,
  loading,
  fetchShelves,
  createShelf,
  deleteShelf,
  toggleShelfVisibility,
} = useShelves();

const newShelfModalOpen = ref(false);
const newShelfName = ref("");
const creating = ref(false);

function goToGame(gameId: string) {
  invoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/library/${gameId}`);
}

async function createNewShelf() {
  const name = newShelfName.value.trim();
  if (!name) return;
  creating.value = true;
  try {
    await createShelf(name);
    newShelfName.value = "";
    newShelfModalOpen.value = false;
  } finally {
    creating.value = false;
  }
}

async function toggleVisibility(shelf: Shelf) {
  await toggleShelfVisibility(shelf.id, !shelf.isPublic);
}

async function confirmDelete(shelf: Shelf) {
  // Defer to a real confirmation modal if we add one — for now, native
  // confirm matches the desktop convention for destructive actions like
  // "uninstall" elsewhere in the app.
  createModal(
    ModalType.Confirmation,
    {
      title: `Delete "${shelf.name}"?`,
      description:
        "This collection's grouping is removed. The games themselves aren't deleted.",
      buttonText: "Delete",
    },
    async (e, c) => {
      c();
      if (e === "confirm") {
        await deleteShelf(shelf.id);
      }
    },
  );
}

onMounted(fetchShelves);
</script>
