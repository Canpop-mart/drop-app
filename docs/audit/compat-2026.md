# Compatibility Testing — Audit (2026)

Audit of Drop's per-game compatibility-testing feature. Scope was the
drop-app (Tauri client) side; the server side was inspected only far
enough to establish where results are stored.

**Outcome: no code rewrite.** The audit brief was written against
incorrect assumptions about the architecture (see "Brief corrections"
below). The feature is working as designed. This document records what
the feature actually is, why it stays dev-gated, and the real
follow-ups.

## What the probe actually does

`src-tauri/src/compat.rs` (687 lines) is the test orchestrator. A compat
test is **not** a metadata probe — `start_compat_test`:

1. Launches the real game binary through `PROCESS_MANAGER.launch_process`
   — the same code path as the "Play" button.
2. Watches the process for up to `DEFAULT_TIMEOUT_SECS = 45` seconds.
3. Reads the per-launch Wine/Proton debug log.
4. Classifies the outcome with crash-signature regexes
   (`extract_crash_signature`, `extract_render_failure_signature`).
5. Kills the process (`ensure_killed`, up to 5 retries).
6. POSTs the result to drop-server.

Implication: the probe is **slow** (tens of seconds per game), **launches
executables**, and **stacks game windows** if run in bulk. It is not
something that can run silently in the background or be triggered
casually.

## The three Rust modules are layered, not duplicated

| File | Lines | Responsibility |
|------|-------|----------------|
| `src-tauri/client/src/compat.rs` | 185 | UMU launcher discovery |
| `src-tauri/process/src/compat.rs` | 391 | Proton path discovery; `fetch_proton_paths` / `install_umu` / `diagnose_launch_environment` commands |
| `src-tauri/src/compat.rs` | 687 | Compat-test orchestrator |

There is **no overlap**. The original audit brief assumed these were
redundant copies and asked for consolidation — that would have merged
three unrelated concerns. No consolidation is warranted or safe.

## Result persistence is server-side, by design

Compat results do **not** live in local Tauri storage. They live in
drop-server:

- `prisma/models/compatibility.prisma` → `GameCompatibilityResult`
- migration `20260425170000_add_game_compatibility_results`
- `server/api/v1/client/compat/results.{get,post}.ts`
- `server/api/v1/client/compat/library-summary.get.ts`
- `server/api/v1/client/compat/work/next.get.ts` — work queue
- `server/api/v1/admin/compat/summary.get.ts`
- `pages/admin/compat.vue`, `components/CompatBadge.vue`

On the client, `main/composables/use-compat-summary.ts` fetches
`GET /api/v1/client/compat/library-summary`. This is a deliberate
crowdsourced design — results are shared across devices and users, and
the server hands out a work queue so the testing load can be
distributed. Forking persistence to a local SQLite/JSON store (as the
original brief proposed) would break the existing multi-device summary
the UI already renders.

## Surfaces and gating

| Surface | Location | Gating |
|---------|----------|--------|
| `CompatBatchPanel.vue` | `pages/library/index.vue` | `v-if="devMode.enabled.value"` |
| `CompatTestButton.vue` | `pages/library/[id]/index.vue` | `v-if="devMode.enabled.value"` |
| `GameCompatPanel.vue` | `pages/library/[id]/index.vue` | un-gated (read-only display of cached results) |

Gating is already **consistent**: the two surfaces that *trigger* tests
are dev-gated; the one that only *displays* cached results is not. This
is the correct split.

`main/pages/settings/compat.vue` is unrelated to compat testing — it is
the **Proton Compatibility Layer** manager (UMU launcher install, Proton
layer discovery and selection). It contains no compat-test controls and
must not be touched by compat-testing work.

## Decision: keep the feature dev-gated

The test triggers stay behind `devMode`. Rationale:

- The probe launches real game binaries for up to 45 seconds each. That
  is inherently disruptive (window focus theft, resource use) and not
  appropriate to expose to all users as a one-click action.
- A batch run over a large library would launch every game in sequence.
  Even with the server work-queue throttling it, this is a power-user
  operation, not a default-on feature.
- The read-only `GameCompatPanel` is already un-gated, so ordinary users
  still *see* crowdsourced compat status on game pages — they just don't
  trigger tests themselves.

### What would change the decision

To un-gate the test triggers, the probe would need a **fast,
non-launching pre-check tier** — e.g. PE/ELF header inspection, required
runtime DLL presence, Proton-layer availability — that produces a
"likely / unknown" signal in milliseconds without starting the game. The
current 45-second binary launch would remain an explicit, opt-in "deep
test". Only the fast tier should ever be un-gated.

## Brief corrections

The original audit brief contained three factual errors, corrected here
for the record:

1. **"There is no server-side compat code."** False. The entire result
   model, work queue, and admin summary live on the server. (The error
   originated from a recon `find` that filtered on `*compat*` filenames;
   the endpoints are `library-summary.get.ts`, `results.get.ts`, etc.,
   which do not contain "compat" in the filename.)
2. **"The three Rust crates are duplicated."** False. They are three
   distinct, correctly layered concerns.
3. **"`CompatBatchPanel` was removed from the library page."** False. It
   is still present at `pages/library/index.vue`, dev-gated.

## Follow-ups (not done — out of scope for this audit)

1. **Fast pre-check tier** (see "What would change the decision"). This
   is the single highest-value improvement and the prerequisite for
   un-gating.
2. **Result invalidation.** Confirm server-side `GameCompatibilityResult`
   rows are keyed on / invalidated by game version + OS + arch +
   Proton/Wine version. A result that claims "compatible" after a major
   version bump is worse than no result. (Server-side change — track in
   drop-server.)
3. **Batch run UX.** If the batch panel is ever un-gated, replace the
   iterate-and-launch model with a queue that surfaces progress in a
   dock notification rather than blocking the library page.

## Files inspected

- `src-tauri/src/compat.rs`, `src-tauri/client/src/compat.rs`,
  `src-tauri/process/src/compat.rs`
- `main/components/CompatBatchPanel.vue`, `CompatTestButton.vue`,
  `GameCompatPanel.vue`
- `main/composables/use-compat-summary.ts`
- `main/pages/settings/compat.vue`
- drop-server: `prisma/models/compatibility.prisma`,
  `server/api/v1/{client,admin}/compat/*`

No source files were modified.
