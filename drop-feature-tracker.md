# Drop Feature Tracker
*Last updated: 2026-03-26*

---

## COMPLETED

### Profile Page Banner Fix
- **What**: Banner gradient only covered ~70% of viewport
- **Fix**: Added `w-full` to root div
- **Where**: Server `pages/` profile page

### In-Game Achievement System (Server)
- **What**: Full achievement sync infrastructure with three providers
- **Providers**: Steam Web API, RetroAchievements, Goldberg (auto-detected)
- **How it works**:
  - Goldberg: Auto-detects `steam_api.dll` during game import, creates `GameExternalLink`, fetches definitions from Steam API
  - Steam: Syncs unlocks for users with linked Steam accounts
  - RetroAchievements: Admin manually sets RA Game ID, definitions auto-fetched via `API_GetGameInfoExtended`
- **Files**:
  - `server/internal/achievements/` — providers + sync service
  - `prisma/models/achievement.prisma` — schema
  - `server/api/v1/games/[id]/achievements.get.ts` — public endpoint
  - `server/api/v1/admin/achievements/scan.post.ts` — bulk scan (fire-and-forget)
  - `server/api/v1/admin/game/[id]/external-link.post.ts` — link RA/Steam IDs (triggers definition fetch)
  - `server/api/v1/admin/game/[id]/external-links.get.ts` — list links
  - `server/api/v1/admin/game/[id]/achievements.delete.ts` — reset achievements

### Achievement Display (Server Web UI)
- **What**: Achievements section on library game page with progress bar + grid
- **Where**: `pages/library/game/[id]/index.vue`
- **Features**: Unlocked/locked states, external icon URLs, date earned, progress bar
- **CSP**: Added `steamcdn-a.akamaihd.net` and `media.retroachievements.org` to img-src

### Admin Achievement Management
- **Achievement filter**: "No achievements" / "Has achievements" filter in admin library
- **RA Game ID linking**: Achievement Links panel in game editor Metadata sidebar
- **Achievement reset**: DELETE endpoint to clear achievements per game/user

### Achievement Display (Desktop Client)
- **What**: Achievements section embedded in client library page via `server://` protocol iframe
- **Where**: `main/pages/library/[id]/index.vue` (client)
- **Layout**: Uses `embedded` layout (no StoreNav header)
- **Features**: Collapsible section, hidden scrollbar, matches server UI

### Collapsible Description (Desktop Client)
- **What**: Game description section is now collapsible on client library page
- **Where**: `main/pages/library/[id]/index.vue`

### Playtime Tracking (Desktop Client)
- **What**: Client reports play session start/stop to server
- **Where**:
  - `src-tauri/remote/src/playtime.rs` — API module
  - `src-tauri/process/src/process_manager.rs` — integrated into launch/exit lifecycle
- **How**: `start_playtime()` called before game spawn (5s timeout), `stop_playtime()` fire-and-forget on exit
- **Capability**: `trackPlaytime` registered during auth handshake

### Achievement Polling (Desktop Client)
- **What**: Client polls server for new achievement unlocks every 15s while game is running
- **Where**: `src-tauri/remote/src/achievements.rs`
- **Status**: Polling works, logs new unlocks. Toast overlay removed (see wishlist).

### Profile Page Fix (Desktop Client)
- **What**: Profile link now opens server web UI in external browser instead of 404
- **Where**: Client navigation uses `gen_drop_url` to construct server profile URL

### Image Loading Fix (Desktop Client)
- **What**: Description images and carousel now load properly via `server://` protocol
- **Where**: `main/pages/library/[id]/index.vue` — `rewriteDescriptionImages()` rewrites URLs

---

## IN PROGRESS

### RetroAchievements Integration
- **Status**: Server-side done. Needs testing with deployed RA_API_KEY.
- **Test case**: Chrono Trigger (RA Game ID 319)
- **Remaining**:
  - Verify definition fetch works when admin saves RA Game ID
  - Verify achievement icons load from `media.retroachievements.org`
  - User achievement sync requires linking RA account (UserExternalAccount)

### ROM Import UX
- **Issue**: "Launch executable/command" is blank after importing ROMs
- **Root cause**: Drop only auto-detects `.exe/.sh/.x86_64` and emulator ROM extensions (if emulator registered with `emulatorSuggestions`). Not a bug — working as designed.
- **Improvement needed**: Better guidance in import UI for ROM files, or auto-suggest based on file extension matching known emulators

---

## WISHLIST / DEFERRED

### Achievement Toast Overlay (In-Game Popups)
- **Priority**: Nice-to-have
- **Status**: Research complete, prototype attempted and removed
- **Problem**: Tauri 2.x transparent window doesn't render properly on Windows
- **Windows options**:
  - Custom Win32 `WS_EX_TOPMOST | WS_EX_LAYERED` window (bypasses Tauri)
  - Windows native toast (simple but suppressed by Focus Assist during gaming)
- **Steam Deck option**: Decky Loader plugin using built-in `toaster` API (~200-300 lines)
- **Best long-term**: Goldberg's built-in overlay (hooks rendering pipeline, works fullscreen)
- **References**: Achievement Watcher, node-powertoast, Drop issue #274

### Decky Loader Plugin (Steam Deck Achievement Toasts)
- **Priority**: Low
- **Status**: Plan documented
- **Approach**: Python backend polls Drop API + React frontend uses Decky toaster
- **Complexity**: Low (~200-300 lines)
- **Requires**: Decky Loader installed, Drop server URL + auth token

### Goldberg Client-Side DLL Injection
- **Priority**: Medium
- **Status**: Server-side detection done, client-side not started
- **What's done**: Server detects `steam_api.dll` during import, creates external links, fetches achievement definitions
- **What's needed**:
  - Client copies Goldberg DLL alongside game before launch
  - Client monitors `achievements.json` for unlocks
  - Client reports unlocks to server via POST endpoint
  - Goldberg has its own built-in overlay for achievement popups (free!)

### Version Tracking / SteamDB Sync
- **Priority**: Low
- **Status**: Concept only
- **Idea**: Master list of game versions/build IDs, admin button to compare against SteamDB, "Update Available" icon on store page

### Profile Customization
- **Priority**: Low
- **Status**: Not started
- **Features**: Profile picture upload, banner upload, theme selector

### Drop Badges (Platform Activity Rewards)
- **Priority**: Low
- **Status**: Not started (user said to ignore for now)

### Cloud Saves
- **Priority**: Medium
- **Status**: Client capability registered (`cloudSaves`), no implementation

### TV Mode / Gamepad Support
- **Priority**: Low
- **Status**: Not started

### Multi-Disc ROM Support
- **Priority**: Low
- **Status**: Not started

---

## CLEANUP / TECHNICAL DEBT

- [ ] Remove `debug.get.ts` endpoint before final deployment
- [ ] Clean up test UserAchievement records from manual testing
- [ ] Consider adding `syncedAt` field update during achievement sync
- [ ] Rate limiting on achievement polling (currently 15s, could be configurable)

---

## ENVIRONMENT NOTES

- **Server**: Docker image `drop-custom:latest`, deployed via SFTP to Synology NAS
- **Client**: Tauri 2.9.5 app, NSIS installer
- **Database**: PostgreSQL 14 (Alpine) via Docker
- **API Keys**: `STEAM_API_KEY` and `RA_API_KEY` in compose.yaml environment
- **Critical patterns**:
  - Never use Prisma `include` (LATERAL JOIN bug with `@prisma/adapter-pg`)
  - Never name API files `index.get.ts` in folders sharing names with `pages/` directories
  - Fire-and-forget: `void service.method().catch(() => {})` for background processing
  - Body validation: `readDropValidatedBody(h3, Schema)` with arktype
