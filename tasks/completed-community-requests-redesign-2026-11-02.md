# Task: Redesign desktop Community and Requests

## Goal
Give Community and Requests a wider, more useful desktop layout without changing their existing data flows or actions.
Source: owner request.

## Map(s) used
`docs/map/desktop-frontend.md`

## Plan
- [x] Audit Community data, failure states, and navigation.
- [x] Audit the embedded Requests board and notification bridge.
- [x] Redesign both desktop surfaces while preserving behavior.
- [x] Run frontend typechecks and focused diff checks.

## Done so far
- Archived the completed Store notes in `tasks/completed-desktop-store-redesign-2026-11-02.md`.
- `main/pages/community.vue`: widened the page to the same 1900px desktop canvas used by Store, converted server
  totals into a compact four-cell summary, paired the weekly recap with a live presence panel, and balanced Recent
  activity against the Top players board. Removed the duplicate hidden presence renderer and the unnecessary
  library-only `fetch_game` prefetch when opening a Store game.
- `../drop-server/pages/requests.vue`: the embedded client view now uses the wide desktop canvas and responsive
  two-column Board and My requests grids. Voting, withdrawal, creation, sorting, decision-read messaging, loading,
  empty, and Retry behavior are unchanged.
- `../drop-server/layouts/default.vue`: removed the redundant embedded StoreNav from client mode. The Drop desktop
  shell already provides navigation, and Requests is the only embedded page.
- Checks: drop-app Nuxt typecheck exit=0; drop-server Nuxt typecheck exit=0; focused diff checks exit=0.

## Findings
- Requests remains server-rendered in the desktop iframe, so the client and web board share one action and error
  implementation.
- Big Picture has its own native Community and Requests pages and was intentionally not changed in this desktop pass.

## Not checked
- Compiles, untested on device. Community and Requests need visual QA at wide and narrow desktop sizes.

## New player-facing strings
- `main/pages/community.vue:68` "Playing now".
- `main/pages/community.vue:117` "Recently active".
- `main/pages/community.vue:123` "Nobody is playing right now.".
- `main/pages/community.vue:305-310` "Players", "Games", "Hours", and "Unlocks".

## Next
Open Community and Requests in the Tauri client at wide and narrow desktop sizes and capture both for visual QA.
