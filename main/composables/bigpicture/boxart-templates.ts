/**
 * The console box-art frames, and where each one expects the cover art to sit.
 *
 * `BpmBoxArtOverlay` paints a frame PNG full-bleed over a tile. That works for
 * most of them because they are *open* frames: a header band, a spine, a couple
 * of rating badges, all painted straight onto the artwork with nothing behind
 * them. A few are not. The DS cartridge and the Switch case are *closed* frames:
 * an opaque body with a hole cut in it, and the cover art is meant to appear
 * only inside that hole. Painting a full-bleed cover behind one of those puts
 * the artwork in the wrong place, then hides the parts that landed under the
 * body — which is the "the box art is cropped and misaligned" report from the
 * Steam Deck.
 *
 * `window` is that hole, measured from the PNG itself rather than guessed:
 * flood-fill the fully transparent pixels, and if the largest region does not
 * touch the image border, it is an enclosed art window and its bounding box is
 * recorded here as percentages of the template. Templates whose transparent
 * region reaches the border are open frames and get `window: null`.
 *
 * To re-derive after adding or replacing a template (needs Pillow):
 *
 *   from PIL import Image; import numpy as np
 *   from collections import deque
 *   im = Image.open("ds.png").convert("RGBA"); w, h = im.size
 *   trans = np.array(im)[:, :, 3] <= 32
 *   # flood-fill each region, keep the largest that never touches x=0/y=0/w-1/h-1,
 *   # then print its bounding box as left/top/right/bottom percentages.
 *
 * The percentages are template-relative, and the frame is stretched to fill its
 * tile (`object-fit: fill`), so they are also tile-relative: the same numbers
 * drive the cover's insets with no extra maths. Stretching does distort a frame
 * whose natural aspect differs from the tile — every tile is a uniform 3:4 grid
 * cell since v5.7.0 and that is deliberate — but it distorts the window by
 * exactly the same amount, so the two stay lined up.
 */
import { usePublicUrl } from "~/composables/use-public-url";

/** Percent insets of a frame's art window, measured from the tile's edges. */
export interface BoxArtWindow {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface BoxArtTemplate {
  /** Path under public/. */
  file: string;
  /**
   * How the frame composites. White-background frames use "multiply" so the
   * white drops out; dark ones use "screen" so the black drops out; frames that
   * are already cut out sit on top untouched.
   */
  blend: "multiply" | "screen" | "none";
  /** The art window, or null when the frame is open and the cover fills it. */
  window: BoxArtWindow | null;
}

const TEMPLATES: Record<string, BoxArtTemplate> = {
  gamecube: { file: "img/boxart/templates/gamecube.png", blend: "multiply", window: null },
  psp: { file: "img/boxart/templates/psp.png", blend: "screen", window: null },
  gameboy: { file: "img/boxart/templates/gameboy.png", blend: "multiply", window: null },
  snes: { file: "img/boxart/templates/snes.png", blend: "multiply", window: null },
  ps2: { file: "img/boxart/templates/ps2.png", blend: "multiply", window: null },
  wii: { file: "img/boxart/templates/wii.png", blend: "multiply", window: null },
  xbox: { file: "img/boxart/templates/xbxo360.png", blend: "multiply", window: null },
  dreamcast: { file: "img/boxart/templates/dreamcast.png", blend: "multiply", window: null },
  ps1: { file: "img/boxart/templates/ps1.png", blend: "none", window: null },
  n64: { file: "img/boxart/templates/n64.png", blend: "none", window: null },
  // 571x600 cartridge, label window at (73,123)-(496,515).
  ds: {
    file: "img/boxart/templates/ds.png",
    blend: "none",
    window: { top: 20.5, right: 12.96, bottom: 14, left: 12.78 },
  },
  // 702x1138 case, sleeve window at (10,31)-(667,1107).
  switch: {
    file: "img/boxart/templates/switch.png",
    blend: "none",
    window: { top: 2.72, right: 4.84, bottom: 2.64, left: 1.42 },
  },
};

export function boxArtTemplate(themeId: string): BoxArtTemplate | null {
  return TEMPLATES[themeId] ?? null;
}

/** Resolved URL for a theme's frame PNG, or null when it has no template. */
export function boxArtTemplateSrc(themeId: string): string | null {
  const template = TEMPLATES[themeId];
  return template ? usePublicUrl(template.file) : null;
}

/** The art window for a theme, or null when the cover should fill the tile. */
export function boxArtWindow(themeId: string): BoxArtWindow | null {
  return TEMPLATES[themeId]?.window ?? null;
}

/**
 * Inline style that parks an absolutely positioned cover in a theme's art
 * window. Null when the theme's frame is open, which is the caller's cue to
 * leave the cover full-bleed.
 */
export function boxArtWindowStyle(
  themeId: string | null | undefined,
): Record<string, string> | null {
  const win = themeId ? boxArtWindow(themeId) : null;
  if (!win) return null;
  return {
    top: `${win.top}%`,
    right: `${win.right}%`,
    bottom: `${win.bottom}%`,
    left: `${win.left}%`,
  };
}
