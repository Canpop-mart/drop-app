# Process-Management Layer Audit — 2026

Scope: the Tauri desktop client's process-management crate —
`src-tauri/process/`. This crate launches game processes, tracks them while
they run, terminates them, and synchronises cloud saves around a session. The
audit also covers the crate's interaction with the centralised game-status
state machine (`games/src/status.rs`) and the gamepad input module
(`process/src/gamepad.rs`).

## Summary

| # | Area | Status before | Action |
|---|------|---------------|--------|
| 1 | **`process_manager.rs` monolith** | 2293-line single file | **DECOMPOSED** — 7 cohesive modules |
| 2 | **Launch/exit status writes** | Ad-hoc `transient_statuses` mutation | **FIXED** — routed through `status.rs` |
| 3 | **Kill** | Killed only the spawned process | **FIXED** — terminates the whole process tree |
| 4 | **`error.rs`** | `ProcessError` with dead variants, stringly typed | **REWRITTEN** — clean enum, documented |
| 5 | Exit detection | Working (`wait()` thread) | Confirmed — no change |
| 6 | Startup reconciliation | Partial | Improved — `reconcile_running_processes` added |
| 7 | **`gamepad.rs`** | Dormant dead code, misleading docs | **CLARIFIED** — see §6, scroll-to-top traced |

## 1. `process_manager.rs` decomposition

`process_manager.rs` was a single 2293-line file holding the entire game
lifecycle. It is now `process_manager/` — a module directory. Each module owns
one phase, and the split lines up with the audit boundaries (security-sensitive
env handling, the kill path, save sync) so each is independently reviewable.

| Module | Lines | Owns |
|--------|-------|------|
| `mod.rs` | 332 | The `ProcessManager` struct + global wrapper, the `ProcessHandler` trait, handler selection (`fetch_process_handler`), `kill_game`, `is_game_running`, `get_launch_options`, and `reconcile_running_processes` (startup safety net). |
| `launch.rs` | 874 | The launch flow — `launch_process` / `launch_process_streaming` and the shared `launch_process_inner` walking numbered steps 1–8, `build_command` (env + RetroArch + Goldberg assembly into a spawnable `Command`), `register_running_process` (playtime/achievements/status/wait-thread), and pre-launch save-sync dispatch. |
| `launch_emulator.rs` | 308 | Emulator launch-command construction (`build_emulator_command` — ROM `{rom}` substitution, RetroArch core auto-resolution) and the Linux ENOEXEC bash-wrapper spawn retry (`spawn_with_enoexec_retry`). Split out because an emulator launch is a meaningfully different path and the ENOEXEC retry is a rare fallback. |
| `env.rs` | 333 | Launch-environment construction and hardening. Holds the `FORBIDDEN_ENV_KEYS` denylist (`LD_PRELOAD`, `LD_AUDIT`, `PATH`, `PYTHONPATH`, …) and `is_env_key_forbidden`, plus AppImage env scrubbing, Gamescope/Steam-Deck display vars, and MangoHud configuration. Isolated because env handling is the crate's main injection-surface and must be auditable on its own. |
| `exit.rs` | 332 | Process-exit handling — `on_process_finish` (the back half of the lifecycle), playtime-stop / session-end reporting, post-exit save upload, the playtime heartbeat loop, and suspicious-exit detection. |
| `kill.rs` | 96 | Process-tree termination — `kill_process_tree` (see §3). |
| `save_sync.rs` | 358 | Pre-launch cloud-save synchronisation — `sync_emulator_saves` / `sync_pc_saves`, the conflict-resolution dance (emit event → block on an mpsc channel → apply choices), and the network timeouts that bound it. Split out purely for size: the conflict logic was drowning the spawn logic in `launch.rs`. |

`process_handlers.rs` (the platform `ProcessHandler` implementations —
`WindowsLauncher`, `NativeLauncher`, `UMUCompatLauncher`, …) was left in place;
its only change was dropping a now-unused `debug` import.

Behaviour is unchanged by the decomposition — it is a pure structural move. The
`mod.rs` doc comment records the lifecycle invariants that the split must
preserve (a game is in `self.processes` *iff* its transient status is
`Running`; exit detection is a blocking `wait()` per game; the process table
lives behind the global `PROCESS_MANAGER` mutex).

## 2. Status-machine integration (launch → Running, exit → Installed)

Before this audit the process crate wrote game status by hand: launch inserted
`ApplicationTransientStatus::Running` straight into the DB map, and exit simply
`remove`d it. There was no logged, validated transition — a launch and an exit
were invisible to the central state machine introduced in
`library-state-db-2026.md`.

Both ends now route through `games/src/status.rs`:

- **Launch → `Running`.** `launch::register_running_process` calls
  `transition_from_db(&db, &game_id, StatusKind::Running)` (which looks up the
  game's current state and logs the `Installed/SetupRequired -> Running`
  transition through `is_valid_transition`) *before* writing the transient
  `Running` status that masks the persistent one in the UI.
- **Exit → `Installed`.** `exit::on_process_finish` reads the live state via
  `GameStatusManager::fetch_state`, calls `transition(&game_id, from,
  StatusKind::Installed)`, then drops the transient `Running` entry — which is
  what actually moves the game back to its persistent status in the UI.
  `is_valid_transition` already whitelists `(Running, Installed | SetupRequired)`.
- A clean exit from a `SetupRequired` install is also detected here and
  promotes `install_type` to `Installed` (setup completed successfully).

Every launch, exit, crash and kill now produces a single greppable
`[game-status]` log line and can never silently leave a game `Running`.

**Startup safety net.** `ProcessManager::reconcile_running_processes` (in
`mod.rs`) is new. The transient-status map is `#[serde(skip)]` and
`self.processes` is process-local, so a crash already drops a game's `Running`
state on the next launch — but this is the belt-and-braces check: at startup,
*any* transient `Running` is by definition stale (its process died with the
previous app instance) and is corrected `Running -> Installed` through
`transition()`. It complements `status::reconcile_on_startup`, which repairs
the persistent layer against the filesystem.

## 3. The kill fix — child-process termination

**Before:** `kill_game` called `SharedChild::kill()` on the single process
Drop spawned. Games are almost never one process: a Proton/Wine title runs as
`bash → umu-run (python) → proton → wine → game.exe`, and a Windows title is
frequently `launcher.exe → game.exe` (Unity/Unreal store launchers, EAC/BE
anti-cheat bootstrappers). Killing only the spawned root left the real game —
and the GPU/audio resources it held — orphaned and still running.

**After:** `kill.rs::kill_process_tree` terminates the *entire* tree:

- **Linux** — the child is placed in its own process group via `setsid()` in a
  `pre_exec` hook at spawn time (`launch.rs` step 8). `kill(-pgid, …)` then
  signals every descendant at once: `SIGTERM` first for a clean Wine flush,
  then `SIGKILL` after a 500 ms grace period, escalated on a detached thread.
- **Windows** — `taskkill /PID <pid> /T /F`. `/T` walks and kills the PID's
  child tree; `/F` forces it. If `taskkill` exits non-zero (the tree already
  died, or no children exist) it falls back to a direct `SharedChild::kill()`.
- **macOS** — direct `child.kill()` (process-group teardown not wired here).

Killing is **fire-and-forget**: `kill_game` marks `manually_killed = true` (so
the exit path reports a user kill rather than a crash), dispatches the signals,
and returns immediately. The per-launch `wait()` thread observes the real exit
and runs the `exit.rs` cleanup, including the `Running -> Installed`
transition. Blocking the caller would freeze the UI for the 10+ seconds a
Proton/Wine teardown can take.

## 4. The `error.rs` rewrite

`ProcessError` was rewritten as a clean, fully documented enum:

- **Dead variants removed.** `InvalidID` and `FailedLaunch(String)` were no
  longer constructed anywhere and were deleted (along with their `Display`
  arms).
- **Every variant documented.** Each carries a doc comment explaining when it
  occurs and, where relevant, why it carries a payload.
- **Stringly-typed concern addressed in docs.** The remaining `String`-carrying
  variants (`FormatError`, `InvalidArguments`, `RequiredDependency`,
  `NeedsCompat`) are *not* stringly-typed error handling — they wrap a foreign
  error with no typed form (`dynfmt` format errors, `shell_words` parse errors)
  or carry an identifier the user-facing message needs (the offending command,
  a dependency's `(game_id, version_id)`). The enum-level doc states this
  explicitly so a future reader does not "fix" it.
- `ProcessError` still derives `SerializeDisplay`, so each variant's `Display`
  text is what the frontend shows — the comments note that this text must stay
  actionable.

## 5. Exit detection — confirmed correct, no change

Exit detection is a blocking `SharedChild::wait()` on a dedicated thread spawned
per launch (`launch.rs::register_running_process`). `wait()` cannot miss a real
exit, and when the whole process tree is gone it hands off to
`on_process_finish`. `on_process_finish` is idempotent against a double-fire
(kill cleanup then wait-thread fire): if the game id is already out of the
process table it logs and returns. The post-exit network work (playtime stop,
session-end notify, save upload) is spawned onto the Tauri async runtime so a
slow network never blocks process cleanup or the UI. No change needed.

## 6. `gamepad.rs` audit

### 6.0 Headline finding — `gamepad.rs` is dormant dead code

`process/src/gamepad.rs` is **not wired into the running application.** Its
entry point `start_polling` has exactly one call site — in `src-tauri/src/lib.rs`
— and that line is **commented out** (since the `feat: Steam Deck native
support` commit):

```rust
// Gamepad input is now handled via the Web Gamepad API in the Vue
// frontend (composables/gamepad.ts) because gilrs's WGI backend on
// Windows intermittently fails to deliver input for HID controllers.
// ::process::gamepad::start_polling(handle.clone());
```

Live controller input for Big Picture Mode is handled **entirely in the Vue
layer** by `main/composables/gamepad.ts`, an independent implementation built
on the browser Web Gamepad API (`navigator.getGamepads()`). Nothing in the Vue
layer subscribes to the `gamepad_button` / `gamepad_axis` /
`gamepad_connected` / `gamepad_disconnected` Tauri events that `gamepad.rs`
emits (confirmed by a repo-wide search — the only file mentioning those event
names is `gamepad.rs` itself). The Rust module, if ever started, would emit
into the void.

The module produces no dead-code compiler warning because `pub mod gamepad`
and `pub fn start_polling` are public crate API — the compiler cannot prove
no external caller exists.

It is retained as a ready fallback (e.g. should background input while the
webview is unfocused ever be needed), so it was **not deleted**. Instead its
module documentation was corrected: the previous doc claimed it "emits
normalised Tauri events to the Vue frontend", which is false and actively
misled an earlier pass. The doc now states the module is dormant and that the
live input path is `gamepad.ts`.

### 6.1 Code review of `gamepad.rs` (as a dormant-but-correct module)

Reviewed so the module is sound if it is ever re-enabled:

- **Hot-plug — clean.** `poll_loop` keeps a `known_connected: HashMap` and
  diffs `gamepad.is_connected()` each frame, emitting `gamepad_connected` /
  `gamepad_disconnected` on edges. A prior-pass fix (kept) clears the
  disconnected controller's `prev_buttons` / `prev_axes` diff state on
  disconnect — without it a reconnect (gilrs reuses the `GamepadId`) would
  inherit a stale "pressed" button (suppressing the first real press) or a
  stale axis value (suppressing the first real movement). Correct.
- **Polling efficiency — no busy-loop, no leaked threads.** The loop sleeps
  16 ms (~60 Hz) each iteration. A single thread is spawned, guarded by a
  `RUNNING: AtomicBool` with a `compare_exchange` so `start_polling` cannot
  spawn a second. `stop_polling` clears the flag and the loop exits. No leak.
- **Event leaks / stacking — none.** Buttons emit only on a press/release edge
  (`pressed != was_pressed`). Axes emit on a change exceeding
  `AXIS_CHANGE_THRESHOLD` (a delta filter) or on a periodic heartbeat (below).
  Events cannot pile up or fire repeatedly for a held input.
- **`AXIS_HEARTBEAT_FRAMES` (prior-pass addition, kept).** A pure delta filter
  has a stale-cache hazard for any consumer that caches the last value seen: if
  a stick settles such that the final step toward rest is below the threshold,
  the last non-zero value is never superseded. The heartbeat re-emits every
  controller's current axis values every 30 frames (~500 ms), bounding that
  staleness. This is correct defensive hardening **for this module's own
  consumers** — its doc comment was corrected because the prior pass wrongly
  claimed it fixes the live BPM scroll-to-top bug, which it cannot (the module
  is dormant).

### 6.2 The Big Picture Mode "scroll-to-top" bug — investigation & verdict

**Report:** "navigating Big Picture Mode with the control stick randomly
scrolls to the top after a couple seconds."

**Verdict: the cause is in the Vue layer, not `gamepad.rs`.** No fix was made
to Rust code for this bug — the bug cannot originate in a module that never
runs. The fix belongs to a separate Vue audit; this section documents the
cause precisely so that audit can act on it directly.

**Exact mechanism (in `main/composables/gamepad.ts`):**

`gamepad.ts`'s `pollFrame()` loop (≈ lines 237–249) is a delta filter with no
heartbeat. It updates its `axes` reactive map **only** when
`Math.abs(filtered - prev) >= AXIS_CHANGE_THRESHOLD` (`AXIS_CHANGE_THRESHOLD`
= 0.05; `applyDeadZone` zeroes anything under `STICK_DEAD_ZONE` = 0.15). The
two scroll consumers — `focus-navigation.ts::startStickPolling` (≈ line 1258,
native pages) and `iframe-controller.ts` (≈ line 101, `server://` iframe
pages) — both poll `gamepad.axisValue("RightStickY")` every 50 ms and scroll
while `Math.abs(val) > 0.3`.

The failure: when the stick is released from a deflection and decelerates back
toward centre, the Web Gamepad API can report a sample sequence whose steps
near rest fall **below** the 0.05 delta threshold and are therefore dropped.
If the last *recorded* value before the drops is still ≥ 0.3 (e.g. the stick
decelerates through `-0.31`, the next sample `-0.27` is a 0.04 step → dropped,
and the stick then settles on drift in the `-0.27 .. -0.31` band), the `axes`
map stays **pinned at that ≥ 0.3 value forever**. `axisValue("RightStickY")`
keeps returning it, so the consumer's 50 ms poll keeps issuing `scrollBy` with
no real input. "After a couple seconds" is simply the time for the page's
scroll to run to `top: 0`. A worn / drifting stick makes it more frequent;
"random" reflects how often a release happens to land in the lossy band.

`gamepad.ts` has the *exact* delta-filter-without-heartbeat flaw that
`gamepad.rs`'s `AXIS_HEARTBEAT_FRAMES` defends against — but `gamepad.ts` has
**no equivalent heartbeat**.

**Recommended fix (Vue audit — deferred, not done here):** in `gamepad.ts`,
mirror the Rust heartbeat — periodically re-publish each axis's current value
to the `axes` map even when the delta is sub-threshold; or have `pollFrame`
always write the dead-zoned value when it is `0` (so a settled stick reliably
reads `0`); or make the consumers read a fresh `navigator.getGamepads()` value
rather than the cached map. Any of the three breaks the stale-cache pin.

## Changes made

| File | Change |
|------|--------|
| `process/src/process_manager.rs` | **Deleted** — replaced by the `process_manager/` directory. |
| `process/src/process_manager/{mod,launch,launch_emulator,env,exit,kill,save_sync}.rs` | **New.** Decomposition of the 2293-line monolith (§1). |
| `process/src/error.rs` | `ProcessError` rewritten — dead variants removed, every variant documented (§4). |
| `process/src/process_handlers.rs` | Dropped an unused `debug` import. |
| `process/src/gamepad.rs` | Module + `AXIS_HEARTBEAT_FRAMES` docs corrected to state the module is dormant and is not the source of the BPM scroll bug; disconnect-state cleanup and the heartbeat (prior-pass changes) reviewed and kept (§6). |
| `games/src/status.rs` | Consumed by the process crate for launch/exit transitions (authored under `library-state-db-2026.md`; no change in this audit). |

## Verification

`cargo check -p process` passes. Only pre-existing unused-`#![feature]`
warnings remain (`vec_try_remove` in `process/src/lib.rs`, and unrelated
warnings in the `games` dependency) — no new errors or warnings introduced by
this audit.

### Acceptance criteria

- **Monolith decomposed.** `process_manager.rs` (2293 lines) → 7 modules, each
  owning one lifecycle phase; behaviour unchanged.
- **Launch/exit go through the state machine.** `register_running_process` and
  `on_process_finish` call `transition` / `transition_from_db`; every session
  start and end is a logged `[game-status]` line.
- **Kill terminates the whole tree.** `kill_process_tree` uses process-group
  `kill` (Linux) / `taskkill /T` (Windows); a launcher's child `game.exe` is
  no longer orphaned.
- **`gamepad.rs` audit complete.** Hot-plug, polling and event-leak review done
  (§6.1); the scroll-to-top bug is precisely diagnosed to `gamepad.ts` in the
  Vue layer (§6.2).

## Deferred / flagged for follow-up

- **Scroll-to-top fix in `gamepad.ts` (Vue).** The bug's root cause is fully
  documented in §6.2; the fix is a small change to `main/composables/gamepad.ts`
  (add an axis heartbeat, or always publish a zeroed axis value). It was not
  applied here because the Vue layer is a separate audit. **This is the
  user-visible bug behind this audit's gamepad scope — it should be the first
  item the Vue audit picks up.**
- **`gamepad.rs` long-term disposition.** The module is dormant fallback code.
  It should either be wired back in behind a feature flag with a frontend
  listener, or removed outright once the Web Gamepad API path is considered
  permanent. Leaving an unreferenced `pub` module indefinitely invites another
  reader to "fix" input bugs in the wrong file (as already happened once).
- **macOS process-tree kill.** `kill_process_tree` on macOS does a plain
  `child.kill()` — no process-group teardown. A macOS game launched via a
  wrapper could orphan children. Low priority (macOS is the least-used target,
  and native Mac games rarely spawn launchers) but noted for completeness.
- **`launch.rs` size.** At 874 lines `launch.rs` remains the largest module.
  It is cohesive (one numbered launch flow) and further splitting would be
  churn without payoff, but if it grows, `build_command` (env + RetroArch +
  Goldberg assembly) is the natural next seam to extract.

## Out of scope (unchanged)

The download/install pipeline (`download-pipeline-2026.md`), the game-status
state machine and DB layer (`library-state-db-2026.md`), compatibility / Proton
testing (`compat-2026.md`), the Vue Big Picture Mode UI, and
`main/composables/gamepad.ts` itself (its fix is deferred to a Vue audit, per
above).
