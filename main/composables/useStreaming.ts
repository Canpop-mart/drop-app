import { invoke } from "@tauri-apps/api/core";
import { devLog } from "./dev-mode";

// ── Types ──────────────────────────────────────────────────────────────────

/** Mirrors `SunshineStatus` in src-tauri/src/streaming.rs (serde camelCase). */
export interface SunshineStatusResult {
  installed: boolean;
  /** Installed and complete. False with `installed` true means damaged files. */
  healthy: boolean;
  running: boolean;
  binaryPath: string | null;
  webUiPort: number;
  version: string;
}

export interface StreamingSession {
  id: string;
  status: "Requested" | "Starting" | "Ready" | "Streaming" | "Stopped";
  hostClient: {
    id: string;
    name: string;
    platform: string;
  };
  game: {
    id: string;
    mName: string;
    mIconObjectId: string;
  } | null;
  sunshinePort: number;
  hostLocalIp: string | null;
  hostExternalIp: string | null;
  hasPairingPin: boolean;
  /**
   * Why the host gave up, in words to show the person who pressed Play. Set
   * only on a `Stopped` session that failed, and those stay in the list for two
   * minutes so the requesting device can read this instead of waiting out its
   * own timeout. Null on a healthy session or a stop the user asked for.
   */
  error: string | null;
  createdAt: string;
  lastHeartbeat: string;
}

export interface StreamingConnectionInfo {
  id: string;
  status: string;
  hostClient: { id: string; name: string; platform: string };
  game: { id: string; mName: string } | null;
  sunshinePort: number;
  hostLocalIp: string | null;
  hostExternalIp: string | null;
  pairingPin: string | null;
  /** See `StreamingSession.error`. */
  error: string | null;
}

// ── Composable ─────────────────────────────────────────────────────────────

export function useStreaming() {
  const sunshineStatus = ref<SunshineStatusResult | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);

  // ── Local Sunshine management (Tauri invoke) ──────────────────────────

  async function checkSunshine(): Promise<SunshineStatusResult> {
    loading.value = true;
    error.value = null;
    try {
      const status =
        await invoke<SunshineStatusResult>("sunshine_status");
      sunshineStatus.value = status;
      return status;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function installSunshine(): Promise<string> {
    loading.value = true;
    error.value = null;
    try {
      const path = await invoke<string>("install_sunshine");
      await checkSunshine();
      return path;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  /** Wipe a damaged install and lay Sunshine down again. */
  async function repairSunshine(): Promise<string> {
    loading.value = true;
    error.value = null;
    try {
      const path = await invoke<string>("repair_sunshine");
      await checkSunshine();
      return path;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function startSunshine(): Promise<string> {
    loading.value = true;
    error.value = null;
    try {
      const webUiUrl = await invoke<string>("start_sunshine");
      await checkSunshine();
      return webUiUrl;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function stopSunshine(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      await invoke("stop_sunshine");
      await checkSunshine();
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  // Sunshine's admin credentials live in one place only: Drop generates them
  // and the Rust side reads them from settings. Nothing passes them in.
  async function sendPin(pin: string, clientName?: string): Promise<void> {
    error.value = null;
    try {
      await invoke("sunshine_send_pin", {
        pin,
        clientName: clientName ?? "Drop Client",
      });
    } catch (e) {
      error.value = String(e);
      throw e;
    }
  }

  async function registerGame(
    gameId: string,
    gameName: string,
    launchCommand: string,
  ): Promise<void> {
    error.value = null;
    try {
      await invoke("sunshine_register_game", {
        gameId,
        gameName,
        launchCommand,
      });
    } catch (e) {
      error.value = String(e);
      throw e;
    }
  }

  // ── Server-side session management (Tauri invoke → Rust → JWT auth) ──

  async function startStreamingSession(
    gameId?: string,
    hostLocalIp?: string,
  ): Promise<{ sessionId: string }> {
    devLog("event","[STREAMING] startStreamingSession called with gameId:", gameId, "hostLocalIp:", hostLocalIp);
    const args: Record<string, string | null> = {
      hostLocalIp: hostLocalIp ?? null,
    };
    // Only include gameId if it has a value — avoids Tauri deserializing "" as None
    if (gameId) {
      args.gameId = gameId;
    }
    devLog("event","[STREAMING] invoke args:", JSON.stringify(args));
    const sessionId = await invoke<string>("streaming_create_session", args);
    return { sessionId };
  }

  async function markSessionReady(
    sessionId: string,
    pairingPin?: string,
  ): Promise<void> {
    await invoke("streaming_mark_ready", {
      sessionId,
      pairingPin: pairingPin ?? null,
    });
  }

  async function stopStreamingSession(sessionId: string): Promise<void> {
    await invoke("streaming_stop_session", { sessionId });
  }

  /** Stop all host-side streaming sessions (heartbeats, Sunshine, etc). */
  async function stopAllHostSessions(): Promise<number> {
    return invoke<number>("stop_all_host_sessions");
  }

  async function sendHeartbeat(
    sessionId: string,
    status?: string,
  ): Promise<void> {
    await invoke("streaming_heartbeat", {
      sessionId,
      status: status ?? null,
    });
  }

  async function listRemoteSessions(): Promise<StreamingSession[]> {
    try {
      return await invoke<StreamingSession[]>("streaming_list_sessions");
    } catch {
      return [];
    }
  }

  async function getConnectionInfo(
    sessionId: string,
  ): Promise<StreamingConnectionInfo> {
    return invoke<StreamingConnectionInfo>("streaming_get_connection_info", {
      sessionId,
    });
  }

  /** Kill the Moonlight process (receiver side). */
  async function killMoonlight(): Promise<void> {
    await invoke("kill_moonlight");
  }

  /** Request a stream from another device (push-based flow).
   *  `gameConfig` is the JSON-serialized UserConfiguration from this client,
   *  so the host PC can apply the Deck's widescreen/quality settings during streaming.
   */
  async function requestStream(
    gameId: string,
    targetClientId?: string,
    gameConfig?: string,
  ): Promise<string> {
    const sessionId = await invoke<string>("streaming_request_stream", {
      gameId,
      targetClientId: targetClientId ?? null,
      gameConfig: gameConfig ?? null,
    });
    return sessionId;
  }

  /** List all registered client devices for the current user. */
  async function listDevices(gameId?: string): Promise<ClientDevice[]> {
    return invoke<ClientDevice[]>("list_devices", {
      gameId: gameId ?? null,
    });
  }

  /** Sync installed games to server. */
  async function syncInstalled(): Promise<void> {
    await invoke("sync_installed_games");
  }

  /** Request a remote install of a game on another device. */
  async function remoteInstall(
    gameId: string,
    targetClientId?: string,
  ): Promise<void> {
    await invoke("request_remote_install", {
      gameId,
      targetClientId: targetClientId ?? null,
    });
  }

  return {
    // State
    sunshineStatus,
    loading,
    error,
    // Local Sunshine
    checkSunshine,
    installSunshine,
    repairSunshine,
    startSunshine,
    stopSunshine,
    sendPin,
    registerGame,
    // Server sessions
    startStreamingSession,
    markSessionReady,
    stopStreamingSession,
    stopAllHostSessions,
    sendHeartbeat,
    listRemoteSessions,
    getConnectionInfo,
    // Push-based streaming
    requestStream,
    killMoonlight,
    // Device management
    listDevices,
    remoteInstall,
    syncInstalled,
  };
}

export interface ClientDevice {
  id: string;
  name: string;
  platform: string;
  lastConnected: string;
  isSelf: boolean;
  hasGame?: boolean;
}
