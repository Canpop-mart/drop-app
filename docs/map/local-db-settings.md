# Local database, data directory and settings

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
The client keeps one local database file, `drop.db`: server URL and auth, settings, install records, per-version
launch configs and user configuration, Proton choices and the pending download queue. It is RON text wrapped in a
versioned envelope and encrypted with AES-128-CTR. Every write rewrites the whole file. Settings pages on both
surfaces read it with `fetch_settings` and write it with `update_settings`. Small UI preferences (themes, zoom,
BPM toggles) live in browser `localStorage`, not in the DB.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/database/src/db.rs` | `DB` (lazy `DatabaseInterface`), `DATA_ROOT_DIR` (`drop` or `drop-debug`), `load_or_generate_key` (`db.key` file first, then keyring, then new), `KEY_IV` |
| `src-tauri/database/src/interface.rs` | `set_up_database` (creates dirs, opens or creates), `open_at_path` (decrypt, `ron::from_str`, migrate, `seed_installs_from_legacy`), `write_to_path` (temp file + rename), `handle_invalid_database`, `DBRead` / `DBWrite`, `borrow_db_checked`, `borrow_db_mut_checked` |
| `src-tauri/database/src/models.rs` | `mod data` type aliases (all point at `v1`), `v1::Database`, `Settings`, `DatabaseApplications` (`game_statuses`, `game_versions`, `installed_game_version`, `installs`, `transient_statuses`, `pending_queue`, Proton paths), `UserConfiguration`, `GameVersion`, `LaunchConfiguration`, `default_template` |
| `src-tauri/database/src/migrations.rs` | `SCHEMA_VERSION` (1), `VersionedDatabase::V1`, `VersionedDatabaseRef`, `migrate_to_latest`, worked example for adding V2 |
| `src-tauri/database/src/error.rs` | `DatabaseError` (`Io`, `InvalidUtf8`, `Deserialize`, `Serialize`, `UnsupportedVersion`, `MigrationFailed`) |
| `src-tauri/database/src/platform.rs`, `debug.rs` | `Platform` enum; `SystemData` (returned by `fetch_system_data`) |
| `src-tauri/src/settings.rs` | Commands `update_settings` (allowlist `ALLOWED_SETTINGS_KEYS`), `fetch_settings`, download dir commands (`fetch_download_dir_stats`, `add_download_dir`, `delete_download_dir`, `open_download_dir`), RA credential commands, `fetch_system_data`, bug-report log commands, `open_log_folder`, Defender helpers |
| `src-tauri/client/src/app_state.rs` | `AppState`, `UmuState`, `SessionType::detect` (Gamescope / Deck detection), `should_auto_bpm` |
| `src-tauri/client/src/app_status.rs` | `AppStatus` (`NotConfigured`, `SignedIn`, `SignedInNeedsReauth`, `Offline`, ...) |
| `src-tauri/client/src/autostart.rs`, `user.rs`, `compat.rs` | `sync_autostart_on_startup`; cached `User`; umu-run discovery (`COMPAT_INFO`) |
| `src-tauri/src/client.rs` | `fetch_state` (AppState as JSON string), `quit`, `cleanup_and_exit` |
| `src-tauri/remote/src/cache.rs` | Object/metadata cache: bitcode-encoded, 600-entry memory cache + disk files named by md5 of the key under `Database.cache_dir` |
| `main/pages/settings.vue` | Desktop settings shell and nav list |
| `main/pages/settings/` | Desktop pages: `index.vue` (Home), `interface.vue`, `downloads.vue`, `compat.vue` (Proton), `achievements.vue`, `cloudsaves.vue`, `streaming.vue` (Remote Play), `account.vue`, `developer.vue` |
| `main/pages/bigpicture/settings.vue` | BPM settings, one page with sections (`sections` computed, around line 1516) |
| `main/composables/app-state.ts`, `main/app.vue` | Frontend app state; `app.vue` calls `invoke("fetch_state")` |

## Data directory layout
`DATA_ROOT_DIR` = `dirs::data_dir()` + `drop` (release) or `drop-debug` (any `debug_assertions` build, i.e.
`pnpm tauri dev`). Windows: `%APPDATA%\drop`; Linux and Deck: `~/.local/share/drop`.
- `drop.db` (+ `drop.db.tmp` during a write, `drop.db.backup-<unix>` after a reset), `db.key` (32 bytes, 0600 on Unix)
- `drop.log` and rolled `drop.{N}.log` (see `docs/map/app-shell-protocols.md`), `crash-<unix>.log`
- `games/` (default install dir), `logs/<game_id>/` (per-launch game stdout/stderr), `cache/`, `pfx/<game_id>/`
  (Proton prefixes)
- `addons/`, `certificates/`, `tools/`, `pending-playtime-stops/`, `pending-playtime-records/`

## How it flows
1. First access to `DB` runs `set_up_database`: `create_dir_all` for root, `games`, `logs`, `cache`, `pfx`; open or
   create `drop.db`.
2. Read: `borrow_db_checked()` returns `DBRead` (RwLock read; poison is logged and recovered).
3. Write: `borrow_db_mut_checked()` returns `DBWrite`; its `Drop` serialises the whole DB, encrypts and writes it
   before releasing the lock. A failed write only logs `database failed to save`.
4. Settings: page -> `invoke("update_settings", { newSettings: { camelKey: value } })` -> each key checked against
   `ALLOWED_SETTINGS_KEYS` and size limits -> merged into `serde_json` of current `Settings` -> deserialised back ->
   stored -> saved on `DBWrite` drop. `fetch_settings` returns the full `Settings`.
5. Load failure: `handle_invalid_database` resets only for `InvalidUtf8`, `Deserialize`, `UnsupportedVersion`,
   `MigrationFailed`: renames the file to `drop.db.backup-<unix>`, deletes and recreates `cache/`, creates a fresh DB
   with `prev_database` set; `lib.rs` setup then shows a "Database corrupted" dialog. `Io` and `Serialize` abort.

## Both surfaces
Desktop (`pages/settings.vue` nav): Home, Interface, Downloads, Proton (only when `umuState !== "NotNeeded"`),
Achievements, Cloud Saves, Remote Play, Account, Developer.
BPM (`pages/bigpicture/settings.vue` sections): Theme, Interface, Controller, Storage, Compatibility (only when
`platform() === "linux"`), Achievements, Cloud Saves, Remote Play, Developer, About.
Gaps: BPM has no Account section (no sign-out text found in `pages/bigpicture` or `components/bigpicture`).
Desktop has no Controller or Theme page; BPM theme and many BPM prefs are `localStorage` (`composables/bp-theme.ts`,
`composables/console-sections.ts`, ...).

## Server side / client side
No server counterpart; account settings proper live on drop-server web pages.

## Rules and gotchas
- The DB is RON, not `pot`.
- `pot` files (`.dropdata`, mod ledgers): `#[serde(default)]` alone does NOT make a change safe
  (`games/src/downloads/drop_data.rs` ~33-75). What breaks them is a reader skipping an UNKNOWN field whose value is a
  unit enum variant or a `deserialize_any` type (e.g. `AspectRatio`). So: never remove or rename a field (breaks every
  existing file); adding a field breaks OLDER builds reading newer files (a downgrade, or `main` reading a `dev`
  file). Prefer a new versioned reader, as `drop_data.rs` does for v0/v1. `ModData` has no fallback reader at all.
  Note `mod_data.rs` ~84-88 still claims pot can't round-trip `#[serde(default)]` fields; `drop_data.rs` contradicts it.
- A new non-optional field on any struct inside `Database` without `#[serde(default)]` makes every existing
  `drop.db` fail with `DatabaseError::Deserialize`; `handle_invalid_database` then backs it up and starts empty
  (all install records gone from the UI). Most `Settings` and `UserConfiguration` fields already carry
  `#[serde(default)]` (`models.rs`).
- Do not change a `vN` module's on-disk shape; add `V2` per `migrations.rs` docs and bump `SCHEMA_VERSION`.
- `transient_statuses` and `prev_database` are `#[serde(skip)]`: never persisted, so `Running` cannot survive a restart.
- A new setting key must be added to `ALLOWED_SETTINGS_KEYS` (`settings.rs` line 192). Otherwise `update_settings`
  drops it with only `error!("Rejecting unknown settings key")`; the command returns `()` so the page cannot tell.
  `maxDownloadThreads` outside 1..=64 is also silently rejected. Worse: if ANY value in the patch has the wrong
  type, `serde_json::from_value` fails and the WHOLE patch is discarded with only `error!("Could not parse settings...")`
  (`settings.rs` ~256-262).
- Holding a `DBWrite` costs a full encrypt + file write on drop. Keep mutable borrows short and never hold one across
  an `.await` or while taking `PROCESS_MANAGER`.
- `DB` is a `std::sync::RwLock` (`database/src/interface.rs:24`): calling `borrow_db_checked()` (or any helper that
  does) while holding a `DBWrite` on the same thread deadlocks. Same for `update/commit.rs` `lock_staging()` (plain Mutex).
- `db.key` is canonical: deleting it while keeping `drop.db` makes the DB undecryptable (decrypt yields garbage,
  then `InvalidUtf8`, then a reset) (inferred from `load_or_generate_key` and `open_at_path`).
- Startup panics here: `DATA_ROOT_DIR` `.expect("Failed to get data dir")` (`db.rs` line 22), `write_key_file`
  `.expect` (line 107), `set_up_database` `panic!` on `create_dir_all` and `.expect` on create/recover
  (`interface.rs` lines 36-93). An `Io` error reading `drop.db` panics (`.expect("failed to recover from failed database")`, `interface.rs` ~86),
  which the player sees as "the app opens and closes instantly".

## Changing things here
- New setting: field on `v1::Settings` with `#[serde(default)]` (or `default = "fn"`) in `models.rs`, add its camelCase
  key to `ALLOWED_SETTINGS_KEYS`, then UI in a `main/pages/settings/*.vue` page AND a section of
  `main/pages/bigpicture/settings.vue` (focus-registered on BPM).
- New per-game option: `UserConfiguration` in `models.rs` with `#[serde(default)]`; read it where it applies (launch
  is `process/src/process_manager/launch.rs`).
- New command: `src-tauri/src/settings.rs`, register in `src-tauri/src/lib.rs` `generate_handler!`.
- Schema change that cannot be a defaulted field: follow the V2 recipe at the top of `migrations.rs`.

## Tests
- No tests in `src-tauri/database` or `src-tauri/client`.
- `src-tauri/src/settings.rs` has one test (`nested_and_equal_folders_overlap`):
  `cargo +nightly-2026-04-20 test -p drop-app settings::` (from `src-tauri/`).
- `cargo +nightly-2026-04-20 test -p remote cache::` for the cache.

## Related maps
`docs/map/app-shell-protocols.md`, `docs/map/launch-process.md`, `docs/map/downloads-install.md`, `docs/map/cloud-saves-playtime.md`
