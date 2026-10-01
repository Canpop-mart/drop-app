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
  /** Newest version on the server; absent from older servers. */
  latestVersionId?: string | null;
};

/**
 * A mod found on disk under the parent's install dir (`list_installed_mods`).
 * `complete` is false for a download that was cancelled, failed or is still
 * running; left out, the entry counts as a finished install.
 */
export type InstalledModEntry = {
  gameId: string;
  fileCount?: number;
  version?: string;
  complete?: boolean;
  downloading?: boolean;
  /** Why the mod's launch override would not be used for the game as it is
   *  installed now: "platform", "unfinished" or "otherMod". Absent when it
   *  would be, or the mod has none. */
  launchOverrideSkipped?: string | null;
  /** With "otherMod": the mod whose override is used instead. */
  launchOverrideWinner?: string | null;
};

/** One mod as both surfaces render it: the server listing and the on-disk
 *  ledger folded into a single row/card. */
export type ModCard = {
  id: string;
  name: string;
  description: string;
  iconObjectId: string | null;
  /** Prerequisite mod names, for the "Requires:" line. */
  requires: string[];
  /** A finished install is on disk. */
  installed: boolean;
  /** A download started but did not finish; offer Resume or Remove. */
  unfinished: boolean;
  /** A download of this mod is queued or running in the download manager. */
  downloading: boolean;
  /** Installed, and the server has a different (newer) version. */
  updateAvailable: boolean;
  fileCount: number | null;
  /** On disk, but the server no longer lists it. Nothing is known about it
   *  beyond its id and file count, and removing it is the only thing to offer. */
  unlisted: boolean;
  /** Why this mod's change to how the game starts is not used, or null. */
  launchNote: string | null;
};

/** What a mod card can do. */
export type ModAction = "install" | "update" | "resume" | "uninstall" | "remove";

/**
 * The card's main action (A on a controller, the coloured button on desktop)
 * and its second one (X / the plain button), or null when there is none.
 * Nothing is offered while a download runs: its files are in use, and the
 * backend refuses to uninstall then anyway.
 */
export function modCardActions(card: ModCard): {
  primary: ModAction | null;
  secondary: ModAction | null;
} {
  if (card.downloading) return { primary: null, secondary: null };
  if (card.unfinished) {
    return {
      primary: card.unlisted ? "remove" : "resume",
      secondary: card.unlisted ? null : "remove",
    };
  }
  if (card.installed) {
    return card.updateAvailable
      ? { primary: "update", secondary: "uninstall" }
      : { primary: "uninstall", secondary: null };
  }
  return { primary: "install", secondary: null };
}

/**
 * The Mods tab only makes sense for an installed game that actually has mods.
 *
 * While the available-mod list is still in flight we keep the tab visible: the
 * tab strip is painted from the first frame, so hiding it during the fetch and
 * restoring it a moment later reads as the tab flickering in and out.
 *
 * An installed mod that the server no longer lists still counts — otherwise
 * uninstalling it would be impossible once the listing went away. So does a
 * failed load, so the error and its Retry are reachable.
 */
export function shouldShowModsTab(opts: {
  installed: boolean;
  loaded: boolean;
  availableCount: number;
  installedCount: number;
  /** Either list failed to load. The tab stays so its error and Retry show;
   *  hiding it would make "could not load" look like "no mods". */
  failed?: boolean;
}): boolean {
  if (!opts.installed) return false;
  if (!opts.loaded || opts.failed) return true;
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
 * Player-facing line for a finished mod whose launch override a launch would
 * skip (see `decide_launch_override` in mod_data.rs), or null. An unfinished
 * install already says it is unfinished, so it gets no second line.
 */
export function launchOverrideNote(
  available: readonly AvailableMod[],
  entry: InstalledModEntry | undefined,
): string | null {
  if (!entry || entry.complete === false) return null;
  switch (entry.launchOverrideSkipped) {
    case "platform":
      return "Its launch change is not used: it has none for this game's platform.";
    case "otherMod":
      return entry.launchOverrideWinner
        ? `Its launch change is not used: ${modDisplayName(available, entry.launchOverrideWinner)} already changes how the game starts.`
        : "Its launch change is not used: another mod already changes how the game starts.";
    default:
      return null;
  }
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
    const installed = onDisk !== undefined && onDisk.complete !== false;
    return {
      id: mod.id,
      name: mod.mName,
      description: mod.mShortDescription ?? "",
      iconObjectId: mod.mIconObjectId || null,
      requires: (mod.requiredMods ?? []).map((r) => r.name),
      installed,
      unfinished: onDisk !== undefined && onDisk.complete === false,
      downloading: onDisk?.downloading === true,
      updateAvailable:
        installed &&
        !!mod.latestVersionId &&
        !!onDisk?.version &&
        mod.latestVersionId !== onDisk.version,
      fileCount: onDisk?.fileCount ?? null,
      unlisted: false,
      launchNote: launchOverrideNote(available, onDisk),
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
      installed: entry.complete !== false,
      unfinished: entry.complete === false,
      downloading: entry.downloading === true,
      updateAvailable: false,
      fileCount: entry.fileCount ?? null,
      unlisted: true,
      launchNote: launchOverrideNote(available, entry),
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

/**
 * Which of a mod's download options to install onto a base game installed for
 * `parentPlatform`. Options come newest first. The newest version's option for
 * the base game's own platform wins: a mod version with its own launch
 * settings only applies its launch override to launches of those platforms
 * (a pure overlay applies it to any). Falls back to the newest option of any
 * platform (the files are the same) when the newest version doesn't list the
 * base game's platform or it isn't known.
 */
export function pickModVersion<T extends { versionId: string; platform: string }>(
  options: readonly T[],
  parentPlatform: string | null | undefined,
): T | undefined {
  const newest = options[0];
  if (!newest || !parentPlatform) return newest;
  return (
    options.find(
      (o) => o.versionId === newest.versionId && o.platform === parentPlatform,
    ) ?? newest
  );
}
