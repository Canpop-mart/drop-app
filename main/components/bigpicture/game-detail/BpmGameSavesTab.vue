<!--
  The "Saves" tab body for the BPM game-detail page.

  Pure presentation: every piece of state and every action comes in via the
  `saves` prop, which is the object returned by `useBpmGameSaves()`. The
  page owns that composable (so its watchers/refs are page-scoped) and the
  parent's focus-nav `registerAction` is passed straight through so the
  buttons here join the page's "content" focus group.

  Extracted verbatim from the 3232-line `pages/bigpicture/library/[id].vue`
  — markup and classes are unchanged.
-->
<template>
  <div class="space-y-4">
    <!-- Unified save list: merges local + cloud saves -->
    <div v-if="saves.mergedSaves.value.length > 0" class="space-y-2">
      <div
        v-for="item in saves.mergedSaves.value"
        :key="item.filename"
        class="flex items-center gap-4 bg-zinc-900/50 rounded-xl p-4"
      >
        <!-- Icon based on file type -->
        <div
          class="size-10 rounded-lg flex items-center justify-center flex-shrink-0"
          :style="{ backgroundColor: saveTypeColor(item.filename).bg }"
        >
          <svg class="size-5" :style="{ color: saveTypeColor(item.filename).text }" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path v-if="item.filename.endsWith('.srm')" stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375" />
            <path v-else-if="item.filename.endsWith('.png')" stroke-linecap="round" stroke-linejoin="round" d="M2.25 15.75l5.159-5.159a2.25 2.25 0 013.182 0l5.159 5.159m-1.5-1.5l1.409-1.409a2.25 2.25 0 013.182 0l2.909 2.909M3.75 21h16.5A2.25 2.25 0 0022.5 18.75V5.25A2.25 2.25 0 0020.25 3H3.75A2.25 2.25 0 001.5 5.25v13.5A2.25 2.25 0 003.75 21z" />
            <path v-else stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3" />
          </svg>
        </div>

        <!-- Info -->
        <div class="flex-1 min-w-0">
          <p class="text-sm font-medium text-zinc-200 truncate">{{ item.filename }}</p>
          <p class="text-xs text-zinc-500">
            {{ saveTypeLabel(item.filename) }}
            <template v-if="item.local">&middot; {{ formatSaveSize(item.local.size) }} &middot; {{ formatTimeAgo(new Date(item.local.modified * 1000).toISOString()) }}</template>
          </p>
          <!-- Sync status badges -->
          <div class="flex gap-2 mt-1">
            <span v-if="item.local" class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-green-900/30 text-green-400">
              Local
            </span>
            <span v-if="item.cloud" class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-blue-900/30 text-blue-400">
              Cloud &middot; {{ formatSaveSize(item.cloud.size) }}
            </span>
            <span v-if="!item.local && item.cloud" class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-yellow-900/30 text-yellow-400">
              Cloud only — download to play
            </span>
          </div>
        </div>

        <!-- Actions -->
        <div class="flex items-center gap-2 flex-shrink-0">
          <button
            v-if="item.local"
            :ref="(el: any) => registerAction(el, { onSelect: () => saves.requestUpload(item.local!) })"
            class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-blue-900/20 text-blue-400 hover:bg-blue-900/30"
            @click="saves.requestUpload(item.local!)"
          >
            {{ saves.cloudSyncStatus.value[item.filename] === 'uploading' ? 'Syncing...' : (item.cloud ? 'Re-sync' : 'Sync to Cloud') }}
          </button>
          <button
            v-if="item.cloud"
            :ref="(el: any) => registerAction(el, { onSelect: () => saves.requestDownload(item.filename, item.cloud!.saveType) })"
            class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-green-900/20 text-green-400 hover:bg-green-900/30"
            @click="saves.requestDownload(item.filename, item.cloud!.saveType)"
          >
            {{ saves.cloudSyncStatus.value[item.filename] === 'downloading' ? 'Downloading...' : 'Download' }}
          </button>
          <button
            v-if="item.local"
            :ref="(el: any) => registerAction(el, { onSelect: () => saves.deleteSave(item.local!) })"
            class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-red-900/20 text-red-400 hover:bg-red-900/30"
            @click="saves.deleteSave(item.local!)"
          >
            Delete
          </button>
        </div>
      </div>
    </div>

    <!-- PC Game saves via Ludusavi -->
    <div v-if="saves.pcSaves.value.length > 0" class="mt-6">
      <div class="flex items-center justify-between mb-3">
        <h4 class="text-sm font-semibold" style="color: var(--bpm-muted)">PC GAME SAVES (via Ludusavi)</h4>
        <div class="flex gap-2">
          <button
            :ref="(el: any) => registerAction(el, { onSelect: saves.backupPcSaves })"
            class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-blue-900/20 text-blue-400 hover:bg-blue-900/30"
            @click="saves.backupPcSaves()"
          >
            {{ saves.pcSaveStatus.value === 'backing-up' ? 'Backing up...' : 'Backup All' }}
          </button>
          <button
            :ref="(el: any) => registerAction(el, { onSelect: saves.restorePcSaves })"
            class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-green-900/20 text-green-400 hover:bg-green-900/30"
            @click="saves.restorePcSaves()"
          >
            {{ saves.pcSaveStatus.value === 'restoring' ? 'Restoring...' : 'Restore' }}
          </button>
        </div>
      </div>
      <!-- Grouped save slots -->
      <div class="space-y-3">
        <div
          v-for="group in saves.pcSaveGroups.value"
          :key="group.name"
          class="rounded-xl overflow-hidden"
          style="background-color: var(--bpm-surface)"
        >
          <!-- Primary save row -->
          <div class="flex items-center gap-4 p-4">
            <div
              class="flex items-center gap-4 flex-1 min-w-0 cursor-pointer"
              :ref="(el: any) => registerAction(el, { onSelect: () => { if (group.backups.length > 0) group.expanded = !group.expanded; } })"
              @click="() => { if (group.backups.length > 0) group.expanded = !group.expanded; }"
            >
              <div
                class="size-10 rounded-lg flex items-center justify-center flex-shrink-0"
                :style="{ backgroundColor: group.type === 'settings' ? 'rgba(156,163,175,0.15)' : 'rgba(34,197,94,0.15)' }"
              >
                <svg v-if="group.type === 'settings'" class="size-5 text-zinc-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" />
                  <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                <svg v-else class="size-5 text-green-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375" />
                </svg>
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-zinc-200">{{ group.label }}</p>
                <p class="text-xs text-zinc-500">
                  {{ group.primary ? pcSaveFileName(group.primary.path) : group.name }}
                  <template v-if="group.primary && group.primary.size > 0"> &middot; {{ formatSaveSize(group.primary.size) }}</template>
                  <template v-if="group.backups.length > 0"> &middot; {{ group.backups.length }} backup{{ group.backups.length !== 1 ? 's' : '' }}</template>
                </p>
              </div>
            </div>
            <!-- Action buttons -->
            <div class="flex items-center gap-2 flex-shrink-0">
              <span v-if="saves.pcCloudStatus.value[group.name]" class="text-xs px-2 py-0.5 rounded-full"
                :class="saves.pcCloudStatus.value[group.name] === 'synced' ? 'bg-green-900/20 text-green-400' : saves.pcCloudStatus.value[group.name] === 'cloud-only' ? 'bg-blue-900/20 text-blue-400' : 'bg-zinc-800 text-zinc-500'"
              >
                {{ saves.pcCloudStatus.value[group.name] === 'synced' ? 'Synced' : saves.pcCloudStatus.value[group.name] === 'cloud-only' ? 'Cloud' : saves.pcCloudStatus.value[group.name] === 'cloud-newer' ? 'Cloud newer' : 'Local newer' }}
              </span>
              <button
                v-if="group.primary && saves.pcSyncStatus.value[group.name] !== 'uploading'"
                :ref="(el: any) => registerAction(el, { onSelect: () => saves.uploadPcSave(group) })"
                class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-blue-900/20 text-blue-400 hover:bg-blue-900/30"
                @click.stop="saves.uploadPcSave(group)"
              >
                Upload
              </button>
              <span v-else-if="saves.pcSyncStatus.value[group.name] === 'uploading'" class="text-xs text-blue-400 animate-pulse">Uploading...</span>
              <button
                v-if="saves.hasPcCloudSave(group.name) && saves.pcSyncStatus.value[group.name] !== 'downloading'"
                :ref="(el: any) => registerAction(el, { onSelect: () => saves.downloadPcSave(group) })"
                class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-green-900/20 text-green-400 hover:bg-green-900/30"
                @click.stop="saves.downloadPcSave(group)"
              >
                Download
              </button>
              <span v-else-if="saves.pcSyncStatus.value[group.name] === 'downloading'" class="text-xs text-green-400 animate-pulse">Downloading...</span>
            </div>
          </div>

          <!-- Expandable backups -->
          <div v-if="group.expanded && group.backups.length > 0" class="border-t px-4 pb-3 pt-2 space-y-1" style="border-color: var(--bpm-border)">
            <div
              v-for="backup in group.backups"
              :key="backup.path"
              class="flex items-center gap-3 py-1.5 pl-14"
            >
              <div class="size-5 rounded flex items-center justify-center flex-shrink-0" style="background-color: rgba(234,179,8,0.1)">
                <svg class="size-3 text-yellow-500" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
                </svg>
              </div>
              <span class="text-xs text-zinc-400 truncate">{{ pcSaveFileName(backup.path) }}</span>
              <span v-if="backup.size > 0" class="text-xs text-zinc-600 flex-shrink-0">{{ formatSaveSize(backup.size) }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="!saves.ludusaviAvailable.value && isNativeGame" class="mt-4 p-4 rounded-xl" style="background-color: var(--bpm-surface)">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-medium" style="color: var(--bpm-text)">PC Save Management</p>
          <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">
            Drop uses Ludusavi to detect and back up PC game saves. Install it to enable save management for this game.
          </p>
        </div>
        <button
          :ref="(el: any) => registerAction(el, { onSelect: saves.doInstallLudusavi })"
          class="px-4 py-2 text-sm font-medium rounded-lg flex-shrink-0 ml-4"
          :style="{ backgroundColor: 'var(--bpm-accent-hex)', color: 'var(--bpm-accent-text)' }"
          :disabled="saves.ludusaviInstalling.value"
          @click="saves.doInstallLudusavi()"
        >
          {{ saves.ludusaviInstalling.value ? 'Installing...' : 'Install Ludusavi' }}
        </button>
      </div>
    </div>

    <p v-if="saves.mergedSaves.value.length === 0 && saves.pcSaves.value.length === 0 && !saves.savesLoading.value" class="text-zinc-500 text-center py-8 text-sm">
      <template v-if="isNativeGame && !saves.ludusaviAvailable.value">
        <!-- Ludusavi prompt handles this case above -->
      </template>
      <template v-else-if="isNativeGame && saves.ludusaviAvailable.value">
        No saves detected by Ludusavi for this game.
      </template>
      <template v-else>
        No save data found for this game. Play the game to create saves.
      </template>
    </p>
    <p v-if="saves.savesLoading.value" class="text-zinc-500 text-center py-8 text-sm">
      Loading saves...
    </p>
  </div>
</template>

<script setup lang="ts">
import {
  useBpmGameSaves,
  saveTypeColor,
  saveTypeLabel,
  formatSaveSize,
  pcSaveFileName,
} from "~/composables/bigpicture/use-bpm-game-saves";

defineProps<{
  /** The object returned by the page's `useBpmGameSaves()` call. */
  saves: ReturnType<typeof useBpmGameSaves>;
  /** True for native (non-emulated) games — drives the Ludusavi prompt. */
  isNativeGame: boolean;
  /** The page's focus-nav action registrar (the "content" group). */
  registerAction: (el: any, opts: { onSelect: () => void }) => void;
  /** Page-supplied "Nm ago" formatter. */
  formatTimeAgo: (dateStr: string) => string;
}>();
</script>
