/**
 * Shared state + actions for ZeroTier co-op "rooms", used by both the desktop
 * page (`/multiplayer`) and the Big Picture page (`/bigpicture/multiplayer`).
 *
 * The room endpoints are JWT/cert-authed on the server, so all server calls go
 * through Rust commands (`room_host`/`room_join`/`room_leave`/`room_members`),
 * which also drive the local ZeroTier daemon. Room state lives in `useState` so
 * it persists across navigation and is consistent across both surfaces.
 */

import { invoke } from "@tauri-apps/api/core";

export interface RoomInfo {
  roomId: string;
  shortCode?: string | null;
  networkId: string;
  gameId?: string | null;
  name?: string | null;
}
export interface ZerotierStatus {
  installed: boolean;
  running: boolean;
  capsReady: boolean;
  nodeId: string | null;
}
export interface RoomMember {
  clientId: string;
  clientName: string;
  status: string;
  joinedAt: string;
  isHost: boolean;
}

// Module-level so polling is a singleton regardless of how many views mount.
let pollTimer: ReturnType<typeof setInterval> | null = null;

export function useCoopRoom() {
  const room = useState<RoomInfo | null>("coopRoom", () => null);
  const status = useState<ZerotierStatus | null>("coopStatus", () => null);
  const members = useState<RoomMember[]>("coopMembers", () => []);
  const serverShortCode = useState<string | null>("coopServerCode", () => null);
  const busy = useState("coopBusy", () => false);
  const error = useState("coopError", () => "");

  const displayCode = computed(
    () => room.value?.shortCode ?? serverShortCode.value ?? "",
  );

  function errMessage(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function loadStatus() {
    try {
      status.value = await invoke<ZerotierStatus>("zerotier_status");
    } catch (e) {
      console.error("zerotier_status failed", e);
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
      // The host may have torn the room down — leave state as-is, just log.
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

  async function join(code: string) {
    const c = code.trim().toUpperCase();
    if (busy.value || c.length === 0) return;
    busy.value = true;
    error.value = "";
    try {
      room.value = await invoke<RoomInfo>("room_join", { shortCode: c });
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

  return {
    room,
    status,
    members,
    busy,
    error,
    displayCode,
    loadStatus,
    pollMembers,
    startPolling,
    stopPolling,
    host,
    join,
    leave,
  };
}
