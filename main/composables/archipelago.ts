/**
 * Shared state + actions for Archipelago multiworld sessions, used by the
 * desktop Multiplayer page (ArchipelagoPanel) and the Big Picture one
 * (BigPictureArchipelago), which can join, leave and set the connect address
 * but not start a session or upload YAMLs.
 *
 * Mirrors `coop-room.ts` deliberately: same singleton-poll + `useState` shape,
 * so both tabs behave identically. The difference is what a session IS — co-op
 * rooms are throwaway networks between players, whereas an Archipelago session
 * is a long-lived group collecting YAMLs for a seed that the Archipelago
 * WebHost generates and hosts. Drop handles reachability, collection and
 * handing out the connect string; WebHost owns generation and trackers.
 *
 * All server calls go through Rust commands (`ap_*`), which are JWT/cert-authed
 * and also drive the local ZeroTier daemon.
 */

import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import {
  type ApZerotierStatus,
  isSessionGoneError,
  normaliseSessionCode,
  parseLeaveError,
  restoreRejoinPlan,
} from "./archipelago-logic";

export interface ApSessionInfo {
  sessionId: string;
  shortCode?: string | null;
  name?: string | null;
  networkId: string;
  serverAddress?: string | null;
}

export interface ApSlot {
  clientId: string;
  clientName: string;
  slotName: string | null;
  game: string | null;
  hasYaml: boolean;
  uploadedAt: string | null;
  isHost: boolean;
  isSelf: boolean;
}

export interface ApSessionDetail {
  sessionId: string;
  shortCode: string;
  name: string | null;
  status: "Setup" | "Running" | "Closed";
  connectAddress: string | null;
  networkId: string | null;
  serverAddress: string | null;
  /** False when the server's ZeroTier node doesn't report holding `serverAddress`. */
  serverAddressVerified?: boolean;
  isHost: boolean;
  slots: ApSlot[];
  readyCount: number;
  totalCount: number;
  allReady: boolean;
}

// Module-level so polling stays a singleton regardless of how many views mount.
let pollTimer: ReturnType<typeof setInterval> | null = null;
let codeCopyTimer: ReturnType<typeof setTimeout> | null = null;
let connectCopyTimer: ReturnType<typeof setTimeout> | null = null;

export function useArchipelago() {
  const session = useState<ApSessionInfo | null>("apSession", () => null);
  const detail = useState<ApSessionDetail | null>("apDetail", () => null);
  const busy = useState("apBusy", () => false);
  const error = useState("apError", () => "");
  const notice = useState("apNotice", () => "");
  const sessionEnded = useState("apSessionEnded", () => false);
  const codeCopied = useState("apCodeCopied", () => false);
  const connectCopied = useState("apConnectCopied", () => false);

  // Each failure has its own state so a failed fetch never looks like "no
  // session", and each has its own retry.
  /** The poll for the current session failed; the last good view stays up. */
  const fetchError = useState("apFetchError", () => "");
  /** Checking the server for an open session after a restart failed. */
  const restoreError = useState("apRestoreError", () => "");
  /** True while that check runs, so the start/join forms don't flash up. */
  const restoring = useState("apRestoring", () => false);
  /** Re-joining the overlay for a restored session failed. */
  const reconnectError = useState("apReconnectError", () => "");
  /**
   * A restored session's overlay wasn't re-joined automatically because that
   * might have needed a UAC or password prompt; the user presses Reconnect.
   */
  const reconnectNeeded = useState("apReconnectNeeded", () => false);
  /** Re-joining needs the one-time ZeroTier setup, which Game Mode can't do. */
  const desktopSetupNeeded = useState("apDesktopSetupNeeded", () => false);
  /** The server didn't record a leave; the session is still open. */
  const leaveError = useState("apLeaveError", () => "");
  /** Loading the WebHost link and game list failed. */
  const configError = useState("apConfigError", () => "");

  // WebHost integration (the separate container that owns YAML generation and
  // room hosting). Null/empty until loadConfig runs, or when the operator hasn't
  // configured a WebHost URL — the panel hides the link + search in that case.
  const webHostUrl = useState<string | null>("apWebHostUrl", () => null);
  const supportedGames = useState<string[]>("apSupportedGames", () => []);
  const configLoaded = useState("apConfigLoaded", () => false);

  const rawCode = computed(
    () => detail.value?.shortCode ?? session.value?.shortCode ?? "",
  );
  const displayCode = computed(() => {
    const c = rawCode.value;
    return c.length === 6 ? `${c.slice(0, 3)}-${c.slice(3)}` : c;
  });

  function errMessage(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function copyText(
    text: string | null | undefined,
    flag: Ref<boolean>,
    timerRef: "code" | "connect",
  ) {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      flag.value = true;
      const existing = timerRef === "code" ? codeCopyTimer : connectCopyTimer;
      if (existing) clearTimeout(existing);
      const t = setTimeout(() => {
        flag.value = false;
      }, 2000);
      if (timerRef === "code") codeCopyTimer = t;
      else connectCopyTimer = t;
    } catch (e) {
      console.error("clipboard write failed", e);
    }
  }

  const copyCode = () => copyText(rawCode.value, codeCopied, "code");
  const copyConnect = () =>
    copyText(detail.value?.connectAddress, connectCopied, "connect");

  /** Forget the current session locally (it was left, closed or is gone). */
  function clearSession() {
    stopPolling();
    session.value = null;
    detail.value = null;
    notice.value = "";
    fetchError.value = "";
    reconnectError.value = "";
    reconnectNeeded.value = false;
    desktopSetupNeeded.value = false;
    leaveError.value = "";
  }

  /** This device is on the session's overlay (just created, joined or reconnected). */
  function markConnected() {
    reconnectError.value = "";
    reconnectNeeded.value = false;
    desktopSetupNeeded.value = false;
  }

  async function refresh() {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id) return;
    try {
      const d = await invoke<ApSessionDetail>("ap_session_get", {
        sessionId: id,
      });
      fetchError.value = "";
      if (d.status === "Closed") {
        clearSession();
        sessionEnded.value = true;
      } else {
        detail.value = d;
      }
    } catch (e) {
      // Mirrors co-op: a vanished session is a calm "ended", not an error.
      if (isSessionGoneError(errMessage(e))) {
        clearSession();
        sessionEnded.value = true;
      } else {
        console.error("ap_session_get failed", e);
        fetchError.value = errMessage(e);
      }
    }
  }

  function startPolling() {
    stopPolling();
    pollTimer = setInterval(refresh, 4000);
  }
  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  async function create(name?: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    notice.value = "";
    sessionEnded.value = false;
    try {
      session.value = await invoke<ApSessionInfo>("ap_session_create", {
        name: name?.trim() || null,
      });
      restoreError.value = "";
      markConnected();
      await refresh();
      startPolling();
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  /** Join by code. Resolves true when this device is now in the session. */
  async function join(code: string): Promise<boolean> {
    const c = normaliseSessionCode(code);
    if (busy.value || c.length === 0) return false;
    busy.value = true;
    error.value = "";
    notice.value = "";
    sessionEnded.value = false;
    try {
      session.value = await invoke<ApSessionInfo>("ap_session_join", {
        shortCode: c,
      });
      restoreError.value = "";
      markConnected();
      await refresh();
      startPolling();
      return true;
    } catch (e) {
      error.value = errMessage(e);
      return false;
    } finally {
      busy.value = false;
    }
  }

  /**
   * Pick a YAML and upload it. Validation errors from the server (bad YAML, a
   * slot name someone else already took) are surfaced verbatim — catching those
   * here instead of at generation time is the point of collecting centrally.
   */
  async function uploadYaml() {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id || busy.value) return;

    const picked = await open({
      multiple: false,
      filters: [{ name: "Archipelago options", extensions: ["yaml", "yml"] }],
    });
    if (typeof picked !== "string") return;

    busy.value = true;
    error.value = "";
    notice.value = "";
    try {
      const res = await invoke<{ slotName?: string }>("ap_yaml_upload", {
        sessionId: id,
        filePath: picked,
      });
      notice.value = res.slotName
        ? `Uploaded settings for "${res.slotName}".`
        : "Settings uploaded.";
      await refresh();
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  /** Save every valid slot's YAML as one file to hand to WebHost's Generate page. */
  async function saveBundle() {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id || busy.value) return;

    const dest = await save({
      defaultPath: `archipelago-${rawCode.value || "session"}.yaml`,
      filters: [{ name: "Archipelago options", extensions: ["yaml"] }],
    });
    if (!dest) return;

    busy.value = true;
    error.value = "";
    notice.value = "";
    try {
      const path = await invoke<string>("ap_bundle_save", {
        sessionId: id,
        destPath: dest,
      });
      notice.value = `Saved to ${path}. Upload it on the Archipelago "Generate" page.`;
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  /**
   * Host stores the connect string from the Archipelago room page. Resolves
   * true only when the server saved it, so callers keep the typed draft
   * otherwise. Every false return leaves a reason in `error`.
   */
  async function setConnect(address: string): Promise<boolean> {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id) {
      error.value = "Not in a session any more, so the address wasn't saved.";
      return false;
    }
    if (!address.trim()) {
      error.value = "Enter the connect address first.";
      return false;
    }
    if (busy.value) {
      error.value =
        "Something else is still in progress. Try saving the address again in a moment.";
      return false;
    }
    busy.value = true;
    error.value = "";
    notice.value = "";
    try {
      await invoke("ap_connect_set", {
        sessionId: id,
        connectAddress: address,
      });
      await refresh();
      return true;
    } catch (e) {
      error.value = errMessage(e);
      return false;
    } finally {
      busy.value = false;
    }
  }

  async function leave() {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id || busy.value) return;
    busy.value = true;
    error.value = "";
    leaveError.value = "";
    try {
      await invoke("ap_session_leave", {
        sessionId: id,
        networkId: session.value?.networkId ?? detail.value?.networkId ?? null,
      });
      clearSession();
    } catch (e) {
      const parsed = parseLeaveError(errMessage(e));
      if (parsed.notRecorded) {
        // The server still has us in the session and nothing local changed:
        // keep it on screen so the leave can be retried.
        leaveError.value = parsed.message;
      } else {
        // The server recorded the leave; this says what didn't happen locally
        // (e.g. ZeroTier wasn't running to disconnect the overlay).
        clearSession();
        error.value = parsed.message;
      }
    } finally {
      busy.value = false;
    }
  }

  /**
   * Forget the session on this device only, for when a leave can't be
   * recorded because the server is gone: leaves the overlay and clears the
   * cached ids so Drop's own ZeroTier daemon can stop. The server isn't told.
   */
  async function forget() {
    if (busy.value || !(session.value || detail.value)) return;
    const networkId =
      session.value?.networkId ?? detail.value?.networkId ?? null;
    busy.value = true;
    error.value = "";
    notice.value = "";
    try {
      await invoke("ap_session_forget", { networkId });
      clearSession();
      notice.value =
        "Removed the session from this device. The server may still list you in it.";
    } catch (e) {
      // The session is forgotten locally either way; this says what the
      // overlay leave couldn't do.
      clearSession();
      error.value = `Removed the session from this device, but couldn't take this device off the Archipelago network: ${errMessage(e)}`;
    } finally {
      busy.value = false;
    }
  }

  /**
   * Re-join the overlay for the session on screen (after a restart the local
   * ZeroTier daemon may not be running, or not on the network). Joining a
   * session you're already in is harmless on the server: it re-authorizes this
   * device, records its node id and counts as activity. May show a UAC or
   * password prompt, so restore() only calls it when none is needed.
   */
  async function reconnect() {
    const code = detail.value?.shortCode;
    if (!code || busy.value) return;
    busy.value = true;
    reconnectError.value = "";
    try {
      session.value = await invoke<ApSessionInfo>("ap_session_join", {
        shortCode: code,
      });
      markConnected();
    } catch (e) {
      reconnectError.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  /**
   * Re-attach to an open session after a client restart. Its overlay is
   * re-joined right away only when that can't prompt (see restoreRejoinPlan);
   * otherwise the view offers Reconnect, or in Game Mode says the setup needs
   * Desktop Mode. A failed check is shown as an error with a retry, not as
   * "no session".
   */
  async function restore() {
    if (session.value || detail.value || restoring.value) return;
    restoring.value = true;
    restoreError.value = "";
    let found: ApSessionDetail | null = null;
    try {
      const open_ = await invoke<Array<{ sessionId: string }>>(
        "ap_session_list",
      );
      const first = open_?.[0];
      if (first) {
        found = await invoke<ApSessionDetail>("ap_session_get", {
          sessionId: first.sessionId,
        });
      }
    } catch (e) {
      console.error("ap_session_list failed", e);
      restoreError.value = errMessage(e);
    } finally {
      restoring.value = false;
    }
    // A join or create may have finished while we were asking.
    if (!found || found.status === "Closed" || session.value || detail.value)
      return;
    detail.value = found;
    startPolling();

    let status: ApZerotierStatus | null = null;
    try {
      status = await invoke<ApZerotierStatus>("zerotier_status");
    } catch (e) {
      // Unknown state: offer Reconnect rather than risk a prompt.
      console.error("zerotier_status failed", e);
    }
    // Left, forgotten or replaced while we asked.
    if (detail.value?.sessionId !== found.sessionId || session.value) return;
    const plan = restoreRejoinPlan(status);
    if (plan === "auto") await reconnect();
    else if (plan === "ask") reconnectNeeded.value = true;
    else desktopSetupNeeded.value = true;
  }

  function dismissSessionEnded() {
    sessionEnded.value = false;
  }

  /**
   * Load the WebHost URL + supported-games list once. Cheap (the server caches
   * the games scrape), and config rarely changes within a run, so we skip the
   * call after the first success.
   */
  async function loadConfig() {
    if (configLoaded.value) return;
    try {
      const cfg = await invoke<{
        webHostUrl: string | null;
        games: string[];
      }>("ap_web_host");
      webHostUrl.value = cfg.webHostUrl ?? null;
      supportedGames.value = Array.isArray(cfg.games) ? cfg.games : [];
      configLoaded.value = true;
      configError.value = "";
    } catch (e) {
      console.error("ap_web_host failed", e);
      configError.value = errMessage(e);
    }
  }

  /** The WebHost options (YAML generator) page for a game, or null if unusable. */
  function gameOptionsUrl(game: string): string | null {
    const base = webHostUrl.value;
    const g = game.trim();
    if (!base || !g) return null;
    return `${base}/games/${encodeURIComponent(g)}/player-options`;
  }

  /** Open the WebHost home (its own game list + search) in the system browser. */
  async function openWebHost() {
    if (webHostUrl.value) await openExternal(webHostUrl.value);
  }

  /** Open a game's options page directly in the system browser. */
  async function openGameOptions(game: string) {
    const url = gameOptionsUrl(game);
    if (url) await openExternal(url);
  }

  return {
    session,
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
    configError,
    webHostUrl,
    supportedGames,
    rawCode,
    displayCode,
    loadConfig,
    openWebHost,
    openGameOptions,
    refresh,
    startPolling,
    stopPolling,
    copyCode,
    copyConnect,
    create,
    join,
    uploadYaml,
    saveBundle,
    setConnect,
    leave,
    forget,
    reconnect,
    restore,
    dismissSessionEnded,
  };
}
