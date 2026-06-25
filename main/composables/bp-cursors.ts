/**
 * BPM Custom Cursors
 *
 * Generates theme-specific custom cursor styles using SVG data URIs.
 * Each theme has a unique cursor design that matches its aesthetic.
 */

import { ref, readonly, computed } from "vue";
import { useBpmTheme } from "./bp-theme";

// ── Cursor SVG definitions ───────────────────────────────────────────────────

const CURSOR_DESIGNS: Record<string, string> = {
  // Steam: Blue dot cursor
  steam: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <circle cx="12" cy="12" r="6" fill="#1a9fff" opacity="0.8"/>
    <circle cx="12" cy="12" r="4" fill="#1a9fff"/>
  </svg>`,

  // Switch: White arrow with red tip
  switch: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <path d="M4 4 L4 18 L12 12 Z" fill="white" stroke="#e60012" stroke-width="1"/>
  </svg>`,

  // Xbox: Green ring cursor
  xbox: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <circle cx="12" cy="12" r="8" fill="none" stroke="#107c10" stroke-width="2"/>
    <circle cx="12" cy="12" r="4" fill="#107c10" opacity="0.6"/>
  </svg>`,

  // PS2: Blue crosshair
  ps2: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <line x1="12" y1="4" x2="12" y2="8" stroke="#003cff" stroke-width="2"/>
    <line x1="12" y1="16" x2="12" y2="20" stroke="#003cff" stroke-width="2"/>
    <line x1="4" y1="12" x2="8" y2="12" stroke="#003cff" stroke-width="2"/>
    <line x1="16" y1="12" x2="20" y2="12" stroke="#003cff" stroke-width="2"/>
    <circle cx="12" cy="12" r="2" fill="#003cff"/>
  </svg>`,

  // Dreamcast: Orange swirl dot
  dreamcast: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <circle cx="12" cy="12" r="5" fill="none" stroke="#ff8800" stroke-width="1.5"/>
    <path d="M12 8 Q14 10 12 12 Q10 14 12 16" fill="none" stroke="#ff8800" stroke-width="1.5" stroke-linecap="round"/>
    <circle cx="12" cy="12" r="2" fill="#ff8800"/>
  </svg>`,

  // Wii: White hand pointer
  wii: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <path d="M5 2 L5 14 L8 18 L11 16 L9 12 L15 12 Q17 12 17 14 L17 18 Q17 20 15 20 L8 20 Q6 20 5 19 Z" fill="white" stroke="#cccccc" stroke-width="0.5"/>
  </svg>`,

  // DS: Orange stylus pointer
  ds: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <path d="M6 3 L10 16 L8 18 L4 14 Z" fill="#ff8800" stroke="#ffaa44" stroke-width="0.5"/>
    <circle cx="7" cy="5" r="1.5" fill="#ffaa44"/>
  </svg>`,

  // GameCube: Indigo dot
  gamecube: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <circle cx="12" cy="12" r="6" fill="#4d3f7f" opacity="0.9"/>
    <circle cx="12" cy="12" r="4" fill="#6f5cbd"/>
    <circle cx="12" cy="12" r="2" fill="#9f8fdf" opacity="0.6"/>
  </svg>`,

  // PSP: White crosshair
  psp: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <line x1="12" y1="2" x2="12" y2="7" stroke="white" stroke-width="1.5"/>
    <line x1="12" y1="17" x2="12" y2="22" stroke="white" stroke-width="1.5"/>
    <line x1="2" y1="12" x2="7" y2="12" stroke="white" stroke-width="1.5"/>
    <line x1="17" y1="12" x2="22" y2="12" stroke="white" stroke-width="1.5"/>
    <circle cx="12" cy="12" r="1.5" fill="white"/>
  </svg>`,

  // Game Boy: Green block cursor
  gameboy: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <rect x="8" y="8" width="8" height="8" fill="#5d7a4c"/>
    <rect x="9" y="9" width="6" height="6" fill="#8fc93a"/>
  </svg>`,

  // N64: Multicolor dot
  n64: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <circle cx="12" cy="12" r="6" fill="none" stroke="#ffff00" stroke-width="1.5"/>
    <circle cx="10" cy="10" r="2" fill="#ff0000"/>
    <circle cx="14" cy="10" r="2" fill="#00ff00"/>
    <circle cx="10" cy="14" r="2" fill="#0000ff"/>
    <circle cx="14" cy="14" r="2" fill="#ffff00"/>
  </svg>`,

  // PS1: Grey crosshair
  ps1: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <line x1="12" y1="3" x2="12" y2="9" stroke="#888888" stroke-width="1.5"/>
    <line x1="12" y1="15" x2="12" y2="21" stroke="#888888" stroke-width="1.5"/>
    <line x1="3" y1="12" x2="9" y2="12" stroke="#888888" stroke-width="1.5"/>
    <line x1="15" y1="12" x2="21" y2="12" stroke="#888888" stroke-width="1.5"/>
    <circle cx="12" cy="12" r="2" fill="#888888"/>
  </svg>`,

  // SNES: Purple dot
  snes: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <circle cx="12" cy="12" r="7" fill="#9945ff" opacity="0.7"/>
    <circle cx="12" cy="12" r="5" fill="#cc66ff"/>
    <circle cx="12" cy="12" r="3" fill="#ee99ff" opacity="0.5"/>
  </svg>`,

  // Default/Custom: Light grey dot
  default: `<svg width="24" height="24" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
    <circle cx="12" cy="12" r="5" fill="#cccccc" opacity="0.8"/>
    <circle cx="12" cy="12" r="3" fill="#ffffff"/>
  </svg>`,
};

// ── Utility function ────────────────────────────────────────────────────────

function svgToDataUri(svg: string): string {
  const encoded = encodeURIComponent(svg.trim());
  return `url('data:image/svg+xml,${encoded}')`;
}

// ── Singleton state ──────────────────────────────────────────────────────────

function readEnabledPref(): boolean {
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem("bpm:customCursor");
    return stored !== "false";
  }
  return true;
}

const customCursorEnabled = ref(readEnabledPref());

// ── Composable ───────────────────────────────────────────────────────────────

export function useBpmCursors() {
  const { themeId } = useBpmTheme();

  const cursorStyle = computed(() => {
    if (!customCursorEnabled.value) return "auto";

    const theme = themeId.value;
    const svg = CURSOR_DESIGNS[theme] ?? CURSOR_DESIGNS.default;
    const dataUri = svgToDataUri(svg);

    // Format: "url(...) offsetX offsetY, fallback"
    return `${dataUri} 8 8, auto`;
  });

  function setEnabled(val: boolean) {
    customCursorEnabled.value = val;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("bpm:customCursor", String(val));
    }
  }

  return {
    cursorStyle,
    enabled: readonly(customCursorEnabled),
    setEnabled,
  };
}
