# Library / Game-State / Local-Database Audit — 2026

Scope: the Tauri desktop client's library, game-status and local-database
layer — `src-tauri/games/src/{library,scan,state,status}.rs`, the
`src-tauri/database/` crate, the `src-tauri/src/{games,collections,scheduler,
updates}.rs` command layer, and `main/composables/game.ts`.

Triggered as the follow-up to the download-pipeline audit: that work added
post-install validation which now **demotes a failed install to
`PartiallyInstalled`**. This audit makes `PartiallyInstalled` a coherent,
well-handled state everywhere and hardens the surrounding state machine, the
DB schema and the frontend cache.

## The game-status model

A game's status has **two layers**:

- **Persistent** — `GameDownloadStatus`, written to disk: `Remote {}` or
  `Installed { install_type, .. }` where `install_type` is `Installed`,
  `SetupRequired` or `PartiallyInstalled`.
- **Transient** — `ApplicationTransientStatus`, `#[serde(skip)]`, never
  persisted: `Queued`, `Downloading`, `Validating`, `Updating`,
  `Uninstalling`, `Running`. When present it masks the persistent layer.

Key consequence: **transient state cannot get "stuck"**. An app crash mid-
`Validating` / `Downloading` / `Updating` loses the transient map on the next
launch and the game falls back to its persistent status. The only thing a
crash *can* leave inconsistent is the persistent status vs. the actual files
on disk — that is what startup reconciliation repairs.

## Findings

| #  | Area | Status before | Action |
|----|------|---------------|--------|
| 1  | **State transitions** | Scattered raw writes across ~6 call sites, no legal-transition definition | **FIXED** — centralised in `status.rs` |
| 2  | **Stuck-state recovery** | Ad-hoc "missing games" sweep only; a half-finished uninstall stayed `Installed` | **FIXED** — `reconcile_on_startup` |
| 3  | **`scan.rs` install detection** | Imported **every** scanned dir as `PartiallyInstalled` (over-conservative) | **FIXED** — checks `.dropdata` chunk flags; complete → `Installed`, incomplete → `PartiallyInstalled` |
| 4  | **DB migrations** | Single-variant version envelope, **no migration steps** | **FIXED** — `migrations.rs` with `SCHEMA_VERSION` + ordered step extension point |
| 5  | **DB error handling** | `anyhow::Error` everywhere; a `// TODO` admitting errors were assumed-Deserialize | **FIXED** — one `DatabaseError` enum |
| 6  | **`game.ts` registry** | Module-level cache, never invalidated; listener leak risk on re-fetch | **FIXED** — in-place refresh on `update_library`, tracked unlisteners |
| 7  | **`DropData::write` swallowed error** | `Err(_) => return` silent | **FIXED** — now logged |
| 8  | **`serde` derive feature** | Missing from `database/Cargo.toml`; only present via workspace unification | **FIXED** — added explicitly |
| 9  | Collections sync | Server is source of truth, cached + offline-filtered | Confirmed — no change |
| 10 | Update checking (`updates.rs`) | Polls every 30 min, sets `update_available` | Reviewed — see Deferred |
| 11 | Scheduler | Single task, error-tolerant, survives failures | Confirmed — no change |

## Changes

### 1. Centralised state machine — `games/src/status.rs` (new)

- `StatusKind` — a flat, comparable view of the `(persistent, transient)`
  pair, with `from_state` / `from_persistent` / `from_transient`.
- `is_valid_transition(from, to)` — the legal transition table (e.g.
  `Remote -> Downloading` legal, `Remote -> Running` not).
- `transition()` / `transition_from_db()` — log every status change under a
  single greppable tag `[game-status]`, flagging illegal ones loudly. They
  log only — a panic here would be worse than a mislabelled status.
- `library.rs` now routes `set_partially_installed_db`, `uninstall_game_logic`
  and `on_game_complete` through `transition_from_db`.

### 2. Startup reconciliation — `reconcile_on_startup`

Runs once early in `setup()` (`src/lib.rs`), **before** the disk scan. For
every game marked `Installed`:

1. **Install dir missing** → demote to `Remote {}` (also drops the stale
   `installed_game_version` entry).
2. **Dir present but `.dropdata` missing** → demote to `PartiallyInstalled` —
   a `.dropdata`-less directory can't be validated or resumed, and a crash
   mid-uninstall that deleted files but not the folder lands here. The user
   can then resume/repair instead of launching a broken game.

Leftover transient statuses are cleared defensively. The old ad-hoc
missing-games sweep in `setup()` was removed (superseded).

**Acceptance:** an app crash mid-`Validating` reconciles to a sane state —
the transient `Validating` is `#[serde(skip)]` so it's gone on reload, and
the persistent status (still `PartiallyInstalled` from the download agent's
demotion, or `Installed` if validation had completed) is cross-checked
against disk by reconciliation.

### 3. `scan.rs` manifest cross-check

The scan has no server manifest offline, but `.dropdata` carries the
downloader's per-chunk completion flags (`contexts`). New policy:

- `.dropdata` present, `contexts` non-empty and **all flags `true`** → import
  as `InstalledGameType::Installed`.
- `.dropdata` present but `contexts` empty or **any flag `false`** → import
  as `PartiallyInstalled`.

Previously *every* scanned directory was imported as `PartiallyInstalled`,
which demoted healthy installs on every launch.

### 4. Versioned DB migrations — `database/src/migrations.rs` (new)

- `SCHEMA_VERSION` constant (currently `1`).
- `VersionedDatabase` / `VersionedDatabaseRef` — the on-disk tagged envelope
  (`V1 { database }`), replacing the old `DatabaseVersionEnum` in `models.rs`.
- `migrate_to_latest()` — validates the version and walks the envelope
  forward. Today an identity step; the `match` is the documented extension
  point (the module header has a full worked `v1 -> v2` example).
- `interface.rs` `open_at_path` / `write_to_path` go through this. A future
  schema change can no longer break an existing client's DB.

### 5. One `DatabaseError` type — `database/src/error.rs` (new)

`Io`, `InvalidUtf8`, `Deserialize`, `Serialize`, `UnsupportedVersion`,
`MigrationFailed` — with `From` conversions. `interface.rs` now returns
`DatabaseResult<T>`. `handle_invalid_database` distinguishes a corrupt
payload (back up + start fresh) from an I/O fault in its logging.

### 6. `game.ts` registry invalidation

- `invalidateGame(id)` — **re-fetches and mutates the existing `game`
  object / `version` ref / `status` ref in place**. In-place (not delete)
  because consumers hold the refs returned by `useGame` for a component's
  lifetime; deleting would detach them and orphan the `update_game/{id}`
  listener.
- `invalidateAllGames()` — wired to the `update_library` backend event
  (emitted by `uninstall_game_logic`), registered once at module load.
- The cached `Game` is now `reactive()` so in-place `Object.assign` updates
  propagate to templates.
- `update_game/{id}` listener handles are tracked in `gameStatusUnlisteners`
  and torn down by `evictGame` (used when a refresh is impossible), closing
  a listener leak.

**Acceptance:** `game.ts` no longer serves stale status after an
install/uninstall — `update_library` triggers an in-place refresh and the
per-game `update_game/{id}` event keeps status live during normal operation.

## Verification

- `cargo check -p database` — clean (pre-existing `nonpoison_rwlock` feature
  warning only).
- `cargo check -p games` — clean (4 pre-existing warnings in
  `download_logic.rs` / `lib.rs` feature flags, none in changed files).
- `cargo check` (root `drop-app`) — compiles; no errors, no new warnings
  attributable to this change.
- `pnpm typecheck` in `main/` — exit 0.

## Deferred items

- **Update-check reliability (`updates.rs`).** The poll works structurally
  (every 30 min it compares the latest server version id to the installed
  one and sets `update_available`). It was **not runtime-verified** in this
  audit — no live server. It also only flips `update_available` *on*; if a
  game is updated through Drop the flag is reset by `on_game_complete`, but
  if the server *retracts* a version the flag is never cleared. Low-risk;
  worth a dedicated check.
- **`models.rs` decomposition.** At ~510 lines it is large but cohesive — it
  is the single `v1` schema module and a clean domain split (settings vs.
  game-version vs. status) would fragment the versioned-schema boundary that
  `migrations.rs` depends on. **Intentionally not split.** The envelope code
  (~45 lines) was removed from it into `migrations.rs`, which is the
  meaningful seam.
- **`Platform::from<&str>` panics** on an unknown platform string
  (`platform.rs`). Out of this scope but flagged — a malformed `.dropdata`
  could crash the scan. Pre-existing.
- **N+1 / query patterns.** The DB is an in-memory `HashMap` graph behind an
  `RwLock`, not SQL — there are no queries to N+1. `fetch_library_logic`
  already uses a `HashSet` for the installed-vs-library diff. No change
  needed.
