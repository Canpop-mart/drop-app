/**
 * Shared state + actions for ZeroTier co-op "rooms", used by both the desktop
 * page (`/multiplayer`) and the Big Picture page (`/bigpicture/multiplayer`).
 *
 * The room endpoints are JWT/cert-authed on the server, so all server calls go
 * through Rust commands (`room_host`/`room_join`/`room_leave`/`room_members`/
 * `room_mine`/`room_resume`), which also drive the local ZeroTier daemon. Room
 * state lives in `useState` so it persists across navigation and is consistent
 * across both surfaces.
 *
 * Member polling is NOT tied to a page: `plugins/coop-room.client.ts` runs it
 * for as long as a room is active, on either surface, so a joiner notices the
 * host ending the room (and cleans up) even while playing or browsing.
 */

import { invoke } from "@tauri-apps/api/core";
import {
  formatRoomCode,
  isRoomGoneError,
  normaliseRoomCode,
  rejoinOffer,
  type MyRoom,
  type ZerotierStatusLike,
} from "./coop-room-logic";

export interface RoomInfo {
  roomId: string;
  shortCode?: string | null;
  networkId: string;
  gameId?: string | null;
  name?: string | null;
  /** Set when joining by code turns out to be this device's own room. */
  isHost?: boolean;
}
export interface ZerotierStatus extends ZerotierStatusLike {
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
/** The game a host picked for the room (optional). */
export interface HostGame {
  id: string;
  name: string;
}
export interface CoopToast {
  id: number;
  message: string;
  /** Wording for Big Picture, when it differs (e.g. names a different menu). */
  bpmMessage?: string;
}

// Module-level so polling is a singleton regardless of how many views mount.
let pollTimer: ReturnType<typeof setInterval> | null = null;
let copyTimer: ReturnType<typeof setTimeout> | null = null;
let hostIpCopyTimer: ReturnType<typeof setTimeout> | null = null;
let toastSeq = 0;
// The room a user-initiated leave is tearing down. Polls for it are ignored so
// the leave's own server-side delete (a 404 on the next poll) isn't mistaken
// for the room ending underneath us.
let leavingRoomId: string | null = null;

const TOAST_LIFETIME_MS = 7_000;
const POLL_INTERVAL_MS = 4_000;

export function useCoopRoom() {
  const room = useState<RoomInfo | null>("coopRoom", () => null);
  const status = useState<ZerotierStatus | null>("coopStatus", () => null);
  const statusError = useState("coopStatusError", () => "");
  const members = useState<RoomMember[]>("coopMembers", () => []);
  const serverShortCode = useState<string | null>("coopServerCode", () => null);
  const busy = useState("coopBusy", () => false);
  const error = useState("coopError", () => "");
  // Whether this device is the room's host (drives host-vs-joiner framing).
  const isHost = useState("coopIsHost", () => false);
  // Set when the room vanishes underneath us (host ended it / expired / admin).
  const sessionEnded = useState("coopSessionEnded", () => false);
  const codeCopied = useState("coopCodeCopied", () => false);
  // The host's ZeroTier IP for this room (what joiners enter in the game's "join
  // by IP"). Set from the member poll once the host reports it.
  const hostIp = useState<string | null>("coopHostIp", () => null);
  const hostIpCopied = useState("coopHostIpCopied", () => false);
  // The room's game name, from the member poll.
  const roomGameName = useState<string | null>("coopGameName", () => null);
  // Browsable open rooms, populated on demand by `browse()`. `browseError` is
  // separate from `error` so a failed list never reads as "no open rooms".
  const browsable = useState<BrowsableRoom[]>("coopBrowsable", () => []);
  const browsing = useState("coopBrowsing", () => false);
  const browseError = useState("coopBrowseError", () => "");
  const browseLoaded = useState("coopBrowseLoaded", () => false);
  // Restart recovery: a room the server says we're still in.
  const pendingRoom = useState<MyRoom | null>("coopPendingRoom", () => null);
  const pendingError = useState("coopPendingError", () => "");
  // The game the host picked for the next room (optional).
  const hostGame = useState<HostGame | null>("coopHostGame", () => null);
  const toasts = useState<CoopToast[]>("coopToasts", () => []);

  // The raw join code (unformatted): what we copy and what `join` expects.
  const rawCode = computed(
    () => room.value?.shortCode ?? serverShortCode.value ?? "",
  );
  // A friendlier, grouped form for display (e.g. "ABC123" -> "ABC-123").
  const displayCode = computed(() => formatRoomCode(rawCode.value));

  function errMessage(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  function pushToast(message: string, bpmMessage?: string) {
    const id = ++toastSeq;
    toasts.value = [...toasts.value, { id, message, bpmMessage }];
    setTimeout(() => {
      toasts.value = toasts.value.filter((t) => t.id !== id);
    }, TOAST_LIFETIME_MS);
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
      error.value = `Couldn't copy the code: ${errMessage(e)}`;
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
      error.value = `Couldn't copy the address: ${errMessage(e)}`;
    }
  }

  async function loadStatus() {
    try {
      status.value = await invoke<ZerotierStatus>("zerotier_status");
      statusError.value = "";
    } catch (e) {
      statusError.value = errMessage(e);
    }
  }

  function resetRoomState() {
    stopPolling();
    room.value = null;
    members.value = [];
    serverShortCode.value = null;
    hostIp.value = null;
    roomGameName.value = null;
  }

  /**
   * The room disappeared on the server (host ended it, it expired, or an admin
   * deleted it). Run the same local cleanup as leaving (clear the seeded peer
   * files, leave the network, stop Drop's daemon if idle) and say so.
   */
  async function handleRoomGone(r: RoomInfo) {
    const wasHost = isHost.value;
    resetRoomState();
    sessionEnded.value = true;
    // Hold `busy` while the cleanup runs so a Host/Join pressed meanwhile
    // can't race the daemon shutdown at the end of room_leave.
    const ownsBusy = !busy.value;
    busy.value = true;
    try {
      // The server side is already gone (a 404 there counts as success).
      await invoke("room_leave", { roomId: r.roomId, networkId: r.networkId });
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      if (ownsBusy) busy.value = false;
    }
    pushToast(
      wasHost
        ? "Your co-op room has ended."
        : "The host ended the co-op room. You've been disconnected from it.",
    );
  }

  async function pollMembers() {
    const r = room.value;
    if (!r || leavingRoomId === r.roomId) return;
    try {
      const detail = await invoke<{
        shortCode?: string;
        hostAddress?: string | null;
        gameName?: string | null;
        members: RoomMember[];
      }>("room_members", { roomId: r.roomId });
      // Ignore a late answer for a room we've since left or are leaving.
      if (room.value?.roomId !== r.roomId || leavingRoomId === r.roomId) return;
      members.value = detail.members ?? [];
      if (detail.shortCode) serverShortCode.value = detail.shortCode;
      roomGameName.value = detail.gameName ?? null;
      // Once the host reports its ZeroTier IP it doesn't change for the room, so
      // only ever set it: never clear it on a transient poll that omits it.
      if (detail.hostAddress) hostIp.value = detail.hostAddress;
    } catch (e) {
      if (room.value?.roomId !== r.roomId || leavingRoomId === r.roomId) return;
      // 404 = the room is gone. Other failures are transient: keep polling
      // and stay in the room.
      if (isRoomGoneError(errMessage(e))) {
        await handleRoomGone(r);
      } else {
        console.error("room_members failed", e);
      }
    }
  }

  function startPolling() {
    if (pollTimer) return;
    pollTimer = setInterval(pollMembers, POLL_INTERVAL_MS);
    pollMembers();
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
      const info = await invoke<RoomInfo>("room_host", {
        gameId: hostGame.value?.id ?? null,
      });
      isHost.value = true;
      pendingRoom.value = null;
      room.value = info;
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  async function join(code: string) {
    const c = normaliseRoomCode(code);
    if (busy.value || c.length === 0) return;
    busy.value = true;
    error.value = "";
    sessionEnded.value = false;
    try {
      const info = await invoke<RoomInfo>("room_join", { shortCode: c });
      isHost.value = info.isHost === true;
      pendingRoom.value = null;
      room.value = info;
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  // Fetch the list of open rooms to join. A failure is kept apart from an
  // empty list (`browseError`), and the previous list stays in place.
  async function browse() {
    browsing.value = true;
    try {
      browsable.value = await invoke<BrowsableRoom[]>("room_browse");
      browseError.value = "";
      browseLoaded.value = true;
    } catch (e) {
      browseError.value = errMessage(e);
    } finally {
      browsing.value = false;
    }
  }

  async function leave() {
    if (!room.value || busy.value) return;
    busy.value = true;
    error.value = "";
    const r = room.value;
    leavingRoomId = r.roomId;
    stopPolling();
    try {
      await invoke("room_leave", { roomId: r.roomId, networkId: r.networkId });
    } catch (e) {
      // Local cleanup already ran; this says what the server didn't confirm.
      error.value = errMessage(e);
    } finally {
      resetRoomState();
      leavingRoomId = null;
      busy.value = false;
    }
  }

  /**
   * Restart recovery: ask the server whether this device is still in a room
   * this app session isn't connected to. Returns the room when one should be
   * offered for rejoin.
   */
  async function checkMine(): Promise<MyRoom | null> {
    try {
      const mine = await invoke<MyRoom | null>("room_mine");
      pendingError.value = "";
      pendingRoom.value = rejoinOffer(mine, room.value?.roomId);
    } catch (e) {
      pendingError.value = errMessage(e);
    }
    return pendingRoom.value;
  }

  async function rejoinPending(): Promise<boolean> {
    const p = pendingRoom.value;
    if (!p || busy.value) return false;
    busy.value = true;
    error.value = "";
    sessionEnded.value = false;
    try {
      const info = await invoke<RoomInfo>("room_resume", {
        shortCode: p.shortCode,
        isHost: p.isHost,
      });
      isHost.value = p.isHost;
      pendingRoom.value = null;
      room.value = { ...info, shortCode: info.shortCode ?? p.shortCode };
      return true;
    } catch (e) {
      error.value = errMessage(e);
      return false;
    } finally {
      busy.value = false;
    }
  }

  async function leavePending() {
    const p = pendingRoom.value;
    if (!p || busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      await invoke("room_leave", { roomId: p.roomId, networkId: p.networkId });
      pendingRoom.value = null;
    } catch (e) {
      // room_leave cleans up locally before reporting a server failure, so the
      // offer is gone either way; the error says what didn't happen.
      pendingRoom.value = null;
      error.value = errMessage(e);
    } finally {
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
    toasts,
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
    checkMine,
    rejoinPending,
    leavePending,
    dismissSessionEnded,
    pushToast,
  };
}
