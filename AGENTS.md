# drop-app: rules for coding agents

Drop is a self-hosted game platform. This repo is the **desktop client**: a Tauri 2 app with a Rust backend
(`src-tauri/`) and a Nuxt 3 frontend (`main/`). It runs on Windows, Linux and the Steam Deck. The server is the
sibling repo `../drop-server`.

## Start here
1. Read `docs/INDEX.md` and open the ONE map in `docs/map/` for the area you are working in.
2. If the task came from the plan, it is in `docs/UPCOMING.md`.
3. Keep notes in `tasks/current.md` as you work (copy `tasks/TEMPLATE.md`). See `docs/workflow.md`.
4. Do not read whole large files. Search first (`rg "name" src-tauri main`), then read only the lines you need.

## Hard rules (breaking these has caused real damage)
- **Never commit, push, tag or release.** The owner does that. Leave changes uncommitted.
- **Never edit `libs/drop-base`** (git submodule).
- **Rust toolchain is pinned.** Always `cargo +nightly-2026-04-20 ...`. Never change the pin.
- **Saved data formats are fragile. Read `docs/map/local-db-settings.md` before changing any saved struct.**
  - App database `drop.db` (RON, encrypted): a new field needs `#[serde(default)]`, or every existing database fails
    to load, gets backed up and replaced with an empty one (the user loses their state).
  - Per-install `.dropdata` files and mod ledgers (`pot`): `#[serde(default)]` is NOT enough. Never remove or rename
    a field, and know that adding one breaks older builds reading the file. This has lost user data.
- **A new setting must be added to `ALLOWED_SETTINGS_KEYS`** in `src-tauri/src/settings.rs`, or saving it is silently
  thrown away.
- **No `.unwrap()` / `.expect()` on the startup path.** It shows up as "the app opens and closes instantly".
  Return an error instead.
- **Two surfaces: desktop AND Big Picture (BPM).** Any UI change must cover both, or say clearly which one it skips.
  In BPM:
  - every clickable thing must be registered with the focus system, or a controller can't reach it;
  - a modal must take the input lock when it opens and release it on close AND on unmount (a leaked lock kills the
    controller; this has shipped four times). Prefer focusable rows over a new modal;
  - in lists that change while open, keep focus on the item's identity, not its index;
  - never remove the ten console themes or turn the purples in the palette into blue.
  Details: `docs/map/big-picture.md`.
- **Images** use `objectImageUrl()` (works in `pnpm tauri dev` too; `object://` from `useObject()` doesn't load from
  the dev server), never `serverUrl()`.
- **Do not run `cargo clippy --fix`.** It breaks code for the other OS (Windows vs Linux `cfg` blocks).
- **Never stage or commit `accounts.json`.**
- **Player-facing text:** keep it plain, no em-dashes, and never mention a button or menu item that doesn't exist
  (search for it first). The owner rewrites wording, so list every new string with file:line in your notes.
- **Errors must be visible.** A failed fetch needs its own error state with Retry, never the same look as "empty".
  If code knows why something failed, show it.

## Check your work (keep output short)
The `exit=` line is the real result: `tail` always succeeds, so without it a failed check looks like a pass.
Commands below are bash. In PowerShell replace `| tail -N; echo "exit=${PIPESTATUS[0]}"` with
`| Select-Object -Last N; "exit=$LASTEXITCODE"`.
Rust, from `src-tauri/`:
```
cargo +nightly-2026-04-20 check --all-targets 2>&1 | tail -60; echo "exit=${PIPESTATUS[0]}"
cargo +nightly-2026-04-20 test -p <crate> <filter> 2>&1 | tail -25; echo "exit=${PIPESTATUS[0]}"
```
Frontend, from `main/`:
```
pnpm exec nuxt typecheck 2>&1 | tail -40; echo "exit=${PIPESTATUS[0]}"
node --test tests/<name>.test.ts        # pass FILES, never the folder
```
A pass needs `exit=0` AND a summary line that shows work was done (for tests: more than 0 tests ran; a filter that
matches nothing "passes" with 0 tests).
Code for `cfg(windows)` only compiles on Windows: say so if you couldn't check it.

## When you finish
Write in `tasks/current.md`: what changed (files), how you checked it, what you could NOT check, new player-facing
strings with file:line. Say "compiles, untested on device" when that is the truth.
