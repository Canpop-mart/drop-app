export interface TrackingRecord {
  description?: string;
  iconUrl?: string;
  achievementId: string | null;
  title: string;
  disposition: string;
  reason: string | null;
  event: {
    externalId: string;
    provider: string;
    providerGameId: string;
    providerAccount: string;
    source: string;
    mode: string;
    kind: string;
    current: number | null;
    target: number | null;
    observedAt: string;
  };
}

export interface TrackingView {
  popupSupported: boolean;
  popupEnabled: boolean;
  journalErrors: string[];
  remoteStates: {
    achievementId: string;
    title: string;
    mode: string;
    unlocked: boolean;
    confirmed: boolean;
  }[];
  status: {
    active: boolean;
    sources: string[];
    lastReadAt: string | null;
    lastSyncAt: string | null;
    error: string | null;
    syncError: string | null;
    popupError: string | null;
  } | null;
  records: TrackingRecord[];
  pending: number;
}

export function overlayLocalUnlocks<
  T extends { id: string; unlocked: boolean },
>(achievements: T[], records: TrackingRecord[]): T[] {
  const unlocked = new Set(
    records
      .filter(
        (r) =>
          r.event.kind === "unlock" &&
          r.event.provider !== "RetroAchievements" &&
          !["before_reset", "superseded", "invalid"].includes(r.disposition),
      )
      .map((r) => r.achievementId),
  );
  return achievements.map((a) =>
    unlocked.has(a.id) ? { ...a, unlocked: true } : a,
  );
}

/** Scope keys retain both account and provider identity, even when API names overlap. */
export function latestTrackingRows(
  records: TrackingRecord[],
): TrackingRecord[] {
  const latest = new Map<string, TrackingRecord>();
  for (const record of records) {
    if (["before_reset", "superseded"].includes(record.disposition)) continue;
    const e = record.event;
    const key = JSON.stringify([
      e.provider,
      e.providerGameId,
      e.providerAccount,
      e.externalId,
      e.mode,
      e.kind,
    ]);
    const previous = latest.get(key);
    if (
      !previous ||
      Date.parse(e.observedAt) >= Date.parse(previous.event.observedAt)
    )
      latest.set(key, record);
  }
  return [...latest.values()];
}

export function progressPercent(
  current: number | null,
  target: number | null,
): number | null {
  if (
    current == null ||
    target == null ||
    !Number.isFinite(current) ||
    !Number.isFinite(target) ||
    target <= 0
  )
    return null;
  return Math.max(0, Math.min(100, (current / target) * 100));
}

export function trackingReason(reason: string): string {
  const reasons: Record<string, string> = {
    definition_missing:
      "The server has no definition for this achievement. Your unlock is saved.",
    game_link_mismatch:
      "This install does not match the server's achievement set.",
    provider_account_mismatch:
      "The linked achievement account does not match this session.",
    invalid_unlock_time: "The source reported an invalid unlock time.",
    invalid_observation_time: "The recorded observation time is invalid.",
    client_clock_ahead: "This device's clock is ahead of the server.",
    invalid_progress: "The source reported invalid progress values.",
    invalid_mode: "The source reported an unsupported achievement mode.",
    event_identity_conflict:
      "This achievement event conflicts with a saved server event.",
  };
  return reasons[reason] ?? reason;
}
