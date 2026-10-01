/**
 * Pure pieces of the Archipelago session flow, kept free of Nuxt/Tauri imports
 * so they run under `node --test main/tests/archipelago.test.ts`.
 */

/**
 * Prefix `ap_session_leave` puts on an error when the server never recorded the
 * leave. Nothing local was changed in that case, so the session stays on
 * screen with a retry. Mirrors `AP_LEAVE_NOT_RECORDED` in src-tauri/src/zerotier.rs.
 */
export const AP_LEAVE_NOT_RECORDED = "ap_leave_not_recorded:";

/** Split an `ap_session_leave` error into "server didn't record it" + reason. */
export function parseLeaveError(message: string): {
  notRecorded: boolean;
  message: string;
} {
  const i = message.indexOf(AP_LEAVE_NOT_RECORDED);
  if (i === -1) return { notRecorded: false, message };
  return {
    notRecorded: true,
    message: message.slice(i + AP_LEAVE_NOT_RECORDED.length).trim(),
  };
}

/** The marker `ap_session_get` returns when the session no longer exists. */
export function isSessionGoneError(message: string): boolean {
  return message.includes("session_not_found");
}

/** Accept a session code in any shape the user might type or paste it. */
export function normaliseSessionCode(input: string): string {
  return input.replace(/[^a-zA-Z0-9]/g, "").toUpperCase();
}

/**
 * localStorage read that never throws: storage can be missing (SSR) or throw
 * (blocked or cleared site data). Null on any failure.
 */
export function safeGetItem(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

/**
 * localStorage write that never throws. Returns whether it was stored, so a
 * caller can still apply the change for this run when it wasn't.
 */
export function safeSetItem(key: string, value: string): boolean {
  try {
    if (!globalThis.localStorage) return false;
    globalThis.localStorage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

/** The parts of `zerotier_status` the restore decision needs. */
export interface ApZerotierStatus {
  running: boolean;
  needsDesktopSetup?: boolean;
  /**
   * Getting ZeroTier ready could show a UAC or password prompt. Missing (an
   * older client build) is treated as "could".
   */
  startNeedsPrompt?: boolean;
}

/**
 * How to re-join the overlay for a session found after a restart:
 * - "auto": ZeroTier already answers, or (Linux) Drop's own copy can be
 *   started without a password prompt, so re-joining asks for nothing.
 * - "ask": getting it ready could show a UAC or password prompt, or the
 *   status couldn't be read; show a Reconnect button the user presses instead
 *   of prompting the moment the tab opens.
 * - "desktop-setup": the one-time setup is still needed and this is Game Mode,
 *   where it can't be done; say so instead of offering a button that can't work.
 */
export function restoreRejoinPlan(
  status: ApZerotierStatus | null | undefined,
): "auto" | "ask" | "desktop-setup" {
  if (!status) return "ask";
  if (status.needsDesktopSetup) return "desktop-setup";
  if (status.running || status.startNeedsPrompt === false) return "auto";
  return "ask";
}

/** localStorage key remembering that the host dismissed the address note for a session. */
export function addressNoteKey(sessionId: string): string {
  return `ap-address-note-dismissed:${sessionId}`;
}

/** Placeholder: shown instead of Reconnect when only Desktop Mode can finish setup. */
export const AP_DESKTOP_SETUP_MSG =
  "Reconnecting needs a one-time setup that can't be done in Game Mode. Switch to Desktop Mode, open Drop and press Reconnect here once. After that it works in Game Mode.";
