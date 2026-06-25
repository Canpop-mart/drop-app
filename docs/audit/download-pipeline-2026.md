# Download + Install Pipeline Audit — 2026

Scope: the Tauri desktop client's download/install pipeline —
`src-tauri/download_manager/` (the queue/worker crate) and
`src-tauri/games/src/downloads/` (the game download agent). Triggered by the
**LWIW incident**.

## The incident

A user imported "Little Witch in the Woods" — a **62 MB partial archive** of a
game whose manifest describes **~660 MB**. The upstream source was incomplete.
Drop downloaded the 62 MB, **marked the install `Installed`, and let the user
launch it.** Unity then failed because ~600 MB of asset files were never on
disk.

### Root cause

`GameDownloadAgent::validate()` in `games/src/downloads/download_agent.rs` was
a **stub**: the entire body was commented out and it returned `Ok(true)`
unconditionally. The download manager (`download_manager_builder.rs`) gates the
transition to `Installed` behind `validate()` returning `Ok(true)` — but since
validation always said "yes", **any** set of bytes on disk was accepted as a
complete install. There was no check that on-disk files matched the manifest.

## Findings

| # | Area | Status before | Action |
|---|------|---------------|--------|
| 1 | **Post-install validation** | **Stub — always `Ok(true)`** | **FIXED** — real validation added |
| 2 | Chunk verification | Working | Confirmed — no change |
| 3 | Resume / retry | Working | Confirmed — no change |
| 4 | Queue persistence | Partial | Documented — see Deferred |
| 5 | Progress accuracy | Working | Confirmed — see notes |
| 6 | Error handling | Mostly good | Improved — new error variant |
| 7 | Concurrency | Sound | Confirmed — no change |
| 8 | Monolith check | Acceptable | New `validate.rs` seam extracted |
| 9 | Depot downloads | Same path | Confirmed — see notes |

### 1. Post-install validation — FIXED (the priority)

New module **`games/src/downloads/validate.rs`** with
`validate_install(dl_info, install_dir) -> ValidationResult`.

`ValidationResult` is structured: `Valid` or
`Incomplete { missing: Vec<MissingFile>, mismatched: Vec<MismatchedChunk> }`.

**Why this design.** The Drop manifest (`droplet_rs::manifest`) stores
per-**chunk** SHA-256 checksums over the *decrypted* chunk payload. There are
**no per-file hashes**. A chunk lists `FileEntry` slices (`filename`, `start`,
`length`). The downloader writes decrypted plaintext straight to disk, so the
bytes on disk for a file region are exactly what the chunk hash was taken over.
Validation therefore works per chunk:

1. **File presence + size.** For every `FileEntry`, confirm the file exists and
   is at least `start + length` bytes. A short/missing file is the LWIW
   signature — recorded in `missing` immediately. (The 62 MB LWIW install fails
   here loudly: hundreds of asset files are absent or truncated.)
2. **Chunk checksum.** If all file regions are present, re-read those exact
   regions in manifest order, SHA-256 them, and compare to
   `chunk_data.checksum`. A mismatch (corruption, or a wrong-but-present file)
   is recorded in `mismatched`.

`validate()` in the agent now calls this and:

- `Valid` → returns `Ok(true)` → manager calls `on_complete()` → `Installed`.
- `Incomplete` → demotes the game to `PartiallyInstalled` (so the user can
  retry/resume) and returns
  `Err(ApplicationDownloadError::ValidationFailed(summary))`. The summary names
  the missing files and bad chunks (e.g. *"install incomplete: 312 file(s)
  missing or incomplete: assets/resources.assets (missing), ..."*).

`Err` — not `Ok(false)` — is deliberate. In the manager loop `Ok(false)` means
"re-run the download"; for a genuinely incomplete upstream that would loop
forever. `Err` surfaces a clear message via the existing `download_error`
frontend event and aborts.

The transition to `Installed` (`on_game_complete` in `games/src/library.rs`) is
unchanged but is now **only reachable when `validate()` returns `Ok(true)`**.

### 2. Chunk verification — already correct

`download_logic.rs::download_game_chunk` (lines ~173-176) decrypts each chunk
and verifies its SHA-256 against `chunk_data.checksum` as it lands; a mismatch
returns `ApplicationDownloadError::Checksum`, which the retry loop re-attempts.
No change needed. (`validate.rs` re-checks the same hashes post-hoc to catch
post-download corruption / partial writes from a crash.)

### 3. Resume / retry — already correct

- **Retry:** `download_agent.rs::run()` retries each chunk up to `RETRY_COUNT`
  (3) with exponential backoff (1s, 2s, 4s).
- **Resume:** completed chunk IDs are persisted to the on-disk `.dropdata`
  file (`drop_data.rs`). On a fresh `GameDownloadAgent`, `run()` loads them and
  `skip()`s done chunks. A chunk interrupted mid-write is simply *not* marked
  complete, so on resume it is re-downloaded from `file.start` and overwrites
  the partial bytes — **no corruption**. An interrupted install lands in DB
  status `PartiallyInstalled`.

### 5. Progress accuracy

`progress_object.rs` / `rolling_progress_updates.rs` are sound. The LWIW
download "completing near-instantly" was **not** a progress bug — with only
62 MB present the download genuinely finished fast; the real defect was that
nothing then checked the result. Per-chunk counters are reset on retry
(`download_logic.rs` lines ~48-54) to prevent overshoot. No change.

### 6. Error handling — improved

`ApplicationDownloadError` is a proper enum (no `String` errors).
`DiskFull`, `Communication`, `IoError`, `Checksum`, `Lock`, `ChannelBroken`
are distinct and actionable. Added **`ValidationFailed(String)`** for
post-install validation failures, with a `Display` impl that names the problem
and tells the user to re-download. Removed a now-obsolete
`#[allow(dead_code)]` on the `Checksum` variant.

### 7. Concurrency

`download_thread_control_flag.rs` is an `Arc<AtomicBool>` — pause/resume/cancel
are race-free. The builder's thread-draining logic
(`drain_previous_download`, the `Go`-signal state machine) is already
carefully commented and handles the spawn/pause window correctly. No change.

### 8. Monolith check

`download_agent.rs` (636 lines) and `download_manager_builder.rs` (474) are
large but cohesive. The one clear seam — validation — has been extracted into
its own `validate.rs` (was inline-commented dead code). The download manager
loop in `download_manager_builder.rs` was made explicit: the
download→validate cycle is now a **bounded** `for` loop (`MAX_DOWNLOAD_PASSES`
= 3) instead of an unbounded `loop {}` that could spin forever if validation
ever returned `Ok(false)`. No further decomposition done — it would be churn
without a payoff.

### 9. Depot downloads

`depot_manager.rs` selects the fastest depot *endpoint* per chunk; it is a
transport-routing layer, not a separate download path. All chunks — depot or
not — go through `download_game_chunk` and the same `validate.rs`. Both paths
get identical validation. No change.

## Changes made

| File | Change |
|------|--------|
| `games/src/downloads/validate.rs` | **New.** `validate_install`, `ValidationResult`, `MissingFile`, `MismatchedChunk`. |
| `games/src/downloads/mod.rs` | Register `validate` module. |
| `games/src/downloads/download_agent.rs` | `validate()` rewritten from stub to real validation; `setup_validate()` un-commented; `DownloadInformation` fields made `pub`. |
| `download_manager/src/error.rs` | New `ValidationFailed(String)` variant + `Display`; dropped stale `#[allow(dead_code)]`. |
| `download_manager/src/download_manager_builder.rs` | Download/validate loop bounded (`MAX_DOWNLOAD_PASSES`); loud `[phase]` logging at download-start / validation / install-complete / pause. |

## Verification

`cargo check` passes for `download_manager`, `games`, and the root `drop-app`
crate. Only pre-existing dead-code / unused-`#![feature]` warnings remain — no
new errors or warnings introduced.

### Acceptance criteria

- **Fewer/smaller files than manifest → not `Installed`.** A short/missing file
  is caught by the file-size check in `validate_install`; `validate()` returns
  `Err(ValidationFailed)`, the game is demoted to `PartiallyInstalled`, and the
  frontend receives a `download_error` naming the missing files. The LWIW
  62 MB import would now fail loudly instead of launching.
- **Interrupt + restart → resume or clean re-queue, no corruption.** Confirmed
  via the `.dropdata` completed-chunk persistence; partial chunk writes are
  re-downloaded and overwritten.
- **Chunk checksums verified.** Confirmed (download time + validation time).

## Deferred

- **Queue auto-persistence / auto-resume.** The in-memory `download_queue`
  (`Queue`, a `VecDeque<DownloadableMetadata>`) is **not** serialized, so the
  *ordering* of a multi-game queue and automatic resume-on-launch are lost on
  restart. This is **not** a corruption risk: each interrupted download is
  individually crash-safe (DB `PartiallyInstalled` + on-disk `.dropdata`), and
  the user can resume it explicitly via the existing `resume_download` command.
  A true serialized auto-resuming queue would touch app startup, agent
  reconstruction, and persistence of `UserConfiguration` per queued item —
  larger than this audit's scope and riskier than the current explicit-resume
  flow. Recommended as a dedicated follow-up.
- **Per-file checksums.** The manifest only carries per-chunk hashes. Validation
  is per-chunk, which is correct and complete, but a corrupt file shared across
  chunks is reported once per affected chunk. A future manifest format with
  per-file hashes would let validation pinpoint and re-fetch a single file.
- **Streaming/incremental validation.** `validate_install` runs after the full
  download. For very large installs a validate-as-you-go mode would surface
  failures sooner; deferred as an optimization.

## Out of scope (unchanged)

Server-side import pipeline (separate repo), the download wire protocol, and
compatibility testing (`compat-2026.md`).
