<template>
  <div
    v-if="panel.status.value === 'error'"
    class="rounded-xl border border-zinc-800 bg-zinc-900 p-4"
  >
    <div class="flex items-center justify-between gap-4">
      <p class="text-sm text-zinc-300">
        Couldn't check this game's league saves.
      </p>
      <button
        class="rounded-md bg-zinc-700 px-3 py-1.5 text-sm font-medium text-zinc-200 hover:bg-zinc-600"
        @click="panel.load()"
      >
        Retry
      </button>
    </div>
  </div>

  <div
    v-else-if="panel.status.value === 'ready' && panel.addons.value.length > 0"
    class="space-y-3"
  >
    <div
      v-for="addon in panel.addons.value"
      :key="addon.name"
      class="rounded-xl border border-zinc-800 bg-zinc-900 p-4"
    >
      <div class="mb-2 flex items-center gap-4">
        <h3 class="text-sm font-medium text-zinc-200">{{ addon.title }}</h3>
        <button
          type="button"
          :disabled="panel.busy.value !== null"
          class="ml-auto text-sm text-blue-400 hover:text-blue-300 disabled:opacity-50"
          @click="panel.load(true)"
        >
          Refresh
        </button>
        <button
          v-if="addon.site"
          type="button"
          class="text-sm text-blue-400 hover:text-blue-300"
          @click="openSite(addon.name, addon.site)"
        >
          Website
        </button>
      </div>
      <p v-if="siteError === addon.name" class="mb-2 text-xs text-red-300">
        Couldn't open the website.
      </p>

      <p v-if="!addon.available" class="text-sm text-zinc-400">
        The league can't be reached right now. Refresh to try again.
      </p>
      <p v-else-if="addon.contexts.length === 0" class="text-sm text-zinc-400">
        You don't have a team in any league yet.
      </p>

      <div v-else class="space-y-2">
        <div
          v-for="ctx in addon.contexts"
          :key="ctx.id"
          class="rounded-lg bg-zinc-800/50 px-3 py-2"
        >
          <div class="flex items-center gap-3">
            <div class="min-w-0 flex-1">
              <p class="text-sm text-zinc-200">{{ ctx.title }}</p>
              <p class="text-xs text-zinc-400">
                {{ view(addon.name, ctx).line }}
              </p>
            </div>
            <button
              v-if="view(addon.name, ctx).secondary"
              type="button"
              :disabled="view(addon.name, ctx).locked"
              class="shrink-0 rounded-md bg-zinc-700 px-3 py-1.5 text-sm font-medium text-zinc-200 hover:bg-zinc-600 disabled:opacity-50"
              @click="panel.checkOut(key(addon.name, ctx.id))"
            >
              {{ actionLabel(view(addon.name, ctx).secondary!, false) }}
            </button>
            <button
              v-if="view(addon.name, ctx).primary"
              type="button"
              :disabled="view(addon.name, ctx).locked"
              class="shrink-0 rounded-md px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
              :class="
                view(addon.name, ctx).urgent
                  ? 'bg-blue-600 ring-2 ring-blue-400 hover:bg-blue-500'
                  : 'bg-blue-600 hover:bg-blue-500'
              "
              @click="onPrimary(addon.name, ctx.id)"
            >
              {{
                actionLabel(
                  view(addon.name, ctx).primary!,
                  view(addon.name, ctx).busy,
                )
              }}
            </button>
          </div>

          <!-- The save here has play never checked in: ask before replacing -->
          <div
            v-if="view(addon.name, ctx).confirming"
            class="mt-2 flex items-center justify-end gap-2"
          >
            <p class="mr-auto text-xs text-zinc-300">
              Replace the save on this device with the league's? A copy is kept.
            </p>
            <button
              type="button"
              :disabled="view(addon.name, ctx).locked"
              class="rounded-md bg-zinc-700 px-3 py-1.5 text-sm font-medium text-zinc-200 hover:bg-zinc-600 disabled:opacity-50"
              @click="panel.cancelReplace()"
            >
              Keep mine
            </button>
            <button
              type="button"
              :disabled="view(addon.name, ctx).locked"
              class="rounded-md bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-500 disabled:opacity-50"
              @click="panel.checkOut(key(addon.name, ctx.id), true)"
            >
              Replace
            </button>
          </div>

          <p
            v-if="panel.notice.value?.key === key(addon.name, ctx.id)"
            class="mt-2 text-xs"
            :class="
              panel.notice.value.kind === 'ok'
                ? 'text-green-400'
                : 'text-red-300'
            "
          >
            {{ panel.notice.value.message }}
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/*
League saves for this game (addons the player added, e.g. the NCAA 14
       dynasty). Renders nothing when there are none. State and actions:
       composables/addon-play.ts; Big Picture has its own copy of this panel
       (components/bigpicture/game-detail/BpmAddonActions.vue).
       Wording is placeholder.
*/
import {
  actionLabel,
  contextView,
  ctxKey,
  type PlayContext,
} from "~/composables/addon-play-logic";
import { useAddonPlay } from "~/composables/addon-play";
import { open as openExternal } from "@tauri-apps/plugin-shell";

const props = defineProps<{ gameId: string; running: boolean }>();

const panel = useAddonPlay(
  () => props.gameId,
  () => props.running,
);

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

function onPrimary(addon: string, ctxId: string) {
  const k = ctxKey(addon, ctxId);
  const found = panel.addons.value
    .find((a) => a.name === addon)
    ?.contexts.find((c) => c.id === ctxId);
  if (!found) return;
  const v = view(addon, found);
  if (v.primary === "check_in") panel.checkIn(k);
  else if (v.primary === "check_out") panel.checkOut(k);
}

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

onMounted(() => panel.load());
watch(
  () => props.gameId,
  () => panel.load(),
);
</script>
