/**
 * Read-side server data for the library game-detail page: the stats bar,
 * the achievement list, and RetroAchievements ROM-hash verification.
 *
 * Extracted from `pages/library/[id]/index.vue` (was ~330 lines of inline
 * `onMounted` fetches and refs). All three concerns share one trait: they
 * are non-critical server reads that must soft-fail — a stats/achievements
 * endpoint being down should never blank the page, so every fetch swallows
 * its error and leaves an empty default.
 *
 * Per-game-detail composable: NOT a singleton. Each call wires fresh refs
 * and component-scoped `useListen` subscriptions, so it must be invoked
 * from a component `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import { useListen } from "~/composables/useListen";
import { serverUrl } from "~/composables/use-server-fetch";

export interface GameStatsData {
  playtimeSeconds: number;
  lastPlayedAt: string | null;
  achievementsUnlocked: number;
  achievementsTotal: number;
}

export interface AchievementData {
  id: string;
  title: string;
  description: string;
  iconUrl: string;
  unlocked: boolean;
}

/** Result of a RetroAchievements ROM-hash check (launch-time or on-demand). */
export interface RomHashResult {
  status: "Match" | "Mismatch" | "NoHashData" | "Error";
  rom_hash?: string;
  matched_label?: string;
  expected_hashes?: { hash: string; label: string; patchUrl: string }[];
  message?: string;
}

export function useGameStats(gameId: string) {
  // ── Stats bar ──────────────────────────────────────────────────────────
  const statsLoading = ref(true);
  const gameStats = reactive<GameStatsData>({
    playtimeSeconds: 0,
    lastPlayedAt: null,
    achievementsUnlocked: 0,
    achievementsTotal: 0,
  });

  onMounted(async () => {
    try {
      const res = await fetch(serverUrl(`api/v1/games/${gameId}/stats`));
      if (res.ok) {
        Object.assign(gameStats, await res.json());
      }
    } catch {
      // Stats are non-critical; silently fall back to the zeroed defaults.
    } finally {
      statsLoading.value = false;
    }
  });

  // ── Achievements ───────────────────────────────────────────────────────
  const achievements = ref<AchievementData[]>([]);
  const achievementsLoading = ref(true);
  const achievementsUnlocked = computed(
    () => achievements.value.filter((a) => a.unlocked).length,
  );

  async function loadAchievements() {
    try {
      const res = await fetch(
        serverUrl(`api/v1/games/${gameId}/achievements`),
      );
      if (res.ok) {
        const data = await res.json();
        // Server returns a plain array; tolerate a wrapped shape too.
        achievements.value = Array.isArray(data)
          ? data
          : (data.achievements ?? []);
      }
    } catch {
      achievements.value = [];
    } finally {
      achievementsLoading.value = false;
    }
  }

  onMounted(loadAchievements);

  // Refresh when the backend reports a new unlock, so the list + progress
  // count update live instead of staying stale until you re-navigate (the
  // unlock toast already fires; this keeps the page itself in sync). The
  // event carries no gameId, so any unlock triggers a (cheap) refetch.
  useListen("achievement_unlocked", () => {
    loadAchievements();
  });

  const resetBusy = ref(false);

  /** Reset every achievement for this game server-side. Returns success. */
  async function resetAchievements(): Promise<boolean> {
    resetBusy.value = true;
    try {
      const res = await fetch(
        serverUrl(`api/v1/user/achievements/reset?gameId=${gameId}`),
        { method: "DELETE" },
      );
      if (res.ok) {
        await res.json();
        achievements.value = achievements.value.map((a) => ({
          ...a,
          unlocked: false,
        }));
        return true;
      }
      return false;
    } catch {
      return false;
    } finally {
      resetBusy.value = false;
    }
  }

  // ── ROM hash verification (RetroAchievements) ──────────────────────────
  const romHashResult = ref<RomHashResult | null>(null);

  // Launch-time hash checks are pushed by the backend.
  useListen<RomHashResult>(`ra_hash_check/${gameId}`, (event) => {
    romHashResult.value = event.payload;
  });

  return {
    // Stats bar
    statsLoading,
    gameStats,
    // Achievements
    achievements,
    achievementsLoading,
    achievementsUnlocked,
    resetBusy,
    resetAchievements,
    // ROM hash
    romHashResult,
  };
}

// ── Formatting helpers (pure, exported for the header/stat-bar templates) ──

/** Format playtime as e.g. "< 1 min", "23 min", "12.4 hours". */
export function formatPlaytime(seconds: number): string {
  if (seconds < 60) return "< 1 min";
  const hours = seconds / 3600;
  if (hours >= 1) {
    const rounded = Math.round(hours * 10) / 10;
    return `${rounded} ${rounded === 1 ? "hour" : "hours"}`;
  }
  return `${Math.round(seconds / 60)} min`;
}

/** Format a "last played" timestamp as "Today" / "Yesterday" / short date. */
export function formatLastPlayed(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 30) return `${diffDays} days ago`;

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: date.getFullYear() !== now.getFullYear() ? "numeric" : undefined,
  });
}
