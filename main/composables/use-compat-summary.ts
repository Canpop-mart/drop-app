import { serverUrl } from "./use-server-fetch";

/**
 * Per-(game, platform) compatibility data, fetched from drop-server.
 *
 * Mirrors the shape returned by drop-server's
 * `GET /api/v1/client/compat/library-summary`. Cached per-Nuxt-state for
 * the page lifetime. Call `refreshCompatSummary()` after a test run to
 * force a re-fetch — the per-game CompatPanel listens to this so the
 * badge updates without a page reload.
 *
 * Internally typed as `T | undefined` so the `() => undefined` initial
 * sentinel is honest (Nuxt's `useState<T>` can't accept `undefined` as a
 * non-nullable T's initial value).
 */
export type GameCompatibilityStatus =
  | "Untested"
  | "Installing"
  | "Testing"
  | "AliveRenders"
  | "AliveNoRender"
  | "EarlyExit"
  | "Crash"
  | "NoLaunch"
  | "InstallFailed";

export type ClientPlatform = "Windows" | "Linux" | "macOS";

export type PlatformCompatResult = {
  status: GameCompatibilityStatus;
  signature: string | null;
  protonVersion: string | null;
  testedAt: string;
};

export type GameCompatSummary = Partial<
  Record<ClientPlatform, PlatformCompatResult>
>;
export type CompatLibrarySummary = Record<string, GameCompatSummary>;

async function fetchSummary(): Promise<CompatLibrarySummary> {
  const res = await fetch(
    serverUrl("/api/v1/client/compat/library-summary"),
  );
  if (!res.ok) {
    throw new Error(`compat summary returned ${res.status}`);
  }
  return (await res.json()) as CompatLibrarySummary;
}

export const useCompatSummary = async () => {
  const state = useState<CompatLibrarySummary | undefined>(
    "compat-summary",
    () => undefined,
  );
  if (state.value === undefined) {
    state.value = await fetchSummary();
  }
  return state as Ref<CompatLibrarySummary>;
};

/**
 * Force a re-fetch and update the cached state ref. Call this after a
 * compatibility test completes (or after `confirm_compat_render` lands)
 * so the on-screen panel reflects the new data without requiring a hard
 * page refresh.
 */
export async function refreshCompatSummary(): Promise<void> {
  const state = useState<CompatLibrarySummary | undefined>("compat-summary");
  state.value = await fetchSummary();
}
