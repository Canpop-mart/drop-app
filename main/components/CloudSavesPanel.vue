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
          <!-- Storage. The cap has always been enforced and never shown, so
               the first anyone heard of it was an upload being rejected. -->
          <p v-if="quotaText" class="mt-0.5 text-xs truncate" :class="quotaClass">
            {{ quotaText }}
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <button
          type="button"
          class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-cyan-600/80 text-white hover:bg-cyan-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="loading || syncing || syncEnabled === false"
          :title="
            syncEnabled === false
              ? 'Turn cloud saves on in Settings to sync'
              : isNativeGame && ludusaviChecked && !ludusaviAvailable
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
        <!-- Cloud saves is opt-in and off until the user turns it on. The
             panel is on every game page now, so it has to say when the thing
             it is a panel for is not running. -->
        <div
          v-if="syncEnabled === false"
          class="mx-6 mt-4 rounded-lg border border-amber-500/30 bg-amber-500/5 px-4 py-3"
        >
          <p class="text-sm font-medium text-amber-200">
            Cloud saves are turned off
          </p>
          <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
            Nothing on this PC is being backed up. Turn cloud saves on in
            Settings, under Cloud Saves. Anything already on your server is
            still listed here.
          </p>
        </div>

        <!-- Who owns what. PC saves really are shared between accounts on this
             server, and the panel has to say so before someone is surprised
             by another person's progress landing in their library. -->
        <p class="px-6 pt-4 text-xs leading-relaxed text-zinc-500">
          Emulator saves are backed up to your account alone, with one
          exception: Switch games keep their saves inside the emulator's own
          system storage, which every account on this computer shares. PC game
          saves are shared with everyone on this Drop server, because Drop
          finds them by where the game puts them on this computer rather than
          by who is signed in. If two accounts have the same PC save, the one
          played most recently is the one you see here.
        </p>

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

        <!-- Empty. Three different empty states, because "nothing here" has
             three different causes and only one of them is fixed by playing
             the game. Telling someone to play a game Drop can never read the
             saves of is the failure this replaced. -->
        <div
          v-else-if="!loading && rows.length === 0 && !loadError"
          class="px-6 py-10 text-center"
        >
          <CloudIcon class="mx-auto size-10 text-zinc-600 mb-3" />
          <template v-if="isNativeGame && ludusaviChecked && !ludusaviAvailable">
            <p class="text-sm text-zinc-400">No saves yet.</p>
            <p class="text-xs text-zinc-500 mt-1">
              Install Ludusavi above, then play the game once so it writes its
              saves.
            </p>
          </template>
          <template v-else-if="saveLocationUnknown">
            <p class="text-sm text-zinc-400">
              Drop cannot find where this game stores its saves.
            </p>
            <p
              class="text-xs text-zinc-500 mt-1.5 max-w-md mx-auto leading-relaxed"
            >
              {{ saveLocationUnknownDetail }}
            </p>
          </template>
          <template v-else-if="syncEnabled === false">
            <p class="text-sm text-zinc-400">Nothing backed up yet.</p>
            <p class="text-xs text-zinc-500 mt-1">
              Cloud saves are turned off, so Drop is not backing this game up.
            </p>
          </template>
          <template v-else>
            <p class="text-sm text-zinc-400">No saves yet.</p>
            <p class="text-xs text-zinc-500 mt-1">
              Play the game once so it writes its save files, then hit
              <span class="text-zinc-400 font-medium">Sync</span>.
            </p>
          </template>
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
                <!-- Only on shared rows: emulator saves are always yours, so
                     naming an owner there would be noise. -->
                <span v-if="row.ownedBy" class="text-zinc-500">
                  Saved by {{ row.ownedBy }}
                </span>
              </div>
              <!-- A second copy of the same save must never be invisible.
                   Newest wins, and the mtime that decides it comes from
                   whichever machine uploaded, so silence here would let a
                   fast clock hide someone's real progress. -->
              <p v-if="row.shadowNote" class="mt-1 text-xs text-amber-400/80">
                {{ row.shadowNote }}
              </p>
              <p v-if="rowError[row.key]" class="mt-1.5 text-xs text-red-400">
                {{ rowError[row.key] }}
              </p>
              <!-- Where a restored PC save actually landed. On a machine that
                   has never run the game the destination comes from Ludusavi's
                   catalogue rather than from a file on this disk, and nothing
                   on screen used to name the folder that was written. -->
              <p
                v-if="rowNote[row.key]"
                class="mt-1.5 text-xs text-zinc-500 break-all"
              >
                {{ rowNote[row.key] }}
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
            <p v-if="deleteIsShared" class="mt-2 text-sm text-zinc-400">
              Delete the cloud copy of
              <span class="text-zinc-200 font-medium">{{
                deleteTarget.name
              }}</span
              >? This removes your copy only. PC game saves are shared with
              everyone on this Drop server, so if another account still has
              this save it can come back the next time you sync. The copy on
              this computer stays where it is.
            </p>
            <p v-else class="mt-2 text-sm text-zinc-400">
              Permanently delete the cloud copy of
              <span class="text-zinc-200 font-medium">{{
                deleteTarget.name
              }}</span
              >? This cannot be undone. The copy on this computer stays where
              it is, but your other devices will remove their copy the next
              time you play there.
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
 * `{install}/drop-saves/{userId}/{gameId}/…`; PC saves re-scan with Ludusavi
 * via `restore_pc_cloud_save` so they land where the game actually reads them.
 *
 * Emulator saves belong to one account. PC saves are shared with every account
 * on the server, so a row here can be somebody else's copy: `ownedBy` names
 * them and `shadowNote` says when a second copy of the same save exists.
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
  cloudSaveQuotaLine,
  cloudSaveQuotaPercent,
} from "~/composables/cloud-save-quota";
import {
  useServerApi,
  type CloudSaveListEntry,
  type CloudSaveQuota,
} from "~/composables/use-server-api";
import type { BackupResult } from "~/types/save-sync";

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
  /**
   * Account that owns the cloud copy, shown only when the row can belong to
   * someone else (a PC save). Null for emulator saves and local-only rows.
   */
  ownedBy: string | null;
  /**
   * Says out loud that another account holds a save with this filename, and
   * whether the copy being shown is theirs rather than yours. Null when this
   * row is the only copy on the server.
   */
  shadowNote: string | null;
}

const expanded = ref(true);
const loading = ref(false);
const loadError = ref<string | null>(null);
const entries = ref<CloudSaveListEntry[]>([]);
const localEntries = ref<LocalSaveEntry[]>([]);
const quota = ref<CloudSaveQuota | null>(null);

// Ludusavi availability — probed on mount; gates the install prompt.
const ludusaviAvailable = ref(false);
const ludusaviChecked = ref(false);
const ludusaviInstalling = ref(false);
const ludusaviError = ref<string | null>(null);

/**
 * What Drop is able to find for this game, from `game_save_coverage`. Null
 * until the check answers; a null keeps the empty state on its neutral
 * wording rather than guessing at a cause.
 */
interface SaveCoverage {
  ludusaviInstalled: boolean;
  knownToLudusavi: boolean;
  canonicalTitle: string | null;
  emulated: boolean;
  emulatorSupported: boolean;
}
const coverage = ref<SaveCoverage | null>(null);

/**
 * The master cloud-saves switch. Null until read. The feature is opt-in and
 * off by default, and this panel now appears on every game page, so it has to
 * be able to say "the thing you are looking at is not running".
 */
const syncEnabled = ref<boolean | null>(null);

/**
 * True when Drop has no way to locate this game's saves, so an empty list is
 * permanent rather than a game that hasn't been played yet.
 */
const saveLocationUnknown = computed(() => {
  const c = coverage.value;
  if (!c) return false;
  if (c.emulated) return !c.emulatorSupported;
  // With Ludusavi missing the panel shows its install prompt instead, and
  // that is a different (fixable) answer.
  return c.ludusaviInstalled && !c.knownToLudusavi;
});

const saveLocationUnknownDetail = computed(() => {
  if (coverage.value?.emulated) {
    return "Drop can read saves from RetroArch and from Switch emulators. This game runs on a different emulator, which keeps its saves somewhere Drop does not know about.";
  }
  return "Drop uses Ludusavi's list of games to know where PC saves live, and this game is not on it. Playing it will not change that.";
});

// Header Sync state.
const syncing = ref(false);
const syncMessage = ref<string | null>(null);
const syncError = ref(false);

// Per-row busy / error, keyed by row.key (filename).
const rowBusy = ref<Record<string, "backup" | "restore" | "delete">>({});
const rowError = ref<Record<string, string>>({});
/** Non-error per-row line, currently the path a restore wrote to. */
const rowNote = ref<Record<string, string>>({});

const deleteTarget = ref<UnifiedRow | null>(null);

/**
 * A PC save's delete is a different promise from an emulator save's: the
 * server only tombstones the caller's own row, so another account's copy of
 * the same shared file survives and can come back. The dialog says which
 * promise it's making.
 */
const deleteIsShared = computed(() => {
  const c = deleteTarget.value?.cloud;
  return !!c && isPcSave(c);
});

// ── Derived: the unified list + summary ───────────────────────────────────────

function cloudMs(c: CloudSaveListEntry): number {
  const t = new Date(c.clientModifiedAt).getTime();
  return Number.isNaN(t) ? 0 : t;
}
function localMs(l: LocalSaveEntry): number {
  return (l.modifiedAt || 0) * 1000;
}

/** "Ada", "Ada and Bob", "Ada, Bob and Cleo". */
function joinNames(names: string[]): string {
  if (names.length <= 1) return names[0] ?? "";
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/**
 * What to say when more than one account has a save with this filename.
 *
 * Only one copy can be shown, and the one shown is whichever was played most
 * recently. That is decided partly by a timestamp the other machine reported,
 * so a copy losing is not proof it is older. Saying nothing would make a
 * second copy of someone's progress invisible with no way to find out it
 * exists.
 */
function shadowNote(cloud: CloudSaveListEntry): string | null {
  const others = cloud.alsoHeldBy ?? [];
  if (cloud.shadowedSaveId) {
    return cloud.ownedBy
      ? `You have your own copy of this save. ${cloud.ownedBy} played more recently, so theirs is the one shown.`
      : "You have your own copy of this save, and a more recent one is shown instead.";
  }
  if (others.length > 0) {
    return others.length > 1
      ? `${joinNames(others)} also have saves with this name.`
      : `${others[0]} also has a save with this name.`;
  }
  return null;
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
    const ownedBy =
      cloud && cloud.ownedBy && isPcSave(cloud) ? cloud.ownedBy : null;
    out.push({
      key,
      name: displayName(key),
      saveType,
      state,
      size,
      whenMs,
      cloud,
      local,
      ownedBy,
      shadowNote: cloud ? shadowNote(cloud) : null,
    });
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

/**
 * Storage line in the header. Account-wide rather than per-game, because the
 * cap is: filling it up on one game is what stops the next one backing up.
 */
const quotaPercent = computed(() => cloudSaveQuotaPercent(quota.value));

const quotaText = computed(() => {
  const line = cloudSaveQuotaLine(quota.value);
  if (!line) return "";
  return quotaPercent.value >= 95
    ? `${line}. Your cloud save space is nearly full.`
    : line;
});

const quotaClass = computed(() => {
  if (quotaPercent.value >= 95) return "text-red-400";
  if (quotaPercent.value >= 80) return "text-amber-400";
  return "text-zinc-500";
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
 * Turn a cloud filename into something readable. The scanner namespaces PC
 * saves so they don't collide with emulator saves (`pc__` is the current,
 * sanitize-safe prefix and `pc/` the legacy one), and it escapes path
 * separators as `%2F` so a save nested in a subfolder keeps a distinct
 * identity through the server's filename sanitizer. Both are undone here for
 * display only — `row.key` stays the wire name every action is keyed on.
 */
function displayName(filename: string): string {
  let name = filename;
  if (name.startsWith("pc__")) name = name.slice(4);
  else if (name.startsWith("pc/")) name = name.slice(3);
  else if (name.startsWith("switch__")) name = name.slice(8);
  return name.replaceAll("%2F", "/").replaceAll("%25", "%");
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
    // Quota rides along with the two calls the panel already makes. Like the
    // local scan it soft-fails: an older server without the endpoint should
    // cost the header a line, not the whole list.
    const [cloud, local, q] = await Promise.all([
      api.saves.list(props.gameId),
      invoke<LocalSaveEntry[]>("scan_local_game_saves", {
        gameId: props.gameId,
        gameName: props.gameName,
      }).catch(() => [] as LocalSaveEntry[]),
      api.saves.quota().catch(() => null),
    ]);
    entries.value = cloud;
    localEntries.value = local;
    quota.value = q;
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

async function readSyncEnabled() {
  try {
    const settings = await invoke<{ cloudSavesEnabled?: boolean }>(
      "fetch_settings",
    );
    syncEnabled.value = settings?.cloudSavesEnabled === true;
  } catch {
    // Unknown, so say nothing rather than accuse the setting of being off.
    syncEnabled.value = null;
  }
}

/**
 * Ask what Drop can find for this game. Separate from `checkLudusavi` because
 * it runs Ludusavi's name resolution against a ~9 MB catalogue, so it is the
 * slow half and must not hold up the install prompt.
 */
async function checkCoverage() {
  coverage.value = null;
  try {
    coverage.value = await invoke<SaveCoverage>("game_save_coverage", {
      gameId: props.gameId,
      gameName: props.gameName,
    });
  } catch {
    // Leave it null. The empty state falls back to neutral wording rather
    // than claiming a cause we failed to determine.
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
    // Only now can Drop answer whether the catalogue knows this game.
    await checkCoverage();
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
    let pushErrors: string[] = [];

    if (toPush.length > 0) {
      const result = await invoke<BackupResult>("backup_saves", {
        gameId: props.gameId,
        gameName: props.gameName,
        filenames: toPush,
      });
      pushed = result.uploaded;
      pushErrors = result.errors;
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

    // A rejected file is a failure the server reported inside a 200, so the
    // call "succeeded" with nothing uploaded. Reporting that as "Everything's
    // already in sync." over rows still reading "Not backed up" is the exact
    // lie this panel used to tell.
    const failed = pushErrors.length + pullFailed;
    syncError.value = failed > 0;
    if (segs.length === 0 && conflicts === 0 && failed === 0) {
      syncMessage.value = "Everything's already in sync.";
    } else {
      const parts: string[] = [];
      if (segs.length > 0) parts.push(`Sync complete: ${segs.join(", ")}.`);
      if (pushErrors.length > 0)
        parts.push(
          `${pushErrors.length} save${pushErrors.length === 1 ? "" : "s"} couldn't be backed up: ${pushErrors[0]}`,
        );
      if (conflicts > 0)
        parts.push(
          `${conflicts} conflict${conflicts === 1 ? "" : "s"} need${conflicts === 1 ? "s" : ""} a choice below.`,
        );
      if (pullFailed > 0)
        parts.push(`${pullFailed} couldn't be restored (game not installed?).`);
      if (parts.length === 0) parts.push("Sync complete.");
      syncMessage.value = parts.join(" ");
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
    const result = await invoke<BackupResult>("backup_saves", {
      gameId: props.gameId,
      gameName: props.gameName,
      filenames: targets,
    });
    // A 200 with an empty `results[]` is still a failure for this row: the
    // server rejected the file and the row is about to redraw as "Not backed
    // up" with nothing saying why.
    if (result.errors.length > 0) {
      for (const k of targets)
        rowError.value[k] = `Back up failed: ${result.errors[0]}`;
    }
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
  delete rowNote.value[row.key];
  try {
    const written = await doRestore(row.cloud);
    if (written) rowNote.value[row.key] = `Restored to ${written}`;
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

/**
 * Download + write a cloud save to its real on-disk location (type-aware).
 *
 * Returns the path a PC save was written to. On a machine that has never run
 * the game that path comes from Ludusavi's catalogue and not from anything on
 * this disk, so it is the one thing the user can check the restore against.
 * Emulator saves go to a location Drop owns, so there is nothing to report.
 */
async function doRestore(entry: CloudSaveListEntry): Promise<string | null> {
  const { data } = await api.saves.download(entry.id);
  if (isPcSave(entry)) {
    return await invoke<string>("restore_pc_cloud_save", {
      gameId: props.gameId,
      filename: entry.filename,
      data,
    });
  }
  await invoke("write_save_file", {
    gameId: props.gameId,
    filename: entry.filename,
    saveType: entry.saveType,
    data,
  });
  return null;
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
    const deleted = await api.saves.delete(row.cloud.id);
    deleteTarget.value = null;
    await refresh();
    if (!deleted) {
      // The row belongs to another account and this user has no copy of their
      // own. A delete only removes your own copy, so nothing changed, and the
      // row is still in the list.
      rowError.value[row.key] =
        "This save belongs to another account, and you do not have a copy of your own to delete.";
    }
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
  checkCoverage();
  readSyncEnabled();
});

watch(
  () => props.gameId,
  () => {
    entries.value = [];
    localEntries.value = [];
    rowBusy.value = {};
    rowError.value = {};
    rowNote.value = {};
    syncMessage.value = null;
    syncError.value = false;
    refresh();
    checkLudusavi();
    checkCoverage();
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
