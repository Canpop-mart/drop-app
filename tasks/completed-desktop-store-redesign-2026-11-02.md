# Task: Redesign the desktop store

## Goal
Overhaul the desktop Store into a more distinctive and usable discovery experience while preserving search, browse,
filters, shelves, roulette, collections, addons, loading, empty, and error behavior.
Source: owner request.

## Map(s) used
`docs/map/desktop-frontend.md`

## Plan
- [x] Audit the current featured area, discovery sections, shelves, browse controls, and game cards.
- [x] Define a clear storefront hierarchy with more personality and less competing chrome.
- [x] Replace the rejected update-led first pass with a discovery-led desktop Store.
- [x] Preserve and improve loading, empty, error, filter, pagination, and narrow-window behavior in code.
- [x] Run Nuxt typecheck and relevant focused tests.

## Done so far
- Archived the desktop library notes in `tasks/completed-desktop-library-redesign-2026-11-02.md`.
- Selected the native desktop Store and its existing discovery components for the next pass.
- `main/pages/store/index.vue`: renamed Featured to Discover and rebuilt its hierarchy around exploration rather than
  release recency. The spotlight now shares a top canvas with roulette. A shuffled,
  dense catalog cross-section and curated collections follow; New to Drop is a smaller supporting shelf.
- Removed Recently Updated and the list-plus-preview discovery widget from the landing page. Recently updated remains
  available as an explicit Browse sort, while Browse remains the full catalog/search/filter surface.
- Removed the rejected genre box and the eyebrow/subtitle pairs from discovery sections. The spotlight still shares
  its top area with roulette.
- Replaced the full-width shelf stack with an asymmetric lower layout: Shuffle and Collections share a matched block,
  with New to Drop spanning beneath both.
- Tightened that layout after device QA: Shuffle is an explicit five-column by two-row grid at wide desktop sizes;
  Collections now uses two columns and shows up to ten entries; the redundant Browse tile was removed and roulette
  fills the right side of the spotlight row.
- Corrected the shared-block alignment after another device screenshot: the two-by-five Collections grid now
  stretches to the same height as Shuffle, and New to Drop spans beneath both columns instead of beginning early
  inside the Shuffle column.
- `main/components/GameRoulette.vue`: redesigned the full-height random picker around a large cover, atmospheric
  cover backdrop, clearer result hierarchy, and a prominent Pick a game / Spin again action. A failed request now has
  its own retryable error presentation instead of looking like an empty catalog.
- `main/pages/store/index.vue`: redesigned the dedicated Collections tab as an editorial bento grid. Its first group
  uses one large anchor card, two small cards, and one wide card before continuing into a denser four-card row.
- `main/components/StoreAddonsTab.vue`: established a scalable desktop addon marketplace with visual addon cards,
  preserved Add/Remove/confirmation/error behavior, responsive two-column growth, and an explanation panel that
  distinguishes Drop addons from game mods. A single addon expands across the available card area.
- `main/pages/store/[id].vue`: rebuilt the desktop Store game page around a bounded 500px cinematic hero that keeps
  the title, short description, tags, cover, and library action together. Replaced the viewport-sized image wall with
  a wide 21:8 media stage, moved the full description into a readable main column, kept facts and support widgets in
  a quiet sticky rail, and moved achievements into a full-width grid. All object-backed images now use
  `objectImageUrl()` so they also load under `pnpm tauri dev`.
- Added explicit retry states for the primary game load, library membership, store metadata, and achievements. The
  previous page could spin forever or silently hide required sections.
- Follow-up: available mods are optional discovery content and appear only when at least one mod is returned. Empty
  results and mod-service failures remain hidden because nearly all Store games have no mods.
- Audited desktop Browse behavior. Fixed ROM/native and release-year filters being applied only after a server page
  arrived, which produced false empty pages and incorrect totals. `main/composables/use-server-api.ts` now sends the
  filters to the Store endpoint; the sibling server applies them before count and pagination.
- `main/pages/store/index.vue`: added Best match for active searches, Newest releases, and reverse alphabetical
  sorting. Search now visibly selects the sort it actually uses, and the toolbar always reports the filtered catalog
  total instead of a misleading current-page count.
- Fixed Store game pages failing for some catalog entries. `main/pages/store/[id].vue` now loads metadata through the
  store-scoped API instead of `useGame()` / Tauri `fetch_game`, which is the library and local-status path.
  `main/composables/use-server-api.ts` exposes the typed Store detail request, and Store/collection click-through no
  longer fires a library-only prefetch that can fail for catalog-only games.
- Kept the coherent 1900px Store column, sticky command header, fixed-height spotlight, and dense Browse catalog from
  the first pass.
- Added distinct visible error states with Retry for complete Featured failure, discovery-tab failure, Browse initial
  failure, Browse refresh failure while retaining stale results, and Collections failure. These previously collapsed
  into empty content or only logged to the console.
- Added visible Retry states for failed tag and library filter catalogs in Browse.
- Checks after the revised direction: `pnpm exec nuxt typecheck` exit=0;
  `node --test tests/store-browse-state.test.ts` exit=0, 14 tests passed;
  `node --test tests/addon-shop.test.ts` exit=0, 8 tests passed; targeted `git diff --check` exit=0.

## Findings
- BPM is intentionally out of scope until this revised desktop storefront is approved.
- The existing BPM addon shop was not redesigned in this desktop pass. Its shared addon state and controller behavior
  remain unchanged.
- Browse/filter/pagination behavior and its session snapshot remain unchanged; this pass changes presentation and
  makes existing failures visible.

## Not checked
- Compiles, untested on device. The mixed spotlight/discovery canvas, shuffled grid, collection cards,
  dedicated Collections bento grid, random picker, Addons marketplace, and dense Browse grid need visual QA at wide
  and narrow desktop sizes.

## New player-facing strings
- `main/pages/store/index.vue:117` "Spotlight".
- `main/pages/store/index.vue:203-213` "Shuffle", "Shuffling...", and "Shuffle again".
- `main/pages/store/index.vue:229-230,262` "Collections", "See all", and "New to Drop".
- `main/pages/store/index.vue:277,327,700,907,1499` empty/failure text, "Libraries unavailable. Retry",
  tag failure/Retry, "Discover", and the complete discovery-feed failure detail.
- `main/components/GameRoulette.vue:60,73-76,102` revised picker copy and its distinct failure state.
- `main/components/StoreAddonsTab.vue:8-12,52,140-152` Addons header/count, "Drop addon", and the addon explanation
  panel. Existing Add/Remove and error strings remain in the same file.
- `main/pages/store/[id].vue:7,31-37,76-94` page-load, library-membership, mod, metadata, and achievement status/error
  strings introduced or revised by the Store game-page redesign.
- `main/pages/store/index.vue:357,1010-1015`: the always-visible filtered game count, "Best match",
  "Newest releases", and "Z-A".

## Next
Run Discover, Collections, and Addons in the Tauri app at wide and narrow sizes. Capture the three surfaces, then use
that QA before adapting the storefront hierarchy to BPM.
