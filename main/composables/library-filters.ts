/**
 * Library list filter / sort / view state, persisted across navigation.
 *
 * These used to be plain `ref`s inside `pages/library/index.vue`'s setup, so
 * clicking into a game (which unmounts the list page — there's no <KeepAlive>)
 * destroyed them and Back rebuilt the page from defaults. Holding them here as
 * module-level singletons seeded from localStorage means they survive both
 * back-navigation and app restarts, matching the `console-sections.ts` pattern.
 *
 * Only the persistent *values* live here. Transient UI state (the search box,
 * whether the filter drawer is open) stays local to the page — those should
 * reset on navigation.
 */
const PREFIX = "drop:library:";

function read<T>(key: string, fallback: T): T {
  if (typeof window === "undefined") return fallback;
  try {
    const raw = window.localStorage.getItem(PREFIX + key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

// Module-level singletons — every caller shares the same reactive state.
const density = ref<"cover" | "compact">(read("density", "cover"));
const installStateFilter = ref<
  "all" | "installed" | "not-installed" | "updates"
>(read("installState", "all"));
const typeFilter = ref<"all" | "game" | "tool">(read("type", "all"));
const sortOrder = ref<"name-asc" | "name-desc">(read("sort", "name-asc"));
const selectedCollectionIds = ref<string[]>(read("collections", []));
const allGamesCollapsed = ref<boolean>(read("allCollapsed", false));
const allView = ref<string>(read("allView", "all"));
const allSort = ref<"name-asc" | "name-desc" | "last-played" | "most-played">(
  read("allSort", "name-asc"),
);

let initialized = false;

export function useLibraryFilters() {
  if (!initialized && typeof window !== "undefined") {
    initialized = true;
    const persist = (key: string, source: Ref<unknown>) =>
      watch(
        source,
        (v) => {
          try {
            window.localStorage.setItem(PREFIX + key, JSON.stringify(v));
          } catch {
            // ignore — a blocked localStorage just means it won't persist
          }
        },
        { deep: true },
      );

    persist("density", density);
    persist("installState", installStateFilter);
    persist("type", typeFilter);
    persist("sort", sortOrder);
    persist("collections", selectedCollectionIds);
    persist("allCollapsed", allGamesCollapsed);
    persist("allView", allView);
    persist("allSort", allSort);
  }

  return {
    density,
    installStateFilter,
    typeFilter,
    sortOrder,
    selectedCollectionIds,
    allGamesCollapsed,
    allView,
    allSort,
  };
}
