# Drop Desktop Client Roadmap

Custom fork of the official Drop-OSS desktop app with enhanced achievement system, playtime tracking, Steam Deck support, and more.

## ✅ Recently Completed

- Full achievement system (server + client)
  - Steam Web API, RetroAchievements, and Goldberg `steam_api.dll` auto-detection
  - Achievement definitions, sync, progress bars, and grid display
  - Admin tools for linking games and bulk scanning
- Playtime tracking (client reports sessions to server)
- Library page improvements (collapsible descriptions, proper image loading via `server://` protocol)
- Profile navigation fix (opens server web UI correctly)
- CSP updates for achievement icons

## 🚧 In Progress

- **RetroAchievements Integration**  
  Final testing with `RA_API_KEY`. Verify definition fetch, icon loading, and user account linking.

- **Improved ROM Import UX**  
  Better guidance and auto-suggestions when importing ROM files that don’t match default executable patterns.

## 📋 Planned / Wishlist

### High Priority
- Achievement toast overlays / in-game popups  
  (Windows transparent window or Goldberg built-in overlay + Steam Deck Decky Loader plugin)
- Goldberg client-side DLL injection & monitoring  
  (copy DLL before launch, read `achievements.json`, report unlocks)

### Medium Priority
- Cloud saves support
- Multi-disc ROM support
- Version tracking / SteamDB sync (“Update Available” indicators)

### Low Priority / Nice-to-have
- Profile customization (avatar, banner, themes)
- TV Mode / full gamepad navigation
- Drop Badges (activity rewards)
- Decky Loader plugin for Steam Deck achievement toasts
