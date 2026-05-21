/**
 * Per-game emulator/launch presets for the Big Picture game-detail page:
 * controller layout, quality preset, aspect ratio, and the CRT shader
 * toggle. Each change is persisted via `update_game_configuration` and
 * surfaced to the user with a transient "Applied on next launch" toast.
 *
 * This is the BPM analogue of `composables/game-detail/use-game-config.ts`
 * (the desktop one). They are kept separate rather than merged because:
 *  - the desktop composable has no `crtShader` toggle,
 *  - BPM cycles values with a gamepad and needs toast feedback per change,
 *  - the desktop one drives a click-menu and resolves config from a
 *    `Ref<GameVersion>` it does not own.
 * The shared logic (the option tables, the cycle math) is small; the
 * couch-UI ergonomics are what differ. Decomposed out of the 3232-line
 * `pages/bigpicture/library/[id].vue`.
 *
 * Per-game-detail composable: NOT a singleton — call from a component
 * `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import { devLog } from "~/composables/dev-mode";
import type {
  AspectRatio,
  ControllerType,
  GameVersion,
  QualityPreset,
} from "~/types";

export const BPM_CONTROLLER_OPTIONS: {
  label: string;
  value: ControllerType | null;
}[] = [
  { label: "Auto", value: null },
  { label: "Xbox (A=South)", value: "Xbox" },
  { label: "Nintendo (A=East)", value: "Nintendo" },
];

export const BPM_QUALITY_OPTIONS: {
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

export function useBpmGameConfig(
  gameId: string,
  version: Ref<GameVersion | null>,
  /** Show a transient toast (e.g. "Quality: High") after each change. */
  showToast: (msg: string) => void,
  /** Surface a config-save failure as a page-level error. */
  onError: (msg: string) => void,
) {
  const selectedController = ref<ControllerType | null>(null);
  const selectedQuality = ref<QualityPreset | null>(null);
  const aspectRatio = ref<AspectRatio>("Standard");
  const crtShaderEnabled = ref(false);

  /**
   * Seed the preset refs from a freshly-loaded `GameVersion`. Called by the
   * page once `useGame()` resolves — the version isn't known at setup time.
   */
  function syncFromVersion(ver: GameVersion | null) {
    if (!ver?.userConfiguration) return;
    selectedController.value = ver.userConfiguration.controllerType ?? null;
    selectedQuality.value = ver.userConfiguration.qualityPreset ?? null;
    // `widescreen` used to be `boolean | AspectRatio`; the type is now just
    // AspectRatio. Keep a null guard for forward-compat with malformed data.
    aspectRatio.value = ver.userConfiguration.widescreen ?? "Standard";
    crtShaderEnabled.value = ver.userConfiguration.crtShader ?? false;
  }

  async function saveUserConfig() {
    const ver = version.value;
    if (!ver) return;
    try {
      const currentConfig = ver.userConfiguration ?? {
        launchTemplate: "{}",
        overrideProtonPath: null,
        enableUpdates: false,
      };
      await invoke("update_game_configuration", {
        gameId,
        options: {
          ...currentConfig,
          controllerType: selectedController.value,
          qualityPreset: selectedQuality.value,
          widescreen: aspectRatio.value,
          crtShader: crtShaderEnabled.value,
        },
      });
    } catch (e) {
      console.error("Failed to save config:", e);
      onError(
        `Failed to save settings: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }

  function setController(value: ControllerType | null) {
    selectedController.value = value;
    saveUserConfig();
    const label =
      BPM_CONTROLLER_OPTIONS.find((o) => o.value === value)?.label ?? "Auto";
    showToast(`Controller: ${label}`);
  }

  function setQuality(value: QualityPreset | null) {
    selectedQuality.value = value;
    saveUserConfig();
    const label =
      BPM_QUALITY_OPTIONS.find((o) => o.value === value)?.label ?? "Auto";
    showToast(`Quality: ${label}`);
  }

  function cycleController() {
    const values = BPM_CONTROLLER_OPTIONS.map((o) => o.value);
    const idx = values.indexOf(selectedController.value);
    setController(values[(idx + 1) % values.length]);
  }

  function cycleQuality() {
    const values = BPM_QUALITY_OPTIONS.map((o) => o.value);
    const idx = values.indexOf(selectedQuality.value);
    setQuality(values[(idx + 1) % values.length]);
  }

  function toggleWidescreen() {
    const idx = ASPECT_CYCLE.indexOf(aspectRatio.value);
    aspectRatio.value = ASPECT_CYCLE[(idx + 1) % ASPECT_CYCLE.length];
    saveUserConfig();
    showToast(`Aspect Ratio: ${aspectLabel.value}`);
  }

  function toggleCrtShader() {
    crtShaderEnabled.value = !crtShaderEnabled.value;
    saveUserConfig();
    showToast(`CRT Shader: ${crtShaderEnabled.value ? "On" : "Off"}`);
  }

  /** Push the user's profile name into a Goldberg/Steam-emu game. */
  async function applyProfileName() {
    try {
      const msg = await invoke<string>("configure_game_emulator", { gameId });
      devLog("launch", "[EMU]", msg);
    } catch (e) {
      console.error("[EMU] Failed to apply profile:", e);
    }
  }

  const controllerLabel = computed(
    () =>
      BPM_CONTROLLER_OPTIONS.find((o) => o.value === selectedController.value)
        ?.label ?? "Auto",
  );
  const qualityLabel = computed(
    () =>
      BPM_QUALITY_OPTIONS.find((o) => o.value === selectedQuality.value)
        ?.label ?? "Auto",
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

  return {
    selectedController,
    selectedQuality,
    aspectRatio,
    crtShaderEnabled,
    controllerLabel,
    qualityLabel,
    aspectLabel,
    syncFromVersion,
    saveUserConfig,
    setController,
    setQuality,
    cycleController,
    cycleQuality,
    toggleWidescreen,
    toggleCrtShader,
    applyProfileName,
  };
}
