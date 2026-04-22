<template>
  <div class="space-y-6">
    <section v-for="category in categories" :key="category.name">
      <div class="flex items-center gap-2 mb-3">
        <span class="text-lg">{{ category.icon }}</span>
        <h3 class="text-sm font-semibold uppercase tracking-wide" :style="{ color: 'var(--bpm-text)' }">
          {{ category.name }}
        </h3>
      </div>
      <div class="space-y-2">
        <div
          v-for="entry in category.entries"
          :key="entry.message"
          :ref="(el: any) => registerContent(el, {})"
          tabindex="0"
          class="rounded-xl p-3 outline-none transition-colors"
          :style="{
            backgroundColor: 'var(--bpm-surface)',
            border: '1px solid var(--bpm-border)',
          }"
        >
          <p class="text-sm font-mono font-semibold" :style="{ color: 'var(--bpm-accent-hex)' }">
            {{ entry.message }}
          </p>
          <div class="grid grid-cols-2 gap-4 mt-2">
            <div>
              <p class="text-[10px] uppercase tracking-wide font-medium" :style="{ color: 'var(--bpm-muted)' }">
                What it means
              </p>
              <p class="text-xs mt-0.5" :style="{ color: 'var(--bpm-text)' }">
                {{ entry.meaning }}
              </p>
            </div>
            <div>
              <p class="text-[10px] uppercase tracking-wide font-medium" :style="{ color: 'var(--bpm-muted)' }">
                Fix
              </p>
              <p class="text-xs mt-0.5" :style="{ color: 'var(--bpm-text)' }">
                {{ entry.fix }}
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { useBpFocusableGroup } from "~/composables/bp-focusable";

const registerContent = useBpFocusableGroup("content");

interface GlossaryEntry {
  message: string;
  meaning: string;
  fix: string;
}

interface GlossaryCategory {
  name: string;
  icon: string;
  entries: GlossaryEntry[];
}

const categories: GlossaryCategory[] = [
  {
    name: "Launch & Proton",
    icon: "🕹️",
    entries: [
      {
        message: "No compat tool configured",
        meaning: "Drop can't find a Proton build to run a Windows game.",
        fix: "Settings → Compatibility → pick Auto (GE-Proton).",
      },
      {
        message: "Wine prefix is locked",
        meaning: "A previous launch didn't clean up, or another process is using the prefix.",
        fix: "Wait 30 seconds, try again. If it persists, reset the prefix from the game's detail page.",
      },
      {
        message: "Game exited immediately (code 1)",
        meaning: "The game launched but crashed before showing a window. Often a missing runtime.",
        fix: "View the launch log. Try a different Proton build (pinned GE-Proton 9).",
      },
      {
        message: "GE-Proton is downloading",
        meaning: "First-launch only — umu fetches GE-Proton (~300 MB) before the game starts.",
        fix: "Wait. Subsequent launches are instant.",
      },
    ],
  },
  {
    name: "Downloads",
    icon: "⬇️",
    entries: [
      {
        message: "Checksum mismatch",
        meaning: "A downloaded chunk doesn't match the server's hash — disk or network corruption.",
        fix: "Drop auto-retries. If it keeps happening, check disk health or switch networks.",
      },
      {
        message: "Insufficient disk space",
        meaning: "Not enough free space on the install directory's drive.",
        fix: "Settings → Storage → add another install folder on a larger drive.",
      },
      {
        message: "Server unreachable",
        meaning: "Drop can't reach your Drop server.",
        fix: "Check your internet and the server URL in Settings. Server may be restarting.",
      },
      {
        message: "Download paused automatically",
        meaning: "Network dropped or app went to sleep — paused to avoid corruption.",
        fix: "Resume from the Downloads page once connectivity is back.",
      },
    ],
  },
  {
    name: "Saves",
    icon: "💾",
    entries: [
      {
        message: "Save sync conflict: local vs. cloud",
        meaning: "The same save was modified in two places since the last sync.",
        fix: "Pick the version you want to keep. The other is kept as a backup for 30 days.",
      },
      {
        message: "Ludusavi not installed",
        meaning: "PC save backup needs Ludusavi, and it hasn't been installed yet.",
        fix: "Re-run the setup wizard, or from Library → Game → Saves.",
      },
      {
        message: "No save found for this game",
        meaning: "Either you haven't played yet, or Ludusavi doesn't know this game's save paths.",
        fix: "Play the game at least once, then manually point Ludusavi at the save folder.",
      },
    ],
  },
  {
    name: "Authentication",
    icon: "🔑",
    entries: [
      {
        message: "Session expired",
        meaning: "Your server session timed out (usually after weeks of inactivity).",
        fix: "Sign in again. Your library and installed games are preserved.",
      },
      {
        message: "Server handshake failed",
        meaning: "The auth token returned from the server was rejected.",
        fix: "Try again. If it keeps failing, clear credentials from Settings → Sign out.",
      },
      {
        message: "Instance unreachable",
        meaning: "The server URL doesn't resolve, or the server is down.",
        fix: "Verify URL in Settings → Server. On LAN-only instances, check your network.",
      },
    ],
  },
  {
    name: "RetroAchievements",
    icon: "🏆",
    entries: [
      {
        message: "RA login failed",
        meaning: "Wrong username/password, or the RA servers are unreachable.",
        fix: "Double-check credentials at retroachievements.org. Case-sensitive.",
      },
      {
        message: "Game not recognized by RA",
        meaning: "The ROM doesn't match any checksum in RA's database.",
        fix: "Use a clean, unmodified ROM dump. Hacked or trimmed ROMs won't match.",
      },
      {
        message: "Hardcore unlock disabled",
        meaning: "You used a save state or rewind, which is disallowed in hardcore mode.",
        fix: "Restart the game for a clean run, or stick with softcore.",
      },
    ],
  },
];
</script>
