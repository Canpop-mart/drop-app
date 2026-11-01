# Addons: shop and shared-save play actions (client)

Checked against the code on 2026-11-01. If a path here no longer exists, the map is stale: fix it or flag it.

## What it does
An addon is an external service the admin runs (first one: the NCAA 14 league website). In the Store's Addons tab a
player adds or removes addons. On a game's page, an addon with "save buttons" shows one row per league the player is in,
with Check out (take the league's lock and put the official dynasty save on this device) and Check in (send the played
save back). The client never talks to the league: the webview talks to drop-server through `server://`, and the Rust
side downloads and uploads the save file to drop-server with the client's web token.

## Where the code is
| Path | What's there |
|---|---|
| `src-tauri/src/addons.rs` | Commands `addon_preflight`, `addon_install_save`, `addon_check_in`, `addon_local_state`. `hold` (takes `process::hold_for_addon`, then checks no game process runs), `game_process_running` (sysinfo), pure `process_matches` |
| `src-tauri/remote/src/addons.rs` | File work: `SaveSlot`, `AddonError`, `check_slot`, `is_safe_segment`, `resolve_save`/`resolve_save_in`, `win_local_appdata`, `Receipt`/`read_receipt`/`write_receipt`, `backup`, `backups_to_prune`, `is_addon_managed`, `local_unsent`, `install_save`, `upload_save`. Consts `MAX_SAVE_BYTES` (64 MB), `BACKUPS_KEPT` (2) |
| `src-tauri/process/src/lib.rs` | `AddonHold`, `AddonHoldRefused`, `hold_for_addon`, `addon_hold_blocked` |
| `src-tauri/process/src/process_manager/launch.rs` | Launch refused while `addon_holds` contains the game |
| `src-tauri/remote/src/save_sync/scan.rs` | Cloud saves skip addon-managed paths (calls `crate::addons::is_addon_managed` in the scan filter and before a restore) |
| `src-tauri/src/lib.rs` | `mod addons;` and the four `addons::addon_*` entries in `generate_handler!` |
| `main/composables/use-server-api.ts` | `api.addons.list`, `add`, `remove` (`api/v1/addons/{name}/membership`), `forGame` (`api/v1/games/{id}/addons`), `checkout` |
| `main/composables/addon-shop.ts` | `useAddonShop()`: `load`, `add`, `remove`, `askRemove`, `cancelRemove`; one request at a time |
| `main/composables/addon-shop-logic.ts` | Pure: `parseShopList`, `withAdded`, `rowView` |
| `main/composables/addon-play.ts` | `useAddonPlay(gameId, running)`: `load`, `checkOut`, `checkIn`, `cancelReplace`, re-reads local state when the game closes |
| `main/composables/addon-play-logic.ts` | Pure: `parseGameAddons`, `isMissingRoute`, `toProblem`, `parseOutcome`, `ctxKey`, `contextView`, `actionLabel` |
| `main/components/StoreAddonsTab.vue` | Desktop Store Addons tab, mounted in `pages/store/index.vue` (`activeTab === 'addons'`) |
| `main/components/bigpicture/BpmStoreAddons.vue` | BPM Store Addons, mounted in `pages/bigpicture/store.vue` |
| `main/components/game-detail/AddonActions.vue` | Desktop league rows, mounted in `components/game-detail/Page.vue` as `GameDetailAddonActions` |
| `main/components/bigpicture/game-detail/BpmAddonActions.vue` | BPM league rows, mounted in `pages/bigpicture/library/[id].vue` |

## How it flows
1. Shop: tab mounts -> `useAddonShop().load()` -> `api.addons.list()` (`GET server://api/v1/addons`) -> rows. Add/remove
   -> POST/DELETE `.../membership`; the row changes only after the server agrees (`withAdded`).
2. Panel: game page -> `useAddonPlay().load()` -> `api.addons.forGame(id)` -> `parseGameAddons` -> for each league
   `invoke("addon_local_state")` (receipt + save hash). `contextView` picks the buttons.
3. Check out: `checkOut(key)` -> `invoke("addon_preflight")` (game not running or held, save folder resolvable, unsent
   play?) BEFORE the lock -> if unsent and not confirmed, show Keep mine / Replace -> `api.addons.checkout(name, ctx)`
   (server takes the league lock, returns `{version, sha1}`) -> `invoke("addon_install_save")` -> Rust `hold` ->
   `install_save`: GET `/api/v1/addons/{addon}/contexts/{ctx}/versions/{v}/file`, size + SHA-1 check, `still_safe()`
   re-check, `backup` the old file (`-replaced` or `-unsent`), `write_atomic`, `write_receipt`.
4. Check in: `checkIn(key)` -> `invoke("addon_check_in")` -> `hold` -> `upload_save`: refuses `nothing_to_send` when the
   file still equals what was checked out -> POST raw bytes to `.../checkin?base_version=<receipt>` -> on success
   receipt `handed_in: true`; on `not_checked_out`/`diverged` the local copy is backed up as `-unsent`.

## Both surfaces
Desktop: `StoreAddonsTab.vue`, `AddonActions.vue`. BPM: `BpmStoreAddons.vue`, `BpmAddonActions.vue`. Both share the
same composables; no feature gap. Neither uses a modal: remove and replace confirmations are inline rows. The web
server's game page shows a read-only panel (server map); checking out and in is client only.

## Server side
`../drop-server/docs/map/addons.md` (routes, `DROP_ADDONS`, the NCAA 14 adapter, `UserAddon`).

## Rules and gotchas
- Nothing in a `SaveSlot` is trusted. Root must be `winLocalAppData`; every segment passes `is_safe_segment`; the path
  is checked lexically (`join_within`) and on the real filesystem (`ensure_parent_within`), again right before the
  write (`check_still_contained`). Don't add a root kind without the same checks.
- `win_local_appdata` on Linux is the game's Wine/Proton prefix; `None` before the game has run once, which surfaces as
  `play_first`.
- Two locks: `process::hold_for_addon` blocks Drop launches; `game_process_running` catches a game started outside Drop
  (matches Linux's 15-char truncated name, e.g. `ncaafootball14.`). Both are needed.
- Cloud saves must skip addon saves or a pre-launch restore overwrites a fresh check-out. `is_addon_managed` reads the
  receipts' folders; `write_receipt` clears the `MANAGED` cache. If you add a new place that writes a receipt, go
  through `write_receipt`.
- Backups live in `<data>/addons/<addon>/<ctx>/backups/`; `backups_to_prune` keeps the newest 2 plus the newest
  `-unsent` copy.
- Never check in from a device whose receipt does not match the lock: `contextView` offers Check out instead when
  `local.baseVersion !== ctx.lockBase` or `handedIn`. There is no "release" button by design.
- Checkout refusals come back as 200 `{ok: false, code, message}`; `parseOutcome` turns them into row text. Network
  failure is a separate message. A 404 on the panel route is treated as "no addons" (`isMissingRoute`, older servers).
- Rust uses the DB's `web_token` (`server_auth`), so the server must grant `addons:play` to client web tokens.
- `receipt.json` is plain JSON, not `pot`, but every `Receipt` field still has `#[serde(default)]`; keep it that way.
- BPM: `useBpFocusableGroup` registers an element once (`composables/bp-focusable.ts:89`). Handlers in
  `BpmAddonActions.vue`/`BpmStoreAddons.vue` take the row KEY or NAME and look the row up at fire time (`current(k)`);
  never close over the row object. `disabled` does not stop `onSelect`, so each handler checks `busy` itself. Focus is
  restored by `data-addon-play-*` key, never by index.
- Player-facing strings are placeholders (owner rewrites): `AddonError` messages in both Rust files, notices in
  `addon-play.ts`, lines in `contextView`.

## Changing things here
- New Rust addon command: `src-tauri/src/addons.rs` -> register in `src-tauri/src/lib.rs` `generate_handler!` ->
  call from `addon-play.ts` -> both `AddonActions.vue` and `BpmAddonActions.vue`.
- New field from the server's context: server `PlayContext` (drop-server `ncaa14.ts`) -> `parseContext` in
  `addon-play-logic.ts` -> `contextView` -> both components. Add a case to `tests/addon-play.test.ts`.
- New save location rule: `SaveSlot` + `check_slot` + `resolve_save_in` in `remote/src/addons.rs`, and the server's
  `slotFor`. New `SaveSlot` fields need `#[serde(default)]` or older servers' slots fail to parse.

## Tests
- Rust (from `src-tauri/`): `cargo +nightly-2026-04-20 test -p remote addons::` and
  `cargo +nightly-2026-04-20 test -p drop-app addons::`.
- Frontend (from `main/`): `node --test tests/addon-play.test.ts` and `node --test tests/addon-shop.test.ts`.

## Related maps
`../drop-server/docs/map/addons.md`, `docs/map/cloud-saves-playtime.md` (the `save_sync` skip), `docs/map/big-picture.md`.
