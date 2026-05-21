import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { devLog } from "./dev-mode";
import type {
  Game,
  GameStatus,
  GameStatusEnum,
  GameVersion,
  RawGameStatus,
} from "~/types";

// The game registry caches one entry per game. `game` is a reactive object
// (its fields are mutated in place on refresh so existing template bindings
// stay live — see `invalidateGame`); `version` is a ref.
const gameRegistry: {
  [key: string]: { game: Game; version: Ref<GameVersion | undefined> };
} = {};

const gameStatusRegistry: { [key: string]: Ref<GameStatus> } = {};

// Tracks the `update_game/{id}` event subscription created lazily in
// `useGame`, so it survives across invalidations (the refs it writes to are
// reused, not replaced) and is only torn down when an entry is truly
// removed. Without this, a stale entry's listener would leak.
const gameStatusUnlisteners: { [key: string]: UnlistenFn } = {};

/**
 * Refresh one game's cached data from the backend, **in place**.
 *
 * Why in-place rather than deleting the cache entry: consumers call
 * `const { game, status, version } = await useGame(id)` once and hold those
 * references for the component's lifetime. Deleting the registry entry would
 * detach every held reference (and a `delete` of the status ref would also
 * orphan the `update_game/{id}` listener), so a view that uninstalled a game
 * would keep rendering the pre-uninstall snapshot. Instead we re-fetch and
 * mutate the *same* `game` object / `version` ref / `status` ref, so every
 * holder sees the new state immediately.
 *
 * This is the explicit-invalidation half of the single-source model: the
 * `update_game/{id}` event keeps `status`/`version` live during normal
 * operation, and this covers the cases that only emit the coarse
 * `update_library` event (notably uninstall, where the `Game` object's
 * library state and the install dir all change at once).
 */
export const invalidateGame = async (gameId: string) => {
  const entry = gameRegistry[gameId];
  const statusRef = gameStatusRegistry[gameId];
  if (!entry && !statusRef) return; // never cached — nothing to refresh

  devLog("state", `[invalidateGame] refreshing cached state for ${gameId}`);
  try {
    const data: {
      game: Game;
      status: RawGameStatus;
      version?: GameVersion;
    } = await deduplicatedInvoke("fetch_game", { gameId });

    if (entry) {
      // Mutate the existing object so template bindings stay reactive.
      Object.assign(entry.game, data.game);
      entry.version.value = data.version;
    }
    if (statusRef) {
      statusRef.value = parseStatus(data.status);
    }
  } catch (e) {
    // A failed refresh (e.g. game removed server-side, offline) should not
    // leave a poisoned half-state. Drop the entry entirely and let the next
    // `useGame` re-fetch from scratch.
    console.warn(`[invalidateGame] refresh failed for ${gameId}, evicting:`, e);
    evictGame(gameId);
  }
};

/**
 * Hard-remove a game from the cache and tear down its event listener.
 * Used when a refresh is impossible (see `invalidateGame`'s catch).
 */
const evictGame = (gameId: string) => {
  const unlisten = gameStatusUnlisteners[gameId];
  if (unlisten) {
    unlisten();
    delete gameStatusUnlisteners[gameId];
  }
  delete gameRegistry[gameId];
  delete gameStatusRegistry[gameId];
};

/**
 * Refresh every cached game. The blanket response to the `update_library`
 * backend event, which fires whenever the installed set changes (uninstall,
 * install completion, scan import) without naming a specific game.
 */
export const invalidateAllGames = async () => {
  const ids = new Set([
    ...Object.keys(gameRegistry),
    ...Object.keys(gameStatusRegistry),
  ]);
  devLog("state", `[invalidateAllGames] refreshing ${ids.size} cached game(s)`);
  await Promise.all([...ids].map((id) => invalidateGame(id)));
};

// The `update_library` event is the backend's "the installed set changed"
// broadcast (see library.rs / uninstall_game_logic). Registered once at
// module load so stale entries are refreshed no matter which view is
// mounted. A failure to register is non-fatal — callers can still
// invalidate explicitly — so it is logged and swallowed.
listen("update_library", () => {
  devLog("state", "[game.ts] update_library received — refreshing game cache");
  invalidateAllGames().catch((e) =>
    console.error("[game.ts] invalidateAllGames failed:", e),
  );
}).catch((e) => {
  console.error("[game.ts] failed to register update_library listener:", e);
});

// Request deduplication: maps command:args to pending promise
const pendingRequests = new Map<string, Promise<any>>();

/**
 * Deduplicates invoke requests - if the same command with same args is already in-flight,
 * returns the existing promise instead of making a new request.
 */
export const deduplicatedInvoke = async <T>(
  command: string,
  args: any,
): Promise<T> => {
  const key = `${command}:${JSON.stringify(args || {})}`;

  // If request is already in-flight, return the existing promise
  if (pendingRequests.has(key)) {
    return pendingRequests.get(key)!;
  }

  // Create new request and store it
  const promise = invoke<T>(command, args).finally(() => {
    // Clean up after request completes (success or error)
    pendingRequests.delete(key);
  });

  pendingRequests.set(key, promise);
  return promise;
};

export const parseStatus = (status: RawGameStatus): GameStatus => {
  if (status[0]) {
    return status[0];
  }
  if (status[1]) {
    return status[1];
  }
  throw new Error("No game status: " + JSON.stringify(status));
};

export const useGame = async (gameId: string) => {
  devLog("state",`[useGame] Fetching game: ${gameId} (cached: ${!!gameRegistry[gameId]})`);
  if (!gameRegistry[gameId]) {
    try {
      console.time(`[useGame] invoke fetch_game ${gameId}`);
      // Use deduplication for fetch_game invocations
      const data: {
        game: Game;
        status: RawGameStatus;
        version?: GameVersion;
      } = await deduplicatedInvoke("fetch_game", {
        gameId,
      });
      console.timeEnd(`[useGame] invoke fetch_game ${gameId}`);
      devLog("state",`[useGame] Got game: ${data.game.mName}, status:`, data.status, "version:", !!data.version);
      gameRegistry[gameId] = { game: reactive(data.game), version: ref(data.version) };
      if (!gameStatusRegistry[gameId]) {
        gameStatusRegistry[gameId] = ref(parseStatus(data.status));

        // Keep the status (and version) ref live for the lifetime of the
        // cache entry. `invalidateGame` reuses these refs, so the listener
        // is registered exactly once per game and only torn down by
        // `evictGame`. The unlisten handle is stored for that teardown.
        listen(`update_game/${gameId}`, (event) => {
          const payload: {
            status: RawGameStatus;
            version?: GameVersion;
          } = event.payload as any;
          // The registry entry may have been invalidated between the event
          // firing and this callback running; guard against a stale write.
          if (gameStatusRegistry[gameId]) {
            gameStatusRegistry[gameId].value = parseStatus(payload.status);
          }
          if (payload.version && gameRegistry[gameId]) {
            gameRegistry[gameId].version.value = payload.version;
          }
        })
          .then((unlisten) => {
            // If the game was invalidated while the listener was still
            // being registered, unlisten immediately.
            if (gameStatusRegistry[gameId]) {
              gameStatusUnlisteners[gameId] = unlisten;
            } else {
              unlisten();
            }
          })
          .catch((e) => {
            console.error(
              `[useGame] failed to register update_game listener for ${gameId}:`,
              e,
            );
          });
      }
    } catch (e) {
      console.error(`[useGame] FAILED for "${gameId}":`, e);
      console.error(`[useGame] Error type: ${e?.constructor?.name}, message: ${e instanceof Error ? e.message : String(e)}`);
      // Don't use createError() in BPM — it triggers Nuxt's error page
      // which breaks out of the BPM layout. Throw a plain error instead.
      throw new Error(`Failed to load game data for ${gameId}`);
    }
  }

  const game = gameRegistry[gameId];
  const status = gameStatusRegistry[gameId];
  return { ...game, status };
};

export type LaunchResult =
  | { result: "Success" }
  | { result: "InstallRequired"; data: [string, string] };

export type VersionOption = {
  versionId: string;
  displayName?: string;
  versionPath: string;
  platform: string;
  size: {
    installSize: number;
    downloadSize: number;
  };
  requiredContent: Array<{
    gameId: string;
    versionId: string;
    name: string;
    iconObjectId: string;
    shortDescription: string;
    size: {
      installSize: number;
      downloadSize: number;
    };
  }>;
};

export type ProtonPath = {
  path: string;
  name: string;
};
