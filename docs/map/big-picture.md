# Big Picture Mode (BPM): the system

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
Big Picture Mode is the full-screen controller UI, and the only UI on a Steam Deck in Gaming Mode (gamescope). It has
its own layout, pages under `/bigpicture/**`, a gamepad-driven spatial focus system, an input lock for overlays, an
on-screen keyboard, toasts and ten console themes. Nothing in BPM is reachable by controller unless it is registered
with the focus system or handled by an overlay's own gamepad subscriptions. There is no mouse in gamescope.

## Where the code is
| Path | What's there |
|---|---|
| `main/composables/focus-navigation.ts` | The focus system singleton. `useFocusNavigation()`: `registerElement`, `focusGroup`, `focusElement`, `clearFocus`, `acquireInputLock`/`releaseInputLock`, `restrictFocus`/`unrestrictFocus`, `autoFocusContent`, `setGroupOrder`, `saveFocusSnapshot`/`restoreFocusSnapshot`, `setRouteState`/`getRouteState`, `destroy`. Button wiring in `wireGamepad()` (~line 881) |
| `main/composables/bp-focusable.ts` | `useBpFocusable(elRef, group, opts)` (single element) and `useBpFocusableGroup(group)` (returns `register(el, opts)` for `:ref` callbacks). This is how every page registers controls |
| `main/composables/gamepad.ts` | Polling, `GamepadButton` names, `useGamepad().onButton(button, cb, { bypassInputLock })`, `onButtonRelease`, `onAnyButton`, `vibrate`, `axisValue`, `setGlobalInputLock` |
| `main/composables/big-picture.ts` | `useBigPictureMode()`: `enter`, `exit`, `toggle`, `isActive`. `useGuideButtonToggle()` (Guide button toggles BPM) |
| `main/composables/deck-mode.ts` | `useDeckMode().isGamescope`, `setSessionType` |
| `main/layouts/bigpicture.vue` | BPM shell: theme class, `BigPictureNavRail`, `BigPictureTopBar`, single scroll container `[data-bp-scroll]`, `BigPictureContextBar`, `LaunchErrorDialog`, `BpmDownloadToast`, `BpmAchievementToastHost`, Select-button debug console. Focus CSS (`.bp-focused`, `.bp-ring-focused`, `.bp-pressed`) |
| `main/layouts/bpm-wizard.vue` | Layout for `main/pages/welcome/` onboarding and `main/pages/help/errors.vue`; group order `["content","wizard-chrome"]` |
| `main/components/bigpicture/BigPictureNavRail.vue` | Left rail, group `nav`; sets group order `["nav","content","chrome"]`; Exit BPM button (hidden in gamescope) |
| `main/components/bigpicture/BigPictureTopBar.vue`, `main/components/bigpicture/BigPictureContextBar.vue` | Group `chrome`: avatar button, download strip |
| `main/components/bigpicture/BigPictureDialog.vue` | Confirm/cancel dialog. The reference modal for lock handling |
| `main/components/bigpicture/BigPictureKeyboard.vue` | On-screen keyboard (custom or Steam OSK) |
| `main/components/bigpicture/LaunchErrorDialog.vue` | Global launch-failure dialog with log viewer (in layout) |
| `main/components/bigpicture/BpmSaveConflictDialog.vue` | Cloud-save conflict dialog, mounted by `main/components/SaveSyncConflictHost.vue` when BPM is active |
| `main/components/bigpicture/BpmRetroArchCheatsheet.vue`, `main/components/ImageLightbox.vue` | Other lock-holding overlays (lightbox is shared with desktop, `gamepad` prop) |
| `main/components/bigpicture/BigPictureSectionError.vue` | Retry card for a failed section (whole card is the target) |
| `main/composables/bp-theme.ts` | `themes` array (the ten themes), `useBpmTheme()`: `setTheme`, `setMode`, `toggleMode`; writes `--bpm-*` CSS vars |
| `main/assets/bpm-themes.scss` | Per-theme CSS: `[class*="bpm-theme-"]` zinc overrides, `accent-remap` mixin that remaps Tailwind `blue-*` to the theme accent |
| `main/composables/bp-audio.ts` | `useBpAudio().play("focus"/"select"/"back"/"error")`, `soundProfiles`, `tryGamepadAudioUnlock` |
| `main/composables/bp-idle.ts`, `main/components/bigpicture/BpmScreensaver.vue` | Idle timer and screensaver (home page only) |
| `main/composables/bp-reduced-motion.ts`, `main/composables/ui-zoom.ts` | Reduced motion (`.bpm-reduced-motion`), UI zoom |
| `main/composables/achievements/toast.ts`, `main/components/bigpicture/BpmAchievementToastHost.vue` | BPM achievement toast; `bpmToastHosts` count silences desktop `AchievementToast` |
| `main/components/bigpicture/BpmDownloadToast.vue` | Download-complete toast, A opens the game, B dismisses |

## How it flows
1. Enter: `main/app.vue` `tryEnterBPM()` when `state.sessionType === "gamescope"` or `localStorage["drop:startInBPM"]`;
   desktop `main/components/Header.vue` button; Guide button (`useGuideButtonToggle`). `enter()` sets
   `focusNav.enabled`, goes fullscreen, `router.push("/bigpicture")`. Pages pick the layout with
   `definePageMeta({ layout: "bigpicture" })`.
2. Exit: Nav rail button -> `exit()`. Blocked when `isGamescope` (no desktop to return to). Pushes the saved
   previous route, leaves fullscreen, then `focusNav.destroy()` and `gamepad.destroy()` (clears groups, locks,
   restriction, route state).
3. Focus move: D-pad or left stick -> `navigate(dir)` -> `findNearest` spatial search within the current group, then
   across all groups unless `focusRestriction` is set -> `applyFocus` adds `.bp-focused`, plays sound, scrolls into
   view, fires `onFocus`.
4. Buttons (all skipped while `enabled` is false or `inputLocked` is true): A fires `onSelect`, or `el.click()` when
   there is none; with `onHold`, A held 450 ms fires `onHold` and a tap fires `onSelect` on release. B: if a
   restriction is set, does nothing (the overlay owns B); else focus jumps to `nav`; from `nav`, route-back using
   route state `backTo` or `PARENT_OVERRIDES` (`profile` -> `/bigpicture/community`, `library` -> `/bigpicture`).
   X (North under gamescope) fires `onContext`. LB/RB `cycleGroup` (blocked while restricted). Right stick scrolls.
5. Overlay with its own controls: `acquireInputLock()` -> `setGlobalInputLock(true)` silences every `onButton`
   subscriber without `bypassInputLock` -> overlay subscribes with `{ bypassInputLock: true }` -> on close
   `releaseInputLock(id)`. Locks are a stack of ids; the app unlocks only when the stack is empty.

## Registering a control
```vue
<button :ref="(el: any) => registerContent(el, { onSelect: () => doThing(item.id) })" @click="doThing(item.id)">
```
with `const registerContent = useBpFocusableGroup("content");`. Use group `content` for page controls. Composite
buttons: register the wrapper with class `bp-focus-delegate` and put `bp-focus-ring` on the inner element that
should glow (e.g. Play button, `main/pages/bigpicture/library/[id].vue` ~line 40). Mouse handlers on inner buttons use
`@click.stop`.

## Modal pattern A: input lock (overlay drives its own buttons)
From `main/components/bigpicture/BigPictureDialog.vue:104-153`:
```ts
watch(() => props.visible, (v) => {
  if (v) { lockId = focusNav.acquireInputLock(); wireGamepad(); }   // subs use { bypassInputLock: true }
  else   { unwireGamepad(); focusNav.releaseInputLock(lockId); }
}, { immediate: true });
onUnmounted(() => { unwireGamepad(); if (props.visible) focusNav.releaseInputLock(lockId); });
```
`immediate: true` matters: a dialog can mount with `visible` already true, and without it A/B are bound to nothing.
Pages holding several overlays must release each one in `onUnmounted`: see `main/pages/bigpicture/library/[id].vue:3139-3154`
(options, play menu, shelf picker).

## Modal pattern B: focus restriction (overlay made of registered elements)
`restrictFocus("filter-menu")` on open, `unrestrictFocus("content")` on close, controls registered in that group
(`main/pages/bigpicture/store.vue:1420-1440`; home context menu and sort overlay in `main/pages/bigpicture/index.vue` ~1817, ~2012).
While restricted, focus-nav's B does nothing, so the page must close the overlay on B itself (store: East handler
~1568). `useBpFocusableGroup`'s `onScopeDispose` clears a restriction still pointing at its group when the owner unmounts.

## Both surfaces
Desktop: none of this is active; `focusNav.enabled` is false outside BPM. `main/components/ImageLightbox.vue` takes the lock only
when its `gamepad` prop is set. Desktop `AchievementToast` stays quiet while `BpmAchievementToastHost` is mounted.
BPM gaps vs desktop: no sign-out (desktop `main/pages/settings/account.vue`), see `docs/map/big-picture-pages.md` for page parity.

## Server side / client side
BPM pages (`main/pages/bigpicture/`) call the server through `useServerApi()` and `serverUrl()`; see `docs/map/desktop-frontend.md`.

## Rules and gotchas
- Registration is opt-in. A `@click` without a `:ref` register call is mouse-only; nothing lints it
  (`main/composables/bp-focusable.ts`). Without `onSelect`, A calls `el.click()` (`fireSelect`, `main/composables/focus-navigation.ts` ~919).
- `useBpFocusableGroup` keeps the FIRST options for an element and ignores later calls (`registrations.has(htmlEl)`,
  `main/composables/bp-focusable.ts:89`). Handlers must look state up by id at press time, not close over a row object
  (`onMineRowSelect`, `main/pages/bigpicture/requests.vue:750`).
- Vue calls a function ref with `null` while the node is still in the document, so the registration and
  `currentFocused` can outlive the element. After a retry or row removal call `focusNav.clearFocus()` before
  refocusing (`main/components/bigpicture/BpmStoreAddons.vue` `retry()` ~223, `main/components/bigpicture/game-detail/BpmAddonActions.vue` ~302).
- A leaked lock kills all controller input for the session. On desktop, exiting BPM resets it via `destroy()`; in
  gamescope (the Deck) `exit()` is blocked, so the only way out is restarting the app. Release on close
  AND unmount. An async open must re-check visibility before acquiring (`main/components/bigpicture/BigPictureKeyboard.vue:255-300`, `stale` flag).
- Never take a lock for an overlay that is not rendered: `openPlayMenu` returns early unless `playMenuRenderable`,
  and a watch closes it if it stops rendering (`main/pages/bigpicture/library/[id].vue` ~1515-1560).
- Lists that change while open: anchor focus to item id. Options menu uses `optionsFocusId` (`main/pages/bigpicture/library/[id].vue` ~2358-2384):
  an async Executable row insert once shifted rows so A could land on "Remove from Library".
- Lock handlers with `bypassInputLock` keep firing under a nested lock; the shelf picker guards with
  `pickerHasInput()` so the keyboard above it does not also move it (same file ~2502).
- Focus snapshots restore by index in registration order (`FocusSnapshot`, `main/composables/focus-navigation.ts` ~171); pages with
  async rows restore by identity instead (store `restoreReturnFocus`).
- Physical X/Y report as North/West swapped under gamescope; pick context buttons via `isGamescope` (`wireGamepad`).
- Prefer plain focusable rows over a new modal: update review, play-with-update prompt and new request form are
  rows (game page ~448, ~482; `main/pages/bigpicture/requests.vue` ~311).
- Themes: ten ids in `ThemeId` (`main/composables/bp-theme.ts:16`): steam, xbox, wii, ps2, ds, dreamcast, gamecube, psp, gameboy,
  snes. Intentional purples: gamecube `#524EAA`, snes `#6464B4`, ps2 blue-purple (`#2040c0`, panels `#0e1040` in
  scss). Hardcoded Tailwind `blue-*` is remapped to the theme accent by `accent-remap`; other colours are not.
  Home spotlight hero has one template per theme (`main/pages/bigpicture/index.vue` 25-591).
- `disabled` does not stop A: `fireSelect` calls `onSelect` with no disabled check (`main/composables/focus-navigation.ts`
  ~919-936). Guard inside the handler.
- Hidden is not unreachable: `navigate` only checks `isConnected` (~440), so a control hidden with `v-show` can still get
  focus. Use `v-if` for registered controls.
- Open overlays from a `watch` on their visible flag, not synchronously in a button handler: the gamepad dispatches over
  a live `Set` (`main/composables/gamepad.ts` ~241), so a `bypassInputLock` subscription added during the press receives
  that same press (inferred from code, matches what the existing dialogs do).
- `wireGamepad` reads `isGamescope` once when wiring (~1073), so the X/Y context-button mapping is fixed at BPM entry.
- Scroll belongs to the layout's single `[data-bp-scroll]` container; no `<Transition>` around the page slot
  (white screens on WebKitGTK, comment in `main/layouts/bigpicture.vue`).

## Changing things here
- New control on a BPM page: add `:ref="(el:any) => registerX(el, { onSelect })"` with
  `useBpFocusableGroup("content")`; check it is reachable with the D-pad from neighbours.
- New overlay: prefer focusable rows; else copy `main/components/bigpicture/BigPictureDialog.vue` (lock + `bypassInputLock` subs + unmount
  release) or pattern B (`restrictFocus` + B handler that closes and calls `unrestrictFocus`).
- New theme colour var: `ThemeColors` + all ten entries in `main/composables/bp-theme.ts`, `applyCssVariables`, and
  `main/assets/bpm-themes.scss` blocks together.
- New nav rail page: `navItems` in `main/components/bigpicture/BigPictureNavRail.vue`, page under `main/pages/bigpicture/` with the layout meta,
  and `PARENT_OVERRIDES` in `main/composables/focus-navigation.ts` if it has no index page.

## Tests
No tests for the focus system, lock or themes. BPM logic tests: `node --test tests/stream-targets.test.ts`,
`node --test tests/store-browse-state.test.ts`, `node --test tests/showcase-merge.test.ts` (from `main/`).

## Related maps
`docs/map/big-picture-pages.md`, `docs/map/desktop-frontend.md`, `docs/map/multiplayer.md`, `docs/map/downloads-install.md`.
