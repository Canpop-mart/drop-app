# Multiplayer: co-op rooms, ZeroTier, Archipelago (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
Co-op rooms put players' devices on a private ZeroTier virtual LAN minted by drop-server, so a game's own LAN
multiplayer finds peers across the internet. Archipelago sessions reuse the same ZeroTier machinery on ONE shared
overlay network and add YAML collection, a YAML bundle download and a host-entered connect address. The client owns
the local ZeroTier daemon (start, join, leave, stop) and seeds Goldberg's `custom_broadcasts.txt` with peer IPs.
Remote play (Sunshine/Moonlight streaming) is a separate area and is NOT covered here (see "Related maps").

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/zerotier.rs` | Everything ZeroTier + room/AP orchestration. Commands: `zerotier_status`, `zerotier_prepare`, `zerotier_join`, `zerotier_leave`, `zerotier_stop`, `zerotier_network_ip`, `room_host`, `room_join`, `room_resume`, `room_leave`, `room_mine`, `room_members`, `room_browse`, `ap_session_create/join/get/list/leave/forget`, `ap_web_host`, `ap_yaml_upload`, `ap_connect_set`, `ap_bundle_save`. Internals: `join_and_wait`, `ensure_daemon`, `stop_daemon_if_idle`, `networks_to_sweep`, `startup_cleanup`, `stage_binary`, `ensure_caps`, `in_game_mode` |
| `src-tauri/src/lib.rs` | Commands registered in `generate_handler!` (the `zerotier_*`, `room_*`, `ap_*` block); spawns `crate::zerotier::startup_cleanup()` after setup |
| `src-tauri/remote/src/coop.rs` | Active room id (`set_active_room`, `current_room_id`, `clear_active_room`), `current_peer_ips`, `seed_and_record`, `observe_peers`, `reseed_all`; pure `SeedBook` |
| `src-tauri/remote/src/goldberg/config.rs` | `write_custom_broadcasts` (writes or clears `steam_settings/custom_broadcasts.txt`) |
| `src-tauri/process/src/process_manager/launch.rs` | Launch hook: for Goldberg-like emulators, `current_peer_ips()` with a 5 s timeout, then `seed_and_record` |
| `main/composables/coop-room.ts` | `useCoopRoom()`: `host`, `join`, `browse`, `leave`, `pollMembers` (every `POLL_INTERVAL_MS` = 4000), `checkMine`, `rejoinPending`, `leavePending`, `handleRoomGone` |
| `main/composables/coop-room-logic.ts` | Pure: `normaliseRoomCode`, `formatRoomCode`, `isRoomGoneError`, `rejoinOffer`, `unavailableReason`, `elevationHint` |
| `main/plugins/coop-room.client.ts` | App-wide: starts/stops member polling when a room is active; after sign-in calls `checkMine()` and pushes the "still in a room" toast |
| `main/components/CoopRoomToastHost.vue` | Shows co-op toasts on both surfaces |
| `main/composables/archipelago.ts` | `useArchipelago()`: `create`, `join`, `refresh` (polled every 4000 ms), `uploadYaml` (dialog `open`), `saveBundle` (dialog `save`), `setConnect`, `leave`, `forget`, `reconnect`, `restore`, `loadConfig`, `openWebHost`, `openGameOptions` |
| `main/composables/archipelago-logic.ts` | Pure: `AP_LEAVE_NOT_RECORDED`, `parseLeaveError`, `isSessionGoneError`, `restoreRejoinPlan`, `safeGetItem/safeSetItem`, `AP_DESKTOP_SETUP_MSG` |
| `main/pages/multiplayer.vue` | Desktop page: Co-op / Archipelago tabs; uses `GamePickerModal.vue` for the host's game |
| `main/components/ArchipelagoPanel.vue` | Desktop Archipelago tab (create, join, YAML upload, bundle save, connect address) |
| `main/pages/bigpicture/multiplayer.vue` | BPM page: co-op host/join/browse/rejoin, on-screen keyboard (`BigPictureKeyboard`) for code and game search |
| `main/components/bigpicture/BigPictureArchipelago.vue` | BPM Archipelago: join, view, copy, set connect address, leave/forget |
| `main/components/Header.vue` | Desktop people-icon quick action pushes `/multiplayer` |
| `main/components/bigpicture/BigPictureNavRail.vue` | BPM nav entry `/bigpicture/multiplayer` |
| `.github/workflows/release.yml` | Linux leg bundles `zerotier-one` into the AppImage and sets a placeholder RPATH with `patchelf` (search `ZT_RPATH_PLACEHOLDER`) |

## How it flows
1. Host a room: `useCoopRoom().host()` -> `invoke("room_host")` -> `room_host_inner`: `DaemonInUse` guard,
   `zerotier_prepare` (Linux: `pre_daemon_setup` stages binary + `ensure_caps`; then `ensure_daemon`; returns node id)
   -> POST `/api/v1/client/room` `{zerotierNodeId, gameId?}` -> `join_and_wait(networkId, Host)` polls local
   `GET /network/{nid}` every 500 ms until `NetPhase::Ok` (config timeout 45 s, address timeout 60 s, refusal grace 4 s)
   -> on failure `rollback_server_room` (POST `.../leave`, which for the host deletes the room) -> on success
   `activate_room` (caches controller id, `remote::coop::set_active_room`, starts 12 s `spawn_coop_reseed_loop`,
   `sweep_after_coop_join`) -> `spawn_host_address_report` POSTs `.../address`.
2. Join: `room_join(shortCode)` -> POST `/api/v1/client/room/join` -> `join_and_wait` -> rollback only if not host ->
   `activate_room`. The plugin then polls `room_members` every 4 s; each poll calls `remote::coop::observe_peers`.
   `room_not_found` from a poll -> `handleRoomGone` runs the full `room_leave` cleanup and shows "Session ended".
3. Launch while in a room: `launch.rs` -> `current_peer_ips` (3 tries, 700 ms apart) -> `seed_and_record(dll_dir)`.
   The dir is remembered even with zero peers, so a host who launched first gets the file filled in as peers join.
4. Restart recovery: `startup_cleanup` asks `/client/room/mine` and the AP list, keeps those networks, sweeps other
   Drop networks, stops an idle daemon. The plugin's `checkMine()` -> `room_mine` -> UI offers Rejoin (`room_resume`,
   never rolls back) or Leave.
5. Archipelago: `ap_session_create`/`ap_session_join` -> POST `/client/archipelago` or `/join` -> `join_and_wait(...,
   Archipelago)` -> `remember_ap_network` -> `sweep_after_ap_join`. `ap_session_leave` tells the server FIRST; if that
   fails nothing local changes and the error starts with `ap_leave_not_recorded:`. `ap_session_forget` is local only.

## Both surfaces
Desktop: `pages/multiplayer.vue` + `components/ArchipelagoPanel.vue`. BPM: `pages/bigpicture/multiplayer.vue` +
`components/bigpicture/BigPictureArchipelago.vue`. Co-op is on both. Gap (deliberate, documented in
`BigPictureArchipelago.vue` header and template comment): BPM cannot START an Archipelago session, upload a YAML or save
the bundle (they need a file picker); it tells the player to use the desktop layout / Desktop Mode.

## Server side
`../drop-server/docs/map/multiplayer.md` (room + archipelago routes, controller, reapers).

## Rules and gotchas
- One room per device is enforced server-side (`leaveOtherRooms` keyed by client id), not here.
- First-time Linux setup needs `pkexec setcap`; in Game Mode `ensure_caps` refuses with `GAME_MODE_SETUP_MSG` and the
  UI shows `needsDesktopSetup`. Detection is logged as `[ZEROTIER] Game Mode check: ...` (`log_game_mode_detection`).
- Replacing the staged binary drops its capabilities. `stage_binary` only re-copies when `staging_marker` changes; do
  not make it copy unconditionally or every launch asks for a password again.
- RPATH: the AppImage binary carries a 200-byte placeholder starting `/drop-zt-libs-placeholder/`;
  `rewrite_rpath_placeholder` overwrites it in place with the user's libs dir. `RPATH_PLACEHOLDER_PREFIX`/`_LEN` in
  `zerotier.rs` must match `ZT_RPATH_PLACEHOLDER` in `release.yml` (test `placeholder_is_fixed_length_and_matches_the_workflow_prefix`).
- Leaves are scoped: `room_leave` leaves only the room network, `ap_session_leave` only the overlay.
  `networks_to_sweep` only touches ids prefixed by the cached controller node id, keeps `keep`, in-flight joins and
  (when `keep_ap`) a network named `drop-archipelago`. User-joined ZeroTier networks are never touched; with no cached
  controller id the sweep does nothing.
- `stop_daemon_if_idle` stops only Drop's own Linux daemon, only when `list_networks()` is empty and `DAEMON_USERS` is 0.
  It never stops a system service (`token_override()` set) or the Windows service.
- `ZT_OP` is never held across a server call (see `sweep_after_coop_join`, `startup_cleanup`). Keep it that way or a
  slow server blocks every join/leave.
- Error markers are matched as strings across Rust and TS: `room_not_found` (`isRoomGoneError`), `session_not_found`
  (`isSessionGoneError`), `ap_leave_not_recorded:` (test `leave_marker_matches_the_frontend`).
- `custom_broadcasts.txt` is read by gbe_fork at Steam API init: rewriting it does not reach an already running game
  (module doc in `coop.rs`). `SeedBook::observe` ignores an empty peer list on purpose.
- Archipelago has no auto-connect: the host types or pastes the connect line (`setConnect` -> `ap_connect_set`).
- BPM: `useBpFocusableGroup` (`composables/bp-focusable.ts:89`) ignores a second registration of the same element, so
  an `onSelect` closure captures the first render's values. Look state up at fire time.

## Changing things here
- New ZeroTier/room command: add `#[tauri::command]` in `zerotier.rs`, register it in `src-tauri/src/lib.rs`
  `generate_handler!`, call it from `coop-room.ts` or `archipelago.ts`, then wire BOTH pages.
- Changing what the server returns for a room: update `RoomInfo`/`ApSessionInfo` (new fields `#[serde(default)]`),
  the TS interfaces at the top of `coop-room.ts`/`archipelago.ts`, and the matching server route.
- Changing peer seeding: `remote/src/coop.rs` (`SeedBook` has unit tests) and the hook in `launch.rs`.
- New player-facing text: placeholders live in `coop-room-logic.ts`, `archipelago-logic.ts` and the `*_MSG` consts in
  `zerotier.rs`; list them for the owner.

## Tests
- Rust: `cargo +nightly-2026-04-20 test -p drop-app zerotier::` and `cargo +nightly-2026-04-20 test -p remote coop::`
  (from `src-tauri/`).
- Frontend (from `main/`): `node --test tests/coop-room.test.ts` and `node --test tests/archipelago.test.ts`.

## Related maps
`../drop-server/docs/map/multiplayer.md`, `docs/map/big-picture.md`; streaming/remote play (`src-tauri/src/streaming.rs`,
`src-tauri/remote/src/streaming_sessions.rs`, `main/composables/useStreaming.ts`) needs its own map.
