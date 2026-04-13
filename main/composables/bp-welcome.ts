/**
 * BPM Welcome Messages
 *
 * Generates contextual welcome-back messages based on recent game activity.
 * Shows for a brief duration then auto-dismisses, once per session.
 */

import { ref, readonly, onMounted, onUnmounted } from "vue";
import type { Ref } from "vue";

// ── Types ────────────────────────────────────────────────────────────────────

export interface GameEntry {
  game: { mName: string };
  status: { playtime: number };
}

// ── Singleton state ──────────────────────────────────────────────────────────

let shownThisSession = false;
let dismissTimeoutId: NodeJS.Timeout | null = null;

// ── Message generation ──────────────────────────────────────────────────────

function generateMessage(recentGames: GameEntry[]): string {
  if (recentGames.length === 0) {
    return "Welcome to Big Picture Mode.";
  }

  const messages = [
    `Continue ${recentGames[0].game.mName}?`,
    `Welcome back — ${recentGames.length} game${recentGames.length !== 1 ? "s" : ""} in your library.`,
    `Ready to play?`,
    `Let's go — ${recentGames[0].game.mName} is waiting.`,
  ];

  // Calculate total playtime this week (rough estimate from first game)
  if (recentGames[0].status.playtime > 0) {
    const hours = Math.round(recentGames[0].status.playtime / 3600);
    if (hours > 0) {
      messages.push(`You've played ${hours} hour${hours !== 1 ? "s" : ""} this week.`);
    }
  }

  // Pick a random message
  return messages[Math.floor(Math.random() * messages.length)];
}

// ── Composable ───────────────────────────────────────────────────────────────

export function useBpmWelcome(recentGames: Ref<GameEntry[]>) {
  const message = ref("");
  const visible = ref(false);

  function dismiss() {
    visible.value = false;
    if (dismissTimeoutId !== null) {
      clearTimeout(dismissTimeoutId);
      dismissTimeoutId = null;
    }
  }

  function show() {
    if (shownThisSession) return;
    shownThisSession = true;

    message.value = generateMessage(recentGames.value);
    visible.value = true;

    // Auto-dismiss after 5 seconds
    dismissTimeoutId = setTimeout(() => {
      visible.value = false;
      dismissTimeoutId = null;
    }, 5000);
  }

  onMounted(() => {
    // Show welcome message on component mount
    show();
  });

  onUnmounted(() => {
    dismiss();
  });

  return {
    message: readonly(message),
    visible: readonly(visible),
    dismiss,
  };
}
