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

    <!-- RetroAchievements account. Linking goes through the Drop server so
         it can record unlocks; see composables/ra-link.ts. -->
    <div class="mt-5 rounded-lg border border-zinc-800 bg-zinc-900/50 p-4">
      <h4 class="text-sm font-semibold text-zinc-100">RetroAchievements</h4>
      <div
        v-if="ra.loadState.value === 'error'"
        class="mt-2 flex items-center gap-3 text-sm text-red-400"
      >
        <span class="flex-1"
          >Could not check your RetroAchievements link with the server.
          {{ ra.loadError.value }}</span
        >
        <button
          class="rounded-md bg-zinc-700 px-3 py-1.5 text-sm font-semibold text-zinc-100 hover:bg-zinc-600"
          @click="ra.refresh"
        >
          Retry
        </button>
      </div>
      <p
        v-else-if="ra.loadState.value === 'loading'"
        class="mt-2 text-sm text-zinc-500"
      >
        Checking...
      </p>
      <div
        v-else-if="ra.state.value === 'mismatch'"
        class="mt-2 flex items-center gap-3"
      >
        <p class="flex-1 text-sm text-amber-300">
          Your server has {{ ra.serverUsername.value }}, but this device still
          signs RetroArch in as {{ ra.localUsername.value }}. It switches on
          the next launch that can reach the server.
          <span v-if="ra.syncError.value" class="block text-red-400">{{
            ra.syncError.value
          }}</span>
        </p>
        <button
          :disabled="ra.busy.value"
          class="rounded-md bg-zinc-700 px-3 py-1.5 text-sm font-semibold text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
          @click="ra.syncLocalCopy"
        >
          Update this device
        </button>
      </div>
      <div
        v-else-if="ra.state.value === 'linked'"
        class="mt-2 flex items-center gap-3"
      >
        <p class="flex-1 text-sm text-zinc-300">
          Linked as
          <span class="font-semibold text-zinc-100">{{
            ra.serverUsername.value
          }}</span
          >. Unlocks are recorded, and RetroArch signs in automatically.
        </p>
        <button
          :disabled="ra.busy.value"
          class="rounded-md bg-zinc-700 px-3 py-1.5 text-sm font-semibold text-zinc-100 hover:bg-zinc-600 disabled:opacity-50"
          @click="unlinkRa"
        >
          Unlink
        </button>
      </div>
      <div v-else class="mt-2 flex flex-col gap-3">
        <p
          v-if="ra.state.value === 'expired'"
          class="text-sm text-amber-300"
        >
          Your RetroAchievements sign-in has expired, so unlocks are no longer
          tracked. Sign in again.
        </p>
        <p
          v-else-if="ra.state.value === 'needs_signin'"
          class="text-sm text-amber-300"
        >
          Linked as {{ ra.serverUsername.value }}, but RetroArch can't sign in
          yet. Link your account again below so RetroArch can sign in for you.
        </p>
        <p
          v-else-if="ra.state.value === 'device_only'"
          class="text-sm text-amber-300"
        >
          Signed in on this device only. Your server does not know this
          account, so unlocks are not being recorded. Sign in again.
        </p>
        <p v-else class="text-sm text-zinc-400">
          Link your account so Drop records RetroAchievements unlocks. Your
          password goes to your Drop server once and is not stored.
        </p>
        <input
          v-model="raUsername"
          type="text"
          autocomplete="off"
          placeholder="RetroAchievements username"
          class="rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-blue-500"
        />
        <input
          v-model="raPassword"
          type="password"
          autocomplete="off"
          placeholder="Password"
          class="rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-blue-500"
        />
        <template v-if="ra.apiKeyRequired.value">
          <input
            v-model="raApiKey"
            type="password"
            autocomplete="off"
            placeholder="Web API key"
            class="rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-blue-500"
          />
          <p class="text-xs text-zinc-500">
            Your server needs your Web API key to track unlocks. It is under
            Settings, Keys on retroachievements.org.
          </p>
        </template>
        <button
          :disabled="
            ra.busy.value ||
            !raUsername ||
            !raPassword ||
            (ra.apiKeyRequired.value && !raApiKey)
          "
          class="self-start rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
          @click="linkRa"
        >
          {{ ra.busy.value ? "Linking..." : "Link account" }}
        </button>
      </div>
      <p v-if="raError" class="mt-2 text-sm text-red-400">{{ raError }}</p>
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
      <p v-if="gamesError" class="text-sm text-red-400">
        Could not load your games.
        <button class="underline hover:text-red-300" @click="loadGames">
          Retry
        </button>
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
import { invoke } from "@tauri-apps/api/core";
import { serverUrl } from "~/composables/use-server-fetch";
import { useRaLink } from "~/composables/ra-link";
import { serverErrorText } from "~/composables/achievements/status";

// ── RetroAchievements link ──────────────────────────────────────────────

const ra = useRaLink();
const raUsername = ref("");
const raPassword = ref("");
const raApiKey = ref("");
const raError = ref("");

onMounted(async () => {
  await ra.refresh();
  raUsername.value = ra.serverUsername.value ?? ra.localUsername.value;
});

async function linkRa() {
  raError.value = "";
  try {
    await ra.link(raUsername.value, raPassword.value, raApiKey.value);
    raPassword.value = "";
    raApiKey.value = "";
  } catch (e: any) {
    raError.value = String(e?.message ?? e);
  }
}

async function unlinkRa() {
  raError.value = "";
  try {
    await ra.unlink();
  } catch (e: any) {
    raError.value = `Could not unlink: ${String(e?.message ?? e)}`;
  }
}

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

const gamesError = ref(false);

async function loadGames() {
  gamesError.value = false;
  try {
    const gamesRes = await fetch(
      serverUrl("api/v1/store?sort=name&order=asc&limit=200"),
    );
    if (!gamesRes.ok) throw new Error(String(gamesRes.status));
    const data = await gamesRes.json();
    const map = new Map<string, { mName: string }>();
    for (const g of data.results ?? []) {
      map.set(g.id, { mName: g.mName });
    }
    games.value = map;
  } catch {
    gamesError.value = true;
  }
}

onMounted(loadGames);

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
  // Placeholder copy. Says what a reset does and doesn't reach.
  const message =
    (resetGameId.value
      ? `This resets your achievements for ${gameName} on your Drop server and in this device's Goldberg save files.`
      : "This resets ALL of your achievements, for every game, on your Drop server and in this device's Goldberg save files.") +
    " RetroAchievements unlocks stay on retroachievements.org, and Drop keeps ignoring those old ones unless you reset them there too." +
    " Unlocks some other emulators saved without a date can come back the next time the game runs." +
    " Are you sure?";

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
      // Clear this device's local save files too, or the next launch
      // re-reports what was just reset (see clearLocalAfterReset).
      const local = await clearLocalAfterReset(resetGameId.value || null);
      achievementMessage.value = `Achievements reset successfully. (${data.deleted} removed)${local}`;
      setTimeout(() => {
        achievementMessage.value = "";
      }, 8000);
    } else {
      let body: unknown = null;
      try {
        body = await res.json();
      } catch {
        // No JSON body; the status code alone goes into the message.
      }
      errorMessage.value = `Failed to reset achievements: ${serverErrorText(res.status, body)}`;
    }
  } catch (e) {
    errorMessage.value = `Failed to reset achievements: ${e}`;
  } finally {
    achievementResetting.value = false;
  }
}

/**
 * After a server reset, mark this device's local Goldberg save files as not
 * earned: one game, or (gameId null) every game installed on this device.
 * Returns a short suffix for the status line. The server ignores re-reported
 * unlocks dated before the reset anyway; this covers files without dates and
 * lets the game award them again. A running game is skipped (it would write
 * its state back).
 */
async function clearLocalAfterReset(gameId: string | null): Promise<string> {
  let running = 0;
  let failures: string[] = [];
  try {
    if (gameId) {
      const r = await invoke<{ status: string; failures: string[] }>(
        "clear_local_achievements",
        { gameId },
      );
      if (r.status === "running") running = 1;
      failures = r.failures;
    } else {
      const r = await invoke<{ running: string[]; failures: string[] }>(
        "clear_all_local_achievements",
      );
      running = r.running.length;
      failures = r.failures;
    }
  } catch (e) {
    failures = [String(e)];
  }
  const notes: string[] = [];
  if (running > 0)
    notes.push(
      "A game that is running kept its local unlocks. Reset it again after closing it.",
    );
  if (failures.length > 0) {
    console.warn("[ACH] clearing local achievements:", failures);
    notes.push(
      `Some local save files could not be cleared: ${failures.join("; ")}`,
    );
  }
  return notes.length ? ` ${notes.join(" ")}` : "";
}
</script>
