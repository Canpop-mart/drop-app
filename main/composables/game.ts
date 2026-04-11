import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Game,
  GameStatus,
  GameStatusEnum,
  GameVersion,
  RawGameStatus,
} from "~/types";

const gameRegistry: {
  [key: string]: { game: Game; version: Ref<GameVersion | undefined> };
} = {};

const gameStatusRegistry: { [key: string]: Ref<GameStatus> } = {};

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
  console.log(`[useGame] Fetching game: ${gameId} (cached: ${!!gameRegistry[gameId]})`);
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
      console.log(`[useGame] Got game: ${data.game.mName}, status:`, data.status, "version:", !!data.version);
      gameRegistry[gameId] = { game: data.game, version: ref(data.version) };
      if (!gameStatusRegistry[gameId]) {
        gameStatusRegistry[gameId] = ref(parseStatus(data.status));

        listen(`update_game/${gameId}`, (event) => {
          const payload: {
            status: RawGameStatus;
            version?: GameVersion;
          } = event.payload as any;
          gameStatusRegistry[gameId].value = parseStatus(payload.status);

          if (payload.version) {
            gameRegistry[gameId].version.value = payload.version;
          }
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
