/**
 * Data layer for the BPM home screen (`pages/bigpicture/index.vue`).
 *
 * Owns the "recently played" list and the spotlight pick:
 *  - `loadRecentGames()` fetches `playtime/recent`, resolves each game's
 *    install status, and builds the `RecentGameEntry[]` the home tiles use.
 *  - `pickRandomFavoriteSpotlight()` swaps the hero spotlight to a random
 *    favorited game (falling back to a random recent entry) so the home
 *    screen feels fresh on every visit.
 *
 * Decomposed out of `index.vue` (was 2124 lines) — this is the page's
 * DOM-free data concern. All theme-specific focus tracking and the
 * per-console markup stay in the page.
 *
 * Per-page composable: NOT a singleton — call from the page `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import { devLog } from "~/composables/dev-mode";
import { useGame, parseStatus } from "~/composables/game";
import { useAppState } from "~/composables/app-state";
import { serverUrl } from "~/composables/use-server-fetch";
import type { Game, GameStatus, RawGameStatus } from "~/types";

export interface RecentGameEntry {
  game: Game;
  status: GameStatus;
  // Cached at load time so templates don't have to type-narrow the
  // discriminated GameStatus union every time. `playtimeSeconds` comes
  // straight from the playtime/recent endpoint, not from `status` —
  // GameStatus tracks install/launch state, not history.
  installed: boolean;
  playtimeSeconds: number;
}

interface RecentGameResponse {
  gameId: string;
  gameName: string;
  coverObjectId: string | null;
  lastPlayedAt: string;
  totalPlaytimeSeconds: number;
}

/** Format playtime compactly for a home tile ("45m", "12h"). */
export function formatHomePlaytime(seconds: number): string {
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  return `${Math.round(seconds / 3600)}h`;
}

export function useBpmHomeData() {
  const appState = useAppState();

  const recentGames = ref<RecentGameEntry[]>([]);
  const loading = ref(true);

  // Names/covers for any queued (downloading) games not in the recent list.
  const gameNames = ref<Record<string, { name: string; coverUrl?: string }>>(
    {},
  );

  // Spotlight: a random favorited game, falling back to the most-recent.
  const spotlightOverride = ref<RecentGameEntry | null>(null);
  const spotlightGame = computed(
    () => spotlightOverride.value ?? recentGames.value[0] ?? null,
  );
  const otherGames = computed(() => {
    const spotlightId = spotlightGame.value?.game.id;
    if (!spotlightId) return recentGames.value.slice(1);
    return recentGames.value.filter((e) => e.game.id !== spotlightId);
  });
  const installedGames = computed(() =>
    recentGames.value.filter((e) => e.installed),
  );

  /**
   * Fetch the recently-played list and resolve each game's install status.
   * `queue` is passed in so names/covers for queued downloads can be
   * resolved in the same pass.
   */
  async function loadRecentGames(queue: { meta: { id: string } }[]) {
    try {
      const url = serverUrl("api/v1/client/playtime/recent");
      devLog("state", "[BPM:HOME] Fetching recent games from:", url);
      const response = await fetch(url);
      if (!response.ok) {
        console.error(
          "[BPM:HOME] Recent games fetch failed:",
          response.status,
          response.statusText,
        );
        recentGames.value = [];
        return;
      }
      const recentData = (await response.json()) as RecentGameResponse[];

      if (!Array.isArray(recentData)) {
        console.warn(
          "[BPM:HOME] Recent games response is not an array:",
          typeof recentData,
        );
        recentGames.value = [];
        return;
      }

      const gamesToLoad = recentData.slice(0, 20);
      const entries: RecentGameEntry[] = [];

      for (const gameData of gamesToLoad) {
        try {
          const statusData: RawGameStatus = await invoke("fetch_game_status", {
            id: gameData.gameId,
          });
          // The playtime/recent payload carries only id/name/cover, but the
          // home page's tiles only ever read those three fields plus
          // playtime/installed (cached on the entry below). Cast through
          // `unknown` to make the partial-Game shape explicit — a fuller
          // Game would require a second fetch per tile we don't need.
          const game = {
            id: gameData.gameId,
            mName: gameData.gameName,
            mCoverObjectId: gameData.coverObjectId,
            mTaglineUrl: null,
            mReleaseDate: null,
            mPlatformId: null,
            mSummary: null,
            mBackgroundUrl: null,
            mPublisher: null,
            mGenre: null,
          } as unknown as Game;

          const status = parseStatus(statusData);
          entries.push({
            game,
            status,
            installed: status.type === "Installed",
            playtimeSeconds: gameData.totalPlaytimeSeconds,
          });
        } catch (e) {
          console.error(`Failed to load recent game ${gameData.gameId}:`, e);
        }
      }

      recentGames.value = entries;

      for (const item of queue) {
        if (!gameNames.value[item.meta.id]) {
          try {
            const gameFetch = await useGame(item.meta.id);
            gameNames.value[item.meta.id] = {
              name: gameFetch.game.mName,
              coverUrl: gameFetch.game.mCoverObjectId
                ? serverUrl(`api/v1/object/${gameFetch.game.mCoverObjectId}`)
                : undefined,
            };
          } catch {
            // Game data not available
          }
        }
      }
    } catch (e) {
      console.error("[BPM:HOME] Failed to fetch recent games:", e);
    } finally {
      loading.value = false;
    }
  }

  /**
   * Swap the spotlight to a random favorited game. Prefers favorites that
   * aren't already the most-recent entry (so the spotlight visibly
   * changes); falls back to any favorite, then to a random non-top recent.
   */
  async function pickRandomFavoriteSpotlight() {
    if (!recentGames.value.length) {
      devLog("state", "[BPM:HOME] Spotlight: no recent games, skipping");
      return;
    }

    const topId = recentGames.value[0]?.game.id;

    try {
      const userId = appState.value?.user?.id;
      if (userId) {
        const resp = await fetch(serverUrl(`api/v1/user/${userId}/showcase`));
        if (resp.ok) {
          const data: {
            items?: Array<{ type: string; gameId: string | null }>;
          } = await resp.json();
          const favoriteIds = new Set(
            (data.items ?? [])
              .filter((i) => i.type === "FavoriteGame" && i.gameId)
              .map((i) => i.gameId as string),
          );
          const favCandidates = recentGames.value.filter((e) =>
            favoriteIds.has(e.game.id),
          );
          // Prefer favorites that aren't already the most-recent entry.
          const nonTopFavs = favCandidates.filter((e) => e.game.id !== topId);
          const pool = nonTopFavs.length > 0 ? nonTopFavs : favCandidates;
          if (pool.length) {
            const pick = pool[Math.floor(Math.random() * pool.length)];
            spotlightOverride.value = pick;
            devLog(
              "state",
              `[BPM:HOME] Spotlight: picked favorite → ${pick.game.mName}`,
            );
            return;
          }
        } else {
          console.warn(
            "[BPM:HOME] Spotlight: showcase fetch failed:",
            resp.status,
          );
        }
      }
    } catch (e) {
      console.warn("[BPM:HOME] Spotlight: showcase fetch error:", e);
    }

    // Fallback: random recent game, excluding index 0 so the spotlight
    // actually changes (Math.random() landing on 0 would be a no-op).
    if (recentGames.value.length > 1) {
      const idx = 1 + Math.floor(Math.random() * (recentGames.value.length - 1));
      spotlightOverride.value = recentGames.value[idx];
    }
  }

  return {
    recentGames,
    loading,
    gameNames,
    spotlightOverride,
    spotlightGame,
    otherGames,
    installedGames,
    loadRecentGames,
    pickRandomFavoriteSpotlight,
  };
}
