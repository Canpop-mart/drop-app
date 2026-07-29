import { computed, toValue, type MaybeRefOrGetter } from "vue";

/**
 * Profile theming — single source of truth for the accent colour that threads
 * through a profile page (name, stats, shelves, cards, buttons, banner).
 *
 * A user's stored `profileTheme` is EITHER one of the preset keys below OR a
 * raw `#rrggbb` custom colour (the field is free-text on the server, so no
 * migration is needed to store a custom hex). `useProfileTheme` resolves that
 * to a set of CSS custom properties bound once on a profile-root wrapper; the
 * children then reference them with static Tailwind arbitrary utilities
 * (`text-[color:var(--accent)]`, `ring-[color:var(--accent-border)]`, …).
 * Never build a class string from the hex — Tailwind's JIT would purge it.
 */

export interface ProfileThemePreset {
  label: string;
  accent: string;
}

/** The built-in accents. Replaces the per-page `themeColors` switches. */
export const PROFILE_THEME_PRESETS: Record<string, ProfileThemePreset> = {
  default: { label: "Blue", accent: "#3b82f6" },
  ocean: { label: "Ocean", accent: "#0ea5e9" },
  sunset: { label: "Sunset", accent: "#f97316" },
  forest: { label: "Forest", accent: "#22c55e" },
  purple: { label: "Purple", accent: "#a855f7" },
  rose: { label: "Rose", accent: "#f43f5e" },
};

const HEX_RE = /^#[0-9a-f]{6}$/i;

/** Resolve a stored `profileTheme` (preset key OR `#hex`) to a concrete hex. */
export function resolveAccentHex(theme?: string | null): string {
  if (theme && HEX_RE.test(theme)) return theme.toLowerCase();
  if (theme && PROFILE_THEME_PRESETS[theme])
    return PROFILE_THEME_PRESETS[theme]!.accent;
  return PROFILE_THEME_PRESETS.default!.accent;
}

interface Rgb {
  r: number;
  g: number;
  b: number;
}

function hexToRgb(hex: string): Rgb {
  const h = hex.replace("#", "");
  return {
    r: parseInt(h.slice(0, 2), 16),
    g: parseInt(h.slice(2, 4), 16),
    b: parseInt(h.slice(4, 6), 16),
  };
}

function rgbToHex(r: number, g: number, b: number): string {
  const c = (x: number) => `0${Math.round(x).toString(16)}`.slice(-2);
  return `#${c(r)}${c(g)}${c(b)}`;
}

function rgbToHsl({ r, g, b }: Rgb): { h: number; s: number; l: number } {
  r /= 255;
  g /= 255;
  b /= 255;
  const mx = Math.max(r, g, b);
  const mn = Math.min(r, g, b);
  let h = 0;
  let s = 0;
  const l = (mx + mn) / 2;
  if (mx !== mn) {
    const d = mx - mn;
    s = l > 0.5 ? d / (2 - mx - mn) : d / (mx + mn);
    h =
      mx === r
        ? (g - b) / d + (g < b ? 6 : 0)
        : mx === g
          ? (b - r) / d + 2
          : (r - g) / d + 4;
    h /= 6;
  }
  return { h, s, l };
}

function hslToHex(h: number, s: number, l: number): string {
  let r: number;
  let g: number;
  let b: number;
  if (s === 0) {
    r = g = b = l;
  } else {
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    const hue = (t: number) => {
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1 / 6) return p + (q - p) * 6 * t;
      if (t < 1 / 2) return q;
      if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
      return p;
    };
    r = hue(h + 1 / 3);
    g = hue(h);
    b = hue(h - 1 / 3);
  }
  return rgbToHex(r * 255, g * 255, b * 255);
}

/**
 * Derive the full CSS-var set from one accent hex. `--accent` is lightness-
 * clamped so text/icons stay legible on the dark background even for a
 * near-black or near-white custom colour, while `--accent-raw` and the banner
 * gradient keep the user's true colour (they sit behind the page scrim).
 */
export function accentVars(hex: string): Record<string, string> {
  const rgb = hexToRgb(hex);
  const { h, s, l } = rgbToHsl(rgb);
  const fg = hslToHex(h, Math.max(s, 0.5), Math.min(Math.max(l, 0.58), 0.82));
  const bannerTo = hslToHex(h, s, Math.max(l - 0.3, 0.1));
  const lum = 0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b;
  return {
    "--accent-raw": hex,
    "--accent": fg,
    "--accent-soft": `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.14)`,
    "--accent-border": `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.42)`,
    "--accent-contrast": lum > 150 ? "#000000" : "#ffffff",
    // A deep shade for large immersive gradients (the Wrapped hero cards).
    "--accent-deep": hslToHex(h, s, Math.max(l - 0.34, 0.12)),
    "--profile-banner": `linear-gradient(135deg, ${hex}, ${bannerTo})`,
  };
}

/**
 * Reactive theme for a profile page. Pass the user's `profileTheme` (a ref or
 * getter). Bind `vars` on a root wrapper's `:style`; read `accent` for JS-side
 * needs (e.g. the live edit preview).
 */
export function useProfileTheme(
  theme: MaybeRefOrGetter<string | undefined | null>,
) {
  const accent = computed(() => resolveAccentHex(toValue(theme)));
  const vars = computed(() => accentVars(accent.value));
  return { accent, vars };
}
