/**
 * BPM Theme System
 *
 * Manages visual themes + sound profiles for Big Picture Mode.
 * Themes apply CSS custom properties for colors and determine
 * which sound profile and home layout to use.
 *
 * Uses a Vue ref so that computed properties in the layout
 * react immediately when the theme changes.
 */

import { ref, computed } from "vue";
import { useBpAudio, type SoundProfileId } from "./bp-audio";

export type ThemeId = "steam" | "switch" | "xbox" | "wii" | "ps2" | "ds" | "dreamcast" | "gamecube" | "psp" | "gameboy" | "snes";

export interface BpmTheme {
  id: ThemeId;
  label: string;
  description: string;
  /** CSS class applied to the BPM layout root for scoped color overrides */
  cssClass: string;
}

export const themes: BpmTheme[] = [
  { id: "steam", label: "Steam", description: "Navy blue with cyan accents", cssClass: "bpm-theme-steam" },
  { id: "switch", label: "Switch", description: "Clean black with Nintendo red", cssClass: "bpm-theme-switch" },
  { id: "xbox", label: "Xbox", description: "Dark with Xbox green", cssClass: "bpm-theme-xbox" },
  { id: "wii", label: "Wii", description: "Blue-grey with sky blue accents", cssClass: "bpm-theme-wii" },
  { id: "ps2", label: "PS2", description: "Dark navy with blue-purple accents", cssClass: "bpm-theme-ps2" },
  { id: "ds", label: "DS", description: "Silver-grey with red-orange accents", cssClass: "bpm-theme-ds" },
  { id: "dreamcast", label: "Dreamcast", description: "Teal-blue with Sega orange accents", cssClass: "bpm-theme-dreamcast" },
  { id: "gamecube", label: "GameCube", description: "Indigo-purple with silver accents", cssClass: "bpm-theme-gamecube" },
  { id: "psp", label: "PSP", description: "Dark with subtle blue-grey accents", cssClass: "bpm-theme-psp" },
  { id: "gameboy", label: "Game Boy", description: "4-shade green pixel aesthetic", cssClass: "bpm-theme-gameboy" },
  { id: "snes", label: "SNES", description: "Light grey with bold button colors", cssClass: "bpm-theme-snes" },
];

const themeMap = new Map<ThemeId, BpmTheme>(themes.map((t) => [t.id, t]));

// ── Shared reactive state (module-level singleton) ──────────────────────
// Initialize from localStorage if available
function readStoredTheme(): ThemeId {
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem("bpmTheme") as ThemeId | null;
    if (stored && themeMap.has(stored)) return stored;
  }
  return "steam";
}

const activeThemeId = ref<ThemeId>(readStoredTheme());
const activeThemeData = computed(() => themeMap.get(activeThemeId.value)!);

export function useBpmTheme() {
  return {
    /** Reactive theme ID — use in computed/watch/template */
    themeId: activeThemeId,
    /** Reactive theme data object */
    themeData: activeThemeData,

    /** Non-reactive read of the current theme ID */
    get theme() { return activeThemeId.value; },

    setTheme(id: ThemeId) {
      if (!themeMap.has(id)) return;
      activeThemeId.value = id;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem("bpmTheme", id);
      }
    },
  };
}
