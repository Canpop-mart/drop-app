# Drop Achievement Toast — Decky Loader Plugin Plan

## Overview
A Decky Loader plugin for Steam Deck that shows achievement unlock toasts over games, powered by the Drop server API.

## Architecture

### Backend (Python — `main.py`)
- On game launch: starts polling Drop server `/api/v1/games/{id}/achievements` every 15 seconds
- Maintains a set of previously-seen unlocked achievement IDs
- When a new unlock is detected (ID not in previous set), pushes event to frontend via Decky WebSocket
- On game close: stops polling
- Stores Drop server URL and auth token in plugin settings

### Frontend (React/TypeScript — `index.tsx`)
- Settings page: Drop server URL input, auth token input, test button
- Listens for unlock events from backend
- Uses Decky's built-in `toaster` API to show achievement toast:
  ```typescript
  serverAPI.toaster.toast({
    title: "Achievement Unlocked",
    body: `${achievement.title} — ${achievement.description}`,
    icon: <img src={achievement.iconUrl} />,
    duration: 5000,
  });
  ```

### File Structure
```
drop-achievements/
├── main.py              # Python backend (~50 lines)
├── index.tsx            # React frontend (~100 lines)
├── package.json         # Node dependencies (decky-frontend-lib)
├── plugin.json          # Decky plugin metadata
├── tsconfig.json        # TypeScript config
├── rollup.config.js     # Build config
└── assets/
    └── drop-icon.png    # Plugin icon
```

## API Integration

### Polling Endpoint
```
GET /api/v1/games/{gameId}/achievements
Authorization: Bearer <token>
```

Response includes `achievements[]` with `unlockedAt` field — if non-null, achievement is unlocked.

### Auth
- Option A: Use existing Drop client JWT auth (requires client certificate registration)
- Option B: Add a simple API key/token system for third-party integrations (simpler)

## Requirements
- Decky Loader installed on Steam Deck
- Drop server accessible from Steam Deck (same network or Tailscale)
- Game must be tracked in Drop with achievements imported

## Game Detection
- Option A: User manually selects which Drop game they're playing (simple but manual)
- Option B: Plugin reads current running game from Steam and matches by name to Drop library (automatic but fuzzy)
- Option C: Plugin uses a mapping file (Steam AppID → Drop Game ID) configured in settings

## Limitations
- Only works in Steam Deck Game Mode (Desktop Mode can use standard Linux notifications)
- Requires network access to Drop server during gameplay
- Achievement definitions must already exist in Drop (imported via admin scan)
- Toast style matches Steam Deck native notifications (cannot be fully customized)

## Estimated Effort
- ~1-2 hours to build initial version
- ~200-300 lines of code total
- Separate repository from main Drop codebase
