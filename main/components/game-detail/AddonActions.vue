<template>
  <div
    v-if="panel.status.value === 'error'"
    data-league-widget
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
    data-league-widget
    class="space-y-4"
  >
    <div
      v-for="addon in panel.addons.value"
      :key="addon.name"
      class="overflow-hidden rounded-2xl border border-white/10 bg-zinc-950/90 shadow-2xl backdrop-blur-xl"
      :class="{ 'league-scoreboard': addon.name === 'ncaa14' }"
    >
      <div class="league-masthead flex flex-wrap items-center gap-2 border-b border-white/10 px-5 py-4">
        <TrophyIcon class="size-5 text-blue-400" aria-hidden="true" />
        <h3 class="flex-1 text-sm font-semibold text-zinc-200">{{ addon.title }}</h3>
        <button
          type="button"
          :disabled="panel.busy.value !== null"
          class="inline-flex items-center gap-2 rounded-lg p-2 text-xs font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-white disabled:opacity-50"
          aria-label="Refresh"
          title="Refresh"
          @click="panel.load(true)"
        >
          <ArrowPathIcon class="size-4" aria-hidden="true" />
        </button>
        <button
          v-if="addon.site"
          type="button"
          class="inline-flex items-center gap-2 rounded-lg p-2 text-xs font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-white"
          aria-label="Website"
          title="Website"
          @click="openSite(addon.name, addon.site)"
        >
          <ArrowTopRightOnSquareIcon class="size-4" aria-hidden="true" />
        </button>
      </div>
      <p v-if="siteError === addon.name" class="px-5 pt-4 text-xs text-red-300">
        Couldn't open the website.
      </p>

      <p v-if="!addon.available" class="p-5 text-sm text-zinc-400">
        The league can't be reached right now. Refresh to try again.
      </p>
      <p v-else-if="addon.contexts.length === 0" class="p-5 text-sm text-zinc-400">
        You don't have a team in any league yet.
      </p>

      <div v-else class="divide-y divide-white/10">
        <div
          v-for="ctx in addon.contexts"
          :key="ctx.id"
          class="league-body bg-gradient-to-br from-blue-500/[0.08] to-transparent p-6"
        >
          <div class="flex flex-col gap-4">
            <div class="min-w-0">
              <p class="league-title mb-5 text-xl font-semibold text-zinc-100">{{ ctx.title }}</p>
              <div v-if="ctx.week !== null" class="league-week-row mb-6 flex items-center justify-between gap-4">
                <div class="league-week-display">
                  <span class="league-week-label block text-xs font-semibold uppercase tracking-[0.2em] text-zinc-400">Week</span>
                  <span class="league-week-number block text-6xl font-bold tabular-nums leading-none tracking-tight text-zinc-100">{{ ctx.week }}</span>
                </div>
                <svg v-if="addon.name === 'ncaa14'" class="league-field" viewBox="0 0 132 172" fill="none" aria-hidden="true">
                  <rect x="1" y="1" width="130" height="170" rx="6" fill="currentColor" fill-opacity=".035" stroke="currentColor" />
                  <path d="M1 21h130M1 47h130M1 73h130M1 99h130M1 125h130M1 151h130M25 1v170M107 1v170" stroke="currentColor" stroke-opacity=".45" />
                  <path d="M48 34h5m26 0h5M48 60h5m26 0h5M48 86h5m26 0h5M48 112h5m26 0h5M48 138h5m26 0h5" stroke="currentColor" />
                  <path d="m44 116 23-28-8-24 28-25m-12 1 12-1-2 12" stroke="#b87951" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
                  <circle cx="44" cy="116" r="5" stroke="#b87951" stroke-width="2" />
                </svg>
              </div>
              <p class="league-save-status rounded-lg border border-white/5 bg-black/20 px-3 py-3 text-sm leading-relaxed text-zinc-300">
                <span class="mr-1.5 inline-block size-1.5 rounded-full" :class="ctx.state === 'free' ? 'bg-emerald-400' : ctx.state === 'mine' ? 'bg-blue-400' : 'bg-zinc-500'" aria-hidden="true" />
                {{ view(addon.name, ctx).line.replace(` (week ${ctx.week})`, '') }}
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
              class="league-primary shrink-0 rounded-lg px-5 py-3 text-sm font-semibold text-white transition-colors disabled:opacity-50"
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
          <button
            v-if="ctx.state === 'mine' && panel.cancelling.value !== key(addon.name, ctx.id)"
            type="button"
            class="mt-3 w-full rounded-lg px-3 py-2 text-sm text-zinc-400 hover:bg-white/5 hover:text-white disabled:opacity-50"
            :disabled="panel.busy.value !== null || running"
            @click="panel.askCancelTurn(key(addon.name, ctx.id))"
          >Cancel turn</button>
          <div v-if="panel.cancelling.value === key(addon.name, ctx.id)" class="mt-4 space-y-3 border-t border-white/10 pt-4">
            <p class="text-sm text-zinc-300">Return the turn without uploading changes? Your local save is kept.</p>
            <div class="flex flex-wrap gap-2">
              <button type="button" class="rounded-lg bg-zinc-700 px-4 py-2 text-sm text-white" :disabled="panel.busy.value !== null" @click="panel.keepTurn()">Keep turn</button>
              <button type="button" class="rounded-lg bg-zinc-700 px-4 py-2 text-sm text-white" :disabled="panel.busy.value !== null || running" @click="panel.cancelTurn(key(addon.name, ctx.id))">{{ panel.busy.value ? 'Cancelling...' : 'Cancel turn' }}</button>
            </div>
          </div>
          <div
            v-if="view(addon.name, ctx).confirming"
            class="mt-4 flex flex-wrap items-center justify-end gap-3 border-t border-zinc-800 pt-4"
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
import { TrophyIcon, ArrowPathIcon, ArrowTopRightOnSquareIcon } from "@heroicons/vue/24/outline";

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
  if (panel.cancelling.value) return;
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

<style scoped src="~/assets/league-scoreboard.css"></style>
