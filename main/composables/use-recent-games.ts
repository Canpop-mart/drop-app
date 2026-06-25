/**
 * Playtime formatting helpers. Originally lived alongside a
 * `useRecentGames()` composable that fed the (since-removed) standalone
 * home dashboard. The composable is gone but the format helpers are still
 * used by the community feed and profile page, so the file stays around
 * for them. Module name kept for import stability.
 */

/** Format playtime as e.g. "23 min" or "12.4 hours". */
export function formatPlaytime(seconds: number): string {
  if (seconds < 60) return "< 1 min";
  const hours = seconds / 3600;
  if (hours >= 1) {
    const rounded = Math.round(hours * 10) / 10;
    return `${rounded} ${rounded === 1 ? "hour" : "hours"}`;
  }
  return `${Math.round(seconds / 60)} min`;
}

/** Format a "last played" timestamp as relative ("2h ago") or short date. */
export function formatLastPlayed(iso: string): string {
  if (!iso) return "Never";
  try {
    const d = new Date(iso);
    const elapsedMs = Date.now() - d.getTime();
    const elapsedMins = Math.floor(elapsedMs / 60_000);
    if (elapsedMins < 1) return "just now";
    if (elapsedMins < 60) return `${elapsedMins}m ago`;
    const hours = Math.floor(elapsedMins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}d ago`;
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: days > 365 ? "numeric" : undefined,
    });
  } catch {
    return iso;
  }
}
