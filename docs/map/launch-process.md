# Launching games and the process manager

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
Turns Play into a running process. The client picks the install, the launch config for the target platform,
a handler (native, Windows, umu/Proton, emulator), formats the user's launch template, syncs cloud saves,
spawns the game, tracks it, and on exit stops playtime, reports a fast or non-zero exit as a failed launch,
and flips the status back to Installed. On Linux and the Deck, Windows games run through `umu-run` with one
Proton prefix per game.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/process.rs` | Tauri commands: `launch_game` (runs `launch_game_inner` on a std thread), `kill_game`, `get_launch_options`, `read_latest_launch_log`, `open_process_logs`, `install_redists`, `resolve_save_conflicts`, `list_gamepads`; `LaunchResult` (`Success` / `InstallRequired`) |
| `src-tauri/process/src/lib.rs` | `PROCESS_MANAGER` global, `ProcessManagerWrapper::init`, `LAUNCH_APP_HANDLE`, `CONFLICT_CHANNELS`, `hold_for_addon` / `AddonHold` |
| `src-tauri/process/src/process_manager/mod.rs` | `ProcessManager` struct (`processes`, `pending_launches`, `addon_holds`, `game_launchers` table), `kill_game`, `is_game_active`, `get_log_dir`, `reconcile_running_processes`, `fetch_process_handler`, `ProcessHandler` trait (`runs_under_proton`) |
| `src-tauri/process/src/process_manager/launch.rs` | `run_launch` -> `prepare_launch` (steps 1-7) -> `pre_launch_save_sync` -> `finish_launch` (step 8) -> `register_running_process`; `build_command`, `configure_retroarch`, `configure_switch_emu`, `detect_script` |
| `src-tauri/process/src/process_manager/launch_emulator.rs` | `build_emulator_command` (ROM into emulator command, `.m3u`, RetroArch core), `spawn_with_enoexec_retry` |
| `src-tauri/process/src/process_manager/exit.rs` | `on_process_finish`, `SUSPICIOUS_EXIT_SECS`, `check_ra_credentials`, `describe_exit`, `spawn_post_exit_sync`, `run_playtime_heartbeat_loop` |
| `src-tauri/process/src/process_manager/kill.rs` | `kill_process_tree` (Linux: SIGTERM then SIGKILL to the `setsid` group; Windows: `taskkill /T /F`) |
| `src-tauri/process/src/process_manager/env.rs` | `FORBIDDEN_ENV_KEYS`, `apply_launch_env`, `sanitize_appimage_env`, `configure_gamescope_env`, `configure_mangohud_env`, `log_launch_env_fingerprint` |
| `src-tauri/process/src/process_handlers.rs` | `WindowsLauncher`, `NativeLauncher`, `UMUNativeLauncher`, `UMUCompatLauncher` (Proton), `AsahiMuvmLauncher`, `MacLauncher`; `DEFAULT_UMU_PROTON`, `install_redists_for_game` |
| `src-tauri/process/src/prefix_prep.rs` | `prepare_prefix` (OnlineFix DLL staging, Linux only), `install_redists_into_prefix` (winetricks), `game_prep_status` events |
| `src-tauri/process/src/compat.rs` | Linux-only commands: `fetch_proton_paths`, `add_proton_layer`, `set_default`, `clear_default_proton`, `install_umu`, `diagnose_launch_environment` |
| `src-tauri/client/src/compat.rs` | `COMPAT_INFO`, `UMU_LAUNCHER_EXECUTABLE` (umu-run discovery), `get_umu_executable_fresh` |
| `src-tauri/process/src/parser.rs`, `format.rs` | `ParsedCommand` (shell_words tokenise / `reconstruct`), `DropFormatArgs` (template placeholders) |
| `src-tauri/process/src/error.rs` | `ProcessError` and its player-facing `Display` text (serialised via `SerializeDisplay`) |
| `src-tauri/src/compat.rs` | Compat test runner `start_compat_test`, `register_launch_telemetry` (turns `game_launch_outcome` into a POST) |
| `main/composables/game-detail/use-game-launch.ts` | Desktop Play: `useGameLaunch` -> `launch` -> `launchIndex` -> `invoke("launch_game")` |
| `main/components/game-detail/Page.vue`, `main/components/game-detail/LaunchOptionsModal.vue` | Desktop game page and the "pick a launch config" modal |
| `main/composables/launch-failure.ts` | `describeLaunchFailure`, `describeExternalLaunchFailure`, `isBenignLaunchError` (shared by both surfaces) |
| `main/composables/state-navigation.ts` | `setupHooks`: desktop listener for `launch_external_error_detail` (Confirmation modal, "Open" calls `open_process_logs`) |
| `main/pages/bigpicture/library/[id].vue` | BPM game page: `invoke("launch_game")`, listens to `launch_trace` and `launch_external_error` |
| `main/pages/bigpicture/index.vue` | BPM home quick-launch (`describeLaunchFailure`, `kill_game`) |
| `main/components/bigpicture/LaunchErrorDialog.vue` | BPM dialog for `launch_external_error_detail`, mounted in `main/layouts/bigpicture.vue` |
| `main/components/GameOptions/Launch.vue`, `main/components/GameOptions/ProtonSelector.vue`, `main/composables/bigpicture/use-bpm-game-config.ts` | Per-game launch template, executable override, Proton choice (desktop / BPM) |

## How it flows
1. Play (desktop): `useGameLaunch.launch` calls `get_launch_options`; more than one option opens
   `LaunchOptionsModal`; then `launchIndex` -> `invoke("launch_game", { id, index, incognito, version })`.
   BPM calls the same command from `pages/bigpicture/library/[id].vue` and `pages/bigpicture/index.vue`.
2. `launch_game` (`src/process.rs`) spawns a std thread -> `launch_game_inner` ->
   `games::downloads::update::recover_before_launch` -> `process::process_manager::run_launch`.
3. `run_launch` takes `PROCESS_MANAGER.lock()` for `prepare_launch`: resolve install (step 1), launch config +
   mod override (2b) + user `executable_override` (2c), `fetch_process_handler` (3), build command, emulator path via
   `build_emulator_command` (4), template format (5), parse argv and refuse a missing absolute target with
   `LaunchTargetMissing` (6), `build_command` incl. RetroArch / Switch emu config (7). Reserves the id in `pending_launches`.
4. Lock released; `pre_launch_save_sync` runs (may block on the conflict dialog, see cloud-saves map).
5. Lock retaken; `finish_launch` spawns (`setsid` on Unix), retries ENOEXEC via bash, then `register_running_process`
   starts playtime + achievement polling, sets transient `Running`, inserts into `processes`, and spawns a thread
   that blocks on `wait()` then calls `on_process_finish`.
6. Exit (`exit.rs`): emits `game_process_exited`; if not user-killed and exit was within 2 s or non-zero, emits
   `launch_external_error` and `launch_external_error_detail`; always emits `game_launch_outcome`; clears `Running`.
   Desktop shows a Confirmation modal (`state-navigation.ts`), BPM shows `LaunchErrorDialog.vue`.
7. Proton (Linux, Windows target): `UMUCompatLauncher::create_launch_process` resolves Proton (per-game
   `override_proton_path` -> `applications.default_proton_path` -> `"GE-Proton"`), prefix
   `DATA_ROOT_DIR/pfx/<game_id>`, runs `prepare_prefix`, returns `GAMEID=.. PROTONPATH=.. WINEPREFIX=.. umu-run <cmd>`.

## Both surfaces
Desktop: `components/game-detail/Page.vue` + `use-game-launch.ts`, `LaunchOptionsModal.vue`, failure modal from
`state-navigation.ts`, Proton settings at `pages/settings/compat.vue`. BPM: `pages/bigpicture/library/[id].vue`
(also maps `launch_trace` steps to status text and shows `bios_warnings`), home quick-launch in
`pages/bigpicture/index.vue`, `LaunchErrorDialog.vue` (reads the log tail via `read_latest_launch_log`).
`read_latest_launch_log` is only used by BPM; desktop opens the folder with `open_process_logs` instead.
`setupHooks()` runs app-wide (`main/app.vue` line 83), so its desktop modal listener is also live while in BPM.

## Server side / client side
Launch configs come from drop-server (`PUT /api/v1/admin/game/{id}/versions/{versionId}/launches`); the client
only reads them from the local DB (`GameVersion.launches`).

## Rules and gotchas
- Blank launch template means `{}`: `launch.rs` line 650 substitutes `"{}"` when `launch_template.trim().is_empty()`.
  `default_template` in `database/src/models.rs` also stores `"{}"`. Placeholders: `{}`/`{0}` (whole command),
  `{dir}`, `{exe}`, `{abs_exe}` (`format.rs`). When an executable override is set and the template has none of `{}`, `{0}`, `{exe}`, `{abs_exe}` (`{dir}` doesn't
  count; `launch.rs` ~655), the override is dropped and `5_template_discards_executable_override` is logged.
- `[LAUNCH]` log lines and the `launch_trace` event carry the exact command: step `6_final_command` has `executable`,
  `args`, `env_vars`, `working_dir`; `info!("[LAUNCH] spawning (cwd ...)")` at `launch.rs` line 738.
- Per-launch stdout/stderr go to `DATA_ROOT_DIR/logs/<game_id>/<version>-<unix>.log` and `...-error.log`
  (`prepare_launch`, `get_log_dir`).
- Never hold `PROCESS_MANAGER` across the save sync. `run_launch` locks twice on purpose; `LaunchReservation`'s
  `Drop` clears `pending_launches` if the sync unwinds, else every later Play answers `AlreadyRunning`.
- `launch_game` must stay `async` and run work on `std::thread::spawn`, not `spawn_blocking`: the launch calls
  `tauri::async_runtime::block_on`, which panics on a Tokio thread (comment in `src/process.rs` `launch_game`).
- The frontend matches `ProcessError` Display text by substring (`launch-failure.ts`, e.g. `"isn't installed yet"`,
  `"Game not installed"`). Rewording a message in `process/src/error.rs` silently breaks the matching dialog title.
- Emulator not installed is not an error to the UI: `ProcessError::RequiredDependency` becomes
  `Ok(LaunchResult::InstallRequired)` in `launch_game_inner`; callers must check `result.result`.
- Handler order matters: for Linux->Windows, `AsahiMuvmLauncher` is listed before `UMUCompatLauncher` in
  `ProcessManager::new`; the first whose `valid_for_platform` passes wins.
- `COMPAT_INFO` / `UMU_LAUNCHER_EXECUTABLE` are `LazyLock`: umu detection is cached for the app's lifetime.
- A native handler finding a Windows binary returns `NeedsCompat`; step 4 rebuilds the command for Windows,
  re-applies overrides, and permanently rewrites `installed_game_version.target_platform` to Windows.
- Launch-config env is filtered by `is_env_key_forbidden` (`LD_PRELOAD`, `PATH`, ...); setting those from a config
  does nothing.
- `prefix_prep.rs` doc comments name `install_vcredist_into_prefix`; the real functions are
  `install_redists_into_prefix` and `install_redists_for_game`.
- Suspicious exit window is `SUSPICIOUS_EXIT_SECS = 2` (`exit.rs` line 42).

## Changing things here
- New launch step or trace: add it in `prepare_launch` / `build_command` with a numbered `launch_trace` step; if
  BPM should show it, add the step id to the label map in `pages/bigpicture/library/[id].vue` (around line 3164).
- New failure message: add a `ProcessError` variant + Display text in `process/src/error.rs`, then a matching
  branch in `describeLaunchFailure` (`main/composables/launch-failure.ts`). Both surfaces pick it up.
- New launch command: `#[tauri::command]` in `src-tauri/src/process.rs`, then register it in `src-tauri/src/lib.rs`
  `generate_handler!`; Linux-only ones need `#[cfg(target_os = "linux")]` on both the fn and the registration.
- New Proton behaviour: `UMUCompatLauncher` in `process_handlers.rs` and `prefix_prep.rs`; keep non-Linux stubs
  compiling (`prefix_prep.rs` has paired `#[cfg]` versions of each pub fn).

## Tests
- `cargo +nightly-2026-04-20 test -p process` (from `src-tauri/`): `exit.rs` (one test,
  `an_incomplete_pre_launch_sync_blocks_the_upload`), `gamepad.rs`, and `save_sync.rs`.
- No tests for `launch.rs`, `process_handlers.rs`, `launch-failure.ts` or the play-button flows.

## Related maps
`docs/map/emulation.md`, `docs/map/cloud-saves-playtime.md`, `docs/map/downloads-install.md`, `docs/map/app-shell-protocols.md`, `docs/map/local-db-settings.md`
