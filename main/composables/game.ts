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
  if (!gameRegistry[gameId]) {
    try {
      // Use deduplication for fetch_game invocations
      const data: {
        game: Game;
        status: RawGameStatus;
        version?: GameVersion;
      } = await deduplicatedInvoke("fetch_game", {
        gameId,
      });
      gameRegistry[gameId] = { game: data.game, version: ref(data.version) };
      if (!gameStatusRegistry[gameId]) {
        gameStatusRegistry[gameId] = ref(parseStatus(data.status));

        listen(`update_game/${gameId}`, (event) => {
          const payload: {
            status: RawGameStatus;
            version?: GameVersion;
          } = event.payload as any;
          gameStatusRegistry[gameId].value = parseStatus(payload.status);

          /**
           * I am not super happy about this.
           *
           * This will mean that we will still have a version assigned if we have a game installed then uninstall it.
           * It is necessary because a flag to check if we should overwrite seems excessive, and this function gets called
           * on transient state updates.
           */
          if (payload.version) {
            gameRegistry[gameId].version.value = payload.version;
          }
        });
      }
    } catch (e) {
      console.error(`Failed to fetch game data for "${gameId}":`, e);
      throw createError({
        statusCode: 500,
        statusMessage: `Failed to load game data. Please try again later.`,
        fatal: false,
      });
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
