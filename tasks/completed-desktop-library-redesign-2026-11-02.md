# Task: Redesign the desktop library

## Goal
Overhaul the desktop library into a clearer, more polished home for browsing and launching games. Preserve all
existing filtering, collections, console/emulator navigation, loading, empty, and error behavior.
Source: owner request and desktop screenshot.

## Map(s) used
`docs/map/desktop-frontend.md`

## Plan
- [x] Audit the current header, continue-playing hero, platform navigation, filters, and game grid.
- [x] Define a cohesive desktop information hierarchy and remove competing visual treatments.
- [x] Implement the first desktop library redesign pass without changing library behavior.
- [x] Preserve loading, empty, error, filtering, density, and narrow-window behavior in code.
- [x] Run Nuxt typecheck and relevant focused tests.

## Done so far
- Archived the completed game-detail redesign notes in `tasks/completed-game-detail-redesign-2026-11-02.md`.
- Selected `main/pages/library/index.vue` and its library card/grid components for the next desktop-only pass.
- `main/pages/library/index.vue`: tightened the sticky command header, search, and Filters control; reduced vertical
  page spacing; shortened the continue-playing feature from 300px to 230px; strengthened its artwork gradients; and
  added the game's cover beside the action so a weak or generic banner still identifies the game.
- `main/components/LibraryRow.vue`: reduced shelf header and card gaps.
- `LibraryConsoleCard.vue`, `LibraryEmulatorCard.vue`, `LibraryCollectionCard.vue`: changed the large 256px-wide
  feature cards into quieter 208x112 navigation cards with smaller art and a restrained lift on hover. This moves
  the actual game grid significantly higher without removing platform or collection navigation.
- Kept all existing search, filters, density, sort, view, collection, console/emulator, loading, empty, and per-section
  Retry behavior unchanged.
- Checks: `pnpm exec nuxt typecheck` exit=0; `node --test tests/console-emulators.test.ts` exit=0, 4 tests passed;
  `node --test tests/store-browse-state.test.ts` exit=0, 14 tests passed; targeted `git diff --check` exit=0.
- Main unfiltered-library correction:
  - `GameTileGrid.vue` now has a dense auto-fill layout with a 168px minimum tile width.
  - `LibraryGrid.vue` exposes that layout explicitly; the main library and filtered results use it while store and
    other promotional grids keep their existing sizing. Large libraries now show substantially more games per row.
- Console-page redesign (`main/pages/library/console/[id].vue`): compacted the platform header; split emulator
  executables out of the game grid into a dedicated management row; added Manage/View routing; and gave ROMs/games
  their own labeled dense grid. Counts now describe games rather than treating the emulator as a game.
- Emulator-management page (`main/pages/library/emulator/[id].vue`): compacted its identity header, softened the
  destructive action, added a real load-error state with Retry, and surfaced failures to open install/core folders
  and uninstall instead of logging them invisibly.
- Rechecked after these changes: Nuxt typecheck exit=0; console/emulator tests exit=0, 4 tests passed; targeted
  `git diff --check` exit=0.

## Findings
- BPM is intentionally out of scope until the desktop library structure is approved.
- The current page has good functional sections but gives the continue-playing hero, consoles, and installed grid
  nearly equal visual weight. The redesign should make game discovery the main task and platform browsing secondary.
- The hero previously relied entirely on a banner/cover stretched across a very wide region. Showing the real cover
  as a separate object keeps the featured game recognizable even when that background asset is abstract or weak.

## Not checked
- Compiles, untested on device. The owner's next screenshot should verify the hero at wide and narrow window sizes,
  horizontal platform scrolling, and how high the first game row now lands.
- Dense grid, console separation, and emulator-management changes compile but are not yet visually checked at narrow
  widths or exercised against real emulator folder/uninstall failures.

## New player-facing strings
- None. This pass only rearranges and restyles existing player-facing content.
- `main/pages/library/console/[id].vue:91` "Runs this platform"
- `main/pages/library/console/[id].vue:108,115,123` "Installed" / "Not installed", "Manage" / "View", "Games"
- `main/pages/library/emulator/[id].vue:60` "Couldn't load this emulator"
- `main/pages/library/emulator/[id].vue:270,281` visible folder-open failure messages

## Next
Run the revised main library, a console page, and the emulator-management page in the Tauri app. Capture wide and
narrow screenshots, then use that visual QA before adapting the approved hierarchy to BPM.
