# Emulation-integration audit — 2026

Audit + streamline of the emulation layer of the Drop Tauri desktop client
(`drop-app`). Scope: `src-tauri/remote/src/retroarch.rs` (2622 lines, the
single largest file in the codebase) and `src-tauri/remote/src/goldberg.rs`
(1266 lines). No other `remote`-crate files were touched — they were just
refactored by the prior remote-comms work.

## TL;DR

- `retroarch.rs` (2622 lines) → a `retroarch/` module of 9 files, largest 490.
- `goldberg.rs` (1266 lines) → a `goldberg/` module of 5 files, largest 278.
- ROM→core mapping is now genuinely data-driven: one table, `EXTENSION_CORE_MAP`,
  with per-extension metadata; the high-res-3D and Dolphin heuristics are
  *derived* from it instead of being three drifting hand-written `match` arms.
- RetroAchievements HTTP (`fetch_ra_credentials`, `fetch_ra_hashes`) now routes
  through the shared retrying `remote_request` helper.
- Goldberg's real state — implemented readers vs the missing detection feature
  — is documented precisely below and in `goldberg/mod.rs`.
- `cargo check` is clean across the whole workspace (pre-existing
  unused-`#![feature]` warnings only — not introduced here).

---

## 1. `retroarch.rs` decomposition

The old file was a 2622-line grab-bag: HTTP, config-file editing, a CRT shader
embedded as string constants, BIOS heuristics, controller remaps, quality
presets and ROM→core resolution all interleaved. Split along cohesive seams
into `src-tauri/remote/src/retroarch/`:

| File             | Lines | Responsibility |
| ---------------- | ----- | -------------- |
| `mod.rs`         | 490   | Module docs, public re-exports, `RetroArchInfo`, and the `configure_retroarch_for_game` orchestrator (drives every sub-module in order). |
| `discovery.rs`   | 94    | `is_retroarch` install detection; AppImage portable-`$HOME` path resolution. |
| `cfg.rs`         | 90    | `retroarch.cfg` patch primitives (`patch_retroarch_cfg{,_with_deletions}`, `path_to_cfg`, `extract_cfg_key`). |
| `cores.rs`       | 285   | The data-driven `EXTENSION_CORE_MAP`, ROM→core resolution, ISO disc-header sniffing, render-class heuristics. |
| `bios.rs`        | 207   | BIOS/firmware detection + auto-placement, data-driven `BIOS_SPECS` table. |
| `controllers.rs` | 260   | Controller layout, hotkey bindings, per-core `.rmp` remap files, stale-key list. |
| `presets.rs`     | 223   | Quality-preset + aspect-ratio config (frontend `retroarch.cfg` half + per-core options half). |
| `shaders.rs`     | 471   | CRT shader: bundled GLSL/slang sources, system-shader discovery, auto-apply preset writing. |
| `ra.rs`          | 283   | RetroAchievements: Connect credentials + ROM-hash verification. |

The orchestrator `configure_retroarch_for_game` previously inlined ~780 lines
of config-key insertion. It is now ~110 lines that delegate to small
override-group helpers (`apply_path_overrides`, `apply_save_overrides`,
`apply_video_input_overrides`, `apply_cheevos_overrides`, …), each documenting
the keys it owns. The public API is unchanged — `mod.rs` re-exports
`configure_retroarch_for_game`, `resolve_core_for_rom`, `RetroArchInfo`,
`RACredentials`, `check_rom_hash`, `hash_rom`, `RAHash*`, `RomHashStatus`,
`EXTENSION_CORE_MAP` — so the `process` and root crates compile untouched
(verified).

## 2. ROM extension → core mapping — verdict

**Before:** `EXTENSION_CORE_MAP` was a `&[(&str, &[&str])]` tuple slice — a
table, so technically data-driven for *core lookup*. But the two related
classification questions — "does this ROM use a high-res 3D core?"
(`rom_implies_high_res_3d_core`) and "does it use Dolphin?"
(`rom_uses_dolphin_core`) — were each a **separate hand-written `match` over
extensions**. Three lists of the same extensions, free to drift. They had
already partly drifted (e.g. the high-res list's commentary didn't match the
core table for some Sega entries).

**After:** `EXTENSION_CORE_MAP` is `&[ExtensionEntry]`, where each row carries
`ext`, `cores` (preference-ordered) **and** a `RenderClass` (`Retro2D` /
`HighRes3D`). The heuristics are now derived:

- `rom_implies_high_res_3d_core` ≡ `entry_for_ext(ext).render == HighRes3D`.
- `rom_uses_dolphin_core` ≡ the row's only candidate core is `dolphin`.

Adding a platform is **one line**; the shader and controller-device behaviour
follow automatically. No `match` to forget to update.

**Correctness / completeness review of the table:**

- Coverage spans Nintendo (GB/GBC/GBA/NES/FDS/SNES/N64/NDS/3DS/GC/Wii), Sony
  (PS1/PSP/PS2), Sega (Genesis/SMS/GG/32X/SegaCD/Saturn/Dreamcast), Atari
  (2600/7800/Lynx) and assorted others (PCE, NeoGeo Pocket, WonderSwan,
  Vectrex, ColecoVision). Solid for a client emulator launcher.
- Ambiguous extensions (`.iso`, `.chd`, `.bin`, `.cue`) are handled correctly:
  `.iso` is disambiguated by reading the disc header magic (GameCube
  `0xC2339F3D` @ `0x1C`, Wii `0x5D1C9EA3` @ `0x18`) before a core is chosen,
  so a PS2 ISO is no longer mis-loaded into Dolphin. `.chd`/`.bin`/`.cue` fall
  through to a preference-ordered core list — acceptable, as the cores
  themselves reject content they can't run.
- `.m3u` multi-disc playlists resolve via their first disc entry.
- **Deferred (not a regression):** `.bin` is marked `HighRes3D` because the PS1
  CD path dominates, but a raw Genesis `.bin` is 2D — it would get a
  resolution-tolerant CRT shader instead of the stronger scanline one. Minor
  cosmetic-only mismatch; documented, not fixed (changing it risks the far
  more common PS1 case). See "Deferred items".

## 3. Core management

Cores are **not** downloaded by this module. `resolve_core_for_rom` discovers
an already-present `*_libretro.{dll,so}` in `<emu_root>/cores/` and returns its
path; the actual `-L <core>` argv injection happens in the `process` crate
(`launch_emulator.rs`). RetroArch itself (or the server-side install payload)
provisions cores. No retry/verification logic exists here because there is no
download here — noted so a future "core updater" feature knows this is green
field. Config writes survive a core update: `global_core_options = true` plus
`clean_stale_per_core_overrides` strip the per-core `.opt`/`.cfg` files
RetroArch writes that would otherwise outrank Drop's settings.

## 4. Launch-command construction — injection safety

Launch-command assembly lives in the `process` crate, **out of audit scope**,
but was reviewed for the verdict: it is argv-based
(`Command::new(exe).arg(...)`), never a shell string — `{rom}` placeholder
substitution and `-L <core>` appends go into discrete `args`, so a ROM path
with spaces or shell metacharacters cannot be misinterpreted. The one process
spawn *inside* scope — `hash_rom` invoking the `RAHasher` CLI — is likewise
discrete-argv (`.arg(console_id).arg(rom_path)`); a comment now records that.
**No injection issue found.**

## 5. Config / preset application

`UserConfiguration` (controller layout, quality preset, aspect ratio, CRT
toggle) maps onto two files: frontend video settings in `retroarch.cfg` and
per-core options in `retroarch-core-options.cfg`. Both are **patched, not
rewritten** — only Drop-managed keys change, user edits survive.

- **Idempotent:** `patch_retroarch_cfg` re-runs to the same output; preset
  helpers `insert` a fixed key set every launch.
- **Survives a core update:** see §3.
- **Controller layout:** Nintendo A↔B / X↔Y swap uses `.rmp` remap files
  (loaded *after* SDL2 autoconfig, so they reliably win) rather than
  `retroarch.cfg` keys (loaded *before* autoconfig, so autoconfig overrides
  them). Switching layout cleans up stale `.rmp` files. This behaviour was
  preserved verbatim — only reorganised into `controllers.rs`.

## 6. Error handling

`retroarch.rs` is pre-launch *local file I/O* plus two optional server fetches.
The file-I/O functions deliberately **do not** return `Result` — a missing
BIOS or an unwritable shader dir must not abort a launch; they `warn!` and
continue, surfacing user-facing problems via `RetroArchInfo.bios_warnings`.
That is the right design and was kept.

The streamline fix is the **HTTP path** (next section): the two server calls
now produce the crate's typed `RemoteAccessError` taxonomy internally, instead
of the old hand-rolled `make_authenticated_get` + `response.json()` +
ad-hoc-string handling. Their *public* signatures still return
`Option`/`RomHashStatus` because RA integration is non-blocking — a failed
fetch degrades to "no RA data", logged, never fatal.

## 7. HTTP routed through `remote_request`

Per the task and `CLAUDE.md`, server calls in scope now go through the unified
retrying helper:

- `ra::fetch_ra_credentials` — was `make_authenticated_get(url)` then a manual
  `response.status()` check then `response.json::<RACreds>()`. Now
  `remote_request::<RACreds, _>(RemoteRequest::get(url))` — gets retry +
  backoff + per-attempt fresh JWT + classified errors for free.
- `ra::fetch_ra_hashes` — same conversion to `remote_request::<RAHashesResponse, _>`.

**`serde_json` constraint honoured:** the `remote` crate has no `serde_json`.
The one inline response type that needed it (`RACreds`) is an inner
`#[derive(Deserialize)] struct` local to the function; the GET helper carries
no body (`RemoteRequest::get` uses the crate's `NoBody`), so nothing in
`retroarch/` calls `serde_json`. (`goldberg/` *does* use `serde_json` — but
only to parse local emulator save *files* on disk, which is unrelated to the
HTTP-helper constraint and unchanged from the original.)

## 8. Loud logging at boundaries

`[RETROARCH]` / `[RA-HASH]` / `[EMU]` / `[GBE-DIAG]` / `[ACH-GSE]` tags were
already present and are kept. Added explicit `info!` lines at the
config-write boundaries that previously logged only at `debug!`:

- `"Writing retroarch.cfg ({n} keys) to {path}"` before the main patch.
- `"Writing core options ({n} keys) to {path}"` before the core-options patch.

Launch / core-resolution / RA boundaries already logged loudly.

---

## Goldberg — real state (the documented gap)

`goldberg.rs` was decomposed into `src-tauri/remote/src/goldberg/`:

| File              | Lines | Responsibility |
| ----------------- | ----- | -------------- |
| `mod.rs`          | 254   | Module docs (incl. this gap), `SteamEmulator`/`EmulatorInfo` types, `gse_save_path`, unified `read_unlocks`/`read_earned`/`configure_saves_for_game`. |
| `discovery.rs`    | 90    | Steam API DLL recursive search; emulator-type detection. |
| `achievements.rs` | 237   | `GoldbergAchievement`, GBE `achievements.json` map/array parser, array→map migration, retrying file reader. |
| `sse.rs`          | 223   | SmartSteamEmu `steam_emu.ini` parsing + `achievements.ini`/`.json` reading. |
| `config.rs`       | 278   | Goldberg `configs.user.ini` writing; GBE runtime diagnostics. |

### What is actually implemented today

- **Emulator detection** — recursive Steam-API-DLL search, then Goldberg/GBE
  (`steam_settings/`) vs SmartSteamEmu (`steam_emu.ini`) discrimination.
- **Goldberg pre-launch config** — writes `local_save_path=./drop-goldberg`
  and `account_name=<display name>` into `configs.user.ini`.
- **Reading achievement files that already exist on disk** — the GBE
  map-vs-definitions-array parser with automatic array→map migration, and the
  SSE numbered-`achievements.ini` parser. These power `read_earned`, which the
  achievement poll loop (`remote/src/achievements.rs`) calls every 15 s in
  Goldberg mode and reports newly-earned achievements to the server.
- **GBE diagnostics** — `check_gbe_activity` looks for GBE log/crash/marker
  files to warn when the shipped DLL isn't really a Goldberg/GBE build.

### The gap — precisely

`CLAUDE.md`: *"Goldberg file-based achievement detection is NOT yet
implemented — currently relies on server-side sync only."*

Precise statement: there is **no push-based / file-watcher detection**. Drop
neither installs an unlock hook into the emulator nor watches
`achievements.json` for change events. "Detection" is purely the 15-second
**poll** in `achievements.rs` re-reading the file on a timer. The authoritative
achievement source is the **server-side sync** — on `session-end` the server
re-reads Steam / RetroAchievements and reconciles. This module's file reading
is a best-effort *supplement* to that, not a complete standalone client-side
achievement engine.

Per the task this gap is a **feature and explicitly out of scope** — it was
not implemented. It is now documented in the `goldberg/mod.rs` module header
so the next reader doesn't mistake the present poll-based reader for a finished
detection system.

### Consistency with the achievement flow

`goldberg::read_earned` returns `GoldbergAchievement { name, earned,
earned_time }`; `achievements.rs` maps each new unlock to an
`AchievementReportEntry { external_id, provider: "Goldberg", unlocked_at }`
and POSTs it (timestamps range-validated 2000–2100 against corrupt save
files). RA mode is mutually exclusive — `AchievementMode` picks RA *or*
Goldberg, never both. The decomposition preserved this contract exactly; no
behavioural change.

---

## Verification

- `cargo check -p remote` — clean (only the pre-existing
  `slice_concat_trait` unused-`#![feature]` warning).
- `cargo check` (whole `src-tauri` workspace) — **zero errors**. Pre-existing
  unused-`#![feature]` warnings in `database`/`games`/`process`/`download_manager`
  remain and are not from this work.
- The `process` and root crates consume the re-exported public API unchanged.

## Deferred items

1. **`.bin` render class.** Marked `HighRes3D` (PS1 CD dominates); a raw
   Genesis `.bin` would get a resolution-tolerant CRT shader rather than the
   stronger scanline one. Cosmetic only. A proper fix needs content sniffing
   (Genesis ROM header vs PS1 track layout) — deferred.
2. **libretro core updater.** No core download/verify/version logic exists in
   the client; cores are provisioned externally. If a "keep cores updated"
   feature is wanted, `cores.rs` is the home for it (green field — §3).
3. **Goldberg file-based achievement detection.** The real watcher/unlock-hook
   feature — out of scope here (a feature), documented above.
4. **RA-hash `patch_url`.** `RAHashEntry.patch_url` is parsed and carried but
   never acted on; ROM-patch application for hash matching is unimplemented.
   Out of scope — flagged for whoever owns RetroAchievements parity.
