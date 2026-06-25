<template>
  <Teleport to="body">
    <div
      v-if="install.installFlowOpen.value"
      class="fixed inset-0 z-[10000] overflow-y-auto bg-zinc-950/75"
      @click.self="install.installFlowOpen.value = false"
    >
      <div
        class="flex min-h-full items-start justify-center p-4 text-center sm:items-center sm:p-0"
        @click.self="install.installFlowOpen.value = false"
      >
        <div
          role="dialog"
          aria-modal="true"
          class="relative w-full rounded-lg bg-zinc-900 text-left shadow-xl sm:my-8 sm:max-w-lg"
        >
          <div class="space-y-4 px-4 pb-4 pt-5 sm:p-6 sm:pb-4">
      <div class="sm:flex sm:items-start">
        <div class="mt-3 text-center sm:mt-0 sm:text-left">
          <h3 class="text-base font-semibold text-zinc-100">
            Install {{ game.mName }}?
          </h3>
          <div class="mt-2">
            <p class="text-sm text-zinc-400">
              Drop will add {{ game.mName }} to the queue to be downloaded.
              While downloading, Drop may use up a large amount of resources,
              particularly network bandwidth and CPU utilisation.
            </p>
          </div>
        </div>
      </div>

      <div class="space-y-6">
        <!-- Compatibility test results — only renders when this game has
             been tested on at least one of the user's devices. -->
        <GameCompatPanel :compat="gameCompat" :game-id="game.id" />

        <div
          v-if="install.versionOptions.value && install.versionOptions.value.length > 0"
        >
          <Listbox as="div" v-model="install.installVersionIndex.value">
            <ListboxLabel class="block text-sm/6 font-medium text-zinc-100">
              Version
            </ListboxLabel>
            <div class="relative mt-2">
              <ListboxButton
                class="relative w-full cursor-default rounded-md bg-zinc-800 py-1.5 pl-3 pr-10 text-left text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-600 sm:text-sm/6"
              >
                <span class="block truncate">{{
                  install.formatVersionOptionText(install.installVersionIndex.value)
                }}</span>
                <span
                  class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2"
                >
                  <ChevronUpDownIcon
                    class="h-5 w-5 text-gray-400"
                    aria-hidden="true"
                  />
                </span>
              </ListboxButton>

              <div
                v-if="install.installVersionIndex.value == -1"
                class="mt-3 rounded-md bg-blue-500/10 p-2 outline outline-blue-500/20"
              >
                <div class="flex">
                  <div class="shrink-0">
                    <InformationCircleIcon
                      class="size-4 text-blue-400"
                      aria-hidden="true"
                    />
                  </div>
                  <div class="ml-2 flex-1 md:flex md:justify-between">
                    <p class="text-xs text-blue-300">
                      "Latest" will notify you when there is a new version
                      available. Choose another version to pin this game's
                      version.
                    </p>
                  </div>
                </div>
              </div>
              <div
                v-else
                class="mt-3 rounded-md bg-blue-500/10 p-2 outline outline-blue-500/20"
              >
                <div class="flex">
                  <div class="shrink-0">
                    <InformationCircleIcon
                      class="size-4 text-blue-400"
                      aria-hidden="true"
                    />
                  </div>
                  <div class="ml-2 flex-1 md:flex md:justify-between">
                    <p class="text-xs text-blue-300">
                      This game will be pinned to "{{
                        install.currentVersionOption.value?.displayName ||
                        install.currentVersionOption.value?.versionPath
                      }}"
                    </p>
                  </div>
                </div>
              </div>

              <transition
                leave-active-class="transition ease-in duration-100"
                leave-from-class="opacity-100"
                leave-to-class="opacity-0"
              >
                <ListboxOptions
                  class="absolute z-10 mt-1 max-h-60 w-full overflow-auto rounded-md bg-zinc-900 py-1 text-base shadow-lg ring-1 ring-black ring-opacity-5 focus:outline-none sm:text-sm"
                >
                  <ListboxOption
                    as="template"
                    :value="-1"
                    v-slot="{ active, selected }"
                  >
                    <li
                      :class="[
                        active ? 'bg-blue-600 text-white' : 'text-zinc-300',
                        'relative cursor-default select-none py-2 pl-3 pr-9',
                      ]"
                    >
                      <span
                        :class="[
                          selected
                            ? 'font-semibold text-zinc-100'
                            : 'font-normal',
                          'block truncate',
                        ]"
                        >{{ install.formatVersionOptionText(-1) }}</span
                      >
                      <span
                        v-if="selected"
                        :class="[
                          active ? 'text-white' : 'text-blue-600',
                          'absolute inset-y-0 right-0 flex items-center pr-4',
                        ]"
                      >
                        <CheckIcon class="h-5 w-5" aria-hidden="true" />
                      </span>
                    </li>
                  </ListboxOption>

                  <ListboxOption
                    as="template"
                    v-for="(versionOption, versionIdx) in install.versionOptions.value"
                    :key="versionOption.versionId"
                    :value="versionIdx"
                    v-slot="{ active, selected }"
                  >
                    <li
                      :class="[
                        active ? 'bg-blue-600 text-white' : 'text-zinc-300',
                        'relative cursor-default select-none py-2 pl-3 pr-9',
                      ]"
                    >
                      <span
                        :class="[
                          selected
                            ? 'font-semibold text-zinc-100'
                            : 'font-normal',
                          'block truncate',
                        ]"
                        >{{ install.formatVersionOptionText(versionIdx) }}</span
                      >
                      <span
                        v-if="selected"
                        :class="[
                          active ? 'text-white' : 'text-blue-600',
                          'absolute inset-y-0 right-0 flex items-center pr-4',
                        ]"
                      >
                        <CheckIcon class="h-5 w-5" aria-hidden="true" />
                      </span>
                    </li>
                  </ListboxOption>
                </ListboxOptions>
              </transition>
            </div>
          </Listbox>
        </div>
        <div
          v-else-if="
            install.versionOptions.value === null ||
            install.versionOptions.value?.length == 0
          "
          class="mt-1 rounded-md bg-red-600/10 p-4"
        >
          <div class="flex">
            <div class="flex-shrink-0">
              <XCircleIcon class="h-5 w-5 text-red-600" aria-hidden="true" />
            </div>
            <div class="ml-3">
              <h3 class="text-sm font-medium text-red-600">
                There are no supported versions to install. Please contact your
                server admin or try again later.
              </h3>
            </div>
          </div>
        </div>
        <div v-else class="w-full flex items-center justify-center p-4">
          <div role="status">
            <svg
              aria-hidden="true"
              class="w-7 h-7 text-transparent animate-spin fill-white"
              viewBox="0 0 100 101"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z"
                fill="currentColor"
              />
              <path
                d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z"
                fill="currentFill"
              />
            </svg>
            <span class="sr-only">Loading...</span>
          </div>
        </div>
        <div v-if="install.installDirs.value">
          <InstallDirectorySelector
            :install-dirs="install.installDirs.value"
            v-model="install.installDir.value"
          />
        </div>
        <div
          v-if="
            install.currentVersionOption.value?.requiredContent &&
            install.currentVersionOption.value.requiredContent.length > 0
          "
        >
          <div class="border-b border-white/10 py-2">
            <h3 class="text-sm font-semibold text-white">
              Install additional dependencies?
            </h3>
            <p class="mt-1 text-xs text-gray-400">
              This game requires additional content to run. Click the components
              to automatically queue for download.
            </p>
          </div>
          <ul role="list" class="mt-2 divide-y divide-white/5">
            <li
              v-for="content in install.currentVersionOption.value.requiredContent"
              :key="content.versionId"
              :class="[
                !install.installDepsDisabled.value[content.versionId]
                  ? 'bg-zinc-950 ring-2 ring-zinc-800'
                  : '',
                'rounded-lg relative flex justify-between px-2 py-3',
              ]"
            >
              <div class="flex min-w-0 gap-x-2">
                <img
                  class="size-12 flex-none"
                  :src="useObject(content.iconObjectId)"
                  alt=""
                />
                <div class="min-w-0 flex-auto">
                  <p class="text-sm/6 font-semibold text-white">
                    <button
                      @click="
                        install.installDepsDisabled.value[content.versionId] =
                          !install.installDepsDisabled.value[content.versionId]
                      "
                    >
                      <span class="absolute inset-x-0 -top-px bottom-0" />
                      {{ content.name }}
                    </button>
                  </p>
                  <p class="mt-1 flex text-xs/5 text-gray-400">
                    {{ content.shortDescription }}
                  </p>
                </div>
              </div>
              <div class="flex shrink-0 items-center gap-x-2">
                <div class="hidden sm:flex sm:flex-col sm:items-end">
                  <p
                    class="inline-flex items-center gap-x-1 text-xs/5 text-gray-400"
                  >
                    {{ formatKilobytes(content.size.installSize / 1024) }}B
                    <ServerIcon class="size-3" />
                  </p>
                </div>
                <CheckIcon
                  v-if="!install.installDepsDisabled.value[content.versionId]"
                  class="size-5 flex-none text-green-500"
                  aria-hidden="true"
                />
                <MinusIcon
                  v-else
                  class="size-5 flex-none text-gray-500"
                  aria-hidden="true"
                />
              </div>
            </li>
          </ul>
        </div>
      </div>

      <div
        v-if="install.installError.value"
        class="mt-1 rounded-md bg-red-600/10 p-4"
      >
        <div class="flex">
          <div class="flex-shrink-0">
            <XCircleIcon class="h-5 w-5 text-red-600" aria-hidden="true" />
          </div>
          <div class="ml-3">
            <h3 class="text-sm font-medium text-red-600">
              {{ install.installError.value }}
            </h3>
          </div>
        </div>
      </div>
          </div>
          <div
            class="rounded-b-lg bg-zinc-800 px-4 py-3 sm:flex sm:flex-row-reverse sm:gap-x-2 sm:px-6"
          >
      <LoadingButton
        @click="install.install()"
        :disabled="
          !(install.versionOptions.value && install.versionOptions.value.length > 0)
        "
        :loading="install.installLoading.value"
        type="submit"
        class="ml-2 w-full sm:w-fit"
      >
        Install
      </LoadingButton>
      <button
        type="button"
        class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
        @click="install.installFlowOpen.value = false"
      >
        Cancel
      </button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
/**
 * The "Install {game}?" modal for the library game-detail page — version
 * picker, install-directory selector, optional-dependency toggles, and the
 * compat panel. All state + the `download_game` calls come from a
 * `useGameInstall` instance, passed in as `install` so the parent owns it.
 *
 * The shell is a plain `<Teleport to="body">` + `v-if` overlay, not the
 * shared Headless UI `ModalTemplate`: that `Dialog` mounted under
 * `tauri dev` but never appeared in the packaged WebView2 build, so this
 * modal owns its own backdrop, z-index, and Escape / click-outside close.
 */
import {
  Listbox,
  ListboxButton,
  ListboxLabel,
  ListboxOption,
  ListboxOptions,
} from "@headlessui/vue";
import {
  CheckIcon,
  ChevronUpDownIcon,
  InformationCircleIcon,
} from "@heroicons/vue/20/solid";
import { MinusIcon, ServerIcon, XCircleIcon } from "@heroicons/vue/24/solid";
import type { Game } from "~/types";
import type { GameCompatSummary } from "~/composables/use-compat-summary";
import type { useGameInstall } from "~/composables/game-detail/use-game-install";

const props = defineProps<{
  game: Game;
  install: ReturnType<typeof useGameInstall>;
  gameCompat: GameCompatSummary | undefined;
}>();

/** Close on Escape — this modal is a plain teleported overlay (no Headless
 *  UI Dialog), so it carries its own keyboard handling. */
function onModalKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && props.install.installFlowOpen.value) {
    props.install.installFlowOpen.value = false;
  }
}

onMounted(() => window.addEventListener("keydown", onModalKeydown));
onUnmounted(() => window.removeEventListener("keydown", onModalKeydown));
</script>
