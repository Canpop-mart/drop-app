<template>
  <div>
    <div class="border-b border-zinc-700 py-5">
      <h3 class="text-base font-semibold font-display leading-6 text-zinc-100">
        Cloud Saves
      </h3>
      <p class="mt-1 text-sm text-zinc-400 max-w-xl">
        See what Drop has backed up to your server, and choose how it syncs
        save files and names this machine in conflict prompts.
      </p>
    </div>

    <!-- ── Backup status ───────────────────────────────────────────────
         The question this page exists to answer, above the settings that
         used to be all it held. Wider than the controls below on purpose:
         it is a status board, not a form field. -->
    <div class="mt-5 max-w-4xl">
      <div class="rounded-xl border border-zinc-700 bg-zinc-800/40 overflow-hidden">
        <div class="flex items-start justify-between gap-4 px-5 py-4">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <component
                :is="headline.icon"
                class="size-5 shrink-0"
                :class="headline.iconClass"
              />
              <p class="text-base font-semibold text-zinc-100">
                {{ headline.text }}
              </p>
            </div>
            <p class="mt-1 text-sm text-zinc-400">{{ headline.detail }}</p>
          </div>
          <button
            type="button"
            class="shrink-0 inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-zinc-700 text-zinc-200 hover:bg-zinc-600 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            :disabled="statusLoading"
            @click="loadStatus"
          >
            <ArrowPathIcon
              class="size-3.5"
              :class="statusLoading ? 'animate-spin' : ''"
            />
            {{ statusLoading ? "Checking…" : "Refresh" }}
          </button>
        </div>

        <!-- Quota. The server has capped this since the feature shipped and
             nothing has ever shown the number. -->
        <div v-if="quota" class="px-5 pb-4">
          <div class="h-1.5 w-full rounded-full bg-zinc-700 overflow-hidden">
            <div
              class="h-full rounded-full transition-all"
              :class="quotaBarClass"
              :style="{ width: `${quotaPercent}%` }"
            />
          </div>
          <p class="mt-1.5 text-xs text-zinc-400">
            {{ quotaLine }}
            <span v-if="quota.revisionBytes > 0" class="text-zinc-500">
              Older versions of your saves hold another
              {{ formatCloudSaveBytes(quota.revisionBytes) }}, which does not count
              towards this.
            </span>
          </p>
        </div>

        <p v-if="statusError" class="px-5 pb-4 text-sm text-red-400">
          {{ statusError }}
        </p>
      </div>

      <!-- Failures. Recorded whenever a sync gives up, so the answer survives
           the modal being dismissed and the app being restarted. -->
      <div v-if="failureList.length > 0" class="mt-4 space-y-2">
        <div class="flex items-center justify-between">
          <h3 class="text-sm font-medium text-zinc-100">
            {{ failureList.length }}
            {{ failureList.length === 1 ? "game needs" : "games need" }}
            attention
          </h3>
          <button
            type="button"
            class="text-xs text-zinc-400 hover:text-zinc-200 transition-colors"
            @click="clearAll"
          >
            Dismiss all
          </button>
        </div>
        <div
          v-for="failure in failureList"
          :key="failure.gameId"
          class="flex items-start gap-3 rounded-lg border border-red-500/30 bg-red-500/5 px-4 py-3"
        >
          <ExclamationTriangleIcon class="size-5 shrink-0 text-red-400 mt-0.5" />
          <div class="min-w-0 flex-1">
            <p class="text-sm font-medium text-zinc-100">
              {{ nameFor(failure.gameId) }}
            </p>
            <p class="mt-0.5 text-sm text-zinc-400">{{ failure.message }}</p>
            <p class="mt-0.5 text-xs text-zinc-500">{{ timeAgo(failure.at) }}</p>
          </div>
          <button
            type="button"
            class="shrink-0 text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
            @click="clearFailure(failure.gameId)"
          >
            Dismiss
          </button>
        </div>
      </div>

      <!-- Per-game grid. Deliberately a grid of small cards rather than a tall
           list: the point is scanning for the game you care about, and this
           project does not do long dead settings lists. -->
      <div v-if="summaries.length > 0" class="mt-6">
        <div class="flex items-center justify-between gap-4">
          <!-- Not "Backed up games": PC saves are readable across accounts, so
               this list can hold a game whose every file belongs to someone
               else on this server. Each card says which. -->
          <h3 class="text-sm font-medium text-zinc-100">
            Games with saves on your server
          </h3>
          <input
            v-model="filter"
            type="text"
            placeholder="Filter"
            class="w-40 rounded-md border-0 py-1 px-2 text-xs text-zinc-100 ring-1 ring-inset ring-zinc-700 bg-zinc-800 placeholder:text-zinc-500 focus:ring-2 focus:ring-inset focus:ring-blue-600"
          />
        </div>
        <div class="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
          <div
            v-for="game in visibleSummaries"
            :key="game.gameId"
            class="rounded-lg bg-zinc-800/60 px-3 py-2.5 min-w-0"
          >
            <p
              class="text-sm font-medium text-zinc-100 truncate"
              :title="game.gameName"
            >
              {{ game.gameName }}
            </p>
            <p class="mt-0.5 text-xs text-zinc-400">
              {{ game.fileCount }}
              {{ game.fileCount === 1 ? "file" : "files" }} ·
              {{ formatCloudSaveBytes(game.totalBytes) }}
            </p>
            <p class="text-xs text-zinc-500">
              Backed up {{ timeAgo(Date.parse(game.lastUploadedAt)) }}
            </p>
            <!-- PC saves are read across every account on this server, so a
                 count here is not necessarily all your own work. The "none of
                 these are yours" case is the one that used to read as a
                 backup. -->
            <p
              v-if="ownSaveCount(game) === 0"
              class="mt-1 text-xs text-amber-400/80"
            >
              None of these are yours. They are another account's copy of a
              shared PC save.
            </p>
            <p
              v-else-if="game.sharedCount > 0"
              class="mt-1 text-xs text-amber-400/80"
            >
              {{ game.sharedCount }} of these
              {{ game.sharedCount === 1 ? "is" : "are" }} another account's copy
              of a shared PC save.
            </p>
          </div>
        </div>
        <button
          v-if="filteredSummaries.length > visibleSummaries.length"
          type="button"
          class="mt-3 text-xs text-blue-400 hover:text-blue-300 transition-colors"
          @click="showAll = true"
        >
          Show all {{ filteredSummaries.length }}
        </button>
        <p
          v-if="filter.trim() && filteredSummaries.length === 0"
          class="mt-3 text-sm text-zinc-500"
        >
          No game matches that.
        </p>
      </div>
    </div>

    <div class="mt-10 space-y-10 max-w-xl">
      <!-- Sync toggle -->
      <div class="flex flex-row items-center justify-between">
        <div class="pr-4">
          <h3 class="text-sm font-medium leading-6 text-zinc-100">
            Sync saves to the cloud
          </h3>
          <p class="mt-1 text-sm leading-6 text-zinc-400">
            When enabled, Drop uploads and restores save files between devices
            using your Drop server. Disable to keep all saves local.
          </p>
        </div>
        <Switch
          v-model="cloudSavesEnabled"
          :class="[
            cloudSavesEnabled ? 'bg-blue-600' : 'bg-zinc-700',
            'relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out',
          ]"
        >
          <span
            :class="[
              cloudSavesEnabled ? 'translate-x-5' : 'translate-x-0',
              'pointer-events-none relative inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
            ]"
          />
        </Switch>
      </div>

      <!-- Who a synced save belongs to. This is not a setting, it's the model,
           and it needs saying before someone finds a housemate's progress in
           their library. -->
      <div class="rounded-lg border border-zinc-700 bg-zinc-800/40 p-4">
        <h3 class="text-sm font-medium leading-6 text-zinc-100">
          Who your saves are shared with
        </h3>
        <p class="mt-2 text-sm leading-6 text-zinc-400">
          Emulator saves are backed up to your account alone. Nobody else
          signed in to this Drop server can see your copy.
        </p>
        <p class="mt-2 text-sm leading-6 text-zinc-400">
          Switch games are the exception on this computer. Their saves live
          inside the emulator's own system storage, which every account here
          shares, so a second account playing the same game is playing the same
          save file and will back up its own copy of it.
        </p>
        <p class="mt-2 text-sm leading-6 text-zinc-400">
          PC game saves are shared with everyone on this Drop server, because
          Drop finds them by where the game puts them on this computer rather
          than by who is signed in. If two accounts have the same PC save, the
          one played most recently is the one everyone sees.
        </p>
        <p class="mt-2 text-sm leading-6 text-zinc-400">
          Deleting a shared PC save removes your copy only. If another account
          still has it, the save can come back the next time you sync.
        </p>
      </div>

      <!-- Device name -->
      <div>
        <label for="deviceName" class="block text-sm font-medium text-zinc-100">
          This device's name
        </label>
        <div class="mt-2">
          <input
            id="deviceName"
            v-model="deviceName"
            type="text"
            maxlength="64"
            :placeholder="hostnamePlaceholder"
            class="block w-full rounded-md border-0 py-1.5 text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 bg-zinc-800 placeholder:text-zinc-500 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:text-sm sm:leading-6"
          />
        </div>
        <p class="mt-2 text-sm text-zinc-400">
          Shown in the cloud save conflict dialog as the machine a save was
          uploaded from. Leave blank to use your computer's name.
        </p>
      </div>

      <!-- Save button -->
      <div>
        <button
          type="button"
          :disabled="saveState.loading"
          :class="[
            'inline-flex items-center rounded-md px-3 py-2 text-sm font-semibold text-white shadow-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 transition-colors duration-300',
            saveState.success
              ? 'bg-green-600 hover:bg-green-500 focus-visible:outline-green-600'
              : 'bg-blue-600 hover:bg-blue-500 focus-visible:outline-blue-600',
            'disabled:bg-blue-600/50 disabled:cursor-not-allowed',
          ]"
          @click="saveSettings"
        >
          {{ saveState.success ? "Saved" : "Save Changes" }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  ArrowPathIcon,
  CheckCircleIcon,
  CloudIcon,
  ExclamationTriangleIcon,
} from "@heroicons/vue/24/outline";
import { Switch } from "@headlessui/vue";
import { invoke } from "@tauri-apps/api/core";
import { hostname } from "@tauri-apps/plugin-os";
import {
  hasOwnSaves,
  ownSaveBytes,
  ownSaveCount,
} from "~/composables/cloud-save-ownership";
import {
  cloudSaveQuotaLine,
  cloudSaveQuotaPercent,
  formatCloudSaveBytes,
} from "~/composables/cloud-save-quota";
import { useSaveSyncStatus } from "~/composables/save-sync-status";
import {
  useServerApi,
  type CloudSaveGameSummary,
  type CloudSaveQuota,
} from "~/composables/use-server-api";
import { type Settings } from "~/types";

const settings = await invoke<Settings>("fetch_settings");

// Opt-in: an absent value means the user has never turned this on, and the
// Rust side reads it the same way. Defaulting the toggle to on here would show
// people a switch that disagrees with what the sync is actually doing.
const cloudSavesEnabled = ref<boolean>(settings.cloudSavesEnabled === true);
// Treat null / undefined / "" all as "use default" — show as empty input,
// the backend will fall back to the OS hostname.
const deviceName = ref<string>(settings.deviceName ?? "");

// Try to read the OS hostname for the placeholder so the user knows what
// they'd inherit by leaving the field blank. Falls back to a generic
// hint if the plugin call fails (some sandboxes / older platforms).
const hostnamePlaceholder = ref<string>("Auto-detected");
onMounted(async () => {
  try {
    const h = await hostname();
    if (h && h.trim().length > 0) {
      hostnamePlaceholder.value = h;
    }
  } catch {
    // Keep "Auto-detected" placeholder.
  }
});

// ── Backup status ───────────────────────────────────────────────────────────
//
// Two requests, no local disk scan. The per-game panel needs a multi-second
// Ludusavi run before it can say anything, which is why it could never have
// been the library-wide answer.

const api = useServerApi();
const { failures, clearFailure, clearAll } = useSaveSyncStatus();

const summaries = ref<CloudSaveGameSummary[]>([]);
const quota = ref<CloudSaveQuota | null>(null);
const statusLoading = ref(false);
const statusError = ref<string | null>(null);
/** Set once a fetch has actually returned a per-game answer. */
const statusLoaded = ref(false);
const filter = ref("");
const showAll = ref(false);

/** How many game cards to show before the grid starts hiding things. */
const INITIAL_GAMES = 9;

async function loadStatus() {
  if (statusLoading.value) return;
  statusLoading.value = true;
  statusError.value = null;
  try {
    // Independent halves: a quota endpoint an older server does not have must
    // not blank out the per-game answer, and vice versa.
    const [games, q] = await Promise.all([
      api.saves.summaries(),
      api.saves.quota().catch(() => null),
    ]);
    summaries.value = games;
    quota.value = q;
    statusLoaded.value = true;
  } catch (e) {
    statusError.value =
      e instanceof Error
        ? `Could not load your backup status: ${e.message}`
        : `Could not load your backup status: ${String(e)}`;
  } finally {
    statusLoading.value = false;
  }
}

const failureList = computed(() =>
  Object.values(failures.value).sort((a, b) => b.at - a.at),
);

// Own rows only, everywhere a number claims a backup. The summary lists every
// game this account can READ, and PC saves are readable across every account on
// the server, so a second user on a family server would otherwise be told nine
// games are backed up when they own none of it.
const ownedGames = computed(() => summaries.value.filter(hasOwnSaves));
const totalFiles = computed(() =>
  ownedGames.value.reduce((n, g) => n + ownSaveCount(g), 0),
);
const totalBytes = computed(() =>
  ownedGames.value.reduce((n, g) => n + ownSaveBytes(g), 0),
);

/**
 * The one-line answer, before any of the detail underneath it.
 *
 * "Nothing is backed up yet" is a claim about the server, so it is only reached
 * once a fetch has come back. A request that failed, or has not resolved, gets
 * its own headline: telling someone their saves are not backed up when the
 * truth is unknown is the worst direction for this to fall, and the red error
 * line underneath is not what people read.
 */
const headline = computed(() => {
  if (statusLoading.value && summaries.value.length === 0) {
    return {
      text: "Checking your backups…",
      detail: "Reading what your Drop server is holding.",
      icon: CloudIcon,
      iconClass: "text-zinc-400",
    };
  }
  // A recorded failure outranks a failed fetch: it is a definite answer about
  // a specific game, and the fetch error still gets its own red line below.
  if (failureList.value.length > 0) {
    return {
      text: `${failureList.value.length} ${
        failureList.value.length === 1 ? "game is" : "games are"
      } not backed up`,
      detail: "The last sync for these did not finish. Details below.",
      icon: ExclamationTriangleIcon,
      iconClass: "text-red-400",
    };
  }
  if (statusError.value !== null) {
    return {
      text: "Drop could not check your backups",
      detail:
        "Your Drop server did not answer, so this page cannot say what is backed up. Press Refresh once it is reachable.",
      icon: ExclamationTriangleIcon,
      iconClass: "text-amber-400",
    };
  }
  if (!statusLoaded.value) {
    return {
      text: "Backup status not checked yet",
      detail: "Press Refresh to read what your Drop server is holding.",
      icon: CloudIcon,
      iconClass: "text-zinc-400",
    };
  }
  if (ownedGames.value.length === 0) {
    return {
      text: "Nothing is backed up yet",
      detail:
        "Play a game with cloud saves turned on, or press Sync in a game's Cloud Saves panel.",
      icon: CloudIcon,
      iconClass: "text-zinc-400",
    };
  }
  return {
    text: `${ownedGames.value.length} ${
      ownedGames.value.length === 1 ? "game" : "games"
    } backed up`,
    detail: `${totalFiles.value} ${
      totalFiles.value === 1 ? "file" : "files"
    } · ${formatCloudSaveBytes(totalBytes.value)}`,
    icon: CheckCircleIcon,
    iconClass: "text-emerald-400",
  };
});

const quotaPercent = computed(() => cloudSaveQuotaPercent(quota.value));
const quotaLine = computed(() => cloudSaveQuotaLine(quota.value));

const quotaBarClass = computed(() => {
  if (quotaPercent.value >= 95) return "bg-red-500";
  if (quotaPercent.value >= 80) return "bg-amber-500";
  return "bg-blue-500";
});

const filteredSummaries = computed(() => {
  const needle = filter.value.trim().toLowerCase();
  if (!needle) return summaries.value;
  return summaries.value.filter((g) =>
    g.gameName.toLowerCase().includes(needle),
  );
});

const visibleSummaries = computed(() =>
  showAll.value
    ? filteredSummaries.value
    : filteredSummaries.value.slice(0, INITIAL_GAMES),
);

/**
 * A failed game's display name, when the summary list happens to know it.
 * A game whose sync has never succeeded has no summary row at all, so the id
 * is genuinely all we have; showing it beats dropping the failure.
 */
function nameFor(gameId: string): string {
  return summaries.value.find((g) => g.gameId === gameId)?.gameName ?? gameId;
}


function timeAgo(ms: number): string {
  if (!ms || Number.isNaN(ms)) return "at an unknown time";
  const diff = Math.floor((Date.now() - ms) / 1000);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  if (diff < 2592000) return `${Math.floor(diff / 604800)}w ago`;
  return `${Math.floor(diff / 2592000)}mo ago`;
}

onMounted(loadStatus);

const saveState = reactive({
  loading: false,
  success: false,
});

async function saveSettings() {
  try {
    saveState.loading = true;
    const trimmed = deviceName.value.trim();
    await invoke("update_settings", {
      newSettings: {
        cloudSavesEnabled: cloudSavesEnabled.value,
        // Send null when blank so the backend stores `None` and falls
        // back to the OS hostname in the conflict UI.
        deviceName: trimmed.length === 0 ? null : trimmed,
      },
    });
    saveState.success = true;
    setTimeout(() => {
      saveState.success = false;
    }, 2000);
  } catch (error) {
    console.error("Failed to save cloud save settings:", error);
  } finally {
    saveState.loading = false;
  }
}

// Persist the moment the sync toggle flips. The Switch reads as an instant
// control, but previously only the "Save Changes" button called
// update_settings — so toggling sync OFF appeared to take effect when it
// hadn't (the launch path kept reading the old value). Auto-saving on change
// closes that trap; the button still covers the device-name field.
watch(cloudSavesEnabled, () => {
  saveSettings();
});
</script>
