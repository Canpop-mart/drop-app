# Remote / Server-Communication Audit — 2026

Scope: `src-tauri/remote/src/` (the Drop-server communication core) plus the
Tauri command layer in `src-tauri/src/remote.rs` and `src-tauri/src/streaming.rs`.

Excluded (separate audit): `remote/src/retroarch.rs`, `remote/src/goldberg.rs`.
Not in scope: the Drop server itself, expanding the dev-gated save-sync /
streaming features.

---

## 1. Summary

The remote layer was functionally sound but inconsistent: every feature
hand-rolled its own `DROP_CLIENT_ASYNC.get(..).send()` with its own (usually
absent) retry story, and the error enum could not distinguish a network blip
from a 5xx from a parse failure. This pass introduces **one shared HTTP
helper** with retry/backoff, **one coherent error taxonomy**, makes the
achievement poll interval **drift-free**, decomposes the 865-line
`save_sync.rs` monolith, and documents cache bounds.

`cargo check -p remote` passes. Warning count went from 5 (baseline) to 2
(both pre-existing, in excluded/`lib.rs` code) — net **−3**, zero new.

---

## 2. Findings

### 2.1 `server://` protocol (`server_proto.rs`) — OK, one hardening

The custom protocol proxies iframe requests to the Drop backend. The handler:

- pulls `web_token` + base URL from the DB, fails `401` if absent;
- rewrites the URI authority/scheme to the real server;
- strips the inbound `User-Agent`, sets `Drop Desktop Client`;
- attaches `Authorization: Bearer {web_token}`;
- strips `Content-Security-Policy` and `X-Frame-Options` from the response so
  the page renders inside the iframe.

**Issue found & fixed:** the `Authorization` header was added with
`HeaderMap::append`, not `insert`. If the iframe document ever already carried
an `Authorization` header, the backend would receive two values (ambiguous,
and a stale credential could leak). Changed to `insert` and marked the value
`sensitive` so it is redacted from any header debug output. No other header
leakage: the request headers are forwarded by design (iframe semantics) and
the response copy is allow-listed against CSP/X-Frame-Options.

### 2.2 Auth (`auth.rs`) — no refreshable token by design

**There is no long-lived bearer token to refresh.** `generate_authorization_header()`
mints a fresh **ES384 JWT with a 10-second TTL** (`nbf = now`, `exp = now + 10`)
on *every* request, signed with the client's stored EC private key. The only
persisted credentials are the EC private key + certificate (the device
identity) and an optional `web_token` for the `server://` proxy.

Consequences for the brief's "proactive refresh / refresh race" objectives:

- **Proactive refresh** — not applicable. A token is created per request and
  expires 10s later; there is nothing to refresh ahead of expiry.
- **Refresh race / torn token** — not possible. No shared mutable token state;
  each request signs its own JWT independently.
- The shared request helper mints the JWT **inside the retry loop**, so a
  retried attempt always carries a fresh, valid token even if the first
  attempt's token has since expired. This is the one real "refresh"
  correctness requirement, and it is now satisfied.

Credential storage: the private key lives in the `Database` (`auth.private`),
persisted via the database layer. `setup()` offline-detection was widened —
it previously matched only `RemoteAccessError::FetchError(_)`; it now uses
`RemoteAccessError::is_retryable()` so a timeout or `ServerUnavailable` also
drops the app to Offline with the cached user, instead of forcing re-auth.

Deferred: `generate_authorization_header()` still `unwrap()`s on key parsing /
JWT signing. A malformed stored key would panic the calling task. Converting
it to return `Result` ripples through every call site (incl. excluded files)
— deferred, noted below.

### 2.3 HTTP layer (`requests.rs`, `utils.rs`) — rebuilt

`requests.rs` was 42 lines: `generate_url` + two bare `make_authenticated_*`
fetches with **no retry, no backoff**, returning raw `reqwest_middleware::Error`.
Each feature module then hand-rolled status handling.

Rewritten — see §3.1. `utils.rs` was already in good shape (bounded
JSON/bytes readers with size caps, an `AutoOffline` middleware, per-purpose
clients with sane connect/total timeouts) and is unchanged except that it is
now consumed exclusively through the new helper.

### 2.4 Cache (`cache.rs`) — bounded in memory, unbounded on disk

- **Memory cache:** `HashMap` behind a `Mutex`, hard cap **100 entries**, 24h
  TTL. When full it drops expired entries, then evicts 25%. Bounded. ✓
- **Disk cache:** one file per key under `cache_dir`, written through on every
  `cache_object`. **No TTL and no eviction on disk** — it grows with the
  number of distinct cached objects (user record, per-game achievement
  configs, media objects). Not unbounded *per session*, but unbounded over the
  lifetime of an install.

Change: replaced the three `60 * 60 * 24` magic numbers with a documented
`DEFAULT_CACHE_TTL_SECS` constant and documented the memory bound + the
disk-cache behaviour inline. Disk eviction is **deferred** (see §5) — it would
expand surface and needs a size-budget design decision.

### 2.5 Error handling (`error.rs`) — now one coherent taxonomy

The enum mixed transport, server, parse and state errors with no way to branch
on *class*. Added (see §3.2):

- `Timeout`, `ServerError { status, message }`, `ServerUnavailable`,
  `Unauthorized` — so network / auth / server-5xx / parse are distinguishable;
- `RemoteAccessError::is_retryable()` and `is_auth_error()` classification
  helpers used by the request helper and by `auth.rs`.

`FetchError` / `FetchErrorLegacy` were kept (load-bearing across the
`download_manager` and `games` crates).

### 2.6 Achievements polling (`achievements.rs`) — scoped correctly, now drift-free

- **Polls only for running games:** `poll_achievements` is spawned by
  `process_manager/launch.rs::register_running_process`, once per launched
  game, and is *not* run otherwise. ✓
- **Stops on exit:** each running process holds an `Arc<Notify>`
  (`achievement_poll_cancel`); `process_manager/exit.rs` calls `notify_one()`
  on it when the game exits, and the poll loop's `tokio::select!` returns. ✓
- **Cannot stack:** the loop is strictly sequential — one poll cycle finishes
  (incl. its network calls) before the next wait begins. There is never more
  than one in-flight poll per game. ✓
- **Interval drift — fixed.** The loop previously waited with
  `sleep(15s)` *after* each cycle, so the true period was `15s + work`. It now
  uses a fixed-cadence `tokio::time::interval(15s)` with
  `MissedTickBehavior::Skip`: ticks land on a fixed 15s grid (drift-free), and
  if a cycle ever overruns 15s the missed ticks are dropped rather than firing
  back-to-back (reinforces no-stacking).

Network calls in this module now route through the shared helper.

### 2.7 Playtime (`playtime.rs`) — hardening verified intact

The earlier hardening is **present and untouched**:

- `start_playtime` / `stop_playtime` — own 3× retry loops with exponential
  backoff (1/2/4s), fast-bail on non-429 4xx, and `stop_playtime`'s "session
  already ended" 400 handled as success;
- `heartbeat_playtime` — best-effort, swallows network errors; driven by
  `process_manager/exit.rs::run_playtime_heartbeat_loop` every 5 min;
- `queue_pending_stop` — persists a failed stop to
  `{data}/pending-playtime-stops/{session}.json`;
- `drain_pending_stops` — replays the queue at startup (called from
  `src/lib.rs`).

Only change: the three transport calls now go through the shared request core
**with the core's own retry disabled** (`with_max_attempts(1)`), because
playtime owns a domain-specific retry loop (queue-on-failure, "already ended"
detection). Routing through the core without disabling its retry would have
compounded the two layers into ~9 attempts / ~30s of backoff — a regression.
This keeps playtime's hardened loop authoritative while still sharing the
timeout, per-attempt JWT and `AutoOffline` middleware. **No hardening undone.**

### 2.8 Save sync (`save_sync.rs`) — decomposed

865-line monolith. It was a genuine grab-bag (types + manifest I/O + local
file scanning + 3 API endpoints + conflict logic) with clean seams. Split into
a `save_sync/` directory module — see §3.3. Surface unchanged: every public
item is re-exported from `save_sync/mod.rs`, so `remote::save_sync::*` paths
used by the `process` crate keep compiling. Dev-gated; not expanded.

### 2.9 Streaming sessions (`streaming_sessions.rs`) — migrated, not expanded

Dev-gated. 469 lines, ~25 near-identical
`make_authenticated_* → status check → bounded_json` blocks. Migrated to the
shared helper (`remote_request` / `remote_request_ok`), which collapsed the
boilerplate substantially with no behaviour change and no new surface. State:
functional; the `Tauri` command layer in `src/streaming.rs` (Sunshine /
Moonlight process management) was read and is **out of scope** to expand —
left as-is.

### 2.10 `fetch_object.rs` (`object://` media path) — migrated + 2 bug fixes

The media-object fetch now routes through `make_authenticated_get` (retry +
per-attempt auth). Two latent bugs fixed while there:

1. **Panic on missing `Content-Type`:** the response builder did
   `.get("Content-Type").expect(...)`. A media object served without that
   header would panic the protocol handler. Now defaults to
   `application/octet-stream`.
2. **Cache poisoning:** if the body read failed (e.g. oversized), the old code
   substituted an **empty `Vec`** and then *cached that empty body*, so every
   later request served a broken image from cache. Now a failed body read
   falls back to the existing cache entry and never overwrites it.

---

## 3. Changes

### 3.1 One HTTP request helper

`requests.rs` is now the single HTTP entry point for the crate.

- **`send_with_retry`** (private core) — takes a closure that builds a fresh
  `RequestBuilder` per attempt (required: `reqwest_middleware::RequestBuilder`
  is not `Clone`). Mints a fresh JWT per attempt, sends, and retries on
  transient failure with exponential backoff (~0.5/1/2s) + small clock-derived
  jitter. Retries: connect/timeout/request transport errors and HTTP 429/5xx.
  Fast-fails: 4xx (≠429) and parse errors. Logs every attempt/retry/give-up
  under a `[REQ]` tag.
- **`RemoteRequest<'a, B>`** — a builder describing one call (method, URL,
  optional JSON body by reference, `with_max_attempts`, `with_json_cap`). The
  body is held by reference and re-serialized by `reqwest` each attempt — **no
  `serde_json` round-trip**, callers keep passing their inner
  `#[derive(Serialize)]` structs (honours the `CLAUDE.md` constraint).
- **`remote_request<R, B>`** — typed: sends, classifies any non-2xx into
  `Unauthorized` / `ServerError` / `ServerUnavailable`, deserializes a 2xx body
  into `R` under a size cap. The preferred API for new code.
- **`remote_request_ok`** — same, for endpoints whose 2xx body is uninteresting.
- **`make_authenticated_get` / `make_authenticated_post`** — kept with their
  original *names* but now route through the retry core. Their error type
  changed from `reqwest_middleware::Error` to `RemoteAccessError`; all
  workspace call sites use `?`-into-`RemoteAccessError` or `.map_err(to_string)`,
  both of which still compile. This means `games`, `download_manager`,
  `retroarch` and the Tauri command layer get retry + backoff + per-attempt
  auth for free, with no edits.

Every in-scope module (`achievements`, `playtime`, `save_sync`,
`streaming_sessions`, `fetch_object`, `auth`) now makes its HTTP calls through
this helper. No hand-rolled `DROP_CLIENT_ASYNC.get(...)` remains in scope.

### 3.2 Error taxonomy

`RemoteAccessError` gained `Timeout`, `ServerError { status, message }`,
`ServerUnavailable(String)`, `Unauthorized`, each with an actionable `Display`
message, plus `is_retryable()` / `is_auth_error()` classifiers. The variant
doc comments group the enum into network / auth / server / parse / state
classes. `serde_with::SerializeDisplay` is retained so Tauri-command
serialization is unaffected.

### 3.3 `save_sync` decomposition

`save_sync.rs` → `save_sync/`:

| File          | Responsibility                                                       |
| ------------- | -------------------------------------------------------------------- |
| `mod.rs`      | Module docs, public types (manifest / response / event), shared helpers, `pub use` re-exports |
| `manifest.rs` | On-disk per-game manifest: load / save (atomic) / corrupt-repair / post-sync update |
| `scan.rs`     | Local save discovery (emulator dirs + Ludusavi) and downloaded-save write-back |
| `api.rs`      | The three Drop-server endpoints + their private wire structs         |
| `conflict.rs` | Sync-check → UI conflicts, and applying user resolutions             |

### 3.4 Misc

- Achievement poll loop converted to drift-free `tokio::time::interval`.
- `cache.rs`: documented `DEFAULT_CACHE_TTL_SECS`, documented memory bound.
- `server_proto.rs`: `Authorization` set via `insert` + marked sensitive.
- `fetch_object.rs`: missing-`Content-Type` panic and empty-body cache
  poisoning fixed.
- Removed 3 baseline warnings (unused `HeaderMap` import; 2 dead struct fields
  in the old `save_sync` upload-response type).

---

## 4. Acceptance criteria

| Criterion                                                            | Status |
| -------------------------------------------------------------------- | ------ |
| Every remote module uses one shared helper with retry + timeout      | Done   |
| Token refresh is proactive and race-safe                             | N/A by design — see §2.2 (no refreshable token; JWT minted per attempt) |
| Achievement polling stops when a game isn't running, cannot stack    | Verified §2.6 (already correct); interval also made drift-free |
| Playtime hardening (heartbeat / retries / queue) confirmed intact    | Verified §2.7 — untouched |
| `error.rs` is one coherent enum                                      | Done §3.2 |
| `cargo check -p remote` passes, no new warnings                      | Passes — 5 → 2 warnings (both pre-existing) |

---

## 5. Deferred items

- **Disk cache eviction / size budget.** The on-disk cache has no TTL or size
  cap; it grows over an install's lifetime. Needs a size-budget + LRU-by-mtime
  sweep. Deferred — expands surface, wants a design decision.
- **`generate_authorization_header()` panics.** Still `unwrap()`s on EC key
  parsing and JWT signing. A corrupt stored key panics the calling task.
  Converting to `Result` ripples through every call site including the
  excluded `retroarch.rs`/`goldberg.rs` — deferred to a focused change.
- **Streaming-session reliability.** The host-side heartbeat and stream-request
  poller in `src/streaming.rs` are functional but dev-gated; a deeper audit of
  Sunshine/Moonlight lifecycle handling was out of scope here.
- **`fetch_drop_object` Tauri command** (`src/remote.rs`) still uses the
  blocking sync client directly rather than the shared helper. Low-traffic,
  left as-is; could be unified in a follow-up.
