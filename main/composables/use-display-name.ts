/**
 * The name this device shows to others in multiplayer (co-op rooms +
 * Archipelago). Backed by the local `displayName` setting, which is pushed to
 * the server-side client record (what the multiplayer lists render). When no
 * custom name is set, it falls back to the signed-in account name — so a fresh
 * install shows "your name" instead of a machine hostname like DESKTOP-XXXX.
 */
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "~/types";

export function useDisplayName() {
  const state = useAppState();

  /** The account name to fall back to: display name if set, else username. */
  function accountName(): string {
    const u = state.value?.user;
    return (u?.displayName?.trim() || u?.username || "").trim();
  }

  /**
   * Apply a chosen name to this device. Pass null/empty to revert to the
   * account name. Persists the choice locally and pushes the effective name to
   * the server. Returns the effective name.
   */
  async function setName(custom: string | null): Promise<string> {
    const trimmed = custom?.trim() ?? "";
    const effective = trimmed || accountName();
    await invoke("update_settings", {
      newSettings: { displayName: trimmed.length ? trimmed : null },
    });
    if (effective) await invoke("rename_client", { name: effective });
    return effective;
  }

  /**
   * Make sure the device name reflects the current choice (custom setting, else
   * the account name). Call when a multiplayer screen opens, so an old hostname
   * label flips to the account name with no action from the user.
   */
  async function ensure(): Promise<void> {
    try {
      const settings = await invoke<Settings>("fetch_settings");
      const effective = settings.displayName?.trim() || accountName();
      if (effective) await invoke("rename_client", { name: effective });
    } catch (e) {
      console.error("ensure display name failed", e);
    }
  }

  return { accountName, setName, ensure };
}
