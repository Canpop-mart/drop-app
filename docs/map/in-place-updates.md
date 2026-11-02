# In-place game updates (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
When the server has a newer version, or a new revision of the installed version, the player can update the existing
install folder instead of installing a second copy. The client plans the change per file (what Drop last installed,
what the server ships now, what is on disk), shows a review where the player decides every conflict, then downloads
only the files that change into a hidden staging folder, verifies them and swaps them in with a crash-safe journal.
Player files Drop never shipped are never touched, except in folders the admin marked as mirrored, where the review
offers to remove them. A check runs every 30 minutes (5 when offline) and on demand.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/updates.rs` | `GameUpdater` (scheduled task), `run_update_check`, `update_available`, `default_target`; commands `check_for_updates`, `plan_game_update`, `apply_game_update`, `recover_game_update` |
| `src-tauri/games/src/downloads/update/mod.rs` | Engine API: `prepare`, `apply`, `UpdateError` (player text), `update_active` / `try_claim` / `release` (one update per game), `with_launches_blocked`, `games_using`, `fetch_revision_files`, `load_baseline`, `move_install_record`, `recover_install`, `recover_update`, `set_aside_update_dir`, `back_up_kept_files`, `recover_all_at_startup`, `recover_before_launch`, `launch_blocked`, `record_fresh_baseline` |
| `src-tauri/games/src/downloads/update/plan.rs` | Pure three-way planner: `plan(PlanInput)`, `resolve`, `Plan` (`writes`, `conflicts()`, `backup_paths()`, `recovery_paths()`, `skipped_linked`, `mirrors`), `Resolution`; rule table in the module docs |
| `src-tauri/games/src/downloads/update/baseline.rs` | `.drop-baseline.json` sidecar: `Sidecar` (`kept_mine`, `healed_protected_folders`, `kept_extras`), `read_sidecar`, `write_sidecar`, `fresh_sidecar`, `carry_over`, `FsDisk` (DiskView over the real folder), `sha256_file` |
| `src-tauri/games/src/downloads/update/agent.rs` | `UpdateAgent` (a `Downloadable`): `run` (stage chunks into `.drop-update/new/`, resume via `state.json`), `verify_staged`, `commit_update`, `stage_meta` (builds the journal, fresh `.dropdata`), `kept_mine_after`, `kept_extras_after` |
| `src-tauri/games/src/downloads/update/commit.rs` | `commit`, `rollback`, `finish`, `Journal` / `Phase::{Moving, Committed}`, `lock_staging`, `recovery_stamp`, `free_bak_path`, `discard_staging` |
| `src-tauri/games/src/downloads/download_agent.rs` | `is_drop_runtime_data`, `is_player_data_folder` (folder rules the planner uses) |
| `src-tauri/process/src/process_manager/launch.rs` | Launch refuses with `ProcessError::UpdateInProgress` when `launch_blocked` |
| `src-tauri/src/process.rs` | Calls `recover_before_launch` before every launch |
| `src-tauri/src/lib.rs` | `set_launch_gate` (process manager), `recover_all_at_startup` before `reconcile_on_startup` |
| `main/composables/game-detail/update-review.ts` | Pure rules shared by both surfaces: `UpdatePlan` type, `pickUpdateInstall`, `isUpToDate`, `buildResolutions`, `applyArgs`, paging (`CONFLICT_PAGE_SIZE` = 8), `shouldAskBeforePlay`, `queueItemLabel`, `NEEDS_RECOVERY_MARKER`, all placeholder strings |
| `main/composables/game-detail/use-game-update.ts` | `useGameUpdate` (phases idle/loading/error/repairing/up_to_date/ready/applying/queued; `review`, `recheck`, `apply`, `repairThenRecheck`), `useUpdateRepair` |
| `main/composables/update-tracking.ts` | `markUpdating` / `isUpdating` (queue labels an update vs install) |
| `main/components/game-detail/UpdateReviewModal.vue` | Desktop review overlay |
| `main/components/game-detail/Page.vue` | Desktop wiring: `updateCtl`, `repairCtl`, play prompt |
| `main/pages/bigpicture/library/[id].vue` | BPM review as page rows (template section "In-place update review as plain focusable rows"; script section "In-place updates" and "A stuck update") |

## How it flows
1. Detect: `GameUpdater::call` -> `run_update_check` -> per install, server versions + revisions -> `update_available`
   (newer version only if `enable_updates`; a higher revision of the installed version always counts; delta versions never)
   -> sets `update_available` on the install and status, emits `update_game/<gameId>`.
2. Review: Update button -> `useGameUpdate.review(versionId)` -> `invoke("plan_game_update", {gameId, installVersionId, targetVersionId?})`
   -> `default_target_for_install` -> `engine::prepare`: refuse if busy; `recover_install`; refuse delta (`DeltaVersion`);
   manifest GET `/api/v1/client/game/manifest` (no `revision` = `ServerUnsupported`); target list GET
   `/api/v1/client/game/version/files?version=..` (404 = `NoFileList`); `load_baseline` (local sidecar, else server
   `revision=earliest`, else empty); `plan::plan` on a blocking thread -> `UpdatePlan` to the UI.
3. Apply: player resolves conflicts -> `applyArgs` -> `invoke("apply_game_update", {gameId, installVersionId, toVersionId,
   toRevision, mirrorFolders, resolutions})` -> `engine::apply`: re-plans, `same_as_reviewed` (revision and mirror folders),
   `check_can_update`, refuses if the game or anything launching through it runs, `plan::resolve`, disk check,
   `try_claim`, queue `UpdateAgent`.
4. Queue: `UpdateAgent::run` stages files -> `validate` -> `verify_staged` (SHA-256 of every staged file) -> `commit_update`:
   `stage_meta` (journal, new sidecar, fresh `.dropdata` with no previous version) -> under the launch gate `commit_staged`
   -> `apply_handover` (mods) -> `move_install_record` -> `commit::finish` (`.bak` / `.drop-removed` copies, clean-up).
5. Stuck update: launch fails `UpdateInProgress` or plan fails with `[needs-recovery] ` -> `invoke("recover_game_update")` ->
   `recover_update`: retry `recover_install`; if that fails, rename `.drop-update` to `.drop-update-set-aside-<unix secs>`.

## Both surfaces
Desktop: `UpdateReviewModal.vue` opened from `components/game-detail/Page.vue`. BPM: no modal and no input lock; the review,
conflict pager, backups list and "Repair update" are focusable rows in `pages/bigpicture/library/[id].vue`, focus moved by
key (`conflict:<path>`, `apply`, `recheck`, `close`) through `registerReviewEl` / `focusReviewEl`. Both read all rules and
text from `update-review.ts`, so a rule change there reaches both. Progress shows on each surface's queue page.

## Server side
Revisions, fingerprints, mirrored folders and "Publish update": `../drop-server/docs/map/library-import.md`.

## Rules and gotchas
- The planner is a three-way decision per path (baseline B, target T, disk D); the full table is the `plan.rs` module doc.
  A path in neither B nor T is never looked at, except the one-shot healing pass and mirrored folders.
- Drop runtime data (`is_drop_runtime_data`: `drop-goldberg/`, `steam_settings/`, GBE save folders at any depth,
  `drop-saves/`, `.mods/`, `.drop-removed/`) is only written when nothing is on disk there; never replaced, deleted or asked about.
- Player-data folders (`is_player_data_folder`: top-level `user/`, `saves/`, `system/`, `nand/`, `sdmc/`, `keys/`, ...): a shipped
  file the update changes or removes becomes a conflict unless it is byte-identical to the baseline; an unchanged one is left
  alone silently. A linked player-data folder (SD card on the Deck) gets no operations and is reported in `skippedLinkedFolders`.
- Mirrored folders (`mirrorFolders`, a per-version server setting delivered with the target file list) end up exactly like
  T: conflicts become replacements, other files become `extra_file` conflicts, lost copies go to
  `.drop-removed/<YYYYMMDDTHHMMSSZ>/<path>`. `kept_mine` does not apply there. If the admin changes them after the review,
  `apply` fails `MirrorsChanged`.
- An unverified baseline entry (hash `""` or no mtime) never raises a conflict: the update wins and the old file is kept as
  `<file>.bak` (`Plan::backup_paths`).
- Staging lives in `<install>/.drop-update/` (`new/`, `old/<n>`, `journal.json`, `state.json`). Everything that moves or
  deletes there must hold `commit::lock_staging()`. The lock is not reentrant: `discard_staging` does NOT take it (call it
  only while holding the lock), `discard_staging_locked` and `recover_install` (`update/mod.rs` ~975) DO take it (never
  call them while holding it, or it deadlocks; `commit.rs` ~676-685). Before `Phase::Committed` recovery rolls back, after it rolls forward.
- `recover_all_at_startup` must run before `reconcile_on_startup` (`src/lib.rs`): mid-commit the `.dropdata` sits in staging
  and the reconcile would demote the game to PartiallyInstalled.
- The update writes a fresh `.dropdata` with `previously_installed_version = None` (`stage_meta`). Recording the old version
  would make a later repair run the full-download sweep and delete files the player chose to keep.
- The baseline is a JSON sidecar on purpose, not a pot field: a bad sidecar only falls back to the server list
  (`baseline.rs` module doc). New `Sidecar` fields need `#[serde(default)]` to read older sidecars.
- `check_can_update` refuses while ANY mod download is in the queue (any game), not just this game's (`ModDownloadActive`).
- `update_active` returns true if its mutex is poisoned; `download_mod` and `uninstall_game` check it and refuse.
- Emulators: `games_using` blocks the commit while any ROM that launches through the emulator is running.
- Repair after an update: `back_up_kept_files` writes `.bak` for "keep mine" files the repair is about to overwrite.

## Changing things here
- New planner rule: edit `plan.rs` (doc table first, then code), add a test in its `tests` module with the in-memory disk.
  If the UI must show it, add the field to `UpdatePlan` (`update/mod.rs` `summary`) and to `UpdatePlan` in `update-review.ts`.
- New `UpdateError`: add the variant and its text in `update/mod.rs`; the UI shows the string as-is (`updateErrorText`). Text
  is placeholder copy for the owner.
- New review control: put the rule in `update-review.ts`, wire it in `UpdateReviewModal.vue` AND as a registered row in
  `pages/bigpicture/library/[id].vue` (key-based focus, no input lock).
- New command: `src-tauri/src/updates.rs` plus `generate_handler!` in `src-tauri/src/lib.rs`.

## Tests
- From `src-tauri/`: `cargo +nightly-2026-04-20 test -p games downloads::update` (plan, baseline, commit, agent, mod.rs tests);
  `cargo +nightly-2026-04-20 test -p drop-app updates::` (`update_available`, `default_target`).
- From `main/`: `node --test tests/update-review.test.ts`.

## Related maps
`docs/map/downloads-install.md`, `docs/map/mods.md`, `../drop-server/docs/map/library-import.md`.
