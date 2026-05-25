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
        <!-- Loading. -->
        <div
          v-if="loading && entries.length === 0"
          class="px-5 py-8 text-center text-sm"
          :style="{ color: 'var(--bpm-muted)' }"
        >
          Loading cloud saves…
        </div>

        <!-- Empty state. -->
        <div
          v-else-if="!loading && entries.length === 0 && !loadError"
          class="px-5 py-8 text-center"
        >
          <p class="text-sm" :style="{ color: 'var(--bpm-text)' }">
            No cloud saves yet.
          </p>
          <p class="text-xs mt-1" :style="{ color: 'var(--bpm-muted)' }">
            They appear after you play and your saves get backed up.
          </p>
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
      :message="deleteTarget
        ? `Permanently delete the cloud copy of '${deleteTarget.filename}'? Your local save file isn't touched.`
        : ''"
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
  useServerApi,
  type CloudSaveListEntry,
} from "~/composables/use-server-api";
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";

const props = defineProps<{
  gameId: string;
  /**
   * Optional — pass the page's focus-nav registrar (e.g. the "content"
   * group from `useBpFocusableGroup("content")`) so controller D-pad
   * navigation reaches the rows in this panel.
   */
  registerAction?: (
    el: any,
    opts: { onSelect: () => void; onContext?: () => void },
  ) => void;
}>();

const api = useServerApi();

const expanded = ref(true);
const loading = ref(false);
const loadError = ref<string | null>(null);
const entries = ref<CloudSaveListEntry[]>([]);

const rowBusy = ref<Record<string, "restore" | "delete">>({});
const rowError = ref<Record<string, string>>({});

const deleteTarget = ref<CloudSaveListEntry | null>(null);

async function refresh() {
  loading.value = true;
  loadError.value = null;
  try {
    entries.value = await api.saves.list(props.gameId);
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
});

watch(
  () => props.gameId,
  () => {
    entries.value = [];
    rowBusy.value = {};
    rowError.value = {};
    refresh();
  },
);

// Server-side scanner produces filenames prefixed with `pc/` (see
// remote/src/save_sync/scan.rs). The earlier `pc:` check here was a typo —
// it never matched, so PC entries always took the emu-restore path and
// silently wrote to the wrong directory.
function isPcSave(entry: CloudSaveListEntry): boolean {
  return entry.saveType === "pc" || entry.filename.startsWith("pc/");
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
    await api.saves.delete(entry.id);
    entries.value = entries.value.filter((e) => e.id !== entry.id);
    deleteTarget.value = null;
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
