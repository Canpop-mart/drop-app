<template>
  <section class="bg-zinc-800/50 rounded-xl backdrop-blur-sm overflow-hidden">
    <!-- Header (always visible — also the collapse toggle). -->
    <button
      type="button"
      class="w-full flex items-center justify-between px-6 py-4 text-left transition-colors hover:bg-zinc-700/30"
      @click="expanded = !expanded"
    >
      <div class="flex items-center gap-3">
        <CloudIcon class="size-5 text-cyan-400" />
        <h3 class="text-base font-semibold text-zinc-100">Cloud Saves</h3>
        <span
          v-if="!loading && entries.length > 0"
          class="inline-flex items-center justify-center min-w-[1.5rem] h-5 px-1.5 rounded-full bg-zinc-700 text-xs font-medium text-zinc-300"
        >
          {{ entries.length }}
        </span>
      </div>
      <div class="flex items-center gap-2">
        <button
          type="button"
          class="rounded-md p-1.5 text-zinc-400 hover:text-zinc-100 hover:bg-zinc-700/60 transition-colors disabled:opacity-40"
          :disabled="loading"
          aria-label="Refresh"
          @click.stop="refresh"
        >
          <ArrowPathIcon
            class="size-4"
            :class="loading ? 'animate-spin' : ''"
          />
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
        <!-- Error banner. -->
        <div
          v-if="loadError"
          class="mx-6 mt-4 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-300"
        >
          {{ loadError }}
        </div>

        <!-- Loading. -->
        <div
          v-if="loading && entries.length === 0"
          class="px-6 py-10 text-center text-sm text-zinc-500"
        >
          Loading cloud saves…
        </div>

        <!-- Empty state. -->
        <div
          v-else-if="!loading && entries.length === 0 && !loadError"
          class="px-6 py-10 text-center"
        >
          <CloudIcon class="mx-auto size-10 text-zinc-600 mb-3" />
          <p class="text-sm text-zinc-400">No cloud saves yet.</p>
          <p class="text-xs text-zinc-500 mt-1">
            They appear after the game runs and changes its save files.
          </p>
        </div>

        <!-- List. -->
        <ul v-else class="divide-y divide-zinc-700/40">
          <li
            v-for="entry in entries"
            :key="entry.id"
            class="flex items-start gap-4 px-6 py-4"
          >
            <!-- Save-type icon. -->
            <div
              class="size-9 rounded-lg flex items-center justify-center flex-shrink-0 mt-0.5"
              :style="{ backgroundColor: chipColor(entry).bg }"
            >
              <CloudIcon
                class="size-5"
                :style="{ color: chipColor(entry).text }"
              />
            </div>

            <!-- Metadata. -->
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span
                  class="text-sm font-medium text-zinc-100 truncate"
                  :title="entry.filename"
                >
                  {{ entry.filename }}
                </span>
                <span
                  class="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded-full font-medium"
                  :style="{
                    backgroundColor: chipColor(entry).bg,
                    color: chipColor(entry).text,
                  }"
                >
                  {{ entry.saveType }}
                </span>
              </div>
              <div class="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-zinc-500">
                <span>{{ formatSize(entry.size) }}</span>
                <span v-if="entry.uploadedFrom">
                  from {{ entry.uploadedFrom }}
                </span>
                <span :title="formatExact(entry.clientModifiedAt)">
                  {{ formatTimeAgo(entry.clientModifiedAt) }}
                </span>
              </div>
              <p
                v-if="rowError[entry.id]"
                class="mt-1.5 text-xs text-red-400"
              >
                {{ rowError[entry.id] }}
              </p>
            </div>

            <!-- Actions. -->
            <div class="flex items-center gap-2 flex-shrink-0">
              <button
                type="button"
                class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-blue-600/80 text-white hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                :disabled="rowBusy[entry.id] !== undefined"
                @click="restore(entry)"
              >
                <ArrowDownTrayIcon class="size-3.5" />
                {{ rowBusy[entry.id] === "restore" ? "Restoring…" : "Restore" }}
              </button>
              <button
                type="button"
                class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-zinc-700 text-zinc-200 hover:bg-red-600 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                :disabled="rowBusy[entry.id] !== undefined"
                @click="askDelete(entry)"
              >
                <TrashIcon class="size-3.5" />
                {{ rowBusy[entry.id] === "delete" ? "Deleting…" : "Delete" }}
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
                deleteTarget.filename
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
              :disabled="rowBusy[deleteTarget.id] === 'delete'"
              @click="confirmDelete"
            >
              {{ rowBusy[deleteTarget.id] === "delete" ? "Deleting…" : "Delete" }}
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
 * Lists the server-side cloud saves for the current user + game, with
 * per-row Restore and Delete actions. All three actions route through
 * the `saves` namespace on `useServerApi()`, which goes through Tauri
 * commands (not `server://` fetches) because the underlying server
 * endpoints use `defineClientEventHandler` and require JWT/cert auth.
 *
 * Restore is type-aware:
 *   - Emulator saves (`.srm` / `.state`) write to
 *     `{install_dir}/drop-saves/{gameId}/(saves|states)` via
 *     `write_save_file`.
 *   - PC-game saves (filename prefixed with `pc/`, or `saveType === "pc"`)
 *     re-scan with Ludusavi via `restore_pc_cloud_save` so they land at
 *     the same on-disk location the game actually reads from.
 */
import {
  ArrowDownTrayIcon,
  ArrowPathIcon,
  ChevronDownIcon,
  CloudIcon,
  TrashIcon,
} from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import { useServerApi, type CloudSaveListEntry } from "~/composables/use-server-api";

const props = defineProps<{
  gameId: string;
}>();

const api = useServerApi();

const expanded = ref(true);
const loading = ref(false);
const loadError = ref<string | null>(null);
const entries = ref<CloudSaveListEntry[]>([]);

// Per-row busy / error state, keyed by cloud save id.
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
      e instanceof Error ? e.message : `Failed to load cloud saves: ${String(e)}`;
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  refresh();
});

// Refetch if the game changes (route navigation between library pages).
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
      // Emulator saves: routed to {install_dir}/drop-saves/{gameId}/.
      await invoke("write_save_file", {
        gameId: props.gameId,
        filename: entry.filename,
        saveType: entry.saveType,
        data,
      });
    }
  } catch (e) {
    rowError.value[entry.id] =
      e instanceof Error ? `Restore failed: ${e.message}` : `Restore failed: ${String(e)}`;
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
      e instanceof Error ? `Delete failed: ${e.message}` : `Delete failed: ${String(e)}`;
    // Leave the confirm modal open so the user can retry.
  } finally {
    delete rowBusy.value[entry.id];
  }
}

// ── Formatters ────────────────────────────────────────────────────────────

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
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

function formatExact(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleString();
  } catch {
    return dateStr;
  }
}

/**
 * Pick a small accent palette per save-type so different kinds of saves
 * are visually distinguishable in the row chip + icon.
 */
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
