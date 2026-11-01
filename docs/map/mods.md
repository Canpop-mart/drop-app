# Mods (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
A mod is a server Game with `type = Mod` and a `parentGameId`. The game page's Mods tab lists the mods the server offers
for the base game and the ones found on disk. Installing a mod downloads its files straight over the base game's install
folder (optionally into a subfolder), after moving every base-game file it replaces into a backup, so uninstall puts the
game back exactly. A mod can carry a launch override (a different executable to start while it is installed). Prerequisite
mods are installed first. Each mod's state lives in a ledger under `<base install>/.mods/`, which is the source of truth for
what is on disk.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/downloads.rs` | Command `download_mod` (install, update and resume all use it) |
| `src-tauri/src/games.rs` | Commands `uninstall_mod`, `list_installed_mods` (`InstalledMod`), `fetch_game_mods` (GET `/api/v1/client/game/{id}/mods`), `mod_parent_platform`; `parent_install_dir`, `mod_version_platforms` |
| `src-tauri/games/src/downloads/mod_agent.rs` | `ModDownloadAgent`: `new`, `run`, `prepare_install_dir`, `check_parent_unchanged`, `parent_folder_settled`, `validate`, `finish_install`, `on_complete`, `on_cancelled`; `mods_changed_event` (`update_mods/<parentId>`) |
| `src-tauri/games/src/downloads/mod_data.rs` | Ledger `ModData` (`.moddata`, pot) and paths (`MODS_DIR`, `moddata_path`, `pending_marker_path`, `originals_dir`); `ModData::generate`; `installed_ledgers`, `ledger_is_complete`, `overlay_rel`; backups `back_up_originals`, `undo_backups`, `settle_unlisted_originals`; removal `remove_mod`, `release_recorded`, `release_files`; base-game hand-over `hand_files_to_base_game`, `discard_originals`; ownership `mod_owned_files`, `mod_owned_files_spelled`; launch `decide_launch_override`, `find_launch_override` |
| `src-tauri/games/src/library.rs` | `clear_mod_install_state`; `mods_installed_under` (mods cleared when the base game is uninstalled) |
| `src-tauri/process/src/process_manager/launch.rs` | Calls `find_launch_override` when launching the base game |
| `main/composables/game-detail/mods-tab.ts` | Pure shared rules: `shouldShowModsTab`, `buildModCards`, `modCardActions` (install/update/resume/uninstall/remove), `modDependentNames`, `pickModVersion`, `launchOverrideNote` |
| `main/composables/game-detail/use-installed-mods.ts` | `list_installed_mods` + refresh on `update_mods/<id>`; `uninstall(modId)`; resume via `download_mod` |
| `main/composables/game-detail/use-mod-install.ts` | Install with prerequisites (recursive), `PrereqConfirm` injectable confirm |
| `main/components/game-detail/Page.vue` | Desktop Mods tab ("Available Mods", "Installed Mods") and mod uninstall confirmation |
| `main/composables/bigpicture/use-bpm-mods.ts` | BPM state: merged cards, controller-answerable prerequisite confirm |
| `main/components/bigpicture/game-detail/BpmModsTab.vue` | BPM Mods tab grid; `BigPictureDialog` for prerequisites and uninstall |
| `main/components/bigpicture/BigPictureDialog.vue` | Takes and releases the input lock (also on unmount) |

On-disk layout inside the base game's install folder:
`.mods/<modId>.moddata` (ledger), `.mods/<modId>.pending` (install not finished), `.mods/<modId>/originals/<path>` (base-game
copies the mod replaced), `.mods/<modId>/stale-originals` (backups that must never be put back).

## How it flows
1. Install: Mods tab -> `useModInstall` (resolve prerequisites, confirm) -> `invoke("download_mod", {modGameId, parentGameId,
   versionId, targetPlatform, modInstallDir, launchOverride})` -> refuses if this mod or another version of it is queued, an
   update of the parent is active (`update_active`), or the parent is not fully installed -> `ModDownloadAgent::new`
   (overlay dir = parent dir + `modInstallDir`; `ModData::generate`; manifest GET `/api/v1/client/game/manifest`) -> queue.
2. Run: `prepare_install_dir`: `check_parent_unchanged` (parent still Installed at the same folder, its `.dropdata` names the
   installed version), `reload_for_run`, `settle_unlisted_originals`, write `.pending`, `back_up_originals`, record the file
   list in `.moddata` -> chunks via `download_game_chunk` -> trim -> `validate` -> `on_complete`: `finish_install` (release
   files the old version had and this one does not, remove `.pending`) -> `on_game_complete` -> emit `update_mods/<parentId>`.
3. Uninstall / remove unfinished: `invoke("uninstall_mod", {modGameId, parentGameId})` -> refused while the mod, the parent
   or a sibling mod has any transient status (`ModBusy` / `GameBusy`) -> `mod_data::remove_mod` (restore originals, delete the
   mod's files unless another mod lists them, delete ledger, marker, state folder) -> `clear_mod_install_state`.
4. Base game writes over a mod file (download, repair or in-place update): `hand_files_to_base_game` per chunk before the
   chunk is recorded, so the file stops being the mod's and its old backup is discarded.

## Both surfaces
Desktop: Mods tab in `components/game-detail/Page.vue` (uses `useInstalledMods`, `useModInstall`, `mods-tab.ts`). BPM:
`BpmModsTab.vue` via `use-bpm-mods.ts`, rendered from `pages/bigpicture/library/[id].vue`. Both call the same commands and the
same `mods-tab.ts` rules. BPM uses `BigPictureDialog` for confirms because the desktop `createModal` stack has no gamepad focus
(`use-mod-install.ts` `PrereqConfirm`). No known feature gap.

## Server side
Mod games, `modInstallDir`, `launchOverride`, prerequisites and the mods listing route: `../drop-server/docs/map/library-import.md`.

## Rules and gotchas
- Never run game-shaped commands on a mod: its recorded install dir is the parent's folder (or a subfolder), so
  `uninstall_game` would delete the base game. `uninstall_game` returns `LibraryError::IsMod`; `resume_download` refuses
  `DownloadType::Mod`. A mod must never get a PartiallyInstalled status (`ModDownloadAgent::on_cancelled` comment).
- `.pending` present = unfinished install, even if the ledger exists (`ledger_is_complete`). Only `finish_install` removes it.
  A launch override from an unfinished mod is skipped (`OverrideSkip::Unfinished`).
- Backups are made BEFORE the ledger lists the new files (`prepare_install_dir` doc): a listed file counts as the mod's own
  and is never backed up. Reordering this loses originals on a crash.
- A corrupt `.moddata` is logged and the download refuses to start over it (`ModData::generate`, `prepare_install_dir`);
  `remove_mod` returns "the mod's file list is unreadable". Do not replace it with an empty ledger.
- `ModData` deliberately has no `UserConfiguration` field: the comment in `mod_data.rs` `v1::ModData` says embedding it made
  `read()` fail to decode what `write()` produced. New ledger fields need the same care as `.dropdata` (pot: never remove or rename a field; see `docs/map/local-db-settings.md`).
  Unlike `.dropdata`, `ModData` has no fallback reader, so a bad ledger blocks downloads and `remove_mod`.
- Keep extra per-mod state as plain files next to the ledger (as `.pending` and `stale-originals` are), not as ledger fields
  (`MODS_DIR` doc).
- If ownership cannot be read, the base game's stale-file sweep removes nothing (`remove_files_dropped_since_previous_version`)
  and the in-place planner treats mod files as the player's (`update::prepare`).
- `discard_originals` runs whenever the base game drops or rewrites a file, for every mod, even ones with unreadable ledgers;
  a backup it cannot delete is listed in `stale-originals` and never restored.
- Uninstalling the base game deletes its mods with the folder; `mods_installed_under` is read first so their statuses are
  cleared (`uninstall_game_logic`).
- `download_mod` does not check whether the game is running; `uninstall_mod` refuses while the parent has any transient status
  (Running included, since running games are recorded in `transient_statuses`).
- Several mods with launch overrides: the first ledger in `installed_ledgers` order wins; the others get
  `OverrideSkip::OtherMod` and the UI shows it via `launch_override_skipped`.

## Changing things here
- New mod action: rule in `mods-tab.ts` (`modCardActions`) + test in `tests/mods-tab.test.ts`, then wire it in `Page.vue`
  AND `BpmModsTab.vue` (registered with `registerAction`, confirm through `BigPictureDialog`).
- New mod command: `src-tauri/src/games.rs` or `downloads.rs`, register in `src-tauri/src/lib.rs` `generate_handler!`, emit
  `mods_changed_event(parent)` after changing anything on disk so both tabs refresh.
- Change how files are restored or removed: `mod_data.rs` (`release_recorded`, `release_files`, `settle_unlisted_originals`)
  and its tests; check `hand_files_to_base_game` and `discard_originals` callers in `download_agent.rs` and `update/mod.rs`.

## Tests
- From `src-tauri/`: `cargo +nightly-2026-04-20 test -p games downloads::mod_data` and `... downloads::mod_agent`.
- From `main/`: `node --test tests/mods-tab.test.ts`.

## Related maps
`docs/map/downloads-install.md`, `docs/map/in-place-updates.md`, `../drop-server/docs/map/library-import.md`.
