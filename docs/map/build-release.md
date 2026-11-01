# drop-app build and release

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
Builds the Tauri desktop client into installers (NSIS on Windows; deb, rpm, AppImage on Linux) and publishes them as
a GitHub release with a signed `latest.json` that the in-app updater reads. Publishing a GitHub release fires
`.github/workflows/release.yml`, which builds on Ubuntu 22.04 and Windows, repacks the AppImage for SteamOS, re-signs
it, and verifies every updater download. Separate shell scripts cover Steam Deck installs and an old Flatpak path.

## Where the code is
| Path | What's there |
|---|---|
| `package.json` | Root: `build` = `node ./scripts/build.mjs`, `tauri` = `tauri`; `@tauri-apps/cli ^2.11.1`; `packageManager pnpm@10.29.1` |
| `scripts/build.mjs` | Checks `libs/drop-base/package.json` exists, then for each top-level dir with a `package.json` (only `main`): `pnpm install`, `pnpm run build` with `NUXT_APP_BASE_URL=/<view>/`, copies `.output/public` to `./.output/<view>`; copies `src-tauri/icons/icon.ico` to `.output/favicon.ico` (the `icons/` dir is not in this snapshot; binaries look excluded) |
| `main/package.json` | `build` = `nuxt generate`, `dev`, `typecheck`, `postinstall` = `node scripts/patch-vite-builder.mjs && nuxt prepare` |
| `main/scripts/patch-vite-builder.mjs` | Re-patches Nuxt 3.21.x vite-builder `resolveServerEntry` after every install (SPA dev-server fix) |
| `src-tauri/tauri.conf.json` | `version`, `identifier` `org.droposs.client`, `beforeDevCommand` (port 1432), `beforeBuildCommand` `pnpm build`, `frontendDist ../.output`, CSP, updater endpoint + `pubkey`, `bundle.targets` `["nsis","deb","rpm","dmg","appimage"]`, `createUpdaterArtifacts: true`, NSIS `installMode: both`, WebView2 `embedBootstrapper` |
| `src-tauri/rust-toolchain.toml` | `channel = "nightly-2026-04-20"` |
| `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` | `[package] name = "drop-app"`, `version`; lock entry `name = "drop-app"` |
| `src-tauri/src/lib.rs` | `tauri_plugin_updater::Builder` (line 327); `updates::check_for_updates` (line 438) is the GAME in-place update check, not the app updater; the app's own update
  flow is the updater plugin plus `main/composables/use-updater.ts` |
| `main/composables/use-updater.ts` | `checkForAppUpdate()` via `@tauri-apps/plugin-updater` `check()`, then `relaunch()` |
| `src-tauri/database/src/db.rs` | `DATA_ROOT_PREFIX = "drop-debug"` (line 17): debug builds use `%APPDATA%\drop-debug` |
| `.github/workflows/release.yml` | Job `publish-tauri` (matrix), job `updater-manifest` |
| `.github/scripts/updater-manifest.py` | `patch` (puts repacked AppImage `.sig` into `latest.json`), `verify` (checks `EXPECTED_PLATFORMS`, downloads and checks each signature) |
| `.github/workflows/clippy.yml` | `cargo clippy --all-targets` on `ubuntu-24.04`, release published or manual |
| `build-deck.sh` | Builds on a Deck host: `steamos-readonly disable`, pacman deps, rustup nightly, nvm Node LTS, `pnpm tauri build`, re-locks |
| `scripts/install-drop-steamdeck.sh` | Installs `drop-app.AppImage` from the same folder to `~/drop-app.AppImage` with a desktop entry (host, not distrobox) |
| `scripts/uninstall-drop-steamdeck.sh`, `scripts/uninstall-drop.sh` | Remove Deck installs (distrobox, Flatpak, shortcuts) / every install type |
| `scripts/build-drop-flatpak.sh` | Flatpak bundle without flatpak-builder, inside `org.gnome.Sdk//47`; output `~/drop-app-export/drop.flatpak` |
| `scripts/build-drop-steamdeck.sh` | Older Deck build from a clone |
| `scripts/install-drop-flatpak.sh`, `scripts/install-drop-flatpak-deck.sh` | Identical files: install `drop.flatpak` from the same folder |
| `scripts/debug-build.sh` | Runs `pnpm tauri build` inside `org.gnome.Sdk//47` using the flatpak build caches under `/root/.cache/drop-flatpak-build` |
| `org.droposs.client.yml` | Flatpak manifest: GNOME 47, rust-stable + node22 extensions, umu-launcher module, `drop-app` module |
| `scripts/optimize-appimage.sh` | Strip/repack notes; its own header says it is largely useless |
| `scripts/check-versions.ps1` | Not about app versions: GETs a game's version endpoints on a server (`-Base -Token -GameId`) |
| `scripts/download-emulators.ps1`, `scripts/download-retroarch.ps1` | Fetch emulators / RetroArch + cores for server import |
| `scripts/rename-game-exes.ps1` | Removes spaces from `.exe` names (preview unless `-Apply`) |

## How it flows
1. Local build: `pnpm build` (root) -> `scripts/build.mjs` -> `main` `nuxt generate` -> `.output/main`. `pnpm tauri build`
   runs that as `beforeBuildCommand`, then cargo in `src-tauri` with the pinned toolchain, then the bundlers.
2. Release: GitHub release published (or `workflow_dispatch`) -> `publish-tauri` matrix `ubuntu-22.04` + `windows-latest`
   (`fail-fast: false`). Windows pre-seeds NSIS 3.11, `nsis_tauri_utils.dll` (SHA1-checked) and the WebView2
   bootstrapper into `%LOCALAPPDATA%\tauri` with curl retries, then `tauri-apps/tauri-action@v0` builds, signs
   (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) and uploads to the release tag.
3. Ubuntu only: extract the AppImage, drop clashing libs (glib, EGL/GL, drm, wayland, ffi...), bundle `umu-run` and
   `zerotier-one` (both tolerant of failure), repack with `appimagetool`, `pnpm tauri signer sign`, re-upload `.AppImage`
   and `.sig`.
4. `updater-manifest` (after both legs): `updater-manifest.py patch` then upload `latest.json`; `verify` downloads the
   served `latest.json` and every artifact it lists and fails if a platform is missing or a signature is wrong.
5. Clients: updater endpoint `https://github.com/Canpop-mart/drop-app/releases/latest/download/latest.json`.

## Release checklist (only with the owner's explicit approval)
1. Bump the version in 4 files, kept in sync: `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` `[package].version`,
   the `drop-app` entry in `src-tauri/Cargo.lock`, `main/package.json`. (All four read 6.1.6 in this snapshot.)
2. Commit on `main`. Push. Tag `vX.Y.Z` and push the tag.
3. `gh release create vX.Y.Z --notes-file <notes>`. Publishing fires `release.yml` (and `clippy.yml`).
4. Verify both matrix legs succeeded AND that `latest.json` has six platform entries including `windows-x86_64`
   (`EXPECTED_PLATFORMS`: `windows-x86_64`, `windows-x86_64-nsis`, `linux-x86_64`, `linux-x86_64-appimage`,
   `linux-x86_64-deb`, `linux-x86_64-rpm`). v5.7.0 once shipped Linux-only while looking green.
5. `gh run rerun` replays the OLD workflow file. After editing the workflow use `gh workflow run release.yml --ref main`.
6. CI matrix is `ubuntu-22.04` + `windows-latest`; there is no macOS build even though `dmg` is in `bundle.targets`.

## Rules and gotchas
- The AppImage repack changes its bytes, so the `.sig` from tauri-action is wrong; the re-sign step and
  `updater-manifest.py patch` fix it. Without them every Linux AppImage update is rejected (6.1.4's was, per the
  workflow comment).
- The NSIS seed must be complete: if any required file is missing the bundler wipes the dir and downloads again
  unretried. The SHA1s are tied to tauri-bundler 2.9.1 / `@tauri-apps/cli` 2.11.1: bump them together with the CLI.
- A manual run without a release event tags `v__VERSION__` / `v<version from tauri.conf.json>`.
- `scripts/build.mjs` `spawn()` resolves on `exit` without checking the exit code, so a failed `pnpm run build` does
  not stop the script (it then fails or copies whatever `.output/public` exists).
- `scripts/build-drop-flatpak.sh`, `scripts/build-drop-steamdeck.sh` and `org.droposs.client.yml` all build upstream
  `Drop-OSS/drop-app` tag `v0.4.0-rc-4`, not this fork, and use unpinned nightly / rust-stable.
- `build-deck.sh` runs `rustup default nightly`, but cargo inside `src-tauri` follows `rust-toolchain.toml` (inferred).
- The zerotier RPATH placeholder in `release.yml` must match `RPATH_PLACEHOLDER_PREFIX` / `RPATH_PLACEHOLDER_LEN` in
  `src-tauri/src/zerotier.rs`.
- `libs/drop-base` is a submodule (not mapped); `build.mjs` throws if it is missing. Never edit it.

## Changing things here
- New bundle step in CI: add it to `release.yml` and decide if a failure must stop the release (NSIS: strict) or only
  warn (umu, zerotier). Anything that changes a signed artifact after tauri-action must re-sign it.
- New updater platform: update `EXPECTED_PLATFORMS` in `.github/scripts/updater-manifest.py`.
- CLI bump: `package.json` `@tauri-apps/cli` and the NSIS URLs/SHA1s in `release.yml` together.
- Toolchain: `src-tauri/rust-toolchain.toml` plus `toolchain:` in both `release.yml` and `clippy.yml`. Do not bump casually.

## Tests
No tests for the build itself. Before a release: from `src-tauri/`, `cargo +nightly-2026-04-20 check --all-targets`,
`cargo +nightly-2026-04-20 clippy --all-targets`, `cargo +nightly-2026-04-20 test --workspace`; from `main/`,
`pnpm exec nuxt typecheck`; from the repo root, `pnpm build`.

## Related maps
`../drop-server/docs/map/deploy.md`.
