<template>
  <div class="border-b border-zinc-700 py-5">
    <h3 class="text-base font-semibold font-display leading-6 text-zinc-100">
      Downloads
    </h3>
  </div>

  <div class="mt-5">
    <div class="border-b border-zinc-600">
      <div
        class="-ml-4 -mt-2 flex flex-wrap items-center justify-between sm:flex-nowrap"
      >
        <div class="ml-4 mt-2 pb-4">
          <h3 class="text-base font-display font-semibold text-zinc-100">
            Install directories
          </h3>
          <p class="mt-1 text-sm text-zinc-400 max-w-xl">
            This is where Drop will download game files to, and store them
            indefinitely while you play. Drop and games may store other
            information elsewhere, like saves or mods.
          </p>
        </div>
        <div class="ml-4 mt-2 shrink-0">
          <button
            @click="() => (open = true)"
            type="button"
            class="relative inline-flex items-center rounded-md bg-blue-600 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600"
          >
            Add new directory
          </button>
        </div>
      </div>
    </div>
    <ul role="list" class="divide-y divide-gray-800">
      <li
        v-for="(dir, dirIdx) in dirs"
        :key="dir"
        class="flex justify-between gap-x-6 py-5"
      >
        <div class="flex min-w-0 gap-x-4">
          <FolderIcon
            class="h-6 w-6 text-blue-600 flex-none rounded-full"
            alt=""
          />
          <div class="min-w-0 flex-auto">
            <p class="text-sm/6 text-zinc-100">
              {{ dir }}
            </p>
          </div>
        </div>
        <div class="flex shrink-0 items-center gap-x-6">
          <button
            @click="() => openDirectory(dirIdx)"
            class="-m-2.5 block p-2.5 text-zinc-400 hover:text-zinc-100"
            title="Open in file explorer"
          >
            <span class="sr-only">Open in file explorer</span>
            <FolderOpenIcon class="size-5" aria-hidden="true" />
          </button>
          <button
            @click="() => deleteDirectory(dirIdx)"
            :disabled="dirs.length <= 1"
            :class="[
              dirs.length <= 1
                ? 'text-zinc-700'
                : 'text-zinc-400 hover:text-zinc-100',
              '-m-2.5 block p-2.5',
            ]"
          >
            <span class="sr-only">Delete directory</span>
            <TrashIcon class="size-5" aria-hidden="true" />
          </button>
        </div>
      </li>
    </ul>

    <!-- Antivirus (Windows Defender) — offer to exclude Drop's folders so
         cracked game files (and Drop itself) stop getting quarantined, which
         silently breaks installs and launches. Windows-only. -->
    <div v-if="defender?.supported" class="border-t border-zinc-600 py-6">
      <h3 class="text-base font-display font-semibold text-zinc-100">
        Antivirus protection
      </h3>
      <p class="mt-1 text-sm text-zinc-400 max-w-xl">
        Windows Defender sometimes quarantines game files — and Drop itself —
        which breaks installs and launches. Add Drop's folders to Defender's
        exclusion list so it leaves them alone. Windows will ask you to confirm
        with an admin (UAC) prompt.
      </p>
      <div class="mt-3 flex flex-wrap items-center gap-3">
        <button
          type="button"
          @click="addExclusions"
          :disabled="defenderBusy"
          class="inline-flex items-center rounded-md bg-blue-600 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600 disabled:bg-blue-600/50 disabled:cursor-not-allowed"
        >
          {{
            defenderBusy
              ? "Waiting for confirmation…"
              : "Protect Drop's folders"
          }}
        </button>
        <span v-if="defenderDone" class="text-sm text-green-400">
          Exclusions added.
        </span>
        <span
          v-else-if="defender.realtimeEnabled === false"
          class="text-sm text-amber-400"
        >
          Real-time protection is currently off.
        </span>
      </div>
      <p v-if="defenderError" class="mt-2 text-sm text-red-400">
        {{ defenderError }}
      </p>
      <p class="mt-3 text-xs text-zinc-500 break-all">
        Will exclude: {{ defender.managedPaths.join("  •  ") }}
      </p>
    </div>

    <div class="border-t border-zinc-600 py-6">
      <h3 class="text-base font-display font-semibold text-zinc-100">
        Download Settings
      </h3>
      <p class="mt-1 text-sm text-zinc-400 max-w-xl">
        Configure how Drop downloads games and other content.
      </p>

      <div class="mt-6 max-w-xl">
        <label for="threads" class="block text-sm font-medium text-zinc-100">
          Maximum Download Threads
        </label>
        <div class="mt-2">
          <input
            type="number"
            name="threads"
            id="threads"
            min="1"
            max="32"
            v-model="downloadThreads"
            @keypress="validateNumberInput"
            @paste="validatePaste"
            class="block w-full rounded-md border-0 py-1.5 text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 bg-zinc-800 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:text-sm sm:leading-6"
          />
        </div>
        <p class="mt-2 text-sm text-zinc-400">
          The maximum number of concurrent download threads. Higher values may
          download faster but use more system resources. Default is 4.
        </p>
      </div>
      <div class="mt-10 space-y-8">
        <div class="flex flex-row items-center justify-between">
          <div>
            <h3 class="text-sm font-medium leading-6 text-zinc-100">
              Force Offline
            </h3>
            <p class="mt-1 text-sm leading-6 text-zinc-400">
              Drop will not make any external connections
            </p>
          </div>
          <Switch
            v-model="forceOffline"
            :class="[
              forceOffline ? 'bg-blue-600' : 'bg-zinc-700',
              'relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out',
            ]"
          >
            <span
              :class="[
                forceOffline ? 'translate-x-5' : 'translate-x-0',
                'pointer-events-none relative inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
              ]"
            />
          </Switch>
        </div>
      </div>

      <div class="mt-6">
        <button
          type="button"
          @click="saveSettings"
          :disabled="saveState.loading"
          :class="[
            'inline-flex items-center rounded-md px-3 py-2 text-sm font-semibold text-white shadow-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 transition-colors duration-300',
            saveState.success
              ? 'bg-green-600 hover:bg-green-500 focus-visible:outline-green-600'
              : 'bg-blue-600 hover:bg-blue-500 focus-visible:outline-blue-600',
            'disabled:bg-blue-600/50 disabled:cursor-not-allowed',
          ]"
        >
          {{ saveState.success ? "Saved" : "Save Changes" }}
        </button>
      </div>
    </div>
  </div>
  <TransitionRoot as="template" :show="open">
    <Dialog class="relative z-50" @close="open = false">
      <TransitionChild
        as="template"
        enter="ease-out duration-300"
        enter-from="opacity-0"
        enter-to="opacity-100"
        leave="ease-in duration-200"
        leave-from="opacity-100"
        leave-to="opacity-0"
      >
        <div
          class="fixed inset-0 bg-zinc-950 bg-opacity-75 transition-opacity"
        />
      </TransitionChild>

      <div class="fixed inset-0 z-10 w-screen overflow-y-auto">
        <div
          class="flex min-h-full items-end justify-center p-4 text-center sm:items-center sm:p-0"
        >
          <TransitionChild
            as="template"
            enter="ease-out duration-300"
            enter-from="opacity-0 translate-y-4 sm:translate-y-0 sm:scale-95"
            enter-to="opacity-100 translate-y-0 sm:scale-100"
            leave="ease-in duration-200"
            leave-from="opacity-100 translate-y-0 sm:scale-100"
            leave-to="opacity-0 translate-y-4 sm:translate-y-0 sm:scale-95"
          >
            <DialogPanel
              class="relative transform overflow-hidden rounded-lg bg-zinc-900 px-4 pb-4 pt-5 text-left shadow-xl transition-all sm:my-8 sm:w-full sm:max-w-lg sm:p-6"
            >
              <div class="sm:flex sm:items-start">
                <div class="mt-3 w-full sm:ml-4 sm:mt-0">
                  <div>
                    <label
                      for="dir"
                      class="block text-sm/6 font-medium text-zinc-100"
                      >Select game directory</label
                    >
                    <div class="mt-2">
                      <button
                        @click="() => selectDirectory()"
                        class="block text-left w-full rounded-md border-0 px-3 py-1.5 text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 bg-zinc-800 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:text-sm/6"
                      >
                        {{
                          currentDirectory ?? "Click to select a directory..."
                        }}
                      </button>
                    </div>
                    <p class="mt-2 text-sm text-zinc-400" id="dir-description">
                      Select an empty directory to add.
                    </p>
                  </div>
                </div>
              </div>
              <div class="mt-5 sm:mt-4 sm:flex sm:flex-row-reverse">
                <LoadingButton
                  :disabled="currentDirectory == undefined"
                  type="button"
                  :loading="createDirectoryLoading"
                  @click="() => submitDirectory()"
                  :class="[
                    'inline-flex w-full shadow-sm sm:ml-3 sm:w-auto',
                    currentDirectory === undefined
                      ? 'text-zinc-400 bg-blue-600/10 hover:bg-blue-600/10'
                      : 'text-white bg-blue-600 hover:bg-blue-500',
                  ]"
                >
                  Add
                </LoadingButton>
                <button
                  type="button"
                  class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-800 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
                  @click="() => cancelDirectory()"
                  ref="cancelButtonRef"
                >
                  Cancel
                </button>
              </div>
              <div v-if="error" class="mt-3 rounded-md bg-red-600/10 p-4">
                <div class="flex">
                  <div class="flex-shrink-0">
                    <XCircleIcon
                      class="h-5 w-5 text-red-600"
                      aria-hidden="true"
                    />
                  </div>
                  <div class="ml-3">
                    <h3 class="text-sm font-medium text-red-600">
                      {{ error }}
                    </h3>
                  </div>
                </div>
              </div>
            </DialogPanel>
          </TransitionChild>
        </div>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

<script setup lang="ts">
import {
  Dialog,
  DialogPanel,
  TransitionChild,
  TransitionRoot,
} from "@headlessui/vue";
import { FolderIcon, TrashIcon, XCircleIcon } from "@heroicons/vue/16/solid";
import { FolderOpenIcon } from "@heroicons/vue/24/solid";
import { invoke } from "@tauri-apps/api/core";
import { Switch } from "@headlessui/vue";
import { type Settings } from "~/types";

const open = ref(false);
const currentDirectory = ref<string | undefined>(undefined);
const error = ref<string | undefined>(undefined);
const createDirectoryLoading = ref(false);

const dirs = ref<Array<string>>([]);

const settings = await invoke<Settings>("fetch_settings");
const downloadThreads = ref(settings?.maxDownloadThreads ?? 4);
const forceOffline = ref(settings?.forceOffline ?? false);

const saveState = reactive({
  loading: false,
  success: false,
});

type DefenderStatus = {
  supported: boolean;
  realtimeEnabled: boolean | null;
  managedPaths: string[];
};
const defender = ref<DefenderStatus | null>(null);
const defenderBusy = ref(false);
const defenderDone = ref(false);
const defenderError = ref<string | undefined>(undefined);

async function loadDefender() {
  try {
    defender.value = await invoke<DefenderStatus>("get_defender_status");
  } catch {
    defender.value = null;
  }
}

async function addExclusions() {
  defenderError.value = undefined;
  defenderDone.value = false;
  defenderBusy.value = true;
  try {
    await invoke("add_defender_exclusions");
    defenderDone.value = true;
    await loadDefender();
  } catch (e) {
    defenderError.value = typeof e === "string" ? e : String(e);
  } finally {
    defenderBusy.value = false;
  }
}

async function updateDirs() {
  const newDirs = await invoke<Array<string>>("fetch_download_dir_stats");
  dirs.value = newDirs;
}

await updateDirs();
await loadDefender();

async function selectDirectoryDialog(): Promise<string> {
  const res = await invoke("plugin:dialog|open", {
    options: { directory: true },
  });

  return res as string;
}

async function selectDirectory() {
  try {
    const dir = await selectDirectoryDialog();
    currentDirectory.value = dir;
  } catch (e) {
    error.value = e as string;
  }
}

function cancelDirectory() {
  open.value = false;
  currentDirectory.value = undefined;
}

async function submitDirectory() {
  try {
    error.value = undefined;
    if (!currentDirectory.value)
      throw new Error("Please select a directory first");
    createDirectoryLoading.value = true;

    // Add directory
    await invoke("add_download_dir", { newDir: currentDirectory.value });

    // Update list
    await updateDirs();

    currentDirectory.value = undefined;
    createDirectoryLoading.value = false;
    open.value = false;
  } catch (e) {
    error.value = e as string;
    createDirectoryLoading.value = false;
  }
}

async function openDirectory(index: number) {
  await invoke("open_download_dir", { index });
}

async function deleteDirectory(index: number) {
  await invoke("delete_download_dir", { index });
  await updateDirs();
}

async function saveSettings() {
  try {
    saveState.loading = true;
    await invoke("update_settings", {
      newSettings: {
        maxDownloadThreads: downloadThreads.value,
        forceOffline: forceOffline.value,
      },
    });

    // Show success state
    saveState.success = true;

    // Reset back to normal state after 2 seconds
    setTimeout(() => {
      saveState.success = false;
    }, 2000);
  } catch (error) {
    console.error("Failed to save settings:", error);
  } finally {
    saveState.loading = false;
  }
}

function validateNumberInput(event: KeyboardEvent) {
  // Allow only numbers and basic control keys
  if (
    !/^\d$/.test(event.key) &&
    !["Backspace", "Delete", "Tab", "ArrowLeft", "ArrowRight"].includes(
      event.key,
    )
  ) {
    event.preventDefault();
  }
}

function validatePaste(event: ClipboardEvent) {
  // Prevent paste if content contains non-numeric characters
  const pastedData = event.clipboardData?.getData("text");
  if (pastedData && !/^\d+$/.test(pastedData)) {
    event.preventDefault();
  }
}
</script>
