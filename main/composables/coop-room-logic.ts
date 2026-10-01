/**
 * Pure pieces of the co-op room flow, kept free of Nuxt/Tauri imports so they
 * run under `node --test main/tests/coop-room.test.ts`.
 *
 * All player-facing strings here are placeholders the owner rewrites.
 */

export interface ZerotierStatusLike {
  installed: boolean;
  running: boolean;
  capsReady: boolean;
  platform?: string;
  bundled?: boolean;
  needsDesktopSetup?: boolean;
}

/** What `room_mine` returns: the room the server says this device is in. */
export interface MyRoom {
  roomId: string;
  shortCode: string;
  networkId: string;
  gameId?: string | null;
  gameName?: string | null;
  name?: string | null;
  isHost: boolean;
  /** Set by the client: this app session is already connected to it. */
  active?: boolean;
}

/** Accept a code in any shape the user might type or paste it. */
export function normaliseRoomCode(input: string): string {
  return input.replace(/[^a-zA-Z0-9]/g, "").toUpperCase();
}

/** "ABC123" -> "ABC-123" for display; anything else is shown as-is. */
export function formatRoomCode(code: string): string {
  return code.length === 6 ? `${code.slice(0, 3)}-${code.slice(3)}` : code;
}

/** The marker `room_members` returns when the room no longer exists. */
export function isRoomGoneError(message: string): boolean {
  return message.includes("room_not_found");
}

/**
 * Should the UI offer "Rejoin / Leave" for `mine`? Only when the server has a
 * room for us that this app session isn't already in, and we aren't in some
 * other room right now.
 */
export function rejoinOffer(
  mine: MyRoom | null | undefined,
  currentRoomId: string | null | undefined,
): MyRoom | null {
  if (!mine || mine.active) return null;
  if (currentRoomId) return null;
  return mine;
}

/**
 * Why co-op can't be used here yet, worded for this platform, or null when
 * it can. Shown instead of the host/join controls.
 */
export function unavailableReason(
  status: ZerotierStatusLike | null | undefined,
): string | null {
  if (!status) return null;
  if (status.needsDesktopSetup)
    return "Co-op needs a one-time setup that can't be done in Game Mode. Switch to Desktop Mode, open Drop, and host or join a room once. After that it works in Game Mode.";
  if (status.installed) return null;
  if (status.platform === "windows")
    return "ZeroTier isn't installed. Install ZeroTier One from zerotier.com, then reopen this page.";
  if (status.platform === "linux")
    return "This copy of Drop doesn't include ZeroTier. Use the Drop AppImage, or install ZeroTier and start its service.";
  return "Co-op rooms aren't available on this system.";
}

/**
 * The one-time permission hint under the host/join controls, or null when
 * nothing will be asked (already set up, or a service is already running).
 */
export function elevationHint(
  status: ZerotierStatusLike | null | undefined,
): string | null {
  if (!status || !status.installed || status.needsDesktopSetup) return null;
  if (status.platform === "windows")
    return "The first time, Windows asks for permission once so Drop can use ZeroTier.";
  if (status.platform === "linux" && !status.running && !status.capsReady)
    return "The first time, you'll be asked for your password once so Drop can set up the network adapter.";
  return null;
}
