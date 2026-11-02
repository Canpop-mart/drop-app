# Downloads and installs (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
The player picks a version and an install folder, and the game goes into the download queue. The queue downloads
encrypted chunks from a depot, writes them into `<install dir>/<game library path>/<version>`, records each finished
chunk in `.dropdata` (so a pause, crash or restart resumes), then validates every file against the manifest and marks
the game Installed. The same code handles resume, automatic repair of bad chunks, picking up existing Drop installs
when a folder is added, and uninstall (one version at a time). In-place updates and mods reuse the queue but have their
own agents (see Related maps).

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/downloads.rs` | Commands `download_game`, `download_mod`, `resume_download`; `enqueue_game_impl`; `restore_pending_queue` (startup) |
| `src-tauri/src/download_manager.rs` | Commands `pause_downloads`, `resume_downloads`, `move_download_in_queue`, `cancel_game` |
| `src-tauri/src/games.rs` | Commands `uninstall_game`, `fetch_game_installs`, `fetch_game_status(es)`, `fetch_game_version_options`, `scan_game_executables`, `open_game_install_dir` |
| `src-tauri/src/settings.rs` | Install folders: `add_download_dir` (then scans it), `delete_download_dir`, `fetch_download_dir_stats` |
| `src-tauri/download_manager/src/download_manager_builder.rs` | Queue thread: `manage_queue`, `manage_go_signal` (download, validate, up to `MAX_DOWNLOAD_PASSES` = 3), `manage_cancel_signal`; emits `update_queue`, `update_stats`, `download_complete` |
| `src-tauri/download_manager/src/download_manager_frontend.rs` | `DownloadManager` handle (`queue_download`, `rearrange`, `cancel`, `pause_downloads`) and `DownloadManagerSignal` |
| `src-tauri/download_manager/src/downloadable.rs` | `Downloadable` trait every agent implements (`download`, `validate`, `on_complete`, `on_error`, `on_cancelled`) |
| `src-tauri/download_manager/src/depot_manager.rs` | `DepotManager::sync_depots` (GET `/api/v1/client/depots`, then each depot's `manifest.json`), `next_depot` |
| `src-tauri/games/src/downloads/download_agent.rs` | `GameDownloadAgent::new` (picks folder, reads ledger, checks disk space), `run`, `validate`, `invalidate_failed_chunks`, `remove_files_dropped_since_previous_version`, protected-folder lists |
| `src-tauri/games/src/downloads/download_logic.rs` | `download_game_chunk` (fetch `content/{game}/{version}/{chunk}`, AES-CTR decrypt, SHA-256, write), `trim_stale_tails` |
| `src-tauri/games/src/downloads/drop_data.rs` | `.dropdata` ledger: `DropData::generate`, `read`, `write`; v0 legacy reader |
| `src-tauri/games/src/downloads/validate.rs` | `validate_install` (presence, size, per-chunk hash), `ValidationResult` |
| `src-tauri/games/src/library.rs` | `on_game_complete` (fetch version, set Installed), `uninstall_game_logic`, `installs_nested_in`, `remove_install_dir`, `set_partially_installed`, `backfill_game_versions` |
| `src-tauri/games/src/scan.rs` | `scan_install_dirs` (startup), `scan_install_dir` (folder added), `candidate_dirs`, `skip_reason` |
| `src-tauri/games/src/status.rs` | Status state machine: `transition`, `is_valid_transition`, `reconcile_on_startup` |
| `src-tauri/games/src/state.rs` | `GameStatusManager::fetch_state` (transient status masks persistent) |
| `src-tauri/games/src/exe_scan.rs` | `scan_executables`, `rank_candidates`, `resolve_override` (per-device executable pick) |
| `src-tauri/database/src/models.rs` | `InstallRecord`, `PendingQueueEntry`, `DatabaseApplications` (`install_dirs`, `installs`, `pending_queue`), `ApplicationTransientStatus` |
| `src-tauri/utils/src/path_guard.rs` | `join_within` (lexical only), `ensure_within` |
| `main/composables/downloads.ts` | Queue state from `update_queue` / `update_stats` / `download_complete` / `download_error` events |
| `main/composables/game-detail/use-game-install.ts` | Desktop install modal state; `invoke("download_game")` |
| `main/composables/game-detail/quick-install.ts` | One-press install used by BPM pages |
| `main/composables/game-detail/use-game-launch.ts` | `uninstall(version?)`, `resume_download` |
| `main/composables/install-dir-scan.ts` | `parseInstallDirScan`, `describeInstallDirScan` (text shown after adding a folder) |
| `main/pages/queue.vue`, `main/pages/bigpicture/downloads.vue` | Queue page, desktop and BPM |
| `main/components/game-detail/InstallModal.vue` | Desktop version + folder picker |

## How it flows
1. Install: `InstallModal.vue` -> `useGameInstall` -> `invoke("download_game", {gameId, versionId, installDir: index, targetPlatform, enableUpdates})`
   -> `enqueue_game_impl` (skips if already queued or already fully installed at that version) -> `GameDownloadAgent::new`
   (folder = existing `InstallRecord.install_dir` for that exact version, else `<base>/<library_path>/<sanitized version>`;
   `DropData::generate`; GET `/api/v1/client/game/manifest?id&version&previous`; disk-space check) -> `queue_download`
   -> entry appended to `pending_queue`.
2. Queue thread `manage_go_signal`: `agent.download()` -> `run()`: `sync_depots`, `remove_files_dropped_since_previous_version`,
   `back_up_kept_files`, then chunks in parallel (`max_download_threads`, 3 retries with backoff, no retry on disk full).
   Each finished chunk: `hand_files_to_base_game`, `set_context(chunk, true)`, `dropdata.write()`. Then `validate()`:
   a few bad chunks -> invalidate them, PartiallyInstalled, `Ok(false)` -> another pass; too many -> `ValidationFailed`.
   Pass -> `on_complete` -> `on_game_complete` (GET `/api/v1/client/game/{id}/version/{version}`, status Installed or
   SetupRequired, `upsert_install`, emit `update_game/{id}` + `update_library`) -> `record_fresh_baseline` -> `download_complete`.
3. Uninstall: `invoke("uninstall_game", {gameId, version?})` -> refuses mods (`IsMod`) and games with an active update
   (`GameBusy`) -> `uninstall_game_logic`: status Uninstalling, background thread lists mods under the folder and nested
   installs, `remove_install_dir(dir, nested)`, clears orphaned mods, removes the `InstallRecord`, repoints the game status
   to another remaining install or Remote, emits `update_library`.
4. Add folder: `add_download_dir` -> `check_new_install_dir` -> push to `install_dirs` -> `scan_install_dir` -> returns
   `ScanReport {installed, partial, known}` -> background `backfill_game_versions`.

## Both surfaces
Desktop: `pages/queue.vue`, `InstallModal.vue` + `use-game-install.ts`, per-version uninstall from `components/game-detail/Page.vue`,
folders in `pages/settings/downloads.vue`. BPM: `pages/bigpicture/downloads.vue`; on `pages/bigpicture/library/[id].vue`
`installSelectedVersion` (cycles version and folder), `quickInstallGame`, Resume; folders in `pages/bigpicture/settings.vue`.
First run: `pages/welcome/storage.vue`. Gap: BPM `doUninstall` calls `uninstall_game` with no version (`[id].vue` near
line 2449), so BPM removes only the current install; desktop can pick which installed version to remove.

## Server side
Manifests, depots and chunk serving: `../drop-server/docs/map/library-import.md`.

## Rules and gotchas
- New fields on `InstallRecord`, `PendingQueueEntry` or `DatabaseApplications` need `#[serde(default)]` (see `pending_queue`
  and `installs` in `database/src/models.rs`) or every existing install fails to load.
- Transient statuses (Queued, Downloading, Validating, Updating, Uninstalling, Running) are never persisted; only
  `GameDownloadStatus` and `pending_queue` survive a restart (`status.rs` module docs). `reconcile_on_startup` demotes an
  Installed game with a missing folder to Remote and one with no `.dropdata` to PartiallyInstalled.
- An unreadable `.dropdata` refuses the download with `UnreadableDropData` instead of starting over (`DropData::generate`).
  Never "fix" that by returning an empty ledger: every chunk would re-download.
- `DropData::decode` always overwrites `base_path` with the folder it was read from, so a moved library works. Do not trust
  the stored path.
- The stale-file sweep (`remove_files_dropped_since_previous_version`) only runs when `.dropdata` names an EARLIER version of the
  same game (`previously_installed_version`). It deletes only files that version's manifest listed and the new one does not.
  It skips `PROTECTED_DATA_DIRS` at top level (`user`, `portable`, `bis`, `keys`, `system`, `mlc01`, `saves`, `states`,
  `nand`, `sdmc`, `.mods`, `.drop-removed`, ...), `PROTECTED_DATA_DIRS_ANY_DEPTH` anywhere (`drop-goldberg`, `drop-saves`,
  `steam_settings`, `GSE Saves`, `Goldberg SteamEmu Saves`), mod-owned files, non-regular files, and anything whose parent
  canonicalizes outside the install. If the old file list cannot be fetched or mod ledgers cannot be read, nothing is removed.
- `path_guard::join_within` is lexical only (its doc says so): a symlink inside the folder can still lead out. Code that
  deletes must also canonicalize, as the sweep and `trim_stale_tails` do.
- Validation never returns `false` for a broad failure: `validate()` returns `Err(ValidationFailed)` when more than 64 chunks
  and more than a quarter are bad (or no bad chunk could be identified), so a broken upstream cannot loop forever (`REPAIRABLE_CHUNK_FLOOR`).
- Uninstall keeps other installs nested inside the folder being removed (`installs_nested_in`, case-insensitive on
  Windows): an old `<base>/<game>` install can contain a newer `<base>/<game>/<version>`. If removal fails half-way, the
  `.dropdata` is deleted so the scan will not re-import the remains.
- `uninstall_game` on a mod would delete the base game (a mod's install dir IS the parent's folder); it is refused with
  `LibraryError::IsMod`. `resume_download` likewise refuses mods.
- The scan imports a folder as Installed only if `.dropdata` has a non-empty chunk map that is all `true`; otherwise
  PartiallyInstalled. It skips dot-folders, games already Installed at another version, and busy games (`skip_reason`).
  Imported installs cannot launch until `backfill_game_versions` has fetched their `GameVersion` (fails `InvalidVersion`).
- `download_game`'s `installDir` is an INDEX into `install_dirs`, not a path. Deleting a folder (`delete_download_dir`) shifts
  indexes; `pending_queue` entries store the index (`src/downloads.rs` ~109) and `restore_pending_queue` reuses it after a restart
  (~258-265), so a restored download can go to a different folder or fail with `InvalidCommand`. Known bug.
- `on_error` passes the installed version to `push_game_update`; without it an already-installed game's UI stays on
  "Downloading" after a failed re-download (comment in `GameDownloadAgent::on_error`).

## Changing things here
- New queue-related command: write it in `src-tauri/src/downloads.rs` or `download_manager.rs`, register it in
  `src-tauri/src/lib.rs` `generate_handler!`, call it from `main/composables/downloads.ts` or the page, on both surfaces.
- Change what the sweep may delete: edit the lists and `should_sweep` in `download_agent.rs`, and add a case to its tests
  (`protected_names_at_top_level`, `drop_runtime_dirs_protected_at_any_depth`). The in-place planner reads the same
  lists through `is_drop_runtime_data` / `is_player_data_folder`, so check `docs/map/in-place-updates.md` too.
- New field on `.dropdata`: read the format comments in `drop_data.rs` (~33-75) first. Never remove or rename a field;
  a new field breaks older builds reading the file, so consider a new versioned reader (as v0/v1). Extend the fixture
  tests in `drop_data.rs`. `#[serde(default)]` alone is not enough.
- Status changes: go through `status::transition` / `transition_from_db`, and keep `InstallRecord` (`upsert_install`,
  `remove_install`) in step with `game_statuses` + `installed_game_version`.

## Tests
- Rust, from `src-tauri/`: `cargo +nightly-2026-04-20 test -p games downloads::download_agent`, `... downloads::drop_data`,
  `... downloads::download_logic`, `... scan::`, `... library::`, `... exe_scan::`. `validate.rs` and `status.rs` have no tests.
- Frontend, from `main/`: `node --test tests/install-dir-scan.test.ts`, `node --test tests/quick-install.test.ts`.

## Related maps
`docs/map/in-place-updates.md`, `docs/map/mods.md`, `../drop-server/docs/map/library-import.md`.
