/**
 * Activity feed clustering — folds a person's whole play session in one game
 * into a single row, so a two-hour session that trickles out achievements
 * doesn't bury the feed under a dozen near-identical rows.
 *
 * Cluster rules:
 *   - Same user + same game collapse into ONE session row, even when other
 *     people's activity is interleaved between them in the feed.
 *   - A session ends when that user+game goes quiet for longer than
 *     SESSION_GAP_MS; a later burst starts a fresh row.
 *   - Session playtime sums; achievements dedupe and accumulate.
 *   - Requests never cluster — they're punctuation, not a session.
 */

import type { CommunityActivityItem } from "~/composables/use-server-api";

export interface ActivityCluster {
  key: string;
  kind: "session-cluster" | "request";
  timestamp: string;
  user: CommunityActivityItem["user"];
  game: CommunityActivityItem["game"] | null;
  /** Sum of session durations (seconds). */
  totalDuration: number;
  /** Inline achievement chips. */
  achievements: Array<{ id: string; title: string }>;
  /** For request rows. */
  request?: { id: string; title: string };
}

// A user's activity in one game stays a single session row until they go quiet
// for longer than this, at which point a later burst becomes a new row. Two
// hours comfortably spans a normal play session (achievements trickle out with
// gaps) while still splitting genuinely separate sittings.
const SESSION_GAP_MS = 2 * 60 * 60 * 1000;

/**
 * The feed arrives newest-first. We keep one "open" session per user+game and
 * fold each older event into it while the gap stays under SESSION_GAP_MS —
 * looking the session up by key (not just the previous row) so interleaved
 * activity from other people doesn't split it.
 */
export function clusterActivity(
  activity: CommunityActivityItem[],
): ActivityCluster[] {
  const clusters: ActivityCluster[] = [];
  const open = new Map<
    string,
    { cluster: ActivityCluster; oldestTs: number }
  >();

  for (const item of activity) {
    // Requests never cluster — they're punctuation, not a session.
    if (item.type === "request") {
      clusters.push({
        key: `req-${item.data.request?.id ?? item.timestamp}`,
        kind: "request",
        timestamp: item.timestamp,
        user: item.user,
        game: item.game ?? null,
        totalDuration: 0,
        achievements: [],
        request: item.data.request,
      });
      continue;
    }

    const itemTs = new Date(item.timestamp).getTime();
    const gameId = item.game?.id ?? "no-game";
    const key = `${item.user.id}::${gameId}`;
    const entry = open.get(key);

    // Fold into the open session unless the quiet gap since its oldest event
    // so far exceeds the session window (missing timestamps never split).
    const withinSession =
      !!entry &&
      (Number.isNaN(itemTs) ||
        Number.isNaN(entry.oldestTs) ||
        entry.oldestTs - itemTs <= SESSION_GAP_MS);

    if (entry && withinSession) {
      const c = entry.cluster;
      if (item.type === "session" && item.data.duration) {
        c.totalDuration += item.data.duration;
      }
      if (item.type === "achievement" && item.data.achievement) {
        // Guard against the feed ever emitting the same unlock twice.
        if (!c.achievements.some((a) => a.id === item.data.achievement!.id)) {
          c.achievements.push({
            id: item.data.achievement.id,
            title: item.data.achievement.title,
          });
        }
      }
      if (!Number.isNaN(itemTs)) {
        entry.oldestTs = Math.min(entry.oldestTs, itemTs);
      }
      continue;
    }

    // Start a new session row — this is its newest event.
    const cluster: ActivityCluster = {
      key: `${item.user.id}-${gameId}-${item.timestamp}`,
      kind: "session-cluster",
      timestamp: item.timestamp,
      user: item.user,
      game: item.game ?? null,
      totalDuration: item.type === "session" ? (item.data.duration ?? 0) : 0,
      achievements:
        item.type === "achievement" && item.data.achievement
          ? [
              {
                id: item.data.achievement.id,
                title: item.data.achievement.title,
              },
            ]
          : [],
    };
    clusters.push(cluster);
    open.set(key, { cluster, oldestTs: Number.isNaN(itemTs) ? 0 : itemTs });
  }

  return clusters;
}
