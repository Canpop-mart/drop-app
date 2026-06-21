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

      <!-- Feature unavailable in this build -->
      <div
        v-if="status && !status.installed"
        class="px-4 py-3 rounded-lg bg-amber-900/20 border border-amber-500/30 text-amber-200 text-sm"
      >
        ZeroTier isn't available in this build. Co-op rooms need the bundled
        client (Steam Deck / Linux AppImage).
      </div>

      <!-- In a room -->
      <div v-else-if="room" class="space-y-5">
        <div class="rounded-xl bg-zinc-900/60 p-6">
          <p class="text-xs uppercase tracking-wide text-zinc-500 mb-1">
            Room code — share it with friends
          </p>
          <span
            class="text-3xl font-mono font-bold tracking-widest text-blue-300"
          >
            {{ displayCode || "…" }}
          </span>
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

        <button
          :ref="(el: any) => registerAction(el, { onSelect: leave })"
          :disabled="busy"
          class="px-5 py-2.5 rounded-lg text-sm font-medium bg-red-900/40 text-red-200 hover:bg-red-900/60 disabled:opacity-50"
          @click="leave"
        >
          Leave room
        </button>
        <p class="text-xs text-zinc-600">
          Now launch your game and use its LAN / "join by IP" option — friends in
          this room appear as if on your local network.
        </p>
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
            {{ busy ? "Working…" : "Host a room" }}
          </button>
        </div>

        <div class="rounded-xl bg-zinc-900/60 p-6">
          <h2 class="text-lg font-medium text-zinc-200 mb-1">Join a room</h2>
          <p class="text-sm text-zinc-500 mb-4">
            Enter the code a friend shared with you.
          </p>
          <div class="flex items-center gap-3">
            <input
              v-model="joinCode"
              :ref="(el: any) => registerAction(el)"
              placeholder="ABC123"
              maxlength="16"
              class="flex-1 px-4 py-2.5 rounded-lg bg-zinc-800 text-zinc-100 text-lg font-mono tracking-widest uppercase placeholder:text-zinc-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
              @keyup.enter="join"
            />
            <button
              :ref="(el: any) => registerAction(el, { onSelect: join })"
              :disabled="busy || joinCode.trim().length === 0"
              class="px-5 py-2.5 rounded-lg text-sm font-medium bg-zinc-700 text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
              @click="join"
            >
              Join
            </button>
          </div>
        </div>

        <p class="text-xs text-zinc-600">
          You may be asked for your password once, to let the app set up the
          virtual network adapter.
        </p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { UserGroupIcon } from "@heroicons/vue/24/outline";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";

definePageMeta({ layout: "bigpicture" });

interface RoomInfo {
  roomId: string;
  shortCode?: string | null;
  networkId: string;
  gameId?: string | null;
  name?: string | null;
}
interface ZerotierStatus {
  installed: boolean;
  running: boolean;
  capsReady: boolean;
  nodeId: string | null;
}
interface RoomMember {
  clientId: string;
  clientName: string;
  status: string;
  joinedAt: string;
  isHost: boolean;
}

// Persist the active room across page navigation within this app session.
const room = useState<RoomInfo | null>("coopRoom", () => null);
const status = ref<ZerotierStatus | null>(null);
const members = ref<RoomMember[]>([]);
const serverShortCode = ref<string | null>(null);
const joinCode = ref("");
const busy = ref(false);
const error = ref("");

const displayCode = computed(
  () => room.value?.shortCode ?? serverShortCode.value ?? "",
);

const focusNav = useFocusNavigation();
const registerAction = useBpFocusableGroup("content");

let pollTimer: ReturnType<typeof setInterval> | null = null;

function errMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}

async function loadStatus() {
  try {
    status.value = await invoke<ZerotierStatus>("zerotier_status");
  } catch (e) {
    console.error("zerotier_status failed", e);
  }
}

async function host() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    room.value = await invoke<RoomInfo>("room_host", {
      gameId: null,
      name: null,
    });
    await pollMembers();
    startPolling();
  } catch (e) {
    error.value = errMessage(e);
  } finally {
    busy.value = false;
  }
}

async function join() {
  const code = joinCode.value.trim().toUpperCase();
  if (busy.value || code.length === 0) return;
  busy.value = true;
  error.value = "";
  try {
    room.value = await invoke<RoomInfo>("room_join", { shortCode: code });
    joinCode.value = "";
    await pollMembers();
    startPolling();
  } catch (e) {
    error.value = errMessage(e);
  } finally {
    busy.value = false;
  }
}

async function leave() {
  if (!room.value || busy.value) return;
  busy.value = true;
  error.value = "";
  const r = room.value;
  try {
    await invoke("room_leave", { roomId: r.roomId, networkId: r.networkId });
  } catch (e) {
    console.error("room_leave failed", e);
  } finally {
    stopPolling();
    room.value = null;
    members.value = [];
    serverShortCode.value = null;
    busy.value = false;
  }
}

async function pollMembers() {
  if (!room.value) return;
  try {
    const detail = await invoke<{
      shortCode?: string;
      members: RoomMember[];
    }>("room_members", { roomId: room.value.roomId });
    members.value = detail.members ?? [];
    if (detail.shortCode) serverShortCode.value = detail.shortCode;
  } catch (e) {
    // The host may have torn the room down — surface nothing, just stop.
    console.error("room_members failed", e);
  }
}

function startPolling() {
  stopPolling();
  pollTimer = setInterval(pollMembers, 4000);
}
function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

onMounted(() => {
  loadStatus();
  if (room.value) {
    pollMembers();
    startPolling();
  }
  focusNav.autoFocusContent("content");
});
onUnmounted(() => {
  stopPolling();
});
</script>
