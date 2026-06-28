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
        <UserGroupIcon class="size-7 text-blue-400" />
        <h1 class="text-2xl font-semibold font-display text-zinc-100">
          Co-op Rooms
        </h1>
        <span
          v-if="room && members.length"
          class="text-sm text-zinc-500 font-medium"
        >
          · {{ members.length }} {{ members.length === 1 ? "player" : "players" }}
        </span>
      </div>
      <p class="text-sm text-zinc-500 mb-6">
        Put friends on a private virtual LAN so LAN / co-op games discover each
        other across the internet.
      </p>

      <div
        v-if="error"
        class="mb-4 px-4 py-3 rounded-lg bg-red-900/30 border border-red-500/30 text-red-200 text-sm"
      >
        {{ error }}
      </div>

      <!-- Host ended the session (calm, expected) -->
      <div
        v-if="sessionEnded"
        class="rounded-xl bg-zinc-900/60 border border-zinc-700 p-6 text-center"
      >
        <p class="text-lg font-medium text-zinc-200 mb-1">Session ended</p>
        <p class="text-sm text-zinc-500 mb-4">
          The host closed the room. You can host or join another anytime.
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
        v-else-if="status && !status.installed"
        class="px-4 py-3 rounded-lg bg-amber-900/20 border border-amber-500/30 text-amber-200 text-sm"
      >
        ZeroTier isn't available in this build. Co-op rooms need the bundled
        client (Steam Deck / Linux AppImage).
      </div>

      <!-- In a room -->
      <div v-else-if="room" class="space-y-5">
        <div class="rounded-xl bg-zinc-900/60 p-6">
          <p class="text-xs uppercase tracking-wide text-zinc-500 mb-2">
            {{ isHost ? "Room code (share with friends)" : "Room code" }}
          </p>
          <button
            :ref="(el: any) => registerAction(el, { onSelect: copyCode })"
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
          <p v-if="room.name" class="text-sm text-zinc-400 mt-2">
            {{ room.name }}
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
            In your game, choose "join by IP" / "direct connect" and enter this
            address. The LAN browser won't list it, so connect directly.
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
          <button
            :ref="(el: any) => registerAction(el, { onSelect: host })"
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
                :ref="(el: any) => registerAction(el, { onSelect: () => join(r.shortCode) })"
                :disabled="busy"
                class="shrink-0 px-4 py-1.5 rounded-md text-sm font-medium bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-50"
                @click="join(r.shortCode)"
              >
                Join
              </button>
            </div>
          </div>
          <p v-else class="text-sm text-zinc-600">
            {{ browsing ? "Looking for rooms…" : "No open rooms right now." }}
          </p>
        </div>

        <p class="text-xs text-zinc-600">
          You may be asked for your password once, to let the app set up the
          virtual network adapter.
        </p>
      </div>
    </div>

    <BigPictureKeyboard
      :visible="showKeyboard"
      :model-value="joinCode"
      placeholder="ABC-123"
      @update:model-value="joinCode = $event"
      @close="showKeyboard = false"
      @submit="onSubmitCode"
    />
  </div>
</template>

<script setup lang="ts">
import { UserGroupIcon } from "@heroicons/vue/24/outline";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useCoopRoom } from "~/composables/coop-room";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";

definePageMeta({ layout: "bigpicture" });

const {
  room,
  status,
  members,
  busy,
  error,
  isHost,
  sessionEnded,
  codeCopied,
  hostIp,
  hostIpCopied,
  browsable,
  browsing,
  displayCode,
  loadStatus,
  pollMembers,
  startPolling,
  stopPolling,
  copyCode,
  copyHostIp,
  host,
  join,
  browse,
  leave,
  dismissSessionEnded,
} = useCoopRoom();

const joinCode = ref("");
const confirmingLeave = ref(false);
const showKeyboard = ref(false);

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

const focusNav = useFocusNavigation();
const registerAction = useBpFocusableGroup("content");

onMounted(() => {
  loadStatus();
  if (room.value) {
    pollMembers();
    startPolling();
  } else {
    browse();
  }
  focusNav.autoFocusContent("content");
});
onUnmounted(() => {
  stopPolling();
});
</script>
