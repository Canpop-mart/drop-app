# App shell: startup, command registration, protocols, logging

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
The `drop-app` crate (`src-tauri/src/`) is the Tauri binary. `run()` installs the panic hook, builds the Tauri app
with its plugins, registers every frontend-callable command in one `generate_handler!` list, registers the
`object://` and `server://` protocols, then in `.setup()` runs `setup()` (logging, managers, DB, reconcile, auth)
before it builds the main window and the `frontend` webview. The `utils` crate holds small shared macros and
helpers used by every other crate.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/main.rs` | `main` -> `drop_app_lib::run()`; `windows_subsystem = "windows"` in release |
| `src-tauri/src/lib.rs` | `run()` (plugins, `generate_handler!` from line 353, `.setup()` from line 547, protocols at lines 840 and 845, tray, close-to-tray), `setup()` (line 104), `custom_panic_handler` (line 285), `recieve_handshake` / `recieve_handshake_logic` (deep link auth), `run_on_tray` (`NO_TRAY_ICON`) |
| `src-tauri/src/*.rs` | Command modules glob-imported into `lib.rs` (`use games::*`, ...): `client`, `compat`, `download_manager`, `downloads`, `emulators`, `games`, `process`, `remote`, `settings`, `streaming`, `zerotier`; `addons` and `updates` are referenced by path in the handler list (`addons::addon_check_in`); `collections`, `host_devices`, `save_scope_migration`, `scheduler` declare no commands |
| `src-tauri/src/scheduler.rs` | `scheduler_task`, spawned at the end of `.setup()` |
| `src-tauri/remote/src/server_proto.rs` | `handle_server_proto_wrapper` / `handle_server_proto` (the `server://` proxy), `proxy_failure_status`; unused `handle_server_proto_offline*` |
| `src-tauri/remote/src/fetch_object.rs` | `fetch_object_wrapper` / `fetch_object` (the `object://` handler), `refresh_object`, `spawn_revalidate`, `fallback_to_cache` |
| `src-tauri/remote/src/cache.rs` | `cache_object`, `get_cached_object` (bitcode, 600-entry memory cache, disk under `cache/`), missing-object negative cache |
| `src-tauri/remote/src/utils.rs` | HTTP clients `DROP_CLIENT_ASYNC` / `DROP_CLIENT_SYNC` / `DROP_CLIENT_DOWNLOAD`, semaphores `SERVER_PROTO_SEM`, `OBJECT_FETCH_SEM`, `OBJECT_REVALIDATE_SEM`, `DROP_APP_HANDLE` |
| `src-tauri/remote/src/auth.rs` | `generate_authorization_header` (client cert auth for API calls), `auth::setup` |
| `src-tauri/utils/src/app_emit.rs`, `lock.rs` | `app_emit!` (emit, panics on failure), `lock!` (Mutex lock, panics on poison) |
| `src-tauri/utils/src/path_guard.rs` | `normalize_relative`, `join_within`, `ensure_parent_within`, `ensure_within` |
| `src-tauri/utils/src/redact.rs` | `redact` (token scrubbing for logs and bug reports) |
| `src-tauri/utils/src/webbrowser_open.rs`, `download_manager_send.rs` | `webbrowser_open`; download-manager send macro |
| `src-tauri/tauri.conf.json` | `identifier` `org.droposs.client`, `version`, CSP (`app.security.csp`), deep-link scheme `drop`, updater endpoint + pubkey, bundle targets |
| `src-tauri/capabilities/default.json` | Permissions for window `main` (core, shell open, window controls, deep-link, dialog, os, opener, updater, `process:allow-restart`) |
| `main/composables/use-object.ts` | `useObject(id)` (`object://`), `objectImageUrl(id)` (falls back to `server://` only in dev) |
| `main/composables/use-server-fetch.ts` | `serverUrl(path)` (`server://`), `rewriteDescriptionImages` |
| `main/composables/useListen.ts` | `useListen(event, handler)`: `listen` with unmount cleanup |
| `main/app.vue` | Root: `invoke("fetch_state")`, `setupHooks()`, `initialNavigation`, global hosts (`ModalStack`, toasts, `SaveSyncConflictHost`) |

## How it flows
1. `run()`: Linux sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`; `std::panic::set_hook` -> `custom_panic_handler`
   writes `DATA_ROOT_DIR/crash-<unix secs>.log` and prints the panic.
2. Builder: plugins (opener, os, updater, process, dialog, single-instance with deep-link forwarding, deep-link,
   shell, autostart) -> `invoke_handler(generate_handler![...])` -> `.setup(..)` -> two
   `register_asynchronous_uri_scheme_protocol` calls -> `on_window_event` (close hides to tray) -> `build`.
3. `.setup()`: `register_launch_telemetry`, `register_installed_sync`, then `block_on(setup(handle))`:
   log4rs init -> `ProcessManagerWrapper::init`, `DownloadManagerWrapper::init` -> `DB.database_is_set_up()` (first DB
   load) -> umu state, `SessionType::detect` -> `update::set_launch_gate`, `recover_all_at_startup` ->
   `reconcile_on_startup` + `reconcile_running_processes` -> `scan_install_dirs` -> if configured: `auth::setup`,
   `save_scope_migration::run`, autostart sync, playtime queue drain, `restore_pending_queue`.
4. Back in `.setup()`: background pollers, `app.manage(Mutex<AppState>)`, `DROP_APP_HANDLE`, window size (Gamescope
   on Deck hardware forced to 1280x800), `WindowBuilder` "main" + child webview "frontend" (`WebviewUrl::App("main")`),
   deep-link handler, tray menu, "Database corrupted" dialog if `prev_database` is set, `scheduler_task`.
5. `server://<path>`: `handle_server_proto` takes `base_url` + `auth.web_token` from the DB, rewrites scheme/authority,
   sets `User-Agent: Drop Desktop Client` and `Authorization: Bearer <web_token>`, sends with a 15 s timeout (one
   attempt), strips `Content-Security-Policy` and `X-Frame-Options`, caps the body at 128 MiB. No-response failures
   map to 504 / 502 / 401.
6. `object://<id>`: `fetch_object` serves any cached copy at once (revalidates in the background if expired), else
   fetches `/api/v1/client/object/<id>` with the client auth header (`refresh_object`), caches a real body, falls back to cache on error.

## Both surfaces
No UI of its own. The protocols, CSP and capabilities apply to desktop and BPM alike (same webview).

## Server side / client side
The server hides its header/footer for `User-Agent: Drop Desktop Client` (`isClientRequest()` in drop-server);
see `../drop-server/docs/map/auth-acls-clients.md` for the client auth and handshake routes.

## Rules and gotchas
- Every new frontend-callable command must be in the single `tauri::generate_handler![...]` list in
  `src-tauri/src/lib.rs` (starts line 353). An unregistered command fails only at `invoke` time. Commands in other
  crates are listed by path (`::remote::web_upload::upload_user_image`, `::process::compat::*`).
- Linux-only commands (`::process::compat::*`, lib.rs lines 469-481) need `#[cfg(target_os = "linux")]` on each list
  entry as well as on the function.
- Do not run `cargo clippy --fix` from one OS across cfg-gated code: the comment at `lib.rs` lines 16-19 records that
  it stripped the Linux-only `UMU_LAUNCHER_EXECUTABLE` import on Windows and broke the Linux build.
- Startup panics present as "the app opens and closes": `setup()` runs inside `.setup()` before the window is built.
  `drop.log` stops at the last line before the panic; the panic text is only in `crash-<unix>.log` (the hook does not
  log through log4rs). Startup `.expect`s include log roller/appender/`init_config` (lines 110-147), main window and
  webview build, tray menu items, and the DB ones listed in `docs/map/local-db-settings.md`.
- `app_emit!` and `lock!` panic on failure (`utils/src/app_emit.rs`, `utils/src/lock.rs`).
- Logs: `drop.log` rolls at 8 MiB (`LOG_FILE_MAX_BYTES`) keeping 3 archives (`LOG_FILE_ARCHIVE_COUNT`) named
  `drop.{N}.log` by log4rs `FixedWindowRoller` with no base set. log4rs's default base 0 would give `drop.0.log`
  (newest) to `drop.2.log` (inferred); the comment at `lib.rs` line 106 says `drop.1.log` is newest. Check both.
  Level from `RUST_LOG` (default `Info`). Bug reports attach only the live `drop.log`
  (`collect_bug_report_log_file`, redacted, tail ~5 MB).
- Images: use `useObject()` / `objectImageUrl()`. `server://` has no cache and a 6-permit semaphore (`SERVER_PROTO_CONCURRENCY`); routing
  art through it caused request storms (doc comment in `use-object.ts`).
- `serverUrl` strips leading slashes; `//api/...` would hit the server's 404 page.
- CSP in `tauri.conf.json`: a new external image host must be added to `img-src`; `connect-src` allows only
  `'self'` and `server:`, so a direct `fetch` to any other origin from the webview is blocked.
- `server://` uses the web token, `object://` and Rust API calls use `generate_authorization_header`. A device with
  `auth` but no `web_token` gets 401 from every `server://` iframe.
- `path_guard::join_within` is lexical only; a symlink inside the base can still escape (its own doc comment). Before deleting,
  moving or overwriting, check with `ensure_parent_within` (per `utils/src/path_guard.rs` ~38-53).
- Window labels: the window is `main`, the webview is `frontend`; `app.windows().get("main")` is the one to show
  (tray "open" handler).

## Changing things here
- New Tauri command: write `#[tauri::command]` in the matching module under `src-tauri/src/` (e.g. `src-tauri/src/games.rs`), make sure the module is
  glob-imported in `lib.rs` (or reference it by path), add it to `generate_handler!`, then call it from the frontend
  with `invoke("name", { camelCaseArgs })`.
- New plugin or permission: add the plugin in the builder in `lib.rs`, add its permission to
  `capabilities/default.json`, and the crate to `src-tauri/Cargo.toml` plus the JS package in `main/package.json`.
- New startup step: put it in `setup()` in order; return an error state instead of `.expect` (a panic there kills
  the app before a window exists).
- Proxy behaviour: `remote/src/server_proto.rs` (`handle_server_proto`); add a test next to the existing three.

## Tests
- `cargo +nightly-2026-04-20 test -p remote server_proto::` (3 tests), `-p remote cache::` (5),
  `-p utils` (`path_guard.rs` 7, `redact.rs` 5), from `src-tauri/`.
- No tests for `lib.rs` startup or `fetch_object.rs`.

## Related maps
`docs/map/local-db-settings.md`, `docs/map/launch-process.md`, `docs/map/downloads-install.md`, `../drop-server/docs/map/auth-acls-clients.md`
