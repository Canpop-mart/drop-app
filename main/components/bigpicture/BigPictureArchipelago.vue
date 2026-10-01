<template>
  <div ref="root" class="space-y-4">
    <div
      v-if="error"
      class="px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
    >
      {{ error }}
    </div>
    <div
      v-if="notice"
      class="px-4 py-3 rounded-lg bg-green-900/25 border border-green-500/30 text-green-200 text-sm"
    >
      {{ notice }}
    </div>

    <!-- Failures, each with its own retry (plain focusable rows, no modal) -->
    <div
      v-if="leaveError"
      class="flex flex-wrap items-center gap-3 px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
    >
      <span class="flex-1 min-w-0">
        Couldn't leave the session, you're still in it: {{ leaveError }}
        <span class="block text-xs text-red-200/70 mt-1">
          If the server is gone for good, you can forget the session on this
          device instead. The server may still list you in it.
        </span>
      </span>
      <button
        :ref="(el: any) => register(el, { onSelect: doLeave })"
        :disabled="busy"
        class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
        @click="doLeave"
      >
        Retry
      </button>
      <button
        :ref="(el: any) => register(el, { onSelect: forget })"
        :disabled="busy"
        class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-50"
        @click="forget"
      >
        Forget this session
      </button>
    </div>
    <div
      v-if="detail && reconnectNeeded && !reconnectError"
      class="flex flex-wrap items-center gap-3 px-4 py-3 rounded-lg bg-zinc-900/60 border border-zinc-700 text-zinc-300 text-sm"
    >
      <span class="flex-1 min-w-0">
        This device isn't connected to the session's network yet.
      </span>
      <button
        :ref="(el: any) => register(el, { onSelect: onReconnect })"
        :disabled="busy"
        class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
        @click="onReconnect"
      >
        Reconnect
      </button>
    </div>
    <div
      v-if="detail && desktopSetupNeeded && !reconnectError"
      class="px-4 py-3 rounded-lg bg-zinc-900/60 border border-zinc-700 text-zinc-300 text-sm"
    >
      {{ AP_DESKTOP_SETUP_MSG }}
    </div>
    <div
      v-if="detail && reconnectError"
      class="flex flex-wrap items-center gap-3 px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
    >
      <span class="flex-1 min-w-0">
        Couldn't reconnect this device to the Archipelago network: {{ reconnectError }}
      </span>
      <button
        :ref="(el: any) => register(el, { onSelect: onReconnect })"
        :disabled="busy"
        class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
        @click="onReconnect"
      >
        Retry
      </button>
    </div>
    <div
      v-if="detail && fetchError"
      class="flex flex-wrap items-center gap-3 px-4 py-3 rounded-lg bg-amber-900/20 border border-amber-500/30 text-amber-200 text-sm"
    >
      <span class="flex-1 min-w-0">
        Couldn't refresh the session, showing the last update: {{ fetchError }}
      </span>
      <button
        :ref="(el: any) => register(el, { onSelect: onRefresh })"
        class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600"
        @click="onRefresh"
      >
        Retry
      </button>
    </div>

    <!-- Session closed (by the host, or by the server after a long idle setup) -->
    <div
      v-if="sessionEnded"
      class="rounded-xl bg-zinc-900/60 border border-zinc-700 p-6 text-center"
    >
      <p class="text-lg font-medium text-zinc-200 mb-1">Session closed</p>
      <p class="text-sm text-zinc-500 mb-4">
        This multiworld was closed. You can join another anytime.
      </p>
      <button
        :ref="(el: any) => { els.ended = el; register(el, { onSelect: onDismissEnded }); }"
        class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600"
        @click="onDismissEnded"
      >
        OK
      </button>
    </div>

    <!-- In a session -->
    <template v-else-if="detail">
      <div class="flex items-center gap-4 rounded-xl bg-zinc-900/60 p-4">
        <div class="min-w-0 flex-1">
          <p class="text-xs text-zinc-500 mb-1">
            {{ detail.name || "Session" }}
          </p>
          <button
            :ref="(el: any) => { els.code = el; register(el, { onSelect: copyCode }); }"
            class="group inline-flex items-center gap-2"
            @click="copyCode"
          >
            <span class="font-mono text-2xl font-bold tracking-widest text-purple-300">
              {{ displayCode || "…" }}
            </span>
            <span
              class="text-xs font-medium"
              :class="codeCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
            >
              {{ codeCopied ? "Copied" : "Copy" }}
            </span>
          </button>
        </div>
        <span
          class="shrink-0 inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-medium"
          :class="detail.allReady ? 'bg-green-900/25 text-green-300' : 'bg-zinc-800 text-zinc-400'"
        >
          {{ detail.readyCount }} / {{ detail.totalCount }} ready
        </span>
      </div>

      <div class="rounded-xl bg-zinc-900/60 p-4 border border-purple-500/20">
        <p class="text-xs uppercase tracking-wide text-zinc-500 mb-1.5">
          Connect address
        </p>
        <template v-if="detail.connectAddress">
          <button
            :ref="(el: any) => register(el, { onSelect: copyConnect })"
            class="group inline-flex items-center gap-2"
            @click="copyConnect"
          >
            <span class="font-mono text-lg font-bold text-purple-300">
              {{ detail.connectAddress }}
            </span>
            <span
              class="text-xs font-medium"
              :class="connectCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
            >
              {{ connectCopied ? "Copied" : "Copy" }}
            </span>
          </button>
          <p class="text-xs text-zinc-600 mt-1.5">
            Enter this in your game's Archipelago client.
          </p>
        </template>
        <p v-else-if="!detail.isHost" class="text-sm text-zinc-500">
          The host will share it here once the multiworld is generated.
        </p>
        <p v-else class="text-sm text-zinc-500">
          After generating, enter the connect line from the Archipelago room
          page so everyone gets it.
        </p>
        <button
          v-if="detail.isHost"
          :ref="(el: any) => register(el, { onSelect: openConnectKeyboard })"
          :disabled="busy"
          class="mt-3 px-4 py-2 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
          @click="openConnectKeyboard"
        >
          {{ detail.connectAddress ? "Change connect address" : "Enter connect address" }}
        </button>
      </div>

      <!-- Players (display only, nothing to select) -->
      <div class="rounded-xl bg-zinc-900/60 p-2">
        <div
          v-for="s in detail.slots"
          :key="s.clientId"
          class="flex items-center gap-3 px-3 py-2.5"
        >
          <span
            class="size-2 shrink-0 rounded-full"
            :class="s.hasYaml ? 'bg-green-400' : 'bg-zinc-600'"
          />
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <span class="text-sm text-zinc-200 truncate">{{ s.clientName }}</span>
              <span
                v-if="s.isHost"
                class="shrink-0 text-[11px] px-2 py-0.5 rounded-full bg-purple-600/20 text-purple-300"
              >
                Host
              </span>
            </div>
            <p v-if="s.slotName || s.game" class="text-xs text-zinc-500 truncate">
              {{ s.slotName }}<span v-if="s.game"> · {{ s.game }}</span>
            </p>
          </div>
          <span
            class="shrink-0 text-xs font-medium"
            :class="s.hasYaml ? 'text-green-400' : 'text-zinc-500'"
          >
            {{ s.hasYaml ? "Ready" : "No settings" }}
          </span>
        </div>
      </div>

      <!--
        Dual-Surface gap, accepted: uploading a YAML and saving everyone's
        settings need a file picker, which a controller can't drive. On a Deck
        in Game Mode the Exit Big Picture item is hidden, so the copy says
        plainly that this needs the desktop layout (Desktop Mode on a Deck).
      -->
      <p class="text-xs text-zinc-500">
        {{
          detail.isHost
            ? "Uploading settings and saving everyone's settings need the desktop layout. On a Steam Deck, switch to Desktop Mode for that."
            : "Uploading your settings needs the desktop layout. On a Steam Deck, switch to Desktop Mode for that."
        }}
      </p>

      <div v-if="!confirmingLeave">
        <button
          :ref="(el: any) => { els.leave = el; register(el, { onSelect: askLeave }); }"
          :disabled="busy"
          class="px-5 py-2.5 rounded-lg text-sm font-medium bg-red-900/40 text-red-200 hover:bg-red-900/60 disabled:opacity-50"
          @click="askLeave"
        >
          {{ detail.isHost ? "Close session" : "Leave session" }}
        </button>
      </div>
      <div
        v-else
        class="flex flex-wrap items-center gap-3 rounded-lg bg-zinc-900/60 px-4 py-3"
      >
        <span class="text-sm text-zinc-300">
          {{ detail.isHost ? "Close this session for everyone?" : "Leave this session?" }}
        </span>
        <div class="flex gap-2 ml-auto">
          <button
            :ref="(el: any) => { els.confirm = el; register(el, { onSelect: doLeave }); }"
            :disabled="busy"
            class="px-3 py-1.5 rounded-md text-sm font-medium bg-red-700 text-white hover:bg-red-600 disabled:opacity-50"
            @click="doLeave"
          >
            {{ detail.isHost ? "Close it" : "Leave" }}
          </button>
          <button
            :ref="(el: any) => register(el, { onSelect: cancelLeave })"
            class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-200 hover:bg-zinc-600"
            @click="cancelLeave"
          >
            Cancel
          </button>
        </div>
      </div>
    </template>

    <!--
      Checking the server for a session from before a restart. Focusable (it
      does nothing when pressed) so focus has somewhere to land when the
      control that held it, e.g. a Retry, disappears into this state.
    -->
    <button
      v-else-if="restoring"
      :ref="(el: any) => { els.checking = el; register(el, { onSelect: () => {} }); }"
      class="block text-left px-4 py-2.5 rounded-lg text-sm text-zinc-500"
    >
      Checking for an open session…
    </button>

    <!-- Not in a session -->
    <template v-else>
      <div
        v-if="restoreError"
        class="flex flex-wrap items-center gap-3 px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
      >
        <span class="flex-1 min-w-0">
          Couldn't check whether you're in a session: {{ restoreError }}
        </span>
        <button
          :ref="(el: any) => { els.restoreRetry = el; register(el, { onSelect: restore }); }"
          class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600"
          @click="restore"
        >
          Retry
        </button>
      </div>
      <div class="rounded-xl bg-zinc-900/60 p-6">
        <h2 class="text-lg font-medium text-zinc-200 mb-1">Join a session</h2>
        <p class="text-sm text-zinc-500 mb-4">
          Enter the code the host shared with you.
        </p>
        <div class="flex items-center gap-3">
          <button
            :ref="(el: any) => { els.joinCode = el; register(el, { onSelect: openKeyboard }); }"
            class="flex-1 px-4 py-2.5 rounded-lg bg-zinc-800 text-left text-lg font-mono tracking-widest uppercase hover:ring-2 hover:ring-purple-500/50"
            @click="openKeyboard"
          >
            <span v-if="joinCode" class="text-zinc-100">{{ joinCode }}</span>
            <span v-else class="text-zinc-600">ABC-123</span>
          </button>
          <button
            :ref="(el: any) => register(el, { onSelect: onJoin })"
            :disabled="busy || joinCode.trim().length === 0"
            class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
            @click="onJoin"
          >
            {{ busy ? "Joining…" : "Join" }}
          </button>
        </div>
      </div>
      <p class="text-xs text-zinc-500">
        Starting a new session needs the desktop layout. On a Steam Deck,
        switch to Desktop Mode for that.
      </p>
    </template>

    <!--
      The only overlay. BigPictureKeyboard takes the input lock when `visible`
      turns true and releases it when it turns false or when it unmounts while
      open; every path here that hides it just sets showKeyboard = false.
    -->
    <BigPictureKeyboard
      :visible="showKeyboard"
      :model-value="kbTarget === 'connect' ? connectDraft : joinCode"
      :placeholder="kbTarget === 'connect' ? '10.243.0.1:38281' : 'ABC-123'"
      :extra-keys="kbTarget === 'connect' ? CONNECT_KEYS : undefined"
      @update:model-value="onKeyboardInput"
      @close="showKeyboard = false"
      @submit="onKeyboardSubmit"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Archipelago in Big Picture: join by code (on-screen keyboard), see the
 * players and the connect address, copy them, set the connect address as the
 * host (on-screen keyboard), and leave or close. Starting a session, uploading
 * a YAML and saving the bundle need a file picker, so they stay on the desktop
 * panel (components/ArchipelagoPanel.vue).
 *
 * Every control is registered with the focus system. There is no modal: the
 * leave confirmation is an inline row, and the only overlay is
 * BigPictureKeyboard, which takes and releases its own input lock.
 */
import { useArchipelago } from "~/composables/archipelago";
import { AP_DESKTOP_SETUP_MSG } from "~/composables/archipelago-logic";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";

const {
  detail,
  busy,
  error,
  notice,
  sessionEnded,
  codeCopied,
  connectCopied,
  fetchError,
  restoreError,
  restoring,
  reconnectError,
  reconnectNeeded,
  desktopSetupNeeded,
  leaveError,
  displayCode,
  refresh,
  startPolling,
  stopPolling,
  copyCode,
  copyConnect,
  join,
  setConnect,
  leave,
  forget,
  reconnect,
  restore,
  dismissSessionEnded,
} = useArchipelago();

const register = useBpFocusableGroup("content");
const focusNav = useFocusNavigation();

const root = ref<HTMLElement | null>(null);
const joinCode = ref("");
const connectDraft = ref("");
const showKeyboard = ref(false);
/** What the one keyboard instance is typing into. */
const kbTarget = ref<"code" | "connect">("code");
const confirmingLeave = ref(false);

// The letter layout has no "." or ":", which every connect address needs.
const CONNECT_KEYS = [".", ":", "-"];

// The control that held focus disappears in most of these handlers and state
// swaps; land on the next logical control rather than letting focus fall
// into nothing.
const els: {
  code: HTMLElement | null;
  joinCode: HTMLElement | null;
  leave: HTMLElement | null;
  confirm: HTMLElement | null;
  ended: HTMLElement | null;
  checking: HTMLElement | null;
  restoreRetry: HTMLElement | null;
} = {
  code: null,
  joinCode: null,
  leave: null,
  confirm: null,
  ended: null,
  checking: null,
  restoreRetry: null,
};
function focusAfterRender(target: () => HTMLElement | null) {
  nextTick(() => {
    const el = target();
    if (el?.isConnected) focusNav.focusElement(el);
  });
}

// Which of the four views is up. They swap on their own too (the host closes
// the session, a poll finds it gone, a restore finishes), not only from a
// button here.
const viewState = computed(() =>
  sessionEnded.value
    ? "ended"
    : detail.value
      ? "session"
      : restoring.value
        ? "checking"
        : "idle",
);
watch(viewState, (state) => {
  // A keyboard or confirmation opened for the old view must not outlive it.
  showKeyboard.value = false;
  confirmingLeave.value = false;
  // Runs before the swap renders, so the old control is still connected here.
  // Move focus only if it was in this view (that control is about to vanish)
  // or nowhere; leave it alone on the page's tabs.
  // (currentFocused is exposed deep-readonly; the element itself is a plain node.)
  const current = focusNav.currentFocused.value?.el as HTMLElement | undefined;
  if (current?.isConnected && !root.value?.contains(current)) return;
  focusAfterRender(() => primaryFor(state));
});

/** Where focus lands in each view. */
function primaryFor(state: string): HTMLElement | null {
  if (state === "ended") return els.ended;
  if (state === "session") return els.code;
  if (state === "checking") return els.checking;
  return restoreError.value ? els.restoreRetry : els.joinCode;
}

/**
 * For actions whose button can vanish without the view changing (a Retry or
 * Reconnect row that goes away on success): if focus is left on a removed
 * element, move it to the view's main control.
 */
function refocusIfLost() {
  nextTick(() => {
    const current = focusNav.currentFocused.value?.el as HTMLElement | undefined;
    if (current?.isConnected) return;
    const el = primaryFor(viewState.value);
    if (el?.isConnected) focusNav.focusElement(el);
  });
}
async function onReconnect() {
  await reconnect();
  refocusIfLost();
}
async function onRefresh() {
  await refresh();
  refocusIfLost();
}

function openKeyboard() {
  kbTarget.value = "code";
  showKeyboard.value = true;
}
function openConnectKeyboard() {
  kbTarget.value = "connect";
  showKeyboard.value = true;
}
function onKeyboardInput(value: string) {
  if (kbTarget.value === "connect") connectDraft.value = value;
  else joinCode.value = value;
}
function onKeyboardSubmit() {
  showKeyboard.value = false;
  if (kbTarget.value === "connect") onSubmitConnect();
  else onJoin();
}
async function onJoin() {
  // Focus moves with the view swap (see the viewState watcher).
  if (await join(joinCode.value)) joinCode.value = "";
}
async function onSubmitConnect() {
  // Keep the draft unless it saved, so a retry doesn't mean retyping it with
  // the on-screen keyboard; setConnect puts the reason in `error`.
  if (await setConnect(connectDraft.value)) connectDraft.value = "";
}
function askLeave() {
  confirmingLeave.value = true;
  focusAfterRender(() => els.confirm);
}
function cancelLeave() {
  confirmingLeave.value = false;
  focusAfterRender(() => els.leave);
}
async function doLeave() {
  confirmingLeave.value = false;
  await leave();
  // Still in it (the server didn't record the leave): back to the Leave
  // button. A recorded leave swaps the view, and the watcher moves focus.
  if (detail.value) focusAfterRender(() => els.leave);
}
function onDismissEnded() {
  // The view swaps to "idle"; the watcher moves focus.
  dismissSessionEnded();
}

onMounted(() => {
  if (detail.value) {
    refresh();
    startPolling();
  } else {
    restore();
  }
});
onUnmounted(() => {
  stopPolling();
});
</script>
