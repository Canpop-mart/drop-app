/**
 * Which library games the signed-in user has backed up, for the tile badge.
 *
 * Backed up BY THEM, not merely present on the server. The tile says "your
 * saves are safe" and there is no room on it to qualify that, so a game whose
 * cloud rows all belong to another account gets no badge at all.
 *
 * The constraint that shapes this: a badge on a grid of two hundred tiles must
 * not cost two hundred requests. `saves/summary` answers for the whole library
 * in one, so the set is fetched once, shared by every grid and shelf on the
 * page, and re-used for a few minutes afterwards.
 *
 * A missing badge is not a wrong answer, it is a quieter tile, so every failure
 * here is swallowed. The Cloud Saves settings page is where a real answer
 * lives, and it reports its own errors.
 *
 * Module-level singletons, the same pattern as `library-filters.ts`.
 */
import { invoke } from "@tauri-apps/api/core";
import { hasOwnSaves } from "~/composables/cloud-save-ownership";
import { useServerApi } from "~/composables/use-server-api";

/** How long a fetched set stays good. Backups do not appear by the second. */
const TTL_MS = 5 * 60_000;

const backedUpGameIds = ref<Set<string>>(new Set());

let fetchedAt = 0;
let inflight: Promise<void> | null = null;

/**
 * Whether cloud saves is switched on, cached for the process. Cloud saves is
 * opt-in, and a library grid must not fire a request for a feature the user
 * has never turned on. Re-read after the setting changes costs a restart,
 * which is the right trade for a badge.
 */
let syncEnabled: Promise<boolean> | null = null;

function cloudSavesOn(): Promise<boolean> {
  syncEnabled ??= invoke<{ cloudSavesEnabled?: boolean }>("fetch_settings")
    .then((s) => s?.cloudSavesEnabled === true)
    .catch(() => false);
  return syncEnabled;
}

export function useCloudSaveBadges() {
  // Resolved here, in the caller's setup context, not after the await below.
  const api = useServerApi();

  /**
   * Populate the set if it is missing or stale. Safe to call from every grid
   * and shelf that mounts: concurrent callers share one request.
   */
  async function ensureLoaded(): Promise<void> {
    // Badging tiles for a feature that isn't running would promise a backup
    // that does not exist.
    if (!(await cloudSavesOn())) return;
    if (inflight) return inflight;
    if (backedUpGameIds.value.size > 0 && Date.now() - fetchedAt < TTL_MS) {
      return Promise.resolve();
    }

    inflight = api.saves
      .summaries()
      .then((games) => {
        // Own rows only. The summary lists every game the user can READ, and
        // PC saves are readable across every account on the server, so badging
        // the raw list puts a "your saves are backed up" marker on a housemate's
        // games — including ones this user has never launched.
        backedUpGameIds.value = new Set(
          games.filter(hasOwnSaves).map((g) => g.gameId),
        );
        fetchedAt = Date.now();
      })
      .catch(() => {
        // Leave whatever we had. An empty set means no badges, which is the
        // right amount of noise for an answer we do not have.
      })
      .finally(() => {
        inflight = null;
      });
    return inflight;
  }

  return { backedUpGameIds, ensureLoaded };
}
