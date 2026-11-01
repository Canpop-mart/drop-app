# Desktop frontend (drop-app `main/`, non-BPM)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
The windowed desktop UI of the Tauri client: store, library, game detail, community, profile, requests,
multiplayer, download queue, settings, auth/setup screens. It is a Nuxt 3 app (`nuxt generate`) with no Pinia: shared
state is `useState(...)`. Native data comes from Rust via `invoke()` and Tauri events; server data goes through the
`server://` protocol; images through `object://`. Big Picture Mode lives in the same app, see `docs/map/big-picture.md`.

## Where the code is
| Path | What's there |
|---|---|
| `main/app.vue` | Root: `NuxtLayout` + `Suspense`, global hosts (`ModalStack`, `AchievementToast`, `SaveSyncToastHost`, `CoopRoomToastHost`, `RequestNotificationToastHost`, `SaveSyncConflictHost`), `fetch_state`, `setupHooks()`, `initialNavigation()`, `useDownloadListeners()`, BPM auto-enter |
| `main/layouts/default.vue` | `Header` + scroll container `[data-app-scroll]`, error boundary, `DebugConsole` |
| `main/layouts/mini.vue` | `MiniHeader` only: auth, setup, server-unavailable pages |
| `main/components/Header.vue` | Top nav: Store, Library, Community, Requests (`navigation` array), back/forward (`useNavHistory`), quick actions (multiplayer, bell -> `/requests?view=mine`, bug report), Big Picture button, queue and Proton widgets, `HeaderUserWidget` |
| `main/pages/library.vue` | Library parent route: `LibrarySearch` sidebar (only on game pages), error boundary |
| `main/pages/library/[id]/index.vue` | Wraps `GameDetailPage` in a per-game `NuxtErrorBoundary` |
| `main/components/game-detail/Page.vue` | Desktop game page (breakdown below) |
| `main/composables/game-detail/` | Page logic, one file per concern: `use-game-install.ts` `useGameInstall`, `use-game-launch.ts` `useGameLaunch`, `use-game-stats.ts`, `use-game-config.ts`, `use-game-update.ts` `useGameUpdate`/`useUpdateRepair`, `use-mod-install.ts`, `use-installed-mods.ts`, pure `update-review.ts`, `mods-tab.ts`, `quick-install-logic.ts` |
| `main/composables/game.ts` | `useGame(id)` (cache + `fetch_game` + `update_game/{id}` listener), `useGameStatuses`, `deduplicatedInvoke`, `invalidateGame`, `parseStatus`; `update_library` listener |
| `main/composables/use-server-api.ts` | `useServerApi()`: namespaces `store`, `addons`, `community`, `profile`, `playtime`, `emulation`, `games`, `saves`; all via `apiFetch` (`serverUrl`, 20 s timeout, one retry for GET/HEAD and on 504) |
| `main/composables/use-server-fetch.ts` | `serverUrl(path)` -> `server://` URL; `rewriteDescriptionImages(html)` |
| `main/composables/use-object.ts` | `useObject(id)` -> `object://` URL; `objectImageUrl(id)` (uses `server://` only under `import.meta.dev`) |
| `main/composables/useListen.ts` | `useListen(event, handler)`: Tauri `listen` with unlisten on unmount |
| `main/composables/downloads.ts` | `useQueueState`, `useStatsState`, `useDownloadListeners` (`update_queue`, `update_stats`, `download_complete`), `useCompletedDownloads` |
| `main/composables/state-navigation.ts` | `setupHooks()` (auth events -> routes), `initialNavigation(state)` |
| `main/composables/app-state.ts` | `useAppState()` = `useState("state")`, filled from `invoke("fetch_state")` |
| `main/composables/render-markdown.ts`, `main/composables/markdown-cleanup.ts` | `renderMarkdown`: `dropOrphanEmphasis` -> micromark -> `sanitizeHtml` -> `rewriteDescriptionImages` |
| `main/composables/request-notifications.ts` | Unread request decisions poll, `registerRequestBoardFrame` for the requests iframe |
| `main/plugins/` | `coop-room.client.ts` (room polling, both surfaces), `request-notifications.client.ts` (decision poll), `save-sync-errors.client.ts` and `streaming-host-errors.client.ts` (surface backend failures as UI), `dev-invoke.client.ts` / `dev-tracing.client.ts` (dev-mode tracing), `global-error-handler.ts` (logs only), `vuedraggable.ts` |
| `main/nuxt.config.ts` | `extends: [["../libs/drop-base"]]`; `ModalStack` and `createModal` come from the drop-base submodule (not mapped) |

## Page index (`main/pages/`, excluding `main/pages/bigpicture/`)
- `main/pages/index.vue`: redirect to `/store`. `main/pages/quit.vue`: calls `invoke("quit")`.
- `main/pages/store/index.vue`: native store (featured, discovery tabs, shelves, roulette, browse with saved state).
  `main/pages/store/[id].vue`: store game page (`renderMarkdown`). `main/pages/store/collection/[id].vue`: collection.
- `main/pages/library/index.vue`: library grid/rows, collection, console and emulator cards.
  `main/pages/library/collections.vue`, `main/pages/library/collection/[id].vue` (shelves),
  `main/pages/library/console/[id].vue`, `main/pages/library/emulator/[id].vue` (emulator cores).
- `main/pages/community.vue`: native community (sections fail and retry separately). `main/pages/multiplayer.vue`: co-op.
- `main/pages/profile/index.vue`, `main/pages/profile/[id].vue`, `main/pages/profile/edit.vue`,
  `main/pages/profile/favorites.vue`, `main/pages/profile/showcase.vue`.
- `main/pages/requests.vue`: the ONLY `server://` iframe page (server `requests`, `?view=mine` passed through).
- `main/pages/queue.vue`: download queue, optimistic pause/resume. `main/pages/bugreport.vue`: bug report form.
- `main/pages/settings.vue` (sidebar) + `main/pages/settings/`: `index`, `interface`, `downloads`, `compat` (Proton),
  `achievements`, `cloudsaves`, `streaming` (Remote Play), `account` (sign out), `developer`.
- `main/pages/auth/`, `main/pages/setup/`, `main/pages/error/serverunavailable.vue`: layout `mini`.
- `main/pages/welcome/`, `main/pages/help/errors.vue`: layout `bpm-wizard` (controller-driven, see `docs/map/big-picture.md`).

## Game page: `main/components/game-detail/Page.vue`
| Lines | Section |
|---|---|
| 1-41 | Options gear menu (`GameDetailOptionsMenu`) |
| 42-67 | `GameDetailHeader`: banner, title, actions, stat bar |
| 68-169 | Installed versions manager (multi-version); league saves (`GameDetailAddonActions`) |
| 170-197 | Tab bar: About, Community, Mods, Cloud Saves (`detailTabs` ~1345) |
| 198-239 | About: description, gallery, HowLongToBeat, controller support |
| 240-326 | Community: achievements, `GameCommunityTab`, compare user |
| 327-517 | Mods: available and installed, actions |
| 518-527 | Cloud Saves (`CloudSavesPanel`) |
| 528-966 | Lightbox, `InstallModal`, `UpdateReviewModal`, play-with-update, repair update, `LaunchOptionsModal`, confirms (reset achievements, remove, mod/version/game uninstall) |
| 969-1072 | Imports, `await useGame(id)`, `useObject(banner)`, compat summary, composables (`useGameInstall`, `useGameLaunch`, `useGameStats`, `useGameConfig`) |
| 1073-1228 | Mods (`useModInstall`, `useInstalledMods`), installed versions (`refreshInstalls`) |
| 1229-1332 | In-place updates (`useGameUpdate`, `requestPlay`, `useUpdateRepair`), version uninstall |
| 1333-1552 | Tab/modal UI state, community surfaces (`useServerApi`) |
| 1553-1684 | Open install folder, VC++ runtime install, reset achievements, remove from library, compat test result |

## How it flows
1. Startup: `main/app.vue` -> `invoke("fetch_state")` -> `useAppState` -> `initialNavigation` routes to setup, auth or
   library; `setupHooks` listens to `auth/*` events.
2. Game page: route `/library/:id` -> `GameDetailPage` -> `useGame(id)` -> `invoke("fetch_game")`, then
   `listen("update_game/{id}")` updates the cached status ref -> buttons call composables that `invoke(...)`.
3. Server data: page -> `useServerApi().community.x()` -> `apiFetch` -> `fetch(serverUrl("api/v1/..."))` -> Rust
   `server://` handler adds auth -> drop-server.

## Both surfaces
Desktop pages here; BPM pages under `main/pages/bigpicture/` (index in `docs/map/big-picture-pages.md`). Requests is an iframe on
desktop but native in BPM. Store, community, profile and multiplayer are native on both. There is no news page on
either surface. Shared components in `main/components/` used by both: `GameCommunityTab.vue`, `GameFriendsTile.vue`,
`ImageLightbox.vue`, `ProfilePicturePicker.vue`, `SaveSyncConflictHost.vue` (picks `BpmSaveConflictDialog` when BPM is active).

## Server side / client side
Server web UI (the page the requests iframe loads, `isClientRequest()` hides header/footer for
`User-Agent: Drop Desktop Client`, set in `src-tauri/remote/src/utils.rs`): see drop-server `../drop-server/docs/map/web-frontend.md`.

## Rules and gotchas
- Images: `objectImageUrl()` / `useObject()` only. `server://` has no cache; `object://` has memory + disk cache
  (comment in `main/composables/use-object.ts`).
- `serverUrl` strips leading slashes (`path.replace(/^\/+/, "")`); a `//api` path would hit the server 404 page.
- `useGame` is a top-level `await`; a throw blanks the route unless caught. `main/pages/library/[id]/index.vue` wraps it in a
  local `NuxtErrorBoundary` keyed by id.
- Use `useListen` rather than raw `listen` in components: it detaches on unmount, including when unmount beats the
  `listen()` promise (`cancelled` flag).
- `apiFetch` retries only GET/HEAD; POST/DELETE fail once visibly (`isRetryable`).
- Stray bold markers (`**` alone on a line) are dropped before rendering by `dropOrphanEmphasis`
  (`main/composables/markdown-cleanup.ts`); fenced and indented code is kept.
- Two achievement toasts exist; desktop `AchievementToast` stays silent while a BPM host is mounted
  (`main/composables/achievements/toast.ts`).

## Changing things here
- New server call: add a typed method in the right namespace of `useServerApi()` (`main/composables/use-server-api.ts`), call it
  from the page, and give the page a distinct error state with retry (see `main/pages/community.vue` per-section flags).
- New Tauri command used by a page: `invoke("name", {...})` here, Rust command registered in
  `src-tauri/src/lib.rs` `generate_handler!` (see `docs/map/downloads-install.md`). Events: `useListen("event", cb)`.
- New desktop page with a BPM counterpart: add both, or document the gap (Dual-Surface rule).
- Pure logic you want tested: put it in a Vue-free logic module (pattern: `main/composables/coop-room-logic.ts`) and add a test file.

## Tests
From `main/`, one file per run: `node --test tests/<name>.test.ts` (a directory fails). All test pure modules:
| Test file (`main/tests/`) | Covers (`main/composables/`) |
|---|---|
| `achievement-status.test.ts` | `achievements/status.ts`, `achievements/toast.ts` (unlock dedupe) |
| `addon-play.test.ts`, `addon-shop.test.ts` | `addon-play-logic.ts`, `addon-shop-logic.ts` (league addons) |
| `archipelago.test.ts`, `coop-room.test.ts` | `archipelago-logic.ts`, `coop-room-logic.ts` |
| `console-emulators.test.ts`, `install-dir-scan.test.ts` | `console-emulators.ts`, `install-dir-scan.ts` |
| `markdown-cleanup.test.ts`, `profile-themes.test.ts` | `markdown-cleanup.ts` `dropOrphanEmphasis`, `profile-themes.ts` |
| `mods-tab.test.ts`, `quick-install.test.ts`, `update-review.test.ts` | `game-detail/mods-tab.ts`, `game-detail/quick-install-logic.ts`, `game-detail/update-review.ts` |
| `save-sync-state.test.ts`, `request-notifications.test.ts` | `save-sync-state.ts`, `request-notifications-logic.ts` |
| `showcase-merge.test.ts`, `store-browse-state.test.ts`, `stream-targets.test.ts` | `bigpicture/showcase-merge.ts`, `bigpicture/store-browse-state.ts`, `bigpicture/stream-targets.ts` |

Typecheck: `pnpm exec nuxt typecheck` from `main/` (needs `node_modules`).

## Related maps
`docs/map/big-picture.md`, `docs/map/big-picture-pages.md`, `docs/map/downloads-install.md`, `docs/map/multiplayer.md`; drop-server `../drop-server/docs/map/web-frontend.md`.
