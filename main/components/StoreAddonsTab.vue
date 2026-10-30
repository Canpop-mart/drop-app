<template>
  <!-- The desktop Store's Addons tab. Big Picture has its own version
       (components/bigpicture/BpmStoreAddons.vue) with the same logic
       (composables/addon-shop.ts). -->
  <div class="mx-auto max-w-[1100px]">
    <p class="mb-6 text-sm text-zinc-400">
      Addons are extras run by your server.
    </p>

    <div v-if="shop.status.value === 'loading'" class="space-y-3">
      <div
        v-for="i in 2"
        :key="i"
        class="h-28 rounded-2xl bg-zinc-800/50 animate-pulse"
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

    <ul v-else class="space-y-4">
      <li
        v-for="addon in shop.addons.value"
        :key="addon.name"
        class="rounded-2xl bg-zinc-900 p-5 ring-1 ring-zinc-800/60"
      >
        <div class="flex flex-wrap items-start justify-between gap-4">
          <div class="min-w-0 flex-1">
            <h3 class="text-lg font-semibold text-zinc-100">
              {{ addon.title }}
              <span
                v-if="addon.added"
                class="ml-2 align-middle rounded-full bg-blue-500/15 px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest text-blue-300"
              >
                Added
              </span>
            </h3>
            <p v-if="addon.summary" class="mt-1 text-sm text-zinc-300">
              {{ addon.summary }}
            </p>
            <p
              v-if="addon.description"
              class="mt-2 whitespace-pre-line text-sm text-zinc-400"
            >
              {{ addon.description }}
            </p>
            <button
              v-if="addon.site"
              class="mt-2 inline-flex items-center gap-1 text-sm text-blue-400 hover:text-blue-300"
              @click="openSite(addon.name, addon.site)"
            >
              Website
              <ArrowTopRightOnSquareIcon class="size-4" />
            </button>
            <p v-if="siteError === addon.name" class="mt-1 text-sm text-red-300">
              Couldn't open the website.
            </p>
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
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { ArrowTopRightOnSquareIcon } from "@heroicons/vue/24/outline";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { rowView, type ShopAddon } from "~/composables/addon-shop-logic";
import { useAddonShop } from "~/composables/addon-shop";

const shop = useAddonShop();

function view(addon: ShopAddon) {
  return rowView(addon, shop.confirming.value, shop.busy.value);
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
