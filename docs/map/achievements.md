# Achievements (client)

Checked against the code on 2026-11-05. Core rebuild approved by owner; uncommitted and not device-tested.
Design and remaining approval gates: `docs/achievement-tracking-overhaul.md`.

## What it does

Captures local unlocks into a durable, account-scoped JSON journal before notifying the frontend. The network
worker independently retries sync, including after restarting Drop without relaunching the game. A failed config
fetch no longer selects a permanently disabled session. RA remains service-synced; no emulator bridge was installed.

## Where the code is

| Path | Responsibility |
|---|---|
| `src-tauri/remote/src/achievements.rs` | Wire types, scoped config/legacy report/RA requests, session entry point |
| `src-tauri/remote/src/achievements/journal.rs` | Scope, immutable observations, atomic versioned records, bounded latest-value progress, config cache, durable popup dedup |
| `src-tauri/remote/src/achievements/tracking.rs` | Pre-launch baseline, local scanner, status, reset invalidation, independent sync worker and per-event acknowledgements |
| `src-tauri/remote/src/achievements/notification.rs` | Opt-in Windows nonactivating popup prototype; no graphics hooks; native Steam popup reuse |
| `src-tauri/remote/src/goldberg/tracking.rs` | Read-only existing Windows-format adapters, runtime JSON progress, directory watchers, recovery scans |
| `src-tauri/remote/src/goldberg/steam.rs` | Complete native Steam binary KeyValues schema/bitfield reader; one installation/account, explicit unknown-format errors |
| `src-tauri/remote/src/goldberg/crackers.rs`, `sse.rs`, `achievements.rs` | Existing local format parsers reused by the adapter |
| `src-tauri/process/src/process_manager/launch.rs` | Prepares baseline/watchers before spawning; starts capture only outside incognito |
| `src-tauri/process/src/process_manager/exit.rs` | Cancels capture; scanner continues three seconds for delayed final writes |
| `src-tauri/src/games.rs` | Local view, Retry, popup preview, reset/clear commands |
| `src-tauri/src/lib.rs` | Starts durable worker at startup and registers commands |
| `src-tauri/database/src/models.rs`, `src-tauri/src/settings.rs` | Default-false achievementGamePopups setting and save allowlist |
| `main/components/AchievementTrackingStatus.vue` | Shared desktop/BPM status, local unlocks/progress, errors, Retry, popup controls, inline BPM reset |
| `main/composables/achievements/tracking.ts` | Pure progress/order/identity helpers and local unlock overlay |
| `main/composables/game-detail/use-game-stats.ts` | Desktop achievement cards reflect durable local unlocks before server acknowledgement |
| `main/pages/bigpicture/library/[id].vue` | Shared tracking panel and local unlock overlay; controller-registered actions |
| `main/components/game-detail/Page.vue` | Desktop panel placement |
| `main/public/achievement-popup.html` | Isolated click-through popup asset; no Tauri IPC or external media |
| `main/composables/achievements/toast.ts` | Existing app-window toast payload and dedup helpers |
| `src-tauri/remote/src/retroarch/ra.rs`, `main/composables/ra-link.ts` | Existing RA credential/hash integration and server-authoritative linking |

## Flow

1. Before launch, resolve cached server links plus local AppID/SSE configuration, snapshot earned achievements,
   and watch the closest existing parent of candidate files. Baseline achievements import quietly.
2. The scanner reacts to filesystem events (60 ms debounce), with one-second recovery scans/rearming. The network
   never runs inside this loop. Existing game setup remains unchanged; the new adapters only read files.
3. New observations are saved atomically before local notification. Unknown acquisition times remain null.
   Initially unavailable config uses an unresolved epoch; the worker resolves it after obtaining server config.
4. A scope-bound worker fetches fresh config at most every 30 seconds (Retry invalidates that budget), then sends
   at most 100 events per request. Each request pins its original server and paired-client JWT, preventing account
   changes during an await from submitting an old event with new credentials.
5. Accepted/duplicate/reset-rejected/invalid events have terminal dispositions. Missing definitions remain queued.
   Network errors keep events and back off. Filesystem events cannot bypass the minimum retry budget.
6. RA is independently polled no more often than every 15 seconds while active; both local and RA sources can run.
   The server retains separate softcore/hardcore states. Native emulator popups remain the immediate RA display.
7. On exit, the local scanner reads for a three-second grace period. Pending events survive app shutdown. A write
   later than that grace period can only be rediscovered next launch and is imported quietly.

## Local storage and identity

- New sidecar root: achievements-v2 under Drop's existing data directory. No pot/install/mod-ledger changes.
- A partition hashes server URL, Drop user and paired client ID. Records retain the source account and provider game
  ID; native Steam never merges every userdata directory. Account changes during a session stop that Steam source
  with a visible reason until relaunch.
- Each unlock has a stable event ID across retries; a reset changes its identity. Progress overwrites one latest-value
  file per achievement/account/mode/epoch, preserving legitimate decreases and not accumulating every offline update.
- Old progress acknowledgements cannot retire a newer value. Corrupt JSON files are preserved and reported separately
  while other records continue syncing. Journal version changes need an explicit reader/migration.
- Popup receipt is persisted before display: a crash can lose the popup between receipt and rendering, but cannot
  lose the durable unlock. Historical imports do not replay celebratory popups.

## Both surfaces

Desktop and BPM share status, source/read/sync problems, pending counts, recent local unlocks, measured progress and
RA mode history. Retry and Windows popup controls are controller-registered in BPM. BPM reset uses inline confirmation
and the existing reset/clear API, with no new modal or input lock. Ten themes are unchanged.
The existing overall achievement cards overlay local durable unlocks while the server is unavailable.

## Reset and compatibility

- Server reset epochs are authoritative, including a global reset for offline/unseen games. Local reset invalidates
  journal state and cached config before the existing local Goldberg clear. Clearing while running remains refused.
- A source with a real old unlock time stays rejected even after reconnect. Timestamp-less historical state cannot
  pass a reset just by becoming "now". The server also checks observation time and reset epoch.
- Old servers still accept bounded legacy unlock reports. Their aggregate response cannot acknowledge individual
  missing IDs, so ambiguous reports remain queued; progress/mode storage needs the new server. Prefer server-first rollout.
- Existing encrypted RON databases get a default-false popup setting; it is in ALLOWED_SETTINGS_KEYS. Old bitcode config
  files are not migrated; this core uses its new scope-bound JSON cache.
- Installed game files, .dropdata, mod ledgers and libs/drop-base are unchanged by the core rebuild.

## Coverage and limits

Native Steam binary formats are undocumented. Bounds/shape errors are visible, and fixtures cover bitfields and
account selection; real Steam builds still need validation. Existing standalone formats are reused, not a universal
achievement API. Actual progress requires source counters/targets. A build that exposes no data needs an adapter.
RA still has service propagation delay and no live Drop challenge feed. A managed emulator event bridge, other
storefront integrations and graphics hooks require separate reviewed work. The Windows popup is opt-in and defaults
off; exclusive fullscreen and Linux/Gamescope notification transport are unverified/not implemented respectively.

## Checks

From src-tauri with the pinned toolchain: cargo +nightly-2026-04-20 check --all-targets;
test -p remote achievements:: --lib; test -p remote goldberg:: --lib.
From main: Nuxt typecheck and explicit tests `main/tests/achievement-status.test.ts`,
`main/tests/achievement-tracking.test.ts`. Tests exercise journal recovery/account separation, old-setting compatibility,
latest progress, binary Steam fixtures, partial writes and actual filesystem watcher directory creation/replacement.
Compiles on Windows, untested on device. PostgreSQL migration rehearsal and gameplay checks remain owner-side gates.

## Server

`../drop-server/docs/map/achievements.md` describes the additive ingest schema, legacy compatibility and RA authority.
