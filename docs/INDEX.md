# drop-app docs: what to read when

Open ONE map for your area. Each map lists its files, flows, gotchas and tests. The server repo's maps are in
`../drop-server/docs/INDEX.md`.

## Always useful
| Doc | Read when |
|---|---|
| `AGENTS.md` | Every session (rules and check commands). |
| `docs/workflow.md` | Starting work, or deciding when to stop a session or hand off to a bigger model. |
| `docs/UPCOMING.md` | Picking a task. The one list of planned work and known bugs for both repos. |
| `docs/glossary.md` | A term you don't know (BPM, GBE, revision, baseline, ledger...). |
| `docs/runbooks/diagnostics.md` | Debugging: logs, crash files, log tags, the Steam Deck over SSH. |
| `../drop-server/docs/runbooks/dev-environment.md` | Running the dev server and dev client; dev vs live. |

## Area maps (`docs/map/`)
| Map | Covers |
|---|---|
| `docs/map/app-shell-protocols.md` | The Tauri app crate: startup, command registration, `server://` and `object://`, logging, panic hook. |
| `docs/map/local-db-settings.md` | `drop.db` (encrypted RON), settings and `ALLOWED_SETTINGS_KEYS`, data folders, settings pages. |
| `docs/map/desktop-frontend.md` | The desktop UI: layouts, page index, game detail page by section, key composables, frontend tests. |
| `docs/map/big-picture.md` | BPM as a system: focus registration, input lock, modal patterns, themes. Read before ANY BPM change. |
| `docs/map/big-picture-pages.md` | Page-by-page index of BPM, and the BPM game page broken into sections with line ranges. |
| `docs/map/downloads-install.md` | Download queue, install, verify/repair, resume, folder import, uninstall. |
| `docs/map/in-place-updates.md` | Updating an install in place: three-way plan, review, staging, recovery. |
| `docs/map/mods.md` | Mod install/uninstall/update, ledgers, originals backup. |
| `docs/map/launch-process.md` | Play button to running process: launch configs, Proton/UMU, prefixes, failure dialogs. |
| `docs/map/emulation.md` | Emulators (RetroArch, Switch), config files, controllers, console pages. |
| `docs/map/cloud-saves-playtime.md` | Cloud-save sync, conflicts, backups, history; playtime reporting and queues. |
| `docs/map/achievements.md` | Goldberg unlock detection, reporting, toasts, RetroAchievements linking, reset. |
| `docs/map/multiplayer.md` | Co-op rooms, the ZeroTier daemon, Archipelago (client side). |
| `docs/map/streaming.md` | Remote play: Sunshine host, Moonlight client, pairing, stream sessions. |
| `docs/map/addons.md` | Addon shop and addon play actions (NCAA 14 league save check-out and check-in). |
| `docs/map/build-release.md` | Builds, CI, the release checklist (version in 4 files, verifying `latest.json`). |

## Not maps
- `README.md`: the fork's feature overview for humans. Parts are older than the code; trust the maps.
- `changelog.md`: release notes.
- `tasks/TEMPLATE.md`: copy to `tasks/current.md` for each task (that file is not committed).
- `scripts/check-docs.mjs`: `node scripts/check-docs.mjs` checks every path mentioned in these docs still exists.
