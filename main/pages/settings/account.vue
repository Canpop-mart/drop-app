<template>
  <div>
    <div class="border-b border-zinc-700 py-5">
      <h3 class="text-base font-semibold font-display leading-6 text-zinc-100">
        General
      </h3>
    </div>

    <div class="mt-5 flex flex-col gap-8">
      <!-- Display name -->
      <div>
        <label for="displayName" class="block text-sm font-medium text-zinc-100">
          Display name
        </label>
        <div class="mt-2 flex items-center gap-3 max-w-md">
          <input
            id="displayName"
            v-model="displayName"
            type="text"
            maxlength="64"
            :placeholder="accountName || 'Your name'"
            class="block w-full rounded-md border-0 py-1.5 text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 bg-zinc-800 placeholder:text-zinc-500 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:text-sm sm:leading-6"
            @keyup.enter="saveName"
          />
          <button
            type="button"
            :disabled="nameSaving"
            :class="[
              'inline-flex shrink-0 items-center rounded-md px-3 py-2 text-sm font-semibold text-white shadow-sm transition-colors duration-300 disabled:cursor-not-allowed',
              nameSaved
                ? 'bg-green-600 hover:bg-green-500'
                : 'bg-blue-600 hover:bg-blue-500 disabled:bg-blue-600/50',
            ]"
            @click="saveName"
          >
            {{ nameSaved ? "Saved" : "Save" }}
          </button>
        </div>
        <p class="mt-2 text-sm text-zinc-400">
          Shown to others in multiplayer (co-op and Archipelago). Leave blank to
          use your account name.
        </p>
      </div>

      <!-- Sign out -->
      <div class="flex flex-row items-center justify-between">
        <div>
          <h3 class="text-sm font-medium leading-6 text-zinc-100">Sign out</h3>
          <p class="mt-1 text-sm leading-6 text-zinc-400">
            Sign out of your Drop account on this device
          </p>
        </div>
        <button
          @click="signOut"
          type="button"
          class="rounded-md bg-red-600 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-red-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-red-600"
        >
          Sign out
        </button>
      </div>

      <div v-if="error" class="rounded-md bg-red-600/10 p-4">
        <div class="flex">
          <div class="flex-shrink-0">
            <XCircleIcon class="h-5 w-5 text-red-600" aria-hidden="true" />
          </div>
          <div class="ml-3">
            <h3 class="text-sm font-medium text-red-600">
              {{ error }}
            </h3>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useListen } from "~/composables/useListen";
import { useDisplayName } from "~/composables/use-display-name";
import { useRouter } from "#imports";
import { XCircleIcon } from "@heroicons/vue/16/solid";
import { type Settings } from "~/types";

const router = useRouter();
const error = ref<string | null>(null);

// ── Display name ────────────────────────────────────────────────────────────

const { setName, accountName: getAccountName } = useDisplayName();
const accountName = computed(() => getAccountName());

const settings = await invoke<Settings>("fetch_settings");
const displayName = ref<string>(settings.displayName ?? "");
const nameSaving = ref(false);
const nameSaved = ref(false);

async function saveName() {
  nameSaving.value = true;
  error.value = null;
  try {
    await setName(displayName.value);
    nameSaved.value = true;
    setTimeout(() => {
      nameSaved.value = false;
    }, 2000);
  } catch (e) {
    error.value = `Failed to save display name: ${e}`;
  } finally {
    nameSaving.value = false;
  }
}

// ── Sign out ──────────────────────────────────────────────────────────────

useListen("auth/signedout", () => {
  router.push("/auth/signedout");
});

async function signOut() {
  try {
    error.value = null;
    await invoke("sign_out");
  } catch (e) {
    error.value = `Failed to sign out: ${e}`;
  }
}
</script>
