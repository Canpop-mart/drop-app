<template>
  <!-- Game options gear — pinned top-left above the banner. -->
  <Menu as="div" class="absolute top-3 left-4 z-20">
    <MenuButton
      class="rounded-lg p-1.5 bg-zinc-800/50 text-zinc-100 hover:bg-zinc-800 transition-colors"
    >
      <Cog6ToothIcon class="size-5" />
    </MenuButton>
    <Transition
      enter-active-class="transition ease-out duration-100"
      enter-from-class="opacity-0 scale-95"
      enter-to-class="opacity-100 scale-100"
      leave-active-class="transition ease-in duration-75"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-95"
    >
      <MenuItems
        class="absolute left-0 z-[500] mt-2 w-56 origin-top-left rounded-lg bg-zinc-900 shadow-lg ring-1 ring-zinc-100/5 focus:outline-none overflow-hidden"
      >
        <div class="py-1">
          <!--
            Actions only. Persistent settings (controller/quality/aspect/
            fullscreen/CRT) moved into the Configure modal's "Video & Controls"
            tab — mixing multi-state toggles into this action list was the source
            of the clutter and the clipped Quality/controller rows.
          -->
          <MenuItem v-if="showConfigure" v-slot="{ active }">
            <button
              @click="$emit('configure')"
              :class="[
                active ? 'bg-zinc-800 text-zinc-100 outline-none' : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Configure
              <Cog6ToothIcon class="size-5 text-blue-400" />
            </button>
          </MenuItem>

          <MenuItem v-if="showAccountName" v-slot="{ active }">
            <button
              @click="$emit('set-account-name')"
              :class="[
                active ? 'bg-zinc-800 text-zinc-100 outline-none' : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Set Account Name
              <UserIcon class="size-5 text-green-400" />
            </button>
          </MenuItem>

          <MenuItem v-if="showOpenFolder" v-slot="{ active }">
            <button
              @click="$emit('open-install-folder')"
              :class="[
                active ? 'bg-zinc-800 text-zinc-100 outline-none' : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Open install folder
              <FolderOpenIcon class="size-5 text-amber-400" />
            </button>
          </MenuItem>

          <template v-if="showInstallVcredist">
            <MenuItem
              v-for="rt in RUNTIMES"
              :key="rt.set"
              v-slot="{ active }"
            >
              <button
                @click="$emit('install-runtime', rt.set)"
                :class="[
                  active ? 'bg-zinc-800 text-zinc-100 outline-none' : 'text-zinc-400',
                  'w-full px-4 py-2 text-sm inline-flex justify-between',
                ]"
              >
                {{ rt.label }}
                <WrenchScrewdriverIcon class="size-5 text-sky-400" />
              </button>
            </MenuItem>
          </template>

          <!-- Divider only when there's a group above it to divide from. -->
          <div v-if="hasSetupGroup" class="border-t border-zinc-800 my-1" />

          <MenuItem v-if="hasAchievements" v-slot="{ active }">
            <button
              @click="$emit('reset-achievements')"
              :class="[
                active ? 'bg-zinc-800 text-zinc-100 outline-none' : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Reset Achievements
              <TrophyIcon class="size-5 text-yellow-500" />
            </button>
          </MenuItem>

          <MenuItem v-if="showUninstall" v-slot="{ active }">
            <button
              @click="$emit('uninstall')"
              :class="[
                active ? 'bg-zinc-800 text-zinc-100 outline-none' : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Uninstall
              <ArrowUturnLeftIcon class="size-5 text-orange-400" />
            </button>
          </MenuItem>

          <MenuItem v-slot="{ active }">
            <button
              @click="$emit('remove-from-library')"
              :class="[
                active ? 'bg-zinc-800 text-zinc-100 outline-none' : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Remove from Library
              <TrashIcon class="size-5 text-red-500" />
            </button>
          </MenuItem>
        </div>
      </MenuItems>
    </Transition>
  </Menu>
</template>

<script setup lang="ts">
/**
 * The per-game options gear on the library detail page. Pure action list:
 * Configure (opens the settings modal), Set Account Name, Open install folder,
 * runtime installs, and the destructive actions. All persistent settings live
 * in the Configure modal now, so this component holds no config state — every
 * item is a prop-gated emit.
 *
 * The parent owns the gates because whether an item applies depends on install
 * status and game type, which the parent already computes.
 */
import { Menu, MenuButton, MenuItem, MenuItems } from "@headlessui/vue";
import { ArrowUturnLeftIcon, Cog6ToothIcon } from "@heroicons/vue/24/outline";
import {
  FolderOpenIcon,
  TrashIcon,
  TrophyIcon,
  UserIcon,
  WrenchScrewdriverIcon,
} from "@heroicons/vue/24/solid";

const RUNTIMES: { set: string; label: string }[] = [
  { set: "vcpp", label: "Install VC++ runtime" },
  { set: "directx", label: "Install DirectX runtime" },
  { set: "dotnet", label: "Install .NET runtime" },
];

const props = defineProps<{
  hasAchievements: boolean;
  /** Opens the per-game options modal. Installed, non-partial games only. */
  showConfigure?: boolean;
  /** Goldberg "Set Account Name". Native games that are actually installed —
   *  the action writes into the install dir, so it's meaningless otherwise. */
  showAccountName?: boolean;
  /** Reveal the install directory in the OS file manager. Installed only. */
  showOpenFolder?: boolean;
  /** Uninstall. Installed games only. */
  showUninstall?: boolean;
  /** Install VC++/DirectX/.NET into the Proton prefix. Windows-on-Linux only. */
  showInstallVcredist?: boolean;
}>();

// The divider sits between the setup group and the destructive group. Only
// render it when the setup group actually has something in it (e.g. an
// uninstalled game shows just Reset Achievements + Remove, no divider).
const hasSetupGroup = computed(
  () =>
    !!props.showConfigure ||
    !!props.showAccountName ||
    !!props.showOpenFolder ||
    !!props.showInstallVcredist,
);

defineEmits<{
  (e: "configure"): void;
  (e: "set-account-name"): void;
  (e: "open-install-folder"): void;
  (e: "uninstall"): void;
  (e: "reset-achievements"): void;
  (e: "remove-from-library"): void;
  (e: "install-runtime", set: string): void;
}>();
</script>
