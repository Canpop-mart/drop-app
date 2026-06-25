<template>
  <div>
    <div class="border-b border-zinc-700 py-5">
      <h3 class="text-base font-semibold font-display leading-6 text-zinc-100">
        Achievements
      </h3>
      <p class="mt-1 text-sm text-zinc-400">
        Reset your unlocked achievements or diagnose achievement tracking
        issues.
      </p>
    </div>

    <!-- Achievement Reset -->
    <div class="mt-5 flex flex-col gap-4">
      <div class="flex items-center gap-3">
        <select
          v-model="resetGameId"
          class="flex-1 rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50"
        >
          <option value="">All Games</option>
          <option v-for="game in gamesList" :key="game.id" :value="game.id">
            {{ game.mName }}
          </option>
        </select>
        <button
          @click="resetAchievements"
          :disabled="achievementResetting"
          class="rounded-md bg-red-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-red-500 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
        >
          <span v-if="achievementResetting">Resetting...</span>
          <span v-else>Reset Achievements</span>
        </button>
      </div>

      <p v-if="achievementMessage" class="text-sm text-green-400">
        {{ achievementMessage }}
      </p>
    </div>

    <!-- Achievement Diagnostics -->
    <div class="border-b border-zinc-700 py-5 mt-10">
      <h3 class="text-base font-semibold font-display leading-6 text-zinc-100">
        Diagnostics
      </h3>
      <p class="mt-1 text-sm text-zinc-400">
        Check achievement system health for a specific game.
      </p>
    </div>

    <div class="mt-5 flex flex-col gap-4">
      <div class="flex items-center gap-3">
        <select
          v-model="debugGameId"
          class="flex-1 rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50"
        >
          <option value="" disabled>Select a game...</option>
          <option v-for="game in gamesList" :key="game.id" :value="game.id">
            {{ game.mName }}
          </option>
        </select>
        <button
          @click="runDiagnostic"
          :disabled="debugLoading || !debugGameId"
          class="rounded-md bg-zinc-700 px-4 py-2 text-sm font-semibold text-zinc-100 shadow-sm hover:bg-zinc-600 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
        >
          <span v-if="debugLoading">Checking...</span>
          <span v-else>Run Diagnostic</span>
        </button>
      </div>

      <div
        v-if="debugResult"
        class="mt-2 rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 text-xs font-mono"
      >
        <!-- Status -->
        <div class="flex items-center gap-2 mb-3">
          <span
            class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium"
            :class="
              debugResult.status === 'OK'
                ? 'bg-green-600/20 text-green-400'
                : 'bg-red-600/20 text-red-400'
            "
          >
            {{ debugResult.status }}
          </span>
          <span class="text-zinc-400">{{ debugResult.game.name }}</span>
        </div>

        <!-- Issues -->
        <div v-if="debugResult.issues.length > 0" class="mb-3 space-y-1">
          <p
            v-for="(issue, i) in debugResult.issues"
            :key="i"
            class="text-red-400 leading-relaxed"
          >
            {{ issue }}
          </p>
        </div>

        <!-- Summary -->
        <div class="grid grid-cols-2 gap-x-6 gap-y-1 text-zinc-400">
          <span>Total achievements:</span>
          <span class="text-zinc-200">{{
            debugResult.summary.totalAchievements
          }}</span>
          <span>Goldberg achievements:</span>
          <span class="text-zinc-200">{{
            debugResult.summary.goldbergAchievements
          }}</span>
          <span>Unlocked by you:</span>
          <span class="text-zinc-200">{{
            debugResult.summary.unlockedByUser
          }}</span>
          <span>Goldberg AppIDs:</span>
          <span class="text-zinc-200">{{
            debugResult.summary.goldbergAppIds.join(", ") || "NONE"
          }}</span>
          <span>External links:</span>
          <span class="text-zinc-200">{{
            debugResult.summary.externalLinks.join(", ") || "NONE"
          }}</span>
          <span>Orphan sessions:</span>
          <span
            :class="
              debugResult.summary.orphanSessions > 0
                ? 'text-red-400'
                : 'text-zinc-200'
            "
          >
            {{ debugResult.summary.orphanSessions }}
          </span>
          <span>Connected clients:</span>
          <span class="text-zinc-200">{{
            debugResult.summary.connectedClients
          }}</span>
        </div>

        <!-- Achievement list (collapsible) -->
        <details class="mt-3">
          <summary class="cursor-pointer text-zinc-500 hover:text-zinc-300">
            Show all {{ debugResult.details.achievements.length }} achievements
          </summary>
          <div class="mt-2 max-h-60 overflow-y-auto space-y-0.5">
            <div
              v-for="a in debugResult.details.achievements"
              :key="a.id"
              class="flex items-center gap-2 py-0.5"
              :class="a.unlocked ? 'text-green-400' : 'text-zinc-500'"
            >
              <span>{{ a.unlocked ? "✓" : "✗" }}</span>
              <span class="truncate">{{ a.title }}</span>
              <span class="ml-auto text-zinc-600"
                >{{ a.provider }}:{{ a.externalId }}</span
              >
            </div>
          </div>
        </details>

        <!-- Active sessions -->
        <div v-if="debugResult.details.activeSessions.length > 0" class="mt-3">
          <p class="text-zinc-500 mb-1">Orphaned sessions:</p>
          <div
            v-for="s in debugResult.details.activeSessions"
            :key="s.id"
            class="text-red-400"
          >
            {{ s.id.slice(0, 8) }}... — started {{ s.ageMinutes }}m ago
          </div>
        </div>
      </div>
    </div>

    <!-- Status messages -->
    <p v-if="errorMessage" class="mt-4 text-sm text-red-400">
      {{ errorMessage }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { serverUrl } from "~/composables/use-server-fetch";

// ── State ──────────────────────────────────────────────────────────────────

const errorMessage = ref("");
const games = ref<Map<string, { mName: string }>>(new Map());

// ── Achievement reset ────────────────────────────────────────────────────

const resetGameId = ref("");
const achievementResetting = ref(false);
const achievementMessage = ref("");

const gamesList = computed(() =>
  Array.from(games.value.entries())
    .map(([id, g]) => ({ id, mName: g.mName }))
    .sort((a, b) => a.mName.localeCompare(b.mName)),
);

// ── Achievement diagnostics ──────────────────────────────────────────────

const debugGameId = ref("");
const debugLoading = ref(false);
const debugResult = ref<{
  game: { id: string; name: string };
  status: string;
  issues: string[];
  summary: {
    totalAchievements: number;
    goldbergAchievements: number;
    unlockedByUser: number;
    goldbergAppIds: string[];
    externalLinks: string[];
    orphanSessions: number;
    connectedClients: number;
  };
  details: {
    achievements: {
      id: string;
      externalId: string;
      provider: string;
      title: string;
      hasIcon: boolean;
      unlocked: boolean;
      unlockedAt: string | null;
    }[];
    activeSessions: {
      id: string;
      startedAt: string;
      ageMinutes: number;
    }[];
    clients: {
      id: string;
      name: string;
      lastConnected: string;
    }[];
  };
} | null>(null);

// ── Data fetching ────────────────────────────────────────────────────────

onMounted(async () => {
  try {
    const gamesRes = await fetch(
      serverUrl("api/v1/store?sort=name&order=asc&limit=200"),
    );
    if (gamesRes.ok) {
      const data = await gamesRes.json();
      const map = new Map<string, { mName: string }>();
      for (const g of data.results ?? []) {
        map.set(g.id, { mName: g.mName });
      }
      games.value = map;
    }
  } catch {
    errorMessage.value = "Failed to load games list.";
  }
});

// ── Actions ──────────────────────────────────────────────────────────────

async function runDiagnostic() {
  if (!debugGameId.value) return;
  debugLoading.value = true;
  debugResult.value = null;
  errorMessage.value = "";

  try {
    const res = await fetch(
      serverUrl(`api/v1/user/achievements/debug/${debugGameId.value}`),
    );
    if (res.ok) {
      debugResult.value = await res.json();
    } else {
      errorMessage.value = "Diagnostic request failed.";
    }
  } catch (e) {
    errorMessage.value = `Diagnostic failed: ${e}`;
  } finally {
    debugLoading.value = false;
  }
}

async function resetAchievements() {
  const gameName =
    gamesList.value.find((g) => g.id === resetGameId.value)?.mName ??
    "all games";
  const message = resetGameId.value
    ? `This will reset all achievements for ${gameName}. Are you sure?`
    : "This will reset ALL of your achievements across every game. Are you sure?";

  if (!confirm(message)) return;

  achievementResetting.value = true;
  achievementMessage.value = "";
  errorMessage.value = "";

  try {
    const query = resetGameId.value ? `?gameId=${resetGameId.value}` : "";
    const res = await fetch(
      serverUrl(`api/v1/user/achievements/reset${query}`),
      { method: "DELETE" },
    );
    if (res.ok) {
      const data = await res.json();
      achievementMessage.value = `Achievements reset successfully. (${data.deleted} removed)`;
      setTimeout(() => {
        achievementMessage.value = "";
      }, 5000);
    } else {
      errorMessage.value = "Failed to reset achievements.";
    }
  } catch (e) {
    errorMessage.value = `Failed to reset achievements: ${e}`;
  } finally {
    achievementResetting.value = false;
  }
}
</script>
