/**
 * Cloud-saves dev gate.
 *
 * Cloud saves doesn't sync seamlessly enough to ship to everyone, so it's a
 * dev-only feature: only active when the dev toggle (`useDevMode`) is on.
 *
 * The Rust save-sync is gated entirely on the `cloudSavesEnabled` DB setting
 * (checked in launch.rs / exit.rs / games.rs), so forcing that setting OFF
 * whenever dev mode is disabled guarantees no sync runs for non-dev users —
 * including users who had it enabled before it was re-gated. When dev mode is
 * ON we leave the user's setting untouched so they can toggle it normally.
 *
 * This runs on every client startup and re-checks whenever the dev toggle
 * flips, so the functional gate can't drift out of sync with the UI gate.
 */
import { useDevMode } from "~/composables/dev-mode";

export default defineNuxtPlugin((nuxtApp) => {
  const { enabled } = useDevMode();

  async function enforce() {
    // Dev mode on → cloud saves is allowed; respect the user's choice.
    if (enabled.value) return;

    // Dev mode off → make sure the master switch is off so the Rust sync
    // (which reads settings.cloud_saves_enabled) never fires.
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const settings = await invoke<{ cloudSavesEnabled?: boolean }>(
        "fetch_settings",
      );
      if (settings?.cloudSavesEnabled) {
        await invoke("update_settings", {
          newSettings: { cloudSavesEnabled: false },
        });
        console.log(
          "[cloud-saves-gate] dev mode off — forced cloudSavesEnabled=false",
        );
      }
    } catch (e) {
      console.warn("[cloud-saves-gate] failed to enforce gate:", e);
    }
  }

  // Wait for mount so the Tauri bridge is ready, then enforce + keep watching.
  nuxtApp.hook("app:mounted", () => {
    enforce();
    watch(enabled, enforce);
  });
});
