# Drop Audit → One Actionable Plan

80 verified findings across `drop-app` (Tauri client) and `drop-server` (Nuxt server), each adversarially re-confirmed. This report merges overlapping items, clusters them into themes, and orders everything by impact-to-effort so you can work top-down.

---

## Executive summary

**Fix these first (top 5):**

1. **IDOR — add games to ANY user's collection** (`drop-server/server/internal/userlibrary/index.ts:145`). The `collectionAdd` upsert relies on a relation filter inside `upsert.where` for authz; on a non-owned collection the unique lookup misses and falls through to the unconditional `create` branch. Any authenticated user can write into another user's collection. This is the single clearest exploitable authz hole. **Quick fix** (explicit ownership check, mirror the DELETE path).
2. **WebAuthn / OIDC nonces never burned → replay** (`auth/mfa/webauthn/finish.post.ts:36`, `auth/passkey/finish.post.ts:29`, `internal/auth/oidc/index.ts:325`). Both WebAuthn finish handlers delete the wrong session key (`webauthn/challenge` was never set; the real key is `webauthn/options`), and OIDC `signinStateTable` entries are never deleted. A captured assertion or `(code,state)` pair is replayable, and the OIDC table grows unbounded. **Quick fix** (delete the correct keys; burn state after `authorize()`).
3. **Stored XSS in the native Tauri webview** (`drop-app/main/pages/library/[id]/index.vue:367`, `store/[id].vue:461`, `news.vue:183`). The server runs DOMPurify on every micromark→v-html sink; the client renders the *same* author content with no sanitizer, inside a webview whose CSP allows `'unsafe-inline'` and that holds `invoke()`/`__TAURI__`. A compromised admin account → code execution in the desktop app. **Medium fix** (port `sanitize.ts` + dompurify dep, or route through the safe `renderMarkdown`).
4. **Download hangs forever on a truncated chunk** (`drop-app/src-tauri/games/src/downloads/download_logic.rs:157`). A short/early-closed stream returns `Ok(0)`; `remaining -= 0` never decreases, so the read loop busy-spins indefinitely instead of erroring into the existing retry path. The sibling `validate.rs` loop already guards this. **Quick fix** (one `if amount == 0` guard → retryable error).
5. **Transient DB I/O error wipes the database + media cache** (`drop-app/src-tauri/database/src/interface.rs:199`). `handle_invalid_database` matches the error kind only to pick a log line, then unconditionally renames the real DB aside and `remove_dir_all`s the cache — even for `DatabaseError::Io`, directly contradicting the crate's own documented contract. A flaky mount on a Steam Deck → all install records and cached art gone. **Quick fix** (branch: only corruption variants reset; Io aborts startup).

**Repo health read:**

- **drop-server** — Generally well-structured (ACL model, CSRF middleware, constant-response on password-reset show real security intent), but has a cluster of **authz-boundary slips**: an IDOR, replayable auth nonces, an admin-stats endpoint gated on a user-level ACL, and a destructive playtime wipe behind `maintenance:read`. None require unusual skill to exploit on a multi-user instance; all are small, surgical fixes. Secondary themes are a handful of `JSON.parse`/missing-FK-check 500s and a couple of dead-code/comment-lies that mislead future work.
- **drop-app** — Solid error-typed pipelines in most crates, but carries **launch/compat fragility** (the Asahi/muvm path is broken outright, several `unwrap`/byte-slice panics sit on post-launch hot paths), one **download liveness bug**, one **destructive DB-recovery bug**, and notable **frontend resource-leak/lifecycle bugs** (dead Guide button, leaking gamepad listeners, a WebSocket with no reconnect that silently kills all live updates). Plus meaningful **dependency bloat** (an unused cache crate dragging in a whole reqwest 0.11/hyper 0.14/openssl stack) and a fully orphaned `tailscale` crate with a latent Windows compile error.

The good news: the overwhelming majority of high-severity items are **small, localized, low-risk fixes**. The "Quick wins" section below is where most of the security and crash exposure disappears for very little effort.

---

## Theme 1 — Server authz, CSRF & auth-nonce hygiene (highest priority)

The most consequential cluster. These are the bugs an actual multi-user instance is exposed to.

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| Collection-add IDOR | `drop-server/server/internal/userlibrary/index.ts:145` | Relation filter in `upsert.where` doesn't gate `create`; non-owned collection falls through to unconditional insert | Fetch collection, 403 if `userId !== owner`, then upsert — mirror `index.delete.ts`. Never use a relation filter as authz on an upsert. |
| OIDC state never burned (replay + unbounded growth) | `drop-server/server/internal/auth/oidc/index.ts:325` | `signinStateTable[state]` is read but never deleted; the CSRF-nonce the callback relies on is replayable and the table leaks one entry per abandoned login forever | `delete this.signinStateTable[state]` after `authorize()`; add TTL/size cap (or persist like reset tokens). |
| WebAuthn/passkey challenge never invalidated (replay) | `drop-server/server/api/v1/auth/mfa/webauthn/finish.post.ts:36` + `auth/passkey/finish.post.ts:29` | Both delete `webauthn/challenge` (never set); real key `webauthn/options` (holds the challenge) persists → assertion replayable in-session | Delete `webauthn/options` (match the registration handler `user/mfa/webauthn/finish.post.ts:34`). Optionally enforce counter monotonicity. |
| Admin endpoints gated on user-ACL not system-ACL | `drop-server/server/api/v1/admin/system-data/ws.get.ts:12` + `admin/settings/dummy-data.get.ts:5` | `getUserIdACL`/`getUserACL` grant any authenticated session ("sessions auto-have all ACLs"); `system-data:listen`/`settings:read` are userACLs → any non-admin reaches admin server-stats WS + dummy-data | Add `allowSystemACL(...)` as the primary gate (it's the only check that verifies `user.admin`). Consider making `getUserIdACL` reject names not in `userACLs`. |
| Destructive playtime wipe behind read ACL | `drop-server/server/api/v1/admin/playtime/soft-reset.post.ts:19` | Bulk-zeroes ALL users' playtime, gated by `maintenance:read` (a read scope) | Gate on a write/action ACL (`task:start`, consistent with `objects/gc.post.ts`, or a new `playtime:reset`). |
| XFF spoof bypasses IP allow/deny firewall | `drop-server/server/internal/security/ipRules.ts:97/113` + `server/internal/utils/rateLimit.ts:104` | `trustProxy()` defaults true and `getClientIp` trusts the **leftmost** XFF token; on a directly-exposed instance an attacker spoofs an allowed IP | Give the IP-filter its own resolver that takes the **rightmost** hop under trusted-proxy (or a hop count); flip `DROP_TRUST_PROXY` to opt-in, and warn in the admin UI when an allowlist is active with proxy trust on. |
| Signin account enumeration (timing + status) | `drop-server/server/api/v1/auth/signin/simple.post.ts:58` | No-hash early return for unknown user (timing oracle) + distinct 401/403 for missing vs disabled | Dummy-argon2 verify on the not-found path; return the same 401 for not-found and disabled. |
| Post-login open-redirect | `drop-server/composables/user.ts:21` + `components/Auth/OpenID.vue:4` + `auth/oidc/callback.get.ts:67` | Unvalidated `redirect` query pushed verbatim; OIDC start URL interpolates it without `encodeURIComponent`; callback `sendRedirect`s it with no origin check | Require same-origin relative path (must start with single `/`, reject `//` and any scheme) before navigating; `encodeURIComponent` in OpenID.vue; validate on the callback too. |
| Forgot-password timing enumeration | `drop-server/server/api/v1/auth/password/forgot.post.ts:93` | Body/status are constant, but only the hit path does a DB write + awaited SMTP send → timing oracle | Send mail out-of-band (don't await in request path), or enforce a fixed minimum wall-clock time. |
| Passkey path collapses 2FA to 1 + replay | `drop-server/server/api/v1/auth/passkey/finish.post.ts:101` | Same wrong-key challenge bug; passwordless passkey alone drives session to level 20 even if the user also enrolled TOTP | Fix the delete key (covered above); decide MFA policy deliberately — re-evaluate remaining factors rather than unconditionally +10. |

**Ordering note:** the IDOR, the two nonce bugs, and the user-vs-system-ACL gate are the highest impact-to-effort (real authz holes, each a few lines). The XFF and enumeration items are real but mitigated on a typical self-hosted single-user deployment — do them in the same sweep but they're lower urgency.

---

## Theme 2 — Client launch / compat robustness

Bugs that break or crash the launch/post-launch path. The Asahi one is a hard break; the rest are panics on hot paths.

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| Asahi muvm path mangles umu exe path | `drop-app/src-tauri/process/src/process_handlers.rs:423` | Splits the assembled command on the literal `"umu-run"` substring — which is inside the quoted absolute umu path — truncating it; breaks every Apple-Silicon launch | Don't string-split on a substring that appears in the path. Re-parse with `ParsedCommand` (or have `UMUCompatLauncher` expose a structured `(env, umu_exe, launch)` tuple) and insert `muvm --` between them. |
| Save-conflict dialog freezes whole app for 5 min | `drop-app/src-tauri/process/src/process_manager/save_sync.rs:92` | Pre-launch conflict resolution `recv_timeout(300s)` runs while holding the global `PROCESS_MANAGER` lock; blocks all kills/launches and stalls exiting games' cleanup | Run the save sync (at least the conflict wait) **without** holding `PROCESS_MANAGER`; only grab it for the brief processes-table insert. |
| Compat telemetry byte-slice UTF-8 panic | `drop-app/src-tauri/src/compat.rs:588` (+546, +797) | Raw byte slices on wine-log excerpts panic on a non-char-boundary; runs after **every** launch → crash log each time the tail lands mid-codepoint | Use char-boundary-safe truncation (`is_char_boundary` advance, as `settings.rs:444` already does; `chars().take(N)` for the `[..180]` prefixes). |
| Malformed VDF panics during launch | `drop-app/src-tauri/process/src/compat.rs:60` | `unwrap_obj()`/`get_obj().unwrap()` in `read_proton_path` panic on a hand-edited/odd `compatibilitytool.vdf` | Replace with `get_obj()?` so a bad VDF degrades to "no display name"/clean `NoCompat`. |
| ParsedCommand env-detection on `=` | `drop-app/src-tauri/process/src/parser.rs:18` | First token without `=` taken as exe; a path containing `=` is misparsed as env or aborts the launch | Only treat a leading token as env if it matches `^[A-Za-z_][A-Za-z0-9_]*=`. |

**Ordering:** Asahi break first (functional regression on a whole platform), then the 5-minute lock freeze (whole-app liveness), then the telemetry slice panic (fires on every launch). The VDF and `=` parser items are edge-case hardening.

---

## Theme 3 — Client download engine correctness

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| Truncated chunk → infinite busy-loop | `drop-app/src-tauri/games/src/downloads/download_logic.rs:157` | `read()==Ok(0)` on a short stream never decrements `remaining`; loop spins forever, never hits checksum/retry | `if amount == 0 { return Err(Communication(UnparseableResponse(...))) }` — converts a hang to a retryable error (matches `validate.rs:263`). |
| Validate/scan use raw `join` (path traversal) | `drop-app/src-tauri/games/src/downloads/validate.rs:161` (+255, `status.rs:348`) | Write path uses `path_guard::join_within`; validate/scan use raw `install_dir.join(filename)` → a `..`/absolute manifest filename resolves outside the sandbox | Use `path_guard::join_within` in validate.rs/status.rs; a failing filename = validation failure. |
| Manifest error-body `.unwrap()` panic | `drop-app/src-tauri/games/src/downloads/download_agent.rs:238` | `response.text().await.unwrap()` panics if the error-body read fails → kills the download task, queue stuck `Downloading` | `unwrap_or_else(|e| format!("<failed to read error body: {e}>"))`. |
| Dead retry-classification block | `drop-app/src-tauri/games/src/downloads/download_agent.rs:410` | `let retry = true;` + commented-out classifier → every error retried 3× (incl. non-retryable), wasting 1+2+4s backoff; redundant `drop(permit)` | Re-enable classification (fail fast on auth/`InvalidResponse`) or delete the indirection + comment; remove the redundant `drop`. |
| Disk-progress not reset on boundary pause | `drop-app/src-tauri/games/src/downloads/download_logic.rs:182` | File-boundary Stop branch zeroes `download_progress` but not `disk_progress` (in-loop branch does both) — cosmetic, self-heals on resume | Add `disk_progress.set(0);` to the boundary branch. |
| Global progress throttle starves UI updates | `drop-app/src-tauri/download_manager/src/util/progress_object.rs:55` | Single global `LAST_UPDATE_TIME` CAS shared by download + (no-op) disk objects → download-speed UI updates ~half as often | Make the throttle per-`ProgressObject` (`AtomicInstant` field, CAS `self.last_update`). |

---

## Theme 4 — Client DB & crypto

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| I/O error wipes DB + media cache | `drop-app/src-tauri/database/src/interface.rs:199` | Match only picks a log line; all variants incl. `Io` fall through to rename-DB-aside + `remove_dir_all(cache)` → empty DB on a transient read fault | Only run reset for corruption variants (`InvalidUtf8`/`Deserialize`/`UnsupportedVersion`/`MigrationFailed`); for `Io`, abort startup, leave DB untouched, don't wipe cache. |
| DB write lock held across encrypt+disk write | `drop-app/src-tauri/database/src/interface.rs:252` | `DBWrite::drop` AES-encrypts + writes the whole DB before releasing the `RwLock`; every reader stalls on every mutation (eMMC/network mount pain) | Snapshot to an owned `Vec<u8>` under the lock, release, then encrypt+write off-lock (atomic rename already prevents races). |
| AES-CTR keystream reuse (fixed key+IV) | `drop-app/src-tauri/database/src/interface.rs:142` | Same `KEY_IV` from counter 0 every write/backup → XOR of two on-disk versions leaks plaintext XOR; no MAC | Prepend a random nonce per write (seed CTR from it), or switch to AES-GCM/ChaCha20-Poly1305. If it's only obfuscation, document that. |

---

## Theme 5 — Frontend lifecycle & resource leaks (client)

These break silently after the first BPM enter/exit cycle or socket drop — invisible on the Deck (Gamescope blocks `exit()`), live on desktop.

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| WebSocket has no reconnect → all live updates die | `drop-app/main/composables/ws.ts:21` | Only `onopen`/`onmessage`; no `onclose`/`onerror`/reconnect; `connected` never reset. A Wi-Fi blip/server restart silently kills notifications, task progress (infinite spinners), admin dashboard | Add `onclose`/`onerror` → `connected=false` + backoff reconnect + replay `outQueue`; surface a disconnected state to the UI. |
| Guide button dead after first BPM exit | `drop-app/main/composables/big-picture.ts:95` | `_resetGuideWired()` has zero callers; `destroy()` clears the handler but `guideWired` stays true → Guide button dead rest of session | Register the Guide handler inside `enter()`/`init()` (re-added on re-init), or wire up `_resetGuideWired()` from `destroy()`. |
| Gamepad window listeners leak each BPM cycle | `drop-app/main/composables/gamepad.ts:332` | `init()` adds anonymous `gamepadconnected`/`disconnected` listeners; `destroy()` never `removeEventListener`s them → unbounded accumulation | Store named handler refs at module scope; `removeEventListener` both in `destroy()`. |
| `useCompatSummary()` no error handling + fetch race | `drop-app/main/composables/use-compat-summary.ts:51` | No try/catch (unlike `shelves.ts`/`proton.ts`); non-atomic undefined-check → concurrent callers double-fetch | Wrap fetch in try/catch (soft-fail to `{}`); de-dupe via a stored in-flight promise. |

---

## Theme 6 — Dual-surface drift (markdown rendering)

The dual-surface rule is being violated in rendering. Consolidating fixes the XSS (Theme 1 #3), broken BPM markdown, and broken news images in one move.

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| Client v-html with NO sanitization (stored XSS) | `drop-app/main/pages/library/[id]/index.vue:367` + `store/[id].vue:461` + `news.vue:183` | Server DOMPurifies every micromark→v-html sink; client renders identical author content raw in a `'unsafe-inline'` webview with `invoke()` access | Port `drop-server/composables/sanitize.ts` + add dompurify dep, wrap all client sinks; **or** route all three through the already-safe `renderMarkdown`. |
| BPM news shows literal markdown; desktop news drops image rewrite | `drop-app/main/pages/bigpicture/news.vue:124` + `news.vue:183` | BPM passes raw markdown to `rewriteDescriptionImages` (never micromark'd) → `# Heading` shows literally; desktop news skips `rewriteDescriptionImages` → relative images 404 | Use `renderMarkdown(...)` in BPM news (renders + rewrites + escapes); add image rewrite (or `renderMarkdown`) in desktop news. |
| Two renderers for the same description field | `drop-app/main/pages/bigpicture/library/[id].vue:1077` | BPM uses safe `renderMarkdown`; desktop library uses raw `micromark` → same description renders differently and unsafely | Standardize on one client markdown strategy across desktop+BPM, library+store+news. |
| BPM vs desktop renderer inconsistency + misleading comment | `drop-server/pages/store/[id].vue:455` | store/[id] comment claims "same pipeline as library detail" — true for desktop only, not BPM | Unify renderers or correct the comment + document the BPM choice. |

**One action covers most of this:** make a single shared, sanitizing client renderer (extend `renderMarkdown` or wrap micromark in dompurify) and route library/store/news on both desktop and BPM through it.

---

## Theme 7 — Server input-validation & error-contract 500s (admin/client API)

Low-severity individually (mostly admin-gated 500s instead of clean 4xx), but a consistent pattern worth a single pass. Root cause: trusting route/body params without an existence check or schema, and `JSON.parse` without try/catch.

| Finding | File:line | Fix |
|---|---|---|
| News tags raw `JSON.parse` → 500 | `drop-server/server/api/v1/admin/news/index.post.ts:44` | Wrap in try/catch → 400. |
| WebAuthn finish body unvalidated/`JSON.parse` `any` | `drop-server/server/api/v1/auth/mfa/webauthn/finish.post.ts:35` | try/catch the parse → 400; arktype-validate the assertion body like the rest of the auth surface. |
| external-link upsert no game-exists check → FK 500 | `drop-server/server/api/v1/admin/game/[id]/external-link.post.ts:22` (+`link-retroachievements.post.ts:54`) | `prisma.game.count` pre-check → 404, mirror `game/image/index.post.ts:47`. |
| requests PATCH writes unvalidated gameId → FK 500 | `drop-server/server/api/v1/admin/requests/[id].patch.ts:38` | Validate `gameId` exists before write. |
| sync-installed: dup/missing gameId rolls back whole batch | `drop-server/server/api/v1/client/sync-installed.post.ts:19` | Dedup + UUID/length cap; `createMany({ skipDuplicates: true })`; intersect against existing Game ids. |
| compat/results unbounded `protonVersion` + unchecked `gameVersionId` FK | `drop-server/server/api/v1/client/compat/results.post.ts:62` | `.slice(0, 100)` like the other fields; validate/whitelist `gameVersionId` or map P2003→400. |
| bulk-upload accepts arbitrary `dataHash` (single path validates it) | `drop-server/server/api/v1/client/saves/bulk-upload.post.ts:150` | Apply the same `/^[0-9a-fA-F]{32}$/` MD5 check as `upload.post.ts`; share a helper so they can't drift. |
| collectionAdd `game!` non-null + opaque 500 on bad id | `drop-server/server/internal/userlibrary/index.ts:162` | Pre-check `findUnique` → 404 (also closes the IDOR cleanly); drop the `!`. |
| ip-rule patch lockout guard matches by (pattern,kind) not id | `drop-server/server/api/v1/admin/security/ip-rules/[id]/index.patch.ts:71` | Match the simulated swap on `r.id` (widen `getIpRules()`/`IpRuleLike` to include `id`). |

---

## Theme 8 — Server SSRF & object-store hardening

Two findings describe the **same** SSRF sink; the object-overwrite items are latent.

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| Metadata image import = SSRF (no URL validation) | `drop-server/server/internal/objects/transactional.ts:84` | `$fetch(data, {responseType:"stream"})` fetches provider-supplied URLs with no scheme/host check; admin import of a game whose provider field points at `169.254.169.254`/`localhost` → internal read, persisted + served via `/api/v1/object/<id>` | Before `$fetch`: `new URL()`, allow only http(s), reject hosts resolving to RFC1918/loopback/link-local; ideally resolve+pin to defeat rebinding. Shared helper used by transactional pull. (Admin-gated → lower urgency, but real.) |
| `createContentAddressed` comment lies about superset ACL | `drop-server/server/internal/objects/objectHandler.ts:149` | Comment promises a permission-superset check that doesn't exist; on a dedup hit a second uploader gets the first's ACLs (latent — zero callers today) | Implement the superset merge, or correct the comment and have callers verify ACLs before reuse. |
| Stale object-overwrite TODO (effectively dead) | `drop-server/server/internal/objects/objectHandler.ts:311` | `// TODO: prevent overwriting` — but no object ever carries a `write`/`delete` ACL, so the path can't succeed; TODO misleads | Remove the endpoint (objects are immutable-by-id), or document that overwrite needs an ACL nothing grants. |

---

## Theme 9 — Dependency hygiene

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| Unused `reqwest-middleware-cache` drags obsolete HTTP stack | `drop-app/src-tauri/Cargo.toml:76` | Pulls reqwest 0.11.27, hyper 0.14, cacache 9, http 0.2, openssl — a whole parallel stack used by nothing | Delete the line, `cargo update`. Shrinks compile + supply-chain surface. |
| `native-tls-alpn` forces OpenSSL the code never uses | `drop-app/src-tauri/Cargo.toml:66` (+`remote/Cargo.toml:24`) | reqwest features enable both native-tls-alpn and rustls; all builders call `.use_rustls_tls()` | Drop `native-tls-alpn` from both manifests → removes native-tls/openssl/hyper-tls. |
| `main/` standalone build has no committed lockfile | `drop-app/main/pnpm-workspace.yaml:6` | Security `overrides:` block but no `main/pnpm-lock.yaml`; every release re-resolves transitives unpinned | Commit `main/pnpm-lock.yaml`; `scripts/build.mjs` uses `--frozen-lockfile`; CI to keep overrides synced with root. |
| Vestigial `deranged = "=0.4.0"` hard pin | `drop-app/src-tauri/Cargo.toml:40` | Direct exact-pin of an unused crate forces a 2nd old copy (time already on 0.5.8) | Remove the line, rebuild. |
| Unused `anyhow = "*"` wildcard in database crate | `drop-app/src-tauri/database/Cargo.toml:9` | Dead dep + wildcard version is a supply-chain smell | Delete the line; audit other wildcards. |

---

## Theme 10 — Dead code & orphaned crates

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| `tailscale` crate fully orphaned + latent Windows compile error | `drop-app/src-tauri/tailscale/` (`lib.rs:333`, `Cargo.toml:1`) | Not a workspace member, referenced nowhere, carries a git submodule; `Write` impl calls `as_raw_fd()` unconditionally while `AsRawFd` is non-Windows-only → won't compile on Windows (masked because never built) | Delete the crate dir + `.gitmodules` submodule entry; OR add to workspace, cfg-gate build.rs, fix `Write` to mirror `Read`'s cfg split. |
| `open_fs` unused arbitrary-path opener on IPC surface | `drop-app/src-tauri/src/client.rs:83` | Registered, zero callers; opens (Windows: executes) any frontend path | Delete it + its registration in `lib.rs:280`. |
| Stale `eslint-disable vue/no-v-html` in files with no v-html | `drop-server/pages/store/t/[id]/index.vue:1` (+`store/c/[id]`, `GameEditor/Version.vue`) | File-level disable of a security lint with nothing to justify it; would mask a future real sink | Remove from the 3 files; use line-scoped disables where v-html is genuine. |
| `DownloadType::{Tool,Dlc,Mod}` never constructed | `drop-app/src-tauri/database/src/models.rs:475` | Dead variants; the referenced `MOD_SYSTEM_ARCHITECTURE.md` doesn't exist | Add a "reserved for future" comment on the enum. |

---

## Theme 11 — Stale docs & comment lies (mislead future agents)

CLAUDE.md is the canonical, behavior-overriding instruction file — stale lines here cost the most in re-investigation.

| Finding | File:line | Fix |
|---|---|---|
| CLAUDE.md says Goldberg detection "NOT implemented" — it ships & polls every 15s | `drop-app/CLAUDE.md:67` | Rewrite: file reading IS implemented (`remote/src/goldberg/`); what's missing is push-based/fs-watcher detection. |
| CLAUDE.md (both repos) says `serde_json` NOT in `remote` — it's a direct dep used in 9 files | `drop-app/CLAUDE.md` + `drop-server/CLAUDE.md` | Update/delete the bullet. |
| `DropTaskSchedule` `{daily}`/`{weekly}` is decorative — never wired to a runner | `drop-server/server/internal/tasks/scheduler.ts:30` | Make scheduler.ts derive daily/weekly buckets from the registry (delete the hardcoded arrays), OR drop the variants from the type + docstring. |
| Interval docstring claims "after last completion" but uses fixed `setInterval` | `drop-server/server/internal/tasks/index.ts:799` | Fix docstring (overlap suppressed by single-flight), or switch to self-rescheduling `setTimeout`. |
| signin.vue superlevel `useHead` title hardcoded English | `drop-server/pages/auth/signin.vue:73` | Add `auth.signin.pageTitleProtected` i18n key. |

---

## Theme 12 — Server-side perf & growth

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| ApplicationSettings inserts a row per change (unbounded + pins old logos vs GC) | `drop-server/server/internal/config/application-configuration.ts:19` | `save()` always `create`s; GC `findReference` scans ALL rows → every historical `mLogoObjectId` keeps replaced logos alive forever | Keep a single row and `upsert`/`update` in place (or prune to newest N); GC scan only the current row. |
| Middleware compares Ref object to undefined → `updateUser()` dead | `drop-server/middleware/require-user.global.ts:12` | `useUser()` is always a Ref, never `=== undefined` → recovery path never runs; masked only by app.vue | `if (user.value === undefined)`. |
| `updateUser()` short-circuits forever once null | `drop-server/composables/user.ts:10` | `if (user.value === null) return` → a logged-out tab never picks up a new session without reload | Drop the short-circuit (or gate behind a `force` flag). |

---

## Theme 13 — Client remote/cache & misc

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| `generate_authorization_header()` panics on corrupt EC key | `drop-app/src-tauri/remote/src/auth.rs:91` | `from_ec_pem(..).unwrap()`/`.expect()` on the hot path of every request → panics whatever task drives it (achievement poller, playtime stop) | Return `Result<String, RemoteAccessError>`, thread `?` through `send_with_retry`. |
| Achievement config double-cached (45s poll serves 5-min-stale) | `drop-app/src-tauri/remote/src/achievements.rs:577` | Inner 5-min disk cache dominates the 45s re-fetch; never invalidated → server unlocks invisible up to 5 min, stale seed on cold start | Add `fetch_achievement_config_uncached`/`force` for the poll branch; clear `achievement-config/{id}` after `report_achievements`. |
| `fetch_user()` bypasses shared helper (uncapped `.json()`) | `drop-app/src-tauri/remote/src/auth.rs:98` | The one in-scope endpoint the migration missed; raw `.json()`, wrong error taxonomy | Route through `remote_request`/`bounded_json`. |
| `fetch_drop_object` blocking HTTP on sync command | `drop-app/src-tauri/src/remote.rs:70` | Sync command + `DROP_CLIENT_SYNC` ties up a worker per call | `async fn` + `DROP_CLIENT_ASYNC`, or `spawn_blocking`. (Already noted as deferred.) |
| In-memory cache "LRU" evicts arbitrary entries | `drop-app/src-tauri/remote/src/cache.rs:132` | `keys().take(N)` is hash-order, not oldest; comment claims LRU | Add a timestamp field + select lowest, or fix the comment to "arbitrary 25%". |
| `delete_download_dir` IPC index panic | `drop-app/src-tauri/src/settings.rs:29` | `Vec::remove(index)` panics on OOB index from frontend | Bounds-check like `open_download_dir:70`; return `Result`. |
| `resume_download` `.expect()` on no-parent path | `drop-app/src-tauri/src/downloads.rs:188` | `.parent().expect(...)` panics on a root install dir | `.ok_or(InvalidCommand)?`. |
| Handshake double-emits `auth/failed` | `drop-app/src-tauri/src/lib.rs:754` | Malformed path emits twice (others once) → duplicate toasts | Remove the redundant emit at 754. |

---

## Theme 14 — Client deep-link / single-instance (Windows/Linux warm-start auth)

| Finding | File:line | Problem (1 line) | Fix |
|---|---|---|---|
| Single-instance argv callback drops `drop://` deep links | `drop-app/src-tauri/src/lib.rs:263` | No-op callback; when Drop is already running, the warm-start `drop://handshake/...` lands in the 2nd instance's argv and is discarded → `recieve_handshake` never called. Breaks the primary auth path on Windows/Linux warm start | In the callback, scan `argv` for `drop://` and route it like `on_open_url` (parse + `spawn(recieve_handshake(...))`). |
| `recieve_handshake` misspelling in command name + IPC contract | `drop-app/src-tauri/src/lib.rs:722` | Misspelling baked into the Tauri command name and the frontend `invoke()` string | Rename across Rust + the matching `invoke()` (do it while touching auth above). |

---

## Quick wins (high-impact, low-effort, safe now)

Do these first — most of the security + crash exposure disappears for a few lines each:

1. **Collection IDOR** — explicit ownership check in `collectionAdd` (`userlibrary/index.ts:145`).
2. **WebAuthn + passkey nonce** — delete `webauthn/options` (the correct key) in both finish handlers.
3. **OIDC state burn** — `delete signinStateTable[state]` after `authorize()` (+ TTL).
4. **Admin user-vs-system ACL** — add `allowSystemACL(...)` to `system-data/ws.get.ts` + `dummy-data.get.ts`; gate `playtime/soft-reset` on a write ACL.
5. **Truncated-chunk hang** — `if amount == 0 { return Err(...) }` in `download_logic.rs:157`.
6. **DB I/O wipe** — branch `handle_invalid_database` so `Io` doesn't reset/wipe (`interface.rs:199`).
7. **Compat telemetry UTF-8 panic** — char-boundary-safe truncation (`compat.rs:588/546/797`).
8. **Single-instance deep-link** — forward `drop://` from the argv callback (`lib.rs:263`).
9. **Manifest/`auth_header`/`resume`/`delete_download_dir` panics** — swap `.unwrap()`/`.expect()` for `?`/bounds-checks (4 one-liners).
10. **Validation 500s pass** — try/catch `JSON.parse` (news tags, webauthn body); add `prisma.game.count` pre-checks (external-link, requests PATCH, collectionAdd); `.slice()` `protonVersion`; MD5-validate bulk `dataHash`.
11. **Dependency deletes** — `reqwest-middleware-cache`, `native-tls-alpn`, `deranged` pin, `anyhow` wildcard, `open_fs`, stale `eslint-disable` comments.
12. **Doc fixes** — CLAUDE.md Goldberg line + serde_json bullet (both repos).

## Bigger projects (valuable, larger effort)

1. **Unify client markdown rendering + sanitization** (Theme 6). Build one shared sanitizing renderer and route desktop+BPM library/store/news through it. Closes the stored-XSS, the literal-markdown BPM bug, and broken news images together. Needs a new dep (dompurify) + 4–5 page edits + a render-parity check.
2. **Decouple DB persistence from the write lock** (`interface.rs:252`). Snapshot under lock, encrypt+write off-lock (ideally a debounced writer thread). Touches the most concurrency-sensitive code; needs care + testing on eMMC/network mounts.
3. **Restructure the Asahi/muvm launcher** (`process_handlers.rs:423`). Have `UMUCompatLauncher` expose a structured `(env, umu_exe, launch)` tuple so `AsahiMuvmLauncher` wraps it without string-splitting; also fixes the move-conflict-lock interaction if you touch the launch path. Needs a Linux/Asahi device to validate.
4. **Save-sync off the global lock** (`save_sync.rs:92`). Re-architect pre-launch sync so the 300s human-wait never holds `PROCESS_MANAGER`. Liveness-sensitive; test concurrent launch/exit/kill.
5. **WebSocket reconnect layer** (`ws.ts:21`). Add close/error/backoff/replay to the shared handler; affects notifications, tasks, and admin home together.
6. **DB encryption: nonce-per-write or AEAD** (`interface.rs:142`). Migration-aware change (must read old fixed-IV files, write new nonce-prefixed/AEAD format).
7. **Tailscale crate decision** — delete (likely) or revive + fix the Windows `Write` impl and wire into the workspace.
8. **Scheduler single-source-of-truth** (`scheduler.ts:30`) — make daily/weekly derive from the registry, delete the hardcoded arrays, so a `{weekly:true}` task can't silently never run.
9. **ApplicationSettings single-row + GC fix** (`application-configuration.ts:19`) — stop unbounded row growth and unpin replaced logos from GC.
10. **`main/` reproducible builds** — commit the lockfile + `--frozen-lockfile` + override-sync CI.
