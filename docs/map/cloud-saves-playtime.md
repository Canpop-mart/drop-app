# Cloud saves and playtime (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
Cloud saves: before a game launches, the client scans local saves, asks the server which side is newer, downloads
cloud copies or asks the player to pick a side on a conflict. After the game exits it uploads what changed. The
player can also browse, back up, restore, delete and roll back cloud saves from a per-game panel. The whole feature
is opt-in (`cloud_saves_enabled`, default `false`, `src-tauri/database/src/models.rs:546`).
Playtime: the client opens a server session at launch, heartbeats every 60 s, and reports the measured duration at
exit. Failed reports are queued on disk and replayed later for the same account.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/remote/src/save_sync/mod.rs` | Types: `SyncManifest`, `SyncFileEntry` (`synced_hash`, `three_way_base`), `SyncCheckResponse`, `SaveConflict`, `current_user_id` |
| `src-tauri/remote/src/save_sync/api.rs` | Server calls: `check_sync`, `bulk_download`, `upload_changed_saves`, `list_cloud_saves`, `list_cloud_save_revisions`, `restore_cloud_save_revision`, `delete_cloud_save` |
| `src-tauri/remote/src/save_sync/decide.rs` | `three_way_verdict`, `reclassify_with_manifest` (server "conflict" -> download/upload when one side changed) |
| `src-tauri/remote/src/save_sync/conflict.rs` | `extract_conflicts`, `apply_conflict_resolutions`, `any_conflict_deferred` |
| `src-tauri/remote/src/save_sync/manifest.rs` | `{DATA_ROOT_DIR}/sync-manifests/{user_id}/{game_id}.json`: `load_manifest`, `save_manifest`, `update_manifest_after_sync`, `other_accounts_have_synced`, `local_copy_last_synced_by` |
| `src-tauri/remote/src/save_sync/scan.rs` | Discovery: `scan_emu_saves`, `scan_pc_saves` (Ludusavi), `pc_scan_context`, `PcScanError`, `split_too_long_names`, `write_downloaded_save`, `write_downloaded_pc_save`, `is_shared_between_accounts` |
| `src-tauri/remote/src/save_sync/backup.rs` | `backup_existing` (`<file>.bak.<unix_secs>`), `write_atomic`, `is_backup_artifact` |
| `src-tauri/remote/src/save_sync/quota.rs` | `fetch_quota`, `preflight_quota`, `plan_within_quota`, `format_bytes` (mirrors server `formatBytes`) |
| `src-tauri/remote/src/save_sync/scope.rs` | Per-user emulator save roots: `resolve_emu_saves_root`, `ensure_user_root`, owner claim file |
| `src-tauri/remote/src/save_sync/tombstone.rs` | `plan_tombstones`, `record_applied` (deletes made on another device) |
| `src-tauri/process/src/process_manager/save_sync.rs` | Pre-launch orchestration: `sync_emulator_saves`, `sync_pc_saves`, `handle_conflicts_and_collect`, `resolve_conflicts`, `apply_shared_device_rule`, `emit_sync_error`, `describe_failure` |
| `src-tauri/process/src/process_manager/exit.rs` | `on_process_finish`, `spawn_post_exit_sync`, `upload_changed_saves_for`, `run_playtime_heartbeat_loop` |
| `src-tauri/process/src/process_manager/launch.rs` | Calls the save sync (~line 1068 / 1100), `start_playtime` (~line 914), incognito handling |
| `src-tauri/process/src/lib.rs` | `CONFLICT_CHANNELS` (game id -> mpsc sender the launch thread blocks on) |
| `src-tauri/src/process.rs` | `resolve_save_conflicts` command (UI answer -> channel) |
| `src-tauri/src/games.rs` | Panel commands: `scan_local_game_saves`, `game_save_coverage`, `backup_saves`, `restore_pc_cloud_save`, `list_cloud_saves`, `backup_pc_game_saves` / `restore_pc_game_saves` (`ludusavi_backup_dir`) |
| `src-tauri/src/save_scope_migration.rs` | One-time move of manifests and emulator saves into per-user dirs; runs at start and after sign-in |
| `src-tauri/remote/src/playtime.rs` | `start_playtime`, `heartbeat_playtime`, `stop_playtime`, `record_session`, `queue_pending_stop`, `drain_pending_stops` |
| `src-tauri/src/lib.rs` | Command registration (`generate_handler!`), playtime drain at startup (~259) and after sign-in (~917) |
| `main/components/SaveSyncConflictHost.vue` | App-level `save_sync_conflict` listener, picks desktop or BPM dialog, queue, countdown, `skip` on dismiss |
| `main/components/SaveConflictDialog.vue`, `main/components/bigpicture/BpmSaveConflictDialog.vue` | The two conflict dialogs |
| `main/components/CloudSavesPanel.vue` | Desktop per-game panel (in `main/components/game-detail/Page.vue`) |
| `main/components/bigpicture/BpmCloudSavesPanel.vue`, `main/components/bigpicture/game-detail/BpmGameSavesTab.vue`, `main/composables/bigpicture/use-bpm-game-saves.ts` | BPM per-game Cloud Saves tab (in `main/pages/bigpicture/library/[id].vue`) |
| `main/composables/save-sync-state.ts` | Pure: `saveSyncState` (panel version of the three-way rule), `displaySaveName`, `conflictLocalNote` |
| `main/composables/use-server-api.ts` | `saves.*` wrappers incl. `history`, `restoreRevision`, `quota` |
| `main/plugins/save-sync-errors.client.ts`, `main/components/SaveSyncToastHost.vue`, `main/composables/save-sync-status.ts` | `save_sync_error` -> modal + failure record; `save_sync_complete` -> toast |
| `main/pages/settings/cloudsaves.vue` | Desktop settings (toggle, status). BPM equivalent: `cloudsaves` section in `main/pages/bigpicture/settings.vue` |

## How it flows
1. **Pre-launch sync** (`launch.rs` -> `sync_emulator_saves` / `sync_pc_saves`): scan (`scan_emu_saves` or
   `scan_pc_saves_all` with `pc_scan_context`) -> `check_sync` (POST `/api/v1/client/saves/sync-check`) ->
   tombstones applied -> `reclassify_with_manifest` -> `apply_shared_device_rule` -> `handle_conflicts_and_collect`
   -> uploads of kept-local files -> `bulk_download` -> manifest update -> `SaveSyncSnapshot` stored for exit.
2. **Conflict prompt**: `resolve_conflicts` emits `save_sync_conflict` `{gameId, conflicts, timeoutSecs}`, inserts a
   sender into `CONFLICT_CHANNELS`, blocks up to `CONFLICT_RESOLVE_TIMEOUT` (45 s). `SaveSyncConflictHost.vue` shows
   the dialog; the dialog calls `invoke("resolve_save_conflicts")`. Timeout or `skip` = sync nothing this launch.
3. **Post-exit** (`on_process_finish` -> `spawn_post_exit_sync`): wait for the playtime start to settle, `stop_playtime`
   (or `record_session` if the start never landed), `notify_session_end` (achievements), then
   `upload_changed_saves_for`: rescan, diff against `pre_hashes`, skip `deferred_conflicts`, `preflight_quota`,
   `upload_changed_saves` (POST `bulk-upload`), emit `save_sync_complete` / `save_sync_error`.
4. **Playtime queue**: stop failure -> `queue_pending_stop` (`<data>/pending-playtime-stops/<session>.json`); start
   never landed -> `record_session` (POST `/api/v1/client/playtime/record`), failure -> `<data>/pending-playtime-records/`.
   `drain_pending_stops` runs at startup, after sign-in, and when the app comes back online (`src-tauri/remote/src/utils.rs`).

## Both surfaces
Desktop: `CloudSavesPanel.vue` (game page), `SaveConflictDialog.vue`, `main/pages/settings/cloudsaves.vue`.
BPM: `BpmCloudSavesPanel.vue` + `BpmGameSavesTab.vue` (Cloud Saves tab), `BpmSaveConflictDialog.vue` (takes the
input lock on `visible` false->true, releases on true->false and in `onUnmounted`), BPM settings `cloudsaves` section.
Both panels have revision history and restore. Gap: the Ludusavi local backup/restore to `<data>/save-backups/<game_id>`
(`backup_pc_game_saves` / `restore_pc_game_saves`) is only called from `use-bpm-game-saves.ts`; desktop has no button
for it. The `save_sync_error` modal (plugin) is a generic `createModal` on both surfaces.

## Server side
See `../drop-server/docs/map/cloud-saves-playtime-stats.md`.

## Rules and gotchas
- A manifest entry is only used as the three-way base when `three_way_base` is true AND its `cloud_id` matches the
  cloud row (`decide.rs` `three_way_verdict`, `SyncFileEntry::trusted_base`). Older entries stay "no record" until a
  successful sync rewrites them. Never write a declined or timed-out conflict into the manifest.
- Manifests are JSON per user, not pot. New fields on `SyncManifest` / `SyncFileEntry` still need `#[serde(default)]`
  or every manifest on disk fails to parse (see `user_id`, `applied_tombstones`, `three_way_base` in `mod.rs`).
- Two accounts on one PC share PC save files and Switch NAND. `apply_shared_device_rule` turns a reclassified upload
  back into a conflict and holds local-only files when `other_accounts_have_synced`. The dialog's local-side note
  comes from `conflictLocalNote` (`localLastSyncedBy`, `localMayBeOtherAccount`).
- Streaming launches never prompt: `handle_conflicts_and_collect` (`if streaming`, save_sync.rs:1831) puts all
  conflicts in `unresolved`, emits `MSG_STREAMING_DEFERRED`, and the exit upload skips those files (`exit.rs:454`).
- A timed-out or dismissed conflict is NOT "keep local": `resolved = false`, so `synced_ok = false` and
  `post_exit_upload_allowed` blocks the whole exit upload (`exit.rs:375`) with a user-visible error.
- Ludusavi: every PC scan must take `pc_scan_context(game_id, ..)` (Steam app id + Wine prefix for Windows games,
  passed as `--wine-prefix`). A failed scan is `Err(PcScanError)`, not an empty list; an empty list made the launch
  pull every cloud save over files the scan could not see. "Unknown game" from Ludusavi is `Ok(vec![])`.
- The exit path rebuilds `pc_scan_context` (`exit.rs` ~427): a first Proton launch creates the prefix mid-session.
- Local overwrites back up first: `backup_existing` writes `<file>.bak.<unix_secs>` (`-n` suffix on same-second
  repeats). If backup fails, do not write. Scans must skip `is_backup_artifact` names or backups get uploaded.
- Names over 255 bytes are not synced (`MAX_CLOUD_FILENAME_BYTES`, `split_too_long_names`); the server refuses them too.
- Pending playtime records only send while their `user_id` is signed in (`drain_pending_records`); stops with
  `user_id: None` (older files) are sent for whoever is signed in. A 401/403 keeps the file.
- Incognito launches open no playtime session and start no achievement poll (`launch.rs` ~900-957).
- `main/composables/save-sync-state.ts` must stay in step with `decide.rs` and `apply_shared_device_rule`. Its header
  names `keep_other_accounts_changes_as_conflicts`, which does not exist; the real function is `apply_shared_device_rule`.

## Changing things here
- New sync rule: change `decide.rs` (pure, tested), then `saveSyncState` in `save-sync-state.ts`, then add cases to
  `decide.rs` tests and `main/tests/save-sync-state.test.ts`.
- New save command: write it in `src-tauri/src/games.rs`, register it in `src-tauri/src/lib.rs` `generate_handler!`,
  call it from both `CloudSavesPanel.vue` and the BPM panel/composable. Gate it on `cloud_saves_enabled` and
  `current_user_id()` like `backup_saves`.
- New server endpoint use: add to `src-tauri/remote/src/save_sync/api.rs`, re-export from `src-tauri/remote/src/save_sync/mod.rs` if `process` needs it, add the
  route in drop-server.
- New user-facing failure: call `emit_sync_error(app, game_id, PHASE_*, msg, retryable)` in `save_sync.rs`; extend
  `describe_failure` for raw server errors.

## Tests
- `cargo +nightly-2026-04-20 test -p remote save_sync::` (decide, manifest, scan, backup, quota, tombstone, scope, conflict)
- `cargo +nightly-2026-04-20 test -p process process_manager::save_sync` and `... -p process process_manager::exit`
- `cargo +nightly-2026-04-20 test -p remote playtime::`
- `cargo +nightly-2026-04-20 test -p drop-app save_scope_migration` (and `ludusavi_backup_tests` for `ludusavi_backup_dir`)
- From `main/`: `node --test tests/save-sync-state.test.ts`

## Related maps
`../drop-server/docs/map/cloud-saves-playtime-stats.md`, `docs/map/launch-process.md`, `docs/map/achievements.md`,
`docs/map/big-picture.md`, `docs/map/local-db-settings.md`.
