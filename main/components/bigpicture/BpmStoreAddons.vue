<template>
  <!-- Big Picture's Store > Addons tab. Same logic as the desktop tab
       (composables/addon-shop.ts), its own instance. Every button is
       registered with the focus system. Removing asks on plain focusable
       rows, never a modal, so there is no input lock to leak.

       Two rules keep the controller path right:
       - Handlers take the addon's NAME and look the addon up when they fire.
         useBpFocusableGroup registers an element once and ignores later
         registrations of the same element, so a handler that closed over the
         addon object would keep acting on the state from the first render.
       - Focus follows the addon's name, never a row index, and moves to a
         button that stays mounted BEFORE the one holding it goes away.

       No Website button here: on a Deck in Game Mode a browser window would
       open behind gamescope. -->
  <div ref="root" class="mx-auto max-w-4xl">
    <p class="mb-6 text-sm" style="color: var(--bpm-muted)">
      Addons are extras run by your server.
    </p>

    <div v-if="shop.status.value === 'loading'" class="space-y-3">
      <div
        v-for="i in 2"
        :key="i"
        class="h-28 rounded-2xl animate-pulse"
        style="background-color: var(--bpm-surface)"
      />
    </div>

    <BigPictureSectionError
      v-else-if="shop.status.value === 'error'"
      :ref="(el: any) => register(el, { onSelect: retry })"
      data-addon-error
      title="Couldn't load addons"
      @retry="retry"
    />

    <p
      v-else-if="shop.addons.value.length === 0"
      class="py-20 text-center text-sm"
      style="color: var(--bpm-muted)"
    >
      No addons on this server yet.
    </p>

    <ul v-else class="space-y-4">
      <li
        v-for="addon in shop.addons.value"
        :key="addon.name"
        class="rounded-2xl p-6"
        style="background-color: var(--bpm-surface)"
      >
        <div class="flex items-start justify-between gap-6">
          <div class="min-w-0 flex-1">
            <h3 class="text-xl font-semibold" style="color: var(--bpm-text)">
              {{ addon.title }}
              <span
                v-if="addon.added"
                class="ml-2 align-middle rounded-full px-2 py-0.5 text-xs font-bold uppercase tracking-widest"
                style="
                  background-color: var(--bpm-accent-hex);
                  color: var(--bpm-accent-text);
                "
              >
                Added
              </span>
            </h3>
            <p
              v-if="addon.summary"
              class="mt-1 text-base"
              style="color: var(--bpm-text)"
            >
              {{ addon.summary }}
            </p>
            <p
              v-if="addon.description"
              class="mt-2 whitespace-pre-line text-sm"
              style="color: var(--bpm-muted)"
            >
              {{ addon.description }}
            </p>
          </div>

          <!-- The main button stays mounted while the row changes, so it can
               always take focus back. -->
          <button
            :ref="(el: any) => register(el, { onSelect: () => onPrimary(addon.name) })"
            :data-addon-primary="addon.name"
            :disabled="view(addon).locked && !view(addon).busy"
            class="shrink-0 rounded-xl px-5 py-2.5 text-base font-semibold transition-colors disabled:opacity-50"
            :style="
              view(addon).primary === 'add'
                ? 'background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)'
                : 'background-color: var(--bpm-bg); color: var(--bpm-text)'
            "
            @click="onPrimary(addon.name)"
          >
            {{ primaryLabel(addon) }}
          </button>
        </div>

        <!-- Remove asks first, as rows under the addon -->
        <div
          v-if="view(addon).confirming"
          class="mt-4 flex items-center justify-end gap-3"
        >
          <p class="mr-auto text-sm" style="color: var(--bpm-text)">
            Remove {{ addon.title }}? Anything it holds for you stays.
          </p>
          <button
            :ref="(el: any) => register(el, { onSelect: () => cancel(addon.name) })"
            :data-addon-keep="addon.name"
            :disabled="view(addon).locked"
            class="rounded-xl px-5 py-2.5 text-base font-semibold disabled:opacity-50"
            style="background-color: var(--bpm-bg); color: var(--bpm-text)"
            @click="cancel(addon.name)"
          >
            Keep it
          </button>
          <button
            :ref="(el: any) => register(el, { onSelect: () => confirmRemove(addon.name) })"
            :disabled="view(addon).locked"
            class="rounded-xl bg-red-600 px-5 py-2.5 text-base font-semibold text-white disabled:opacity-50"
            @click="confirmRemove(addon.name)"
          >
            {{ view(addon).busy ? "Removing..." : "Remove" }}
          </button>
        </div>

        <p
          v-if="shop.actionError.value?.name === addon.name"
          class="mt-3 text-right text-sm text-red-300"
        >
          {{
            shop.actionError.value.kind === "add"
              ? "Couldn't add it. Try again."
              : "Couldn't remove it. Try again."
          }}
        </p>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { rowView, type ShopAddon } from "~/composables/addon-shop-logic";
import { useAddonShop } from "~/composables/addon-shop";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

const shop = useAddonShop();
const focusNav = useFocusNavigation();
const register = useBpFocusableGroup("content");
const root = ref<HTMLElement | null>(null);

function view(addon: ShopAddon) {
  return rowView(addon, shop.confirming.value, shop.busy.value);
}

function primaryLabel(addon: ShopAddon) {
  const v = view(addon);
  if (v.busy) return v.primary === "add" ? "Adding..." : "Removing...";
  return v.primary === "add" ? "Add" : "Remove";
}

/** The addon as it is now, by name. */
function current(name: string): ShopAddon | undefined {
  return shop.addons.value.find((a) => a.name === name);
}

/** Focus an element of the named addon's row, found by name, never index. */
async function focusRow(
  attr: "data-addon-primary" | "data-addon-keep",
  name: string,
) {
  await nextTick();
  const el = root.value?.querySelector<HTMLElement>(
    `[${attr}="${CSS.escape(name)}"]`,
  );
  focusNav.focusElement(el);
}

// `disabled` does not stop the controller (the focus system fires onSelect
// regardless), so every handler checks `busy` itself.

async function onPrimary(name: string) {
  const addon = current(name);
  if (!addon || shop.busy.value) return;
  const v = view(addon);
  if (v.primary === "add") {
    await shop.add(name);
  } else if (v.confirming) {
    cancel(name);
  } else {
    shop.askRemove(name);
    // Open on the safe choice, so a second press of A keeps the addon.
    await focusRow("data-addon-keep", name);
  }
}

async function cancel(name: string) {
  if (shop.busy.value) return;
  // Move focus first: "Keep it" is about to unmount.
  await focusRow("data-addon-primary", name);
  shop.cancelRemove();
}

async function confirmRemove(name: string) {
  if (shop.busy.value) return;
  // Move focus to the main button, which stays mounted, before the confirm
  // rows go away; otherwise the focus system jumps to the first element.
  await focusRow("data-addon-primary", name);
  await shop.remove(name);
  // On failure the confirm rows close too; the error shows on the row.
  shop.cancelRemove();
}

async function retry() {
  if (shop.status.value === "loading") return;
  await shop.load();
  await nextTick();
  // The error card that held focus has unmounted. Vue hands the focus
  // system its null ref while the node is still in the document, so the
  // registration (and the focus system's idea of what is focused) can
  // outlive it. Clear that, then land on something real.
  focusNav.clearFocus();
  const first = shop.addons.value[0];
  if (first) {
    await focusRow("data-addon-primary", first.name);
  } else if (shop.status.value === "error") {
    // Failed again: the new error card, so A retries once more.
    focusNav.focusElement(
      root.value?.querySelector<HTMLElement>("[data-addon-error]"),
    );
  } else {
    focusNav.autoFocusContent("content");
  }
}

onMounted(async () => {
  await shop.load();
  // Restored straight onto this tab, the page's own autofocus ran while the
  // list was loading and found nothing. Does nothing if focus is already set.
  focusNav.autoFocusContent("content");
});
</script>
