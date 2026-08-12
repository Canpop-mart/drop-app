<template>
  <section
    class="rounded-xl overflow-hidden"
    style="background-color: var(--bpm-surface)"
  >
    <!-- Header / collapse toggle. -->
    <div
      :ref="(el: any) => registerAction?.(el, { onSelect: () => (expanded = !expanded) })"
      class="bp-focus-delegate w-full flex items-center justify-between px-5 py-4 cursor-pointer transition-colors"
      :style="{ color: 'var(--bpm-text)' }"
      @click="expanded = !expanded"
    >
      <div class="flex items-center gap-3">
        <div
          class="size-9 rounded-lg flex items-center justify-center flex-shrink-0"
          style="background-color: rgba(59, 130, 246, 0.18)"
        >
          <svg
            class="size-5 text-blue-300"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M2.25 15a4.5 4.5 0 004.5 4.5H18a3.75 3.75 0 001.332-7.257 3 3 0 00-3.758-3.848 5.25 5.25 0 00-10.233 2.33A4.502 4.502 0 002.25 15z"
            />
          </svg>
        </div>
        <div>
          <p class="text-base font-semibold font-display">Cloud Saves</p>
          <p class="text-xs" style="color: var(--bpm-muted)">
            <template v-if="loading">Loading…</template>
            <template v-else-if="loadError">{{ loadError }}</template>
            <template v-else-if="entries.length === 0">No cloud saves yet.</template>
            <template v-else>
              {{ entries.length }} save{{ entries.length === 1 ? "" : "s" }}
              on the server
            </template>
          </p>
          <!-- Storage. The cap has always been enforced and never shown, so
               the first anyone heard of it was an upload being rejected. -->
          <p v-if="quotaText" class="text-xs" :style="{ color: quotaColor }">
            {{ quotaText }}
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <button
          type="button"
          :ref="(el: any) => registerAction?.(el, { onSelect: refresh })"
          class="bp-focus-delegate p-2 rounded-lg transition-colors"
          style="background-color: rgba(255, 255, 255, 0.05); color: var(--bpm-muted)"
          aria-label="Refresh"
          @click.stop="refresh"
        >
          <svg
            class="size-4"
            :class="loading ? 'animate-spin' : ''"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99"
            />
          </svg>
        </button>
        <svg
          class="size-5 transition-transform"
          :class="expanded ? 'rotate-180' : ''"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          viewBox="0 0 24 24"
          :style="{ color: 'var(--bpm-muted)' }"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M19.5 8.25l-7.5 7.5-7.5-7.5"
          />
        </svg>
      </div>
    </div>

    <Transition
      enter-active-class="overflow-hidden transition-all duration-200"
      leave-active-class="overflow-hidden transition-all duration-150"
      enter-from-class="max-h-0 opacity-0"
      enter-to-class="max-h-[80rem] opacity-100"
      leave-from-class="max-h-[80rem] opacity-100"
      leave-to-class="max-h-0 opacity-0"
    >
      <div
        v-if="expanded"
        class="border-t"
        style="border-color: var(--bpm-border)"
      >
        <!-- Cloud saves is opt-in and off until turned on. Mirrors the desktop
             panel: the panel has to say when the thing it is a panel for is
             not running. -->
        <div
          v-if="syncEnabled === false"
          class="mx-5 mt-4 rounded-lg px-4 py-3"
          style="
            background-color: rgba(245, 158, 11, 0.08);
            border: 1px solid rgba(245, 158, 11, 0.3);
          "
        >
          <p class="text-sm font-medium" style="color: rgb(253, 230, 138)">
            Cloud saves are turned off
          </p>
          <p
            class="text-xs mt-1 leading-relaxed"
            :style="{ color: 'var(--bpm-muted)' }"
          >
            Nothing on this PC is being backed up. Turn cloud saves on in
            Settings, under Cloud Saves. Anything already on your server is
            still listed here.
          </p>
        </div>

        <!-- Who owns what. Mirrors the desktop panel: PC saves really are
             shared between accounts on this server, so the panel says so. -->
        <p
          class="px-5 pt-4 text-xs leading-relaxed"
          :style="{ color: 'var(--bpm-muted)' }"
        >
          Emulator saves are backed up to your account alone, with one
          exception: Switch games keep their saves inside the emulator's own
          system storage, which every account on this computer shares. PC game
          saves are shared with everyone on this Drop server, because Drop
          finds them by where the game puts them on this computer rather than
          by who is signed in. If two accounts have the same PC save, the one
          played most recently is the one you see here.
        </p>

        <!-- Loading. -->
        <div
          v-if="loading && entries.length === 0"
          class="px-5 py-8 text-center text-sm"
          :style="{ color: 'var(--bpm-muted)' }"
        >
          Loading cloud saves…
        </div>

        <!-- Empty state. "Play and they appear" is only true for games Drop
             can actually locate saves for; for the rest it is a promise that
             never comes good. -->
        <div
          v-else-if="!loading && entries.length === 0 && !loadError"
          class="px-5 py-8 text-center"
        >
          <template v-if="saveLocationUnknown">
            <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
              Drop cannot find where this game stores its saves.
            </p>
            <p
              class="text-xs mt-1.5 max-w-md mx-auto leading-relaxed"
              :style="{ color: 'var(--bpm-muted)' }"
            >
              {{ saveLocationUnknownDetail }}
            </p>
          </template>
          <template v-else-if="syncEnabled === false">
            <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
              Nothing backed up yet.
            </p>
            <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
              Cloud saves are turned off, so Drop is not backing this game up.
            </p>
          </template>
          <template v-else>
            <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
              No cloud saves yet.
            </p>
            <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
              They appear after you play and your saves get backed up.
            </p>
          </template>
        </div>

        <!-- List. -->
        <div v-else class="space-y-2 p-3">
          <div
            v-for="entry in entries"
            :key="entry.id"
            class="flex items-center gap-4 rounded-xl px-4 py-3"
            style="background-color: rgba(255, 255, 255, 0.04)"
          >
            <!-- Icon. -->
            <div
              class="size-10 rounded-lg flex items-center justify-center flex-shrink-0"
              :style="{ backgroundColor: chipColor(entry).bg }"
            >
              <svg
                class="size-5"
                :style="{ color: chipColor(entry).text }"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  d="M2.25 15a4.5 4.5 0 004.5 4.5H18a3.75 3.75 0 001.332-7.257 3 3 0 00-3.758-3.848 5.25 5.25 0 00-10.233 2.33A4.502 4.502 0 002.25 15z"
                />
              </svg>
            </div>

            <!-- Info. -->
            <div class="flex-1 min-w-0">
              <p
                class="text-sm font-medium truncate"
                :style="{ color: 'var(--bpm-text)' }"
              >
                {{ entry.filename }}
              </p>
              <p class="text-xs mt-0.5" :style="{ color: 'var(--bpm-muted)' }">
                {{ entry.saveType }} &middot; {{ formatSize(entry.size) }}
                <template v-if="entry.uploadedFrom">
                  &middot; from {{ entry.uploadedFrom }}
                </template>
                &middot; {{ formatTimeAgo(entry.clientModifiedAt) }}
                <!-- Only on shared rows: emulator saves are always yours. -->
                <template v-if="entry.ownedBy && isPcSave(entry)">
                  &middot; saved by {{ entry.ownedBy }}
                </template>
              </p>
              <!-- Only one copy of a filename can be shown, and which one is
                   decided partly by a timestamp another machine reported. A
                   second copy must never be invisible. -->
              <p
                v-if="shadowNote(entry)"
                class="text-xs mt-1 text-amber-400/80"
              >
                {{ shadowNote(entry) }}
              </p>
              <p
                v-if="rowError[entry.id]"
                class="text-xs mt-1 text-red-400"
              >
                {{ rowError[entry.id] }}
              </p>
            </div>

            <!-- Actions. -->
            <div class="flex items-center gap-2 flex-shrink-0">
              <button
                type="button"
                :ref="(el: any) => registerAction?.(el, { onSelect: () => restore(entry) })"
                class="bp-focus-delegate px-3 py-1.5 text-xs rounded-lg transition-colors bg-blue-900/30 text-blue-300 hover:bg-blue-800/40"
                :disabled="rowBusy[entry.id] !== undefined"
                @click="restore(entry)"
              >
                {{ rowBusy[entry.id] === "restore" ? "Restoring…" : "Restore" }}
              </button>
              <button
                type="button"
                :ref="(el: any) => registerAction?.(el, { onSelect: () => askDelete(entry) })"
                class="bp-focus-delegate px-3 py-1.5 text-xs rounded-lg transition-colors bg-red-900/30 text-red-300 hover:bg-red-800/40"
                :disabled="rowBusy[entry.id] !== undefined"
                @click="askDelete(entry)"
              >
                {{ rowBusy[entry.id] === "delete" ? "Deleting…" : "Delete" }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Delete confirmation dialog. -->
    <BigPictureDialog
      :visible="deleteTarget !== null"
      title="Delete Cloud Save?"
      :message="deleteMessage"
      confirm-label="Delete"
      cancel-label="Cancel"
      :destructive="true"
      @confirm="confirmDelete"
      @cancel="deleteTarget = null"
    />
  </section>
</template>

<script setup lang="ts">
/**
 * Cloud Saves panel for the BPM per-game library page.
 *
 * Mirrors the desktop CloudSavesPanel: a collapsible card with one row per
 * cloud save, plus Restore + Delete actions. List/Delete are pure HTTP via
 * `useServerApi().saves`; Restore for emulator saves uses the existing
 * `write_save_file` Tauri command. PC-game saves (filename prefix `pc:`)
 * keep a disabled Restore — the per-launch sync handles those.
 *
 * Interactive rows are registered with the page-supplied focus-nav group
 * so the controller can navigate them.
 */
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
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";

const props = withDefaults(
  defineProps<{
    gameId: string;
    /**
     * Display name, used to ask Ludusavi whether it knows this game. Without
     * it the panel can still list saves, it just can't explain an empty list.
     */
    gameName?: string;
    /**
     * Optional — pass the page's focus-nav registrar (e.g. the "content"
     * group from `useBpFocusableGroup("content")`) so controller D-pad
     * navigation reaches the rows in this panel.
     */
    registerAction?: (
      el: any,
      opts: { onSelect: () => void; onContext?: () => void },
    ) => void;
  }>(),
  { gameName: "", registerAction: undefined },
);

const api = useServerApi();

/**
 * What Drop is able to find for this game, from `game_save_coverage`. Mirrors
 * the desktop panel: an empty list has more than one cause, and only one of
 * them is fixed by playing the game.
 */
interface SaveCoverage {
  ludusaviInstalled: boolean;
  knownToLudusavi: boolean;
  canonicalTitle: string | null;
  emulated: boolean;
  emulatorSupported: boolean;
}
const coverage = ref<SaveCoverage | null>(null);

/** The master cloud-saves switch. Null until read. */
const syncEnabled = ref<boolean | null>(null);

async function readSyncEnabled() {
  try {
    const settings = await invoke<{ cloudSavesEnabled?: boolean }>(
      "fetch_settings",
    );
    syncEnabled.value = settings?.cloudSavesEnabled === true;
  } catch {
    syncEnabled.value = null;
  }
}

const saveLocationUnknown = computed(() => {
  const c = coverage.value;
  if (!c) return false;
  if (c.emulated) return !c.emulatorSupported;
  return c.ludusaviInstalled && !c.knownToLudusavi;
});

const saveLocationUnknownDetail = computed(() => {
  if (coverage.value?.emulated) {
    return "Drop can read saves from RetroArch and from Switch emulators. This game runs on a different emulator, which keeps its saves somewhere Drop does not know about.";
  }
  return "Drop uses Ludusavi's list of games to know where PC saves live, and this game is not on it. Playing it will not change that.";
});

async function checkCoverage() {
  coverage.value = null;
  try {
    coverage.value = await invoke<SaveCoverage>("game_save_coverage", {
      gameId: props.gameId,
      gameName: props.gameName,
    });
  } catch {
    // Leave it null — the empty state stays on its neutral wording.
  }
}

const expanded = ref(true);
const loading = ref(false);
const loadError = ref<string | null>(null);
const entries = ref<CloudSaveListEntry[]>([]);

const quota = ref<CloudSaveQuota | null>(null);

const quotaPercent = computed(() => cloudSaveQuotaPercent(quota.value));

const quotaText = computed(() => {
  const line = cloudSaveQuotaLine(quota.value);
  if (!line) return "";
  return quotaPercent.value >= 95
    ? `${line}. Your cloud save space is nearly full.`
    : line;
});

const quotaColor = computed(() => {
  if (quotaPercent.value >= 95) return "rgb(248, 113, 113)";
  if (quotaPercent.value >= 80) return "rgb(251, 191, 36)";
  return "var(--bpm-muted)";
});

const rowBusy = ref<Record<string, "restore" | "delete">>({});
const rowError = ref<Record<string, string>>({});

const deleteTarget = ref<CloudSaveListEntry | null>(null);

async function refresh() {
  loading.value = true;
  loadError.value = null;
  try {
    // Quota soft-fails so an older server without the endpoint costs the
    // header a line rather than the whole list.
    const [cloud, q] = await Promise.all([
      api.saves.list(props.gameId),
      api.saves.quota().catch(() => null),
    ]);
    entries.value = cloud;
    quota.value = q;
  } catch (e) {
    loadError.value =
      e instanceof Error
        ? `Couldn't load cloud saves: ${e.message}`
        : `Couldn't load cloud saves: ${String(e)}`;
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  refresh();
  checkCoverage();
  readSyncEnabled();
});

watch(
  () => props.gameId,
  () => {
    entries.value = [];
    rowBusy.value = {};
    rowError.value = {};
    refresh();
    checkCoverage();
  },
);

// PC saves are namespaced so they don't collide with emulator saves. `pc__`
// is the current sanitize-safe prefix (see remote/src/save_sync/scan.rs);
// `pc/` is the legacy one. Recognising both routes PC entries to the
// Ludusavi-aware restore path instead of the emu-restore path.
function isPcSave(entry: CloudSaveListEntry): boolean {
  return (
    entry.saveType === "pc" ||
    entry.filename.startsWith("pc__") ||
    entry.filename.startsWith("pc/")
  );
}

async function restore(entry: CloudSaveListEntry) {
  if (rowBusy.value[entry.id]) return;
  rowBusy.value[entry.id] = "restore";
  delete rowError.value[entry.id];
  try {
    const { data } = await api.saves.download(entry.id);
    if (isPcSave(entry)) {
      // PC saves: re-scan with Ludusavi to find the destination path. The
      // command surfaces a friendly error if the game's saves haven't been
      // populated on this device yet (cold-restore edge case).
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
  } catch (e) {
    rowError.value[entry.id] =
      e instanceof Error
        ? `Restore failed: ${e.message}`
        : `Restore failed: ${String(e)}`;
  } finally {
    delete rowBusy.value[entry.id];
  }
}

/**
 * A PC save's delete is a different promise from an emulator save's: the
 * server only tombstones the caller's own row, so another account's copy of
 * the same shared file survives and can come back.
 */
const deleteMessage = computed(() => {
  const entry = deleteTarget.value;
  if (!entry) return "";
  if (isPcSave(entry)) {
    return `Delete the cloud copy of '${entry.filename}'? This removes your copy only. PC game saves are shared with everyone on this Drop server, so if another account still has this save it can come back the next time you sync. The copy on this device stays where it is.`;
  }
  return `Permanently delete the cloud copy of '${entry.filename}'? The copy on this device stays where it is, but your other devices will remove their copy the next time you play there.`;
});

/** "Ada", "Ada and Bob", "Ada, Bob and Cleo". */
function joinNames(names: string[]): string {
  if (names.length <= 1) return names[0] ?? "";
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/**
 * What to say when more than one account has a save with this filename. The
 * copy shown is whichever was played most recently, decided partly by a
 * timestamp the other machine reported, so a copy losing is not proof it is
 * older. Silence would make a second copy of someone's progress invisible.
 */
function shadowNote(entry: CloudSaveListEntry): string | null {
  const others = entry.alsoHeldBy ?? [];
  if (entry.shadowedSaveId) {
    return entry.ownedBy
      ? `You have your own copy of this save. ${entry.ownedBy} played more recently, so theirs is the one shown.`
      : "You have your own copy of this save, and a more recent one is shown instead.";
  }
  if (others.length > 0) {
    return others.length > 1
      ? `${joinNames(others)} also have saves with this name.`
      : `${others[0]} also has a save with this name.`;
  }
  return null;
}

function askDelete(entry: CloudSaveListEntry) {
  deleteTarget.value = entry;
}

async function confirmDelete() {
  const entry = deleteTarget.value;
  if (!entry) return;
  if (rowBusy.value[entry.id]) return;
  rowBusy.value[entry.id] = "delete";
  delete rowError.value[entry.id];
  try {
    const deleted = await api.saves.delete(entry.id);
    deleteTarget.value = null;
    if (deleted) {
      entries.value = entries.value.filter((e) => e.id !== entry.id);
    } else {
      // The row belongs to another account and this user has no copy of their
      // own. A delete only removes your own copy, so the row is still there.
      rowError.value[entry.id] =
        "This save belongs to another account, and you do not have a copy of your own to delete.";
    }
  } catch (e) {
    rowError.value[entry.id] =
      e instanceof Error
        ? `Delete failed: ${e.message}`
        : `Delete failed: ${String(e)}`;
  } finally {
    delete rowBusy.value[entry.id];
  }
}

// ── Formatters ────────────────────────────────────────────────────────────

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatTimeAgo(dateStr: string): string {
  const diff = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
  if (Number.isNaN(diff)) return "—";
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  if (diff < 2592000) return `${Math.floor(diff / 604800)}w ago`;
  return `${Math.floor(diff / 2592000)}mo ago`;
}

function chipColor(entry: CloudSaveListEntry): { bg: string; text: string } {
  const t = (entry.saveType ?? "").toLowerCase();
  const fn = (entry.filename ?? "").toLowerCase();
  if (t === "state" || fn.endsWith(".state"))
    return { bg: "rgba(168,85,247,0.18)", text: "#c4a4f5" };
  if (fn.endsWith(".state.png") || fn.endsWith(".png"))
    return { bg: "rgba(59,130,246,0.18)", text: "#93c5fd" };
  if (t === "save" || fn.endsWith(".srm") || fn.endsWith(".sav"))
    return { bg: "rgba(34,197,94,0.18)", text: "#86efac" };
  return { bg: "rgba(156,163,175,0.18)", text: "#d4d4d8" };
}
</script>
