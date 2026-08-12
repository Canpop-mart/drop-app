<template>
  <ModalTemplate :model-value="true">
    <template #default
      ><div class="flex items-start gap-x-3">
        <img
          v-if="iconObjectId"
          :src="objectImageUrl(iconObjectId)"
          class="size-12"
        />
        <div class="mt-3 text-center sm:mt-0 sm:text-left">
          <h3 class="text-base font-semibold text-zinc-100">
            {{ displayName }} isn't installed
          </h3>
          <div class="mt-2">
            <p class="text-sm text-zinc-400">
              This game runs through {{ displayName }}, so it can't start until
              {{ displayName }} is installed. Install it below and press Play
              again.
            </p>
            <p v-if="versionLabel" class="mt-1 text-xs text-zinc-500">
              Version: {{ versionLabel }}
            </p>
          </div>
        </div>
      </div>

      <InstallDirectorySelector
        v-if="installDirs.length > 0"
        :install-dirs="installDirs"
        v-model="installDir"
      />

      <div v-if="loadError" class="mt-1 rounded-md bg-amber-600/10 p-4">
        <div class="flex">
          <div class="flex-shrink-0">
            <XCircleIcon class="h-5 w-5 text-amber-500" aria-hidden="true" />
          </div>
          <div class="ml-3">
            <h3 class="text-sm font-medium text-amber-500">
              {{ loadError }}
            </h3>
          </div>
        </div>
      </div>

      <div v-if="installError" class="mt-1 rounded-md bg-red-600/10 p-4">
        <div class="flex">
          <div class="flex-shrink-0">
            <XCircleIcon class="h-5 w-5 text-red-600" aria-hidden="true" />
          </div>
          <div class="ml-3">
            <h3 class="text-sm font-medium text-red-600">
              {{ installError }}
            </h3>
          </div>
        </div>
      </div>
    </template>
    <template #buttons>
      <LoadingButton
        @click="() => install()"
        :loading="installLoading"
        :disabled="installLoading || !canInstall"
        type="submit"
        class="ml-2 w-full sm:w-fit"
      >
        Install
      </LoadingButton>
      <button
        type="button"
        class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
        @click="cancel"
        ref="cancelButtonRef"
      >
        Cancel
      </button>
    </template>
  </ModalTemplate>
</template>

<script setup lang="ts">
/**
 * "The emulator this game needs isn't installed" modal.
 *
 * Setup is deliberately synchronous. It used to `await useGame()` and
 * `await invoke("fetch_game_version_options")` at the top level and then
 * dereference the matching version with a `!`. All three can fail for exactly
 * the game this modal is about — a dependency that is not installed is also
 * the one most likely to be missing from the local cache — and because an
 * async-setup component's rejection is swallowed by the page's <Suspense>, the
 * failure rendered as nothing at all. Pressing Play with no emulator installed
 * looked like the button did nothing. (Same trap that
 * `GameOptionsModal.vue` documents.)
 *
 * Everything fetched here is therefore optional: the modal always renders,
 * naming the dependency from the backend's own payload, and degrades to a
 * message instead of disappearing when a lookup fails.
 */
import { invoke } from "@tauri-apps/api/core";
import { XCircleIcon } from "@heroicons/vue/24/solid";
import { objectImageUrl } from "~/composables/use-object";

const model = defineModel<{
  gameId: string;
  versionId: string;
  name: string | null;
}>({ required: true });

const dependency = { ...model.value };

const cachedName = ref<string | null>(null);
const iconObjectId = ref<string | null>(null);
const versionLabel = ref<string | null>(null);
const installPlatform = ref<string | null>(null);
const installDirs = ref<string[]>([]);
const installDir = ref(0);
const loadError = ref<string | undefined>();
const installError = ref<string | undefined>();
const installLoading = ref(false);

// The backend's name is authoritative; the library lookup is only there to
// upgrade the id fallback and supply an icon.
const displayName = computed(
  () => dependency.name ?? cachedName.value ?? "The required program",
);

// Installing needs a platform and a target directory. Without either, the
// button would fire an invoke that fails for reasons the user can't act on.
const canInstall = computed(
  () => !!installPlatform.value && installDirs.value.length > 0,
);

async function loadDetails() {
  try {
    const { game } = await useGame(dependency.gameId);
    cachedName.value = game.mName ?? null;
    iconObjectId.value = game.mIconObjectId ?? null;
  } catch (e) {
    console.warn("[DEPENDENCY] Could not load library entry:", e);
  }

  try {
    const versionOptions = await invoke<Array<VersionOption>>(
      "fetch_game_version_options",
      { gameId: dependency.gameId },
    );
    // Prefer the version the launch asked for, but a re-versioned emulator
    // orphans that id — falling back to the first option is what makes the
    // Install button work at all in that case.
    const version =
      versionOptions.find((v) => v.versionId === dependency.versionId) ??
      versionOptions[0];
    if (version) {
      dependency.versionId = version.versionId;
      versionLabel.value = version.displayName ?? version.versionPath;
      installPlatform.value = version.platform;
    } else {
      loadError.value = `${displayName.value} has no downloadable versions on your Drop server.`;
    }
  } catch (e) {
    loadError.value = `Couldn't load versions for ${displayName.value}: ${e instanceof Error ? e.message : String(e)}`;
  }

  try {
    installDirs.value = await invoke<string[]>("fetch_download_dir_stats");
  } catch (e) {
    loadError.value = `Couldn't read your install directories: ${e instanceof Error ? e.message : String(e)}`;
  }
}
loadDetails();

function cancel() {
  // @ts-expect-error
  model.value = undefined;
}

async function install() {
  if (!canInstall.value) return;
  try {
    installLoading.value = true;
    installError.value = undefined;
    await invoke("download_game", {
      gameId: dependency.gameId,
      versionId: dependency.versionId,
      installDir: installDir.value,
      targetPlatform: installPlatform.value,
    });
    cancel();
  } catch (error) {
    installError.value = `Install failed: ${error instanceof Error ? error.message : String(error)}`;
  }

  installLoading.value = false;
}
</script>
