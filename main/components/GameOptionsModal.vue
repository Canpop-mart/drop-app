<template>
  <ModalTemplate size-class="max-w-4xl" v-model="open">
    <template #default>
      <!-- Grow to fit the active tab (the Video tab is taller than the old
           fixed h-96), but never past the viewport — then the content pane
           scrolls instead of the whole modal overflowing. -->
      <div class="flex flex-row gap-x-4 min-h-[20rem] max-h-[70vh]">
        <nav class="flex flex-1 flex-col" aria-label="Sidebar">
          <ul role="list" class="-mx-2 space-y-1">
            <li v-for="(tab, tabIdx) in tabs" :key="tab.name">
              <button
                @click="() => (currentTabIndex = tabIdx)"
                :class="[
                  tabIdx == currentTabIndex
                    ? 'bg-zinc-800 text-zinc-100'
                    : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100',
                  'transition w-full group flex gap-x-3 rounded-md p-2 text-sm/6 font-semibold',
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
        <div
          class="border-l-2 border-zinc-800 w-full grow pl-4 overflow-y-auto min-h-0"
        >
          <component
            v-model="configuration"
            :is="tabs[currentTabIndex]?.page"
            :proton-enabled="protonEnabled"
            :game-id="game.game.id"
            :install-dir="installDir"
          />
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
        Cancel
      </button>
    </template>
  </ModalTemplate>
</template>

<script setup lang="ts">
import type { Component } from "vue";
import {
  AdjustmentsHorizontalIcon,
  RocketLaunchIcon,
  ServerIcon,
  XCircleIcon,
} from "@heroicons/vue/20/solid";
import Launch from "./GameOptions/Launch.vue";
import Updates from "./GameOptions/Updates.vue";
import Video from "./GameOptions/Video.vue";
import Storage from "./GameOptions/Storage.vue";
import { invoke } from "@tauri-apps/api/core";
import { ArrowPathIcon } from "@heroicons/vue/24/solid";
import type { GameVersion } from "~/types";

const appState = useAppState();

const open = defineModel<boolean>();
// `emulated` is supplied by the parent, which detects it robustly (console
// library membership OR a real emulator launch). The version alone is not
// enough: a ROM imported by a disk scan is Installed but has no cached
// GameVersion yet (game_versions is only filled on an online library sync), so
// `version` is undefined for it even though it is an emulated game.
const props = defineProps<{ gameId: string; emulated?: boolean }>();
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

const tabs = computed<Array<{ name: string; icon: Component; page: Component }>>(
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
  ],
);
const currentTabIndex = ref(0);

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
