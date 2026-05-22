<template>
  <div class="min-h-full w-full flex items-center justify-center">
    <div class="flex flex-col items-center">
      <div class="text-center">
        <h1 class="text-3xl font-semibold font-display leading-6 text-zinc-100">
          Device authorization
        </h1>
        <div v-if="code" class="mt-4">
          <p class="text-sm text-zinc-400 max-w-md mx-auto">
            Open Drop on another one of your devices, and use your account
            dropdown to "Authorize client", and enter the code below.
          </p>
          <div
            class="mt-8 flex items-center justify-center gap-x-5 text-8xl font-bold text-zinc-100"
          >
            <span v-for="letter in code.split('')">{{ letter }}</span>
          </div>
        </div>
        <div v-else-if="error" class="mt-6 max-w-md mx-auto">
          <div class="rounded-md bg-red-600/10 p-4 text-left">
            <div class="flex">
              <XCircleIcon
                class="h-5 w-5 shrink-0 text-red-600"
                aria-hidden="true"
              />
              <div class="ml-3">
                <h3 class="text-sm font-medium text-red-600">
                  Couldn't request a code from the server
                </h3>
                <p class="mt-1 text-xs text-red-400 break-words">{{ error }}</p>
              </div>
            </div>
          </div>
        </div>
        <div class="mt-10 flex items-center justify-center gap-x-6">
          <NuxtLink href="/auth" class="text-sm font-semibold text-blue-600"
            ><span aria-hidden="true">&larr;</span> Use a different method
          </NuxtLink>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { XCircleIcon } from "@heroicons/vue/16/solid";
import { invoke } from "@tauri-apps/api/core";

// `auth_initiate_code` hits the server's handshake endpoint, which can fail
// for any number of reasons (version-skew capability mismatch, server
// unreachable, expired session). Catch it here so the page renders the
// error inline instead of silently blanking when setup throws.
const code = ref<string | undefined>();
const error = ref<string | undefined>();

try {
  code.value = await invoke<string>("auth_initiate_code");
} catch (e) {
  error.value = String(e);
}

definePageMeta({
  layout: "mini",
});
</script>
