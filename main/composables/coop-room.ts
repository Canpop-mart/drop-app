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
export interface BrowsableRoom {
  roomId: string;
  shortCode: string;
  name?: string | null;
  gameId?: string | null;
  gameName?: string | null;
  hostName: string;
  memberCount: number;
  createdAt: string;
  isSelf: boolean;
}

// Module-level so polling is a singleton regardless of how many views mount.
let pollTimer: ReturnType<typeof setInterval> | null = null;
let copyTimer: ReturnType<typeof setTimeout> | null = null;
let hostIpCopyTimer: ReturnType<typeof setTimeout> | null = null;

export function useCoopRoom() {
  const room = useState<RoomInfo | null>("coopRoom", () => null);
  const status = useState<ZerotierStatus | null>("coopStatus", () => null);
  const members = useState<RoomMember[]>("coopMembers", () => []);
  const serverShortCode = useState<string | null>("coopServerCode", () => null);
  const busy = useState("coopBusy", () => false);
  const error = useState("coopError", () => "");
  // Whether this device is the room's host (drives host-vs-joiner framing).
  const isHost = useState("coopIsHost", () => false);
  // Set when a joiner's room vanishes underneath them (host ended it / expired).
  const sessionEnded = useState("coopSessionEnded", () => false);
  const codeCopied = useState("coopCodeCopied", () => false);
  // The host's ZeroTier IP for this room (what joiners enter in the game's "join
  // by IP"). Set from the member poll once the host reports it.
  const hostIp = useState<string | null>("coopHostIp", () => null);
  const hostIpCopied = useState("coopHostIpCopied", () => false);
  // Browsable open rooms, populated on demand by `browse()`.
  const browsable = useState<BrowsableRoom[]>("coopBrowsable", () => []);
  const browsing = useState("coopBrowsing", () => false);

  // The raw join code (unformatted) — what we copy and what `join` expects.
  const rawCode = computed(
    () => room.value?.shortCode ?? serverShortCode.value ?? "",
  );
  // A friendlier, grouped form for display (e.g. "ABC123" -> "ABC-123").
  const displayCode = computed(() => {
    const c = rawCode.value;
    return c.length === 6 ? `${c.slice(0, 3)}-${c.slice(3)}` : c;
  });

  function errMessage(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function copyCode() {
    if (!rawCode.value) return;
    try {
      await navigator.clipboard.writeText(rawCode.value);
      codeCopied.value = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        codeCopied.value = false;
      }, 2000);
    } catch (e) {
      console.error("clipboard write failed", e);
    }
  }

  async function copyHostIp() {
    if (!hostIp.value) return;
    try {
      await navigator.clipboard.writeText(hostIp.value);
      hostIpCopied.value = true;
      if (hostIpCopyTimer) clearTimeout(hostIpCopyTimer);
      hostIpCopyTimer = setTimeout(() => {
        hostIpCopied.value = false;
      }, 2000);
    } catch (e) {
      console.error("clipboard write failed", e);
    }
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
        hostAddress?: string | null;
        members: RoomMember[];
      }>("room_members", { roomId: room.value.roomId });
      members.value = detail.members ?? [];
      if (detail.shortCode) serverShortCode.value = detail.shortCode;
      // Once the host reports its ZeroTier IP it doesn't change for the room, so
      // only ever set it — never clear it on a transient poll that omits it.
      if (detail.hostAddress) hostIp.value = detail.hostAddress;
    } catch (e) {
      // 404 = the room is gone (host ended it / expired). Treat as a calm
      // "session ended", not an error. Other failures are transient — keep
      // polling and stay in the room.
      if (errMessage(e).includes("room_not_found")) {
        stopPolling();
        room.value = null;
        members.value = [];
        serverShortCode.value = null;
        hostIp.value = null;
        sessionEnded.value = true;
      } else {
        console.error("room_members failed", e);
      }
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
    sessionEnded.value = false;
    try {
      room.value = await invoke<RoomInfo>("room_host", {
        gameId: null,
        name: null,
      });
      isHost.value = true;
      await pollMembers();
      startPolling();
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  async function join(code: string) {
    // Accept the code in any shape the user might type/paste it.
    const c = code.replace(/[^a-zA-Z0-9]/g, "").toUpperCase();
    if (busy.value || c.length === 0) return;
    busy.value = true;
    error.value = "";
    sessionEnded.value = false;
    try {
      room.value = await invoke<RoomInfo>("room_join", { shortCode: c });
      isHost.value = false;
      await pollMembers();
      startPolling();
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  // Fetch the list of open rooms to join. Best-effort: surfaces errors via the
  // shared `error` state, leaving any previously-loaded list in place.
  async function browse() {
    browsing.value = true;
    try {
      browsable.value = await invoke<BrowsableRoom[]>("room_browse");
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      browsing.value = false;
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
      hostIp.value = null;
      busy.value = false;
    }
  }

  // Dismiss the "session ended" notice and return to the idle view.
  function dismissSessionEnded() {
    sessionEnded.value = false;
  }

  return {
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
    rawCode,
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
  };
}
