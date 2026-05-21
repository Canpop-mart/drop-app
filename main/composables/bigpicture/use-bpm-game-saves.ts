/**
 * Save-data management for the BPM game-detail page's "Saves" tab.
 *
 * Three save sources, all merged into one couch-friendly UI:
 *  - Emulator saves (`.srm` / `.state`) read via `list_game_saves`.
 *  - Cloud saves synced through the Drop server (`api/v1/client/saves`).
 *  - PC-game saves detected by Ludusavi (`list_pc_game_saves`), grouped
 *    into save slots with their auto-backups.
 *
 * Plus the cross-source plumbing: a `mergedSaves` view (local ∪ cloud),
 * per-PC-save cloud-sync status, upload/download with automatic `.bak`
 * backups, and a small library of pure formatting helpers.
 *
 * Extracted verbatim — behaviour-identical — from the 3232-line
 * `pages/bigpicture/library/[id].vue`. The Saves tab is dev-mode gated by
 * the caller; this composable wires the data + actions behind it.
 *
 * Per-game-detail composable: NOT a singleton — call from a component
 * `setup()`.
 */

import { invoke } from "@tauri-apps/api/core";
import { devLog } from "~/composables/dev-mode";
import { serverUrl } from "~/composables/use-server-fetch";

// ── Types ───────────────────────────────────────────────────────────────

export interface SaveFile {
  filename: string;
  size: number;
  modified: number;
  save_type: string;
}

export interface LudusaviFile {
  path: string;
  size: number;
  modified: number;
}

export interface CloudSaveEntry {
  id: string;
  filename: string;
  saveType: string;
  size: number;
  clientModifiedAt: string;
  uploadedAt: string;
}

/** A Ludusavi save slot with its auto-backups attached. */
export interface PcSaveGroup {
  name: string;
  label: string;
  type: "save" | "settings" | "other";
  primary: LudusaviFile | null;
  backups: LudusaviFile[];
  expanded: boolean;
}

/** A unified row in the Saves tab: a save that exists locally, in cloud, or both. */
export interface MergedSave {
  filename: string;
  local: SaveFile | null;
  cloud: CloudSaveEntry | null;
}

export type SyncConfirmAction = {
  type: "upload" | "download";
  save: SaveFile | null;
  filename: string;
  saveType: string;
};

// ── Pure formatting helpers (exported for the tab template) ─────────────

/** Filename of a Ludusavi path (handles Windows + POSIX separators). */
export function pcSaveFileName(fullPath: string): string {
  const parts = fullPath.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || fullPath;
}

/** Human label for an emulator save file based on its extension. */
export function saveTypeLabel(filename: string): string {
  if (filename.endsWith(".srm")) return "Game Progress (Battery Save)";
  if (filename.endsWith(".state.png")) return "Save State Screenshot";
  if (filename.endsWith(".state")) return "Save State (Exact Position)";
  if (filename.endsWith(".sav")) return "Game Save";
  return "Save File";
}

/** Icon background/foreground colours for an emulator save file. */
export function saveTypeColor(filename: string): { bg: string; text: string } {
  if (filename.endsWith(".srm"))
    return { bg: "rgba(34,197,94,0.15)", text: "#22c55e" };
  if (filename.endsWith(".state.png"))
    return { bg: "rgba(168,85,247,0.15)", text: "#a855f7" };
  if (filename.endsWith(".state"))
    return { bg: "rgba(59,130,246,0.15)", text: "#3b82f6" };
  return { bg: "rgba(156,163,175,0.15)", text: "#9ca3af" };
}

/** Format a byte count as B / KB / MB. */
export function formatSaveSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── Composable ──────────────────────────────────────────────────────────

export function useBpmGameSaves(
  gameId: string,
  /** The loaded game's display name — Ludusavi needs it to match saves. */
  gameName: Ref<string | undefined>,
  /** Whether this is a native (non-emulated) game — gates the Ludusavi UI. */
  isNativeGame: Ref<boolean>,
  /** Surface a save operation failure as a page-level error. */
  onError: (msg: string) => void,
) {
  // ── Emulator saves ────────────────────────────────────────────────────
  const gameSaves = ref<SaveFile[]>([]);
  const savesLoading = ref(false);

  async function fetchSaves() {
    savesLoading.value = true;
    try {
      gameSaves.value = await invoke<SaveFile[]>("list_game_saves", {
        gameId,
      });
    } catch {
      gameSaves.value = [];
    } finally {
      savesLoading.value = false;
    }
  }

  async function deleteSave(save: SaveFile) {
    try {
      await invoke("delete_game_save", {
        gameId,
        filename: save.filename,
        saveType: save.save_type,
      });
      gameSaves.value = gameSaves.value.filter(
        (s) => s.filename !== save.filename,
      );
    } catch (e) {
      console.error("[BPM:GAME] Failed to delete save:", e);
      onError(
        `Failed to delete save: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }

  // ── Cloud saves ───────────────────────────────────────────────────────
  const cloudSaves = ref<CloudSaveEntry[]>([]);
  const cloudSyncStatus = ref<Record<string, string>>({});

  async function fetchCloudSaves() {
    try {
      const resp = await fetch(
        serverUrl(`api/v1/client/saves/list?gameId=${gameId}`),
      );
      if (resp.ok) {
        cloudSaves.value = await resp.json();
      }
    } catch {
      /* non-critical */
    }
  }

  const confirmSyncAction = ref<SyncConfirmAction | null>(null);

  /** Upload a local save — confirm first if a cloud copy already exists. */
  function requestUpload(save: SaveFile) {
    const hasCloud = cloudSaves.value.some((c) => c.filename === save.filename);
    if (hasCloud) {
      confirmSyncAction.value = {
        type: "upload",
        save,
        filename: save.filename,
        saveType: save.save_type,
      };
    } else {
      doUpload(save);
    }
  }

  /** Download a cloud save — confirm first if a local copy already exists. */
  function requestDownload(filename: string, saveType: string) {
    const hasLocal = gameSaves.value.some((s) => s.filename === filename);
    if (hasLocal) {
      confirmSyncAction.value = {
        type: "download",
        save: null,
        filename,
        saveType,
      };
    } else {
      doDownload(filename, saveType);
    }
  }

  function confirmSync() {
    if (!confirmSyncAction.value) return;
    const action = confirmSyncAction.value;
    confirmSyncAction.value = null;
    if (action.type === "upload" && action.save) {
      doUpload(action.save);
    } else if (action.type === "download") {
      doDownload(action.filename, action.saveType);
    }
  }

  async function doUpload(save: SaveFile) {
    cloudSyncStatus.value[save.filename] = "uploading";
    try {
      // Before overwriting, back up the existing cloud version with a
      // `.bak` suffix so the user can recover from a bad sync.
      const existingCloud = cloudSaves.value.find(
        (c) => c.filename === save.filename,
      );
      if (existingCloud) {
        const backupResp = await fetch(
          serverUrl(`api/v1/client/saves/download?id=${existingCloud.id}`),
        );
        if (backupResp.ok) {
          const backupData = await backupResp.json();
          await fetch(serverUrl("api/v1/client/saves/upload"), {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              gameId,
              filename: save.filename + ".bak",
              saveType: save.save_type,
              data: backupData.data,
              clientModifiedAt: existingCloud.clientModifiedAt,
            }),
          });
        }
      }

      const base64Data = await invoke<string>("read_save_file", {
        gameId,
        filename: save.filename,
        saveType: save.save_type,
      });

      const resp = await fetch(serverUrl("api/v1/client/saves/upload"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          gameId,
          filename: save.filename,
          saveType: save.save_type,
          data: base64Data,
          clientModifiedAt: new Date(save.modified * 1000).toISOString(),
        }),
      });
      if (!resp.ok) throw new Error(`Upload failed: ${resp.status}`);

      await fetchCloudSaves();
    } catch (e) {
      console.error("[BPM:GAME] Cloud save upload failed:", e);
      onError(`Upload failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      delete cloudSyncStatus.value[save.filename];
    }
  }

  async function doDownload(filename: string, saveType: string) {
    const cloudEntry = cloudSaves.value.find((c) => c.filename === filename);
    if (!cloudEntry) return;

    cloudSyncStatus.value[filename] = "downloading";
    try {
      // Before overwriting the local file, back it up with `.bak`.
      const localSave = gameSaves.value.find((s) => s.filename === filename);
      if (localSave) {
        try {
          const localData = await invoke<string>("read_save_file", {
            gameId,
            filename: localSave.filename,
            saveType: localSave.save_type,
          });
          await invoke("write_save_file", {
            gameId,
            filename: filename + ".bak",
            saveType,
            data: localData,
          });
        } catch {
          // Backup failed — continue anyway.
        }
      }

      const resp = await fetch(
        serverUrl(`api/v1/client/saves/download?id=${cloudEntry.id}`),
      );
      if (!resp.ok) throw new Error(`Download failed: ${resp.status}`);
      const { data } = await resp.json();

      await invoke("write_save_file", { gameId, filename, saveType, data });
      await fetchSaves();
    } catch (e) {
      console.error("[BPM:GAME] Cloud save download failed:", e);
      onError(`Download failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      delete cloudSyncStatus.value[filename];
    }
  }

  // ── Merged save view (local ∪ cloud) ──────────────────────────────────
  const mergedSaves = computed((): MergedSave[] => {
    const map = new Map<string, MergedSave>();
    for (const save of gameSaves.value) {
      map.set(save.filename, {
        filename: save.filename,
        local: save,
        cloud: null,
      });
    }
    for (const cloud of cloudSaves.value) {
      const existing = map.get(cloud.filename);
      if (existing) {
        existing.cloud = cloud;
      } else {
        map.set(cloud.filename, {
          filename: cloud.filename,
          local: null,
          cloud,
        });
      }
    }
    // .srm first, then .state, then .png; newest first within each group.
    return [...map.values()].sort((a, b) => {
      const extOrder = (f: string) =>
        f.endsWith(".srm") ? 0 : f.endsWith(".state") ? 1 : 2;
      const diff = extOrder(a.filename) - extOrder(b.filename);
      if (diff !== 0) return diff;
      return (b.local?.modified ?? 0) - (a.local?.modified ?? 0);
    });
  });

  // ── Ludusavi PC-game saves ────────────────────────────────────────────
  const pcSaves = ref<LudusaviFile[]>([]);
  const pcSaveStatus = ref("");
  const ludusaviAvailable = ref(false);
  const ludusaviInstalling = ref(false);
  const pcSaveGroups = ref<PcSaveGroup[]>([]);

  async function doInstallLudusavi() {
    ludusaviInstalling.value = true;
    try {
      await invoke("install_ludusavi");
      ludusaviAvailable.value = true;
      await fetchPcSaves();
    } catch (e) {
      console.error("[BPM:GAME] Ludusavi install failed:", e);
      onError(
        `Ludusavi install failed: ${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      ludusaviInstalling.value = false;
    }
  }

  async function fetchPcSaves() {
    try {
      ludusaviAvailable.value = await invoke("check_ludusavi");
      if (!ludusaviAvailable.value || !gameName.value) return;
      const result = await invoke<{ files: LudusaviFile[]; game_name: string }>(
        "list_pc_game_saves",
        { gameId, gameName: gameName.value },
      );
      pcSaves.value = result.files;
    } catch {
      pcSaves.value = [];
    }
  }

  async function backupPcSaves() {
    if (!gameName.value) return;
    pcSaveStatus.value = "backing-up";
    try {
      const backupPath = await invoke<string>("backup_pc_game_saves", {
        gameId,
        gameName: gameName.value,
      });
      devLog("state", "[BPM:GAME] Ludusavi backup at:", backupPath);
    } catch (e) {
      onError(`Backup failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      pcSaveStatus.value = "";
    }
  }

  async function restorePcSaves() {
    pcSaveStatus.value = "restoring";
    try {
      const backupPath =
        `${await invoke("get_temp_dir")}drop-ludusavi-${gameId}`.replace(
          /\\/g,
          "/",
        );
      await invoke("restore_pc_game_saves", { backupPath });
    } catch (e) {
      onError(`Restore failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      pcSaveStatus.value = "";
    }
  }

  // Group raw Ludusavi files into save slots, attaching `_backupN` files
  // to their parent slot. Crash reports / logs / temp files are dropped.
  watch(
    pcSaves,
    (saves) => {
      const filtered = saves.filter((f) => {
        const lower = f.path.toLowerCase();
        if (lower.includes("crashreportclient")) return false;
        if (lower.includes("uecc-windows-")) return false;
        if (lower.endsWith(".log") || lower.endsWith(".tmp")) return false;
        return true;
      });

      const groups = new Map<string, PcSaveGroup>();
      // Case-insensitive keys to avoid duplicates on Windows.
      for (const file of filtered) {
        const filename = pcSaveFileName(file.path);
        const lower = filename.toLowerCase();

        const backupMatch = lower.match(/^(.+?)_backup\d*\.(\w+)$/);
        if (backupMatch) {
          const parentKey = `${backupMatch[1]}.${backupMatch[2]}`;
          const existing = groups.get(parentKey);
          if (existing) {
            existing.backups.push(file);
          } else {
            const displayName = filename.replace(/_backup\d*/, "");
            groups.set(parentKey, {
              name: displayName,
              label: displayName.replace(/_/g, " ").replace(/\.\w+$/, ""),
              type: "save",
              primary: null,
              backups: [file],
              expanded: false,
            });
          }
          continue;
        }

        const isSettings =
          lower.endsWith(".ini") || lower.endsWith(".cfg");
        const type = isSettings ? ("settings" as const) : ("save" as const);

        const existing = groups.get(lower);
        if (existing) {
          if (!existing.primary || file.size > existing.primary.size) {
            existing.primary = file;
          }
          existing.type = type;
          existing.label = isSettings
            ? "Settings"
            : filename.replace(/_/g, " ").replace(/\.\w+$/, "");
        } else {
          groups.set(lower, {
            name: filename,
            label: isSettings
              ? "Settings"
              : filename.replace(/_/g, " ").replace(/\.\w+$/, ""),
            type,
            primary: file,
            backups: [],
            expanded: false,
          });
        }
      }

      // Saves first, then settings.
      pcSaveGroups.value = [...groups.values()].sort((a, b) => {
        const typeOrder = (t: string) =>
          t === "save" ? 0 : t === "settings" ? 2 : 1;
        return typeOrder(a.type) - typeOrder(b.type);
      });
    },
    { immediate: true },
  );

  // ── Per-PC-save cloud sync ────────────────────────────────────────────
  const pcSyncStatus = ref<Record<string, string>>({});
  const pcCloudSaves = ref<Record<string, CloudSaveEntry>>({});
  const pcCloudStatus = ref<Record<string, string>>({});

  function refreshPcCloudStatus() {
    const map: Record<string, CloudSaveEntry> = {};
    for (const cloud of cloudSaves.value) {
      // PC-game cloud saves are namespaced with a `pc:` filename prefix.
      if (cloud.filename.startsWith("pc:")) {
        map[cloud.filename.slice(3).toLowerCase()] = cloud;
      }
    }
    pcCloudSaves.value = map;

    const status: Record<string, string> = {};
    for (const group of pcSaveGroups.value) {
      const cloud = map[group.name.toLowerCase()];
      if (!cloud) continue;
      if (!group.primary) {
        status[group.name] = "cloud-only";
      } else {
        const localModified = group.primary.modified * 1000;
        const cloudModified = new Date(cloud.clientModifiedAt).getTime();
        if (Math.abs(localModified - cloudModified) < 2000) {
          status[group.name] = "synced";
        } else if (cloudModified > localModified) {
          status[group.name] = "cloud-newer";
        } else {
          status[group.name] = "local-newer";
        }
      }
    }
    pcCloudStatus.value = status;
  }

  watch(cloudSaves, refreshPcCloudStatus, { immediate: true });
  watch(pcSaveGroups, refreshPcCloudStatus);

  /** Whether a PC save group has a cloud counterpart (template helper). */
  function hasPcCloudSave(groupName: string): boolean {
    return groupName.toLowerCase() in pcCloudSaves.value;
  }

  async function uploadPcSave(group: PcSaveGroup) {
    if (!group.primary || !gameName.value) return;
    pcSyncStatus.value[group.name] = "uploading";
    try {
      const base64Data = await invoke<string>("read_pc_save_file", {
        filePath: group.primary.path,
      });
      const resp = await fetch(serverUrl("api/v1/client/saves/upload"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          gameId,
          filename: `pc:${group.name}`,
          saveType: "save",
          data: base64Data,
          clientModifiedAt: new Date(
            group.primary.modified * 1000,
          ).toISOString(),
        }),
      });
      if (!resp.ok) throw new Error(`Upload failed: ${resp.status}`);
      await fetchCloudSaves();
      devLog("state", "[BPM:GAME] PC save uploaded:", group.name);
    } catch (e) {
      console.error("[BPM:GAME] PC save upload failed:", e);
      onError(`Upload failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      delete pcSyncStatus.value[group.name];
    }
  }

  async function downloadPcSave(group: PcSaveGroup) {
    const cloudEntry = pcCloudSaves.value[group.name.toLowerCase()];
    if (!cloudEntry) return;
    pcSyncStatus.value[group.name] = "downloading";
    try {
      const resp = await fetch(
        serverUrl(`api/v1/client/saves/download?id=${cloudEntry.id}`),
      );
      if (!resp.ok) throw new Error(`Download failed: ${resp.status}`);
      const { data } = await resp.json();

      if (!group.primary) {
        onError("No local path known for this save — cannot restore.");
        return;
      }
      await invoke("write_pc_save_file", {
        filePath: group.primary.path,
        data,
      });
      await fetchPcSaves();
      devLog("state", "[BPM:GAME] PC save downloaded:", group.name);
    } catch (e) {
      console.error("[BPM:GAME] PC save download failed:", e);
      onError(`Download failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      delete pcSyncStatus.value[group.name];
    }
  }

  /**
   * Load every save source. Call when the Saves tab becomes active. Skips
   * the emulator-save fetch if it already ran (cloud + PC always refresh).
   */
  function loadAll() {
    if (gameSaves.value.length === 0) fetchSaves();
    fetchCloudSaves();
    if (isNativeGame.value) fetchPcSaves();
  }

  return {
    // Emulator saves
    gameSaves,
    savesLoading,
    fetchSaves,
    deleteSave,
    // Cloud saves
    cloudSaves,
    cloudSyncStatus,
    confirmSyncAction,
    requestUpload,
    requestDownload,
    confirmSync,
    // Merged view
    mergedSaves,
    // Ludusavi PC saves
    pcSaves,
    pcSaveStatus,
    pcSaveGroups,
    ludusaviAvailable,
    ludusaviInstalling,
    doInstallLudusavi,
    backupPcSaves,
    restorePcSaves,
    // PC cloud sync
    pcSyncStatus,
    pcCloudStatus,
    hasPcCloudSave,
    uploadPcSave,
    downloadPcSave,
    // Lifecycle
    loadAll,
  };
}
