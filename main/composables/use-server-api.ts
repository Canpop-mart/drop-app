/**
 * Server API composable for fetching data from the Drop server
 * via the server:// protocol (Tauri proxy with auto-auth).
 *
 * Phase 6 — provides typed fetch wrappers for all BPM-relevant endpoints.
 */

import { invoke } from "@tauri-apps/api/core";
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
  isEmulated?: boolean;
  launchPlatform?: string | null;
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

// ── Store collection types ──────────────────────────────────────────────────

/** One curated collection in the store-home Collections list (lightweight). */
export interface StoreCollectionSummary {
  id: string;
  name: string;
  description: string | null;
  coverObjectId: string | null;
  gameCount: number;
}

/** A game as returned inside a store collection's detail payload. */
export interface StoreCollectionGame {
  id: string;
  mName: string;
  mShortDescription: string;
  mCoverObjectId: string;
  mBannerObjectId: string;
  mIconObjectId: string;
}

/** A store collection plus its games, for the collection landing page. */
export interface StoreCollectionDetail {
  id: string;
  name: string;
  description: string | null;
  coverObjectId: string | null;
  games: StoreCollectionGame[];
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

// Per-game community surfaces. Agent C owns the server endpoints
// (`/api/v1/community/game/:gameId/{players,activity,firsts}`). All three
// soft-fail at call sites — if any endpoint hasn't shipped yet, the UI
// degrades to an empty / "no data" state rather than an error.

export interface GamePlayerEntry {
  userId: string;
  displayName: string;
  avatarObjectId: string | null;
  playtimeSeconds: number;
  achievementsUnlocked: number;
  achievementsTotal: number;
}

export interface GameAchievementFirst {
  achievementId: string;
  achievementName: string;
  achievementIconUrl: string;
  userId: string;
  displayName: string;
  unlockedAt: string;
}

export interface NowPlayingEntry {
  userId: string;
  displayName: string;
  avatarObjectId: string | null;
  game: {
    id: string;
    name: string;
    coverObjectId: string | null;
  };
  startedAt: string;
}

/**
 * Weekly-recap slide shape. The server splits the old `subtitle` blob into
 * structured fields so the client can build a real hierarchy:
 *   - `title`           kicker label (small, uppercase)
 *   - `headline`        the *thing* the slide is about (game or player name)
 *   - `meta`            quieter supporting line under the headline
 *   - `coverObjectId`   game cover for the thumbnail slot (preferred)
 *   - `avatarObjectId`  player avatar — used when no cover is in scope
 *     (most_unlocks / milestone / new_player) or as a secondary signal.
 */
export interface WeeklyRecapSlide {
  kind: string;
  title: string;
  headline: string;
  meta: string;
  gameId: string | null;
  userId: string | null;
  coverObjectId: string | null;
  avatarObjectId: string | null;
}

// ── Games: per-game achievements ───────────────────────────────────────────

/**
 * One achievement row for a given game, with the caller's unlock state
 * baked in. Returned by `/api/v1/games/[id]/achievements` and consumed
 * by the store detail page's Achievements section.
 *
 * `iconUrl` / `iconLockedUrl` are raw CDN URLs (Steam / RetroAchievements)
 * — they are NOT object-store IDs, so render `<img :src="iconUrl">`
 * directly without going through `useObject` / `serverUrl`.
 */
export interface StoreAchievement {
  id: string;
  gameId: string;
  externalId: string;
  provider: string;
  title: string;
  description: string;
  iconUrl: string;
  iconLockedUrl: string;
  displayOrder: number;
  unlocked: boolean;
  unlockedAt: string | null;
  /** Percent of owners who have unlocked this achievement (server-wide). */
  rarity: number;
  /** Raw count of unlocks across all users. */
  unlockCount: number;
}

// ── Client: recent playtime ────────────────────────────────────────────────

/**
 * One entry per game the caller has ever played, capped at 20, ordered by
 * the most recent session's `startedAt` desc. Drives the library page's
 * "Continue playing" hero card and "Recently played" shelf.
 */
export interface RecentPlaytimeEntry {
  gameId: string;
  gameName: string;
  coverObjectId: string | null;
  lastPlayedAt: string;
  totalPlaytimeSeconds: number;
}

// ── Community: roulette & weekly-challenge ─────────────────────────────────

// `discover` was added when the roulette pool was widened to include EVERY
// game on the Drop store — not just the caller's installs. A discover pick
// is a game the caller doesn't own; `alsoPlayedBy` may still be populated
// if other server users have hours in it.
export type RouletteSource =
  | "rediscovery"
  | "library"
  | "social"
  | "discover";

export interface RouletteResult {
  game: {
    id: string;
    name: string;
    coverObjectId: string | null;
    bannerObjectId: string | null;
  };
  source: RouletteSource;
  /** Optional context for "social" and "discover" picks. */
  alsoPlayedBy?: Array<{
    userId: string;
    displayName: string;
    avatarObjectId: string | null;
  }>;
}

export type WeeklyChallengeKind =
  | "play_hours"
  | "unlock_count"
  | "play_variety"
  | "rediscover"
  | "marathon"
  | "night_owl"
  | "new_to_you"
  | "genre_focus"
  | "fresh_drop";

export interface WeeklyChallenge {
  kind: WeeklyChallengeKind;
  title: string;
  description: string;
  /** Target the caller is striving for. */
  targetValue: number;
  /** Caller's own progress toward `targetValue`. */
  currentValue: number;
  /** Caller's progress as a 0–100 integer (capped). */
  percentComplete: number;
  /** True when `currentValue >= targetValue`. */
  completed: boolean;
  weekStart: string;
  weekEnd: string;
  daysRemaining: number;
}

/**
 * "Tonight's MVP" — once-per-day pick of the user with the most activity
 * today (score = today_session_seconds + today_unlocks * 600). The
 * front-end uses this to drop a small crown next to the MVP's row in the
 * playtime leaderboard; null means no one's played yet today.
 */
export interface MvpToday {
  userId: string;
  displayName: string;
  avatarObjectId: string | null;
  score: number;
  sessionSeconds: number;
  achievementsUnlocked: number;
  asOf: string;
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

// ── Cloud-save types ────────────────────────────────────────────────────────

export interface CloudSaveListEntry {
  id: string;
  filename: string;
  saveType: string;
  size: number;
  dataHash?: string;
  uploadedFrom?: string | null;
  clientModifiedAt: string;
  uploadedAt: string;
}

export interface CloudSaveDownload {
  filename: string;
  saveType: string;
  /** Base64-encoded raw bytes of the save. */
  data: string;
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

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const url = serverUrl(path);
  const res = await fetch(url, init);
  if (!res.ok) {
    throw new Error(`API ${path} failed: ${res.status} ${res.statusText}`);
  }
  return res.json();
}

/**
 * Read a File into base64 (no data-URL prefix) for the native
 * `upload_user_image` command. FileReader avoids the call-stack blowups of
 * `btoa(String.fromCharCode(...bytes))` on large images.
 */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () =>
      reject(reader.error ?? new Error("Failed to read file"));
    reader.readAsDataURL(file);
  });
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
        apiFetch<Array<{ id: string; name: string }>>("api/v1/store/libraries"),

      /** Search/browse the store. */
      browse: (
        params: {
          skip?: number;
          take?: number;
          q?: string;
          tags?: string;
          /** How multiple tags combine server-side: "or" = any (default), "and" = all. */
          tagMode?: "and" | "or";
          platform?: string;
          library?: string;
          sort?:
            | "default"
            | "newest"
            | "recent"
            | "updated"
            | "name"
            | "relevance"
            | "random";
          order?: "asc" | "desc";
        } = {},
      ) => {
        const qs = new URLSearchParams();
        if (params.skip) qs.set("skip", String(params.skip));
        if (params.take) qs.set("take", String(params.take));
        if (params.q) qs.set("q", params.q);
        if (params.tags) qs.set("tags", params.tags);
        if (params.tagMode) qs.set("tagMode", params.tagMode);
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

      /** Curated store collections (public + featured) for the Collections tab. */
      collections: () =>
        apiFetch<StoreCollectionSummary[]>("api/v1/store/collection"),

      /** One store collection with its games, for the collection landing page. */
      collection: (id: string) =>
        apiFetch<StoreCollectionDetail>(`api/v1/store/collection/${id}`),

      /**
       * Add an entire collection to the caller's library: every game is added
       * to their library and a personal copy of the collection is saved as a
       * shelf. Returns the new shelf id and the number of games added.
       */
      addCollectionToLibrary: (id: string) =>
        apiFetch<{ shelfId: string; gameCount: number }>(
          `api/v1/store/collection/${id}/add-to-library`,
          { method: "POST" },
        ),
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
      nowPlaying: () =>
        apiFetch<NowPlayingEntry[]>("api/v1/community/now-playing"),
      weeklyRecap: () =>
        apiFetch<WeeklyRecapSlide[]>("api/v1/community/weekly-recap"),

      /**
       * "Spin to pick" — server-side roulette over the caller's library
       * with a social-discovery fallback. Returns one game (or null) so
       * the UI can settle on a single result. Soft-fail on the front-end.
       */
      roulette: () =>
        apiFetch<RouletteResult | null>("api/v1/community/roulette"),

      /**
       * The current week's personal quest plus the *caller's* live
       * progress. The same prompt is shown to every user on the server
       * (one `WeeklyChallenge` row per week), but `currentValue` /
       * `percentComplete` / `completed` are scoped to the caller. The
       * endpoint creates the row on first GET of a new week. Returns
       * null when no quest can be constructed (e.g. `genre_focus` on a
       * tag-empty server).
       */
      weeklyChallenge: () =>
        apiFetch<WeeklyChallenge | null>("api/v1/community/weekly-challenge"),

      /**
       * "Tonight's MVP" — at most one user, picked once per day by score
       * (today's play seconds + today's unlocks * 600). Returns null when
       * nobody has activity yet today. Soft-fail to no crown.
       */
      mvpToday: () => apiFetch<MvpToday | null>("api/v1/community/mvp-today"),

      /**
       * Server users with any playtime on this game, ranked by playtime desc.
       * Powers the "Friends X of 6" tile and the leaderboard inside the
       * per-game Community tab. Owned by Agent C — soft-fail to empty list.
       */
      gamePlayers: (gameId: string) =>
        apiFetch<GamePlayerEntry[]>(`api/v1/community/game/${gameId}/players`),

      /**
       * Activity feed (sessions + achievement unlocks) filtered to a single
       * game across all users on this Drop instance. Mirrors the shape of
       * the global activity endpoint so the same row template can be reused.
       *
       * Backed by the existing `community/activity` endpoint with an optional
       * `?gameId=` filter — the server-side agent extended the existing
       * handler rather than create a new endpoint (the spec preferred the
       * extension to avoid endpoint proliferation).
       */
      gameActivity: (gameId: string, limit = 20) =>
        apiFetch<CommunityActivityItem[]>(
          `api/v1/community/activity?gameId=${encodeURIComponent(gameId)}&limit=${limit}`,
        ),

      /**
       * Per-achievement "first to unlock on this server" tracking. Used by
       * BOTH the per-game Community tab (horizontal scroll) and the existing
       * Achievements tab (gold ring + caption on the affected rows).
       */
      gameFirsts: (gameId: string) =>
        apiFetch<GameAchievementFirst[]>(
          `api/v1/community/game/${gameId}/firsts`,
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

      /** Update profile fields (display name, bio, theme). */
      update: (data: {
        displayName?: string;
        bio?: string;
        profileTheme?: string;
      }) =>
        apiFetch<void>("api/v1/user/profile", {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(data),
        }),

      /** Upload avatar image. Returns new object ID. */
      uploadAvatar: async (file: File) => {
        // Route through the native upload command rather than a multipart POST
        // over the server:// webview protocol — that path hard-crashes
        // WebKitGTK on the Steam Deck. See remote/src/web_upload.rs.
        const json = await invoke<string>("upload_user_image", {
          path: "api/v1/user/avatar",
          filename: file.name,
          contentType: file.type || "image/png",
          dataBase64: await fileToBase64(file),
        });
        return JSON.parse(json) as { profilePictureObjectId: string };
      },

      /** Upload banner image. Returns new object ID. */
      uploadBanner: async (file: File) => {
        // Native command upload (see uploadAvatar) — avoids the WebKitGTK
        // multipart-over-server:// crash on the Steam Deck.
        const json = await invoke<string>("upload_user_image", {
          path: "api/v1/user/banner",
          filename: file.name,
          contentType: file.type || "image/png",
          dataBase64: await fileToBase64(file),
        });
        return JSON.parse(json) as { bannerObjectId: string };
      },

      /** Update showcase items. */
      updateShowcase: (
        items: Array<{
          type: string;
          gameId: string | null;
          itemId: string | null;
          title: string;
          data: any;
        }>,
      ) =>
        apiFetch<void>("api/v1/user/showcase", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ items }),
        }),

      /** Get the current user's own profile. */
      me: () => apiFetch<UserProfile>("api/v1/user"),
    },

    playtime: {
      /**
       * Most-recently-played games for the current user, distinct per game,
       * sorted by last session desc. Drives the library page's "Continue
       * playing" hero card and the "Recently played" shelf.
       */
      recent: () =>
        apiFetch<RecentPlaytimeEntry[]>("api/v1/client/playtime/recent"),
    },

    games: {
      /**
       * Per-game achievement list for the calling user. Each row carries
       * the achievement metadata (title, description, icon URLs), the
       * caller's unlock state (`unlocked` + `unlockedAt`), and the
       * server-wide rarity stats (`rarity` percent + `unlockCount`).
       *
       * Drives the store detail page's Achievements section. The library
       * detail page has its own richer pipeline via `useGameStats` —
       * this binding is intentionally read-only and unstateful.
       */
      achievements: (gameId: string) =>
        apiFetch<StoreAchievement[]>(`api/v1/games/${gameId}/achievements`),
    },

    saves: {
      // The cloud-save endpoints under /api/v1/client/saves/ use
      // defineClientEventHandler on the server, which requires the
      // desktop client's JWT/cert auth — NOT the Bearer <web_token>
      // that the `server://` Tauri protocol injects on `apiFetch`.
      // We route through Tauri commands so the request gets signed
      // with the right credentials (the same path the launch-time
      // sync already uses).
      //
      // Error surface: every method here forwards the Rust command's
      // error string via Tauri's `invoke` rejection. Callers should
      // present `e.message` (Error) or `String(e)` directly — strings
      // like "Save quota exceeded: would be 1.2 GiB / 1.0 GiB" come
      // through verbatim from the server's quota check.

      /**
       * List cloud saves for a game (current user only). Returns the
       * active (non-tombstoned) rows, newest `clientModifiedAt` first.
       */
      list: (gameId: string) =>
        invoke<CloudSaveListEntry[]>("list_cloud_saves", { gameId }),

      /**
       * Download one cloud save by its id. Returns base64-encoded bytes
       * wrapped in `{ data }` so existing call sites that destructure
       * `const { data } = await api.saves.download(...)` keep working.
       *
       * `filename` and `saveType` are returned as empty strings — they
       * were only ever used as a sanity echo, and callers already have
       * the canonical values from their `CloudSaveListEntry`. If a future
       * call site genuinely needs them, change `download_cloud_save` on
       * the Rust side to return the full struct rather than papering
       * over it here.
       */
      download: async (id: string): Promise<CloudSaveDownload> => {
        const data = await invoke<string>("download_cloud_save", { id });
        return { filename: "", saveType: "", data };
      },

      /**
       * Soft-delete one cloud save by its id. The server tombstones the
       * row (sets `deletedAt`) and records the deleting device so other
       * clients delete their local copy on next sync. A re-upload of the
       * same filename revives the row. Idempotent for already-deleted ids.
       */
      delete: (id: string) => invoke<void>("delete_cloud_save", { id }),
    },

    news: {
      list: (
        params: {
          limit?: number;
          skip?: number;
          order?: "asc" | "desc";
          tags?: string;
          search?: string;
        } = {},
      ) => {
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
