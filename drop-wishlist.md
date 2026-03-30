# Drop Feature Wishlist

## Achievement Toast Overlay (Deferred)
**Priority**: Nice-to-have
**Status**: Research complete, implementation removed pending better approach

### Windows Approach
- Custom transparent overlay window using Tauri WebviewWindow
- Polls server for new achievement unlocks every 15s during gameplay
- Shows golden toast in bottom-right corner
- Works for borderless windowed games only (not exclusive fullscreen)
- Attempted implementation had issues with Tauri 2.x window transparency on Windows

### Steam Deck Approach — Decky Loader Plugin
- Decky plugins render inside Steam's own CEF layer, which Gamescope allows over games
- **Backend (Python):** Polls Drop server API for achievement unlocks
- **Frontend (React/TypeScript):** Uses Decky's built-in `toaster` API to show notifications
- Estimated ~200-300 lines total
- Requires: Decky Loader installed, Drop server URL + auth token
- See: https://github.com/deckbrew/deckbrew.xyz for Decky plugin docs

### Alternative Approaches Considered
| Approach | Over fullscreen? | Platform | Complexity |
|---|---|---|---|
| Windows native toast (PowerShell/WinRT) | No (Focus Assist blocks) | Windows | Very simple |
| Custom transparent window (Tauri) | Borderless windowed only | Windows/Linux | Medium |
| Goldberg built-in overlay | Yes (hooks rendering) | Windows/Linux/Deck | Requires DLL injection |
| Decky Loader plugin | Yes (uses Steam's CEF) | Steam Deck only | Low |
| Custom Vulkan layer (MangoHud-style) | Yes | Linux/Deck | Extremely high |
| Win32 API raw window (WS_EX_TOPMOST) | Borderless windowed only | Windows | Medium |

### References
- Achievement Watcher: https://github.com/xan105/Achievement-Watcher
- node-powertoast: https://github.com/xan105/node-powertoast
- Drop issue #274: https://github.com/Drop-OSS/drop/issues/274

---

## Version Tracking / SteamDB Sync (Idea)
**Priority**: Low
**Status**: Concept only

- Maintain a master list of game versions/build IDs
- Admin button to sync/compare build IDs against SteamDB
- Show "Update Available" icon on store page when newer version exists
- Could use SteamDB's public API or scraping

---

## Goldberg Client-Side DLL Injection (Deferred)
**Priority**: Medium
**Status**: Server-side detection done, client injection not started

- Server already detects steam_api.dll during import
- Server already fetches achievement definitions from Steam API
- Client needs to: copy Goldberg DLL alongside game before launch, monitor achievement file for unlocks, report back to server
- Goldberg has its own built-in overlay for achievement popups
