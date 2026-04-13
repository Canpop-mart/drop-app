/**
 * BPM Page Transitions
 *
 * Provides theme-specific Vue transition names for smooth page navigation.
 * Each theme has a unique transition that matches its visual style.
 */

import { computed } from "vue";
import { useBpmTheme, type ThemeId } from "./bp-theme";

// ── Theme to transition mapping ──────────────────────────────────────────────

const TRANSITION_MAP: Record<ThemeId, string> = {
  steam: "bpm-slide",
  switch: "bpm-slide",
  xbox: "bpm-zoom",
  wii: "bpm-float",
  ps2: "bpm-fade-blue",
  ds: "bpm-split",
  dreamcast: "bpm-swirl",
  gamecube: "bpm-spin",
  psp: "bpm-slide",
  gameboy: "bpm-pixel",
  n64: "bpm-zoom",
  ps1: "bpm-fade",
  snes: "bpm-slide",
  custom: "bpm-fade",
};

// ── Composable ───────────────────────────────────────────────────────────────

export function useBpmTransitions() {
  const { themeId } = useBpmTheme();

  const transitionName = computed(() => {
    return TRANSITION_MAP[themeId.value] ?? "bpm-fade";
  });

  return {
    transitionName,
  };
}
