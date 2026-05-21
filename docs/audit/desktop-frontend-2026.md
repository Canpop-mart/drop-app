# Desktop Frontend Audit — 2026

Scope: the Nuxt 3 / Vue 3 layer of the Tauri client (`main/`), restricted to
the **non-BPM desktop surface** — the shared composable layer
(`main/composables/`), the desktop pages (`main/pages/`, excluding
`bigpicture/`), and their components (`main/components/`, excluding
`bigpicture/`).

Big Picture Mode (`pages/bigpicture/`, `components/bigpicture/`,
`focus-navigation.ts`, `bp-audio.ts`, `gamepad.ts`) is covered by a separate
audit and was not touched. `composables/game.ts` was refactored by a parallel
audit and was left alone.

## Summary

| # | Area | Status before | Action |
|---|------|---------------|--------|
| 1 | **`library/[id]/index.vue` monolith** | 1746-line single file | **DECOMPOSED** — 6 child components + 4 composables, page now 326 lines |
| 2 | **Duplicated tile/grid markup** | Copy-pasted across 5 sites | **CONSOLIDATED** — shared `GameTile` + `GameTileGrid` |
| 3 | **Composable state pattern** | 3 patterns, 1 inconsistency (`shelves.ts`) | **DOCUMENTED + FIXED** — `shelves.ts` migrated to `useState` |
| 4 | **Dead code** | `generateGameMeta.ts` orphan, dead refs in detail page | **REMOVED** |
| 5 | **Type safety** | Stray `any` in `state-navigation.ts` | **FIXED** — typed `VersionOption[]` |
| 6 | iframe vs native split | Consistent | Confirmed — no change |
| 7 | Loading/empty/error states | Mostly consistent, ad-hoc in places | Reviewed — see §6, no churn |

## 1. `library/[id]/index.vue` decomposition (headline deliverable)

The library game-detail page was a 1746-line single-file component holding the
banner, three content tabs (About / Gallery / Achievements), the install flow,
the launch/kill/uninstall actions, the per-game options menu, the stats bar,
achievement fetching, ROM-hash verification, and five modals — all inline.

It is now a **326-line coordinator** that wires four composables to six child
components.

### Extracted composables — `composables/game-detail/`

The page mixed four unrelated concerns. Each became a per-game-detail
composable (NOT singletons — fresh refs per call, invoked from `setup()`):

| Composable | Lines | Owns |
|------------|-------|------|
| `use-game-install.ts` | 115 | Version picker, install-dir selection, dependency toggles, `download_game` calls |
| `use-game-launch.ts` | 136 | `launch` / `launchIndex` / `kill` / `uninstall` / `resume`, launch-options + dependency modal state |
| `use-game-stats.ts` | 171 | Stats-bar fetch, achievement list + reset, ROM-hash listener; exports `formatPlaytime` / `formatLastPlayed` |
| `use-game-config.ts` | 135 | Emulated-game presets (controller / quality / aspect), Goldberg "Set Account Name" |

### Extracted components — `components/game-detail/`

| Component | Lines | Owns |
|-----------|-------|------|
| `Header.vue` | 204 | Blurred banner, title, version/update line, action-button row, stat bar |
| `Gallery.vue` | 205 | Image carousel + fullscreen viewer (owns its own index/open state) |
| `Achievements.vue` | 176 | ROM-hash status banner + achievement list |
| `OptionsMenu.vue` | 161 | The gear menu — presets, account name, reset achievements |
| `InstallModal.vue` | 356 | The "Install {game}?" modal — version/dir/dependency UI |
| `LaunchOptionsModal.vue` | 60 | The "pick a launch option" modal |

**Naming note:** Nuxt prefixes nested components with the PascalCased directory
name, so files in `components/game-detail/` are named `GameDetail*`. The files
were named without a redundant `Game`/`GameDetail` prefix (`Header.vue`, not
`GameDetailHeader.vue`) so the auto-generated name is `GameDetailHeader`, not
`GameDetailGameDetailHeader`.

### Design decisions

- **Composables passed in, not re-instantiated.** `OptionsMenu`, `InstallModal`
  and `LaunchOptionsModal` take a `ReturnType<typeof useGameX>` prop so the
  page owns exactly one instance of each composable's state. Children mutate it
  directly (via `v-model` on the composable's refs) rather than holding a
  duplicate copy.
- **Presentational children re-emit.** `Header` has no Tauri knowledge — every
  action button emits upward (`@install`, `@launch`, …) and the page routes it
  to the right composable.
- **Behaviour is identical.** No flow changed. The load-bearing `v-if` guard on
  `GameOptionsModal` (with DecDuck's comment) was preserved verbatim, as were
  the slide/scrollbar styles.

The decomposition rationale is referenced from the page's own header comment so
the next reader knows where the logic went.

## 2. Tile/grid markup consolidation

The box-art tile (cover image + name fallback + ROM/update/installed badges)
and the responsive grid wrapper (`grid-cols-3 sm:… 2xl:grid-cols-8`) were
copy-pasted across **five** places, and had already drifted (ROM badge in some,
update badge in others):

- `components/LibraryGrid.vue`
- `components/StoreShelf.vue`
- `pages/store/index.vue` (Browse grid + loading skeleton)
- `pages/library/collections.vue`
- (the profile showcase uses a similar but intentionally distinct denser tile —
  left as-is, see Deferred)

Two shared components now own this:

- **`GameTile.vue`** — the one canonical box-art card. Deliberately takes an
  already-resolved `coverUrl` rather than an object ID, because callers resolve
  through two different protocols: `useObject()` for native `object://` IDs and
  `serverUrl("api/v1/object/…")` for store metadata. Resolving inside the tile
  would force one strategy on every caller.
- **`GameTileGrid.vue`** — the responsive grid container, with a `density` prop
  (`default` 8-col / `compact` 7-col) covering both observed layouts.

`LibraryGrid` and `StoreShelf` are now thin compositions of these. The tile
markup is no longer duplicated anywhere.

## 3. Composable state architecture

There are 34 composables (the non-BPM subset audited here is ~20). The shared
state layer uses **three deliberate patterns**, and all but one composable were
already consistent. The pattern is now documented here as the canonical
reference:

| Pattern | When | Examples |
|---------|------|----------|
| **`useState`-keyed singleton** | Process-wide shared *data cache* — server data or app state that any component reads | `app-state`, `downloads`, `use-compat-summary`, `proton`, **`shelves` (now)** |
| **Module-level `ref` singleton** | App-wide *settings / UI state* that needs an eager value at module load (before any component mounts) | `dev-mode`, `ui-zoom` |
| **Per-call factory** | Returns fresh state each call — a namespacing helper, not shared state | `useServerApi`, `useStreaming`, `current-page-engine`, the new `game-detail/*` |

**Rule of thumb:** shared *data caches* use `useState` (keyed, SSR-honest,
discoverable by key); *settings* that must initialise from `localStorage`
synchronously at import time use a module `ref`; everything else is a per-call
factory.

### Inconsistency found and fixed: `shelves.ts`

`shelves.ts` is a server-data cache (collections fetched from the server,
shared between the desktop and BPM library views) — squarely in the first
category — but it used module-level `const shelves = ref([])`. Since the app is
`ssr: false` this was not a runtime bug, but it was the lone cache not using
`useState`, making the pattern look ad-hoc.

It was migrated to `useState("shelves", …)` / `useState("shelves-loading", …)`.
Behaviour is unchanged (still a process-wide singleton); it now matches every
other cache and the convention is noted in the file header.

`dev-mode.ts` and `ui-zoom.ts` keep their module refs **on purpose** — they
read `localStorage` at module-init so `devLog()` and the first paint have a
value before any component instantiates the composable. That is the correct use
of the second pattern, not an inconsistency.

## 4. Dead code removed

- **`composables/generateGameMeta.ts`** — a `DownloadableMetadata` factory with
  zero call sites anywhere in the app (only `.nuxt` auto-import scaffolding
  referenced it). Deleted.
- **In the old `library/[id]/index.vue`:** the `descriptionOpen` ref (a
  leftover from the pre-tabbed layout, commented as such), and the on-demand
  `checkRomHash()` / `romHashChecking` pair (a "Phase 3" feature never wired to
  any button — only the launch-time `ra_hash_check` *listener* was used, which
  is preserved in `use-game-stats.ts`). Dropped during the rewrite.

No orphaned non-BPM composables remain — every other composable has live
importers.

## 5. Type safety

- `state-navigation.ts` had `const versions: any[] = await invoke(...)` for the
  remote-install flow. Typed to `invoke<VersionOption[]>(...)`.
- The four new composables and six new components are fully typed. `invoke()`
  calls carry explicit result type parameters; child-component props are typed
  against the shared `~/types` and the composables' exported interfaces.
- One pragmatic `unknown` cast: `Header.vue` emits the compat-test outcome as
  `unknown` and the page casts it to its local `CompatTestOutcome` shape. The
  payload originates from the `CompatTestButton` component's own emit; threading
  its type through would couple three components, so the cast is contained to
  the one boundary.

## 6. Items reviewed, no change needed

- **iframe vs native split.** Consistent with `CLAUDE.md`. `requests.vue` is the
  only pure iframe page in the desktop set and it matches the documented
  `server://` template exactly. `community.vue`, `news.vue`, and `profile/` are
  **native** (they use `useServerApi` + `serverUrl`), which is a deliberate,
  consistent choice — they are not half-iframe/half-native.
- **Error handling.** Desktop pages already avoid `createError()` in render
  paths (which, per `CLAUDE.md`, breaks layouts). `store`, `library`,
  `community`, `news`, `profile`, `store/[id]` all catch `invoke()`/`fetch()`
  failures and fall back to an empty state or inline error block. `useGame()`
  (in the untouched `game.ts`) already throws a plain `Error`, not
  `createError`, by design. The new `game-detail` composables follow the same
  soft-fail convention (stats/achievements/compat all swallow and default).
- **Loading/empty states.** Treatment is reasonably consistent — a spinner or
  "Loading…" line, an icon + message empty state. It is not centralised into a
  shared component; see Deferred.

## Deferred (not done — out of scope or not worth the churn)

- **Shared `LoadingState` / `EmptyState` components.** Each page hand-rolls its
  loading/empty markup. A shared pair would cut ~10 small duplications, but the
  variations (full-page vs in-section, spinner vs text) mean a flexible API;
  deferred as a follow-up — it is polish, not a correctness issue.
- **Profile showcase tile.** `pages/profile/index.vue` has a denser
  cover-art tile (smaller radius, no badges, different grid breakpoints). It
  *could* use `GameTile` with new props, but it is genuinely a different visual
  treatment for a different context; forcing it onto the shared component would
  add conditional props that only it uses. Left as-is.
- **`store/index.vue` (882 lines) and `library/index.vue` (706 lines).** Both
  are large but cohesive single-purpose pages (a store browser, a library
  browser) with most of their length being filter-drawer markup from the
  earlier intentional redesign. They are under the 1500-line bar and decomposing
  them was not in scope; a future pass could extract their filter drawers into a
  shared `FilterDrawer` component.
- **`store/[id].vue` `v-html` of `mDescription`.** The store detail page renders
  the raw description via `v-html` without running it through
  `rewriteDescriptionImages()` (the library detail page does). Server-relative
  `<img>` tags in a store description would not resolve. Flagged for a separate
  fix — it is a pre-existing behaviour, not introduced here.

## Verification

`pnpm typecheck` (`nuxt typecheck`, which regenerates `.nuxt/` and runs
`vue-tsc`) passes with **exit 0 and zero errors** after all changes — the
decomposition, the tile consolidation, the `shelves.ts` migration, the
`state-navigation.ts` type fix, and the `generateGameMeta.ts` removal. No new
type errors were introduced; the run was already clean before and remains
clean.
