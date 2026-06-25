# Big Picture Mode Audit — 2026

Scope: the gamepad / couch-UI surface of the Tauri client — `main/pages/bigpicture/`,
`main/components/bigpicture/`, and the BPM-specific composables (`focus-navigation.ts`,
`bp-audio.ts`, `gamepad.ts`, `iframe-controller.ts`).

Out of scope (covered by parallel audits): the Rust backend (`process/src/gamepad.rs`
is confirmed-dead — the live gamepad path is `composables/gamepad.ts`); the desktop
(non-BPM) frontend; `composables/game.ts` (refactored by a parallel audit — BPM pages
integrate with its reactive `Game` / registry-invalidation API as-is).

## Summary

| # | Area | Status before | Action |
|---|------|---------------|--------|
| 1 | **Gamepad scroll-to-top bug** | Stick at rest could stay pinned ≥ 0.3 forever | **FIXED** — axis heartbeat in `gamepad.ts` |
| 2 | **`bigpicture/library/[id].vue` monolith** | 3232-line single file (largest in repo) | **DECOMPOSED** — 3 composables + 1 component + 1 shared helper; page now 1983 lines |
| 3 | **`bigpicture/index.vue` monolith** | 2124 lines | **DECOMPOSED** — home-data composable; page now 1956 lines |
| 4 | **Audio-node leak in `bp-audio.ts`** | Stopped oscillators never disconnected | **FIXED** — `onended` disconnect in all 3 generators |
| 5 | **`focus-navigation.ts` (1303 lines)** | Cohesive focus engine | **AUDITED** — sound; intentionally not decomposed (see §5) |
| 6 | **`iframe-controller.ts` scroll** | `scrollBy` every 50ms on a phantom-pinned axis | **FIXED at source** by §1 — no change needed here |
| 7 | **`BpmAnimatedBackground` perf** | 664-line component, Steam Deck concern | **AUDITED** — pure CSS/GPU, not a CPU hog (see §7) |
| 8 | **Logic shared with desktop** | Markdown renderer inline; config logic duplicated | **PARTIALLY SHARED** — `render-markdown.ts` (see §8) |
| 9 | Dead code / type safety | `filteredPcSaves`, `pcSaveFileType`, `pcSaveFileColor` orphans | **REMOVED** during §2 |

## 1. The gamepad scroll-to-top bug (priority deliverable)

**Symptom (user-reported):** navigating BPM with the control stick randomly scrolls
to the top after a couple of seconds.

**Root cause.** `composables/gamepad.ts`'s `pollFrame()` axis loop was a *delta filter*:
it only wrote the cached `axes` value when the frame-to-frame change cleared
`AXIS_CHANGE_THRESHOLD` (0.05). When a stick decelerates back toward center, the
sub-threshold steps near rest are each below 0.05 and were all dropped. If the last
*recorded* value was still ≥ 0.3 and the stick physically settled on drift inside that
band, the cached value stayed pinned ≥ 0.3 **indefinitely**. The two axis consumers —
`focus-navigation.ts`'s stick-scroll poll (`~line 1258`) and `iframe-controller.ts`'s
scroll poll (`~line 101`) — both run `scrollBy` every 50ms whenever `axisValue() > 0.3`,
so the page ran to the top with no actual input.

**Fix.** Added an **axis heartbeat** to `pollFrame()`: the cached `axes` value is now
*unconditionally* refreshed to the current deadzone-filtered reading every frame
(`axes.set(name, filtered)` before the threshold check). `AXIS_CHANGE_THRESHOLD` now
only gates dev-log noise — never the cache. A settled stick therefore reads its true
value (≈ 0 after the 0.15 deadzone) within one frame.

**Why it doesn't break slow-stick navigation.** The heartbeat writes the *current*
reading, not zero. A stick deliberately held at, say, 0.5 still caches 0.5 every frame,
so `axisValue() > 0.3` stays true and slow scrolling/navigation works. The heartbeat
only stops *stale* values from outliving the input that produced them. It also cures
phantom left-stick navigation (`focus-navigation.ts:1252-1254`), which had the same
pinned-axis failure mode.

**Verification.** All three axis consumers read through `gamepad.ts`'s `axisValue()`,
which returns the cached `axes` map — so the single heartbeat fix covers every consumer.
`applyDeadZone` (threshold `STICK_DEAD_ZONE = 0.15`) correctly maps a settled stick to
0; both consumers' `0.3` thresholds sit well above it, so a settled stick reliably
suppresses `scrollBy`. `iframe-controller.ts` needed no change of its own.

## 2. `bigpicture/library/[id].vue` decomposition (headline deliverable #2)

The BPM game-detail page was a **3232-line** single-file component — the largest file in
the repo. It held the hero banner + action buttons, four content tabs, the full
remote-streaming flow, three save-data subsystems, the per-game emulator presets, the
options menu, the shelf picker, recommendations, ROM-hash verification, a markdown
renderer, and ten modals — all inline.

It is now a **1983-line** page that delegates its three big independent concerns to
composables and the heaviest tab to a component.

### Extracted composables — `composables/bigpicture/`

- **`use-bpm-game-streaming.ts`** (453 lines) — the entire receiver-side streaming flow
  (request a stream, poll for a host, auto-launch Moonlight, watch for session end) plus
  cross-device discovery (`streamableDevices` / `installableDevices`). Owns every
  `setInterval` and tears them all down in `dispose()`.
- **`use-bpm-game-saves.ts`** (637 lines) — all three save sources: emulator saves,
  cloud sync, and Ludusavi PC-game saves, plus the merged-view computed, per-PC-save
  cloud-sync status, and the pure formatters.
- **`use-bpm-game-config.ts`** (196 lines) — controller / quality / aspect-ratio / CRT
  presets with persistence and toast feedback.

These are the BPM analogue of the desktop `composables/game-detail/` set. They are kept
**separate** from the desktop ones (rather than merged) because the BPM versions add
concerns the desktop ones lack (CRT shader, gamepad-cycle ergonomics, the whole
streaming/saves surface) — see §8.

### Extracted component — `components/bigpicture/game-detail/`

- **`BpmGameSavesTab.vue`** (246 lines) — the entire Saves-tab body. Pure presentation:
  takes the `useBpmGameSaves()` return object as a prop and the page's focus-nav
  `registerAction` so its buttons join the page's `content` focus group.

### Shared helper — `composables/render-markdown.ts`

The page's inline `renderMarkdown` (lightweight markdown → HTML for game descriptions)
moved to a standalone pure module so it can be reused by the desktop game-detail page if
it ever needs rich descriptions.

### Dead code removed

`filteredPcSaves` (a `computed` shadowing `pcSaveGroups`), `pcSaveFileType`, and
`pcSaveFileColor` were unreferenced — dropped during extraction.

Behavior is identical: markup, classes, focus registration, and the streaming/saves
flows are unchanged — only the code's *home* moved.

## 3. `bigpicture/index.vue` decomposition

The BPM home screen was **2124 lines** — a ~940-line template with one per-console
`<template v-if="theme === '…'">` block for ~13 retro themes, plus a ~330-line script.

The cleanest seam was the **data layer**, extracted to
**`composables/bigpicture/use-bpm-home-data.ts`** (238 lines): the recently-played list
(`loadRecentGames`), the random-favorite spotlight pick (`pickRandomFavoriteSpotlight`),
and the `spotlightGame` / `otherGames` / `installedGames` computeds. This concern is
DOM-free and had no focus-registration coupling, so the extraction is low-risk. The page
is now **1956 lines**.

The per-theme markup was deliberately **not** split into 13 components: each theme block
is presentational and tightly bound to its own `focusedXxxIdx` tracking wired into
specific elements, so splitting it would be a large mechanical refactor with real
focus-regression risk for little structural gain — the *logic* (the part worth reusing)
is already isolated in the composable. Tracked as a deferred item.

`settings.vue` (1412) and `store.vue` (1109) were reviewed and left as-is: neither is a
2000+-line monolith, `settings.vue` is a flat list of small independent handlers with no
monolithic block, and `store.vue`'s one cohesive seam (bulk-select) is only ~125 lines.

## 4. Audio-node leak in `bp-audio.ts`

`bp-audio.ts` synthesizes every BPM UI sound via the Web Audio API. The three primitive
generators — `tone()`, `sweep()`, `noise()` — created `OscillatorNode` /
`AudioBufferSourceNode` + `GainNode` (+ `BiquadFilterNode`), called `.start()`/`.stop()`,
but **never disconnected them**. A stopped node stays wired into the graph
(`osc → env → out`) until the GC traces and reclaims it. Over a long BPM session that
generates hundreds of focus/select sounds, this accumulates a graph of dead nodes the GC
must walk — a real, if slow, leak on a Steam Deck.

**Fix.** Each generator now wires `node.onended` to `disconnect()` itself and its
downstream gain/filter, so the graph is reclaimed immediately when the sound finishes.

Audio settings were also reviewed and are sound: `enabled` is read from `localStorage`
and checked in every generator; mute / profile switching work correctly.

## 5. `focus-navigation.ts` — audited, not decomposed

The 1303-line spatial/focus-navigation engine was audited for the failure modes called
out in the brief:

- **Focus traps / unreachable elements.** `navigate()` does a within-group spatial cone
  search, then a cross-group fallback (skipped only under an explicit `restrictFocus`
  for modals). Grid groups fall through to spatial search so focus can *escape* a grid.
  No traps found.
- **Stale focus after a route change.** When the focused element unmounts, its
  unregister callback sets `currentFocused = null`. `navigate()` handles a null
  `currentFocused` by re-focusing the current group's first element, and every BPM page
  calls `autoFocusContent()` on mount. Stale focus therefore **self-heals** — not a bug.
- **`scrollBy` behavior.** Fixed at the source — see §1.
- **Timer cleanup.** All repeat / left-stick / hold timers are module-scoped and cleared
  in `destroy()`; gamepad subscriptions are tracked in `gamepadUnsubs` and unwired.

**Decision: not decomposed.** Unlike the BPM pages — which mixed *genuinely independent*
concerns — this file is a single cohesive concern. Its spatial engine, grid engine,
repeat timers, ownership-based input lock, and per-route focus snapshots are all
intertwined through shared module state (`groups`, `currentFocused`, `gridContexts`).
Splitting it would move code across files without producing independently-testable units
and would add cross-file coupling. It is left intact.

## 6. `iframe-controller.ts`

119 lines, single concern (forwards gamepad input to a `server://` iframe). Its
`scrollInterval` is correctly cleared in `onUnmounted`; the lock is ownership-based so a
new iframe page's lock can't be clobbered by an old page's teardown. The only issue —
the `scrollBy`-every-50ms firing on a phantom-pinned axis — is fixed upstream by §1.
With a settled stick now reading ≈ 0, `Math.abs(val) > 0.3` is false and no scroll
fires. No change needed.

## 7. `BpmAnimatedBackground` performance

664 lines, but **100% CSS** — zero JavaScript, zero `requestAnimationFrame`. Audited as
**performance-sound**, including for the Steam Deck:

- All animation is on GPU-compositable properties (`transform`, `opacity`,
  `background-position`); `will-change` is declared throughout.
- The container uses `contain: strict` to isolate layout/paint.
- It honors `prefers-reduced-motion` (CSS media query) **and** a `reduced` prop that
  drops particle counts (cubes/bubbles) for low-power devices.

It is not a CPU hog — the work is offloaded to the compositor. The 664 lines are all
per-theme `@keyframes` blocks with no logic; splitting would only fragment cohesive
styling. No change.

## 8. Consistency with desktop

BPM duplicates desktop store/library/profile *presentation* but the brief asked to share
*logic*, not presentation.

- **Shared:** `renderMarkdown` is now one module (`composables/render-markdown.ts`),
  reusable by both surfaces.
- **Deliberately not merged:** the desktop `composables/game-detail/` set
  (`use-game-install`, `use-game-launch`, `use-game-config`, `use-game-stats`) and the
  new `composables/bigpicture/` set. They look similar but diverge materially: the BPM
  config composable adds a CRT-shader toggle and gamepad-cycle + toast ergonomics the
  desktop one lacks; the BPM page additionally owns the entire streaming and three-source
  save surface that the desktop page does not have. Forcing a merge would push
  surface-specific branching into shared code. The genuinely shared sliver (option
  tables, cycle math) is small; the couch-UI ergonomics are what differ. Each set stays
  surface-specific, mirroring the desktop audit's own `game-detail/` decision.

## 9. Type safety

`use-bpm-game-streaming.ts` carries 3 `any` casts (`sessions.find((s: any) => …)`).
These are inherited verbatim from the original page and originate in the loosely-typed
`useStreaming()` composable, which is not BPM-specific. Tightening them requires
retyping the `useStreaming` API — out of scope for this audit, which preserved streaming
behavior exactly. Tracked as a deferred item.

## Verification

`pnpm typecheck` in `main/` passes with **no new errors** from these changes.

## Files changed / added

**Modified:**
- `composables/gamepad.ts` — axis heartbeat (the bug fix)
- `composables/bp-audio.ts` — `onended` node disconnection in `tone`/`sweep`/`noise`
- `pages/bigpicture/library/[id].vue` — 3232 → 1983 lines
- `pages/bigpicture/index.vue` — 2124 → 1956 lines

**Added:**
- `composables/render-markdown.ts` — shared markdown renderer
- `composables/bigpicture/use-bpm-game-streaming.ts`
- `composables/bigpicture/use-bpm-game-saves.ts`
- `composables/bigpicture/use-bpm-game-config.ts`
- `composables/bigpicture/use-bpm-home-data.ts`
- `components/bigpicture/game-detail/BpmGameSavesTab.vue`

## Deferred items

1. **`bigpicture/index.vue` per-theme markup** — the ~940-line template could be split
   into ~13 per-console child components. Deferred: high focus-regression risk, low
   structural gain (the logic is already extracted in §3).
2. **`useStreaming()` typing** — replacing the `any`-typed session objects (§9) with a
   real `StreamingSession` type would let `use-bpm-game-streaming.ts` drop its 3 casts.
3. **`bp-audio.ts` minor gaps** — `setVolume()` neither persists to `localStorage` nor
   re-applies on a freshly-created `AudioContext` (master gain is hardcoded to `0.18`);
   the silent `<audio>` unlock element is appended once and never removed (a single
   orphan, not a per-sound leak). Low-priority polish.
4. **`store.vue` bulk-select** — the ~125-line bulk-selection feature is a candidate for
   a `use-bpm-bulk-select.ts` composable if `store.vue` grows further.
