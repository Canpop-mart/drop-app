import type { DownloadableMetadata } from "~/types";
import { useListen } from "./useListen";
import { devLog } from "./dev-mode";
import { clearUpdating, isUpdating } from "./update-tracking";

export type QueueState = {
  queue: Array<{
    meta: DownloadableMetadata;
    status: string;
    dl_progress: number | null;
    dl_current: number;
    dl_max: number;
    disk_progress: number | null;
    disk_current: number;
    disk_max: number;
  }>;
  status: string;
};

export type StatsState = {
  speed: number; // Bytes per second
  time: number; // Seconds,
};

export const useQueueState = () =>
  useState<QueueState>("queue", () => ({ queue: [], status: "Unknown" }));

export const useStatsState = () =>
  useState<StatsState>("stats", () => ({ speed: 0, time: 0 }));

export function useDownloadListeners() {
  // Backend emits update_queue / update_stats every ~100ms during an active
  // download (~10-20 lines/sec). The dev-mode firehose has a 5000-line ring
  // buffer in `layouts/bigpicture.vue`, so unthrottled logs would overwrite
  // every other category's output within a few minutes. Log only meaningful
  // transitions for the queue, and throttle stats to ~1Hz. (The earlier
  // "Rate-limited implicitly: backend throttles update_stats emission."
  // comment was aspirational, not true — the backend fires on every tick.)
  let lastQueueLen = -1;
  let lastQueueStatus = "";
  useListen<QueueState>("update_queue", (event) => {
    const queue = useQueueState();
    const prev = queue.value.queue.length;
    queue.value = event.payload;
    const nextLen = event.payload.queue.length;
    const nextStatus = event.payload.status;
    if (nextLen !== lastQueueLen || nextStatus !== lastQueueStatus) {
      devLog(
        "download",
        `queue: ${prev} -> ${nextLen} items, status="${nextStatus}"`,
      );
      lastQueueLen = nextLen;
      lastQueueStatus = nextStatus;
    }
  });

  // Backend emits this ONLY on real completion, never on cancel. This is the
  // signal the UI uses to promote a game to "Recently Completed".
  useListen<string>("download_complete", (event) => {
    const completed = useCompletedDownloads();
    const gameId = event.payload;
    // An in-place update completes through the same event; remember which it
    // was so the history and the toast don't call it an install.
    const kind: CompletedDownload["kind"] = isUpdating(gameId)
      ? "update"
      : "install";
    clearUpdating(gameId);
    devLog("download", `complete: ${gameId} (${kind})`);
    // One entry per game, the newest first. An update of a game installed
    // earlier in the session replaces that entry rather than being dropped,
    // so it still shows (and the Big Picture toast still fires).
    completed.value = [
      { gameId, completedAt: Date.now(), kind },
      ...completed.value.filter((c) => c.gameId !== gameId),
    ].slice(0, 50);
  });

  // A failed download is the one at the front of the queue (the manager
  // reports the error before taking it off). If that was an update, it is no
  // longer one: without this a later fresh install of the same game would be
  // labelled as an update. The payload is only the error text.
  useListen<string>("download_error", () => {
    const front = useQueueState().value.queue.at(0)?.meta.id;
    if (front) clearUpdating(front);
  });

  // Throttle stats logs to at most one per second. State updates still go
  // through every tick — only the dev-log fires on the throttled cadence.
  let lastStatsLogAt = 0;
  useListen<StatsState>("update_stats", (event) => {
    const stats = useStatsState();
    stats.value = event.payload;
    const now = Date.now();
    if (now - lastStatsLogAt >= 1000) {
      devLog(
        "download",
        `stats: ${event.payload.speed} B/s, eta=${event.payload.time}s`,
      );
      lastStatsLogAt = now;
    }
  });
}

export const useDownloadHistory = () =>
  useState<Array<number>>("history", () => []);

export type CompletedDownload = {
  gameId: string;
  completedAt: number; // Unix ms timestamp
  /** An in-place update rather than an install. Absent on older entries. */
  kind?: "install" | "update";
};

export const useCompletedDownloads = () =>
  useState<CompletedDownload[]>("completed_downloads", () => []);

export function formatKilobytes(bytes: number): string {
  const units = ["K", "M", "G", "T", "P"];
  let value = bytes;
  let unitIndex = 0;
  const scalar = 1000;

  while (value >= scalar && unitIndex < units.length - 1) {
    value /= scalar;
    unitIndex++;
  }

  return `${value.toFixed(1)} ${units[unitIndex]}`;
}
