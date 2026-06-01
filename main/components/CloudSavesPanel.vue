<template>
  <section class="bg-zinc-800/50 rounded-xl backdrop-blur-sm overflow-hidden">
    <!-- Header (also the collapse toggle). One primary action — Sync — plus
         the collapse chevron. The old refresh / "Sync now" / "Upload these"
         buttons are gone: Sync reconciles both directions and refreshes the
         list, and per-row actions cover the granular cases. -->
    <button
      type="button"
      class="w-full flex items-center justify-between gap-3 px-6 py-4 text-left transition-colors hover:bg-zinc-700/30"
      @click="expanded = !expanded"
    >
      <div class="flex items-center gap-3 min-w-0">
        <CloudIcon class="size-5 text-cyan-400 shrink-0" />
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <h3 class="text-base font-semibold text-zinc-100">Cloud Saves</h3>
            <span
              v-if="!loading && rows.length > 0"
              class="inline-flex items-center justify-center min-w-[1.5rem] h-5 px-1.5 rounded-full bg-zinc-700 text-xs font-medium text-zinc-300"
            >
              {{ rows.length }}
            </span>
          </div>
          <!-- At-a-glance state: counts by sync status + last-synced time. -->
          <p
            v-if="summaryText"
            class="mt-0.5 text-xs text-zinc-500 truncate"
          >
            {{ summaryText }}
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <button
          type="button"
          class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-cyan-600/80 text-white hover:bg-cyan-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="loading || syncing"
          :title="
            isNativeGame && ludusaviChecked && !ludusaviAvailable
              ? 'Install Ludusavi first to sync PC saves'
              : 'Back up new local saves and pull down cloud-only saves'
          "
          @click.stop="reconcile"
        >
          <ArrowPathIcon
            class="size-3.5"
            :class="syncing ? 'animate-spin' : ''"
          />
          {{ syncing ? "Syncing…" : "Sync" }}
        </button>
        <ChevronDownIcon
          class="size-5 text-zinc-400 transition-transform"
          :class="expanded ? 'rotate-180' : ''"
        />
      </div>
    </button>

    <Transition
      enter-active-class="overflow-hidden transition-all duration-200 ease-out"
      leave-active-class="overflow-hidden transition-all duration-150 ease-in"
      enter-from-class="max-h-0 opacity-0"
      enter-to-class="max-h-[80rem] opacity-100"
      leave-from-class="max-h-[80rem] opacity-100"
      leave-to-class="max-h-0 opacity-0"
    >
      <div v-if="expanded" class="border-t border-zinc-700/50">
        <!-- Sync result / error line. -->
        <p
          v-if="syncMessage"
          class="px-6 pt-3 text-xs"
          :class="syncError ? 'text-red-400' : 'text-cyan-300'"
        >
          {{ syncMessage }}
        </p>

        <!-- Load error banner. -->
        <div
          v-if="loadError"
          class="mx-6 mt-4 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300"
        >
          {{ loadError }}
        </div>

        <!-- Ludusavi-missing prompt. Native (PC) games can't have their saves
             discovered without Ludusavi, and Drop doesn't bundle it. -->
        <div
          v-if="isNativeGame && ludusaviChecked && !ludusaviAvailable"
          class="mx-6 mt-4 rounded-lg border border-cyan-500/30 bg-cyan-500/5 px-4 py-3"
        >
          <div class="flex items-start justify-between gap-4">
            <div class="min-w-0">
              <p class="text-sm font-medium text-cyan-200">
                PC save sync needs Ludusavi
              </p>
              <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
                Drop uses Ludusavi to find where this game keeps its save
                files. It isn't bundled — install it once (a ~15&nbsp;MB
                download) to enable cloud saves for PC games. Emulator
                saves don't need it.
              </p>
              <p v-if="ludusaviError" class="text-xs text-red-400 mt-1.5">
                {{ ludusaviError }}
              </p>
            </div>
            <button
              type="button"
              class="shrink-0 inline-flex items-center gap-1.5 rounded-md px-3 py-2 text-xs font-semibold bg-cyan-600 text-white hover:bg-cyan-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              :disabled="ludusaviInstalling"
              @click="installLudusavi"
            >
              <ArrowDownTrayIcon
                class="size-3.5"
                :class="ludusaviInstalling ? 'animate-pulse' : ''"
              />
              {{ ludusaviInstalling ? "Installing…" : "Install Ludusavi" }}
            </button>
          </div>
        </div>

        <!-- Loading. -->
        <div
          v-if="loading && rows.length === 0"
          class="px-6 py-10 text-center text-sm text-zinc-500"
        >
          Loading cloud saves…
        </div>

        <!-- Empty. -->
        <div
          v-else-if="!loading && rows.length === 0 && !loadError"
          class="px-6 py-10 text-center"
        >
          <CloudIcon class="mx-auto size-10 text-zinc-600 mb-3" />
          <p class="text-sm text-zinc-400">No saves yet.</p>
          <p
            v-if="isNativeGame && ludusaviChecked && !ludusaviAvailable"
            class="text-xs text-zinc-500 mt-1"
          >
            Install Ludusavi above, then play the game once so it writes its
            saves.
          </p>
          <p v-else class="text-xs text-zinc-500 mt-1">
            Play the game once so it writes its save files, then hit
            <span class="text-zinc-400 font-medium">Sync</span>.
          </p>
        </div>

        <!-- Unified status list — one row per save, deduped across cloud and
             local by its stable filename, tagged with its sync state. -->
        <ul v-else class="divide-y divide-zinc-700/40">
          <li
            v-for="row in rows"
            :key="row.key"
            class="flex items-start gap-4 px-6 py-3.5"
          >
            <!-- State icon. -->
            <div
              class="size-9 rounded-lg flex items-center justify-center shrink-0 mt-0.5"
              :class="stateMeta(row.state).chipBg"
            >
              <component
                :is="stateMeta(row.state).icon"
                class="size-5"
                :class="stateMeta(row.state).iconClass"
              />
            </div>

            <!-- Info. -->
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span
                  class="text-sm font-medium text-zinc-100 truncate"
                  :title="row.key"
                >
                  {{ row.name }}
                </span>
                <span
                  class="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded-full bg-zinc-700/60 text-zinc-400"
                >
                  {{ row.saveType }}
                </span>
              </div>
              <div class="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs">
                <span class="text-zinc-500">{{ formatSize(row.size) }}</span>
                <span class="text-zinc-500" :title="exact(row.whenMs)">
                  {{ timeAgo(row.whenMs) }}
                </span>
                <span :class="stateMeta(row.state).labelClass">
                  {{ stateMeta(row.state).label }}
                </span>
              </div>
              <p v-if="rowError[row.key]" class="mt-1.5 text-xs text-red-400">
                {{ rowError[row.key] }}
              </p>
            </div>

            <!-- State-appropriate actions. -->
            <div class="flex items-center gap-1.5 shrink-0">
              <!-- Not backed up → push. -->
              <button
                v-if="row.state === 'localOnly'"
                type="button"
                class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-cyan-600/80 text-white hover:bg-cyan-500 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                :disabled="isRowBusy(row)"
                @click="backupRows([row.key])"
              >
                <ArrowUpTrayIcon class="size-3.5" />
                {{ rowBusy[row.key] === "backup" ? "Backing up…" : "Back up" }}
              </button>

              <!-- In cloud only → pull. -->
              <button
                v-if="row.state === 'cloudOnly'"
                type="button"
                class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-blue-600/80 text-white hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                :disabled="isRowBusy(row)"
                @click="restoreRow(row)"
              >
                <ArrowDownTrayIcon class="size-3.5" />
                {{ rowBusy[row.key] === "restore" ? "Restoring…" : "Restore" }}
              </button>

              <!-- Conflict → explicit choice; never auto-resolved by Sync. -->
              <template v-if="row.state === 'conflict'">
                <button
                  type="button"
                  class="inline-flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs font-medium bg-zinc-700 text-zinc-200 hover:bg-cyan-600 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                  :disabled="isRowBusy(row)"
                  title="Overwrite the cloud copy with this PC's version"
                  @click="backupRows([row.key])"
                >
                  <ArrowUpTrayIcon class="size-3.5" />
                  Keep&nbsp;PC
                </button>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs font-medium bg-zinc-700 text-zinc-200 hover:bg-blue-600 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                  :disabled="isRowBusy(row)"
                  title="Overwrite this PC's copy with the cloud version"
                  @click="restoreRow(row)"
                >
                  <ArrowDownTrayIcon class="size-3.5" />
                  Keep&nbsp;cloud
                </button>
              </template>

              <!-- Delete cloud copy — available wherever a cloud copy exists,
                   kept muted so it doesn't shout on every row. -->
              <button
                v-if="row.cloud && row.state !== 'conflict'"
                type="button"
                class="rounded-md p-1.5 text-zinc-500 hover:text-white hover:bg-red-600 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                :disabled="isRowBusy(row)"
                title="Delete the cloud copy"
                @click="askDelete(row)"
              >
                <TrashIcon class="size-4" />
              </button>
            </div>
          </li>
        </ul>
      </div>
    </Transition>

    <!-- Delete confirmation. -->
    <Transition
      enter-active-class="ease-out duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="deleteTarget"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
        @click.self="deleteTarget = null"
      >
        <div
          class="w-full max-w-sm rounded-xl bg-zinc-900 border border-zinc-700 shadow-2xl"
        >
          <div class="px-6 py-5">
            <h3 class="text-base font-semibold font-display text-zinc-100">
              Delete Cloud Save?
            </h3>
            <p class="mt-2 text-sm text-zinc-400">
              Permanently delete the cloud copy of
              <span class="text-zinc-200 font-medium">{{
                deleteTarget.name
              }}</span
              >? This cannot be undone. Your local save file isn't touched.
            </p>
          </div>
          <div class="flex justify-end gap-3 border-t border-zinc-700 px-6 py-4">
            <button
              type="button"
              class="rounded-md px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-800 transition-colors"
              @click="deleteTarget = null"
            >
              Cancel
            </button>
            <button
              type="button"
              class="rounded-md px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 disabled:opacity-50 transition-colors"
              :disabled="rowBusy[deleteTarget.key] === 'delete'"
              @click="confirmDelete"
            >
              {{ rowBusy[deleteTarget.key] === "delete" ? "Deleting…" : "Delete" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </section>
</template>

<script setup lang="ts">
/**
 * Cloud Saves panel for the per-game library page (desktop variant).
 *
 * Presents ONE unified list: every save the user has for this game appears
 * exactly once, deduped across the cloud and this PC by its stable filename,
 * and tagged with a sync state:
 *
 *   - Synced        — local copy and cloud copy match (same hash).
 *   - Not backed up  — exists on this PC, not in the cloud yet.
 *   - In cloud only  — in the cloud, not on this PC (fresh/other device).
 *   - Conflict       — both exist but differ; the user picks a side.
 *
 * The header's single **Sync** button reconciles both directions — it backs
 * up not-backed-up files and pulls down cloud-only ones — but deliberately
 * leaves conflicts for an explicit per-row choice so it can never silently
 * clobber a copy. Restore is type-aware: emulator saves write to
 * `{install}/drop-saves/{gameId}/…`; PC saves re-scan with Ludusavi via
 * `restore_pc_cloud_save` so they land where the game actually reads them.
 */
import {
  ArrowDownTrayIcon,
  ArrowPathIcon,
  ArrowUpTrayIcon,
  CheckCircleIcon,
  ChevronDownIcon,
  CloudArrowDownIcon,
  CloudArrowUpIcon,
  CloudIcon,
  ExclamationTriangleIcon,
  TrashIcon,
} from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import {
  useServerApi,
  type CloudSaveListEntry,
} from "~/composables/use-server-api";

const props = withDefaults(
  defineProps<{
    gameId: string;
    /**
     * Display name — passed to the scan/backup commands, which resolve it to
     * Ludusavi's canonical manifest title internally. Empty string is
     * tolerated (PC saves just won't match anything).
     */
    gameName?: string;
    /**
     * Whether this game is native/PC (not emulated). Native games rely on
     * Ludusavi for save discovery; emulator games use Drop's own drop-saves
     * scan and don't need it. Drives the "Install Ludusavi" prompt. Defaults
     * to true so the prompt still surfaces if the parent forgets the flag.
     */
    isNativeGame?: boolean;
  }>(),
  { gameName: "", isNativeGame: true },
);

const api = useServerApi();

// ── State ───────────────────────────────────────────────────────────────────

type SyncState = "synced" | "localOnly" | "cloudOnly" | "conflict";

/** A save file detected on disk (from `scan_local_game_saves`). */
interface LocalSaveEntry {
  filename: string;
  saveType: string;
  size: number;
  modifiedAt: number; // unix seconds
  dataHash: string;
}

/** One merged row in the unified list. */
interface UnifiedRow {
  key: string; // stable filename identity, e.g. "pc__gen.sav" / "Game.srm"
  name: string; // display name (namespace prefix stripped)
  saveType: string;
  state: SyncState;
  size: number;
  whenMs: number; // most recent activity, ms epoch
  cloud: CloudSaveListEntry | null;
  local: LocalSaveEntry | null;
}

const expanded = ref(true);
const loading = ref(false);
const loadError = ref<string | null>(null);
const entries = ref<CloudSaveListEntry[]>([]);
const localEntries = ref<LocalSaveEntry[]>([]);

// Ludusavi availability — probed on mount; gates the install prompt.
const ludusaviAvailable = ref(false);
const ludusaviChecked = ref(false);
const ludusaviInstalling = ref(false);
const ludusaviError = ref<string | null>(null);

// Header Sync state.
const syncing = ref(false);
const syncMessage = ref<string | null>(null);
const syncError = ref(false);

// Per-row busy / error, keyed by row.key (filename).
const rowBusy = ref<Record<string, "backup" | "restore" | "delete">>({});
const rowError = ref<Record<string, string>>({});

const deleteTarget = ref<UnifiedRow | null>(null);

// ── Derived: the unified list + summary ───────────────────────────────────────

function cloudMs(c: CloudSaveListEntry): number {
  const t = new Date(c.clientModifiedAt).getTime();
  return Number.isNaN(t) ? 0 : t;
}
function localMs(l: LocalSaveEntry): number {
  return (l.modifiedAt || 0) * 1000;
}

const rows = computed<UnifiedRow[]>(() => {
  const map = new Map<
    string,
    { cloud: CloudSaveListEntry | null; local: LocalSaveEntry | null }
  >();
  for (const c of entries.value) {
    const e = map.get(c.filename) ?? { cloud: null, local: null };
    e.cloud = c;
    map.set(c.filename, e);
  }
  for (const l of localEntries.value) {
    const e = map.get(l.filename) ?? { cloud: null, local: null };
    e.local = l;
    map.set(l.filename, e);
  }

  const out: UnifiedRow[] = [];
  for (const [key, { cloud, local }] of map) {
    let state: SyncState;
    let size: number;
    let whenMs: number;
    let saveType: string;
    if (cloud && local) {
      const same =
        !!cloud.dataHash && !!local.dataHash && cloud.dataHash === local.dataHash;
      state = same ? "synced" : "conflict";
      size = local.size || cloud.size;
      whenMs = Math.max(cloudMs(cloud), localMs(local));
      saveType = cloud.saveType || local.saveType;
    } else if (cloud) {
      state = "cloudOnly";
      size = cloud.size;
      whenMs = cloudMs(cloud);
      saveType = cloud.saveType;
    } else {
      state = "localOnly";
      size = local!.size;
      whenMs = localMs(local!);
      saveType = local!.saveType;
    }
    out.push({ key, name: displayName(key), saveType, state, size, whenMs, cloud, local });
  }

  // Float the rows that need attention to the top, alphabetic within a state.
  const order: Record<SyncState, number> = {
    conflict: 0,
    localOnly: 1,
    cloudOnly: 2,
    synced: 3,
  };
  out.sort(
    (a, b) => order[a.state] - order[b.state] || a.name.localeCompare(b.name),
  );
  return out;
});

const counts = computed(() => {
  const c = { synced: 0, localOnly: 0, cloudOnly: 0, conflict: 0 };
  for (const r of rows.value) c[r.state]++;
  return c;
});

const lastSyncedLabel = computed(() => {
  let max = 0;
  for (const e of entries.value) {
    const t = new Date(e.uploadedAt).getTime();
    if (!Number.isNaN(t) && t > max) max = t;
  }
  return max > 0 ? timeAgo(max) : "";
});

const summaryText = computed(() => {
  const total = rows.value.length;
  if (total === 0) return "";
  const c = counts.value;
  const segs: string[] = [];
  if (c.conflict) segs.push(`${c.conflict} conflict${c.conflict === 1 ? "" : "s"}`);
  if (c.localOnly) segs.push(`${c.localOnly} not backed up`);
  if (c.cloudOnly) segs.push(`${c.cloudOnly} in cloud only`);
  const head =
    segs.length === 0
      ? total === 1
        ? "1 save · backed up"
        : `${total} saves · all backed up`
      : `${total} save${total === 1 ? "" : "s"} · ${segs.join(" · ")}`;
  const ls = lastSyncedLabel.value;
  return ls ? `${head} · synced ${ls}` : head;
});

function stateMeta(state: SyncState): {
  label: string;
  icon: typeof CheckCircleIcon;
  iconClass: string;
  chipBg: string;
  labelClass: string;
} {
  switch (state) {
    case "synced":
      return {
        label: "Synced",
        icon: CheckCircleIcon,
        iconClass: "text-emerald-400",
        chipBg: "bg-emerald-500/10",
        labelClass: "text-emerald-400",
      };
    case "localOnly":
      return {
        label: "Not backed up",
        icon: CloudArrowUpIcon,
        iconClass: "text-amber-400",
        chipBg: "bg-amber-500/10",
        labelClass: "text-amber-400",
      };
    case "cloudOnly":
      return {
        label: "In cloud only",
        icon: CloudArrowDownIcon,
        iconClass: "text-sky-400",
        chipBg: "bg-sky-500/10",
        labelClass: "text-sky-400",
      };
    case "conflict":
      return {
        label: "Conflict",
        icon: ExclamationTriangleIcon,
        iconClass: "text-orange-400",
        chipBg: "bg-orange-500/10",
        labelClass: "text-orange-400",
      };
  }
}

function isRowBusy(row: UnifiedRow): boolean {
  return rowBusy.value[row.key] !== undefined;
}

// ── Filename identity helpers ────────────────────────────────────────────────

/**
 * Strip the PC-save namespace prefix for display. The scanner namespaces PC
 * saves so they don't collide with emulator saves; `pc__` is the current,
 * sanitize-safe prefix and `pc/` the legacy one.
 */
function displayName(filename: string): string {
  if (filename.startsWith("pc__")) return filename.slice(4);
  if (filename.startsWith("pc/")) return filename.slice(3);
  return filename;
}

function isPcSave(entry: { saveType: string; filename: string }): boolean {
  return (
    entry.saveType === "pc" ||
    entry.filename.startsWith("pc__") ||
    entry.filename.startsWith("pc/")
  );
}

function isPcKey(key: string): boolean {
  return key.startsWith("pc__") || key.startsWith("pc/");
}

// ── Data loading ──────────────────────────────────────────────────────────────

async function refresh() {
  loading.value = true;
  loadError.value = null;
  try {
    // Cloud list + local disk scan in parallel. The local scan is best-effort
    // — a Ludusavi miss shouldn't blank the cloud list, so it soft-fails empty.
    const [cloud, local] = await Promise.all([
      api.saves.list(props.gameId),
      invoke<LocalSaveEntry[]>("scan_local_game_saves", {
        gameId: props.gameId,
        gameName: props.gameName,
      }).catch(() => [] as LocalSaveEntry[]),
    ]);
    entries.value = cloud;
    localEntries.value = local;
  } catch (e) {
    loadError.value =
      e instanceof Error
        ? e.message
        : `Failed to load cloud saves: ${String(e)}`;
  } finally {
    loading.value = false;
  }
}

async function checkLudusavi() {
  try {
    ludusaviAvailable.value = await invoke<boolean>("check_ludusavi");
  } catch {
    ludusaviAvailable.value = false;
  } finally {
    ludusaviChecked.value = true;
  }
}

async function installLudusavi() {
  if (ludusaviInstalling.value) return;
  ludusaviInstalling.value = true;
  ludusaviError.value = null;
  try {
    await invoke("install_ludusavi");
    ludusaviAvailable.value = true;
    await refresh();
  } catch (e) {
    ludusaviError.value =
      e instanceof Error
        ? `Install failed: ${e.message}`
        : `Install failed: ${String(e)}`;
  } finally {
    ludusaviInstalling.value = false;
  }
}

// ── Sync (two-way reconcile) ──────────────────────────────────────────────────

/**
 * Header Sync: back up everything not yet in the cloud, pull down everything
 * that's cloud-only, and leave conflicts alone (those need an explicit choice
 * so we never silently overwrite a copy). Pulls are best-effort — a cloud-only
 * save for an uninstalled game can't be placed, and that's reported, not fatal.
 */
async function reconcile() {
  if (syncing.value) return;
  if (props.isNativeGame && ludusaviChecked.value && !ludusaviAvailable.value) {
    syncError.value = true;
    syncMessage.value = "Install Ludusavi first (below) to sync PC saves.";
    return;
  }
  syncing.value = true;
  syncMessage.value = null;
  syncError.value = false;
  try {
    // Re-scan so the push/pull lists reflect what's actually on disk now.
    await refresh();
    const toPush = rows.value
      .filter((r) => r.state === "localOnly")
      .map((r) => r.key);
    const toPull = rows.value
      .filter((r) => r.state === "cloudOnly" && r.cloud)
      .map((r) => r.cloud as CloudSaveListEntry);

    let pushed = 0;
    let pulled = 0;
    let pullFailed = 0;

    if (toPush.length > 0) {
      pushed = await invoke<number>("backup_saves", {
        gameId: props.gameId,
        gameName: props.gameName,
        filenames: toPush,
      });
    }
    for (const c of toPull) {
      try {
        await doRestore(c);
        pulled++;
      } catch {
        pullFailed++;
      }
    }

    await refresh();

    const conflicts = counts.value.conflict;
    const segs: string[] = [];
    if (pushed > 0) segs.push(`backed up ${pushed}`);
    if (pulled > 0) segs.push(`restored ${pulled}`);
    syncError.value = false;
    if (segs.length === 0 && conflicts === 0 && pullFailed === 0) {
      syncMessage.value = "Everything's already in sync.";
    } else {
      let msg = segs.length > 0 ? `Sync complete — ${segs.join(", ")}.` : "Sync complete.";
      if (conflicts > 0)
        msg += ` ${conflicts} conflict${conflicts === 1 ? "" : "s"} need${conflicts === 1 ? "s" : ""} a choice below.`;
      if (pullFailed > 0)
        msg += ` ${pullFailed} couldn't be restored (game not installed?).`;
      syncMessage.value = msg;
    }
  } catch (e) {
    syncError.value = true;
    syncMessage.value =
      e instanceof Error ? `Sync failed: ${e.message}` : `Sync failed: ${String(e)}`;
  } finally {
    syncing.value = false;
  }
}

// ── Per-row actions ───────────────────────────────────────────────────────────

/** Push the given files to the cloud (per-row "Back up" and conflict "Keep PC"). */
async function backupRows(keys: string[]) {
  const targets = keys.filter((k) => rowBusy.value[k] === undefined);
  if (targets.length === 0) return;
  if (
    props.isNativeGame &&
    ludusaviChecked.value &&
    !ludusaviAvailable.value &&
    targets.some(isPcKey)
  ) {
    for (const k of targets)
      rowError.value[k] = "Install Ludusavi first to back up PC saves.";
    return;
  }
  for (const k of targets) {
    rowBusy.value[k] = "backup";
    delete rowError.value[k];
  }
  try {
    await invoke("backup_saves", {
      gameId: props.gameId,
      gameName: props.gameName,
      filenames: targets,
    });
    await refresh();
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    for (const k of targets) rowError.value[k] = `Back up failed: ${m}`;
  } finally {
    for (const k of targets) delete rowBusy.value[k];
  }
}

/** Pull a cloud save down to disk (per-row "Restore" and conflict "Keep cloud"). */
async function restoreRow(row: UnifiedRow) {
  if (!row.cloud || rowBusy.value[row.key] !== undefined) return;
  rowBusy.value[row.key] = "restore";
  delete rowError.value[row.key];
  try {
    await doRestore(row.cloud);
    await refresh();
  } catch (e) {
    rowError.value[row.key] =
      e instanceof Error
        ? `Restore failed: ${e.message}`
        : `Restore failed: ${String(e)}`;
  } finally {
    delete rowBusy.value[row.key];
  }
}

/** Download + write a cloud save to its real on-disk location (type-aware). */
async function doRestore(entry: CloudSaveListEntry) {
  const { data } = await api.saves.download(entry.id);
  if (isPcSave(entry)) {
    await invoke("restore_pc_cloud_save", {
      gameId: props.gameId,
      filename: entry.filename,
      data,
    });
  } else {
    await invoke("write_save_file", {
      gameId: props.gameId,
      filename: entry.filename,
      saveType: entry.saveType,
      data,
    });
  }
}

function askDelete(row: UnifiedRow) {
  if (row.cloud) deleteTarget.value = row;
}

async function confirmDelete() {
  const row = deleteTarget.value;
  if (!row?.cloud || rowBusy.value[row.key] !== undefined) return;
  rowBusy.value[row.key] = "delete";
  delete rowError.value[row.key];
  try {
    await api.saves.delete(row.cloud.id);
    deleteTarget.value = null;
    await refresh();
  } catch (e) {
    rowError.value[row.key] =
      e instanceof Error
        ? `Delete failed: ${e.message}`
        : `Delete failed: ${String(e)}`;
    // Leave the modal open so the user can retry.
  } finally {
    delete rowBusy.value[row.key];
  }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

onMounted(() => {
  refresh();
  checkLudusavi();
});

watch(
  () => props.gameId,
  () => {
    entries.value = [];
    localEntries.value = [];
    rowBusy.value = {};
    rowError.value = {};
    syncMessage.value = null;
    syncError.value = false;
    refresh();
    checkLudusavi();
  },
);

// ── Formatters ────────────────────────────────────────────────────────────────

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function timeAgo(ms: number): string {
  if (!ms || Number.isNaN(ms)) return "—";
  const diff = Math.floor((Date.now() - ms) / 1000);
  if (diff < 0) return "just now";
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  if (diff < 2592000) return `${Math.floor(diff / 604800)}w ago`;
  return `${Math.floor(diff / 2592000)}mo ago`;
}

function exact(ms: number): string {
  if (!ms || Number.isNaN(ms)) return "—";
  try {
    return new Date(ms).toLocaleString();
  } catch {
    return "—";
  }
}
</script>
