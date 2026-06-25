import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * Check for an app update once at startup. If one is available, prompt the
 * user; on confirm, download + install it and relaunch.
 *
 * Every failure path is swallowed (logged, never thrown): a dev build with no
 * updater configured, no published `latest.json` yet, a network outage, or an
 * unsigned/mismatched artifact must NEVER block app startup. The updater only
 * does anything in a signed release build whose `tauri.conf.json` carries a
 * real `plugins.updater.pubkey`.
 */
export async function checkForAppUpdate(): Promise<void> {
  let update;
  try {
    update = await check();
  } catch (e) {
    console.warn("[updater] update check failed (non-fatal):", e);
    return;
  }
  if (!update) return;

  createModal(
    ModalType.Confirmation,
    {
      title: `Update available — v${update.version}`,
      description: update.body
        ? `${update.body}\n\nInstall now? Drop will download the update and restart.`
        : `A new version (v${update.version}) is available. Install now? Drop will download it and restart.`,
      buttonText: "Update & restart",
    },
    async (e, c) => {
      c();
      if (e !== "confirm") return;
      try {
        await update!.downloadAndInstall();
        await relaunch();
      } catch (err) {
        console.error("[updater] download/install failed:", err);
        createModal(
          ModalType.Notification,
          {
            title: "Update failed",
            description:
              "Drop couldn't install the update automatically. You can download the latest version manually from the releases page.",
            buttonText: "Close",
          },
          (_e, cc) => cc(),
        );
      }
    },
  );
}
