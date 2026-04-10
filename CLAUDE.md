# Drop App — Notes for Claude

## Dual-Surface Rule (PERMANENT)

**Every feature must consider both the web server and the desktop client.**

Drop has two surfaces:

- **Web** (`drop-server`) — Nuxt 3 server, accessed via browser
- **Client** (`drop-app`) — Tauri desktop app, which has its own native pages AND embeds server pages via `server://` iframes

When implementing any feature, always explicitly decide what happens on each surface:

| Strategy                 | When to use                                                                                                                  |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| **Native on both**       | Feature needs different behaviour per surface (e.g. library game page needs Tauri `invoke()` on client, web fetch on server) |
| **Web only**             | Feature is web-only by nature (admin panel, auth pages, account settings)                                                    |
| **Client iframe**        | Feature works fine as a web page embedded in the client (community, news, requests, profile, store)                          |
| **Not needed on client** | Feature is irrelevant in a desktop context                                                                                   |

**Never ship a feature to one surface and leave the other as "Under Construction" without a deliberate, documented reason.**

---

## Client Page Patterns

### iframe pattern (server:// protocol)

Used for: store, community, news, requests, profile.
The server detects client requests via `User-Agent: Drop Desktop Client` and hides its own header/footer.

```vue
<template>
  <div class="flex min-h-full flex-col bg-zinc-950">
    <div class="flex w-full flex-1 flex-col">
      <iframe :src="pageUrl" class="h-full min-h-full w-full border-0 flex-1" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
const pageUrl = convertFileSrc("dummyvalue", "server").replace(
  "dummyvalue",
  "your-path",
);
</script>
```

### Native pattern

Used for: library (needs Tauri invoke for install/launch), queue, settings.

- Data via `invoke("command_name")` from `@tauri-apps/api/core`
- Real-time updates via `listen("event_name", callback)` from `@tauri-apps/api/event`
- Images via `useObject(id)` composable (returns `object://` protocol URL)
- Server API calls via `serverUrl(path)` composable (returns `server://` protocol URL)

---

## Architecture Notes

- `server://` protocol — Tauri custom protocol that proxies all requests to the Drop backend with auth headers automatically applied.
- `object://` protocol — Tauri protocol for game media (banners, covers, icons).
- `serde_json` is available in the `process` crate but NOT in the `remote` crate — use inner `#[derive(Serialize)] struct` instead.
- Achievement polling is in `remote/src/achievements.rs` — polls `/api/v1/client/game/{id}/achievement-config` every 15 seconds.
- Goldberg file-based achievement detection is NOT yet implemented — currently relies on server-side sync only.
