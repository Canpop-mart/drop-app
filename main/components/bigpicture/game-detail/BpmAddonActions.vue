<template>
  <!-- Big Picture's league-save panel on a game page. Same logic as the
       desktop panel (composables/addon-play.ts), its own instance. Renders
       nothing for games without a league. Every button is registered with
       the focus system; "replace the save here?" asks on plain focusable
       rows, never a modal, so there is no input lock to leak.

       Focus rules, as in the Store's Addons tab:
       - Handlers take the row's KEY and look the row up when they fire:
         useBpFocusableGroup registers an element once and ignores later
         registrations, so a closure over the row would act on stale state.
       - Each row's main button stays mounted whatever the save's state (it
         becomes "Refresh" when there is nothing to do), so focus has
         somewhere to go back to by key, never by index, when rows change.

       No Website button: on a Deck in Game Mode a browser would open behind
       gamescope. Wording is placeholder. -->
  <div ref="root">
    <BigPictureSectionError
      v-if="panel.status.value === 'error'"
      :ref="(el: any) => register(el, { onSelect: retry })"
      data-addon-play-error
      title="Couldn't check league saves"
      @retry="retry"
    />

    <div
      v-else-if="
        panel.status.value === 'ready' && panel.addons.value.length > 0
      "
      class="space-y-3"
    >
      <section
        v-for="addon in panel.addons.value"
        :key="addon.name"
        class="rounded-2xl p-5"
        style="background-color: var(--bpm-surface)"
      >
        <h3 class="mb-2 text-lg font-semibold" style="color: var(--bpm-text)">
          {{ addon.title }}
        </h3>
        <div v-if="!addon.available" class="flex items-center gap-4">
          <p class="flex-1 text-sm" style="color: var(--bpm-muted)">
            The league can't be reached right now.
          </p>
          <button
            :ref="
              (el: any) =>
                register(el, { onSelect: () => retryAddon(addon.name) })
            "
            :data-addon-play-retry="addon.name"
            class="shrink-0 rounded-xl px-5 py-2.5 text-base font-semibold"
            style="background-color: var(--bpm-bg); color: var(--bpm-text)"
            @click="retryAddon(addon.name)"
          >
            Try again
          </button>
        </div>
        <p
          v-else-if="addon.contexts.length === 0"
          class="text-sm"
          style="color: var(--bpm-muted)"
        >
          You don't have a team in any league yet.
        </p>

        <template v-else>
          <div
            v-for="ctx in addon.contexts"
            :key="ctx.id"
            class="mt-2 rounded-xl p-4"
            style="background-color: var(--bpm-bg)"
          >
            <div class="flex items-center gap-4">
              <div class="min-w-0 flex-1">
                <p class="text-base font-medium" style="color: var(--bpm-text)">
                  {{ ctx.title }}
                </p>
                <p class="text-sm" style="color: var(--bpm-muted)">
                  {{ view(addon.name, ctx).line }}
                </p>
              </div>
              <button
                v-if="view(addon.name, ctx).secondary"
                :ref="
                  (el: any) =>
                    register(el, {
                      onSelect: () => onSecondary(key(addon.name, ctx.id)),
                    })
                "
                :disabled="view(addon.name, ctx).locked"
                class="shrink-0 rounded-xl px-5 py-2.5 text-base font-semibold disabled:opacity-50"
                style="
                  background-color: var(--bpm-surface);
                  color: var(--bpm-text);
                "
                @click="onSecondary(key(addon.name, ctx.id))"
              >
                {{ actionLabel(view(addon.name, ctx).secondary!, false) }}
              </button>
              <button
                :ref="
                  (el: any) =>
                    register(el, {
                      onSelect: () => onPrimary(key(addon.name, ctx.id)),
                    })
                "
                :data-addon-play-primary="key(addon.name, ctx.id)"
                :disabled="
                  view(addon.name, ctx).locked && !view(addon.name, ctx).busy
                "
                class="shrink-0 rounded-xl px-5 py-2.5 text-base font-semibold transition-colors disabled:opacity-50"
                :class="
                  view(addon.name, ctx).urgent
                    ? 'ring-4 ring-offset-2 ring-offset-transparent'
                    : ''
                "
                :style="
                  view(addon.name, ctx).primary
                    ? 'background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text); --tw-ring-color: var(--bpm-accent-hex)'
                    : 'background-color: var(--bpm-surface); color: var(--bpm-text)'
                "
                @click="onPrimary(key(addon.name, ctx.id))"
              >
                {{ primaryLabel(addon.name, ctx) }}
              </button>
            </div>

            <div
              v-if="view(addon.name, ctx).confirming"
              class="mt-3 flex items-center justify-end gap-3"
            >
              <p class="mr-auto text-sm" style="color: var(--bpm-text)">
                Replace the save on this device with the league's? A copy is
                kept.
              </p>
              <button
                :ref="
                  (el: any) =>
                    register(el, {
                      onSelect: () => keepMine(key(addon.name, ctx.id)),
                    })
                "
                :data-addon-play-keep="key(addon.name, ctx.id)"
                :disabled="view(addon.name, ctx).locked"
                class="rounded-xl px-5 py-2.5 text-base font-semibold disabled:opacity-50"
                style="
                  background-color: var(--bpm-surface);
                  color: var(--bpm-text);
                "
                @click="keepMine(key(addon.name, ctx.id))"
              >
                Keep mine
              </button>
              <button
                :ref="
                  (el: any) =>
                    register(el, {
                      onSelect: () => replace(key(addon.name, ctx.id)),
                    })
                "
                :disabled="view(addon.name, ctx).locked"
                class="rounded-xl bg-red-600 px-5 py-2.5 text-base font-semibold text-white disabled:opacity-50"
                @click="replace(key(addon.name, ctx.id))"
              >
                Replace
              </button>
            </div>

            <p
              v-if="panel.notice.value?.key === key(addon.name, ctx.id)"
              class="mt-3 text-sm"
              :class="
                panel.notice.value.kind === 'ok'
                  ? 'text-green-300'
                  : 'text-red-300'
              "
            >
              {{ panel.notice.value.message }}
            </p>
          </div>
        </template>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  actionLabel,
  contextView,
  ctxKey,
  type PlayContext,
} from "~/composables/addon-play-logic";
import { useAddonPlay } from "~/composables/addon-play";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

const props = defineProps<{ gameId: string; running: boolean }>();

const panel = useAddonPlay(
  () => props.gameId,
  () => props.running,
);
const focusNav = useFocusNavigation();
const register = useBpFocusableGroup("content");
const root = ref<HTMLElement | null>(null);
const key = ctxKey;

function view(addon: string, ctx: PlayContext) {
  return contextView(
    addon,
    ctx,
    panel.local.value[ctxKey(addon, ctx.id)] ?? null,
    {
      confirming: panel.confirming.value,
      busy: panel.busy.value,
      running: props.running,
    },
  );
}

/** The row as it is now, by key. */
function current(k: string) {
  for (const addon of panel.addons.value) {
    for (const ctx of addon.contexts) {
      if (ctxKey(addon.name, ctx.id) === k)
        return { addon: addon.name, ctx, v: view(addon.name, ctx) };
    }
  }
  return null;
}

function primaryLabel(addon: string, ctx: PlayContext) {
  const v = view(addon, ctx);
  return v.primary ? actionLabel(v.primary, v.busy) : "Refresh";
}

async function focusPrimary(k: string) {
  await nextTick();
  const el = root.value?.querySelector<HTMLElement>(
    `[data-addon-play-primary="${CSS.escape(k)}"]`,
  );
  if (el) focusNav.focusElement(el);
}

// `disabled` doesn't stop the controller (the focus system fires onSelect
// regardless), so every handler checks `busy` itself.

async function onPrimary(k: string) {
  const row = current(k);
  if (!row || panel.busy.value) return;
  if (row.v.primary === "check_in") await panel.checkIn(k);
  else if (row.v.primary === "check_out") await panel.checkOut(k);
  else await panel.load(true);
  await settle(k);
}

async function onSecondary(k: string) {
  const row = current(k);
  if (!row || panel.busy.value || row.v.secondary !== "replace") return;
  // Focus first: the secondary button may go away with the new state.
  await focusPrimary(k);
  await panel.checkOut(k);
  await settle(k);
}

async function keepMine(k: string) {
  if (panel.busy.value) return;
  // Move focus before "Keep mine" unmounts.
  await focusPrimary(k);
  panel.cancelReplace();
}

async function replace(k: string) {
  if (panel.busy.value) return;
  await focusPrimary(k);
  await panel.checkOut(k, true);
  await settle(k);
}

/**
 * After an action, rows may have come or gone (the league can change under
 * us). Land on this row's main button if it is still here, or on the
 * "keep mine" choice if the replace question just opened.
 */
async function settle(k: string) {
  await nextTick();
  if (panel.confirming.value === k) {
    const keep = root.value?.querySelector<HTMLElement>(
      `[data-addon-play-keep="${CSS.escape(k)}"]`,
    );
    if (keep) {
      focusNav.focusElement(keep);
      return;
    }
  }
  if (current(k)) {
    await focusPrimary(k);
    return;
  }
  // The row is gone; its button's registration may outlive it.
  focusNav.clearFocus();
  focusNav.autoFocusContent("content");
}

/** "Try again" on a league that couldn't be reached. */
async function retryAddon(name: string) {
  if (panel.busy.value || panel.status.value === "loading") return;
  await panel.load(true);
  await nextTick();
  const addon = panel.addons.value.find((a) => a.name === name);
  const first = addon?.contexts[0];
  if (addon && first) {
    focusNav.clearFocus();
    await focusPrimary(ctxKey(name, first.id));
    return;
  }
  const again = root.value?.querySelector<HTMLElement>(
    `[data-addon-play-retry="${CSS.escape(name)}"]`,
  );
  if (again) {
    focusNav.focusElement(again);
    return;
  }
  focusNav.clearFocus();
  focusNav.autoFocusContent("content");
}

async function retry() {
  if (panel.status.value === "loading") return;
  await panel.load();
  await nextTick();
  // The error card that held focus has unmounted; see BpmStoreAddons.vue.
  focusNav.clearFocus();
  if (panel.status.value === "error") {
    focusNav.focusElement(
      root.value?.querySelector<HTMLElement>("[data-addon-play-error]"),
    );
    return;
  }
  const first = panel.addons.value.flatMap((a) =>
    a.contexts.map((c) => ctxKey(a.name, c.id)),
  )[0];
  if (first) await focusPrimary(first);
  else focusNav.autoFocusContent("content");
}

onMounted(() => panel.load());
watch(
  () => props.gameId,
  () => panel.load(),
);
</script>
