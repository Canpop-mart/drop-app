/**
 * Pure rules behind the game-detail Mods tab, shared by the desktop page and
 * the Big Picture tab.
 *
 * Kept out of the page components so the moving parts — "should the tab be
 * there at all", "where does the selection go when it isn't", and "what rows
 * does the tab actually show" — can be reasoned about (and exercised) without
 * mounting a page.
 */

/** A prerequisite declared on a mod's latest version. */
export type ModRequirement = { gameId: string; name: string };

/** A mod the server lists for this base game (`fetch_game_mods`). */
export type AvailableMod = {
  id: string;
  mName: string;
  mShortDescription: string;
  mIconObjectId: string;
  requiredMods?: ModRequirement[];
};

/** A mod found on disk under the parent's install dir (`list_installed_mods`). */
export type InstalledModEntry = { gameId: string; fileCount?: number };

/** One mod as both surfaces render it: the server listing and the on-disk
 *  ledger folded into a single row/card. */
export type ModCard = {
  id: string;
  name: string;
  description: string;
  iconObjectId: string | null;
  /** Prerequisite mod names, for the "Requires:" line. */
  requires: string[];
  installed: boolean;
  fileCount: number | null;
  /** Installed, but the server no longer lists it. Nothing is known about it
   *  beyond its id and file count, and uninstall is the only thing to offer. */
  unlisted: boolean;
};

/**
 * The Mods tab only makes sense for an installed game that actually has mods.
 *
 * While the available-mod list is still in flight we keep the tab visible: the
 * tab strip is painted from the first frame, so hiding it during the fetch and
 * restoring it a moment later reads as the tab flickering in and out.
 *
 * An installed mod that the server no longer lists still counts — otherwise
 * uninstalling it would be impossible once the listing went away.
 */
export function shouldShowModsTab(opts: {
  installed: boolean;
  loaded: boolean;
  availableCount: number;
  installedCount: number;
}): boolean {
  if (!opts.installed) return false;
  if (!opts.loaded) return true;
  return opts.availableCount > 0 || opts.installedCount > 0;
}

/**
 * Keep the selected tab pointed at a tab that exists. Returns `active` when it
 * is still visible, otherwise `fallback` — so a Mods tab that disappears
 * underneath the user lands them on About instead of on a blank panel.
 */
export function resolveActiveTab<T extends string>(
  visible: readonly T[],
  active: T,
  fallback: T,
): T {
  return visible.includes(active) ? active : fallback;
}

/**
 * Name for a mod id. The on-disk ledger only stores ids, so an installed mod
 * the server has since stopped listing falls back to showing its id — which is
 * ugly but is the only handle the user has on the thing they need to remove.
 */
export function modDisplayName(
  available: readonly AvailableMod[],
  modId: string,
): string {
  return available.find((m) => m.id === modId)?.mName ?? modId;
}

/**
 * Fold the server listing and the on-disk ledger into one ordered set of mods.
 *
 * Server order is preserved and installed-but-unlisted mods are appended, so a
 * card never moves under the cursor when its install state changes — which
 * matters much more on a controller than sorting installed ones to the front.
 */
export function buildModCards(
  available: readonly AvailableMod[],
  installed: readonly InstalledModEntry[],
): ModCard[] {
  const installedById = new Map(installed.map((m) => [m.gameId, m]));

  const cards: ModCard[] = available.map((mod) => {
    const onDisk = installedById.get(mod.id);
    return {
      id: mod.id,
      name: mod.mName,
      description: mod.mShortDescription ?? "",
      iconObjectId: mod.mIconObjectId || null,
      requires: (mod.requiredMods ?? []).map((r) => r.name),
      installed: onDisk !== undefined,
      fileCount: onDisk?.fileCount ?? null,
      unlisted: false,
    };
  });

  const listed = new Set(available.map((m) => m.id));
  for (const entry of installed) {
    if (listed.has(entry.gameId)) continue;
    cards.push({
      id: entry.gameId,
      name: entry.gameId,
      description: "",
      iconObjectId: null,
      requires: [],
      installed: true,
      fileCount: entry.fileCount ?? null,
      unlisted: true,
    });
  }

  return cards;
}

/**
 * Installed mods on this game that require `modId` — surfaced as a warning
 * before uninstalling a prerequisite like SMAPI, which would otherwise
 * silently break everything stacked on top of it.
 */
export function modDependentNames(
  available: readonly AvailableMod[],
  installed: readonly InstalledModEntry[],
  modId: string | null,
): string[] {
  if (!modId) return [];
  return installed
    .filter((m) => m.gameId !== modId)
    .filter((m) =>
      available
        .find((a) => a.id === m.gameId)
        ?.requiredMods?.some((r) => r.gameId === modId),
    )
    .map((m) => modDisplayName(available, m.gameId));
}
