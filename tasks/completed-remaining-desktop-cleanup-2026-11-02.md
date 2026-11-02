# Task: Finish the remaining desktop UI cleanup

## Goal
Bring the remaining everyday desktop screens into the approved Store, Library, Community, and Requests visual
system, and fix concrete bugs found during the audit. Source: owner request.

## Map(s) used
`docs/map/desktop-frontend.md`

## Plan
- [x] Inventory the remaining non-BPM pages.
- [x] Update the outdated everyday surfaces without changing their workflows.
- [x] Fix concrete loading and lifecycle bugs found during the audit.
- [x] Run Nuxt typecheck, multiplayer tests, and focused diff checks.

## Done so far
- Archived the Community and Requests notes in `tasks/completed-community-requests-redesign-2026-11-02.md`.
- `main/pages/multiplayer.vue`: expanded the desktop page to the shared 1900px canvas and arranged Host, Join, and
  Open rooms as a responsive dashboard. Existing Co-op and Archipelago behavior, errors, and recovery remain intact.
- `main/pages/queue.vue`: rebuilt Downloads around the shared page header, larger status surface, cleaner download
  cards, and a deliberate empty state. Fixed the window resize listener being installed during setup and never
  removed; it is now attached on mount and detached on unmount.
- `main/pages/settings.vue`: widened the Settings workspace, converted the navigation into a sticky card, and put
  each settings route in a consistent content surface without changing any individual setting behavior.
- `main/pages/bugreport.vue`: brought the report form into the current card language while preserving diagnostics,
  log, screenshot, submission, and error handling.
- `main/pages/profile/index.vue` and `main/pages/profile/[id].vue`: profile load failures now include a working Retry
  action. Public profile game links no longer fire a library-only `fetch_game` prefetch before opening Store pages.
- Checks: Nuxt typecheck exit=0; `node --test tests/coop-room.test.ts` exit=0, 6 tests passed; focused diff check exit=0.

## Findings
- Profile, secondary Library pages, and the focused onboarding/auth flows already use the newer visual language and
  did not justify structural rewrites.
- Big Picture remains intentionally unchanged in this desktop approval phase.
- Repository-wide git status is blocked by sandbox access to the `libs/drop-base` submodule metadata; focused diff
  checks on every changed file passed.

## Not checked
- Compiles, untested on device. Downloads, Multiplayer, Settings, Bug Report, and both profile Retry states need
  visual and interaction QA in the Tauri client.

## New player-facing strings
- `main/pages/queue.vue:5-8` "Downloads" and "Install, update, and reorder your games.".
- `main/pages/queue.vue:199` "Nothing is downloading right now.".
- `main/pages/settings.vue:5` "Manage Drop on this device.".
- `main/pages/profile/index.vue:21` and `main/pages/profile/[id].vue:21` "Retry".

## Next
Open Downloads, Multiplayer, Settings, Bug Report, and a failing profile in the Tauri client for visual QA.
