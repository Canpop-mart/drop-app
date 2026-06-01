import { invoke } from "@tauri-apps/api/core";
import { AppStatus, type AppState } from "~/types";
import { useListen } from "./useListen";
import { devLog } from "./dev-mode";
import type { VersionOption } from "./game";

export function setupHooks() {
  const router = useRouter();
  const state = useAppState();

  useListen("auth/processing", () => {
    router.push("/auth/processing");
  });

  useListen<string>("auth/failed", (event) => {
    router.push(`/auth/failed?error=${encodeURIComponent(event.payload)}`);
  });

  useListen("auth/finished", async () => {
    router.push("/library");
    state.value = JSON.parse(await invoke("fetch_state"));
  });

  useListen<string>("download_error", (event) => {
    createModal(
      ModalType.Notification,
      {
        title: "Drop encountered an error while downloading",
        description: `Drop encountered an error while downloading your game: "${event.payload.toString()}"`,
        buttonText: "Close",
      },
      (e, c) => c(),
    );
  });

  // Handle remote install requests from other devices.
  //
  // Fired by the host's stream-request poller when another of the user's
  // devices asks this one to install a game it doesn't have (the BPM library
  // "Install on {device}" rows). A request can only come from the same user's
  // own devices (server gates by account), so it's safe to act on — we start
  // the download and surface a notification so it isn't a silent surprise.
  useListen<{ gameId: string; gameName: string; sessionId: string }>(
    "remote-install-request",
    async (event) => {
      const payload = event.payload;
      devLog(
        "state",
        "[REMOTE-INSTALL] Received request to install:",
        payload.gameName,
        payload.gameId,
      );
      try {
        const versions = await invoke<VersionOption[]>(
          "fetch_game_version_options",
          { gameId: payload.gameId },
        );
        if (versions && versions.length > 0) {
          const vo = versions[0];
          await invoke("download_game", {
            gameId: payload.gameId,
            versionId: vo.versionId,
            installDir: 0,
            targetPlatform: vo.platform,
            enableUpdates: true,
          });
          devLog(
            "state",
            "[REMOTE-INSTALL] Download started for:",
            payload.gameName,
          );
          createModal(
            ModalType.Notification,
            {
              title: "Install started from another device",
              description: `“${payload.gameName}” is now downloading — it was requested from one of your other devices.`,
              buttonText: "OK",
            },
            (e, c) => c(),
          );
        } else {
          console.warn(
            "[REMOTE-INSTALL] No versions available for:",
            payload.gameId,
          );
          createModal(
            ModalType.Notification,
            {
              title: "Couldn't start remote install",
              description: `A device asked this PC to install “${payload.gameName}”, but no installable version is available here.`,
              buttonText: "Close",
            },
            (e, c) => c(),
          );
        }
      } catch (e) {
        console.warn("[REMOTE-INSTALL] Failed to start download:", e);
      }
    },
  );

  // This is for errors that (we think) aren't our fault
  useListen<string>("launch_external_error", (event) => {
    createModal(
      ModalType.Confirmation,
      {
        title: "Did something go wrong?",
        description:
          "Drop detected that something might've gone wrong with launching your game. Do you want to open the log directory?",
        buttonText: "Open",
      },
      async (e, c) => {
        if (e == "confirm") {
          await invoke("open_process_logs", { gameId: event.payload });
        }
        c();
      },
    );
  });
}

export function initialNavigation(state: ReturnType<typeof useAppState>) {
  if (!state.value)
    throw createError({
      statusCode: 500,
      statusMessage: "App state not valid",
      fatal: true,
    });
  const router = useRouter();

  switch (state.value.status) {
    case AppStatus.NotConfigured:
      router.push({ path: "/setup" });
      break;
    case AppStatus.SignedOut:
      router.push("/auth");
      break;
    case AppStatus.SignedInNeedsReauth:
      router.push("/auth/signedout");
      break;
    case AppStatus.ServerUnavailable:
      // Offline mode: if the server is unreachable but we have cached data
      // (user was previously signed in), go to the library so installed games
      // can still be launched. The home dashboard needs `/playtime/recent`
      // which requires the server, so library is the better fallback.
      if (state.value.user) {
        router.push("/library");
      } else {
        router.push("/error/serverunavailable");
      }
      break;
    default:
      // Signed in with the server reachable — land on the store. The
      // standalone home dashboard was removed because it duplicated the
      // store's Featured tab; users seeking the "what's new" view land
      // there directly.
      router.push("/store");
  }
}
