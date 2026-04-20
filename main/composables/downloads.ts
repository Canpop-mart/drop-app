import type { DownloadableMetadata } from "~/types";
import { useListen } from "./useListen";

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
  useListen<QueueState>("update_queue", (event) => {
    const queue = useQueueState();
    queue.value = event.payload;
  });

  // Backend emits this ONLY on real completion, never on cancel. This is the
  // signal the UI uses to promote a game to "Recently Completed".
  useListen<string>("download_complete", (event) => {
    const completed = useCompletedDownloads();
    const gameId = event.payload;
    if (!completed.value.some((c) => c.gameId === gameId)) {
      completed.value = [
        { gameId, completedAt: Date.now() },
        ...completed.value,
      ].slice(0, 50);
    }
  });

  useListen<StatsState>("update_stats", (event) => {
    const stats = useStatsState();
    stats.value = event.payload;
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
