import { listen } from "@tauri-apps/api/event";
import type { DownloadableMetadata } from "~/types";

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
  let unlistenQueue: (() => void) | undefined;
  let unlistenStats: (() => void) | undefined;

  onMounted(async () => {
    unlistenQueue = await listen("update_queue", (event) => {
      const queue = useQueueState();
      const completed = useCompletedDownloads();
      const prev = queue.value;
      const next = event.payload as QueueState;

      // Detect items that were in the previous queue but are no longer present
      // — these have completed (or been cancelled, but we treat them as done).
      if (prev.queue.length > 0) {
        const nextIds = new Set(next.queue.map((q) => q.meta.id));
        for (const item of prev.queue) {
          if (
            !nextIds.has(item.meta.id) &&
            !completed.value.some((c) => c.gameId === item.meta.id)
          ) {
            completed.value = [
              { gameId: item.meta.id, completedAt: Date.now() },
              ...completed.value,
            ].slice(0, 50); // Keep last 50
          }
        }
      }

      queue.value = next;
    });

    unlistenStats = await listen("update_stats", (event) => {
      const stats = useStatsState();
      stats.value = event.payload as StatsState;
    });
  });

  onUnmounted(() => {
    unlistenQueue?.();
    unlistenStats?.();
  });
}

export const useDownloadHistory = () =>
  useState<Array<number>>("history", () => []);

export type CompletedDownload = {
  gameId: string;
  completedAt: number; // Unix ms timestamp
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
