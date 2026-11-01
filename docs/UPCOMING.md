# Upcoming work (both repos)

The one list of planned work for `drop-app` and `drop-server`. Each item names the map to read and the first file to
open, so a session can start from here. Status words: **Next**, **Waiting on owner**, **Later**, **Idea** (needs a
design decision first).

Keep this file current: when an item is done, move it to "Done, waiting for release" with the commit.
Last full update: 2026-11-01.

## Release state (2026-11-01)
- Live: `main` in both repos. drop-app 6.1.6.
- `dev` is 2 commits ahead of `main` in each repo; the newest commit on each `dev` is not pushed yet:
  - drop-app: `a76437b` addon store + scan existing installs; `a20acfa` shared league saves + library fixes (unpushed).
  - drop-server: `57e6601` addon management in the Store; `bae2f5d` NCAA 14 addon work (unpushed).
- The server must be deployed before the client whenever both change. Release steps: `docs/map/build-release.md`
  and `../drop-server/docs/map/deploy.md`.

## 1. NCAA 14 league in Drop (priority)
Goal: players check the shared dynasty save out of the league site and back in from inside Drop, on PC and Deck.
Maps: `docs/map/addons.md`, `../drop-server/docs/map/addons.md`. NCAA 14 Drop game id `8fcb1216-8b7a-4da6-8d53-5ac3d4dca1c6`.
The league repo (`ncaa14-league`) is separate and gets no more work for this; Drop does the hand-off.
- **Waiting on owner, gates:**
  - G1: confirm a dynasty save made under one PC profile folder loads under another PC's profile. Everything
    depends on this.
  - G2: header file question (see the design doc in the Claude Project).
  - G3: rotate the live `DROP_ADDONS` secret (it was pasted into a chat on 2026-10-30).
  - G4: rehearse the migrations on a fresh dev snapshot (`migrate deploy`).
- Phase 1, ship the addon shop: needs G3, G4. Code is on `dev` (`a76437b`, `57e6601`).
- Phase 2, server adapter onto the league's existing API (checkout, version file download, checkin with base
  version), state route, client proxy, web panel. Started on `dev` (`bae2f5d`: `server/internal/addons/ncaa14.ts`,
  `play.ts`, `upstream.ts`). Status of each piece: to confirm by reading the code against this list.
- Phase 3, client file steps + desktop + BPM buttons. Started on `dev` (`a20acfa`: `src-tauri/src/addons.rs`,
  `src-tauri/remote/src/addons.rs`, `AddonActions.vue`, `BpmAddonActions.vue`).
- Phase 4: prove on PC against a throwaway test league (never the real dynasty from dev), release server then
  client, then test on the Deck with the published update.
- Must hold: addon save slots excluded from cloud-save restore (a restore must not overwrite a fresh check-out);
  2 backups per league per PC; one profile folder per PC, several = stop with a message naming them.

## 2. Testing that has never happened (Waiting on owner)
Large parts of the 2026-09/10 work compile and pass tests but were never run on a device. Scripts exist in the
Claude Project docs (`device-test-batch1.md`, `device-test-batch2.md`, `device-test-updates.md`).
- **H: do cloud saves work at all?** Run the batch 2 cloud-saves section on dev. Map: `docs/map/cloud-saves-playtime.md`.
- Co-op rooms PC + Deck, both join orders, Game Mode first-time setup. Map: `docs/map/multiplayer.md`.
- Achievements: Goldberg on PC and under Proton, RA linked from BPM, reset sticks.
- In-place updates on the Deck (Big Picture review rows, SD-card symlinks).
- Mods: cancel/resume, update, uninstall restores originals.
- Run `cargo audit` for Rust dependencies on the PC (npm side was cleaned 2026-10-30).

## 3. Design work (Idea: needs the owner's call before code)
- **F: game page redesign.** The gear menu, Configure modal, "Version pinned" and "Install another version" are
  cluttered. Proposal: one primary button, one grouped overflow menu, a Versions/Build tab. Both surfaces.
  Files: `main/components/game-detail/Page.vue`, `main/pages/bigpicture/library/[id].vue`.
- **I: settings redesign.** One settings model rendered on desktop and BPM; search; per-page restore; autosave.
  Start with an inventory of every setting (`main/pages/settings/**`, `main/pages/bigpicture/settings.vue`,
  `src-tauri/src/settings.rs`).
- **G: optional features.** Move cloud saves and remote play behind per-user/per-server switches. Today an "addon"
  means an external website; built-in features need a different mechanism. Decide first.
- **B: Addons page design.** After G.

## 4. Known bugs and gaps (Later unless they bite)
- Imported games whose version has setup steps are marked Installed, so setup never runs (rare).
- A depot-imported version with no launches or setups is re-fetched on every library refresh.
- BPM: no achievement reset; home-screen quick Play doesn't ask about pending updates.
- Mods: an update that can never finish (version removed) blocks mod installs on that game until reinstall. A corrupt
  ledger blocks mod changes (the error names the file).
- Archipelago: BPM can't upload a YAML or start a session (deliberate). Open decision noted in
  `../drop-server/server/internal/archipelago/index.ts` (header, "DECISION PENDING"): do players reach the AP server
  directly (then remove the overlay code) or only over ZeroTier? Neither is fully built.
- Server: version revision snapshots are never pruned.
- Server: an IP allowlist rejects the server's own internal fetches (they carry no client IP).
- Dev environment: emulator and mod paths in the dev database copy aren't rewritten to `R:`.
- D: emulator card says "15 cores" because that device's RetroArch `cores/` folder has 15 files; the owner expected
  16. Check which core is missing on the device.
- The debug cargo profile (opt-level + incremental) can produce bogus linker errors; fix: `cargo clean -p <crate>`.
- `README.md` in drop-app describes features as of an older release (e.g. some marked "gated behind dev mode");
  check before trusting it.

## 4b. Possible bugs noticed while writing the maps (2026-11-01, read in code, not reproduced)
Confirm each before fixing.
- **Security, check first:** `../drop-server/server/api/v1/admin/token/index.post.ts:14` uses `allowSystemACL(h3, [])`,
  which passes for any System token (`../drop-server/server/internal/acls/index.ts:249`). A System token with one
  narrow scope could mint a new System token with every scope. The "session only" comment there isn't enforced.
  The same check also accepts the `setup` token on `admin/token/*`, `admin/security/ip-rules/*` and bugreports.
- **Security:** client JWTs get ~8.3 hours of clock tolerance instead of 30 seconds:
  `../drop-server/server/internal/clients/event-handler.ts` passes `JWT_TIME_WIGGLE = 30_000` as jsonwebtoken's
  `clockTolerance`, which is in seconds. A JWT meant to last 10 s can be replayed for hours. Bad signatures throw
  (500) instead of returning 403.
- **Security:** disabled users keep working sessions and tokens: `getUserIdACL` / `allowSystemACL` never check
  `user.enabled` (`../drop-server/server/internal/acls/index.ts`).
- **Security:** `../drop-server/server/api/v1/client/auth/callback/index.post.ts` checks `session.authenticated` but not
  the MFA level, so a session still waiting on MFA can approve a client.
- Server boot inserts a new `ApplicationSettings` row on EVERY start (`03.metadata-init.ts`; `set()` compares arrays
  with `!==`, `save()` creates), so the table grows forever.
- Server tasks: a daily task running over an hour is restarted by the next hourly check, `create()` throws "already
  exists", and the uncaught throw skips every later daily and weekly task that hour. Failed tasks aren't retried for
  24 h / 7 days.
- `../drop-server/server/api/v1/admin/import/game/search.get.ts`: "no match" and "every provider failed" return the
  same 404 (drops `failedProviders`), so provider outages look like no results.
- Server tasks: starting a task while its group runs throws a plain `Error` (`../drop-server/server/internal/tasks/index.ts`
  around 208/223), likely a bare 500 in admin. `buildId()` in task registries is ignored (ids are overwritten with
  random UUIDs). `scheduler.ts:10` comment says interval tasks run "on the next tick"; they first run one interval
  after boot (e.g. `check:game-updates` after 6 hours).
- `../drop-server/server/internal/clients/README.md` documents a client session endpoint, but
  `../drop-server/server/api/v1/client/auth/session.post.ts` is an empty handler.
- `../drop-server/server/internal/zerotier/index.ts` around line 448: `getRoom`'s membership check ignores member
  status, so a player who left can still read the room and other players' IPs.
- `../drop-server/server/internal/archipelago/index.ts` `createSession` (around line 561): the host's device is
  authorized before its slot is written; the daily reconcile could de-authorize a host mid-create.
- `main/plugins/coop-room.client.ts:43`: toast tells players to open "Co-op Rooms" from the top bar; no item has that
  label (the people icon in `Header.vue` opens it). Player-facing string, owner to rewrite.
- BPM uninstall can only remove the current install: `doUninstall` in `main/pages/bigpicture/library/[id].vue`
  (around line 2449) sends no version; desktop lets you pick. Dual-surface gap.
- `src-tauri/games/src/downloads/update/mod.rs` `check_can_update` (around line 743): an in-place update is refused
  while ANY mod download is queued, for any game.
- The saved download queue stores the install folder as an index into the folder list (`src-tauri/src/downloads.rs`
  around 109); `delete_download_dir` shifts indexes and `restore_pending_queue` reuses the stale one after a restart,
  so a restored download can go to a different folder or fail. (Traced in code by the reviewer.)
- Startup: an I/O error reading `drop.db` panics (`.expect` in `src-tauri/database/src/interface.rs` around 86), so the
  app "opens and closes instantly" instead of showing an error.
- `update_settings` throws away the whole patch if any value has the wrong type, with only a log line
  (`src-tauri/src/settings.rs` around 256).
- Contradicting comments about `pot`: `src-tauri/games/src/downloads/mod_data.rs` (~84-88) says pot can't round-trip
  `#[serde(default)]` fields; `drop_data.rs` (~33-75) says that was a wrong guess. Fix the stale one.
- Stale comments: `mod_agent.rs` `on_complete` (around line 878) says the stored install dir is the parent game's
  folder, it is the mod's subfolder; `../drop-server/server/internal/tasks/registry/regenerate-manifests.ts` cites
  `download_logic.rs:173` as the checksum error, that line is a decryption call.
- **Build:** `scripts/build-drop-flatpak.sh`, `scripts/build-drop-steamdeck.sh` and `org.droposs.client.yml` clone
  UPSTREAM `Drop-OSS/drop-app` at `v0.4.0-rc-4`, not this fork. `scripts/build.mjs` doesn't check child exit codes,
  so a failed frontend build doesn't stop it.
- **Server CI:** `../drop-server/.github/workflows/ci.yml` runs only on release or by hand, so pushes to `main` (which
  build the live image) are never typechecked.
- Server services: nginx health check probes port 8080 (`../drop-server/server/internal/services/services/nginx.ts:25`)
  but nginx listens on 3000 (`../drop-server/build/nginx.conf`); `setInterval(this.runHealthcheck, ...)` in
  `../drop-server/server/internal/services/index.ts` isn't bound, so it likely throws.
- `../drop-server/composables/request.ts` (`$dropFetch`) caches SSR results by path only, ignoring the query.
- Language cookie mismatch: `../drop-server/i18n/localeDetector.ts` reads `i18n_redirected`, `nuxt.config.ts` sets
  `drop_i18n_redirected`.
- Web error-vs-empty: `../drop-server/pages/community.vue` and `../drop-server/pages/user/[id]/index.vue` turn fetch
  errors into empty data.
- One-surface features: nothing calls `/api/v1/screenshots/**`; `now-playing`, `roulette`, `mvp-today`, `weekly-*`
  show only in the client, not the web community page; admins aren't notified of new game requests.
- `../drop-server/layouts/default.vue` still injects a `bp-controller` bridge script nothing sends to any more.
- Admin "reset achievements" for a game (`../drop-server/server/api/v1/admin/game/[id]/achievements-reset.post.ts`)
  deletes definitions and every user's unlocks but writes no `AchievementReset` marker, so unlocks probably come back
  when clients re-report from local files (inferred).
- Desktop has no button for the local save Backup/Restore that BPM has (`backup_pc_game_saves` /
  `restore_pc_game_saves`, only used by `main/composables/bigpicture/use-bpm-game-saves.ts`). Dual-surface gap.
- `sync_game_saves_now` is registered in `src-tauri/src/lib.rs` but never called from `main/`.
- `../drop-server/server/api/v1/user/[id]/stats.get.ts` (around 61) closes orphan play sessions after 4 hours while the
  daily task uses 1 hour, and this GET writes to the database.
- Stale comments: `main/composables/save-sync-state.ts:16` names a function that doesn't exist (real one:
  `apply_shared_device_rule` in `src-tauri/process/src/process_manager/save_sync.rs`);
  `../drop-server/server/internal/achievements/config-cache.ts` header still describes 15-second config polling;
  `../drop-server/server/internal/tasks/registry/goldberg-readiness.ts` says Drop no longer swaps the Steam DLL, but
  import does (`setupEmulators.ts`, `swapDll: true`).
- Streaming (map: `docs/map/streaming.md`):
  - Copy is wrong: `main/pages/settings/developer.vue` says dev mode reveals Streaming; it isn't gated anywhere
    (comment in `main/pages/bigpicture/settings.vue` around 1541 says the same).
  - Dual-surface gaps: desktop can host but can't start or receive a stream (BPM only); BPM settings lack Sunshine
    install/repair, firewall, paired devices and active sessions (desktop and `welcome/streaming.vue` only).
  - Stream and remote-install requests look the same to the host poller (`spawn_stream_request_poller` in
    `src-tauri/src/streaming.rs`, around 3015), which picks by "is it installed locally"; a stale list could turn an
    install request into a stream. Remote install always uses version option 0 and install dir 0; streamed launches
    always use launch option 0.
  - Linux `kill_moonlight` runs `pkill -f moonlight`, killing any process with that word in its command line.
  - Host IP is found via a UDP socket to 8.8.8.8, so a VPN advertises the wrong address; `hostExternalIp` is never
    set, so only LAN streaming works.
  - `../drop-server/server/api/v1/client/streaming/request.post.ts`: a request with no target is stored with
    `hostClientId` = requester and is never picked up (BPM always passes a target today).
  - `../drop-server/components/StreamAvailableBadge.vue` calls a client-auth route from the browser; probably always
    403 and hidden.
- Settings: `update_settings` silently drops keys not in `ALLOWED_SETTINGS_KEYS` (`src-tauri/src/settings.rs`
  around line 192) and returns nothing, so a page can't tell a save failed.
- (unconfirmed, `ModalStack` is in the drop-base submodule) A game crash in BPM may show the desktop crash modal
  as well as `LaunchErrorDialog` (listener in `main/app.vue` around line 83).
- BPM download toast: `main/components/bigpicture/BpmDownloadToast.vue` (around 70-82) listens for A and B without
  the input lock, so for its 5 seconds A opens the game AND activates the focused control, and B also triggers "back".
- BPM shelf picker in `main/pages/bigpicture/library/[id].vue` (around 2505) counts rows once on open; after "Create
  New Shelf" adds a row, "Done" is unreachable by D-pad (B still closes).
- `main/components/bigpicture/BigPictureKeyboard.vue` (around 254): the `visible` watch isn't `immediate`; if ever
  mounted already visible it takes no lock. No current caller does this.
- `main/assets/bpm-themes.scss` (around 117) has a `.bpm-theme-switch` block no `ThemeId` can select. Unused BPM
  components: `BpmBootScreen.vue`, `BpmStreamButton.vue`, `BpmPlayDropdown.vue`. Owner decides; don't delete themes.
- BPM has no Account / sign-out section and no emulator-host page (`list_installed_emulators`,
  `open_emulator_cores_dir` are desktop only). Dual-surface gap.
- `main/composables/launch-failure.ts` recognises launch errors by matching substrings of `ProcessError` messages;
  rewording a message in `src-tauri/process/src/error.rs` silently breaks its dialog.
- Bug reports attach only the live `drop.log`, not the rotated archives.
- `../drop-server/server/internal/library/README.md` describes an older design. Trust the code and the map.
- Addons: only the NCAA 14 adapter is wired; `../drop-server/server/api/v1/games/[id]/addons.get.ts` and the context
  routes import it directly. A second addon needs them to pick the adapter from the config.

## 5. Housekeeping decisions (Waiting on owner)
- `archive/old-dev` branch (protonfixes/OnlineFix, never shipped): keep, mine for parts, or delete.
- The Claude Project's instructions are out of date: sections 1 and 6 describe the old branch layout, and
  "Releasing drop-server" says push to `develop`; the image now builds from `main`
  (`../drop-server/.github/workflows/build-custom.yml`). They also call the local database `pot`-encoded: `drop.db`
  is RON (encrypted); `pot` is the `.dropdata` and mod-ledger format.

## Done, waiting for release
- (2026-11-01) Docs for local models: `AGENTS.md`, `docs/map/`, this file. Branch `chore/local-model-docs`.
