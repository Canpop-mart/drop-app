import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AppStatus, type AppState } from "~/types";

export function setupHooks() {
  const router = useRouter();
  const state = useAppState();

  const unlistenFns: Array<Promise<() => void>> = [];

  unlistenFns.push(
    listen("auth/processing", (event) => {
      router.push("/auth/processing");
    }),
  );

  unlistenFns.push(
    listen("auth/failed", (event) => {
      router.push(
        `/auth/failed?error=${encodeURIComponent(event.payload as string)}`,
      );
    }),
  );

  unlistenFns.push(
    listen("auth/finished", async (event) => {
      router.push("/library");
      state.value = JSON.parse(await invoke("fetch_state"));
    }),
  );

  unlistenFns.push(
    listen("download_error", (event) => {
      createModal(
        ModalType.Notification,
        {
          title: "Drop encountered an error while downloading",
          description: `Drop encountered an error while downloading your game: "${(
            event.payload as unknown as string
          ).toString()}"`,
          buttonText: "Close",
        },
        (e, c) => c(),
      );
    }),
  );

  // This is for errors that (we think) aren't our fault
  unlistenFns.push(
    listen("launch_external_error", (event) => {
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
    }),
  );

  onUnmounted(async () => {
    const resolvedFns = await Promise.all(unlistenFns);
    resolvedFns.forEach((fn) => fn());
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
      // can still be launched. Only show the error page if there's no user data.
      if (state.value.user) {
        router.push("/library");
      } else {
        router.push("/error/serverunavailable");
      }
      break;
    default:
      router.push("/library");
  }
}
