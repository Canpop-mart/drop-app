/**
 * Per-game bits the desktop cog menu still needs after the options makeover:
 * game-type detection (to gate menu items) and the Goldberg "Set Account Name"
 * action.
 *
 * The emulator/video PRESETS (controller, quality, aspect, fullscreen, CRT) no
 * longer live here — they moved into the Configure modal's "Video & Controls"
 * tab (`components/GameOptions/Video.vue`), which edits the shared configuration
 * object directly. The option tables live in `./emulator-options`.
 *
 * Per-game-detail composable: NOT a singleton — call from a component `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import { devLog } from "~/composables/dev-mode";
import type { Game, GameVersion } from "~/types";

export function useGameConfig(game: Game, version: Ref<GameVersion | undefined>) {
  // ROM games run through an emulator (a launch with a REAL emulator reference).
  // Check `emulator?.gameId`, not just `emulator != null`: a PC game's launch can
  // carry an empty/placeholder emulator object, and `!= null` wrongly flags that
  // as emulated. Everything else is "native" and may use a Goldberg/Steam emu,
  // which is what "Set Account Name" targets.
  const isEmulatedGame = computed(
    () => version.value?.launches?.some((l) => !!l.emulator?.gameId) ?? false,
  );
  const isNativeGame = computed(() => !isEmulatedGame.value);

  /** Push the user's profile name into a Goldberg/Steam-emu game. */
  async function applyProfileName() {
    try {
      const msg = await invoke<string>("configure_game_emulator", {
        gameId: game.id,
      });
      devLog("state", "[EMU]", msg);
    } catch (e) {
      console.error("[EMU] Failed to apply profile:", e);
    }
  }

  return {
    isEmulatedGame,
    isNativeGame,
    applyProfileName,
  };
}
