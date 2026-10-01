/**
 * Pure helpers for achievement status on both surfaces: why a game shows no
 * achievements, what the server said when an RA link failed, and whether the
 * server knows the player's RetroAchievements account.
 *
 * No Vue, Nuxt or Tauri imports, so the Node test runner can load it:
 *   node --test main/tests/achievement-status.test.ts
 */

/** `GET /api/v1/games/{id}/achievements/status`. */
export interface AchievementStatus {
  definitionCount: number;
  /** Why there are no achievements. Null when there are some. */
  reason: string | null;
  /** RA-tracked game and this player has no RA account on the server. */
  raAccountMissing: boolean;
}

/**
 * Placeholder copy for each server reason. The owner rewrites these. Returns
 * null for no reason, and a generic line for a reason this build doesn't know
 * (a newer server), rather than hiding it.
 */
export function unavailableReasonText(reason: string | null | undefined): string | null {
  switch (reason) {
    case null:
    case undefined:
    case "":
      return null;
    case "no_link":
      return "This game is not linked to Steam or RetroAchievements on the server.";
    case "steam_key_missing":
      return "The server has no Steam API key, so it cannot load this game's achievements.";
    case "ra_credentials_missing":
      return "The server has no RetroAchievements login, so it cannot load this game's achievements.";
    case "not_scanned":
      return "No achievements are stored for this game yet. An admin may need to scan it.";
    default:
      return `No achievements are available (${reason}).`;
  }
}

/** Placeholder copy for an RA-tracked game when the player hasn't linked RA. */
export const RA_ACCOUNT_MISSING_TEXT =
  "Link your RetroAchievements account in Settings to record unlocks for this game.";

/** Normalises the status route's JSON; anything malformed becomes "unknown". */
export function parseAchievementStatus(json: unknown): AchievementStatus | null {
  if (!json || typeof json !== "object") return null;
  const o = json as Record<string, unknown>;
  return {
    definitionCount: typeof o.definitionCount === "number" ? o.definitionCount : 0,
    reason: typeof o.reason === "string" ? o.reason : null,
    raAccountMissing: o.raAccountMissing === true,
  };
}

/**
 * The human-readable part of a failed server response. h3 errors carry it in
 * `statusMessage` (or `message`); fall back to the HTTP status.
 */
export function serverErrorText(status: number, body: unknown): string {
  if (body && typeof body === "object") {
    const o = body as Record<string, unknown>;
    for (const key of ["statusMessage", "message"]) {
      const v = o[key];
      if (typeof v === "string" && v.trim() && v.trim() !== String(status)) {
        return v.trim();
      }
    }
  }
  return `The server returned an error (${status}).`;
}

/** What `GET /api/v1/user/external-accounts` says about RetroAchievements. */
export interface RaServerAccount {
  /** RA username the server has on file, or null when not linked. */
  username: string | null;
  /**
   * The server holds a RetroArch sign-in (Connect token) for it. False for
   * accounts linked before tokens were stored. Older servers don't say;
   * that reads as true.
   */
  hasConnectToken: boolean;
  /** The server needs the player's own Web API key to link. */
  apiKeyRequired: boolean;
}

export function parseRaServerAccount(json: unknown): RaServerAccount {
  const o = (json && typeof json === "object" ? json : {}) as Record<string, unknown>;
  const accounts = Array.isArray(o.accounts) ? o.accounts : [];
  const ra = accounts.find(
    (a): a is { provider: string; externalId: string } =>
      !!a &&
      typeof a === "object" &&
      (a as { provider?: unknown }).provider === "RetroAchievements" &&
      typeof (a as { externalId?: unknown }).externalId === "string",
  );
  return {
    username: ra && ra.externalId ? ra.externalId : null,
    hasConnectToken: !(ra && (ra as { hasConnectToken?: unknown }).hasConnectToken === false),
    // Older servers don't send the flag and always required the key.
    apiKeyRequired: o.raApiKeyRequired !== false,
  };
}

/**
 * The single word the RA settings screens build their copy on.
 * - `linked`: the server has the account and this device's copy (if any)
 *   is the same account; unlocks are tracked.
 * - `mismatch`: the server has an account but this device still holds a
 *   different one (re-linked elsewhere and the device couldn't refresh yet).
 *   RetroArch would sign in as the old one until it refreshes.
 * - `device_only`: this device has a sign-in from an older build, but the
 *   server has no account, so nothing is recorded until the player signs in
 *   again.
 * - `expired`: RetroArch's token was rejected; sign in again.
 * - `needs_signin`: the server has the account but no RetroArch sign-in for
 *   it (linked before those were stored); sign in again once.
 * - `none`: nothing linked anywhere.
 */
export type RaLinkState =
  | "linked"
  | "mismatch"
  | "device_only"
  | "expired"
  | "needs_signin"
  | "none";

export function raLinkState(input: {
  serverUsername: string | null;
  /** Defaults to true (older servers don't report it). */
  serverHasConnectToken?: boolean;
  localUsername?: string;
  localHasToken: boolean;
  localExpired: boolean;
}): RaLinkState {
  if (input.localExpired) return "expired";
  if (input.serverUsername) {
    const local = (input.localUsername ?? "").trim().toLowerCase();
    const sameAccount = local === input.serverUsername.trim().toLowerCase();
    // The server lost (or never had) the Connect token, but this device still
    // holds a working one for the same account, which the client keeps using
    // (sync_ra_credentials), so RetroArch does sign in.
    if (input.serverHasConnectToken === false) {
      return input.localHasToken && sameAccount ? "linked" : "needs_signin";
    }
    if (input.localHasToken && local && local !== input.serverUsername.trim().toLowerCase()) {
      return "mismatch";
    }
    return "linked";
  }
  if (input.localHasToken) return "device_only";
  return "none";
}

/** `ra_refresh_credentials` (Rust `RaSyncReport`). */
export interface RaSyncReport {
  status: "linked" | "linked_no_token" | "not_linked" | "unreachable";
  serverUsername: string | null;
  signsInAs: string | null;
  error: string | null;
}

/** Text for a sync that couldn't change anything, or null when it worked. */
export function raSyncProblemText(report: RaSyncReport | null): string | null {
  if (!report) return "Could not update this device.";
  if (report.status === "unreachable") {
    return `Could not reach your server to update this device${report.error ? `: ${report.error}` : "."}`;
  }
  return null;
}
