<template>
  <div class="h-full flex flex-col overflow-y-auto">
    <!-- Header — back link + the emulator's icon, name and install path.
         Mirrors the console page's calm themed banner. -->
    <div
      class="relative overflow-hidden border-b border-zinc-800/40 bg-gradient-to-br from-zinc-900 via-zinc-950 to-zinc-900 px-8 xl:px-12 pt-8 pb-7"
    >
      <NuxtLink
        to="/library"
        class="inline-flex items-center gap-1.5 text-sm text-zinc-400 transition-colors hover:text-zinc-200"
      >
        <ChevronLeftIcon class="size-4" />
        Library
      </NuxtLink>
      <div class="mt-4 flex min-w-0 items-end gap-5">
        <img
          v-if="host?.iconObjectId"
          :src="useObject(host.iconObjectId)"
          :alt="host.name"
          class="size-24 shrink-0 rounded-2xl object-cover shadow-lg"
        />
        <div
          v-else
          class="flex size-24 shrink-0 items-center justify-center rounded-2xl bg-zinc-800"
        >
          <CpuChipIcon class="size-12 text-zinc-500" />
        </div>
        <div class="min-w-0">
          <p
            class="mb-1 text-[11px] font-semibold uppercase tracking-[0.2em] text-blue-300"
          >
            Emulator
          </p>
          <h1
            class="font-display text-4xl font-bold leading-none text-white drop-shadow-lg"
          >
            {{ host?.name ?? "Emulator" }}
          </h1>
          <p
            v-if="host"
            class="mt-3 max-w-2xl truncate text-sm text-zinc-500"
            :title="host.installDir"
          >
            {{ host.installDir }}
          </p>
        </div>
      </div>
    </div>

    <div class="flex-1 px-8 xl:px-12 py-8 pb-16">
      <div
        v-if="loading"
        class="flex items-center justify-center py-20 text-sm text-zinc-500"
      >
        Loading...
      </div>

      <div
        v-else-if="!host"
        class="flex flex-col items-center justify-center py-20 text-center"
      >
        <p class="text-sm text-zinc-400">This emulator isn't installed.</p>
        <NuxtLink
          to="/library"
          class="mt-3 text-sm text-blue-400 hover:text-blue-300"
        >
          Back to library
        </NuxtLink>
      </div>

      <template v-else>
        <!-- Actions. -->
        <div class="flex flex-wrap items-center gap-3">
          <button
            class="inline-flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2.5 text-sm font-semibold text-zinc-100 transition-colors hover:bg-zinc-700"
            @click="openInstallFolder"
          >
            <FolderOpenIcon class="size-4" />
            Open install folder
          </button>
          <button
            v-if="host.retroarch"
            class="inline-flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2.5 text-sm font-semibold text-zinc-100 transition-colors hover:bg-zinc-700"
            @click="openCoresFolder"
          >
            <FolderOpenIcon class="size-4" />
            Open cores folder
          </button>
          <div class="flex-1" />
          <button
            class="inline-flex items-center gap-2 rounded-lg bg-red-600/90 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-red-600"
            @click="uninstallOpen = true"
          >
            <TrashIcon class="size-4" />
            Uninstall
          </button>
        </div>

        <!-- Cores — RetroArch only. -->
        <section v-if="host.retroarch" class="mt-8">
          <div class="mb-3 flex items-baseline gap-3">
            <h2 class="font-display text-lg font-semibold text-zinc-100">
              Cores
            </h2>
            <span class="text-xs tabular-nums text-zinc-500">
              {{ host.cores.length }}
            </span>
          </div>
          <p class="mb-4 max-w-2xl text-sm text-zinc-400">
            Cores are the individual system emulators RetroArch loads to run a
            ROM. To add one, open the cores folder and drop the core file in.
            Core files are named like
            <code class="rounded bg-zinc-800 px-1 py-0.5 text-xs text-zinc-300"
              >mgba_libretro.dll</code
            >
            on Windows and
            <code class="rounded bg-zinc-800 px-1 py-0.5 text-xs text-zinc-300"
              >mgba_libretro.so</code
            >
            on Linux.
          </p>

          <ul
            v-if="host.cores.length > 0"
            class="divide-y divide-zinc-800 overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900/40"
          >
            <li
              v-for="core in host.cores"
              :key="core"
              class="flex items-center gap-3 px-4 py-2.5"
            >
              <PuzzlePieceIcon class="size-4 shrink-0 text-zinc-500" />
              <span class="min-w-0 truncate text-sm text-zinc-200">
                {{ prettyCoreName(core) }}
              </span>
              <span class="ml-auto shrink-0 font-mono text-xs text-zinc-500">
                {{ core }}
              </span>
            </li>
          </ul>
          <div
            v-else
            class="rounded-xl border border-dashed border-zinc-700 bg-zinc-900/30 px-4 py-8 text-center"
          >
            <p class="text-sm text-zinc-400">No cores installed yet.</p>
            <button
              class="mt-2 text-sm text-blue-400 hover:text-blue-300"
              @click="openCoresFolder"
            >
              Open the cores folder to add one
            </button>
          </div>
        </section>
      </template>
    </div>

    <!-- Uninstall confirmation. Destructive: deletes the emulator's installed
         files, so it must never fire on a stray click. -->
    <Transition
      enter-active-class="ease-out duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="uninstallOpen"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
        @click.self="uninstallOpen = false"
      >
        <div
          class="w-full max-w-sm rounded-xl bg-zinc-900 border border-zinc-700 shadow-2xl"
        >
          <div class="px-6 py-5">
            <h3 class="text-base font-semibold font-display text-zinc-100">
              Uninstall {{ host?.name ?? "emulator" }}?
            </h3>
            <p class="mt-2 text-sm text-zinc-400">
              This permanently deletes the emulator's installed files, including
              any cores you added. Games that rely on this emulator won't launch
              until you reinstall it. It can't be undone.
            </p>
          </div>
          <div
            class="flex justify-end gap-3 border-t border-zinc-700 px-6 py-4"
          >
            <button
              class="rounded-md px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-800 transition-colors"
              @click="uninstallOpen = false"
            >
              Cancel
            </button>
            <button
              class="rounded-md px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 disabled:opacity-50 transition-colors"
              :disabled="uninstalling"
              @click="uninstall"
            >
              {{ uninstalling ? "Uninstalling..." : "Uninstall" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import {
  ChevronLeftIcon,
  CpuChipIcon,
  FolderOpenIcon,
  PuzzlePieceIcon,
  TrashIcon,
} from "@heroicons/vue/24/outline";
import { invoke } from "@tauri-apps/api/core";
import { useListen } from "~/composables/useListen";
import {
  listInstalledEmulators,
  type EmulatorHost,
} from "~/composables/emulators";

const route = useRoute();
const router = useRouter();
const id = computed(() => String(route.params.id));

const host = ref<EmulatorHost | null>(null);
const loading = ref(true);
const uninstallOpen = ref(false);
const uninstalling = ref(false);

async function load() {
  loading.value = true;
  try {
    const hosts = await listInstalledEmulators();
    host.value = hosts.find((h) => h.id === id.value) ?? null;
  } catch (e) {
    console.warn("[emulator] load failed:", e);
    host.value = null;
  } finally {
    loading.value = false;
  }
}

async function openInstallFolder() {
  if (!host.value) return;
  try {
    await invoke("open_game_install_dir", { gameId: host.value.id });
  } catch (e) {
    console.error("[emulator] open install folder failed:", e);
  }
}

async function openCoresFolder() {
  if (!host.value) return;
  try {
    await invoke("open_emulator_cores_dir", { gameId: host.value.id });
  } catch (e) {
    console.error("[emulator] open cores folder failed:", e);
  }
}

async function uninstall() {
  if (!host.value || uninstalling.value) return;
  uninstalling.value = true;
  try {
    await invoke("uninstall_game", { gameId: host.value.id });
    uninstallOpen.value = false;
    router.push("/library");
  } catch (e) {
    console.error("[emulator] uninstall failed:", e);
    uninstalling.value = false;
  }
}

/** Turn `mupen64plus_next_libretro.dll` into "Mupen64plus Next". Falls back to
 *  the raw filename if stripping leaves nothing. */
function prettyCoreName(file: string): string {
  const pretty = file
    .replace(/\.(dll|so|dylib)$/i, "")
    .replace(/_libretro$/i, "")
    .replace(/_/g, " ")
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
  return pretty || file;
}

onMounted(load);

// If an uninstall (or install) completes while this page is open, re-read so
// the cores list and header stay in sync.
useListen("update_library", load);
</script>
