/**
 * Remote-streaming + cross-device control for the BPM game-detail page.
 *
 * Covers both halves of Drop's push-based streaming flow for one game:
 *  - Receiver side (the Steam Deck): request a stream from another device,
 *    poll for a host to accept, auto-launch Moonlight when the session is
 *    Ready, and watch for the host ending the session.
 *  - Cleanup: `stopStreaming()` cancels pending requests, stops host-side
 *    Sunshine sessions, kills Moonlight, and clears every interval.
 *  - Device discovery: the list of other registered devices, split into
 *    `streamableDevices` (have this game) and `installableDevices` (don't).
 *
 * Extracted verbatim — behaviour-identical — from the 3232-line
 * `pages/bigpicture/library/[id].vue`. All timers are owned here and torn
 * down in `dispose()`, which the page calls from `onUnmounted`.
 *
 * Per-game-detail composable: NOT a singleton — call from a component
 * `setup()`. Streaming/devices UI is dev-mode gated by the caller.
 */

import { invoke } from "@tauri-apps/api/core";
import { devLog } from "~/composables/dev-mode";
import { useStreaming } from "~/composables/useStreaming";
import type { ClientDevice } from "~/composables/useStreaming";
import type { GameVersion } from "~/types";

/** Granular streaming phase, drives the loading indicator label. */
export type StreamingPhase =
  | "requesting"
  | "host-preparing"
  | "connecting"
  | "streaming"
  | "ending"
  | null;

export function useBpmGameStreaming(
  gameId: string,
  /** The page's loaded version — its `userConfiguration` is sent to the host. */
  version: Ref<GameVersion | null>,
  /** Whether streaming/device UI is enabled (dev mode). */
  devModeEnabled: Ref<boolean>,
  /** Surface a streaming failure as a page-level error dialog. */
  onError: (msg: string) => void,
  /** Show a transient neutral toast (e.g. remote-install acknowledgement). */
  onInfo: (msg: string) => void,
) {
  const {
    stopStreamingSession,
    stopAllHostSessions,
    listRemoteSessions,
    getConnectionInfo,
    requestStream,
    killMoonlight,
    listDevices,
    remoteInstall,
  } = useStreaming();

  // ── State ─────────────────────────────────────────────────────────────
  const isStreaming = ref(false);
  const streamingPhase = ref<StreamingPhase>(null);
  const availableStream = ref<any>(null);
  const pendingRequestSessionId = ref<string | null>(null);
  const devices = ref<ClientDevice[]>([]);

  let activeStreamSessionId: string | null = null;
  let heartbeatInterval: ReturnType<typeof setInterval> | null = null;
  let moonlightWatchInterval: ReturnType<typeof setInterval> | null = null;
  let streamPollInterval: ReturnType<typeof setInterval> | null = null;
  let streamGuard = false;

  const streamingPhaseLabel = computed(() => {
    switch (streamingPhase.value) {
      case "requesting":
        return "Waiting for host...";
      case "host-preparing":
        return "Host starting Sunshine...";
      case "connecting":
        return "Connecting to stream...";
      case "streaming":
        return "Streaming";
      case "ending":
        return "Ending stream...";
      default:
        return "";
    }
  });

  // ── Device discovery ──────────────────────────────────────────────────
  // Deduplicate by name+platform, keeping the most recently connected entry.
  const otherDevices = computed(() => {
    const others = devices.value.filter((d) => !d.isSelf);
    const byKey = new Map<string, ClientDevice>();
    for (const d of others) {
      const key = `${d.name}::${d.platform}`;
      const existing = byKey.get(key);
      if (!existing || d.lastConnected > existing.lastConnected) {
        byKey.set(key, d);
      }
    }
    return [...byKey.values()];
  });

  // Devices that have this game installed (can stream from). Empty when dev
  // mode is off so the play menu's "Stream from {device}" rows disappear.
  const streamableDevices = computed(() =>
    devModeEnabled.value
      ? otherDevices.value.filter((d) => d.hasGame === true)
      : [],
  );
  // Devices that definitively do NOT have this game (can install on).
  // Strict `=== false` — devices that haven't reported (`undefined`) are
  // unknown and excluded from BOTH lists, so an already-installed game
  // never shows a spurious "Install on X" entry.
  const installableDevices = computed(() =>
    devModeEnabled.value
      ? otherDevices.value.filter((d) => d.hasGame === false)
      : [],
  );

  async function loadDevices() {
    try {
      devices.value = await listDevices(gameId);
    } catch {
      devices.value = [];
    }
  }

  // ── Remote session polling ────────────────────────────────────────────
  async function pollRemoteSessions() {
    try {
      const sessions = await listRemoteSessions();
      const found =
        sessions.find(
          (s: any) =>
            s.game?.id === gameId &&
            (s.status === "Ready" ||
              s.status === "Starting" ||
              s.status === "Streaming"),
        ) ?? null;
      availableStream.value = found;

      if (pendingRequestSessionId.value && found) {
        if (found.status === "Starting") {
          streamingPhase.value = "host-preparing";
        }
      }

      // Pending request just became Ready → auto-connect.
      if (
        pendingRequestSessionId.value &&
        found &&
        found.status === "Ready"
      ) {
        devLog(
          "event",
          "[BPM:STREAM] Our requested session is now Ready! Auto-connecting...",
        );
        streamingPhase.value = "connecting";
        pendingRequestSessionId.value = null;
        isStreaming.value = false;
        await connectToRemoteStream();
      }
    } catch {
      // Silently ignore poll errors
    }
  }

  async function connectToRemoteStream() {
    if (!availableStream.value) return;
    try {
      const sessionId = availableStream.value.id;
      const info = await getConnectionInfo(sessionId);
      devLog("event", "[BPM:STREAM] Connection info:", JSON.stringify(info));
      const host = info.hostLocalIp || info.hostExternalIp;
      if (!host) {
        onError("No host IP available for streaming");
        return;
      }
      const port = info.sunshinePort || 47989;
      devLog("event", `[BPM:STREAM] Launching Moonlight → ${host}:${port}`);
      await invoke("launch_moonlight", {
        host,
        port,
        pin: info.pairingPin ?? null,
        appName: info.game?.mName ?? null,
      });
      activeStreamSessionId = sessionId;
      isStreaming.value = true;
      streamingPhase.value = "streaming";
      streamGuard = false;

      // Rust-side watcher is the authoritative kill mechanism and keeps
      // running even if the Vue component unmounts.
      invoke("watch_moonlight_session", { sessionId }).catch((e: any) => {
        console.warn(
          "[BPM:STREAM] Failed to start Rust-side session watcher:",
          e,
        );
      });

      // Restore normal poll interval
      if (streamPollInterval) clearInterval(streamPollInterval);
      streamPollInterval = setInterval(pollRemoteSessions, 15_000);

      // Watch for the host ending the session → kill Moonlight.
      if (moonlightWatchInterval) clearInterval(moonlightWatchInterval);
      moonlightWatchInterval = setInterval(async () => {
        try {
          const sessions = await listRemoteSessions();
          const current = sessions.find((s: any) => s.id === sessionId);
          if (!current || current.status === "Stopped") {
            devLog(
              "event",
              "[BPM:STREAM] Session ended on host side — killing Moonlight",
            );
            if (moonlightWatchInterval) {
              clearInterval(moonlightWatchInterval);
              moonlightWatchInterval = null;
            }
            try {
              await killMoonlight();
            } catch (e) {
              console.warn("[BPM:STREAM] Failed to kill Moonlight:", e);
            }
            isStreaming.value = false;
            streamingPhase.value = null;
            activeStreamSessionId = null;
            availableStream.value = null;
          }
        } catch {
          // Silently ignore poll errors
        }
      }, 5_000);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("[BPM:STREAM] Failed to connect to stream:", msg);
      onError(`Stream connect failed: ${msg}`);
      streamGuard = false;
    }
  }

  /**
   * Request a stream from another device (push-based flow):
   * 1. Creates a "Requested" session on the server.
   * 2. Polls faster (every 3s) waiting for a host to pick it up.
   * 3. When the session becomes "Ready", auto-launches Moonlight.
   */
  async function streamGame(targetClientId?: string) {
    devLog(
      "event",
      "[BPM:STREAM] streamGame() called — target:",
      targetClientId ?? "any",
    );
    if (streamGuard) return;
    streamGuard = true;
    isStreaming.value = true;
    streamingPhase.value = "requesting";
    try {
      // Send this device's per-game config so the host applies the Deck's
      // settings (widescreen, quality, etc.) when launching the game.
      const gameConfigJson = version.value?.userConfiguration
        ? JSON.stringify(version.value.userConfiguration)
        : undefined;
      const sessionId = await requestStream(
        gameId,
        targetClientId,
        gameConfigJson,
      );
      pendingRequestSessionId.value = sessionId;
      devLog("event", "[BPM:STREAM] Stream requested, session:", sessionId);

      // Speed up polling while waiting for the host to accept.
      if (streamPollInterval) clearInterval(streamPollInterval);
      streamPollInterval = setInterval(pollRemoteSessions, 3_000);

      // Time out after 60s if no host responds.
      setTimeout(() => {
        if (pendingRequestSessionId.value === sessionId) {
          console.warn(
            "[BPM:STREAM] Stream request timed out — no host responded",
          );
          pendingRequestSessionId.value = null;
          isStreaming.value = false;
          streamingPhase.value = null;
          streamGuard = false;
          onError(
            "No host responded to the stream request. Make sure Drop is running on your PC.",
          );
          if (streamPollInterval) clearInterval(streamPollInterval);
          streamPollInterval = setInterval(pollRemoteSessions, 15_000);
        }
      }, 60_000);
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      console.error("[BPM:STREAM] Stream request failed:", errMsg);
      onError(`Stream request failed: ${errMsg}`);
      isStreaming.value = false;
      streamingPhase.value = null;
      streamGuard = false;
    }
  }

  /**
   * Stop the current streaming session — works for requester and host
   * sides. Cancels pending requests, stops all host sessions, kills
   * Moonlight, and clears every interval.
   */
  async function stopStreaming() {
    devLog("event", "[BPM:STREAM] stopStreaming() called");
    streamingPhase.value = "ending";
    try {
      if (pendingRequestSessionId.value) {
        try {
          await stopStreamingSession(pendingRequestSessionId.value);
        } catch (e) {
          console.warn("[BPM:STREAM] Failed to stop requested session:", e);
        }
        pendingRequestSessionId.value = null;
      }

      if (activeStreamSessionId) {
        try {
          await stopStreamingSession(activeStreamSessionId);
        } catch (e) {
          console.warn("[BPM:STREAM] Failed to stop active session:", e);
        }
        activeStreamSessionId = null;
      }

      try {
        const stopped = await stopAllHostSessions();
        if (stopped > 0) {
          devLog("event", `[BPM:STREAM] Stopped ${stopped} host session(s)`);
        }
      } catch (e) {
        console.warn("[BPM:STREAM] Failed to stop host sessions:", e);
      }

      // Also stop any active server-side sessions for this game (catches
      // sessions from before the cancellation code was deployed).
      try {
        const sessions = await listRemoteSessions();
        for (const s of sessions) {
          if (s.status !== "Stopped") {
            try {
              await stopStreamingSession(s.id);
            } catch {
              // May fail if we're not the host — that's ok.
            }
          }
        }
      } catch (e) {
        console.warn("[BPM:STREAM] Failed to clean up server sessions:", e);
      }

      try {
        await killMoonlight();
      } catch (e) {
        console.warn("[BPM:STREAM] Failed to kill Moonlight:", e);
      }

      if (heartbeatInterval) {
        clearInterval(heartbeatInterval);
        heartbeatInterval = null;
      }
      if (moonlightWatchInterval) {
        clearInterval(moonlightWatchInterval);
        moonlightWatchInterval = null;
      }
      if (streamPollInterval) clearInterval(streamPollInterval);
      streamPollInterval = setInterval(pollRemoteSessions, 15_000);
    } finally {
      isStreaming.value = false;
      streamingPhase.value = null;
      streamGuard = false;
      availableStream.value = null;
    }
  }

  /** Request a stream from a specific device. */
  function streamFromDevice(device: ClientDevice) {
    devLog(
      "event",
      `[BPM:STREAM] Requesting stream from device: ${device.name} (${device.id})`,
    );
    streamGame(device.id);
  }

  /** Ask another device to install this game. */
  async function installOnDevice(device: ClientDevice) {
    // Belt-and-braces gate — `installableDevices` is already empty when dev
    // mode is off, so the UI can't get here. But if a stale focus index
    // ever fires this with no device, bail cleanly.
    if (!devModeEnabled.value || !device) return;
    devLog(
      "event",
      `[BPM:STREAM] Remote install on device: ${device.name} (${device.id})`,
    );
    try {
      await remoteInstall(gameId, device.id);
      onInfo(
        `Install requested on ${device.name}. The download will start automatically when that device picks it up.`,
      );
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      onError(`Remote install failed: ${msg}`);
    }
  }

  /** Start receiver-side polling. Call from the page's `onMounted`. */
  function startPolling() {
    pollRemoteSessions();
    streamPollInterval = setInterval(pollRemoteSessions, 15_000);
  }

  /** Tear down every interval. Call from the page's `onUnmounted`. */
  function dispose() {
    if (streamPollInterval) {
      clearInterval(streamPollInterval);
      streamPollInterval = null;
    }
    if (heartbeatInterval) {
      clearInterval(heartbeatInterval);
      heartbeatInterval = null;
    }
    if (moonlightWatchInterval) {
      clearInterval(moonlightWatchInterval);
      moonlightWatchInterval = null;
    }
  }

  return {
    // State
    isStreaming,
    streamingPhase,
    streamingPhaseLabel,
    devices,
    streamableDevices,
    installableDevices,
    // Actions
    loadDevices,
    streamGame,
    streamFromDevice,
    installOnDevice,
    stopStreaming,
    startPolling,
    dispose,
    /** Exposed so `killGame` can stop streaming if the game is killed. */
    get hasActiveStream() {
      return isStreaming.value || activeStreamSessionId !== null;
    },
  };
}
