/*
What adding an install folder found. `add_download_dir` scans the folder and
returns how many games it picked up; desktop Settings, Big Picture Settings and
the first-run storage step all show the same line. Kept free of Vue and Nuxt so
node --test can run it: main/tests/install-dir-scan.test.ts.
*/

export interface InstallDirScan {
  installed: number;
  partial: number;
  /** Installs found that were already in the library. */
  known: number;
}

/** The backend's reply, checked. Anything else reads as "found nothing". */
export function parseInstallDirScan(raw: unknown): InstallDirScan {
  const r = (raw ?? {}) as Record<string, unknown>;
  const count = (v: unknown) =>
    typeof v === "number" && Number.isFinite(v) && v > 0 ? Math.floor(v) : 0;
  return {
    installed: count(r.installed),
    partial: count(r.partial),
    known: count(r.known),
  };
}

const games = (n: number) => (n === 1 ? "1 game" : `${n} games`);

/** One line for the player. Placeholder copy, the owner rewrites it. */
export function describeInstallDirScan(scan: InstallDirScan): string {
  const total = scan.installed + scan.partial;
  if (total === 0) {
    if (scan.known > 0)
      return `Folder added. ${scan.known === 1 ? "The game" : `All ${scan.known} games`} in it ${scan.known === 1 ? "was" : "were"} already in your library.`;
    return "Folder added. No Drop games were found in it.";
  }
  const found = `Folder added. Found ${games(total)} already in it.`;
  if (scan.partial === 0) return found;
  return `${found} ${scan.partial === 1 ? "1 needs" : `${scan.partial} need`} to finish downloading.`;
}
