/**
 * BPM Theme System
 *
 * Manages visual themes + dark/light mode for Big Picture Mode.
 * Each theme defines color palettes for both modes. CSS custom properties
 * are injected at runtime so templates can use `var(--bpm-bg)` etc.
 *
 * Uses Vue refs so computed properties react immediately.
 */

import { ref, computed, watchEffect } from "vue";

// ── Types ─────────────────────────────────────────────────────────────────

export type ThemeId = "steam" | "xbox" | "wii" | "ps2" | "ds" | "dreamcast" | "gamecube" | "psp" | "gameboy" | "snes";
export type ThemeMode = "dark" | "light";

export interface ThemeColors {
  /** Main background */
  bg: string;
  /** Elevated surface (cards, panels) */
  surface: string;
  /** Surface hover state */
  surfaceHover: string;
  /** Primary text */
  text: string;
  /** Secondary/muted text */
  muted: string;
  /** Theme accent color */
  accent: string;
  /** Accent text (for text on accent backgrounds) */
  accentText: string;
  /** Border/divider */
  border: string;
}

export interface BpmTheme {
  id: ThemeId;
  label: string;
  description: string;
  cssClass: string;
  colors: { dark: ThemeColors; light: ThemeColors };
}

// ── Theme Definitions ─────────────────────────────────────────────────────

export const themes: BpmTheme[] = [
  {
    id: "steam", label: "Steam", description: "Navy blue with cyan accents", cssClass: "bpm-theme-steam",
    colors: {
      dark:  { bg: "#1b2838", surface: "#1e3a50", surfaceHover: "#264d66", text: "#ffffff", muted: "#8f98a0", accent: "#66c0f4", accentText: "#ffffff", border: "rgba(255,255,255,0.08)" },
      light: { bg: "#d5e3f0", surface: "#e4eef6", surfaceHover: "#dae8f3", text: "#1b2838", muted: "#4a6278", accent: "#1a9fff", accentText: "#ffffff", border: "rgba(27,40,56,0.12)" },
    },
  },
  {
    id: "xbox", label: "Xbox", description: "Dark with Xbox green", cssClass: "bpm-theme-xbox",
    colors: {
      dark:  { bg: "#0a0a0a", surface: "#1a1a1a", surfaceHover: "#252525", text: "#ffffff", muted: "#888888", accent: "#107c10", accentText: "#ffffff", border: "rgba(255,255,255,0.06)" },
      light: { bg: "#dce8dc", surface: "#e8f0e8", surfaceHover: "#e0eae0", text: "#1a1a1a", muted: "#4a5a4a", accent: "#107c10", accentText: "#ffffff", border: "rgba(16,124,16,0.12)" },
    },
  },
  {
    id: "wii", label: "Wii", description: "Blue-grey with sky blue accents", cssClass: "bpm-theme-wii",
    colors: {
      dark:  { bg: "#1a2a3a", surface: "#243848", surfaceHover: "#2e4858", text: "#e8f0f8", muted: "#8aa0b8", accent: "#34beed", accentText: "#ffffff", border: "rgba(255,255,255,0.08)" },
      light: { bg: "#d0e4f2", surface: "#deedf8", surfaceHover: "#d5e7f4", text: "#1a2a3a", muted: "#4a6a88", accent: "#0098d0", accentText: "#ffffff", border: "rgba(0,152,208,0.12)" },
    },
  },
  {
    id: "ps2", label: "PS2", description: "Dark navy with blue-purple accents", cssClass: "bpm-theme-ps2",
    colors: {
      dark:  { bg: "#0a0e1a", surface: "#141828", surfaceHover: "#1e2438", text: "#d0d8e8", muted: "#6878a0", accent: "#2040c0", accentText: "#ffffff", border: "rgba(255,255,255,0.06)" },
      light: { bg: "#d8dae8", surface: "#e2e4f0", surfaceHover: "#dcdee8", text: "#0a0e1a", muted: "#405080", accent: "#2040c0", accentText: "#ffffff", border: "rgba(32,64,192,0.12)" },
    },
  },
  {
    id: "ds", label: "DS", description: "Silver-grey with red-orange accents", cssClass: "bpm-theme-ds",
    colors: {
      dark:  { bg: "#1a1a1e", surface: "#28282e", surfaceHover: "#333338", text: "#e0e0e0", muted: "#888890", accent: "#d05028", accentText: "#ffffff", border: "rgba(255,255,255,0.08)" },
      light: { bg: "#ddd8d6", surface: "#eae5e3", surfaceHover: "#e2dcd9", text: "#1a1a1e", muted: "#6a5a54", accent: "#d05028", accentText: "#ffffff", border: "rgba(208,80,40,0.12)" },
    },
  },
  {
    id: "dreamcast", label: "Dreamcast", description: "Teal-blue with Sega orange accents", cssClass: "bpm-theme-dreamcast",
    colors: {
      dark:  { bg: "#0c1820", surface: "#162430", surfaceHover: "#203040", text: "#e0f0f0", muted: "#688898", accent: "#d05010", accentText: "#ffffff", border: "rgba(255,255,255,0.08)" },
      light: { bg: "#cee0e0", surface: "#dceaea", surfaceHover: "#d4e4e4", text: "#0c1820", muted: "#406068", accent: "#d05010", accentText: "#ffffff", border: "rgba(208,80,16,0.12)" },
    },
  },
  {
    id: "gamecube", label: "GameCube", description: "Indigo-purple with silver accents", cssClass: "bpm-theme-gamecube",
    colors: {
      dark:  { bg: "#12102a", surface: "#1e1a3e", surfaceHover: "#282450", text: "#d8d0f0", muted: "#7870a8", accent: "#524EAA", accentText: "#ffffff", border: "rgba(255,255,255,0.08)" },
      light: { bg: "#d8d6e8", surface: "#e4e2f0", surfaceHover: "#dedce8", text: "#12102a", muted: "#4a4890", accent: "#524EAA", accentText: "#ffffff", border: "rgba(82,78,170,0.12)" },
    },
  },
  {
    id: "psp", label: "PSP", description: "Dark with subtle blue-grey accents", cssClass: "bpm-theme-psp",
    colors: {
      dark:  { bg: "#101418", surface: "#1a2028", surfaceHover: "#242c38", text: "#d0d8e0", muted: "#6080a0", accent: "#3C5078", accentText: "#ffffff", border: "rgba(255,255,255,0.06)" },
      light: { bg: "#d4d8de", surface: "#e0e4ea", surfaceHover: "#d8dce4", text: "#101418", muted: "#405878", accent: "#3C5078", accentText: "#ffffff", border: "rgba(60,80,120,0.12)" },
    },
  },
  {
    id: "gameboy", label: "Game Boy", description: "4-shade green pixel aesthetic", cssClass: "bpm-theme-gameboy",
    colors: {
      dark:  { bg: "#0f380f", surface: "#1a4a1a", surfaceHover: "#245c24", text: "#9BBC0F", muted: "#6a8a0a", accent: "#9BBC0F", accentText: "#0f380f", border: "rgba(155,188,15,0.15)" },
      light: { bg: "#8ea808", surface: "#a0bc0c", surfaceHover: "#96b20a", text: "#0f380f", muted: "#2a5a1a", accent: "#0f380f", accentText: "#9BBC0F", border: "rgba(15,56,15,0.2)" },
    },
  },
  {
    id: "snes", label: "SNES", description: "Light grey with bold button colors", cssClass: "bpm-theme-snes",
    colors: {
      dark:  { bg: "#1a1a28", surface: "#282838", surfaceHover: "#323248", text: "#e0e0f0", muted: "#8888a8", accent: "#6464B4", accentText: "#ffffff", border: "rgba(255,255,255,0.08)" },
      light: { bg: "#d4d4e0", surface: "#e0e0ec", surfaceHover: "#d8d8e6", text: "#1a1a28", muted: "#4a4a80", accent: "#6464B4", accentText: "#ffffff", border: "rgba(100,100,180,0.12)" },
    },
  },
];

const themeMap = new Map<ThemeId, BpmTheme>(themes.map((t) => [t.id, t]));

// ── Shared reactive state ─────────────────────────────────────────────────

function readStoredTheme(): ThemeId {
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem("bpmTheme") as ThemeId | null;
    if (stored && themeMap.has(stored)) return stored;
  }
  return "steam";
}

function readStoredMode(): ThemeMode {
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem("bpmThemeMode") as ThemeMode | null;
    if (stored === "dark" || stored === "light") return stored;
  }
  return "dark";
}

const activeThemeId = ref<ThemeId>(readStoredTheme());
const activeMode = ref<ThemeMode>(readStoredMode());
const activeThemeData = computed(() => themeMap.get(activeThemeId.value)!);
const activeColors = computed(() => activeThemeData.value.colors[activeMode.value]);

/**
 * Inject CSS custom properties into the document root whenever theme or mode changes.
 * Templates use `var(--bpm-bg)`, `var(--bpm-text)`, etc.
 */
function applyCssVariables() {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const c = activeColors.value;
  root.style.setProperty("--bpm-bg", c.bg);
  root.style.setProperty("--bpm-surface", c.surface);
  root.style.setProperty("--bpm-surface-hover", c.surfaceHover);
  root.style.setProperty("--bpm-text", c.text);
  root.style.setProperty("--bpm-muted", c.muted);
  root.style.setProperty("--bpm-accent", c.accent);
  root.style.setProperty("--bpm-accent-text", c.accentText);
  root.style.setProperty("--bpm-border", c.border);
}

// Watch for changes and apply immediately
watchEffect(applyCssVariables);

// ── Public API ────────────────────────────────────────────────────────────

export function useBpmTheme() {
  return {
    themeId: activeThemeId,
    themeData: activeThemeData,
    mode: activeMode,
    colors: activeColors,

    get theme() { return activeThemeId.value; },

    setTheme(id: ThemeId) {
      if (!themeMap.has(id)) return;
      activeThemeId.value = id;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem("bpmTheme", id);
      }
    },

    setMode(mode: ThemeMode) {
      activeMode.value = mode;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem("bpmThemeMode", mode);
      }
    },

    toggleMode() {
      const next: ThemeMode = activeMode.value === "dark" ? "light" : "dark";
      activeMode.value = next;
      if (typeof localStorage !== "undefined") {
        localStorage.setItem("bpmThemeMode", next);
      }
    },
  };
}
