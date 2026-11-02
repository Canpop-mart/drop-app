<template>
  <ModalTemplate size-class="max-w-6xl" v-model="open">
    <template #default>
      <div class="mb-6">
        <p class="text-xs font-semibold uppercase tracking-[0.2em] text-blue-400">Game settings</p>
        <h2 class="mt-1 text-2xl font-semibold text-zinc-100">{{ game.game.mName }}</h2>
      </div>
      <div class="flex min-h-[28rem] max-h-[72vh] flex-row gap-x-7">
        <nav class="w-52 shrink-0" aria-label="Game settings">
          <ul role="list" class="space-y-1">
            <li v-for="(tab, tabIdx) in tabs" :key="tab.name">
              <button
                @click="() => (currentTabIndex = tabIdx)"
                :class="[
                  tabIdx == currentTabIndex
                    ? 'bg-zinc-800 text-zinc-100'
                    : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100',
                  'group flex w-full gap-x-3 rounded-lg px-3 py-2.5 text-sm font-semibold transition',
                ]"
              >
                <component
                  :is="tab.icon"
                  :class="[
                    tabIdx == currentTabIndex
                      ? 'text-zinc-100'
                      : 'text-gray-400 group-hover:text-zinc-100',
                    'size-6 shrink-0',
                  ]"
                  aria-hidden="true"
                />
                {{ tab.name }}
              </button>
            </li>
          </ul>
        </nav>
        <div class="min-h-0 w-full grow overflow-y-auto border-l border-zinc-800 pl-7 pr-2">
          <component
            v-if="activeTab?.page"
            v-model="configuration"
            :is="activeTab.page"
            :proton-enabled="protonEnabled"
            :game-id="game.game.id"
            :install-dir="installDir"
          />
          <div v-else-if="activeTab?.kind === 'versions'">
            <div class="mb-6 flex items-start justify-between gap-6">
              <div>
                <h3 class="text-lg font-semibold text-zinc-100">Versions</h3>
                <p class="mt-1 text-sm text-zinc-400">Choose the version used by the main Play button.</p>
              </div>
              <button class="rounded-lg bg-zinc-100 px-4 py-2 text-sm font-semibold text-zinc-950 hover:bg-white" @click="$emit('install-version')">
                Install another version
              </button>
            </div>
            <div class="divide-y divide-zinc-800 border-y border-zinc-800">
              <div v-for="inst in installs" :key="inst.versionId" class="flex items-center gap-4 py-4">
                <button class="flex min-w-0 flex-1 items-center gap-4 text-left" @click="$emit('select-version', inst.versionId)">
                  <span class="flex size-5 shrink-0 items-center justify-center rounded-full border" :class="selectedVersionId === inst.versionId ? 'border-blue-400' : 'border-zinc-600'">
                    <span v-if="selectedVersionId === inst.versionId" class="size-2.5 rounded-full bg-blue-400" />
                  </span>
                  <span>
                    <span class="block font-medium text-zinc-200">{{ versionLabels[inst.versionId] ?? inst.versionId }}</span>
                    <span class="mt-0.5 block text-xs text-zinc-500">
                      {{ inst.installType === 'PartiallyInstalled' ? 'Incomplete' : inst.installType === 'SetupRequired' ? 'Setup needed' : inst.updateAvailable ? 'Update available' : selectedVersionId === inst.versionId ? 'Used by Play' : 'Installed' }}
                    </span>
                  </span>
                </button>
                <button v-if="inst.updateAvailable && inst.installType !== 'PartiallyInstalled'" class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500" @click="$emit('update-version', inst.versionId)">Update</button>
                <button class="rounded-lg px-4 py-2 text-sm font-medium text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200" @click="$emit('uninstall-version', inst.versionId)">Uninstall</button>
              </div>
            </div>
          </div>
          <CloudSavesPanel
            v-else-if="activeTab?.kind === 'cloud-saves'"
            :game-id="game.game.id"
            :game-name="game.game.mName"
            :is-native-game="isNativeGame"
          />
          <div v-else-if="activeTab?.kind === 'manage'" class="max-w-2xl">
            <h3 class="text-lg font-semibold text-zinc-100">Manage game</h3>
            <p class="mt-1 text-sm text-zinc-400">Tools and actions for this installation.</p>
            <div class="mt-6 divide-y divide-zinc-800 border-y border-zinc-800">
              <button v-if="isNativeGame" class="settings-action" @click="$emit('set-account-name')"><span><strong>Set account name</strong><small>Change the account name used when this game launches.</small></span><UserIcon class="size-5 text-green-400" /></button>
              <button class="settings-action" @click="$emit('open-install-folder')"><span><strong>Open install folder</strong><small>View this game's files in the system file manager.</small></span><FolderOpenIcon class="size-5 text-amber-400" /></button>
              <button v-for="rt in installRuntimes" :key="rt.set" class="settings-action" @click="$emit('install-runtime', rt.set)"><span><strong>{{ rt.label }}</strong><small>Install this runtime into the game's compatibility prefix.</small></span><WrenchScrewdriverIcon class="size-5 text-sky-400" /></button>
              <button v-if="hasAchievements" class="settings-action" @click="$emit('reset-achievements')"><span><strong>Reset achievements</strong><small>Clear Drop's achievement progress for this game.</small></span><TrophyIcon class="size-5 text-yellow-500" /></button>
            </div>
            <div class="mt-8">
              <h4 class="text-sm font-semibold text-red-300">Remove game</h4>
              <div class="mt-3 flex flex-wrap gap-3">
                <button class="rounded-lg bg-red-500/10 px-4 py-2.5 text-sm font-semibold text-red-300 ring-1 ring-inset ring-red-500/25 hover:bg-red-500/20" @click="$emit('uninstall-game')">Uninstall files</button>
                <button class="rounded-lg px-4 py-2.5 text-sm font-semibold text-zinc-400 ring-1 ring-inset ring-zinc-700 hover:bg-zinc-800 hover:text-zinc-200" @click="$emit('remove-from-library')">Remove from library</button>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div v-if="saveError" class="mt-5 rounded-md bg-red-600/10 p-4">
        <div class="flex">
          <div class="flex-shrink-0">
            <XCircleIcon class="h-5 w-5 text-red-600" aria-hidden="true" />
          </div>
          <div class="ml-3">
            <h3 class="text-sm font-medium text-red-600">
              {{ saveError }}
            </h3>
          </div>
        </div>
      </div>
    </template>
    <template #buttons>
      <LoadingButton
        v-if="activeTab?.page"
        @click="() => save()"
        :loading="saveLoading"
        type="submit"
        class="ml-2 w-full sm:w-fit"
      >
        Save
      </LoadingButton>
      <button
        @click="() => (open = false)"
        type="button"
        class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
        ref="cancelButtonRef"
      >
        {{ activeTab?.page ? "Cancel" : "Close" }}
      </button>
    </template>
  </ModalTemplate>
</template>

<script setup lang="ts">
import type { Component } from "vue";
import {
  AdjustmentsHorizontalIcon,
  CloudIcon,
  Cog6ToothIcon,
  RocketLaunchIcon,
  ServerIcon,
  Squares2X2Icon,
  XCircleIcon,
} from "@heroicons/vue/20/solid";
import Launch from "./GameOptions/Launch.vue";
import Updates from "./GameOptions/Updates.vue";
import Video from "./GameOptions/Video.vue";
import Storage from "./GameOptions/Storage.vue";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowPathIcon,
  FolderOpenIcon,
  TrophyIcon,
  UserIcon,
  WrenchScrewdriverIcon,
} from "@heroicons/vue/24/solid";
import type { GameVersion } from "~/types";

const appState = useAppState();

const open = defineModel<boolean>();
// `emulated` is supplied by the parent, which detects it robustly (console
// library membership OR a real emulator launch). The version alone is not
// enough: a ROM imported by a disk scan is Installed but has no cached
// GameVersion yet (game_versions is only filled on an online library sync), so
// `version` is undefined for it even though it is an emulated game.
type InstalledVersion = {
  versionId: string;
  installType: string;
  updateAvailable: boolean;
};

const props = withDefaults(
  defineProps<{
    gameId: string;
    emulated?: boolean;
    isNativeGame?: boolean;
    installs?: InstalledVersion[];
    versionLabels?: Record<string, string>;
    selectedVersionId?: string | null;
    hasAchievements?: boolean;
    showInstallRuntimes?: boolean;
  }>(),
  {
    installs: () => [],
    versionLabels: () => ({}),
    selectedVersionId: null,
  },
);
defineEmits<{
  (e: "select-version", versionId: string): void;
  (e: "install-version"): void;
  (e: "update-version", versionId: string): void;
  (e: "uninstall-version", versionId: string): void;
  (e: "set-account-name"): void;
  (e: "open-install-folder"): void;
  (e: "install-runtime", runtime: string): void;
  (e: "reset-achievements"): void;
  (e: "uninstall-game"): void;
  (e: "remove-from-library"): void;
}>();
const game = await useGame(props.gameId);

// A game whose GameVersion is not cached yet (see above) has no stored
// configuration to edit, so fall back to the backend's default UserConfiguration
// (mirrors `UserConfiguration::default()` in database/src/models.rs). Without
// this fallback the `version.value!` dereference threw during this component's
// async setup, and under the page's <Suspense> that quietly dropped the whole
// modal, so clicking "Configure" opened nothing.
const DEFAULT_USER_CONFIGURATION: GameVersion["userConfiguration"] = {
  launchTemplate: "{}",
  overrideProtonPath: "",
  enableUpdates: false,
  controllerType: null,
  qualityPreset: null,
  widescreen: "Standard",
  fullscreen: null,
  mangohud: null,
  crtShader: false,
  executableOverride: null,
};

const configuration: Ref<GameVersion["userConfiguration"]> = ref(
  game.version.value?.userConfiguration ?? { ...DEFAULT_USER_CONFIGURATION },
);

const hasWindows = !!(
  game.version.value?.setups?.find((v) => v.platform === "Windows") ??
  game.version.value?.launches?.find((v) => v.platform === "Windows")
);

// `appState` can still be null at the instant this modal's async setup runs (it
// is filled by the app's boot sequence). The old `appState.value!` dereference
// threw in that window, and because this is an async-setup component that
// rejection was swallowed by the page's <Suspense>, silently dropping the whole
// modal — so "Configure" opened nothing on *every* game in release builds. (Dev
// happened to have appState ready by then, which is why it only reproduced in
// the built app.) Guard it, and keep it a computed so the Proton options appear
// once state settles rather than being captured as `false` forever.
const protonEnabled = computed(
  () =>
    !!appState.value &&
    appState.value.umuState !== "NotNeeded" &&
    hasWindows,
);

// Emulated games get a "Video & Controls" tab (RetroArch presets). It's listed
// first for them, so the default `currentTabIndex = 0` opens straight to it when
// you hit Configure on a ROM.
//
// Prefer the parent's `emulated` flag, which holds even when `version` is
// missing (a scanned ROM). Fall back to the version's launches for any caller
// that does not pass it. Require a real emulator reference (a truthy `gameId`)
// rather than just `emulator != null`: a PC game's launch can carry an
// empty/placeholder `emulator` object that `!= null` wrongly treats as emulated
// (this showed the tab on native games like Vampire Survivors).
const isEmulatedGame = computed(
  () =>
    props.emulated ||
    (game.version.value?.launches?.some((l) => !!l.emulator?.gameId) ?? false),
);
const installDir = computed(() => {
  const s = game.status.value;
  return s && s.type === "Installed" ? s.install_dir : null;
});

type SettingsTab = {
  name: string;
  icon: Component;
  page?: Component;
  kind?: "versions" | "cloud-saves" | "manage";
};

const tabs = computed<SettingsTab[]>(
  () => [
    ...(isEmulatedGame.value
      ? [
          {
            name: "Video & Controls",
            icon: AdjustmentsHorizontalIcon,
            page: Video,
          },
        ]
      : []),
    { name: "Launch", icon: RocketLaunchIcon, page: Launch },
    { name: "Updates", icon: ArrowPathIcon, page: Updates },
    { name: "Storage", icon: ServerIcon, page: Storage },
    ...(props.installs.length > 0
      ? [{ name: "Versions", icon: Squares2X2Icon, kind: "versions" as const }]
      : []),
    { name: "Cloud Saves", icon: CloudIcon, kind: "cloud-saves" },
    { name: "Manage", icon: Cog6ToothIcon, kind: "manage" },
  ],
);
const currentTabIndex = ref(0);
const activeTab = computed(() => tabs.value[currentTabIndex.value]);
watch(tabs, (next) => {
  if (currentTabIndex.value >= next.length) currentTabIndex.value = 0;
});

const installRuntimes = computed(() =>
  props.showInstallRuntimes
    ? [
        { set: "vcpp", label: "Install VC++ runtime" },
        { set: "directx", label: "Install DirectX runtime" },
        { set: "dotnet", label: "Install .NET runtime" },
      ]
    : [],
);

const saveLoading = ref(false);
const saveError = ref<undefined | string>();
async function save() {
  saveLoading.value = true;
  saveError.value = undefined;
  try {
    await invoke("update_game_configuration", {
      gameId: game.game.id,
      options: configuration.value,
    });
    open.value = false;
    saveError.value = undefined;
  } catch (e) {
    saveError.value = (e as unknown as string).toString();
  }
  saveLoading.value = false;
}
</script>

<style scoped>
.settings-action {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 0.25rem;
  text-align: left;
}

.settings-action:hover strong {
  color: rgb(244 244 245);
}

.settings-action span {
  display: flex;
  min-width: 0;
  flex-direction: column;
}

.settings-action strong {
  color: rgb(212 212 216);
  font-size: 0.875rem;
  font-weight: 600;
}

.settings-action small {
  margin-top: 0.125rem;
  color: rgb(113 113 122);
  font-size: 0.75rem;
}
</style>
