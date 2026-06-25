/**
 * Activity feed clustering — folds consecutive events from the same user
 * in the same game into a single row. Without this, a 20-minute play
 * session that unlocks two achievements shows up as three rows
 * (the play session + each achievement), which is visually noisy
 * for a 6-user community.
 *
 * Cluster rules:
 *   - Same user
 *   - Same game (or both `request` events with no game match)
 *   - Events within 10 minutes of each other
 *   - Sessions and achievements can merge; requests stay solo
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

const TEN_MIN_MS = 10 * 60 * 1000;

/**
 * Activity is already sorted newest-first from the server. We walk forward,
 * folding subsequent (older) events into a running cluster while they
 * match the same user + same game within the time window.
 */
export function clusterActivity(
  activity: CommunityActivityItem[],
): ActivityCluster[] {
  const clusters: ActivityCluster[] = [];

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

    const head = clusters[clusters.length - 1];
    const sameUser = head && head.user.id === item.user.id;
    const sameGame =
      head && head.game && item.game && head.game.id === item.game.id;
    const withinWindow =
      head &&
      Math.abs(
        new Date(head.timestamp).getTime() - new Date(item.timestamp).getTime(),
      ) <= TEN_MIN_MS;

    if (
      head &&
      head.kind === "session-cluster" &&
      sameUser &&
      sameGame &&
      withinWindow
    ) {
      // Fold into existing cluster
      if (item.type === "session" && item.data.duration) {
        head.totalDuration += item.data.duration;
      }
      if (item.type === "achievement" && item.data.achievement) {
        // Avoid duplicate achievements if the feed ever emits the same one twice
        if (!head.achievements.some((a) => a.id === item.data.achievement!.id)) {
          head.achievements.push({
            id: item.data.achievement.id,
            title: item.data.achievement.title,
          });
        }
      }
      // Keep the newest timestamp on the cluster (already newest, since head)
      continue;
    }

    // Start a new cluster
    clusters.push({
      key: `${item.user.id}-${item.game?.id ?? "no-game"}-${item.timestamp}`,
      kind: "session-cluster",
      timestamp: item.timestamp,
      user: item.user,
      game: item.game ?? null,
      totalDuration:
        item.type === "session" ? (item.data.duration ?? 0) : 0,
      achievements:
        item.type === "achievement" && item.data.achievement
          ? [
              {
                id: item.data.achievement.id,
                title: item.data.achievement.title,
              },
            ]
          : [],
    });
  }

  return clusters;
}
