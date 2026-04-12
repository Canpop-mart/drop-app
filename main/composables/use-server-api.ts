/**
 * Server API composable for fetching data from the Drop server
 * via the server:// protocol (Tauri proxy with auto-auth).
 *
 * Phase 6 — provides typed fetch wrappers for all BPM-relevant endpoints.
 */

import { serverUrl } from "./use-server-fetch";

// ── Store types ─────────────────────────────────────────────────────────────

export interface StoreGame {
  id: string;
  mName: string;
  mShortDescription: string;
  mCoverObjectId: string;
  mBannerObjectId: string;
  mIconObjectId: string;
  mReleased?: string | null;
  updateAvailable?: boolean | null;
  developers?: Array<{ id: string; mName: string }>;
  publishers?: Array<{ id: string; mName: string }>;
  tags?: Array<{ id: string; name: string }>;
  versions?: Array<{ displayName?: string | null; versionIndex?: number }>;
}

export interface TrendingGame extends StoreGame {
  recentPlayers: number;
}

export interface StoreSearchResult {
  results: StoreGame[];
  count: number;
}

export interface StoreTag {
  id: string;
  name: string;
}

// ── Community types ─────────────────────────────────────────────────────────

export interface CommunityStats {
  totalGames: number;
  totalUsers: number;
  totalPlaytimeHours: number;
  totalPlaySessions: number;
  totalAchievementUnlocks: number;
  totalRequests: number;
  pendingRequests: number;
  totalLeaderboardEntries: number;
}

export interface CommunityActivityItem {
  type: "session" | "achievement" | "request";
  timestamp: string;
  user: {
    id: string;
    username: string;
    displayName: string;
    profilePictureObjectId: string;
  };
  game: {
    id: string;
    mName: string;
    mIconObjectId: string;
    mCoverObjectId: string;
  };
  data: {
    duration?: number;
    endedAt?: string;
    achievement?: {
      id: string;
      title: string;
      description: string;
      iconUrl: string;
    };
    request?: {
      id: string;
      title: string;
    };
  };
}

export interface LeaderboardUser {
  rank: number;
  user: {
    id: string;
    username: string;
    displayName: string;
    profilePictureObjectId: string;
  };
  playtimeHours: number;
  gamesPlayed: number;
  achievements: number;
  gamesOwned: number;
}

// ── Profile types ──────────────────────────────────────────────────────────

export interface UserProfile {
  id: string;
  username: string;
  displayName: string;
  profilePictureObjectId?: string;
  bannerObjectId?: string;
  bio?: string;
  profileTheme?: string;
}

export interface UserStats {
  totalPlaytimeSeconds: number;
  gamesPlayed: number;
  achievementsUnlocked: number;
  recentSessions: Array<{
    id: string;
    gameId: string;
    startedAt: string;
    endedAt: string | null;
    durationSeconds: number | null;
    lastHeartbeatAt: string | null;
    game: {
      id: string;
      mName: string;
      mIconObjectId?: string;
      mCoverObjectId?: string;
    } | null;
  }>;
}

export interface UserActivity {
  sessions: Array<{
    id: string;
    gameId: string;
    startedAt: string;
    endedAt: string | null;
    durationSeconds: number | null;
    lastHeartbeatAt: string | null;
    game: {
      id: string;
      mName: string;
      mIconObjectId?: string;
      mCoverObjectId?: string;
    } | null;
  }>;
  achievements: Array<{
    id: string;
    userId: string;
    achievementId: string;
    unlockedAt: string;
    achievement: {
      id: string;
      title: string;
      description: string;
      iconUrl: string;
      gameId: string;
    } | null;
    game: {
      id: string;
      mName: string;
      mIconObjectId?: string;
    } | null;
  }>;
}

export interface ShowcaseItem {
  id: string;
  type: "FavoriteGame" | "Achievement" | "Review" | "GameStats" | "Custom";
  gameId: string | null;
  itemId: string | null;
  title: string;
  data: unknown;
  sortOrder: number;
  game: {
    id: string;
    mName: string;
    mIconObjectId?: string;
    mCoverObjectId?: string;
    mBannerObjectId?: string;
  } | null;
  achievement?: {
    id: string;
    title: string;
    description: string;
    iconUrl: string;
  } | null;
  gameStats?: {
    playtimeSeconds: number;
    achievementsUnlocked: number;
    achievementsTotal: number;
  };
}

export interface UserShowcase {
  items: ShowcaseItem[];
}

// ── News types ──────────────────────────────────────────────────────────────

export interface NewsArticle {
  id: string;
  title: string;
  description: string;
  content: string;
  publishedAt: string;
  imageObjectId?: string | null;
  authorId?: string | null;
  author?: {
    id: string;
    displayName: string;
  };
  tags?: Array<{ id: string; name: string }>;
}

// ── Fetch helper ────────────────────────────────────────────────────────────

async function apiFetch<T>(path: string): Promise<T> {
  const url = serverUrl(path);
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`API ${path} failed: ${res.status} ${res.statusText}`);
  }
  return res.json();
}

// ── Store API ───────────────────────────────────────────────────────────────

export function useServerApi() {
  return {
    store: {
      /** Get featured games for the hero carousel. */
      featured: () => apiFetch<StoreGame[]>("api/v1/store/featured"),

      /** Get trending games. */
      trending: (take = 10, days = 7) =>
        apiFetch<{ results: TrendingGame[] }>(
          `api/v1/store/trending?take=${take}&days=${days}`,
        ),

      /** Search/browse the store. */
      /** List all libraries (for filter UI). */
      libraries: () =>
        apiFetch<Array<{ id: string; name: string }>>(
          "api/v1/store/libraries",
        ),

      /** Search/browse the store. */
      browse: (params: {
        skip?: number;
        take?: number;
        q?: string;
        tags?: string;
        platform?: string;
        library?: string;
        sort?: "default" | "newest" | "recent" | "name" | "relevance" | "random";
        order?: "asc" | "desc";
      } = {}) => {
        const qs = new URLSearchParams();
        if (params.skip) qs.set("skip", String(params.skip));
        if (params.take) qs.set("take", String(params.take));
        if (params.q) qs.set("q", params.q);
        if (params.tags) qs.set("tags", params.tags);
        if (params.platform) qs.set("platform", params.platform);
        if (params.library) qs.set("library", params.library);
        if (params.sort) qs.set("sort", params.sort);
        if (params.order) qs.set("order", params.order);
        const query = qs.toString();
        return apiFetch<StoreSearchResult>(
          `api/v1/store${query ? `?${query}` : ""}`,
        );
      },

      /** Get all available tags. */
      tags: () => apiFetch<StoreTag[]>("api/v1/store/tags"),
    },

    community: {
      stats: () => apiFetch<CommunityStats>("api/v1/community/stats"),
      activity: (limit = 30, before?: string) => {
        const qs = new URLSearchParams({ limit: String(limit) });
        if (before) qs.set("before", before);
        return apiFetch<CommunityActivityItem[]>(
          `api/v1/community/activity?${qs}`,
        );
      },
      leaderboard: () =>
        apiFetch<{ playtime: LeaderboardUser[] }>(
          "api/v1/community/leaderboard",
        ),
    },

    profile: {
      /** Get user profile info. */
      get: (id: string) => apiFetch<UserProfile>(`api/v1/user/${id}`),

      /** Get user stats (playtime, games played, achievements, recent sessions). */
      stats: (id: string) => apiFetch<UserStats>(`api/v1/user/${id}/stats`),

      /** Get user activity (sessions + achievements). */
      activity: (id: string) =>
        apiFetch<UserActivity>(`api/v1/user/${id}/activity`),

      /** Get user showcase items. */
      showcase: (id: string) =>
        apiFetch<UserShowcase>(`api/v1/user/${id}/showcase`),
    },

    news: {
      list: (params: {
        limit?: number;
        skip?: number;
        order?: "asc" | "desc";
        tags?: string;
        search?: string;
      } = {}) => {
        const qs = new URLSearchParams();
        if (params.limit) qs.set("limit", String(params.limit));
        if (params.skip) qs.set("skip", String(params.skip));
        if (params.order) qs.set("order", params.order);
        if (params.tags) qs.set("tags", params.tags);
        if (params.search) qs.set("search", params.search);
        const query = qs.toString();
        return apiFetch<NewsArticle[]>(
          `api/v1/news${query ? `?${query}` : ""}`,
        );
      },
      get: (id: string) => apiFetch<NewsArticle>(`api/v1/news/${id}`),
    },
  };
}
