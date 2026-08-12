/**
 * Canonical option tables + label/cycle helpers for the per-game emulator and
 * launch settings, shared by the desktop Configure modal ("Video & Controls"
 * tab, Launch tab) and the Big Picture inline cyclers.
 *
 * These used to be duplicated across `use-game-config.ts` (desktop) and
 * `use-bpm-game-config.ts` (Big Picture), which is how they drifted — desktop
 * grew a Fullscreen toggle, BPM grew CRT, and neither exposed MangoHud despite
 * the backend supporting all three. One copy here keeps the two surfaces in
 * sync; only the presentation (modal rows vs gamepad cyclers) differs.
 *
 * The values map 1:1 onto `UserConfiguration` fields
 * (src-tauri/database/src/models.rs) and are applied at launch by the
 * RetroArch orchestrator (src-tauri/remote/src/retroarch/).
 */

import type {
  AspectRatio,
  ControllerType,
  MangoHudPreset,
  QualityPreset,
} from "~/types";

/**
 * Which physical pad layout the emulator should assume.
 *
 * "Auto" is not "do nothing" — it means Drop detects the connected pad and
 * writes that layout's button numbers. The explicit entries exist to correct
 * it, which matters because a pad Drop cannot identify falls back to the Xbox
 * numbers, and on a PlayStation pad those rotate every face button by one
 * position and leave the emulator shortcuts on buttons that do not exist.
 */
export const CONTROLLER_OPTIONS: {
  label: string;
  value: ControllerType | null;
}[] = [
  { label: "Auto", value: null },
  { label: "Xbox (A=South)", value: "Xbox" },
  { label: "PlayStation (Cross=South)", value: "PlayStation" },
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

export const ASPECT_CYCLE: AspectRatio[] = [
  "Standard",
  "Wide16_9",
  "Wide16_10",
];

// MangoHud is a Linux performance overlay. `null` in the config means "off";
// we write an explicit value from the UI. Ordered off → most detail.
export const MANGOHUD_OPTIONS: { label: string; value: MangoHudPreset }[] = [
  { label: "Off", value: "off" },
  { label: "Minimal", value: "minimal" },
  { label: "Standard", value: "standard" },
  { label: "Full", value: "full" },
];

export function aspectLabel(a: AspectRatio): string {
  switch (a) {
    case "Wide16_9":
      return "16:9";
    case "Wide16_10":
      return "16:10";
    default:
      return "4:3";
  }
}

export function nextAspect(a: AspectRatio): AspectRatio {
  const idx = ASPECT_CYCLE.indexOf(a);
  return ASPECT_CYCLE[(idx + 1) % ASPECT_CYCLE.length];
}

export function controllerLabel(v: ControllerType | null): string {
  return CONTROLLER_OPTIONS.find((o) => o.value === v)?.label ?? "Auto";
}

export function qualityLabel(v: QualityPreset | null): string {
  return QUALITY_OPTIONS.find((o) => o.value === v)?.label ?? "Auto";
}

// Reading tolerates the legacy `null` = off; writing always uses a concrete value.
export function mangohudLabel(v: MangoHudPreset | null | undefined): string {
  return MANGOHUD_OPTIONS.find((o) => o.value === (v ?? "off"))?.label ?? "Off";
}

export function nextMangohud(v: MangoHudPreset | null | undefined): MangoHudPreset {
  const idx = MANGOHUD_OPTIONS.findIndex((o) => o.value === (v ?? "off"));
  return MANGOHUD_OPTIONS[(idx + 1) % MANGOHUD_OPTIONS.length].value;
}
