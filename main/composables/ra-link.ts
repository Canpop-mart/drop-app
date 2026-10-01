/**
 * RetroAchievements account linking for the desktop client, shared by the
 * desktop settings page, Big Picture settings and the welcome wizard.
 *
 * Linking goes through the Drop SERVER, not straight to retroachievements.org:
 * the server needs the player's RA username to record their unlocks (ra-poll
 * and session-end look them up by it). The call is a plain fetch to the
 * user-level route through the `server://` protocol, which attaches this
 * client's web token (`Bearer`, so the server's CSRF check doesn't apply).
 * The server returns the Connect token too; `ra_store_credentials` keeps a
 * local copy for RetroArch.
 */
import { invoke } from "@tauri-apps/api/core";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  parseRaServerAccount,
  raLinkState,
  raSyncProblemText,
  serverErrorText,
  type RaLinkState,
  type RaSyncReport,
} from "~/composables/achievements/status";

const RA_ROUTE = "api/v1/user/external-accounts/retroachievements";
const TIMEOUT_MS = 20_000;

async function readJson(res: Response): Promise<unknown> {
  try {
    return await res.json();
  } catch {
    return null;
  }
}

export function useRaLink() {
  /** "loading" until the first refresh finishes, "error" if the server check failed. */
  const loadState = ref<"loading" | "ready" | "error">("loading");
  const loadError = ref("");
  const serverUsername = ref<string | null>(null);
  const serverHasConnectToken = ref(true);
  /** Why the last device sync changed nothing (server unreachable), or "". */
  const syncError = ref("");
  const apiKeyRequired = ref(true);
  const localUsername = ref("");
  const localHasToken = ref(false);
  const localExpired = ref(false);
  const busy = ref(false);

  const state = computed<RaLinkState>(() =>
    raLinkState({
      serverUsername: serverUsername.value,
      serverHasConnectToken: serverHasConnectToken.value,
      localUsername: localUsername.value,
      localHasToken: localHasToken.value,
      localExpired: localExpired.value,
    }),
  );

  async function readLocal() {
    const settings = await invoke<Record<string, any>>("fetch_settings");
    localUsername.value = settings.raUsername ?? "";
    localHasToken.value = !!(settings.raToken && settings.raToken.length > 0);
    localExpired.value = !!settings.raExpiredToken;
  }

  /** Re-reads the local copy and asks the server what it has on file. */
  async function refresh() {
    loadState.value = "loading";
    loadError.value = "";
    try {
      await readLocal();
    } catch (e) {
      // Local settings are only used for the expired/device-only hints.
      console.warn("[RA-LINK] fetch_settings failed:", e);
    }
    try {
      const res = await fetch(serverUrl("api/v1/user/external-accounts"), {
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      if (!res.ok) throw new Error(serverErrorText(res.status, await readJson(res)));
      const parsed = parseRaServerAccount(await readJson(res));
      serverUsername.value = parsed.username;
      serverHasConnectToken.value = parsed.hasConnectToken;
      apiKeyRequired.value = parsed.apiKeyRequired;
      loadState.value = "ready";
      // The server is authoritative. If this device's copy is a different
      // account, a leftover from an older build, or the server has none, bring
      // it in line now instead of at the next RetroArch launch.
      if (localHasToken.value && state.value !== "linked") {
        await syncLocalCopy();
      }
    } catch (e: any) {
      loadError.value = String(e?.message ?? e);
      loadState.value = "error";
    }
  }

  /**
   * Re-syncs the local copy with the server's account (same rule a RetroArch
   * launch uses). Leaves the state showing `mismatch` if the server's
   * credentials route couldn't be reached.
   */
  async function syncLocalCopy() {
    busy.value = true;
    syncError.value = "";
    let report: RaSyncReport | null = null;
    try {
      report = await invoke<RaSyncReport>("ra_refresh_credentials");
    } catch (e) {
      console.warn("[RA-LINK] ra_refresh_credentials failed:", e);
    } finally {
      busy.value = false;
    }
    syncError.value = raSyncProblemText(report) ?? "";
    await readLocal().catch(() => {});
  }

  /**
   * Links on the server, then stores the returned token locally. Throws an
   * Error carrying the server's own message on failure.
   */
  async function link(username: string, password: string, apiKey?: string) {
    busy.value = true;
    try {
      const body: Record<string, string> = { username: username.trim(), password };
      if (apiKey && apiKey.trim()) body.apiKey = apiKey.trim();
      const res = await fetch(serverUrl(RA_ROUTE), {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      const json = await readJson(res);
      if (!res.ok) throw new Error(serverErrorText(res.status, json));
      const account = (json ?? {}) as { externalId?: string; connectToken?: string };
      if (!account.externalId) {
        throw new Error("The server did not confirm the link.");
      }
      serverUsername.value = account.externalId;
      serverHasConnectToken.value = !!account.connectToken;
      syncError.value = "";
      if (account.connectToken) {
        await invoke("ra_store_credentials", {
          username: account.externalId,
          connectToken: account.connectToken,
        });
      }
      await readLocal().catch(() => {});
    } finally {
      busy.value = false;
    }
  }

  /** Unlinks on the server (not linked there is fine) and clears the local copy. */
  async function unlink() {
    busy.value = true;
    try {
      const res = await fetch(serverUrl(RA_ROUTE), {
        method: "DELETE",
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      if (!res.ok && res.status !== 404) {
        throw new Error(serverErrorText(res.status, await readJson(res)));
      }
      serverUsername.value = null;
      serverHasConnectToken.value = true;
      await invoke("ra_clear_credentials");
      await readLocal().catch(() => {});
    } finally {
      busy.value = false;
    }
  }

  return {
    loadState,
    loadError,
    serverUsername,
    apiKeyRequired,
    localUsername,
    localExpired,
    syncError,
    state,
    busy,
    refresh,
    syncLocalCopy,
    link,
    unlink,
  };
}
