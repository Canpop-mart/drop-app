/**
 * Maps a console/library name to its bundled isometric console render.
 *
 * The emulation view groups games by their Compatibility library, named by the
 * operator ("Playstation 2", "Super Nintendo", even "Swtich" with a typo).
 * This resolves those free-form names to a canonical console + render slug, so
 * the console cards show artwork instead of plain text.
 *
 * Art lives in /public/console-art/<slug>.png. Sources (see console-art/CREDITS.md):
 * Jude Coram (the smooth flat-shaded renders) and the es-theme-pixel theme
 * by Rookervik / ehettervik / TheGemsbok (the pixel renders for the consoles
 * Coram doesn't cover: GameCube, Game Boy Advance/Color, Switch, PS3, 3DS).
 */
export interface ConsoleArt {
  /** Render asset slug (file is /console-art/<slug>.png). */
  slug: string;
  /** Canonical display name (also corrects operator typos like "Swtich"). */
  name: string;
}

// One entry per console we have a render for. `aliases` are matched against the
// normalized library name (lowercased, non-alphanumerics collapsed to spaces).
const CONSOLES: Array<ConsoleArt & { aliases: string[] }> = [
  { slug: "ps1", name: "PlayStation", aliases: ["ps1", "psx", "playstation", "playstation 1", "playstation one"] },
  { slug: "ps2", name: "PlayStation 2", aliases: ["ps2", "playstation 2"] },
  { slug: "ps3", name: "PlayStation 3", aliases: ["ps3", "playstation 3"] },
  { slug: "ps4", name: "PlayStation 4", aliases: ["ps4", "playstation 4"] },
  { slug: "nes", name: "NES", aliases: ["nes", "famicom", "nintendo entertainment system"] },
  { slug: "snes", name: "Super Nintendo", aliases: ["snes", "super nintendo", "super famicom", "super nintendo entertainment system"] },
  { slug: "n64", name: "Nintendo 64", aliases: ["n64", "nintendo 64"] },
  { slug: "gamecube", name: "GameCube", aliases: ["gamecube", "gc", "ngc", "nintendo gamecube"] },
  { slug: "wii", name: "Wii", aliases: ["wii", "nintendo wii"] },
  { slug: "wiiu", name: "Wii U", aliases: ["wiiu", "wii u", "nintendo wii u"] },
  { slug: "switch", name: "Switch", aliases: ["switch", "swtich", "nintendo switch"] },
  { slug: "gba", name: "Game Boy Advance", aliases: ["gba", "game boy advance", "gameboy advance"] },
  { slug: "gbc", name: "Game Boy Color", aliases: ["gbc", "game boy color", "gameboy color"] },
  { slug: "3ds", name: "Nintendo 3DS", aliases: ["3ds", "nintendo 3ds"] },
  { slug: "dreamcast", name: "Dreamcast", aliases: ["dreamcast", "sega dreamcast"] },
  { slug: "saturn", name: "Sega Saturn", aliases: ["saturn", "sega saturn"] },
  { slug: "megadrive", name: "Sega Genesis", aliases: ["megadrive", "mega drive", "genesis", "sega genesis", "sega mega drive"] },
  { slug: "mastersystem", name: "Master System", aliases: ["mastersystem", "master system", "sega master system", "sms"] },
  { slug: "xbox", name: "Xbox", aliases: ["xbox", "microsoft xbox"] },
  { slug: "xbox360", name: "Xbox 360", aliases: ["xbox360", "xbox 360"] },
  { slug: "atari2600", name: "Atari 2600", aliases: ["atari2600", "atari 2600", "atari"] },
  { slug: "zxspectrum", name: "ZX Spectrum", aliases: ["zxspectrum", "zx spectrum", "spectrum"] },
];

const BY_ALIAS = new Map<string, ConsoleArt>();
for (const c of CONSOLES) {
  const art: ConsoleArt = { slug: c.slug, name: c.name };
  for (const a of c.aliases) BY_ALIAS.set(a, art);
}

// Renders from the pixel theme (vs Coram's higher-res flat art). These stay
// crisp with nearest-neighbour scaling; Coram's should scale smoothly.
const PIXEL_SLUGS = new Set(["gamecube", "gba", "gbc", "switch", "ps3", "3ds"]);

// Slugs that have an official logo SVG bundled (the rest fall back to the
// canonical text name). Most logos keep their brand colours on the dark card;
// only the monochrome / dark ones (WHITEN_SLUGS) get inverted to white.
const LOGO_SLUGS = new Set([
  "ps1", "ps2", "ps3", "ps4", "nes", "snes", "n64", "gamecube", "wii",
  "wiiu", "switch", "gba", "gbc", "3ds", "dreamcast", "atari2600", "xbox360",
]);

// Logos that are a single dark colour in source (the PlayStation wordmark and
// the Switch icon + wordmark are black, the GameCube wordmark too, the Wii mark
// silver-grey). These get a white filter so they read on the dark cards; every
// other logo keeps its colour as-is.
const WHITEN_SLUGS = new Set(["ps1", "wii", "gamecube", "switch"]);

// Logos bundled as PNG instead of SVG (the official Switch icon + wordmark
// lockup is only available as a raster).
const PNG_LOGOS = new Set(["switch"]);

// Square icon-over-wordmark logos (Switch) look small next to the wide
// wordmarks at the same height, so they render larger on the cards.
const BIG_LOGOS = new Set(["switch"]);

function normalize(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

/** Resolve a console/library name to its render, logo, + canonical name. */
export function consoleArt(
  name: string,
):
  | (ConsoleArt & {
      render: string;
      logo: string | null;
      pixel: boolean;
      whiten: boolean;
      big: boolean;
    })
  | null {
  const n = normalize(name);
  const hit = BY_ALIAS.get(n) ?? BY_ALIAS.get(n.replace(/\s+/g, ""));
  if (!hit) return null;
  const ext = PNG_LOGOS.has(hit.slug) ? "png" : "svg";
  return {
    ...hit,
    render: `/console-art/${hit.slug}.png`,
    logo: LOGO_SLUGS.has(hit.slug)
      ? `/console-art/logos/${hit.slug}.${ext}`
      : null,
    pixel: PIXEL_SLUGS.has(hit.slug),
    whiten: WHITEN_SLUGS.has(hit.slug),
    big: BIG_LOGOS.has(hit.slug),
  };
}
