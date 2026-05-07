# Changelog

This changelog covers the Canpop fork. For upstream Drop-OSS history, see
[Drop-OSS/drop-app](https://github.com/Drop-OSS/drop-app/commits/main).

The pre-1.0 raw commit dump from the upstream codebase has been moved to
[`changelog.legacy.md`](./changelog.legacy.md).

---

## 2.0.0

First general-use release of the Canpop fork. The 1.x series was effectively
a long beta — the final shape of the achievement system, BPM, and Steam Deck
integration only stabilised over rc.1 → rc.4. 2.0 is the cut where every
in-development feature is either user-ready or hidden behind dev mode, and
the typecheck baseline is clean.

### Headline features (cumulative since the upstream fork point)

- **Achievement system end-to-end.** Steam Web API + RetroAchievements +
  Goldberg `steam_api.dll` auto-detection. Per-game progress UI, in-game
  toast overlays (themed per BPM theme), achievement icons, rarity colours,
  unlock animations, manual reset. Server-side admin tools for game ↔
  achievement mapping and bulk scanning.
- **Big Picture Mode.** Full-screen, gamepad-driven UI for Steam Deck and
  TV. Ten themes (Steam, Xbox, Wii, PS2, DS, Dreamcast, GameCube, PSP, Game
  Boy, SNES) with matching sound profiles, focus-navigation system, on-screen
  keyboard, debug overlay, gamescope auto-detection (boots straight into BPM
  on Game Mode), and per-game shortcuts that register games into the user's
  Steam library.
- **Steam Deck integration.** UMU + GE-Proton auto-install, flatpak build,
  scripts for installing the flatpak directly to a Deck, Decky Loader plugin
  scaffolding for native achievement toasts.
- **Compatibility test orchestrator.** Per-game compat probe runs the game
  for a 45s observation window, classifies the launch (`AliveRenders` /
  `AliveNoRender` / `EarlyExit` / `Crash` / `NoLaunch` / `InstallFailed`),
  scrapes vkd3d-proton / Godot render-failure markers from the launch log
  for the result signature, and posts to the server's compat database. Batch
  worker variant tests every installed game in sequence with a stop button.
  *Gated behind dev mode.*
- **Streaming.** Sunshine install / start / stop, device pairing PIN, host
  game registration, push-based "stream from {device}" sessions across
  registered Drop clients. *Gated behind dev mode.*
- **Cloud save sync.** Ludusavi-backed PC save backup/restore, per-game
  unified local + cloud save browser, conflict resolution dialog. *Gated
  behind dev mode.*
- **Playtime tracking.** Client reports session start/stop to the server,
  populating recent-games shelves and per-game stat bars.
- **Welcome wizard.** First-run BPM-only onboarding (profile, storage,
  Proton, RetroAchievements, controller navigation, common error glossary).
- **Dev mode.** `/settings/developer` and BPM Settings → Developer toggle
  in-development features (compat tests, streaming, cloud saves) and tagged
  debug logs in one place. Off by default for general-use builds.

### Changes since v1.10.0-rc.4

These are the only commits between rc.4 and 2.0:

- **Gate streaming + cloud saves behind dev mode** ([fdc9422](https://github.com/Canpop-mart/drop-app/commit/fdc9422)) —
  streaming UI (StreamButton, settings tab, BPM section, BPM "Stream from
  device" play-menu rows) and cloud-save UI (BPM Saves tab, welcome wizard
  saves step, BpmErrorReference Saves category) now follow the same dev-mode
  gating pattern as compat tests. Adds `/settings/developer` so dev mode
  can be toggled without entering BPM.
- **Refresh transitive deps + drop dead packages** ([1efb649](https://github.com/Canpop-mart/drop-app/commit/1efb649)) —
  bulk `cargo update` + `pnpm update` clears the dependabot critical
  (`simple-git`) and most highs across both ecosystems. Removed dead deps:
  `koa`, `markdown-it`, `scss` placeholder, `tauri@0.15.0` (the
  pre-Tauri-2 npm package), `slice-deque` (declared in Cargo.toml but never
  imported, carrying two double-free advisories). Net: −615 transitive
  packages from the root lockfile.
- **Include `libs/drop-base` as a pnpm workspace package** ([76f40aa](https://github.com/Canpop-mart/drop-app/commit/76f40aa)) —
  the Nuxt layer's deps (`@headlessui/vue`, `@heroicons/vue`) now resolve
  for typechecking, clearing 32 errors caused by missing `createModal` /
  `ModalType` auto-imports.
- **Clear remaining typecheck errors** ([5cf9ed4](https://github.com/Canpop-mart/drop-app/commit/5cf9ed4)) —
  baseline goes from 80 errors → 0. RecentGameEntry now caches `installed`
  and `playtimeSeconds` instead of the page reading non-existent fields off
  the GameStatus union; null guards on `dl_progress`; legacy
  `widescreen === true/false` branches removed (the type is now a string
  union); various small one-offs.
- **Correct error-page redirects + remove dead refs** ([67d7c8b](https://github.com/Canpop-mart/drop-app/commit/67d7c8b)) —
  error.vue's BPM and desktop branches now redirect correctly per surface
  (was sending desktop users into BPM on every button); broken `/docs`
  link replaced with `https://docs.droposs.org`; welcome wizard subtitle
  no longer promises a "Settings → Help" section that doesn't exist;
  removed orphan `emulator-launchids.json`.

### Known issues

- The welcome wizard is BPM-only by design. Desktop first-run users have
  no guided onboarding; they're dropped straight into the library.
- A few residual high-severity dependabot alerts remain on dev-only paths
  (`simple-git` via `@nuxt/devtools`, `node-forge` via `listhen`'s dev
  server). These are accepted risks until upstream Nuxt updates the pins.

---

## v1.10.0-rc.4 — compat polish + emulator render fixes

Fixed the rc.3 tokio panic, made RetroArch 3D-emulator games actually render
with CRT shader on, let users clear batch-test review backlog without
re-launching games, and smoothed out PS2 deinterlace artifacts. See
[48a4b0c](https://github.com/Canpop-mart/drop-app/commit/48a4b0c).

## v1.10.0-rc.3 — compat round 2

`createModal` fix, Proton version detection, batch worker, auto-refresh
after test, log4rs `append=true`. See
[d1403b2](https://github.com/Canpop-mart/drop-app/commit/d1403b2).

## v1.10.0-rc.2 — compatibility test orchestrator

Initial compat orchestrator. See
[3569b39](https://github.com/Canpop-mart/drop-app/commit/3569b39).

## v1.10.0-rc.1 — dev mode overlay, welcome wizard, BPM stability pass

Dev mode debug overlay, BPM welcome wizard, BPM stability fixes. See
[026afcf](https://github.com/Canpop-mart/drop-app/commit/026afcf).

## v1.9.1 and earlier

See [`changelog.legacy.md`](./changelog.legacy.md) for the autogenerated
upstream changelog covering everything before the Canpop fork point.
