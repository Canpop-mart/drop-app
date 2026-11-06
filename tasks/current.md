# Task: Achievement tracking core rebuild

## Goal and authorization
Owner approved the app/server core rebuild on 2026-11-05. A separate agent is changing server metadata,
imports, services and recomp updates; preserve those changes. No commits, pushes, release or deployment.
Managed emulator replacements, graphics hooks and game binary/config replacement remain separate gates.

## Maps used
- `docs/map/achievements.md`, `docs/map/local-db-settings.md`, `docs/map/big-picture.md`.
- `../drop-server/docs/map/achievements.md`, `../drop-server/docs/map/database-prisma.md`.
- Design/audit: `docs/achievement-tracking-overhaul.md` (updated to distinguish approved core from follow-on work).

## Completed implementation
- Client `src-tauri/remote/src/achievements.rs` and new achievements journal/tracking/notification modules:
  replace the mutually exclusive, server-dependent poll loop with local capture and independent durable sync.
- JSON journal is scoped to server, user and paired client. Atomic writes precede popup receipt. Stable unlock IDs,
  latest-value progress storage, persistent dispositions, quiet historical imports, corrupt-record preservation,
  reset invalidation and startup recovery. No pot/install/mod-ledger changes.
- `src-tauri/remote/src/goldberg/tracking.rs`: reuse standalone Windows readers, runtime JSON progress,
  native directory watchers plus one-second recovery scans, pre-launch baselines and delayed final reads.
- `src-tauri/remote/src/goldberg/steam.rs`: independent read-only binary KeyValues schema/bitfield parser.
  Bind one Steam installation and account; stop that source visibly if the account changes during play.
  Research reference: https://github.com/redstone-md/gse_moss/blob/dev/tools/steam_stats_converter/README.md
  No external application or code dependency on Achievement Watcher was installed.
- Launch prepares capture before process spawn. Existing incognito semantics remain. Worker starts at app startup.
- `src-tauri/remote/src/requests.rs`: optional one-request credential binding for queued events. Original server and
  client JWT are pinned before awaits; default request behavior is unchanged. Worker retries independently.
- Desktop and BPM share `main/components/AchievementTrackingStatus.vue`, with sources, read/sync errors, pending
  observations, local unlocks, counters, RA modes and Retry. Cards overlay durable local unlocks independently of
  server success. BPM adds inline reset confirmation with focus-registered actions and no modal/input lock.
- Windows popup prototype: isolated static HTML, nonactivating/click-through native window, opt-in/default false.
  `achievement_game_popups` has serde default and Default initialization; camelCase key is in ALLOWED_SETTINGS_KEYS.
  Same controls/preview exist on desktop/BPM. Native Steam notifications are reused to avoid duplication.
- Server: new v2 per-event ingest, additive receipt/progress/global-reset tables, provider-scoped config negotiation,
  effective global/per-game epochs, optional epoch race guard in existing unique/reset-aware unlock writer.
  Existing IDs and legacy endpoint remain. Local RA evidence cannot award official credit; service sync records
  separate confirmed softcore/hardcore modes. Server invalid-body text reuses existing localized errors.invalidBody.
- Own server notes: `../drop-server/tasks/achievement-core-rebuild.md`. Other agent's shared notes were preserved.

## Validation completed
- Pinned Windows `cargo +nightly-2026-04-20 check --all-targets`: exit=0, Finished dev profile.
- `cargo +nightly-2026-04-20 test -p remote achievements:: --lib`: 7 passed, exit=0 (includes two existing Goldberg tests).
- `cargo +nightly-2026-04-20 test -p remote goldberg:: --lib`: 9 passed, exit=0. Includes actual filesystem watcher
  late-directory/replacement test, native Steam bitfield/account fixtures and existing prefix/reset regressions.
  Total unique tests across the two filters: 14, with two tests appearing in both filters.
- An extra requests:: filter matched 0 tests. It is NOT counted as verification; requests.rs has no unit tests.
- Main Nuxt typecheck: exit=0. Explicit achievement-status and achievement-tracking frontend files: 13 passed, exit=0.
- Server Prisma client generation: exit=0, generated 7.3.0 client. No database-connected command was run.
- Schema-only Prisma migrate diff vs handwritten SQL: all 12 normalized statements match, exit=0. Corrected a long
  PostgreSQL index identifier to exactly match Prisma's truncation. Temporary schema comparison artifacts removed.
- Eight pure server contract assertions passed without adding a test runner (times, modes, progress and batch limit).
- Server Nuxt typecheck: exit=0; focused changed achievement TypeScript ESLint: exit=0; Prettier: all files pass, exit=0.
- New frontend/helper/asset formatting passes. Rust modules formatted with pinned rustfmt, explicit file list only.
- Client docs: 25 docs / 788 paths / 0 missing, exit=0. Server docs: 17 docs / 471 paths / 0 missing, exit=0.
- Scoped diff whitespace check: exit=0. Initial failed checks were fixed; no broad formatter or clippy fix was run.

## Not checked / rollout requirements
- Compiles, untested on device. No live game, controller, popup visibility/sound/focus, Linux/Deck build or latency run.
- Native Steam cache support is fixture-tested, not verified against an installed game. Formats can change.
- No PostgreSQL migration rehearsal, live database access, multi-device DB race test or HTTP auth-switch integration test.
- RA still syncs through its website at a bounded cadence; native emulator popups are immediate. A managed event bridge
  is not implemented or authorized for distribution. Other storefronts and graphics hooks are also separate work.
- Windowed/borderless popup prototype needs device validation; exclusive fullscreen and Gamescope transport remain open.
- No claim of universal achievement support. A standalone build must expose readable data; unknown formats are visible.
- Server receipts retain event history; retention and resumable cross-device change delivery remain follow-on work.
- Server-first rollout is required. OWNER applies additive migration on dev before running the new server/client,
  coordinating the other agent's pending migration work. Agent did not run migrate deploy/dev/reset or db commands.

## New player-facing strings
See `tasks/achievement-core-strings.md` for the complete file:line inventory of 91 display/diagnostic literals,
including shared status/reset/popup controls, source diagnostics and recovered-journal errors. Source/protocol names
are technical diagnostics; known failure reasons map to plain text in the new helper. No server locale edits.

## Next
Review the uncommitted app/achievement server diff. Owner rehearses the additive migration on dev, then validate a
representative native Steam, GBE/SSE/alternate Windows-format and RetroArch game. Test the popup preview with the game
before enabling it. Measure actual events and visible popup timing, preserve real progress, and review any emulator
bridge/fullscreen hook as a separate concrete change. Do not deploy or commit automatically.

## Preserved previous work
Prior BPM/release task notes remain in `tasks/previous-bpm-before-achievement-audit-2026-11-05.md`.

## Admin game editor redesign (2026-11-05)
Server-only Details/Media/Versions/Connections redesign is complete locally. Single image workspace, optional connections and explicitly typed game fetch (fixes owner-reported route-inference stack errors). Final typecheck, scoped lint/format, 11 pure cases and isolated desktop/mobile browser checks passed. Real DB-backed app/device validation pending. No client runtime/UI strings, schema migration, commit or deployment. Full files/checks/string inventory: ../drop-server/tasks/current.md.


## Admin panel audit and consolidation (2026-11-06)
Owner requested an audit of every server admin tab and implementation of a simpler organization. Work is in
`../drop-server`: eight main workflow destinations, contextual navigation, merged devices/pairing history,
merged services/provider diagnostics, visible load/action errors, collection/settings/token save fixes,
request/achievement/history race guards and tag-create ACL correction. Owner-reported metadata fetch type
recursion is addressed with an explicit admin resource helper and bounded external HTTP request types.
Audit: `../drop-server/docs/admin-panel-audit.md`. Current checks and string inventory:
`../drop-server/tasks/current.md`. Prior client achievement notes preserved above. No native desktop/BPM
UI change, saved format, migration, commit or deployment. Real database-backed integration remains pending.
