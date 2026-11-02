# Achievements (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
While a game runs, the client detects achievement unlocks and reports them to the server, then shows a toast for
each unlock the server says it just recorded. Steam-style games (Goldberg/GBE, SmartSteamEmu, other crack formats,
the real Steam cache) are detected by reading unlock files on disk. RetroAchievements (RA) games are detected by
asking the server, which reads RA. The player links their RA account through the server, sees per-game achievement
lists with a reason when there are none, and can reset achievements (desktop only).

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/remote/src/achievements.rs` | `fetch_achievement_config`, `report_achievements`, `poll_achievements` (the session loop), `check_and_report_local`, `report_and_announce`, `poll_ra`, `notify_session_end`; types `AchievementConfigResponse` (`reason`, `ra_account_missing`), `AchievementReportResponse` (`unlocks`, `ignored_before_reset`) |
| `src-tauri/remote/src/goldberg/mod.rs` | `configure_saves_for_game`, `read_unlocks`, `read_earned`, `gse_candidate_paths`, `clear_local_unlocks`, `local_app_ids`, `read_local_steam_appid`, `wine_prefix_for_game`, `save_roots`, `DROP_GSE_FOLDER` (`drop-goldberg`) |
| `src-tauri/remote/src/goldberg/discovery.rs` | `find_steam_api_dir` (recursive, 8 levels), `detect_emulator_type` (`steam_settings/` = Goldberg, `steam_emu.ini` = SSE) |
| `src-tauri/remote/src/goldberg/config.rs` | `configure_goldberg` (writes `local_save_path=./drop-goldberg` to `steam_settings/configs.user.ini`), `check_gbe_activity` (`[GBE-DIAG]` logs), `write_custom_broadcasts` |
| `src-tauri/remote/src/goldberg/achievements.rs` | GBE `achievements.json` parser, `clear_earned_json`, `migrate_runtime_achievements_if_needed` |
| `src-tauri/remote/src/goldberg/sse.rs` | SmartSteamEmu `steam_emu.ini` and `achievements.ini` parsing |
| `src-tauri/remote/src/goldberg/crackers.rs` | `scan_all_crackers`: CODEX/RUNE, OnlineFix, RLD, SKIDROW, 3DM, CreamAPI, Razor1911, Steam client cache locations |
| `src-tauri/process/src/process_manager/launch.rs` | Pre-launch `configure_saves_for_game` (~1218); spawns `poll_achievements` and emits `achievement_unlocked` (~952-990) |
| `src-tauri/process/src/process_manager/exit.rs` | Cancels the poll (`achievement_poll_cancel`), calls `notify_session_end` |
| `src-tauri/src/games.rs` | Commands `clear_local_achievements`, `clear_all_local_achievements`, `check_ra_rom_hash`, `configure_game_emulator` |
| `src-tauri/remote/src/retroarch/ra.rs` | RetroArch RA credentials: `sync_ra_credentials` (GET `/api/v1/client/user/ra-credentials`), `RaSyncReport`, `hash_rom`, `check_rom_hash` |
| `src-tauri/src/settings.rs` | Commands `ra_store_credentials`, `ra_refresh_credentials`, `ra_clear_credentials` |
| `main/composables/achievements/status.ts` | Pure: `unavailableReasonText`, `parseAchievementStatus`, `raLinkState`, `parseRaServerAccount`, `raSyncProblemText` |
| `main/composables/achievements/toast.ts` | `AchievementUnlockedPayload`, `bpmToastHosts`, `acceptUnlock` (dedup) |
| `main/composables/ra-link.ts` | `useRaLink()`: link/unlink via server route `api/v1/user/external-accounts/retroachievements`, then local copy |
| `main/components/game-detail/Achievements.vue`, `main/composables/game-detail/use-game-stats.ts` | Desktop per-game list, reason text, reset + `clear_local_achievements` |
| `main/components/AchievementToast.vue` | Desktop toast (mounted in `main/app.vue`), stands down while a BPM host is mounted |
| `main/components/bigpicture/BpmAchievementToastHost.vue`, `main/components/bigpicture/BpmAchievementToast.vue` | Themed BPM toast (mounted in `main/layouts/bigpicture.vue`), queue of 5, 10 s dedup |
| `main/pages/settings/achievements.vue` | Desktop settings: RA link, reset one/all games (+ `clear_all_local_achievements`), debug view |
| `main/pages/welcome/retroachievements.vue` | Welcome-wizard RA link step |
| `main/composables/state-navigation.ts` | `achievement_tracking_inactive` listener (one modal per game per session) |

## How it flows
1. **Launch**: `configure_saves_for_game(install_dir, display_name)` finds the Steam API DLL dir, writes the Goldberg
   config, returns `EmulatorInfo`. Not incognito: `poll_achievements(game_id, emulator_info, cancel, on_new)` is spawned.
2. **Mode pick** (once, at start): `fetch_achievement_config` GETs `/api/v1/client/game/{id}/achievement-config`.
   An RA external link -> RA mode; else Goldberg links plus the on-disk `steam_appid` (`read_local_steam_appid`) ->
   Goldberg mode; else none (logs `reason`, waits for cancel). Seeds the known-unlocked sets from the config.
3. **Every 15 s** (`tokio::time::interval`, `MissedTickBehavior::Skip`):
   Goldberg mode: `check_and_report_local` -> `read_earned` (DLL dir, `%APPDATA%` fork dirs on host and in the Wine
   prefix, every cracker location) -> `report_achievements` (POST `achievements-report` with `clientNow`) ->
   `report_and_announce` calls `on_new` for each entry in the response's `unlocks`.
   RA mode: `poll_ra` (POST `/api/v1/client/game/{id}/ra-poll`), `on_new` for each new `external_id`.
   First Goldberg tick runs `check_gbe_activity`; no activity -> `achievement_tracking_inactive` event.
4. **Exit**: cancel fires one last local check (Goldberg) or three RA polls 3 s apart; exit then calls
   `notify_session_end` (POST `session-end`) when a playtime session was opened.
5. **Toast**: `on_new` emits `achievement_unlocked` `{id, title, description, iconUrl, gameId, gameName}`;
   `AchievementToast.vue` or `BpmAchievementToastHost.vue` shows it; game pages refresh their list.
6. **Reset**: `use-game-stats.ts` / settings page `fetch(serverUrl("api/v1/user/achievements/reset?gameId="))`
   (DELETE) -> then `invoke("clear_local_achievements")` so the local GBE files read as not earned.

## Both surfaces
Desktop: game page `Achievements.vue`, `AchievementToast.vue`, `main/pages/settings/achievements.vue` (RA link, reset,
debug), welcome step. BPM: achievements list and reason inside `main/pages/bigpicture/library/[id].vue` (fetches
`achievements/status`), `BpmAchievementToastHost.vue`, RA link in `main/pages/bigpicture/settings.vue` via `useRaLink`.
Gap: BPM has no achievement reset (no reset call or `clear_local_achievements` anywhere under `main/pages/bigpicture` or
`main/components/bigpicture`). The tracking-inactive and other notifications are generic `createModal` calls on both.

## Server side
See `../drop-server/docs/map/achievements.md`.

## Rules and gotchas
- `achievement-config` is fetched once per session, not polled. The 15 s loop reads local files (or `ra-poll`).
  The comment in `CLAUDE.md` and the server's `config-cache.ts` header still say the config is polled every 15 s.
- Toasts come only from the report response's `unlocks` list. Older servers without it: if `newly_unlocked > 0`
  the client refetches the config and toasts what flipped (`report_and_announce`).
- GBE unlock files land at `<dll_dir>/drop-goldberg/<AppID>/achievements.json`, where `dll_dir` is wherever
  `find_steam_api_dir` found the DLL (can be deep, e.g. Unreal `Binaries/Win64`). Never assume the install root.
- Drop's own all-`false` `drop-goldberg` file must not mask real unlocks: `gse_candidate_paths` collects every
  candidate and the reader keeps whichever has unlocks.
- On Linux, a Windows game under Proton writes `%APPDATA%` inside its Wine prefix: pass
  `wine_prefix_for_game(game_id)` to every read and clear (`poll_achievements`, `clear_local_achievements_for`).
- Unlock times outside 2000..2100 (including 0) are reported as "now", and future times are clamped to now
  (`check_and_report_local`). A "now" time is after any reset, so it gets past the server's reset marker: that is why
  a reset must also run `clear_local_achievements`.
- `clear_local_unlocks` refuses while the game runs (GBE rewrites its state on exit) and only rewrites GBE/Goldberg
  JSON; other cracker formats rely on the server reset marker.
- RA linking always goes through the server; `ra-link.ts` then stores the returned Connect token locally. The server
  is authoritative: `sync_ra_credentials` overwrites or clears the local copy to match it.
- `AchievementConfigResponse` / `AchievementItem` derive bitcode `Encode, Decode` for the offline cache
  (`cache_object`). A changed shape makes old cached copies unreadable, which only loses the offline fallback (inferred).

## Changing things here
- New cracker format: add a `CrackerFormat` variant, its locations and parser in `src-tauri/remote/src/goldberg/crackers.rs`; results must
  be `GoldbergAchievement` with `earned_time` in unix seconds.
- New config field from the server: add it with `#[serde(default)]` to `AchievementConfigResponse` in
  `achievements.rs`, then read it in `poll_achievements`.
- New "no achievements" reason: server `availability.ts` first, then `unavailableReasonText` in
  `main/composables/achievements/status.ts` and `main/tests/achievement-status.test.ts`.
- Reset on BPM: add a focus-registered action on the BPM game page calling the same reset route and
  `clear_local_achievements`; follow the BPM modal input-lock rules if it confirms.

## Tests
- `cargo +nightly-2026-04-20 test -p remote goldberg::` (`goldberg/mod.rs`, `goldberg/achievements.rs`; none in
  `crackers.rs`, `sse.rs`, `config.rs`, `discovery.rs`, `achievements.rs`)
- From `main/`: `node --test tests/achievement-status.test.ts`

## Related maps
`../drop-server/docs/map/achievements.md`, `docs/map/launch-process.md`, `docs/map/emulation.md` (RetroArch, RA ROM hashes),
`docs/map/big-picture.md`, `docs/map/cloud-saves-playtime.md`.
