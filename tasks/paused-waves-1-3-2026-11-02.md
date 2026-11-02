# Task: Waves 1-3 of docs/UPCOMING.md execution plan (2026-11-01)

Owner asked to tackle waves 1-3 from the new "Execution order" section in `docs/UPCOMING.md`.
Multi-task session: work through items in order, update this file as I go.

## Goal
Ship wave 1 (client quick wins), wave 2 (saved data + settings behavior), wave 3 (dual-surface gaps).
Compiles on Windows; device/BPM behavior untested unless noted.

## Map(s) used
`docs/map/local-db-settings.md`, `docs/map/big-picture.md`, `docs/map/downloads-install.md`, `docs/map/desktop-frontend.md` (as needed per item)

## Plan
Wave 1:
- [x] W1-1 startup `.expect` panic on drop.db read -> return error (`src-tauri/database/src/interface.rs`)
- [x] W1-2 scope in-place update refusal to the game's own mod queue (`update/mod.rs` ~743)
- [x] W1-3 BPM download toast A/B without input lock (`BpmDownloadToast.vue` ~70-82)
- [x] W1-4 BPM shelf picker "Done" unreachable after Create New Shelf (`library/[id].vue` ~2505)
- [x] W1-5 stale comments (pot round-trip, mod_agent on_complete, save-sync-state.ts, regenerate-manifests.ts)
- [ ] W1-6 bug reports attach rotated log archives too
- [ ] W1-7 `sync_game_saves_now`: wire up or remove registration
Wave 2:
- [ ] W2-1 download queue folder index -> stable id (format change, keep old DBs loading)
- [ ] W2-2 `update_settings` partial patch + visible save failures
Wave 3:
- [ ] W3-1 BPM uninstall version pick (parity with desktop)
- [ ] W3-2 desktop local-save Backup/Restore button (parity with BPM)
- [ ] W3-3 launch-failure.ts structured errors instead of substring matching

Checks (bash, from the stated dir):
- `cargo +nightly-2026-04-20 check --all-targets 2>&1 | tail -60; echo "exit=${PIPESTATUS[0]}"` (src-tauri/)
- `pnpm exec nuxt typecheck 2>&1 | tail -40; echo "exit=${PIPESTATUS[0]}"` (main/)

## Done so far
- created this file from TEMPLATE.md
- **W1-1 done** (code + `cargo check --all-targets` exit=0). Changes:
  - `database/src/interface.rs`: `set_up_database()` now returns `DatabaseResult<DatabaseInterface>`; the five `create_dir_all(...).panic!` became one loop returning `Err(DatabaseError::Io)` with per-dir context; both `.expect("Database could not be created")` and the recovery `.expect` are gone (errors propagate). Added `DatabaseInterface::initialize()` (idempotent, sets the global once) and `DatabaseInterface::global()` (invariant-checked accessor).
  - `database/src/db.rs`: `DB` is now `pub(crate) static OnceLock<DatabaseInterface>` (was `LazyLock`, was pub + re-exported). Crate root no longer re-exports it; re-exports `DatabaseInterface` instead.
  - `src/lib.rs` setup(): calls `DatabaseInterface::initialize()` first thing after log4rs init; on Err logs via log4rs, shows native dialog (`handle.dialog().message(...)`), then `std::process::exit(1)`. Player sees a real message instead of instant close.
  - `remote/src/requests.rs`: `generate_url` used `DB.fetch_base_url()` directly (found during this item; not in the original access-site list). Now `DatabaseInterface::global().fetch_base_url()`. All other DB use goes through `borrow_db_checked`/`borrow_db_mut_checked`, which now route via `global()`.
  - The only remaining `.expect` on a DB path is inside `global()` itself: fires only if some code uses the DB before init succeeded, which setup() makes unreachable (it exits first). Documented as programming-error invariant.
- **W1-2 done** (code + test; `cargo check --all-targets` exit=0, new unit test passes). Changes in `src-tauri/games/src/downloads/update/mod.rs`:
  - `apply()` now reads this install's on-disk mod ledgers (`mod_data::installed_ledgers`, before the db lock, same pattern as `uninstall_mod`) and collects their game ids; unreadable/missing `.mods` fails open to an empty list.
  - `check_can_update` takes those ids and refuses with `ModDownloadActive` only when a queued/running mod's id is one of THIS install's mods (a mod's transient status is keyed by the mod's own id, not its parent game). Mods of other games no longer hold this update back.
  - Fresh-mod gap (never installed here => no ledger until it starts) is safe: downloads run strictly sequentially and `commit_staged`'s per-file "still as planned" check refuses to swap over anything unplanned (`ChangedSinceReview`).
  - New regression test `only_this_games_mods_block_its_in_place_update`: other-game mod queued => Ok; this-game mod (in ledger list) queued => `ModDownloadActive`.
  - Map updated: the "Rules and gotchas" line in `docs/map/in-place-updates.md` documented the OLD any-game refusal; rewritten to describe the scoped check + ledger source. No new player-facing strings (`UpdateError::ModDownloadActive` text stays accurate).
- **W1-3 done** (code + `pnpm exec nuxt typecheck` exit=0, probe-verified that the checker actually runs). Changes:
  - `main/composables/focus-navigation.ts`: new read-only `isTopOfInputLock(id)` next to acquire/release; exported from `useFocusNavigation()`. True only for the most recently acquired lock id.
  - `main/components/bigpicture/BpmDownloadToast.vue` (Modal Pattern A, mirrors BigPictureDialog): while a toast is visible it holds an input lock (`watch(current !== null, ..., { immediate: true })`, release on hide AND unmount); both A/B subscriptions now `{ bypassInputLock: true }` and act only when `ownsInput()` = our id is top of the lock stack.
  - Why the topmost guard (not just lock + bypass): gamepad dispatch is flat, so a bypass sub keeps firing under every nested lock; without deferring, A on a launch-error/save-conflict dialog that opens over the toast would also navigate to library. Today those cases work only because our non-bypass subs are silenced by their lock.
  - Mount-ordering dependency confirmed: `main/layouts/bigpicture.vue` calls `useFocusNavigation()` in layout setup (runs before any child onMounted), which wires focus-nav's own button subs first; ours register later in the same Set, so on one press focus-nav skips (lock held) and only our handler acts. Same guarantee BigPictureDialog relies on.
  - Accepted limitation (codebase-wide, not new): if an overlay was ALREADY open when a download completes, its lock is below ours and flat dispatch means both fire — same gotcha as every other bypass overlay; noted in the map.
  - Map updated: `docs/map/big-picture.md` focus-navigation row lists `isTopOfInputLock`; BpmDownloadToast row + nested-lock gotcha note it. No new player-facing strings.

- **W1-4 done** (code + `pnpm exec nuxt typecheck` exit=0). Change in `main/pages/bigpicture/library/[id].vue`, shelf picker section:
  - Root cause confirmed by reading: `wireShelfGamepad()` captured `const totalItems = shelves.length + 2` ONCE when the picker opened, and only the D-pad-down clamp used it. After "Create New Shelf" (which calls `createShelfAndAdd` -> `shelvesData.createShelf` -> full server refetch replacing the list), Done moved to index N+2 but the stale max stayed at N+1, so D-pad could never reach it. The A/South handler already read the shelf count live, which is why only navigation broke.
  - Fix: removed the captured const; D-pad-down now computes `totalItems = shelvesData.shelves.value.length + 2` inside the handler at press time (comment explains why). No re-wiring needed because handlers are registered once but read live state. Focus idx is unchanged by creation, so after creating a shelf the highlight lands on the new row (server appends it) and one D-pad-down reaches Done.
  - BPM rules respected: picker's existing input lock + `pickerHasInput()` guard untouched; no DOM focus registration involved (highlight is index-driven CSS); no new player-facing strings.
- **W1-5 done** (comments only; `cargo check --all-targets` exit=0 + `pnpm exec nuxt typecheck` exit=0). Four fixes:
  - `src-tauri/games/src/downloads/mod_data.rs` ModData NOTE: dropped the wrong "pot cannot round-trip #[serde(default)] fields" guess (and its "same latent bug affects DropData resume" tail); now states the real cause per drop_data.rs — AspectRatio's hand-written Deserialize desyncs on pot's unit-enum encoding, serde defaults are fine in pot.
  - `src-tauri/games/src/downloads/mod_agent.rs` on_complete: "install_dir it stores is the parent dir" -> "the mod's base_path (the parent game's install dir joined with the version's modInstallDir)"; matches what is actually passed (`self.moddata.base_path`) and mod_data.rs's field doc.
  - `main/composables/save-sync-state.ts:16`: nonexistent `keep_other_accounts_changes_as_conflicts` -> real fn `apply_shared_device_rule` (verified at process_manager/save_sync.rs:1516; it is the launch-sync other-account rule, so "plus the launch sync's ..." wording stays accurate).
  - `../drop-server/server/internal/tasks/registry/regenerate-manifests.ts`: checksum-error cite `download_logic.rs:173` (a decryption call, `cipher.apply_keystream`) -> `:205`, where `digest != chunk_data.checksum` returns `ApplicationDownloadError::Checksum`. Needed write access to ../drop-server (granted this session); change uncommitted there too.
## Findings
- W1-1: the panics are in one fn around interface.rs:77-95 (`open`/startup path): `.expect("Database could not be created")` x2 and `.expect("failed to recover from failed database")`. `handle_invalid_database` already returns Err on the non-reset branch (line ~231), so only the recovery-failure expect is a real panic; the create_at_path expects fire when writing a fresh DB fails.
- W1-1: direct `DB.` access sites were lib.rs setup, interface.rs internals (`DBWrite::drop`, both borrow fns) AND `remote/src/requests.rs` (missed in initial investigation). All converted.
- ENV QUIRK (Windows + Git Bash): this shell has no `/tmp`, so MSVC linker fails with `LNK1104: cannot open file 'C:\\tmp\\lnk{...}.tmp'` on any link step. Fix for cargo commands: prefix with `export TMP="$(cygpath -w /c/Users/Marts_PC/AppData/Local/Temp)" TEMP="$TMP";`. (Use cygpath -w, not the raw /c/ path.)
- W1-1 dialog uses tauri-plugin-dialog 2.6.0 `DialogExt`, which is a blanket impl over all `Manager<R>` (verified in registry source), so `handle.dialog()` works on AppHandle pre-window; `.show(|_| {})` blocking call mirrors the existing "Database corrupted" dialog pattern at lib.rs ~830.
- W1-3: `useFocusNavigation()` wires gamepad subs on first call (`wireGamepad()`, guarded by `gamepadWired`); layout setup calls it before child onMounted, so focus-nav's South/East subs always precede BpmDownloadToast's in the dispatch Set. `releaseInputLock` is out-of-order safe (lastIndexOf + splice).
- W1-3: env quirk also hits node/pnpm — same missing /tmp; and a single `export A=... B="$A"` expands $A to its OLD value, so TMP/TEMP/TMPDIR must be exported in separate statements before pnpm commands.

## Not checked
- W1-1: dialog + exit(1) path verified by compile only, not run on a device.
- W1-2: unit-tested, not exercised end-to-end in a running app.
- W1-3: typecheck only; BPM behavior (lock acquire/release, deferral under launch-error dialog) untested on device/BPM.
- W1-4: typecheck only; D-pad reachability after Create New Shelf untested on device/BPM.

## New player-facing strings
- W1-1, `src-tauri/src/lib.rs:168` (dialog body): "Drop could not start because its local database could not be opened or created.\n\n{e}\n\nCheck that there is space on the disk and that Drop can write to its data folder, then try again." (`{e}` = `DatabaseError` Display, e.g. "database I/O error: ...")
- W1-1, `src-tauri/src/lib.rs:170` (dialog title): "Drop could not start"

## Next
W1-6: bug reports attach rotated log archives too (`collect_bug_report_log_file`; live file is `drop.log`, rotated archives are `drop.{N}.log`).
