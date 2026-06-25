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
          <!-- Configure (game options modal) — moved here from the
               separate chevron-dropdown that used to sit next to the
               Play button.  Consolidating both menus into this gear
               keeps "all the per-game knobs" in one place. -->
          <MenuItem v-if="showConfigure" v-slot="{ active }">
            <button
              @click="$emit('configure')"
              :class="[
                active
                  ? 'bg-zinc-800 text-zinc-100 outline-none'
                  : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Configure
              <Cog6ToothIcon class="size-5 text-blue-400" />
            </button>
          </MenuItem>

          <MenuItem v-if="config.isNativeGame.value" v-slot="{ active }">
            <button
              @click="config.applyProfileName()"
              :class="[
                active
                  ? 'bg-zinc-800 text-zinc-100 outline-none'
                  : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Set Account Name
              <UserIcon class="size-5 text-green-400" />
            </button>
          </MenuItem>

          <!-- Controller layout — emulated (RetroArch) games only. -->
          <MenuItem v-if="config.isEmulatedGame.value" as="div" disabled>
            <div class="w-full px-4 py-2 text-sm text-zinc-300">
              <div class="flex justify-between items-center mb-1.5">
                <span>Controller Layout</span>
                <AdjustmentsHorizontalIcon class="size-4 text-blue-400" />
              </div>
              <div class="flex gap-1">
                <button
                  v-for="opt in CONTROLLER_OPTIONS"
                  :key="String(opt.value)"
                  class="flex-1 px-2 py-1 rounded text-xs font-medium transition-colors"
                  :class="
                    config.selectedController.value === opt.value
                      ? 'bg-blue-600 text-white'
                      : 'bg-zinc-700 text-zinc-400 hover:bg-zinc-600 hover:text-zinc-200'
                  "
                  @click.stop="config.setController(opt.value)"
                >
                  {{ opt.label }}
                </button>
              </div>
            </div>
          </MenuItem>

          <!-- Quality preset — emulated (RetroArch) games only. -->
          <MenuItem v-if="config.isEmulatedGame.value" as="div" disabled>
            <div class="w-full px-4 py-2 text-sm text-zinc-300">
              <div class="flex justify-between items-center mb-1.5">
                <span>Quality Preset</span>
                <SparklesIcon class="size-4 text-teal-400" />
              </div>
              <div class="flex gap-1">
                <button
                  v-for="opt in QUALITY_OPTIONS"
                  :key="String(opt.value)"
                  class="flex-1 px-2 py-1 rounded text-xs font-medium transition-colors"
                  :class="
                    config.selectedQuality.value === opt.value
                      ? 'bg-teal-600 text-white'
                      : 'bg-zinc-700 text-zinc-400 hover:bg-zinc-600 hover:text-zinc-200'
                  "
                  @click.stop="config.setQuality(opt.value)"
                >
                  {{ opt.label }}
                </button>
              </div>
            </div>
          </MenuItem>

          <!-- Aspect ratio cycle — emulated (RetroArch) games only. -->
          <MenuItem v-if="config.isEmulatedGame.value" as="div" disabled>
            <div class="w-full px-4 py-2 text-sm text-zinc-300">
              <div class="flex justify-between items-center">
                <span>Aspect Ratio</span>
                <button
                  class="px-2.5 py-0.5 rounded-md text-xs font-medium transition-colors"
                  :class="
                    config.aspectRatio.value !== 'Standard'
                      ? 'bg-green-600 text-white'
                      : 'bg-zinc-700 text-zinc-300'
                  "
                  @click.stop="config.toggleWidescreen()"
                >
                  {{ config.aspectLabel.value }}
                </button>
              </div>
            </div>
          </MenuItem>

          <!-- Fullscreen on launch — emulated (RetroArch) games only.
               Defaults to on; users who'd rather windowed flip this off. -->
          <MenuItem v-if="config.isEmulatedGame.value" as="div" disabled>
            <div class="w-full px-4 py-2 text-sm text-zinc-300">
              <div class="flex justify-between items-center">
                <span>Fullscreen</span>
                <button
                  class="px-2.5 py-0.5 rounded-md text-xs font-medium transition-colors"
                  :class="
                    config.fullscreen.value
                      ? 'bg-green-600 text-white'
                      : 'bg-zinc-700 text-zinc-300'
                  "
                  @click.stop="config.toggleFullscreen()"
                >
                  {{ config.fullscreen.value ? "On" : "Off" }}
                </button>
              </div>
            </div>
          </MenuItem>

          <div class="border-t border-zinc-800 my-1" />

          <MenuItem v-if="hasAchievements" v-slot="{ active }">
            <button
              @click="$emit('reset-achievements')"
              :class="[
                active
                  ? 'bg-zinc-800 text-zinc-100 outline-none'
                  : 'text-zinc-400',
                'w-full px-4 py-2 text-sm inline-flex justify-between',
              ]"
            >
              Reset Achievements
              <TrophyIcon class="size-5 text-yellow-500" />
            </button>
          </MenuItem>

          <template v-if="showInstallVcredist">
            <MenuItem v-slot="{ active }">
              <button
                @click="$emit('install-runtime', 'vcpp')"
                :class="[
                  active
                    ? 'bg-zinc-800 text-zinc-100 outline-none'
                    : 'text-zinc-400',
                  'w-full px-4 py-2 text-sm inline-flex justify-between',
                ]"
              >
                Install VC++ runtime
                <WrenchScrewdriverIcon class="size-5 text-sky-400" />
              </button>
            </MenuItem>
            <MenuItem v-slot="{ active }">
              <button
                @click="$emit('install-runtime', 'directx')"
                :class="[
                  active
                    ? 'bg-zinc-800 text-zinc-100 outline-none'
                    : 'text-zinc-400',
                  'w-full px-4 py-2 text-sm inline-flex justify-between',
                ]"
              >
                Install DirectX runtime
                <WrenchScrewdriverIcon class="size-5 text-sky-400" />
              </button>
            </MenuItem>
            <MenuItem v-slot="{ active }">
              <button
                @click="$emit('install-runtime', 'dotnet')"
                :class="[
                  active
                    ? 'bg-zinc-800 text-zinc-100 outline-none'
                    : 'text-zinc-400',
                  'w-full px-4 py-2 text-sm inline-flex justify-between',
                ]"
              >
                Install .NET runtime
                <WrenchScrewdriverIcon class="size-5 text-sky-400" />
              </button>
            </MenuItem>
          </template>

          <MenuItem v-if="showUninstall" v-slot="{ active }">
            <button
              @click="$emit('uninstall')"
              :class="[
                active
                  ? 'bg-zinc-800 text-zinc-100 outline-none'
                  : 'text-zinc-400',
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
                active
                  ? 'bg-zinc-800 text-zinc-100 outline-none'
                  : 'text-zinc-400',
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
 * The per-game options gear menu on the library detail page: emulated-game
 * presets (controller / quality / aspect), the Goldberg "Set Account Name"
 * action, and "Reset Achievements".
 *
 * Preset state + actions come from `useGameConfig`, passed in as `config`
 * so the parent owns a single instance. "Reset Achievements" is emitted
 * upward because the confirmation modal + the reset call live with the
 * achievements data on the parent.
 */
import { Menu, MenuButton, MenuItem, MenuItems } from "@headlessui/vue";
import { ArrowUturnLeftIcon, Cog6ToothIcon } from "@heroicons/vue/24/outline";
import {
  AdjustmentsHorizontalIcon,
  SparklesIcon,
  TrashIcon,
  TrophyIcon,
  UserIcon,
  WrenchScrewdriverIcon,
} from "@heroicons/vue/24/solid";
import {
  CONTROLLER_OPTIONS,
  QUALITY_OPTIONS,
  useGameConfig,
} from "~/composables/game-detail/use-game-config";

defineProps<{
  config: ReturnType<typeof useGameConfig>;
  hasAchievements: boolean;
  /**
   * Whether to show the "Configure" item — opens the per-game options
   * modal.  Only meaningful for installed games (the modal needs a
   * GameVersion).  Mirrors the precondition the old chevron-dropdown
   * on the Play button used to check inline.
   */
  showConfigure?: boolean;
  /**
   * Whether to show the "Uninstall" item.  Only meaningful for
   * installed games; was previously in GameStatusButton's chevron
   * dropdown, now consolidated into this gear menu.
   */
  showUninstall?: boolean;
  /**
   * Whether to show the "Install VC++ Runtime" item.  Only meaningful for
   * installed Windows games that launch via Proton (Linux host) — the parent
   * gates on that.  Runs winetricks against the game's prefix on demand.
   */
  showInstallVcredist?: boolean;
}>();

defineEmits<{
  (e: "configure"): void;
  (e: "uninstall"): void;
  (e: "reset-achievements"): void;
  (e: "remove-from-library"): void;
  (e: "install-runtime", set: string): void;
}>();
</script>
