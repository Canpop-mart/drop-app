# Big Picture pages index

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
A page-by-page index of `main/pages/bigpicture/**` plus a section map of the 3400-line game page, so a reader can
jump to the right lines. For the focus system, input lock, keyboard and themes read `docs/map/big-picture.md` first. All
pages use `definePageMeta({ layout: "bigpicture" })` and register controls with `useBpFocusableGroup(...)`.

## Where the code is
| Path | What's there |
|---|---|
| `main/pages/bigpicture/index.vue` | Home and library in one page (2245 lines): themed spotlight hero (25-591, one `<template v-if="theme === ...">` per console), rails: Continue Playing, Collections, Consoles (592-722), All Games grid with sticky sort/filter/search header (723-912), keyboards (913+), context menu (`restrictFocus("context-menu")` ~1817), sort overlay (`restrictFocus("sort-overlay")` ~2012), bulk select. Uses `main/composables/bigpicture/use-bpm-home-data.ts`, `main/components/bigpicture/BigPictureGameTile.vue`, `main/components/bigpicture/BpmLaunchScreen.vue`, `main/components/bigpicture/BpmScreensaver.vue` |
| `main/pages/bigpicture/library/index.vue` | Redirect only: `/bigpicture/library` -> `/bigpicture` |
| `main/pages/bigpicture/library/[id].vue` | Game page (see breakdown below) |
| `main/pages/bigpicture/store.vue` | Store with tabs Featured, Browse, Collections, Addons (`tabs` ~1671). Browse filter menu (`restrictFocus("filter-menu")` ~1420), bulk shelf picker, search keyboard, `restoreReturnFocus` by game id (~1503). Uses `main/composables/bigpicture/store-browse-state.ts`, `main/components/bigpicture/BpmStoreAddons.vue`, `useShelves` |
| `main/pages/bigpicture/community.vue` | Native community page: stats strip, weekly recap hero, activity feed (clustered), leaderboard; each section has its own error/retry. Uses `useServerApi().community`, `main/composables/use-community-clusters.ts`, `main/components/bigpicture/BigPictureSectionError.vue` |
| `main/pages/bigpicture/requests.vue` | Native game requests: Board, My requests, New request form (plain rows + keyboard, no modal). Desktop shows the server page in an iframe instead. Uses `serverUrl`, `main/components/bigpicture/BigPictureKeyboard.vue`, `main/components/bigpicture/BigPictureDialog.vue` |
| `main/pages/bigpicture/downloads.vue` | Download queue: pause/resume controls, reorder (drag mode takes the input lock, `enterDragMode`/`exitDragMode` ~339-360), history, cancel confirm. Uses `useQueueState` from `main/composables/downloads.ts`, `BigPictureDialog` |
| `main/pages/bigpicture/multiplayer.vue` | Co-op rooms (create/join by code, connect address, leave) and Archipelago panel. Uses `useCoopRoom`, `main/components/bigpicture/BigPictureArchipelago.vue`, two keyboards |
| `main/pages/bigpicture/settings.vue` | Settings sidebar + sections: Theme (theme grid, dark/light, sounds, effects), Interface, Controller, Storage, Compatibility, Achievements (RetroAchievements link via `useRaLink`), Cloud Saves, Streaming, Developer, About. Uses `useBpmTheme`, `useBpAudio`, `useUiZoom`, `useDevMode` |
| `main/pages/bigpicture/profile/index.vue` | Redirect to `/bigpicture/profile/<own username>` |
| `main/pages/bigpicture/profile/[id].vue` | Profile view: banner, stats, game and achievement showcases, recent activity. Uses `objectImageUrl`, `resolveThemeGradient` |
| `main/pages/bigpicture/profile/edit.vue` | Profile editor: banner, name, bio, profile theme, showcases, avatar picker (`main/components/ProfilePicturePicker.vue`, `restrictFocus("picker")` ~816). Uses `main/composables/bigpicture/showcase-merge.ts`, `useServerApi().profile` |
| `main/pages/bigpicture/bugreport.vue` | Bug report form with two keyboards and include-logs toggle; posts via `serverUrl` |
| `main/pages/welcome/`, `main/pages/help/errors.vue` | Onboarding wizard and error reference, layout `bpm-wizard`, shell `main/components/bigpicture/BpmWizardShell.vue`; step order in `main/composables/onboarding.ts` |

## Game page: `main/pages/bigpicture/library/[id].vue`
Template (lines 1-1295):
| Lines | Section |
|---|---|
| 1-35 | Hero banner, title overlay |
| 36-111 | Installed: Play button (delegate wrapper, `onContext: togglePlayMenu`) and play/stream dropdown |
| 112-177 | Resume, Setup, Stop, Update buttons, launch status line, in-flight status |
| 178-205 | Play on another device (stream) |
| 206-277 | Install button with device dropdown |
| 278-299 | Add to Library (not installed, remote) |
| 300-376 | Emulator cycle buttons: controller, quality, widescreen |
| 377-406 | Options button (Start also opens it), stream status/stop |
| 407-481 | Repair stuck update prompt; play-with-update-pending rows |
| 482-741 | In-place update review as focusable rows (backups, conflicts pager ~669, stuck update ~708) |
| 742-782 | Playtime/achievement stats bar, `GameFriendsTile`, league saves (`BpmAddonActions`) |
| 783-813 | Tab bar (About, Community, Mods, Cloud Saves) |
| 814-992 | Community tab: achievement summary, ROM hash banners, achievement list with rarity, error retry, `GameCommunityTab` |
| 993-1028 | About tab: description (`renderMarkdown`) + gallery |
| 1029-1054 | Mods tab (`BpmModsTab`), Cloud Saves tab (`BpmGameSavesTab`, `BpmCloudSavesPanel`) |
| 1055-1116 | Recommended games, `ImageLightbox`, settings and info toasts |
| 1117-1162 | Dialogs: launch error, uninstall, remove from library, save action (`BigPictureDialog`) |
| 1163-1248 | New-shelf keyboard, shelf picker overlay |
| 1249-1295 | Options menu overlay, RetroArch cheatsheet |

Script (lines 1297-3395):
| Lines | Section |
|---|---|
| 1297-1408 | Imports, `gameId`, core refs (`game`, `status`, `version`), multi-version install state |
| 1409-1676 | Streaming (`useBpmGameStreaming`, `main/composables/bigpicture/stream-targets.ts`), play menu: `openPlayMenu` ~1548, `closePlayMenu` ~1572, `runMenuItem` ~1583, `wirePlayMenuGamepad` ~1616; `runDiagnostics`; `focusNav`, `registerAction`, `registerTab` ~1670 |
| 1677-1836 | Markdown, description images, game type, toasts, per-game presets (`useBpmGameConfig`), VC++ runtime install |
| 1837-1951 | Multi-version helpers: `cycleActiveVersion`, `installSelectedVersion`, `loadInstalledVersions` |
| 1952-2165 | In-place updates (`useGameUpdate`, `registerReviewEl`/`focusReviewEl` by key), repair (`useUpdateRepair`), play with update pending (`requestPlay`) |
| 2166-2465 | Options menu: `optionsMenuItems`, id-anchored `optionsFocusId` ~2358, `wireOptionsGamepad` ~2387, lock watch ~2424, uninstall |
| 2466-2592 | Remove from library; shelf picker (`wireShelfGamepad` ~2502, lock watch ~2545) |
| 2593-2697 | Mods (`useBpmMods`), `tabs` computed ~2613, playtime, saves (`useBpmGameSaves`) |
| 2698-2876 | Community surfaces (`useServerApi`), recommended games, tab indicator |
| 2877-3015 | Achievement rarity, achievement loading/retry, ROM hash check, RA cheatsheet |
| 3016-3128 | Main `onMounted`: page gamepad bindings (Start = options, X = play menu), data load |
| 3129-3154 | Resize handler; `onUnmounted` releases all three overlay locks and disposes streaming |
| 3155-3317 | `launchGame` (`invoke("launch_game")`), `killGame`, `resumePartialDownload`, `downloadGame` |
| 3318-3395 | Add to library, open store, recommendations, `checkForUpdates` |

## Both surfaces
Desktop equivalents: home/library -> `main/pages/library/index.vue`; game page -> `main/components/game-detail/Page.vue`;
store -> `main/pages/store/index.vue`; downloads -> `main/pages/queue.vue`; requests -> `main/pages/requests.vue` (iframe).
Desktop pages with no dedicated BPM page: `main/pages/library/collections.vue` and `main/pages/library/console/[id].vue`
(BPM home filters the grid instead via `selectGroup('collection'|'console', ...)`), `main/pages/library/emulator/[id].vue`,
`main/pages/profile/favorites.vue`, `main/pages/profile/showcase.vue` (BPM edits showcases in its profile editor).
BPM has no sign-out (desktop: `main/pages/settings/account.vue`).

## Server side / client side
Server endpoints are reached through `main/composables/use-server-api.ts`; see `docs/map/desktop-frontend.md`.

## Rules and gotchas
- B from a deep page goes to the parent by URL; `library` maps to `/bigpicture` and `profile` to
  `/bigpicture/community` (`PARENT_OVERRIDES`, `main/composables/focus-navigation.ts`). To return elsewhere set
  `focusNav.setRouteState("backTo", origin, target)` before navigating (`main/components/bigpicture/BpmDownloadToast.vue` `goToLibrary`).
- Game page overlays (options, play menu, shelf picker) each hold their own lock id and must all be released in
  `onUnmounted` (lines 3139-3154). Add any new overlay there too.
- Play menu rows carry their own action (`runMenuItem`), and a watch clamps `playMenuFocus` when the list shrinks
  (~1522). It is still index-based, unlike the options menu.
- Shelf picker's `totalItems` is computed once in `wireShelfGamepad` (~2505) (index-based; after "Create New Shelf" the D-pad can't reach "Done", a known bug).
- Sections that fetch must show `BigPictureSectionError` on failure, distinct from empty (home ~635/688/858,
  requests ~76/233).

## Changing things here
- Game page action button: add in 36-406 with a delegate wrapper registered via `registerAction`; wire the
  function in the script section that owns that feature.
- New game page tab: `tabs` computed (~2613), tab content block in 814-1054, register with `registerTab`.
- New options menu row: push `{ id, label, action }` in `optionsMenuItems` (2166-2357); the `id` keeps focus stable.

## Tests
From `main/`, one file per run, e.g. `node --test tests/stream-targets.test.ts`. Files: `main/tests/stream-targets.test.ts`,
`main/tests/store-browse-state.test.ts`, `main/tests/showcase-merge.test.ts`, `main/tests/mods-tab.test.ts`,
`main/tests/update-review.test.ts`, `main/tests/quick-install.test.ts`. Pages
themselves have no tests.

## Related maps
`docs/map/big-picture.md`, `docs/map/desktop-frontend.md`, `docs/map/multiplayer.md`, `docs/map/downloads-install.md`.
