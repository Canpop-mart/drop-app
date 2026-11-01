# Emulation (RetroArch, Switch emulators, consoles)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
ROMs are ordinary Drop games whose launch config names an `emulator` (another Drop game: RetroArch, Eden,
Ryujinx, ...). At launch the client finds the emulator's install, builds its command with the ROM, and for
RetroArch writes `retroarch.cfg` + `retroarch-core-options.cfg` (dirs, saves, controller layout, quality,
aspect, CRT shader, RetroAchievements login, BIOS placement). Yuzu-family Switch emulators get portable mode
and fresh player-1 bindings each launch. The library groups ROMs into "consoles" (one per server
FlatFilesystem library) and lists installed emulator hosts with their cores.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/process/src/process_manager/launch_emulator.rs` | `build_emulator_command`: resolve emulator install, `{rom}` substitution or `-L core rom` for RetroArch, `.m3u` for multi-disc, `RequiredDependency` when emulator missing |
| `src-tauri/process/src/process_manager/launch.rs` | `configure_retroarch` (2 s RA credential fetch, `--appendconfig`, `--verbose`), `configure_switch_emu`; both called from `build_command` |
| `src-tauri/process/src/m3u.rs` | `generate_m3u`, `cleanup_m3u` |
| `src-tauri/process/src/gamepad.rs` | gilrs pad detection: `detect_primary_pad_family` (RetroArch), `resolve_primary_pad` (Switch), `list_connected_gamepads` |
| `src-tauri/remote/src/retroarch/mod.rs` | `configure_retroarch_for_game` (entry point), `write_core_options`, stale-key list, `RetroArchInfo` (`bios_warnings`, `video_driver`) |
| `src-tauri/remote/src/retroarch/cfg.rs` | `PathStyle` (`Native` / `Wine`), `to_wine_path`, `path_to_cfg`, `patch_retroarch_cfg(_with_deletions)` |
| `src-tauri/remote/src/retroarch/presets.rs` | `apply_quality_preset`, `apply_widescreen` (retroarch.cfg); `apply_core_quality_options`, `apply_core_widescreen_options` (core options); `InstalledCores` |
| `src-tauri/remote/src/retroarch/cores.rs` | `EXTENSION_CORE_MAP`, `resolve_core_for_rom`, `detect_iso_disc_type` |
| `src-tauri/remote/src/retroarch/controllers.rs` | `PadFamily`, `apply_controller_mappings`, `set_face_button_fallback`, Nintendo `.rmp` remaps, `apply_hotkey_bindings` |
| `src-tauri/remote/src/retroarch/bios.rs` | `BIOS_SPECS`, `check_and_place_bios` |
| `src-tauri/remote/src/retroarch/shaders.rs` | `apply_crt_shader` (writes `presets/global.slangp`), `disable_crt_shader` |
| `src-tauri/remote/src/retroarch/ra.rs` | `fetch_ra_credentials`, `sync_ra_credentials`, `detect_ra_login_failure`, `mark_credentials_expired`, `check_rom_hash` |
| `src-tauri/remote/src/retroarch/logs.rs`, `discovery.rs` | `detect_fatal_video_error`; `is_retroarch`, `find_appimage_config_dir` |
| `src-tauri/remote/src/switchemu/mod.rs` | `configure_switch_emu_for_game`, `SwitchEmuOutcome` |
| `src-tauri/remote/src/switchemu/discovery.rs` | `detect_switch_emulator` (`SwitchEmuFamily::YuzuLike` / `Ryujinx`), `ensure_portable_dir`, `qt_config_path` |
| `src-tauri/remote/src/switchemu/controllers.rs`, `ini.rs`, `gamecontrollerdb.txt` | SDL GUID -> mapping (`lookup_pad_mapping`, `build_player0_entries`); `patch_ini_section` |
| `src-tauri/src/emulators.rs` | Commands `list_installed_emulators` (`EmulatorHost`), `open_emulator_cores_dir`; `fetch_and_cache_game`, `list_core_files` |
| `src-tauri/src/games.rs` | `check_ra_rom_hash` command (on-demand RA hash check) |
| `src-tauri/src/settings.rs` | `ra_store_credentials`, `ra_refresh_credentials`, `ra_clear_credentials` |
| `main/composables/console-emulators.ts` | `withEmulatorsFirst` (pure, tested) |
| `main/composables/emulators.ts` | `listInstalledEmulators`, `EmulatorHost` type |
| `main/composables/use-server-api.ts` | `api.emulation.consoles()` -> `api/v1/emulation/consoles` |
| `main/pages/library/console/[id].vue` | Desktop console page: consoles + `fetch_library`, emulators first, error state |
| `main/pages/library/emulator/[id].vue`, `main/pages/library/index.vue` | Desktop emulator host page (open folder, open cores, uninstall) and the library's Emulators section |
| `main/pages/bigpicture/index.vue` | BPM library: `loadConsoles`, `loadConsoleEmulators`, console filter (`activeGroup.kind === "console"`) |
| `main/composables/console-sections.ts`, `main/composables/console-art.ts` | BPM console section toggle (localStorage) and console artwork |
| `main/components/GameOptions/Video.vue`, `main/composables/bigpicture/use-bpm-game-config.ts` | Per-game controller type, quality preset, aspect ratio, CRT shader (desktop / BPM) |

## How it flows
1. Launch an emulated game: `prepare_launch` sees `launch.emulator` -> `fetch_process_handler` uses the
   emulator's own target platform -> `build_emulator_command` sets `effective_cwd` = emulator install dir.
2. `build_command` -> `configure_retroarch` -> `remote::retroarch::configure_retroarch_for_game(emu_dir, user_id,
   game_id, ra_creds, user_config, detected_pad, rom, under_proton)`; `under_proton` comes from
   `ProcessHandler::runs_under_proton` (true only for `UMUCompatLauncher`).
3. Writes are patches (keys replaced, stale keys deleted). Saves dir comes from
   `save_sync::resolve_emu_saves_root`. Trace step `7_retroarch_config_result` carries `bios_warnings`.
4. `configure_switch_emu` runs for every emulator launch; non-Switch dirs return `NotSwitchEmulator` and nothing
   is traced.
5. Exit: `on_process_finish` reads RetroArch's session log for a token rejection (`check_ra_credentials`) and
   for a video-driver failure (`detect_fatal_video_error`), which goes into `launch_external_error_detail`.
6. Library: `api.emulation.consoles()` returns `{ consoles: [{ id, name, shortName, gameIds, emulatorIds }] }`; desktop and
   BPM both put emulators first with `withEmulatorsFirst`. Emulator hosts: `list_installed_emulators` unions
   "referenced as some launch config's emulator" with `is_retroarch(dir)`.

## Both surfaces
Desktop: `pages/library/console/[id].vue`, `pages/library/emulator/[id].vue`, Emulators section in
`pages/library/index.vue`, per-game emulator options in `components/GameOptions/Video.vue`.
BPM: console filter on `pages/bigpicture/index.vue`, per-game options via `use-bpm-game-config.ts` on
`pages/bigpicture/library/[id].vue`, `check_ra_rom_hash` and `list_gamepads` are called from that page.
Gap: BPM has no emulator host list or host page; `list_installed_emulators` / `open_emulator_cores_dir` are only
called from desktop pages (grep `main/`).

## Server side / client side
Server: `../drop-server/server/api/v1/emulation/consoles.get.ts` (consoles = FlatFilesystem libraries) and
`../drop-server/server/internal/library/emulatorResolution.ts`. No emulator logic runs server-side at launch.

## Rules and gotchas
- Core options go to `retroarch-core-options.cfg`, not `retroarch.cfg`. `mod.rs` sets `global_core_options = true`
  and `core_options_path` (line 418) and deletes legacy `dolphin_renderer` from `retroarch.cfg` (stale-key list
  near line 67). RetroArch silently replaces a rejected value with the core default and logs nothing.
- Core option values come from the core's own option table, not menu labels. Example: `dolphin_anti_aliasing`
  takes `"0".."6"` (`presets.rs` line 197). Options are only written for cores present in `cores/`
  (`InstalledCores`).
- Under Proton, every path written into a config must be `PathStyle::Wine` (`cfg.rs`): a Unix path is resolved
  against the current Wine drive and doubles (`/home/deck/home/deck/...`). Arguments like `-L <core>` stay native
  (comment in `configure_retroarch`).
- `--appendconfig` cannot be injected when RetroArch is script-wrapped (`is_script`); it is skipped with a warn.
- RetroArch writer and save-sync scanner must agree on the saves path: both use
  `save_sync::resolve_emu_saves_root` (`mod.rs` line 174). Writing saves elsewhere makes cloud sync miss them.
- RA Connect tokens expire (45-60 days) and RetroArch only says so in its own log; `check_ra_credentials` runs
  on exit and `mark_credentials_expired` stops re-injecting a dead token.
- Ryujinx is detected but never written to (`SwitchEmuFamily::Ryujinx`). Yuzu-family gets `user/` portable dir
  every launch (`ensure_portable_dir`), or saves land in a throwaway Proton prefix.
- Pad detection at launch uses gilrs (`process/src/gamepad.rs`), but app input polling is the Web Gamepad API in
  `main/composables/gamepad.ts`; `start_polling` is commented out in `src-tauri/src/lib.rs` line 155.
- Emulator host cards: on cache miss with network, `fetch_and_cache_game` asks `/api/v1/client/game/{id}` with a
  3 s `LOOKUP_TIMEOUT`; 404s are remembered for the session; fallback name is `"Emulator"`. Core count is the
  number of `.dll` / `.so` / `.dylib` files in `<install>/cores/` (`list_core_files`).
- `ra.rs` module doc says `serde_json` is unavailable in `remote`; `remote/Cargo.toml` now has it.
- `configure_game_emulator` (`src/games.rs`) is the Goldberg Steam emulator, not a console emulator.

## Changing things here
- New ROM extension or core: one row in `EXTENSION_CORE_MAP` (`cores.rs`); add core option keys in `presets.rs`
  only with values from that core's `libretro_core_options.h`; add a test in the same file.
- New per-game emulator setting: field on `UserConfiguration` in `database/src/models.rs` with `#[serde(default)]`,
  apply it in `configure_retroarch_for_game`, add UI in `components/GameOptions/Video.vue` AND
  `composables/bigpicture/use-bpm-game-config.ts` (focus-registered controls on BPM).
- New Switch emulator family: `src-tauri/remote/src/switchemu/discovery.rs` (`detect_switch_emulator`), then `src-tauri/remote/src/switchemu/controllers.rs` mapping.
- New emulator command: `src-tauri/src/emulators.rs`, register in `src-tauri/src/lib.rs` `generate_handler!`.

## Tests
- `cargo +nightly-2026-04-20 test -p remote retroarch::` and `cargo +nightly-2026-04-20 test -p remote switchemu::`
  (from `src-tauri/`). Tests live in `cfg.rs`, `presets.rs`, `controllers.rs`, `cores.rs`, `bios.rs`, `ra.rs`,
  `shaders.rs`, `logs.rs` and all four `switchemu` files.
- `node --test tests/console-emulators.test.ts` (from `main/`).
- No tests for `launch_emulator.rs` or `src-tauri/src/emulators.rs`.

## Related maps
`docs/map/launch-process.md`, `docs/map/cloud-saves-playtime.md`, `docs/map/local-db-settings.md`, `docs/map/downloads-install.md`
