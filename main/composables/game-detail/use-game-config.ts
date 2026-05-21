/**
 * Per-game user configuration for the library game-detail page's options
 * menu: emulated-game presets (controller layout, quality, aspect ratio)
 * and the "Set Account Name" action for Goldberg/Steam-emu games.
 *
 * Extracted from `pages/library/[id]/index.vue`. The options-menu markup
 * lives in `components/game-detail/GameOptionsMenu.vue`.
 *
 * Per-game-detail composable: NOT a singleton — call from a component
 * `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import { devLog } from "~/composables/dev-mode";
import type {
  AspectRatio,
  ControllerType,
  Game,
  GameVersion,
  QualityPreset,
} from "~/types";

export const CONTROLLER_OPTIONS: {
  label: string;
  value: ControllerType | null;
}[] = [
  { label: "Auto", value: null },
  { label: "Xbox (A=South)", value: "Xbox" },
  { label: "Nintendo (A=East)", value: "Nintendo" },
];

export const QUALITY_OPTIONS: {
  label: string;
  value: QualityPreset | null;
}[] = [
  { label: "Auto", value: null },
  { label: "Low", value: "Low" },
  { label: "Med", value: "Medium" },
  { label: "High", value: "High" },
  { label: "Ultra", value: "Ultra" },
];

const ASPECT_CYCLE: AspectRatio[] = ["Standard", "Wide16_9", "Wide16_10"];

export function useGameConfig(game: Game, version: Ref<GameVersion | undefined>) {
  // ── Game-type detection ─────────────────────────────────────────────────
  // ROM games run through an emulator (RetroArch) — they get the
  // controller/quality/widescreen presets. Non-emulator games may use
  // Goldberg/Steam emu — they get "Set Account Name".
  const isEmulatedGame = computed(
    () => version.value?.launches?.some((l) => l.emulator != null) ?? false,
  );
  const isNativeGame = computed(() => !isEmulatedGame.value);

  // ── Preset state ────────────────────────────────────────────────────────
  const selectedController = ref<ControllerType | null>(
    version.value?.userConfiguration?.controllerType ?? null,
  );
  const selectedQuality = ref<QualityPreset | null>(
    version.value?.userConfiguration?.qualityPreset ?? null,
  );
  // `widescreen` used to be `boolean | AspectRatio`; the type is now just
  // AspectRatio. Keep a null guard for forward-compat with malformed data.
  const aspectRatio = ref<AspectRatio>(
    version.value?.userConfiguration?.widescreen ?? "Standard",
  );
  const aspectLabel = computed(() => {
    switch (aspectRatio.value) {
      case "Wide16_9":
        return "16:9";
      case "Wide16_10":
        return "16:10";
      default:
        return "4:3";
    }
  });

  async function saveUserConfig() {
    if (!version.value) return;
    try {
      await invoke("update_game_configuration", {
        gameId: game.id,
        options: {
          ...version.value.userConfiguration,
          controllerType: selectedController.value,
          qualityPreset: selectedQuality.value,
          widescreen: aspectRatio.value,
        },
      });
    } catch (e) {
      console.error("Failed to save config:", e);
    }
  }

  function setController(value: ControllerType | null) {
    selectedController.value = value;
    saveUserConfig();
  }

  function setQuality(value: QualityPreset | null) {
    selectedQuality.value = value;
    saveUserConfig();
  }

  function toggleWidescreen() {
    const idx = ASPECT_CYCLE.indexOf(aspectRatio.value);
    aspectRatio.value = ASPECT_CYCLE[(idx + 1) % ASPECT_CYCLE.length];
    saveUserConfig();
  }

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
    selectedController,
    selectedQuality,
    aspectRatio,
    aspectLabel,
    setController,
    setQuality,
    toggleWidescreen,
    applyProfileName,
  };
}
