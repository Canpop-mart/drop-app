/**
 * Shared state + actions for Archipelago multiworld sessions, used by the
 * desktop Multiplayer page and (read-only) the Big Picture one.
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
  validationError: string | null;
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

  async function refresh() {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id) return;
    try {
      detail.value = await invoke<ApSessionDetail>("ap_session_get", {
        sessionId: id,
      });
      if (detail.value.status === "Closed") {
        stopPolling();
        session.value = null;
        detail.value = null;
        sessionEnded.value = true;
      }
    } catch (e) {
      // Mirrors co-op: a vanished session is a calm "ended", not an error.
      if (errMessage(e).includes("session_not_found")) {
        stopPolling();
        session.value = null;
        detail.value = null;
        sessionEnded.value = true;
      } else {
        console.error("ap_session_get failed", e);
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
      await refresh();
      startPolling();
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  async function join(code: string) {
    const c = code.replace(/[^a-zA-Z0-9]/g, "").toUpperCase();
    if (busy.value || c.length === 0) return;
    busy.value = true;
    error.value = "";
    notice.value = "";
    sessionEnded.value = false;
    try {
      session.value = await invoke<ApSessionInfo>("ap_session_join", {
        shortCode: c,
      });
      await refresh();
      startPolling();
    } catch (e) {
      error.value = errMessage(e);
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

  /** Host stores the connect string from the Archipelago room page. */
  async function setConnect(address: string) {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id || busy.value || !address.trim()) return;
    busy.value = true;
    error.value = "";
    notice.value = "";
    try {
      await invoke("ap_connect_set", {
        sessionId: id,
        connectAddress: address,
      });
      await refresh();
    } catch (e) {
      error.value = errMessage(e);
    } finally {
      busy.value = false;
    }
  }

  async function leave() {
    const id = session.value?.sessionId ?? detail.value?.sessionId;
    if (!id || busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      await invoke("ap_session_leave", {
        sessionId: id,
        networkId: session.value?.networkId ?? detail.value?.networkId ?? null,
      });
    } catch (e) {
      console.error("ap_session_leave failed", e);
    } finally {
      stopPolling();
      session.value = null;
      detail.value = null;
      notice.value = "";
      busy.value = false;
    }
  }

  /** Re-attach to an open session after a client restart. */
  async function restore() {
    if (session.value || detail.value) return;
    try {
      const open_ = await invoke<Array<{ sessionId: string }>>(
        "ap_session_list",
      );
      const first = open_?.[0];
      if (!first) return;
      detail.value = await invoke<ApSessionDetail>("ap_session_get", {
        sessionId: first.sessionId,
      });
      startPolling();
    } catch (e) {
      console.error("ap_session_list failed", e);
    }
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
    } catch (e) {
      console.error("ap_web_host failed", e);
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
    restore,
    dismissSessionEnded,
  };
}
