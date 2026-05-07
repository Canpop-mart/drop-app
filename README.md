# Drop Desktop Client (Canpop fork)

A custom fork of the official [Drop-OSS](https://github.com/Drop-OSS/drop-app)
desktop client with a substantially expanded feature set on top of the upstream
Tauri + Nuxt 3 base.

Drop itself is the open-source, self-hosted game distribution platform — see
the upstream [Drop server](https://github.com/Drop-OSS/drop) and the
[Drop docs](https://docs.droposs.org) for what the platform does at the macro
level. This README focuses on what's specific to *this* fork.

## What's in this fork

Everything from upstream, plus:

- **Achievement system** — Steam Web API, RetroAchievements, and Goldberg
  `steam_api.dll` auto-detection, with progress UI, in-game toast overlays,
  and admin tooling for linking games / bulk-scanning a server.
- **Big Picture Mode (BPM)** — full-screen, gamepad-driven UI for Steam Deck
  and TV use. Theme system (Steam, Xbox, Wii, PS2, DS, Dreamcast, GameCube,
  PSP, Game Boy, SNES), focus navigation, on-screen keyboard, BPM debug
  overlay, and gamescope auto-detection that boots straight into BPM on Game
  Mode.
- **Steam Deck integration** — auto-installs UMU + GE-Proton, builds a
  flatpak, ships a Decky Loader plugin path for native achievement toasts,
  and registers per-game Steam shortcuts so games show up alongside the
  user's Steam library.
- **Compat orchestrator** — per-game and batch compatibility test runner that
  classifies launches as `AliveRenders`/`AliveNoRender`/`EarlyExit`/`Crash`,
  scrapes vkd3d-proton / Godot render failure signatures from the launch
  log, and posts the results back to the server's compat database. Gated
  behind dev mode while it's still maturing.
- **Streaming (Sunshine + Moonlight)** — Sunshine install/start/stop, device
  pairing PIN flow, push-based "stream from {device}" sessions, host
  registration, and a remote-session inbox for accepting incoming streams.
  Gated behind dev mode.
- **Cloud save sync** — Ludusavi-backed PC save backup/restore plus a
  unified local + cloud save browser per game, with conflict resolution.
  Gated behind dev mode.
- **Playtime tracking** — client reports session start/stop to the server,
  recent-games shelf populates from server-side aggregation.
- **Welcome wizard** — first-run onboarding for BPM users (profile,
  storage, Proton, RetroAchievements, controller navigation, common error
  glossary).
- **Dev mode** — single toggle in `/settings/developer` (or BPM Settings →
  Developer) reveals in-progress features and turns on tagged debug logs.
  Off by default for general-use builds.

## Architecture

Two surfaces, one repo:

- **`drop-server`** (separate repo) — Nuxt 3 server, the source of truth for
  the library, store, achievements, compat, etc.
- **`drop-app`** (this repo) — Tauri desktop client. Native pages for things
  that need OS access (game install, launch, queue, settings) and embedded
  `server://` iframes for things that work fine as web pages (store,
  community, news, requests, profile).

See [`CLAUDE.md`](./CLAUDE.md) for the dual-surface rule and the protocol
details (`server://`, `object://`, etc.).

## Development setup

1. **Server** — set up a Drop server first if you don't have one. Follow the
   [upstream quickstart](https://docs.droposs.org/docs/guides/quickstart).
2. **Submodules** — this repo uses git submodules for the Nuxt layer at
   `libs/drop-base` and the libtailscale binding at `src-tauri/tailscale/libtailscale`.
   ```sh
   git clone --recurse-submodules https://github.com/Canpop-mart/drop-app.git
   # OR, if already cloned:
   git submodule update --init --recursive
   ```
3. **Dependencies** — pnpm-managed workspace. The root, `main/`, and
   `libs/drop-base` are all workspace packages.
   ```sh
   pnpm install
   ```
4. **Run the dev client**:
   ```sh
   pnpm tauri dev
   ```
   On NVIDIA + Linux, use `./nvidia-prop-dev.sh` to work around the GBM
   crash in WebKitGTK.

   Set `RUST_LOG=debug` (or `info`/`warn`/`error`) in front of the command to
   change the Rust-side log level:
   ```sh
   RUST_LOG=debug pnpm tauri dev
   ```
5. **Build a release bundle**:
   ```sh
   pnpm tauri build
   ```
   For Steam Deck (flatpak), use `./build-deck.sh`. For Windows, see
   `scripts/build-windows.ps1`.

## Typechecking

The frontend is typed with `vue-tsc`. Run from `main/`:

```sh
cd main && pnpm typecheck
```

This must pass before any PR. There's no separate lint step yet — the
typechecker plus Vue's template type-checking is the bar.

## Contributing

This is a fork; upstream contribution lives at
[Drop-OSS/drop-app](https://github.com/Drop-OSS/drop-app). Fork-specific work
goes here. See the upstream
[Developer Docs](https://developer.droposs.org/contributing) for the
contribution conventions that still apply.
