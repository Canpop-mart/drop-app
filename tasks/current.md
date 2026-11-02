# Task: Redesign Big Picture Mode

## Merge-conflict recovery
- Resolved nine Git conflicts plus older embedded stash markers in docs/UPCOMING.md, docs/map/big-picture.md
  and docs/map/in-place-updates.md. Preserved NCAA vertical widget, white stitches/brown accents, Cancel turn,
  addon API methods and current desktop/BPM redesign. Removed duplicate older inline addon/version layout.
- Files: docs/map/addons.md; both AddonActions.vue components; game-detail/Header.vue and Page.vue;
  composables/addon-play.ts, addon-play-logic.ts, use-server-api.ts; this note; three docs above.
- Checks on current main checkout: Nuxt typecheck exit=0; addon-play, addon-shop and focus-navigation tests
  22 passed, exit=0. Marker scan of main/docs/tasks found none. No new player-facing strings.
- No commit or merge continuation performed. Owner must Continue merge after reviewing. Device QA remains pending.

## NCAA 14 addon panel follow-up
- Third owner revision: kept desktop widget shape/placement, added NCAA-specific scoreboard styling in
  `main/components/game-detail/AddonActions.vue`. Warm cream monospace week numerals, gold striped masthead,
  dark field-green surfaces, decorative SVG football play diagram, and gold checkout action. No invented league
  statistics or extra controls. Scoped to addon name `ncaa14`; BPM retains its existing theme-token appearance.
  No new player-facing strings. Nuxt typecheck exit=0. Live visual review pending.
- Second owner revision: desktop panel moved into `GameDetailHeader`'s new aside slot via `Page.vue`.
  On wide screens it is a 360-440px vertical widget beside the title/actions, with a large week number,
  status inset and full-width primary action. Below 1200px it stacks below the header. CSS `:has` reserves
  a column only when a ready panel or error exists, so games without addons retain full header width.
  No duplicate panel instance or fetch. Refresh/Website use labelled icon buttons.
  BPM placement is unchanged in this second revision; the earlier controller-friendly panel remains.
- Second revision checks: Nuxt typecheck exit=0; addon-play tests 11 passed, exit=0; diff check exit=0.
  Live screenshot review still pending. New standalone label: `main/components/game-detail/AddonActions.vue:76`, `Week`.
- Owner requested a visual redesign of the league-save panel.
- Read `docs/INDEX.md`, `docs/map/addons.md`, `docs/workflow.md`; BPM focus rules remain as documented in `docs/map/big-picture.md`.
- `main/components/game-detail/AddonActions.vue`: capped desktop width, separated utility actions into a header,
  added trophy icon, larger league title, week badge and status dot, roomier primary action, responsive league cards.
- `main/components/bigpicture/game-detail/BpmAddonActions.vue`: matching hierarchy with theme tokens and larger text.
  All existing identity-keyed registrations and checkout/replace handlers retained; no new click targets or locks.
- Checks: Nuxt typecheck exit=0; `node --test tests/addon-play.test.ts`, 11 passed, exit=0.
- Not checked: live visual review, physical controller, real save checkout/check-in. Compiles, untested on device.
- New player-facing strings: `main/components/game-detail/AddonActions.vue:69` and
  `main/components/bigpicture/game-detail/BpmAddonActions.vue:79`: `Week {number}`.
- Next: review the panel in the running app with a free save and a checked-out save.
## Goal
Bring BPM through the same screen-by-screen usability and visual cleanup as desktop while keeping it controller-first,
safe across overlays, and compatible with all ten console themes. Source: owner request.

## Map(s) used
`docs/map/big-picture.md`

## Plan
- [x] Inspect the live BPM shell and audit the controller/focus architecture.
- [x] Start with the first visibly broken surface, Requests, and the shared top bar.
- [x] Review Home, Store, Community, game detail, and Settings from owner-provided captures.
- [ ] Review Downloads, Multiplayer, and Profile in the running app.
- [ ] Verify focus registration, overlay cleanup, themes, typecheck, and focused tests after each batch.

## Done so far
- Archived the remaining desktop cleanup notes in `tasks/completed-remaining-desktop-cleanup-2026-11-02.md`.
- Read `docs/map/big-picture.md` completely. Its linked `docs/map/big-picture-pages.md` is missing and needs to be
  restored or removed from the documentation index.
- Inspected the running BPM Requests page. The live capture exposed low-contrast theme surfaces, a redundant nested
  scrollbar, a one-column layout wasting the wide screen, and a shared breadcrumb bug that labelled Requests as Home.
- `main/pages/bigpicture/requests.vue`: moved board and personal requests to responsive two-column grids, replaced
  semi-transparent zinc cards with explicit theme surface and border tokens, made the header sticky, and returned
  scrolling to the layout's single BPM scroll container. Existing focus registrations, vote actions, withdraw flow,
  keyboard, input locks, errors, and identity-keyed rows remain intact.
- `main/components/bigpicture/BigPictureTopBar.vue`: added missing Requests and Multiplayer breadcrumbs.
- `main/pages/bigpicture/index.vue`: replaced the active per-console Home hero with one shared compact hero whose
  palette, motion, and cover trim still follow the ten existing theme IDs. Removed the spotlight game from the
  Continue Playing rail, collapsed an empty Collections rail into a small toolbar action, and kept all actions in
  the BPM focus system.
- `main/pages/bigpicture/settings.vue`: reframed the ten visual choices as compact color themes under Appearance;
  the layout no longer claims to change with the palette. Sound profiles use the same compact two-column treatment.
- `main/components/bigpicture/BigPictureTopBar.vue`: shortened raw controller hardware identifiers in the visible
  status area while retaining the full value as a title.
- `main/pages/bigpicture/library/[id].vue`: reduced the oversized hero, fixed the Play dropdown being clipped into
  what looked like a duplicate Play button, removed emulator controls duplicated by Options, and redesigned Game
  Options into controller-sized Game setup, Versions and updates, and Library groups. Focus remains identity-based
  and the existing input lock lifecycle is unchanged.
- `main/pages/bigpicture/store.vue`: reduced the featured carousel from a full-screen 21:9 block to a compact
  spotlight card with a readable left-aligned information column.
- `main/components/bigpicture/BpmStoreAddons.vue`: added a real Addons heading, an informative empty state, a
  two-column addon grid, and themed borders.
- `main/pages/bigpicture/community.vue`: changed the very wide, sparse activity list to a two-column card grid.
- `main/composables/focus-navigation.ts`: streamlined BPM globally. Focus reveal now uses exact immediate movement instead
  of overlapping smooth animations; LB/RB no longer recenters the whole viewport; D-pad fallback cannot accidentally
  escape into the side rail or chrome; held D-pad and stick repeat are slower; the stick dead zone is more deliberate;
  right-stick scrolling ignores horizontal-only rails; route focus restores in DOM order without overwriting the
  focused element's visible position; and reactive lists keep the same visual slot when the focused item disappears.
  Follow-up: fixed a regression where Right from the side rail could select the top-bar profile instead of entering
  page content. Right from `nav` now explicitly restores the page's remembered `content` target, and spatial fallback
  never chooses another chrome group. A second follow-up makes every focus handoff clear all stale DOM focus markers
  before applying the new one, preventing the sidebar and main content from showing focus simultaneously after async
  remounts or duplicate registrations. A third follow-up identified native browser link focus as the remaining second
  ring: sidebar links are now removed from the native tab order, pointer and controller activation share one route
  handler, BPM focus blurs unrelated native focus, and the route handler retries the content handoff for up to 1.5s
  while async screens register their first target. `focusGroup()` now reports whether the handoff succeeded. Final
  route-level fix: the persistent BPM layout now owns every page handoff, clears the departing page's focus, and waits
  up to 2s for the arriving route's `content` group. This prevents old-page unregister callbacks and page-local
  autofocus from racing the persistent sidebar on Store, Community, Requests, Settings, and other routed screens.
  Controller follow-up: route changes now discard page-local focus restrictions, and Right from the persistent rail
  also self-heals a stale restriction before entering `content`. This fixes the state where rail Up/Down worked but
  every non-Home page remained unreachable.
- `main/pages/bigpicture/store.vue` and `main/pages/bigpicture/community.vue`: carousel indicators are display-only,
  removing several tiny, low-value controller focus stops.
- `main/components/bigpicture/BigPictureContextBar.vue`: shortened the bumper hint to `Section`.
- Check: Nuxt typecheck exit=0.
- Check: addon and Store focused tests, 33 passed.

## Findings
- Latest navigation investigation reproduced a registry identity bug with Vue's actual reactivity in
  `main/tests/focus-navigation.test.ts`: unregister compared a raw registration with a reactive proxy and failed
  to clear the focused control. Marked registrations raw and added recovery when the focused node is detached.
  Before fix: focused-control cleanup regression failed. After fix: all 3 lifecycle/navigation tests pass, exit=0;
  Nuxt typecheck exit=0. No new player-facing strings. Desktop behavior unchanged.
  This is verified at the focus-engine level, not with a physical controller. Earlier notes claiming the full
  navigation issue was fixed were premature. Current live-controller outcome remains unverified.
- The live app was in a light console theme. BPM Requests cards were nearly white with muted gray content because
  `bg-zinc-900/60` was composited over the light themed background instead of using `--bpm-surface`.
- The user interacted with the Drop window during computer-control inspection, so automation yielded immediately and
  no further input was sent.
- Big Picture must remain its own controller-first composition; desktop layouts will not be copied mechanically.
- The ten saved visual theme IDs remain intact. They now select palette and small decorative details over one Home
  hierarchy instead of selecting ten independently maintained Home layouts.
- `tests/profile-themes.test.ts` currently fails only because the app and sibling server copies have LF versus CRLF
  line endings; the assertion compares the complete source text. None of the BPM files changed those copies.

## Not checked
- Compiles, untested with a controller after the Requests layout change.
- Requests and the new shared Home, Store, Community, Addons, and game-detail layouts still need live QA in dark and
  light modes across several console themes.

## New player-facing strings
- `main/pages/bigpicture/settings.vue:34` — Appearance
- `main/pages/bigpicture/settings.vue:43` — One interface, with a different palette and small details.
- `main/pages/bigpicture/library/[id].vue:1273` — Game options
- `main/pages/bigpicture/library/[id].vue:2398` — Game setup
- `main/pages/bigpicture/library/[id].vue:2399` — Versions and updates
- `main/pages/bigpicture/store.vue:87` — Spotlight
- `main/components/bigpicture/BpmStoreAddons.vue:21` — Add features to Drop itself.
- `main/components/bigpicture/BpmStoreAddons.vue:48` — No addons available
- `main/components/bigpicture/BpmStoreAddons.vue:50` — When your server offers features such as Cloud Saves or
  Remote Play, they will appear here.
- `main/components/bigpicture/BigPictureContextBar.vue:68` — Section

## Next
Live-QA this batch with a controller, then continue with Downloads, Multiplayer, and Profile.

## Release diagnosis (2026-11-02)
- The `v7.0` updater-manifest job failed because the source it built still declares `6.1.6` in
  `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, the `drop-app` entry in `src-tauri/Cargo.lock`, and
  `main/package.json`.
- The 6.1.6 artifact names and `latest.json` therefore match the source build. All updater signatures passed.
  The verifier correctly rejected the mismatch between manifest version `6.1.6` and release tag version `7.0`.
- Recovery requires a four-file version bump committed before publishing a matching release tag. Prefer the full
  SemVer form `7.0.0` / `v7.0.0`. Do not rerun the existing release workflow because it reuses the old source.
- No code or release state was changed during this diagnosis.
- Owner approved the release bump. Updated the four authoritative version locations from `6.1.6` to `7.0.0`:
  `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, the `drop-app` entry in `src-tauri/Cargo.lock`, and
  `main/package.json`. No player-facing strings.
