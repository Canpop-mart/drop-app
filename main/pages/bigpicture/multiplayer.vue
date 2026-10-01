<template>
  <div
    class="px-8 py-6"
    :style="{
      backgroundColor: 'var(--bpm-bg)',
      color: 'var(--bpm-text)',
      minHeight: '100%',
    }"
  >
    <div class="max-w-2xl mx-auto">
      <div class="flex items-center gap-3 mb-2">
        <UserGroupIcon
          class="size-7"
          :class="tab === 'coop' ? 'text-blue-400' : 'text-purple-400'"
        />
        <h1 class="text-2xl font-semibold font-display text-zinc-100">
          {{ tab === "coop" ? "Co-op Rooms" : "Archipelago" }}
        </h1>
        <span
          v-if="tab === 'coop' && room && members.length"
          class="text-sm text-zinc-500 font-medium"
        >
          · {{ members.length }} {{ members.length === 1 ? "player" : "players" }}
        </span>
      </div>
      <p class="text-sm text-zinc-500 mb-5">
        {{
          tab === "coop"
            ? "Put friends on a private virtual LAN so LAN / co-op games discover each other across the internet."
            : "Join a multiworld and see where to connect. Starting one and uploading settings need the desktop layout (Desktop Mode on a Steam Deck)."
        }}
      </p>

      <div class="flex gap-1 mb-6 p-1 rounded-lg bg-zinc-900/60 w-fit">
        <button
          v-for="t in tabs"
          :key="t.id"
          :ref="(el: any) => registerAction(el, { onSelect: () => (tab = t.id) })"
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
          :class="
            tab === t.id
              ? 'bg-zinc-700 text-zinc-100'
              : 'text-zinc-400 hover:text-zinc-200'
          "
          @click="tab = t.id"
        >
          {{ t.label }}
        </button>
      </div>

      <!--
        Join, leave and the connect address work here; uploading a YAML needs a
        file picker, so that and starting a session stay on the desktop layout
        (an accepted Dual-Surface gap, see BigPictureArchipelago.vue).
      -->
      <BigPictureArchipelago v-if="tab === 'archipelago'" />

      <template v-else>
      <div
        v-if="error"
        class="mb-4 px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
      >
        {{ error }}
      </div>

      <!--
        Restart recovery: the server says we're still in a room. A plain row of
        focusable buttons, not a modal, so there's no input lock to leak.
      -->
      <div
        v-if="pendingRoom && !room"
        class="mb-4 flex flex-wrap items-center gap-3 rounded-lg bg-blue-900/20 border border-blue-500/30 px-4 py-3"
      >
        <span class="text-sm text-blue-100 flex-1 min-w-0">
          You're still in a co-op room{{
            pendingRoom.gameName ? ` for ${pendingRoom.gameName}` : ""
          }}
          ({{ formatRoomCode(pendingRoom.shortCode) }}).
        </span>
        <button
          :ref="(el: any) => registerAction(el, { onSelect: onRejoin })"
          :disabled="busy"
          class="px-3 py-1.5 rounded-md text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-50"
          @click="onRejoin"
        >
          {{ busy ? "Rejoining…" : "Rejoin" }}
        </button>
        <button
          :ref="(el: any) => registerAction(el, { onSelect: leavePending })"
          :disabled="busy"
          class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-200 hover:bg-zinc-600 disabled:opacity-50"
          @click="leavePending"
        >
          {{ pendingRoom.isHost ? "End it" : "Leave" }}
        </button>
      </div>
      <div
        v-else-if="pendingError && !room"
        class="mb-4 flex flex-wrap items-center gap-3 rounded-lg bg-zinc-900/60 px-4 py-3 text-sm text-zinc-400"
      >
        <span class="flex-1 min-w-0">
          Couldn't check whether you're still in a room: {{ pendingError }}
        </span>
        <button
          :ref="(el: any) => registerAction(el, { onSelect: checkMine })"
          class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-200 hover:bg-zinc-600"
          @click="checkMine"
        >
          Retry
        </button>
      </div>

      <!-- The room ended underneath us (host ended it, it expired) -->
      <div
        v-if="sessionEnded"
        class="rounded-xl bg-zinc-900/60 border border-zinc-700 p-6 text-center"
      >
        <p class="text-lg font-medium text-zinc-200 mb-1">Session ended</p>
        <p class="text-sm text-zinc-500 mb-4">
          The room has ended. You can host or join another anytime.
        </p>
        <button
          :ref="(el: any) => registerAction(el, { onSelect: dismissSessionEnded })"
          class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600"
          @click="dismissSessionEnded"
        >
          OK
        </button>
      </div>

      <div
        v-else-if="!room && unavailable"
        class="px-4 py-3 rounded-lg bg-amber-900/20 border border-amber-500/30 text-amber-200 text-sm"
      >
        {{ unavailable }}
      </div>
      <div
        v-else-if="!room && statusError"
        class="flex flex-wrap items-center gap-3 px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
      >
        <span class="flex-1 min-w-0">
          Couldn't check ZeroTier on this device: {{ statusError }}
        </span>
        <button
          :ref="(el: any) => registerAction(el, { onSelect: loadStatus })"
          class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600"
          @click="loadStatus"
        >
          Retry
        </button>
      </div>

      <!-- In a room -->
      <div v-else-if="room" class="space-y-5">
        <div class="rounded-xl bg-zinc-900/60 p-6">
          <p class="text-xs uppercase tracking-wide text-zinc-500 mb-2">
            {{ isHost ? "Room code (share with friends)" : "Room code" }}
          </p>
          <button
            :ref="(el: any) => { els.code = el; registerAction(el, { onSelect: copyCode }); }"
            class="group inline-flex items-center gap-3"
            @click="copyCode"
          >
            <span
              class="text-3xl font-mono font-bold tracking-widest text-blue-300"
            >
              {{ displayCode || "…" }}
            </span>
            <span
              class="text-xs font-medium"
              :class="codeCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
            >
              {{ codeCopied ? "✓ Copied!" : "Copy" }}
            </span>
          </button>
          <p v-if="roomGameName || room.name" class="text-sm text-zinc-400 mt-2">
            {{ roomGameName || room.name }}
          </p>
        </div>

        <div>
          <p class="text-sm font-medium text-zinc-400 mb-2">In this room</p>
          <div class="space-y-2">
            <div
              v-for="m in members"
              :key="m.clientId"
              class="flex items-center justify-between rounded-lg bg-zinc-900/40 px-4 py-3"
            >
              <span class="text-zinc-200">{{ m.clientName }}</span>
              <span
                v-if="m.isHost"
                class="text-xs px-2 py-0.5 rounded bg-blue-600/20 text-blue-300"
              >
                Host
              </span>
            </div>
            <p v-if="members.length === 0" class="text-sm text-zinc-600 px-1">
              Waiting for the member list…
            </p>
          </div>
        </div>

        <!-- Leave (with confirmation) -->
        <div v-if="!confirmingLeave">
          <button
            :ref="(el: any) => registerAction(el, { onSelect: () => (confirmingLeave = true) })"
            :disabled="busy"
            class="px-5 py-2.5 rounded-lg text-sm font-medium bg-red-900/40 text-red-200 hover:bg-red-900/60 disabled:opacity-50"
            @click="confirmingLeave = true"
          >
            {{ isHost ? "End session" : "Leave room" }}
          </button>
        </div>
        <div
          v-else
          class="flex items-center gap-3 rounded-lg bg-zinc-900/60 px-4 py-3 flex-wrap"
        >
          <span class="text-sm text-zinc-300">
            {{ isHost ? "End the session for everyone?" : "Leave this room?" }}
          </span>
          <div class="flex gap-2 ml-auto">
            <button
              :ref="(el: any) => registerAction(el, { onSelect: doLeave })"
              :disabled="busy"
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-red-700 text-white hover:bg-red-600 disabled:opacity-50"
              @click="doLeave"
            >
              {{ isHost ? "End it" : "Leave" }}
            </button>
            <button
              :ref="(el: any) => registerAction(el, { onSelect: () => (confirmingLeave = false) })"
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-200 hover:bg-zinc-600"
              @click="confirmingLeave = false"
            >
              Cancel
            </button>
          </div>
        </div>

        <!-- Connect address — what every player enters in the game's join-by-IP -->
        <div class="rounded-xl bg-zinc-900/60 p-5">
          <p class="text-xs uppercase tracking-wide text-zinc-500 mb-2">
            Connect address
          </p>
          <button
            v-if="hostIp"
            :ref="(el: any) => registerAction(el, { onSelect: copyHostIp })"
            class="group inline-flex items-center gap-3"
            @click="copyHostIp"
          >
            <span class="text-xl font-mono font-bold text-blue-300">{{
              hostIp
            }}</span>
            <span
              class="text-xs font-medium"
              :class="hostIpCopied ? 'text-green-400' : 'text-zinc-500 group-hover:text-zinc-300'"
            >
              {{ hostIpCopied ? "✓ Copied!" : "Copy" }}
            </span>
          </button>
          <p v-else class="text-sm text-zinc-500">Waiting for the host's address…</p>
          <p class="text-xs text-zinc-600 mt-2">
            If the game lists LAN games, the host should show up there. If it
            doesn't, choose join by IP or direct connect in the game and enter
            this address.
          </p>
        </div>
      </div>

      <!-- Not in a room -->
      <div v-else class="space-y-6">
        <div class="rounded-xl bg-zinc-900/60 p-6">
          <h2 class="text-lg font-medium text-zinc-200 mb-1">Host a room</h2>
          <p class="text-sm text-zinc-500 mb-4">
            Create a room and share the code with friends.
          </p>
          <!--
            Optional game: search with the on-screen keyboard, then pick from at
            most GAME_RESULTS_MAX rows (spatial, not a scrolling list). Rows are
            keyed by game id so focus stays on the same game if results change.
          -->
          <div class="flex flex-wrap items-center gap-3 mb-3">
            <span class="text-sm text-zinc-400">Game:</span>
            <button
              :ref="(el: any) => registerAction(el, { onSelect: openGameSearch })"
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-800 text-zinc-200 hover:bg-zinc-700"
              @click="openGameSearch"
            >
              {{ hostGame ? hostGame.name : "Choose a game (optional)" }}
            </button>
            <button
              v-if="hostGame"
              :ref="(el: any) => registerAction(el, { onSelect: clearHostGame })"
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
              @click="clearHostGame"
            >
              Clear
            </button>
          </div>
          <div v-if="gameSearching" class="mb-3 text-sm text-zinc-500">
            Searching…
          </div>
          <div
            v-else-if="gameSearchError"
            class="mb-3 flex flex-wrap items-center gap-3 rounded-lg bg-red-900/30 border border-red-500/30 px-3 py-2 text-sm text-red-200"
          >
            <span class="flex-1 min-w-0">Search failed: {{ gameSearchError }}</span>
            <button
              :ref="(el: any) => registerAction(el, { onSelect: () => runGameSearch(gameQuery) })"
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600"
              @click="runGameSearch(gameQuery)"
            >
              Retry
            </button>
          </div>
          <div
            v-else-if="gameResults.length"
            class="mb-3 grid grid-cols-2 gap-2"
          >
            <button
              v-for="g in gameResults"
              :key="g.id"
              :ref="(el: any) => registerAction(el, { onSelect: () => pickGame(g) })"
              class="truncate rounded-md bg-zinc-800 px-3 py-2 text-left text-sm text-zinc-200 hover:bg-zinc-700"
              @click="pickGame(g)"
            >
              {{ g.mName }}
            </button>
          </div>
          <p
            v-else-if="gameSearched"
            class="mb-3 text-sm text-zinc-500"
          >
            No games found.
          </p>
          <button
            :ref="(el: any) => { els.host = el; registerAction(el, { onSelect: host }); }"
            :disabled="busy"
            class="px-5 py-2.5 rounded-lg text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-50"
            @click="host"
          >
            {{ busy ? "Setting up…" : "Host a room" }}
          </button>
        </div>

        <div class="rounded-xl bg-zinc-900/60 p-6">
          <h2 class="text-lg font-medium text-zinc-200 mb-1">Join a room</h2>
          <p class="text-sm text-zinc-500 mb-4">
            Enter the code a friend shared with you.
          </p>
          <div class="flex items-center gap-3">
            <button
              :ref="(el: any) => registerAction(el, { onSelect: () => (showKeyboard = true) })"
              class="flex-1 px-4 py-2.5 rounded-lg bg-zinc-800 text-left text-lg font-mono tracking-widest uppercase hover:ring-2 hover:ring-blue-500/50"
              @click="showKeyboard = true"
            >
              <span v-if="joinCode" class="text-zinc-100">{{ joinCode }}</span>
              <span v-else class="text-zinc-600">ABC-123</span>
            </button>
            <button
              :ref="(el: any) => registerAction(el, { onSelect: onJoin })"
              :disabled="busy || joinCode.trim().length === 0"
              class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
              @click="onJoin"
            >
              {{ busy ? "Joining…" : "Join" }}
            </button>
          </div>
        </div>

        <div class="rounded-xl bg-zinc-900/60 p-6">
          <div class="flex items-center justify-between mb-1">
            <h2 class="text-lg font-medium text-zinc-200">Open rooms</h2>
            <button
              :ref="(el: any) => registerAction(el, { onSelect: browse })"
              :disabled="browsing"
              class="text-xs text-zinc-400 hover:text-zinc-200 disabled:opacity-50"
              @click="browse"
            >
              {{ browsing ? "Refreshing…" : "Refresh" }}
            </button>
          </div>
          <p class="text-sm text-zinc-500 mb-4">
            Jump into a room someone on this server is hosting.
          </p>
          <div
            v-if="browseError"
            class="mb-3 flex flex-wrap items-center gap-3 rounded-lg bg-red-900/30 border border-red-500/30 px-4 py-3 text-sm text-red-200"
          >
            <span class="flex-1 min-w-0">
              Couldn't load open rooms: {{ browseError }}
            </span>
            <button
              :ref="(el: any) => registerAction(el, { onSelect: browse })"
              :disabled="browsing"
              class="px-3 py-1.5 rounded-md text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
              @click="browse"
            >
              Retry
            </button>
          </div>
          <div v-if="browsable.length" class="space-y-2">
            <div
              v-for="r in browsable"
              :key="r.roomId"
              class="flex items-center gap-3 rounded-lg bg-zinc-800/50 px-4 py-3"
            >
              <div class="min-w-0 flex-1">
                <p class="text-sm font-medium text-zinc-200 truncate">
                  {{ r.gameName || r.name || "Co-op room" }}
                </p>
                <p class="text-xs text-zinc-500 truncate">
                  {{ r.hostName }} · {{ r.memberCount }}
                  {{ r.memberCount === 1 ? "player" : "players" }}
                </p>
              </div>
              <button
                v-if="!r.isSelf"
                :ref="(el: any) => registerAction(el, { onSelect: () => join(r.shortCode) })"
                :disabled="busy"
                class="shrink-0 px-4 py-1.5 rounded-md text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-50"
                @click="join(r.shortCode)"
              >
                Join
              </button>
              <span
                v-else
                class="shrink-0 text-xs px-2 py-0.5 rounded bg-blue-600/20 text-blue-300"
              >
                Your room
              </span>
            </div>
          </div>
          <p v-else-if="browsing" class="text-sm text-zinc-600">
            Looking for rooms…
          </p>
          <p v-else-if="browseLoaded && !browseError" class="text-sm text-zinc-600">
            No open rooms right now.
          </p>
        </div>

        <p v-if="hint" class="text-xs text-zinc-600">
          {{ hint }}
        </p>
      </div>
      </template>
    </div>

    <BigPictureKeyboard
      :visible="showKeyboard"
      :model-value="joinCode"
      placeholder="ABC-123"
      @update:model-value="joinCode = $event"
      @close="showKeyboard = false"
      @submit="onSubmitCode"
    />
    <BigPictureKeyboard
      :visible="showGameKeyboard"
      :model-value="gameQuery"
      placeholder="Game name"
      @update:model-value="gameQuery = $event"
      @close="showGameKeyboard = false"
      @submit="onSubmitGameSearch"
    />
  </div>
</template>

<script setup lang="ts">
import { UserGroupIcon } from "@heroicons/vue/24/outline";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useCoopRoom } from "~/composables/coop-room";
import {
  elevationHint,
  formatRoomCode,
  unavailableReason,
} from "~/composables/coop-room-logic";
import { useDisplayName } from "~/composables/use-display-name";
import {
  useServerApi,
  type FavoriteSearchRow,
} from "~/composables/use-server-api";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import BigPictureArchipelago from "~/components/bigpicture/BigPictureArchipelago.vue";

definePageMeta({ layout: "bigpicture" });

const {
  room,
  status,
  statusError,
  members,
  busy,
  error,
  isHost,
  sessionEnded,
  codeCopied,
  hostIp,
  hostIpCopied,
  roomGameName,
  browsable,
  browsing,
  browseError,
  browseLoaded,
  pendingRoom,
  pendingError,
  hostGame,
  displayCode,
  loadStatus,
  copyCode,
  copyHostIp,
  host,
  join,
  browse,
  leave,
  checkMine,
  rejoinPending,
  leavePending,
  dismissSessionEnded,
} = useCoopRoom();

const tabs = [
  { id: "coop" as const, label: "Co-op" },
  { id: "archipelago" as const, label: "Archipelago" },
];
const tab = ref<"coop" | "archipelago">("coop");

const joinCode = ref("");
const confirmingLeave = ref(false);
const showKeyboard = ref(false);

const unavailable = computed(() => unavailableReason(status.value));
const hint = computed(() => elevationHint(status.value));

function onJoin() {
  join(joinCode.value);
}
function onSubmitCode() {
  showKeyboard.value = false;
  onJoin();
}
async function doLeave() {
  confirmingLeave.value = false;
  await leave();
}

// ── Optional game for the room ──────────────────────────────────────────
// Same server search the desktop picker (GamePickerModal) uses.
const GAME_RESULTS_MAX = 6;
const api = useServerApi();
const showGameKeyboard = ref(false);
const gameQuery = ref("");
const gameResults = ref<FavoriteSearchRow[]>([]);
const gameSearching = ref(false);
const gameSearched = ref(false);
const gameSearchError = ref("");

function openGameSearch() {
  showGameKeyboard.value = true;
}
function onSubmitGameSearch() {
  showGameKeyboard.value = false;
  runGameSearch(gameQuery.value);
}
async function runGameSearch(term: string) {
  const q = term.trim();
  gameSearchError.value = "";
  if (!q) {
    gameResults.value = [];
    gameSearched.value = false;
    return;
  }
  gameSearching.value = true;
  try {
    const rows = await api.profile.favorites.search(q);
    gameResults.value = rows.slice(0, GAME_RESULTS_MAX);
    gameSearched.value = true;
  } catch (e) {
    gameResults.value = [];
    gameSearchError.value = e instanceof Error ? e.message : String(e);
  } finally {
    gameSearching.value = false;
  }
}
// The control that held focus (a result row, Clear, Rejoin) disappears in
// these handlers, and the focus system would otherwise fall back to the first
// element on the page (the tabs). Land on the next logical control instead.
const els: { host: HTMLElement | null; code: HTMLElement | null } = {
  host: null,
  code: null,
};
function focusAfterRender(target: () => HTMLElement | null) {
  nextTick(() => focusNav.focusElement(target()));
}
function pickGame(g: FavoriteSearchRow) {
  hostGame.value = { id: g.id, name: g.mName };
  gameResults.value = [];
  gameSearched.value = false;
  focusAfterRender(() => els.host);
}
function clearHostGame() {
  hostGame.value = null;
  focusAfterRender(() => els.host);
}
async function onRejoin() {
  if (await rejoinPending()) focusAfterRender(() => els.code);
}

const focusNav = useFocusNavigation();
const registerAction = useBpFocusableGroup("content");

onMounted(() => {
  // Flip an old hostname label to the account name (or a custom one) so it's
  // correct in both co-op and Archipelago before anyone sees it.
  useDisplayName().ensure();
  loadStatus();
  // Member polling runs app-wide (plugins/coop-room.client.ts); this page only
  // refreshes what's shown when you're not in a room.
  if (!room.value) {
    checkMine();
    browse();
  }
  focusNav.autoFocusContent("content");
});
</script>
