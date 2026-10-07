/**
 * Big Picture store: what the player was looking at, so opening a game and
 * pressing Back lands on the same Browse page / search / sort / filter / open
 * collection instead of page one of the default view.
 *
 * The store page is not kept alive (app.vue renders a plain <NuxtPage/>), so
 * every return is a fresh mount. The page seeds its refs from this snapshot
 * before any watcher exists, which keeps the "filter changed, reset to page
 * one" watchers from firing on mount.
 *
 * Pure functions, tested by: node --test tests/store-browse-state.test.ts
 */

export type StoreTab = "featured" | "browse" | "collections";
export type BrowseSort = "default" | "newest" | "name" | "recent";

/** Where focus goes when the player comes back from the game they opened. */
export interface ReturnFocus {
  /** "tile": a Browse or collection grid tile. "roulette": the roulette card. */
  kind: "tile" | "roulette";
  /** The game whose page was opened. Tiles are found by this id, not by index. */
  gameId: string;
  /** scrollTop of the BPM layout's [data-bp-scroll] container when it was opened. */
  scrollTop: number;
}

export interface StoreBrowseSnapshot {
  activeTab: StoreTab;
  searchQuery: string;
  browseSort: BrowseSort;
  browseLibraryFilter: string;
  /** Zero-based. */
  browsePage: number;
  /** Collection detail view that was open on the Collections tab, if any. */
  collectionId: string | null;
  /** Set when a game is opened, consumed by the next store mount. */
  returnFocus: ReturnFocus | null;
}

export const DEFAULT_SNAPSHOT: StoreBrowseSnapshot = {
  activeTab: "featured",
  searchQuery: "",
  browseSort: "default",
  browseLibraryFilter: "",
  browsePage: 0,
  collectionId: null,
  returnFocus: null,
};

const TABS: readonly StoreTab[] = ["featured", "browse", "collections"];
const SORTS: readonly BrowseSort[] = ["default", "newest", "name", "recent"];

const str = (v: unknown, fallback: string) =>
  typeof v === "string" ? v : fallback;

/**
 * Parse a stored snapshot. Anything missing or of the wrong type falls back to
 * its default, so an entry written by an older build (or a corrupt one) can
 * never put the page into a state it can't render. Returns null for no entry
 * or unparseable JSON.
 */
export function parseSnapshot(raw: string | null): StoreBrowseSnapshot | null {
  if (!raw) return null;
  let v: unknown;
  try {
    v = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!v || typeof v !== "object") return null;
  const o = v as Record<string, unknown>;

  const page =
    typeof o.browsePage === "number" && Number.isFinite(o.browsePage)
      ? Math.max(0, Math.floor(o.browsePage))
      : 0;

  let returnFocus: ReturnFocus | null = null;
  const rf = o.returnFocus as Record<string, unknown> | null | undefined;
  if (
    rf &&
    typeof rf === "object" &&
    (rf.kind === "tile" || rf.kind === "roulette") &&
    typeof rf.gameId === "string" &&
    rf.gameId.length > 0
  ) {
    returnFocus = {
      kind: rf.kind,
      gameId: rf.gameId,
      scrollTop:
        typeof rf.scrollTop === "number" && Number.isFinite(rf.scrollTop)
          ? Math.max(0, rf.scrollTop)
          : 0,
    };
  }

  return {
    activeTab: TABS.includes(o.activeTab as StoreTab)
      ? (o.activeTab as StoreTab)
      : DEFAULT_SNAPSHOT.activeTab,
    searchQuery: str(o.searchQuery, ""),
    browseSort: SORTS.includes(o.browseSort as BrowseSort)
      ? (o.browseSort as BrowseSort)
      : DEFAULT_SNAPSHOT.browseSort,
    browseLibraryFilter: str(o.browseLibraryFilter, ""),
    browsePage: page,
    collectionId:
      typeof o.collectionId === "string" && o.collectionId.length > 0
        ? o.collectionId
        : null,
    returnFocus,
  };
}

/**
 * Whether more results must be fetched before `page` can be shown in full:
 * true while fewer than (page + 1) * perPage are loaded and the server still
 * has more.
 */
export function needsMoreResults(
  loaded: number,
  total: number,
  page: number,
  perPage: number,
): boolean {
  const needed = (page + 1) * perPage;
  return loaded < Math.min(needed, total);
}

export function totalPages(total: number, perPage: number): number {
  if (total <= 0 || perPage <= 0) return 0;
  return Math.ceil(total / perPage);
}

/**
 * Clamp a restored page to what the (possibly shrunken) catalog has. Also
 * clamps to what is actually loaded, so a fetch that stopped early (error or
 * a short server page) can't leave the player on a page with no tiles.
 */
export function clampPage(
  page: number,
  total: number,
  loaded: number,
  perPage: number,
): number {
  if (perPage <= 0) return 0;
  const lastByTotal = totalPages(total, perPage) - 1;
  const lastByLoaded = totalPages(loaded, perPage) - 1;
  const last = Math.min(lastByTotal, lastByLoaded);
  if (last < 0) return 0;
  return Math.min(Math.max(0, Math.floor(page)), last);
}

/**
 * Should a pending return-focus be honoured on this mount? Only when the page
 * the player came from is the game page they opened (Back, or the nav rail
 * straight from that page). A later, unrelated visit to the store keeps the
 * list state but not the focus jump.
 */
export function shouldRestoreFocus(
  rf: ReturnFocus | null,
  previousPath: string | null | undefined,
): rf is ReturnFocus {
  if (!rf || !previousPath) return false;
  const path = previousPath.split(/[?#]/)[0].replace(/\/+$/, "");
  return path === `/bigpicture/library/${rf.gameId}`;
}

// ── Storage ─────────────────────────────────────────────────────────────
// Real module state (this file is imported, not a <script setup> body, so the
// `let` lives once per app session), backed by sessionStorage so it also
// survives a module re-evaluation (dev HMR). Cleared when the app closes.

const STORAGE_KEY = "drop.bpm.store.browseSnapshot";
let cached: StoreBrowseSnapshot | null = null;

export function readStoreSnapshot(): StoreBrowseSnapshot {
  if (cached) return cached;
  let raw: string | null = null;
  try {
    raw = globalThis.sessionStorage?.getItem(STORAGE_KEY) ?? null;
  } catch {
    // Storage unavailable: the in-memory copy is enough for this session.
  }
  cached = parseSnapshot(raw) ?? { ...DEFAULT_SNAPSHOT };
  return cached;
}

export function writeStoreSnapshot(patch: Partial<StoreBrowseSnapshot>) {
  cached = { ...readStoreSnapshot(), ...patch };
  try {
    globalThis.sessionStorage?.setItem(STORAGE_KEY, JSON.stringify(cached));
  } catch {
    // Quota or disabled storage: the in-memory copy still works this session.
  }
}

export type FocusPlan =
  | { kind: "roulette" }
  | { kind: "tile"; gameId: string }
  | { kind: "first-tile" };

/**
 * Pick the element to focus once the grid has rendered. The opened game's
 * tile when it is on the visible page, the roulette card when that was used,
 * otherwise the first tile of the page.
 */
export function planReturnFocus(
  rf: ReturnFocus,
  visibleGameIds: readonly string[],
): FocusPlan {
  if (rf.kind === "roulette") return { kind: "roulette" };
  if (visibleGameIds.includes(rf.gameId)) {
    return { kind: "tile", gameId: rf.gameId };
  }
  return { kind: "first-tile" };
}
