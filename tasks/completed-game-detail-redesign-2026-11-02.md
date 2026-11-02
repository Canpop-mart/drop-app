# Task: Redesign the game detail experience on desktop and Big Picture Mode

## Goal
Use the game detail screen as the pilot for a broader Drop UI overhaul. Make its primary actions and information
clearer, give it more personality, and preserve complete desktop and controller-first BPM behavior.
Source: owner request.

## Map(s) used
`docs/map/big-picture.md`

## Plan
- [x] Inventory the existing desktop and BPM game-detail layouts, actions, states, and shared components.
- [x] Define the paired desktop/BPM information architecture and visual direction.
- [x] Implement the desktop redesign without changing game behavior.
- [x] Implement the BPM redesign with complete focus registration and safe overlay/input-lock behavior.
- [x] Preserve the existing loading, empty, error, disabled, and changing-list state behavior on both surfaces.
- [x] Run `pnpm exec nuxt typecheck` and relevant focused frontend tests.

## Done so far
- Paused the previous Waves 1-3 task in `tasks/paused-waves-1-3-2026-11-02.md`; its uncommitted changes remain untouched.
- Selected the desktop and BPM game detail pages as the redesign pilot.
- `main/components/game-detail/Header.vue`: rebuilt the desktop hero treatment with a deeper artwork field, stronger
  title/description hierarchy, a contained action row, and a calmer glass summary panel.
- `main/components/game-detail/OptionsMenu.vue`: replaced the easy-to-miss floating gear with a labeled
  `Game options` control and a larger, right-aligned action menu. Existing action gates and emits are unchanged.
- `main/components/game-detail/Page.vue`: restyled the page shell, installed-build manager, single-build affordance,
  and tab navigation into one consistent layered surface. Existing state and action logic is unchanged.
- `main/pages/bigpicture/library/[id].vue`: applied the same identity/action hierarchy to BPM with a taller artwork
  hero and a contained, wrapping controller action zone. Every existing focus registration, handler, overlay, and
  input lock remains in place.
- Checks: `pnpm exec nuxt typecheck` exit=0; `node --test tests/store-browse-state.test.ts
  tests/showcase-merge.test.ts tests/stream-targets.test.ts` exit=0, 52 tests passed; targeted `git diff --check`
  exit=0.
- Desktop visual-QA correction from the owner's screenshot:
  - `Header.vue`: shortened the artwork field, removed the decorative eyebrow and redundant local Store button,
    constrained the content width, and changed the stats from a full-width panel to quiet inline metadata.
  - `Page.vue`: removed the standalone version-management block from between the hero and tabs. Added a Versions
    tab with version selection, update, uninstall, and install-another-version actions. Removed the separate tab/body
    background band and changed About's main description and gallery from nested collapsible cards to editorial
    sections on the page surface.
  - `OptionsMenu.vue`: aligned Game options with the hero content instead of leaving it in the artwork's empty corner.
  - BPM was intentionally not changed in this correction; settle the desktop structure before adapting it for a pad.
- Re-ran checks after the structural correction: Nuxt typecheck exit=0; 52 focused tests passed; targeted
  `git diff --check` exit=0.
- Desktop settings consolidation from the owner's second screenshot pass:
  - `Header.vue`: Settings is now a normal secondary action beside Play; the floating dropdown is gone.
  - Deleted the now-unused `main/components/game-detail/OptionsMenu.vue`.
  - `GameOptionsModal.vue`: widened and refreshed the existing configuration window. It now contains Video &
    Controls when applicable, Launch, Updates, Storage, Versions, Cloud Saves, and Manage. Versions retains version
    selection/install/update/uninstall behavior. Manage contains account name, install folder, optional compatibility
    runtimes, achievement reset, uninstall, and remove-from-library actions, with destructive actions separated.
  - Opening another dialog or install/update flow from settings closes settings first, preventing stacked modals.
  - `Page.vue`: removed Versions and Cloud Saves from the page tab strip, leaving content-oriented tabs only.
- Description-width correction: the rendered description now uses the full main column and stretches promotional
  images to that column; paragraphs and lists remain capped at 90 characters for readable lines.
- Final checks after consolidation: Nuxt typecheck exit=0; 52 focused tests passed; targeted `git diff --check`
  exit=0.

## Findings
- The working tree already contains unrelated, uncommitted Waves 1-3 changes. Preserve them and avoid rewriting their
  touched sections unless the redesign requires it.
- Desktop and BPM now share an information hierarchy, not identical markup: artwork and identity first, primary
  action zone second, secondary management third, deeper content last.
- The game-specific Store button was redundant with the persistent Store navigation and was removed from the hero.
  Version actions were moved, not removed.

## Not checked
- Compiles, untested on device. The Nuxt dev server started successfully, but the isolated visual browser could not
  reach `127.0.0.1`, so desktop spacing and BPM controller traversal still need in-app visual/device QA.

## New player-facing strings
- `main/pages/bigpicture/library/[id].vue:25` "Game details"
- `main/components/game-detail/Header.vue:91` "Settings"
- `main/components/GameOptionsModal.vue:5` "Game settings"
- `main/components/GameOptionsModal.vue:48-52` "Versions", "Choose the version used by the main Play button.",
  "Install another version"
- `main/components/GameOptionsModal.vue:80-92` "Manage game", "Tools and actions for this installation.", action
  labels/descriptions, "Remove game", "Uninstall files", and "Remove from library"
- `main/components/game-detail/Page.vue` "About {game name}" and "Gallery" remain as the page's content headings.

## Next
Run the revised desktop page in the Tauri app and capture another screenshot. Check long titles, long descriptions,
one and several installed versions, and a narrow window. After the desktop structure is approved, adapt the same
hierarchy to BPM and traverse every action with a controller.
