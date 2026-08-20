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
import { platform } from "@tauri-apps/plugin-os";
import { devLog } from "~/composables/dev-mode";
import {
  CONTROLLER_OPTIONS,
  mangohudLabel as mangohudLabelOf,
  nextMangohud,
} from "~/composables/game-detail/emulator-options";
import {
  launchTemplateUsesExecutable,
  scanGameExecutables,
  type ExecutableCandidate,
} from "~/composables/game-detail/executable-picker";
import type {
  AspectRatio,
  ControllerType,
  GameVersion,
  MangoHudPreset,
  QualityPreset,
} from "~/types";

/**
 * One picker entry for the Proton cycler. `path: null` represents "fall
 * back to the app-level default"; concrete paths come from drop-app's
 * `fetch_proton_paths` Tauri command (auto-discovered + user-added).
 */
type BpmProtonOption = { label: string; path: string | null };

/**
 * Re-exported, not redeclared. This used to be a second copy of the same
 * table, which is how the two surfaces drifted apart last time — and it is why
 * the PlayStation layout existed in the backend but was offered by neither.
 */
export const BPM_CONTROLLER_OPTIONS: {
  label: string;
  value: ControllerType | null;
}[] = CONTROLLER_OPTIONS;

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

/**
 * How many executables the couch surface will cycle through. A cycler is one
 * focusable row rather than a scrolling list, which is what Big Picture wants,
 * but it costs an A press per step, so an install with dozens of binaries would
 * be miserable. The backend ranks the real game binary to the front, so the cap
 * bites only on unusual installs; the desktop Configure modal lists them all.
 */
const BPM_EXECUTABLE_LIMIT = 12;

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
  const fullscreen = ref<boolean>(true);
  const mangohud = ref<MangoHudPreset | null>(null);
  const selectedProtonPath = ref<string | null>(null);
  const executableOverride = ref<string | null>(null);
  const executableCandidates = ref<ExecutableCandidate[]>([]);

  // Proton override picker: list starts with just "Default" so it works on
  // Windows/macOS hosts (where `fetch_proton_paths` is a Linux-only Tauri
  // command and would just throw). On Linux we hydrate it on init.
  const protonOptions = ref<BpmProtonOption[]>([
    { label: "Default", path: null },
  ]);

  async function loadProtonPaths() {
    // Linux-only — the Tauri command is `#[cfg(target_os = "linux")]`.
    if (platform() !== "linux") return;
    try {
      const result = await invoke<{
        autodiscovered: Array<{ name: string; path: string }>;
        custom: Array<{ name: string; path: string }>;
      }>("fetch_proton_paths");
      const options: BpmProtonOption[] = [{ label: "Default", path: null }];
      for (const p of result.autodiscovered) {
        options.push({ label: p.name, path: p.path });
      }
      for (const p of result.custom) {
        options.push({ label: `${p.name} (custom)`, path: p.path });
      }
      protonOptions.value = options;
    } catch (e) {
      console.warn("[BPM:GAME-CONFIG] fetch_proton_paths failed:", e);
    }
  }

  /**
   * Ask the backend which programs sit in the install folder. Returns nothing
   * for an emulated or not-yet-installed game, which is what keeps the options
   * row hidden in those cases.
   */
  async function loadExecutables() {
    try {
      const result = await scanGameExecutables(gameId);
      executableCandidates.value = result.supported
        ? result.candidates.slice(0, BPM_EXECUTABLE_LIMIT)
        : [];
    } catch (e) {
      console.warn("[BPM:GAME-CONFIG] scan_game_executables failed:", e);
      executableCandidates.value = [];
    }
  }

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
    fullscreen.value = ver.userConfiguration.fullscreen ?? true;
    mangohud.value = ver.userConfiguration.mangohud ?? null;
    selectedProtonPath.value = ver.userConfiguration.overrideProtonPath ?? null;
    executableOverride.value = ver.userConfiguration.executableOverride ?? null;
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
          fullscreen: fullscreen.value,
          mangohud: mangohud.value,
          overrideProtonPath: selectedProtonPath.value,
          executableOverride: executableOverride.value,
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

  function toggleFullscreen() {
    fullscreen.value = !fullscreen.value;
    saveUserConfig();
    showToast(`Fullscreen: ${fullscreen.value ? "On" : "Off"}`);
  }

  function cycleMangohud() {
    mangohud.value = nextMangohud(mangohud.value);
    saveUserConfig();
    showToast(`MangoHud: ${mangohudLabelOf(mangohud.value)}`);
  }

  /**
   * Cycle through available Proton versions (incl. "Default" at index 0).
   * Sticky on the Deck: a user with a library of fussy Windows games can
   * walk Select-button down the row until they find a Proton that runs
   * the game, without leaving Game Mode to edit settings.
   */
  function cycleProton() {
    const paths = protonOptions.value.map((o) => o.path);
    // indexOf returns -1 if the current saved value is no longer in the
    // list (e.g. a Proton path was removed) — incrementing -1 lands on
    // index 0 ("Default"), which is the desired fallback.
    const idx = paths.indexOf(selectedProtonPath.value);
    const nextIdx = (idx + 1) % paths.length;
    selectedProtonPath.value = paths[nextIdx];
    saveUserConfig();
    showToast(`Proton: ${protonOptions.value[nextIdx].label}`);
  }

  /**
   * Step to the next executable in the install folder, wrapping back round to
   * "Automatic" (the launch command the server set up).
   */
  function cycleExecutable() {
    const choices: (string | null)[] = [
      null,
      ...executableCandidates.value.map((c) => c.relativePath),
    ];
    const idx = choices.indexOf(executableOverride.value);
    // -1 means the saved override is no longer on disk. +1 lands on index 0,
    // which is Automatic, and that is the right place to end up.
    const nextIdx = (idx + 1) % choices.length;
    executableOverride.value = choices[nextIdx];
    saveUserConfig();
    const position = `(${nextIdx + 1}/${choices.length})`;
    // The row stays available even when the template wins, because it is the
    // only way to clear a pick from the couch. Saying so in the toast is the
    // one feedback channel this surface has: the template field itself lives
    // in the desktop app.
    showToast(
      launchTemplateBlocksExecutable.value
        ? `Executable: ${executableLabel.value} ${position}. Not used yet: this game's launch string template replaces it. Clear that field in the desktop app.`
        : `Executable: ${executableLabel.value} ${position}`,
    );
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
  const protonLabel = computed(
    () =>
      protonOptions.value.find((o) => o.path === selectedProtonPath.value)
        ?.label ?? "Default",
  );
  const mangohudLabel = computed(() => mangohudLabelOf(mangohud.value));
  /**
   * True when this game's launch string template would throw the picked
   * executable away. The launcher applies the template last, so a template
   * with no `{}` in it (the old workaround: a literal path typed into that
   * field) wins over anything chosen here.
   */
  const launchTemplateBlocksExecutable = computed(() => {
    const template = version.value?.userConfiguration?.launchTemplate;
    // Version not loaded yet: stay quiet rather than warn about a template
    // nobody has read.
    return template === undefined
      ? false
      : !launchTemplateUsesExecutable(template);
  });
  const executableLabel = computed(() => {
    const current = executableOverride.value;
    if (!current) return "Automatic";
    return (
      executableCandidates.value.find((c) => c.relativePath === current)
        ?.fileName ?? current
    );
  });

  // Kick off the Linux Proton-path fetch eagerly — no `await` here so the
  // composable returns synchronously and the BPM page can mount; the
  // `protonOptions` ref updates reactively when the fetch resolves.
  loadProtonPaths();
  loadExecutables();

  return {
    selectedController,
    selectedQuality,
    aspectRatio,
    crtShaderEnabled,
    fullscreen,
    mangohud,
    selectedProtonPath,
    protonOptions,
    executableOverride,
    executableCandidates,
    executableLabel,
    controllerLabel,
    qualityLabel,
    aspectLabel,
    mangohudLabel,
    protonLabel,
    syncFromVersion,
    loadExecutables,
    saveUserConfig,
    setController,
    setQuality,
    cycleController,
    cycleQuality,
    toggleWidescreen,
    toggleCrtShader,
    toggleFullscreen,
    cycleMangohud,
    cycleProton,
    cycleExecutable,
    applyProfileName,
  };
}
