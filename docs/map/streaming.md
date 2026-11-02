# Remote play: Sunshine host, Moonlight receiver, push sessions (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
A player on one device (in practice the Steam Deck in Big Picture) picks "Play on {PC}" for a game installed on another
of their devices. The request goes to drop-server as a `StreamingSession`; the PC's background poller picks it up,
starts Drop's bundled Sunshine, pre-pairs a random 4-digit PIN, launches the game in "streaming mode", and marks the
session Ready. The receiver then launches Moonlight (auto-installed) against the host's LAN IP and streams the
"Desktop" app. The same request mechanism carries "Install on {device}" (remote install). The client owns Sunshine's
install, config, credentials, process lifetime and Windows Firewall rules, and Moonlight's install and lifetime.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/streaming.rs` | Everything client-side. Section index below |
| `src-tauri/remote/src/streaming_sessions.rs` | HTTP wrappers to `/api/v1/client/streaming/*`: `start_streaming_session`, `mark_session_ready`, `stop_streaming_session(_with_error)` (caps error at 500 chars), `heartbeat_streaming`, `list_streaming_sessions`, `get_streaming_connection_info`, `request_stream`, `poll_pending_requests`, `accept_stream_request`, `list_devices` (`/api/v1/client/devices`), `request_remote_install`, `sync_installed_games` (`/api/v1/client/sync-installed`). Types `StreamingSession`, `StreamingConnectionInfo`, `PendingStreamRequest`, `ClientDevice` |
| `src-tauri/src/lib.rs` | Commands in `generate_handler!` ("Streaming (Sunshine)" and "Streaming sessions" blocks); setup calls `register_installed_sync`, `spawn_stream_request_poller`, and a 5 s delayed `sync_installed_games` |
| `src-tauri/src/process.rs` | `launch_game_streaming(id, index, config_override)` = `launch_game_inner(.., streaming=true, .., incognito=false, version=None)` |
| `src-tauri/process/src/process_manager/launch.rs` | `prepare_launch`: `config_override` replaces the version's `user_configuration` (search `config_override.unwrap_or_else`) |
| `src-tauri/process/src/process_manager/save_sync.rs` | `handle_conflicts_and_collect` `if streaming` branch sets conflicts aside; `MSG_STREAMING_DEFERRED` |
| `src-tauri/src/host_devices.rs` | `DisplayEntry`, `AudioSinkEntry` behind `sunshine_list_displays` / `sunshine_list_audio_sinks` |
| `src-tauri/database/src/models.rs` | Settings: `sunshine_username/password`, `streaming_quality`, `streaming_resolution`, `streaming_hdr`, `streaming_auto_resolution`, `streaming_display`, `streaming_adapter`, `streaming_audio_sink`, `streaming_virtual_sink`, `streaming_enabled` (default true), `streaming_firewall_configured` |
| `main/composables/useStreaming.ts` | `useStreaming()`: thin `invoke` wrappers; TS types `SunshineStatusResult`, `StreamingSession`, `StreamingConnectionInfo`, `ClientDevice` |
| `main/composables/use-remote-sessions.ts` | `useRemoteSessions()`: ONE module-level poll of `streaming_list_sessions` shared by all subscribers (idle 15 s, fastest requested interval wins via `setPollInterval`) |
| `main/composables/bigpicture/stream-targets.ts` | Pure rules: `isDeviceOnline` (`DEVICE_ONLINE_WINDOW_MS` 10 min), `isDeviceForgotten` (30 days), `isOwnDevice`, `partitionDevices`, `buildPlayMenuItems`, `buildInstallMenuItems`, `nextEnabledIndex` |
| `main/composables/bigpicture/use-bpm-game-streaming.ts` | `useBpmGameStreaming()`: `loadDevices`, `streamGame`, `streamFromDevice`, `installOnDevice`, `connectToRemoteStream`, `stopStreaming`, `startPolling`, `dispose`; `StreamingPhase` labels |
| `main/pages/bigpicture/library/[id].vue` | BPM game page: Play/Install dropdown (`openPlayMenu`/`closePlayMenu`, `runMenuItem`), quick "Play on {device}" button, streaming overlay label, `killGame` calls `stopStreaming` |
| `main/pages/settings/streaming.vue` | Desktop "Remote Play" settings: listener toggle, Network Access (firewall), quality, host resolution, capture display, audio, Paired Devices (list/unpair), Active sessions (stop), reset sign-in |
| `main/components/StreamingSetup.vue` | Desktop "This PC" card: set up / repair / start / stop Sunshine, manual PIN pair (`sendPin`) |
| `main/pages/bigpicture/settings.vue` | BPM "Remote Play" section: listener toggle, quality, HDR, auto/manual resolution, capture display, virtual sink, audio sink |
| `main/pages/welcome/streaming.vue` | Onboarding step (BP focus system): install/repair, firewall, display/audio, start Sunshine, list clients |
| `main/plugins/streaming-host-errors.client.ts` | Listens for `stream-host-error`, shows "Remote play couldn't start" modal on the host |
| `main/composables/state-navigation.ts` | Listens for `remote-install-request`: `fetch_game_version_options` then `download_game` with the FIRST version, install dir 0 |
| `main/tests/stream-targets.test.ts` | Tests for `stream-targets.ts` |

### `streaming.rs` section index (approximate lines)
| Lines | Contents |
|---|---|
| 1-35 | Constants: `SUNSHINE_VERSION`, `SUNSHINE_ARCHIVE`, ports 47989 / 47990 |
| 37-200 | Paths (`tools_dir`, `sunshine_dir`, `sunshine_config_dir`), `SUNSHINE_REQUIRED_PATHS`, `find_sunshine`, `probe_sunshine_install`, `check_sunshine` |
| 200-510 | Zip extraction (`sanitise_zip_entry`, `strip_wrapper`, `extract_zip`), `wipe_sunshine_install`, `install_sunshine`, `repair_sunshine`, `configure_firewall_once`, `download_and_install_sunshine` (GitHub LizardByte release) |
| 510-600 | apps.json types, `DESKTOP_APP_NAME`, `ensure_desktop_app_entry` |
| 600-720 | `StreamQuality` (fps/kbps), `parse_stream_resolution`, `resolve_stream_resolution` |
| 720-1080 | Windows Firewall: `SUNSHINE_FIREWALL_RULES`, `ensure_sunshine_firewall`, `firewall_allows_sunshine`; non-Windows stubs ~1018; commands `sunshine_configure_firewall`, `sunshine_firewall_status` |
| 1080-1290 | `wait_for_sunshine_ready`, `sunshine_port_open`, conf blocks (`display_conf_block`, `audio_conf_block`, `resolve_virtual_sink`), `sunshine_list_displays/audio_sinks`, `sunshine_is_foreign` |
| 1288-1540 | `generate_sunshine_conf`, `sunshine_credentials`, `ensure_sunshine_credentials` (`sunshine <conf> --creds`), `register_game_app` |
| 1540-1630 | `SUNSHINE_PROCESS`, `SUNSHINE_ADOPTED`, `HostFailure` + user-facing error strings, `ACTIVE_HOST_SESSIONS` |
| 1630-1960 | `sunshine_status`, `spawn_sunshine`, `ensure_sunshine_running` (adopt or refuse), `start_sunshine`, `stop_sunshine` |
| 1960-2215 | Sunshine REST client `sunshine_api_request`, `is_auth_failure`, `sunshine_send_pin`, `sunshine_list_apps`, `sunshine_register_game`, `sunshine_list_clients`, `sunshine_unpair_client`, `sunshine_reset_credentials` |
| 2215-2350 | Server session commands, `stop_sunshine_if_last_holder`, `stop_all_host_sessions` |
| 2350-2860 | Moonlight: `find_moonlight`, `moonlight_command`, `install_moonlight` (flatpak on Linux), `kill_moonlight`, `launch_moonlight`, `watch_moonlight_session` |
| 2860-2945 | `list_devices`, `request_remote_install`, `register_installed_sync`, `sync_installed_games` |
| 2945-3110 | `streaming_request_stream`, `spawn_stream_request_poller` |
| 3110-3460 | `emit_stream_host_error`, `fail_stream`, `release_reservation`, `host_can_serve`, `fulfill_stream_request` |
| 3461-3863 | `mod tests`, `mod apps_json_tests` |

## How it flows
1. Request (receiver, BPM): menu row -> `streamFromDevice` (refuses if `!target.online`) -> `streamGame(device.id)`
   -> `invoke("streaming_request_stream", {gameId, targetClientId, gameConfig})` (gameConfig = JSON of the page
   version's `userConfiguration`) -> POST `/streaming/request` creates a `Requested` session with
   `hostClientId = target` -> shared poll drops to 3 s; 60 s timeout calls `failPendingRequest`.
2. Host pick-up: `spawn_stream_request_poller` every 10 s (backs off x2 to 300 s on error; skips if signed out or
   `!streaming_enabled`) -> GET `/streaming/pending-requests` -> per new session: if game Installed locally ->
   `fulfill_stream_request`; else accept with no PIN and emit `remote-install-request`.
3. `fulfill_stream_request`: `host_can_serve` (port open, or healthy install + firewall) -> `sunshine_credentials`
   -> random PIN `{:04}` -> local IP via UDP connect to 8.8.8.8 -> `accept_stream_request` (server: `Starting`) ->
   insert into `ACTIVE_HOST_SESSIONS` -> `ensure_sunshine_running` -> `wait_for_sunshine_ready` -> POST Sunshine
   `/api/pin` `{pin, name:"Drop Client"}` -> `mark_session_ready` (server: `Ready`, stores PIN) ->
   `launch_game_streaming(game_id, 0, cfg)` -> spawn heartbeat loop. Every failure exit calls `fail_stream`.
4. Heartbeat loop (5 s): cancel signal -> stop session + kill game; game exited -> stop session; heartbeat
   `Streaming` fails twice -> kill game. Then remove from map and `stop_sunshine_if_last_holder`.
5. Connect (receiver): poll sees own session `Ready` -> `connectToRemoteStream` -> `streaming_get_connection_info`
   (POST `/streaming/connect`, returns `pairingPin`) -> `launch_moonlight(host, port, pin, clientResolution)`:
   `moonlight list` to detect pairing, else `moonlight pair --pin`; then `moonlight stream --fps --bitrate [--resolution]
   [--hdr] --(no-)game-optimization <ip:port> Desktop` -> `watch_moonlight_session` (Rust, 5 s) kills Moonlight when
   the session is Stopped or gone; the page also runs its own 5 s watch.
6. Stop (receiver): `stopStreaming` stops its pending and active session ids, `stop_all_host_sessions`,
   `kill_moonlight`. Host sees heartbeat 404 and kills the game.
7. Manual pairing (desktop host settings): `StreamingSetup.vue` `doPair` -> `sunshine_send_pin` with a PIN the user
   reads off an external Moonlight.

## Both surfaces
Desktop: host setup and management only (`settings/streaming.vue`, `StreamingSetup.vue`). The desktop game page has
NO stream or "Play on" button (comment in `components/game-detail/Header.vue` ~line 90), so a desktop can host but
cannot start a stream as receiver.
BPM: the receiver UI (`bigpicture/library/[id].vue` via `use-bpm-game-streaming.ts`) plus stream settings in
`bigpicture/settings.vue`. BPM settings has no install/repair Sunshine, firewall, paired-device or session list;
those exist in desktop settings and in `welcome/streaming.vue` only.
`components/bigpicture/BpmStreamButton.vue` and `BpmPlayDropdown.vue` are not imported anywhere (rg, no matches).
Dev mode: nothing in streaming is gated. `pages/settings.vue` says "Not dev-gated" for Remote Play; BPM `sections`
lists Remote Play unconditionally. `pages/settings/developer.vue` copy still claims dev mode reveals streaming (stale).

## Server side
Paths after the first are relative to drop-server. No internal module; all logic is inline in `../drop-server/server/api/v1/client/streaming/` routes `start`, `ready`,
`accept`, `request`, `remote-install`, `stop`, `heartbeat` (POST), `sessions`, `pending-requests` (GET), `connect`
(POST), plus `server/api/v1/client/devices.get.ts` and `sync-installed.post.ts`. Prisma:
`prisma/models/streaming.prisma` (`StreamingSession`, enum `StreamingStatus` Requested/Starting/Ready/Streaming/Stopped),
`ClientInstalledGame` in `prisma/models/client.prisma`. Sweeps: `pending-requests` stops Requested older than 2 min;
`sessions` stops active rows with no heartbeat for 5 min and keeps errored Stopped rows listed 2 min. Web UI:
`components/StreamAvailableBadge.vue` calls the client-auth `sessions` route via `$dropFetch` (likely 403 in a plain
browser, inferred).

## Rules and gotchas
- Never kill a Sunshine Drop did not spawn: `stop_sunshine` returns early when `SUNSHINE_ADOPTED`. An adopted
  Sunshine uses its own conf, so display/audio settings do nothing (`sunshine_is_foreign`, `host_device_settings_chosen`).
- Sunshine must only be stopped through `stop_sunshine_if_last_holder`; `fulfill_stream_request` reserves its map
  entry (step 4) BEFORE starting Sunshine so another ending session does not stop it mid-handover.
- `stop_all_host_sessions` touches only this PC's `ACTIVE_HOST_SESSIONS`. The server's session list is account-wide;
  stopping everything in it ends other people's streams.
- Every failure path in `fulfill_stream_request` must call `fail_stream` (event + stop-with-error) and, after the
  reservation, `release_reservation`. A silent return leaves the receiver waiting 60 s with the wrong message.
- `HostFailure` strings are shown on the OTHER device and must name the host (`host_name()`); `stream_error_credentials`
  names the "Reset remote play sign-in" button, which must keep existing in `settings/streaming.vue`.
- Moonlight always streams the `Desktop` app; Drop launches the game itself. `ensure_desktop_app_entry` keeps it in
  apps.json. Registering per-game apps would launch the game twice (comment in `launch_moonlight`).
- `--game-optimization` is only passed when the user picked a manual resolution (`change_host_mode`); Sunshine ignores
  `dd_resolution_option` without it.
- Sunshine's password is generated once (`sunshine_credentials`) and written by Sunshine itself via `--creds`, with the
  conf path FIRST. Cert/key are pinned into `sunshine-config/credentials/` so Repair does not unpair devices.
- Streaming launch: conflicts are set aside (both copies kept, not uploaded, asked next local launch), never incognito,
  always launch index 0, receiver's `UserConfiguration` overrides the host's.
- Own-device detection is by id only (`isOwnDevice`: `isSelf` or `clientId` from `fetch_system_data`); names are
  shared across an account and must not be used. Rust `list_devices` deliberately does not filter.
- Installed list is re-synced on every `update_library` event (2 s coalesce); `hasGame` is only as fresh as that.
- The host poller decides "stream" vs "install" ONLY by whether the game is installed locally; the server rows look
  identical.
- New fields on the settings struct need `#[serde(default)]` (pot DB). New response fields from the server should be
  `#[serde(default)]` in `streaming_sessions.rs` (see `error`).

## Changing things here
- New Tauri command: add to `streaming.rs`, register in `src-tauri/src/lib.rs` `generate_handler!`, wrap in
  `useStreaming.ts`.
- New host failure reason: add a `HostFailure` variant + message fn, route it through `fail_stream`; the receiver
  shows `session.error` automatically (`onRemoteSessions`).
- New stream setting: field in `database/src/models.rs` (with serde default), read in `generate_sunshine_conf` or
  `launch_moonlight`, UI in BOTH `pages/settings/streaming.vue` and `pages/bigpicture/settings.vue`.
- Changing device/menu rules: edit `stream-targets.ts` and its test together; the BPM page only renders the items.
- Server contract change: route in drop-server `server/api/v1/client/streaming/`, body/response struct in
  `streaming_sessions.rs`, TS type in `useStreaming.ts`.

## Tests
- `cargo +nightly-2026-04-20 test -p drop-app streaming::` (from `src-tauri/`): zip sanitising, conf blocks, install
  probe, paired-client parsing, auth-failure detection, apps.json Desktop entry. No tests for the session/poller flow
  or `streaming_sessions.rs`.
- `node --test tests/stream-targets.test.ts` (from `main/`).

## Related maps
`docs/map/big-picture-pages.md`, `docs/map/launch-process.md`, `docs/map/cloud-saves-playtime.md`, `docs/map/local-db-settings.md`, `docs/map/multiplayer.md`,
`docs/map/desktop-frontend.md`, `../drop-server/docs/map/auth-acls-clients.md`, `../drop-server/docs/map/database-prisma.md`
