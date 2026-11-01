# Glossary

Terms used across both repos and the docs.

| Term | Meaning |
|---|---|
| **Drop** | The self-hosted game platform. Server = `drop-server`, desktop client = `drop-app`. This is the owner's fork of Drop-OSS. |
| **Live / dev** | Live = the NAS server everyone uses. Dev = the copy on the owner's PC (`localhost:4000`). See `../drop-server/docs/runbooks/dev-environment.md`. |
| **`main` / `dev` branches** | `main` = released/live. `dev` = finished work waiting for approval. Feature work goes on short branches. |
| **BPM** | Big Picture Mode: the full-screen controller UI, used on the Steam Deck. Pages under `main/pages/bigpicture/`. |
| **Desktop** | The normal mouse-and-keyboard layout of the client. |
| **Dual-surface rule** | Every UI feature must work on desktop AND in BPM, or the gap must be stated. |
| **Focus system** | BPM's controller navigation. A control must be registered with it or the controller can't reach it. |
| **Input lock** | BPM mechanism a modal takes so the D-pad only moves inside it. Must be released on close and on unmount. |
| **Console themes** | The ten BPM themes (Steam, Xbox, Wii, PS2, DS, Dreamcast, GameCube, PSP, Game Boy, SNES). Keep all of them. |
| **Gamescope / Game Mode** | The Steam Deck's console session. No mouse. Desktop Mode is the Deck's normal Linux desktop. |
| **Proton / Wine prefix** | How Windows games run on Linux. Each game has a prefix folder holding its fake Windows drive, including saves. |
| **UMU** | The launcher Drop uses to run games under Proton on Linux. |
| **`server://`** | Client protocol that forwards requests to the Drop server with the user's auth attached. |
| **`object://`** | Client protocol that serves game images and media through a memory and disk cache. In code use `objectImageUrl()`. |
| **Tauri command** | A Rust function the frontend calls with `invoke("name")`. Must be registered in `src-tauri/src/lib.rs`. |
| **`drop.db`** | The client's app database: RON text in a versioned envelope, AES-encrypted. New struct fields need `#[serde(default)]`. |
| **`pot`** | Binary format of per-install `.dropdata` files and mod ledgers. Never remove or rename a field; `serde(default)` alone isn't enough. |
| **Game / version** | A game has one or more versions (builds). Launch configurations, files and manifests belong to a version. |
| **Manifest** | The server's list of a version's chunks and checksums. Must be regenerated when files in a library folder change. |
| **Revision** | A republish of the same version after its folder changed on the server. Clients update in place. |
| **Baseline** | The client's record (`.drop-baseline.json`) of what it installed, used to tell pack changes from player changes. |
| **Fingerprints** | Per-file SHA-256 hashes the server records per revision ("Record fingerprints" in admin). |
| **Mirrored folders** | Per-version setting: folders the client makes match the server exactly, offering to remove extra files. |
| **Delta version** | A version that only ships changed files on top of another. Never updated in place. |
| **Depot / torrential** | The Rust service that actually serves game file chunks. A submodule of drop-server. |
| **Library** | A folder on the server holding games, mods or emulation content. |
| **Mod ledger** | `.moddata` file recording which files a mod installed, so it can be removed cleanly. |
| **GBE / Goldberg / gbe_fork** | Steam API emulator placed next to a game's Steam DLL. Gives achievements, saves and LAN co-op. |
| **`steam_settings`** | GBE's config folder. The server writes it into library folders; the client also writes one next to the DLL at launch. |
| **RA** | RetroAchievements: achievements for emulated games, through RetroArch. |
| **Ludusavi** | Tool Drop uses to find PC game save files for cloud saves. |
| **Conflict (saves)** | Both the local and the cloud copy changed since the last sync; the player chooses. |
| **ZeroTier** | Virtual network used for co-op rooms and Archipelago. The server runs a controller. |
| **Co-op room** | A temporary ZeroTier network so players can see each other in LAN-mode games. |
| **Archipelago (AP)** | Multiworld randomizer. Drop collects player YAMLs and shows the connect address. |
| **Addon** | An external website integrated with Drop (configured in `DROP_ADDONS`). First one: the NCAA 14 league. |
| **NCAA 14 league** | The owner's league site for NCAA Football 14 dynasty saves. Separate repo and container. Drop hands saves in and out. |
| **Scope / ACL** | A permission on a token, like `store:read` or `addons:manage`. Routes check them. |
| **Task** | A server background job (scheduled or run from Admin > Tasks). |
| **Placeholder string** | Player-facing text written by an agent; the owner rewrites it. Always list it with file:line. |
