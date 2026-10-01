/**
 * Read-side server data for the library game-detail page: the stats bar,
 * the achievement list, and RetroAchievements ROM-hash verification.
 *
 * Extracted from `pages/library/[id]/index.vue` (was ~330 lines of inline
 * `onMounted` fetches and refs). All three concerns are non-critical server
 * reads: an endpoint being down must never blank the page. The stats bar
 * soft-fails to zeroes; the achievement list records its failure in
 * `achievementsError` so the UI can show an error with a retry instead of the
 * "no achievements" empty state, and `achievementStatus` says WHY a game has
 * none (or can't record unlocks for this player).
 *
 * Per-game-detail composable: NOT a singleton. Each call wires fresh refs
 * and component-scoped `useListen` subscriptions, so it must be invoked
 * from a component `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import { useListen } from "~/composables/useListen";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  parseAchievementStatus,
  serverErrorText,
  type AchievementStatus,
} from "~/composables/achievements/status";

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
  /** Gamerscore-style points (RetroAchievements). 0/absent for Steam. */
  points?: number;
  /** Global unlock rarity % from RA/Steam (0-100). Null/absent = unknown. */
  globalPercent?: number | null;
  /** Per-instance unlock rarity % across this server's players. */
  rarity?: number;
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
  /** Set when the list could not be fetched; null after a good fetch. */
  const achievementsError = ref<string | null>(null);
  /** Why there are none / why unlocks can't record. Null until known. */
  const achievementStatus = ref<AchievementStatus | null>(null);
  /** The status route failed, so the reason is unknown (list may be fine). */
  const achievementStatusError = ref(false);
  const achievementsUnlocked = computed(
    () => achievements.value.filter((a) => a.unlocked).length,
  );

  async function loadStatus() {
    try {
      const res = await fetch(
        serverUrl(`api/v1/games/${gameId}/achievements/status`),
      );
      if (!res.ok) throw new Error(String(res.status));
      achievementStatus.value = parseAchievementStatus(await res.json());
      achievementStatusError.value = false;
    } catch (e) {
      console.warn("[ACH] achievement status fetch failed:", e);
      achievementStatusError.value = true;
    }
  }

  async function loadAchievements() {
    try {
      const res = await fetch(
        serverUrl(`api/v1/games/${gameId}/achievements`),
      );
      if (!res.ok) {
        let body: unknown = null;
        try {
          body = await res.json();
        } catch {
          // No JSON body; the status code carries the message.
        }
        throw new Error(serverErrorText(res.status, body));
      }
      const data = await res.json();
      // Server returns a plain array; tolerate a wrapped shape too.
      achievements.value = Array.isArray(data)
        ? data
        : (data.achievements ?? []);
      achievementsError.value = null;
    } catch (e: any) {
      // Keep whatever list is already shown (a refresh after an unlock can
      // fail too); the UI decides how loud to be based on the list length.
      achievementsError.value = String(e?.message ?? e);
    } finally {
      achievementsLoading.value = false;
    }
  }

  /** Retry both reads (the list and the reason). */
  async function retryAchievements() {
    achievementsLoading.value = achievements.value.length === 0;
    await Promise.all([loadAchievements(), loadStatus()]);
  }

  onMounted(() => {
    loadAchievements();
    loadStatus();
  });

  // Refresh when the backend reports a new unlock, so the list + progress
  // count update live instead of staying stale until you re-navigate (the
  // unlock toast already fires; this keeps the page itself in sync). The
  // event carries the gameId, but any unlock triggering a cheap refetch is
  // harmless, so it isn't filtered.
  useListen("achievement_unlocked", () => {
    loadAchievements();
  });

  const resetBusy = ref(false);
  const resetError = ref<string | null>(null);
  /** Extra line for the user after a reset (e.g. the game was running). */
  const resetNote = ref<string | null>(null);

  /**
   * Reset every achievement for this game server-side, then clear this
   * device's local save files so the next launch doesn't re-report them.
   * Returns success; on failure `resetError` says why.
   */
  async function resetAchievements(): Promise<boolean> {
    resetBusy.value = true;
    resetError.value = null;
    resetNote.value = null;
    try {
      const res = await fetch(
        serverUrl(`api/v1/user/achievements/reset?gameId=${gameId}`),
        { method: "DELETE" },
      );
      if (!res.ok) {
        let body: unknown = null;
        try {
          body = await res.json();
        } catch {
          // No JSON body; the status code carries the message.
        }
        resetError.value = serverErrorText(res.status, body);
        return false;
      }
      await res.json();
      achievements.value = achievements.value.map((a) => ({
        ...a,
        unlocked: false,
      }));
      try {
        const local = await invoke<{ status: string; failures: string[] }>(
          "clear_local_achievements",
          { gameId },
        );
        if (local.status === "running") {
          resetNote.value =
            "The game is running, so its local unlocks were kept. Reset again after closing it.";
        } else if (local.failures.length > 0) {
          resetNote.value =
            "Some local save files could not be cleared: " +
            local.failures.join("; ");
        }
      } catch (e: any) {
        resetNote.value = `Local save files were not cleared: ${String(e?.message ?? e)}`;
      }
      return true;
    } catch (e: any) {
      resetError.value = String(e?.message ?? e);
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
    achievementsError,
    achievementStatus,
    achievementStatusError,
    achievementsUnlocked,
    retryAchievements,
    resetBusy,
    resetError,
    resetNote,
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
