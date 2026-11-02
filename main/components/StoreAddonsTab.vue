<template>
  <!-- The desktop Store's Addons tab. Big Picture has its own version
       (components/bigpicture/BpmStoreAddons.vue) with the same logic
       (composables/addon-shop.ts). -->
  <div class="mx-auto max-w-[1900px] pt-6">
    <div class="mb-7 flex items-end justify-between gap-4">
      <div>
        <h2 class="font-display text-2xl font-semibold text-zinc-100">Addons</h2>
        <p class="mt-1 text-sm text-zinc-400">Add features and services to Drop.</p>
      </div>
      <p v-if="shop.status.value === 'ready'" class="text-sm tabular-nums text-zinc-500">
        {{ shop.addons.value.length }} available
      </p>
    </div>

    <div v-if="shop.status.value === 'loading'" class="grid gap-5 md:grid-cols-2">
      <div
        v-for="i in 2"
        :key="i"
        class="h-96 animate-pulse rounded-2xl bg-zinc-800/50"
      />
    </div>

    <SectionError
      v-else-if="shop.status.value === 'error'"
      title="Couldn't load addons"
      @retry="shop.load"
    />

    <div
      v-else-if="shop.addons.value.length === 0"
      class="py-20 text-center text-sm text-zinc-500"
    >
      No addons on this server yet.
    </div>

    <div v-else class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
      <ul class="grid gap-5 md:grid-cols-2">
        <li
          v-for="addon in shop.addons.value"
          :key="addon.name"
          class="overflow-hidden rounded-2xl bg-zinc-900 ring-1 ring-zinc-800/70"
          :class="{ 'md:col-span-2': shop.addons.value.length === 1 }"
        >
          <div class="relative h-40 overflow-hidden bg-gradient-to-br from-blue-950 via-indigo-950 to-zinc-950 p-6">
            <div class="absolute -right-12 -top-16 size-56 rounded-full bg-blue-500/15 blur-3xl" />
            <div class="relative flex h-full items-end justify-between gap-4">
              <div class="flex size-16 items-center justify-center rounded-2xl bg-white/10 text-xl font-bold text-white ring-1 ring-white/15">
                {{ addonMark(addon) }}
              </div>
              <span class="rounded-full bg-black/30 px-3 py-1 text-[10px] font-semibold uppercase tracking-[0.18em] text-blue-200 ring-1 ring-white/10">
                Drop addon
              </span>
            </div>
          </div>

          <div class="p-6">
            <div class="flex flex-wrap items-start justify-between gap-4">
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <h3 class="font-display text-xl font-semibold text-zinc-100">{{ addon.title }}</h3>
                  <span
                    v-if="addon.added"
                    class="rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest text-emerald-300"
                  >
                    Added
                  </span>
                </div>
                <p v-if="addon.summary" class="mt-2 text-sm text-zinc-300">{{ addon.summary }}</p>
                <p v-if="addon.description" class="mt-3 max-w-3xl whitespace-pre-line text-sm leading-6 text-zinc-400">
                  {{ addon.description }}
                </p>
                <button
                  v-if="addon.site"
                  class="mt-4 inline-flex items-center gap-1.5 text-sm font-medium text-blue-400 hover:text-blue-300"
                  @click="openSite(addon.name, addon.site)"
                >
                  Website
                  <ArrowTopRightOnSquareIcon class="size-4" />
                </button>
                <p v-if="siteError === addon.name" class="mt-2 text-sm text-red-300">Couldn't open the website.</p>
              </div>

              <div class="flex shrink-0 flex-col items-end gap-2">
            <template v-if="view(addon).confirming">
              <p class="max-w-xs text-right text-sm text-zinc-300">
                Remove {{ addon.title }}? Anything it holds for you stays.
              </p>
              <div class="flex gap-2">
                <button
                  :disabled="view(addon).locked"
                  class="rounded-lg bg-red-600 px-3 py-1.5 text-sm font-semibold text-white hover:bg-red-500 disabled:opacity-50"
                  @click="shop.remove(addon.name)"
                >
                  {{ view(addon).busy ? "Removing..." : "Remove" }}
                </button>
                <button
                  :disabled="view(addon).locked"
                  class="rounded-lg px-3 py-1.5 text-sm font-semibold text-zinc-300 hover:text-zinc-100 disabled:opacity-50"
                  @click="shop.cancelRemove"
                >
                  Cancel
                </button>
              </div>
            </template>
            <button
              v-else-if="view(addon).primary === 'remove'"
              :disabled="view(addon).locked"
              class="rounded-lg bg-zinc-800 px-3 py-1.5 text-sm font-semibold text-zinc-100 hover:bg-zinc-700 disabled:opacity-50"
              @click="shop.askRemove(addon.name)"
            >
              Remove
            </button>
            <button
              v-else
              :disabled="view(addon).locked"
              class="rounded-lg bg-blue-600 px-3 py-1.5 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-50"
              @click="shop.add(addon.name)"
            >
              {{ view(addon).busy ? "Adding..." : "Add" }}
            </button>
            <p
              v-if="shop.actionError.value?.name === addon.name"
              class="max-w-xs text-right text-sm text-red-300"
            >
              {{
                shop.actionError.value.kind === "add"
                  ? "Couldn't add it. Try again."
                  : "Couldn't remove it. Try again."
              }}
            </p>
              </div>
            </div>
          </div>
        </li>
      </ul>

      <aside class="rounded-2xl bg-zinc-900/60 p-6 ring-1 ring-zinc-800/70">
        <PuzzlePieceIcon class="size-8 text-blue-300" />
        <h3 class="mt-5 font-display text-lg font-semibold text-zinc-100">About addons</h3>
        <p class="mt-2 text-sm leading-6 text-zinc-400">
          Addons extend Drop with services provided by this server. They are not game mods and do not change game files.
        </p>
        <div class="mt-6 space-y-4 border-t border-zinc-800 pt-5 text-sm">
          <div>
            <p class="font-medium text-zinc-200">Account based</p>
            <p class="mt-1 text-zinc-500">Addons follow your Drop account across devices.</p>
          </div>
          <div>
            <p class="font-medium text-zinc-200">Server provided</p>
            <p class="mt-1 text-zinc-500">Available addons are chosen and operated by your server.</p>
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ArrowTopRightOnSquareIcon, PuzzlePieceIcon } from "@heroicons/vue/24/outline";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { rowView, type ShopAddon } from "~/composables/addon-shop-logic";
import { useAddonShop } from "~/composables/addon-shop";

const shop = useAddonShop();

function view(addon: ShopAddon) {
  return rowView(addon, shop.confirming.value, shop.busy.value);
}

function addonMark(addon: ShopAddon): string {
  return addon.title
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("") || "D";
}

// Name of the addon whose website failed to open, shown on its row.
const siteError = ref<string | null>(null);

async function openSite(name: string, url: string) {
  siteError.value = null;
  try {
    await openExternal(url);
  } catch (e) {
    console.error("[ADDONS] could not open the website:", e);
    siteError.value = name;
  }
}

onMounted(shop.load);
</script>
