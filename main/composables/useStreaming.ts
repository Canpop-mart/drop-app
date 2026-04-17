import { invoke } from "@tauri-apps/api/core";

// ── Types ──────────────────────────────────────────────────────────────────

export interface SunshineStatusResult {
  installed: boolean;
  running: boolean;
  version: string | null;
  web_ui_url: string | null;
  paired_clients: number;
}

export interface StreamingSession {
  id: string;
  status: "Starting" | "Ready" | "Streaming" | "Stopped";
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

  async function startSunshine(
    adminUsername: string,
    adminPassword: string,
  ): Promise<string> {
    loading.value = true;
    error.value = null;
    try {
      const webUiUrl = await invoke<string>("start_sunshine", {
        adminUsername,
        adminPassword,
      });
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

  async function sendPin(
    pin: string,
    clientName?: string,
    username?: string,
    password?: string,
  ): Promise<void> {
    error.value = null;
    try {
      await invoke("sunshine_send_pin", {
        pin,
        clientName: clientName ?? "Drop Client",
        username: username ?? "sunshine",
        password: password ?? "sunshine",
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
    username?: string,
    password?: string,
  ): Promise<void> {
    error.value = null;
    try {
      await invoke("sunshine_register_game", {
        gameId,
        gameName,
        launchCommand,
        username: username ?? "sunshine",
        password: password ?? "sunshine",
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
    console.log("[STREAMING] startStreamingSession called with gameId:", gameId, "hostLocalIp:", hostLocalIp);
    const args: Record<string, string | null> = {
      hostLocalIp: hostLocalIp ?? null,
    };
    // Only include gameId if it has a value — avoids Tauri deserializing "" as None
    if (gameId) {
      args.gameId = gameId;
    }
    console.log("[STREAMING] invoke args:", JSON.stringify(args));
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

  /** Request a stream from another device (push-based flow). */
  async function requestStream(gameId: string): Promise<string> {
    const sessionId = await invoke<string>("streaming_request_stream", {
      gameId,
    });
    return sessionId;
  }

  /** List all registered client devices for the current user. */
  async function listDevices(): Promise<ClientDevice[]> {
    return invoke<ClientDevice[]>("list_devices");
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
    startSunshine,
    stopSunshine,
    sendPin,
    registerGame,
    // Server sessions
    startStreamingSession,
    markSessionReady,
    stopStreamingSession,
    sendHeartbeat,
    listRemoteSessions,
    getConnectionInfo,
    // Push-based streaming
    requestStream,
    // Device management
    listDevices,
    remoteInstall,
  };
}

export interface ClientDevice {
  id: string;
  name: string;
  platform: string;
  lastConnected: string;
  isSelf: boolean;
}
