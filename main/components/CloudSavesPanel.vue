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
            Nothing on this device is being backed up. Turn cloud saves on in
            Settings, under Cloud Saves. Anything already on your server is
            still listed here.
          </p>
        </div>

        <!-- Who owns what. Cloud copies are per account, but the files on
             this computer are not, and that is what a conflict after
             switching accounts comes from. -->
        <p class="px-6 pt-4 text-xs leading-relaxed text-zinc-500">
          The cloud copies here are your account's only. PC and Switch saves
          on this device are the same files for every Drop account that plays
          here, so after switching accounts Drop may ask which copy to keep.
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
          class="mx-6 mt-4 flex items-start justify-between gap-3 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300"
        >
          <span>{{ loadError }}</span>
          <button
            type="button"
            class="shrink-0 rounded px-2 py-0.5 text-xs font-medium bg-red-500/20 hover:bg-red-500/30"
            :disabled="loading"
            @click="refresh"
          >
            Retry
          </button>
        </div>

        <!-- The local scan failed. Not the same as "no saves on this PC":
             every cloud row would otherwise read as missing here, and Sync
             would pull them all down over files the scan could not see. -->
        <div
          v-if="localError"
          class="mx-6 mt-4 flex items-start justify-between gap-3 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-sm text-amber-200"
        >
          <span>
            Drop could not check the saves on this device, so it cannot tell
            what is backed up. {{ localError }}
          </span>
          <button
            type="button"
            class="shrink-0 rounded px-2 py-0.5 text-xs font-medium bg-amber-500/20 hover:bg-amber-500/30"
            :disabled="loading"
            @click="refresh"
          >
            Retry
          </button>
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
          v-else-if="!loading && rows.length === 0 && !loadError && !localError"
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
              </div>
              <!-- Two accounts on one PC share its save files. Say so when the
                   copy here is the other person's. -->
              <p
                v-if="
                  row.local &&
                  row.local.lastSyncedByOtherAccount !== null &&
                  row.local.lastSyncedByOtherAccount !== undefined
                "
                class="mt-1 text-xs text-amber-400/80"
              >
                {{
                  row.local.lastSyncedByOtherAccount
                    ? `The copy on this device was last synced by ${row.local.lastSyncedByOtherAccount}.`
                    : "The copy on this device was last synced by another Drop account."
                }}
              </p>
              <p v-if="row.legacy" class="mt-1 text-xs text-zinc-500">
                {{ legacyNote(row.legacy) }}
              </p>
              <p
                v-if="heldOnSharedDevice(row)"
                class="mt-1 text-xs text-amber-400/80"
              >
                Not backed up automatically, because another Drop account also
                plays this game here. Back it up if it is yours.
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

              <!-- Previous versions the server kept of this save. Restoring
                   one changes the cloud copy only; the row then offers its
                   Restore button like any other newer cloud copy. -->
              <div
                v-if="row.cloud && historyOpen[row.key]"
                class="mt-2 rounded-md border border-zinc-700/60 bg-zinc-900/40 px-3 py-2"
              >
                <p
                  v-if="historyFor(row)?.loading && !historyFor(row)?.data"
                  class="text-xs text-zinc-500"
                >
                  Loading earlier versions…
                </p>
                <div
                  v-else-if="historyFor(row)?.error"
                  class="flex items-center justify-between gap-2 text-xs text-red-400"
                >
                  <span>{{ historyFor(row)?.error }}</span>
                  <button
                    type="button"
                    class="rounded px-2 py-0.5 bg-zinc-700 text-zinc-200 hover:bg-zinc-600"
                    @click="loadHistory(row)"
                  >
                    Retry
                  </button>
                </div>
                <p
                  v-else-if="(historyFor(row)?.data?.revisions.length ?? 0) === 0"
                  class="text-xs text-zinc-500"
                >
                  No earlier versions are kept for this save.
                </p>
                <ul v-else class="space-y-1.5">
                  <li
                    v-for="rev in historyFor(row)?.data?.revisions ?? []"
                    :key="rev.id"
                    class="flex items-center justify-between gap-3 text-xs"
                  >
                    <span class="text-zinc-400">
                      {{ exact(Date.parse(rev.clientModifiedAt)) }} ·
                      {{ formatSize(rev.size) }}
                      <template v-if="rev.uploadedFrom">
                        · from {{ rev.uploadedFrom }}</template
                      >
                    </span>
                    <button
                      type="button"
                      class="shrink-0 rounded px-2 py-1 font-medium bg-zinc-700 text-zinc-200 hover:bg-blue-600 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                      :disabled="isRowBusy(row)"
                      @click="restoreTarget = { row, revision: rev }"
                    >
                      Restore this version
                    </button>
                  </li>
                </ul>
              </div>
            </div>

            <!-- State-appropriate actions. -->
            <div class="flex items-center gap-1.5 shrink-0">
              <!-- Not backed up, or only changed here → push. -->
              <button
                v-if="row.state === 'localOnly' || row.state === 'localNewer'"
                type="button"
                class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-cyan-600/80 text-white hover:bg-cyan-500 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                :disabled="isRowBusy(row)"
                @click="backupRows([row.key])"
              >
                <ArrowUpTrayIcon class="size-3.5" />
                {{ rowBusy[row.key] === "backup" ? "Backing up…" : "Back up" }}
              </button>

              <!-- In cloud only, or only changed in the cloud → pull. Also
                   offered when this PC could not be checked: pulling is an
                   explicit choice there, never something Sync does. -->
              <button
                v-if="
                  (row.state === 'cloudOnly' ||
                    row.state === 'cloudNewer' ||
                    row.state === 'unchecked') &&
                  !(row.legacy && !row.legacy.localFilename)
                "
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
                  title="Overwrite the cloud copy with this device's version"
                  @click="backupRows([row.key])"
                >
                  <ArrowUpTrayIcon class="size-3.5" />
                  Keep&nbsp;PC
                </button>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs font-medium bg-zinc-700 text-zinc-200 hover:bg-blue-600 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                  :disabled="isRowBusy(row)"
                  title="Overwrite this device's copy with the cloud version"
                  @click="restoreRow(row)"
                >
                  <ArrowDownTrayIcon class="size-3.5" />
                  Keep&nbsp;cloud
                </button>
              </template>

              <!-- Earlier versions of the cloud copy. -->
              <button
                v-if="row.cloud"
                type="button"
                class="rounded-md px-2 py-1.5 text-xs text-zinc-400 hover:text-white hover:bg-zinc-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                :disabled="isRowBusy(row)"
                title="Earlier versions of the cloud copy"
                @click="toggleHistory(row)"
              >
                History
              </button>

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
              >? This cannot be undone. The copy on this device stays where
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

    <!-- Restore-a-version confirmation. -->
    <Transition
      enter-active-class="ease-out duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="restoreTarget"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
        @click.self="restoreTarget = null"
      >
        <div
          class="w-full max-w-sm rounded-xl bg-zinc-900 border border-zinc-700 shadow-2xl"
        >
          <div class="px-6 py-5">
            <h3 class="text-base font-semibold font-display text-zinc-100">
              Restore this version?
            </h3>
            <p class="mt-2 text-sm text-zinc-400">
              The cloud copy of
              <span class="text-zinc-200 font-medium">{{
                restoreTarget.row.name
              }}</span>
              goes back to the version from
              {{ exact(Date.parse(restoreTarget.revision.clientModifiedAt)) }}.
              The current cloud copy is kept in its history. The next time the
              game starts on any device, this one included, a copy that has not
              changed since that device last synced is replaced with this
              version, and a changed copy makes Drop ask which to keep.
            </p>
          </div>
          <div class="flex justify-end gap-3 border-t border-zinc-700 px-6 py-4">
            <button
              type="button"
              class="rounded-md px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-800 transition-colors"
              @click="restoreTarget = null"
            >
              Cancel
            </button>
            <button
              type="button"
              class="rounded-md px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-500 disabled:opacity-50 transition-colors"
              :disabled="rowBusy[restoreTarget.row.key] === 'revision'"
              @click="confirmRestoreRevision"
            >
              {{
                rowBusy[restoreTarget.row.key] === "revision"
                  ? "Restoring…"
                  : "Restore"
              }}
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
 * and tagged with a sync state (`saveSyncState`, the same three-way rule the
 * launch sync uses):
 *
 *   - Synced          — local copy and cloud copy match (same hash).
 *   - Not backed up   — exists on this PC, not in the cloud yet.
 *   - In cloud only   — in the cloud, not on this PC (fresh/other device).
 *   - Newer in cloud  — only the cloud changed since the last sync.
 *   - Newer here      — only this PC changed since the last sync.
 *   - Conflict        — both changed (or no record); the user picks a side.
 *   - Unchecked       — the local scan failed, so only the cloud is known.
 *
 * The header's single **Sync** button reconciles both directions — it backs
 * up what is only or newer here and pulls down what is only or newer in the
 * cloud — but deliberately leaves conflicts for an explicit per-row choice so
 * it can never silently clobber a copy, and refuses to run at all when the
 * local scan failed. Restore is type-aware: emulator saves write to
 * `{install}/drop-saves/{userId}/{gameId}/…`; PC saves re-scan with Ludusavi
 * via `restore_pc_cloud_save` so they land where the game actually reads them.
 *
 * History lists the earlier versions the server keeps (up to three) and can
 * make one the live cloud copy again.
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
import {
  displaySaveName,
  heldOnSharedDeviceState,
  legacyRowNote,
  matchLegacyRow,
  saveSyncState,
  type LegacyRow,
  type SaveSyncState,
} from "~/composables/save-sync-state";
import type {
  CloudSaveHistory,
  CloudSaveRevision,
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

/** "unchecked": the local scan failed, so only the cloud side is known. */
type SyncState = SaveSyncState | "unchecked";

/** A save file detected on disk (from `scan_local_game_saves`). */
interface LocalSaveEntry {
  filename: string;
  saveType: string;
  size: number;
  modifiedAt: number; // unix seconds
  dataHash: string;
  /** What this account's sync manifest recorded at the last sync. */
  syncedHash?: string | null;
  syncedCloudId?: string | null;
  /** Another Drop account on this device last synced these exact bytes. */
  lastSyncedByOtherAccount?: string | null;
  /** Another Drop account also syncs this game on this device. */
  otherAccountsOnThisDevice?: boolean;
  /** The name an older build's upload of this file is stored under. */
  legacyCloudName?: string | null;
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
   * For a cloud-only row that is an older build's copy of a local file (its
   * name was changed by the server's sanitizer back then): which file, and
   * whether the bytes match. Sync leaves such a row alone, and Restore writes
   * it into that file instead of a stray second file the game never reads.
   */
  legacy: LegacyRow | null;
}

const expanded = ref(true);
const loading = ref(false);
const loadError = ref<string | null>(null);
const entries = ref<CloudSaveListEntry[]>([]);
const localEntries = ref<LocalSaveEntry[]>([]);
/**
 * Why the local scan failed, or null. Kept apart from `loadError` (the cloud
 * list) because the cloud list is still worth showing, and apart from an empty
 * `localEntries` because "could not look" is not "nothing there".
 */
const localError = ref<string | null>(null);
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
const rowBusy = ref<
  Record<string, "backup" | "restore" | "delete" | "revision">
>({});
const rowError = ref<Record<string, string>>({});
/** Non-error per-row line, currently the path a restore wrote to. */
const rowNote = ref<Record<string, string>>({});

const deleteTarget = ref<UnifiedRow | null>(null);

// ── Version history ─────────────────────────────────────────────────────────

/** Which rows have their history expanded, keyed by row.key. */
const historyOpen = ref<Record<string, boolean>>({});
interface HistoryState {
  loading: boolean;
  error: string | null;
  data: CloudSaveHistory | null;
}
/** History per cloud row id, so a refresh that re-keys nothing keeps it. */
const histories = ref<Record<string, HistoryState>>({});
/** The version a confirm dialog is asking about. */
const restoreTarget = ref<{
  row: UnifiedRow;
  revision: CloudSaveRevision;
} | null>(null);

function historyFor(row: UnifiedRow): HistoryState | null {
  return row.cloud ? (histories.value[row.cloud.id] ?? null) : null;
}

async function loadHistory(row: UnifiedRow) {
  const id = row.cloud?.id;
  if (!id) return;
  // Keep what is already shown while it reloads.
  histories.value[id] = {
    loading: true,
    error: null,
    data: histories.value[id]?.data ?? null,
  };
  try {
    const data = await api.saves.history(id);
    histories.value[id] = { loading: false, error: null, data };
  } catch (e) {
    histories.value[id] = {
      loading: false,
      error: `Couldn't load earlier versions: ${e instanceof Error ? e.message : String(e)}`,
      data: null,
    };
  }
}

function toggleHistory(row: UnifiedRow) {
  const open = !historyOpen.value[row.key];
  historyOpen.value[row.key] = open;
  if (open) loadHistory(row);
}

async function confirmRestoreRevision() {
  const target = restoreTarget.value;
  if (!target || rowBusy.value[target.row.key] !== undefined) return;
  const { row, revision } = target;
  rowBusy.value[row.key] = "revision";
  delete rowError.value[row.key];
  delete rowNote.value[row.key];
  try {
    const result = await api.saves.restoreRevision(revision.id);
    restoreTarget.value = null;
    await refresh();
    const fresh = rows.value.find((r) => r.key === row.key);
    // Name the button the row actually shows now.
    const takeCloud =
      fresh?.state === "conflict" ? "Keep cloud" : "Restore";
    rowNote.value[row.key] = !result.restored
      ? "That version is already the cloud copy."
      : fresh?.state === "synced"
        ? "The cloud copy is now that earlier version, which matches this device."
        : `The cloud copy is now that earlier version. Press ${takeCloud} to put it on this device now.`;
    if (fresh && historyOpen.value[row.key]) await loadHistory(fresh);
  } catch (e) {
    restoreTarget.value = null;
    rowError.value[row.key] =
      e instanceof Error
        ? `Restore failed: ${e.message}`
        : `Restore failed: ${String(e)}`;
  } finally {
    delete rowBusy.value[row.key];
  }
}

// ── Derived: the unified list + summary ───────────────────────────────────────

function cloudMs(c: CloudSaveListEntry): number {
  const t = new Date(c.clientModifiedAt).getTime();
  return Number.isNaN(t) ? 0 : t;
}
function localMs(l: LocalSaveEntry): number {
  return (l.modifiedAt || 0) * 1000;
}

/** The note under an older build's cloud row. */
function legacyNote(legacy: LegacyRow): string {
  return legacyRowNote(
    legacy,
    legacy.localFilename ? displaySaveName(legacy.localFilename) : null,
  );
}

/**
 * A save only on this device that Sync leaves alone because another Drop
 * account also plays this game here (same rule as the launch sync).
 */
function heldOnSharedDevice(row: UnifiedRow): boolean {
  return heldOnSharedDeviceState(row.state, row.local);
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
      state = saveSyncState(local, {
        id: cloud.id,
        dataHash: cloud.dataHash ?? "",
      });
      size = local.size || cloud.size;
      whenMs = Math.max(cloudMs(cloud), localMs(local));
      saveType = cloud.saveType || local.saveType;
    } else if (cloud) {
      // With the local scan failed, "not on this PC" is unknown, not true.
      state = localError.value ? "unchecked" : "cloudOnly";
      size = cloud.size;
      whenMs = cloudMs(cloud);
      saveType = cloud.saveType;
    } else {
      state = "localOnly";
      size = local!.size;
      whenMs = localMs(local!);
      saveType = local!.saveType;
    }
    out.push({
      key,
      name: displaySaveName(key),
      saveType,
      state,
      size,
      whenMs,
      cloud,
      local,
      legacy:
        cloud && !local ? matchLegacyRow(localEntries.value, cloud) : null,
    });
  }

  // Float the rows that need attention to the top, alphabetic within a state.
  const order: Record<SyncState, number> = {
    conflict: 0,
    localOnly: 1,
    localNewer: 2,
    cloudNewer: 3,
    cloudOnly: 4,
    unchecked: 5,
    synced: 6,
  };
  out.sort(
    (a, b) => order[a.state] - order[b.state] || a.name.localeCompare(b.name),
  );
  return out;
});

const counts = computed(() => {
  const c: Record<SyncState, number> = {
    synced: 0,
    localOnly: 0,
    cloudOnly: 0,
    cloudNewer: 0,
    localNewer: 0,
    conflict: 0,
    unchecked: 0,
  };
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
  if (c.localNewer) segs.push(`${c.localNewer} newer here`);
  if (c.cloudNewer) segs.push(`${c.cloudNewer} newer in cloud`);
  if (c.cloudOnly) segs.push(`${c.cloudOnly} in cloud only`);
  if (c.unchecked) segs.push(`${c.unchecked} not checked on this device`);
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
    case "cloudNewer":
      return {
        label: "Newer in cloud",
        icon: CloudArrowDownIcon,
        iconClass: "text-sky-400",
        chipBg: "bg-sky-500/10",
        labelClass: "text-sky-400",
      };
    case "localNewer":
      return {
        label: "Newer on this device",
        icon: CloudArrowUpIcon,
        iconClass: "text-amber-400",
        chipBg: "bg-amber-500/10",
        labelClass: "text-amber-400",
      };
    case "unchecked":
      return {
        label: "In cloud (this device not checked)",
        icon: CloudIcon,
        iconClass: "text-zinc-400",
        chipBg: "bg-zinc-500/10",
        labelClass: "text-zinc-400",
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
    // Cloud list + local disk scan in parallel. A failed local scan must not
    // blank the cloud list, but it must not read as "no saves here" either:
    // it gets its own error state, and Sync refuses to run while it stands.
    // Quota soft-fails: an older server without the endpoint should cost the
    // header a line, not the whole list.
    let scanError: string | null = null;
    const [cloud, local, q] = await Promise.all([
      api.saves.list(props.gameId),
      invoke<LocalSaveEntry[]>("scan_local_game_saves", {
        gameId: props.gameId,
        gameName: props.gameName,
      }).catch((e) => {
        scanError = e instanceof Error ? e.message : String(e);
        return [] as LocalSaveEntry[];
      }),
      api.saves.quota().catch(() => null),
    ]);
    entries.value = cloud;
    localEntries.value = local;
    localError.value = scanError;
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
    if (localError.value) {
      // Pulling "cloud only" saves when the scan simply could not see the
      // local ones would write over them.
      syncError.value = true;
      syncMessage.value =
        "Sync did not run, because Drop could not check the saves on this device. Retry above.";
      return;
    }
    if (loadError.value) {
      syncError.value = true;
      syncMessage.value =
        "Sync did not run, because the cloud saves could not be loaded. Retry above.";
      return;
    }
    // A local file whose older-build cloud copy holds different bytes is a
    // choice for the user, as at launch: neither side moves on Sync.
    const legacyDisputed = new Set(
      rows.value
        .filter((r) => r.legacy && !r.legacy.same && r.legacy.localFilename)
        .map((r) => r.legacy!.localFilename as string),
    );
    const held = rows.value.filter(heldOnSharedDevice);
    const toPush = rows.value
      .filter(
        (r) =>
          (r.state === "localOnly" || r.state === "localNewer") &&
          !heldOnSharedDevice(r) &&
          !legacyDisputed.has(r.key),
      )
      .map((r) => r.key);
    const toPull = rows.value
      .filter(
        (r) =>
          (r.state === "cloudOnly" || r.state === "cloudNewer") &&
          r.cloud &&
          !r.legacy,
      )
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
    if (
      segs.length === 0 &&
      conflicts === 0 &&
      failed === 0 &&
      held.length === 0
    ) {
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
      if (held.length > 0)
        parts.push(
          `${held.length} save${held.length === 1 ? " was" : "s were"} not backed up, because another Drop account also plays this game here. Use Back up on the ones that are yours.`,
        );
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
    const written = await doRestore(
      row.cloud,
      row.legacy?.localFilename ?? undefined,
    );
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
async function doRestore(
  entry: CloudSaveListEntry,
  intoFilename?: string,
): Promise<string | null> {
  const { data } = await api.saves.download(entry.id);
  // An older build's row is written into the local file it is a copy of
  // (`intoFilename`), never next to it under the old name.
  const filename = intoFilename ?? entry.filename;
  if (isPcSave(entry)) {
    return await invoke<string>("restore_pc_cloud_save", {
      gameId: props.gameId,
      filename,
      data,
    });
  }
  await invoke("write_save_file", {
    gameId: props.gameId,
    filename,
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
      // Only an older server that shared PC saves across accounts answers
      // this: the row was another account's and nothing of yours was there.
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
    localError.value = null;
    rowBusy.value = {};
    rowError.value = {};
    rowNote.value = {};
    historyOpen.value = {};
    histories.value = {};
    restoreTarget.value = null;
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
