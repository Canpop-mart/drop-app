<template>
  <!-- BPM error: stay in BPM layout, don't break out to desktop -->
  <div v-if="isBigPicture" class="flex h-screen w-screen bg-zinc-950 items-center justify-center">
    <div class="max-w-lg text-center px-8">
      <p class="text-6xl font-bold text-red-500 mb-4">{{ error?.statusCode ?? "Error" }}</p>
      <h1 class="text-3xl font-bold font-display text-zinc-100 mb-3">Something went wrong</h1>
      <p v-if="message" class="text-base text-red-400 mb-4">{{ message }}</p>
      <p class="text-sm text-zinc-500 mb-8">
        Press the back button or select an option below to continue.
      </p>
      <div class="flex items-center justify-center gap-4">
        <button
          class="px-6 py-3 bg-blue-600 hover:bg-blue-500 text-white font-semibold rounded-xl transition-colors"
          @click="goBack"
        >
          Go Back
        </button>
        <button
          class="px-6 py-3 bg-zinc-800 hover:bg-zinc-700 text-zinc-200 font-semibold rounded-xl transition-colors"
          @click="goLibrary"
        >
          Library
        </button>
      </div>
    </div>
  </div>

  <!-- Desktop error: original layout -->
  <NuxtLayout v-else name="default">
    <div
      class="grid min-h-full grid-cols-1 grid-rows-[1fr,auto,1fr] lg:grid-cols-[max(50%,36rem),1fr]"
    >
      <header
        class="mx-auto w-full max-w-7xl px-6 pt-6 sm:pt-10 lg:col-span-2 lg:col-start-1 lg:row-start-1 lg:px-8"
      >
        <Logo class="h-10 w-auto sm:h-12" />
      </header>
      <main
        class="mx-auto w-full max-w-7xl px-6 py-24 sm:py-32 lg:col-span-2 lg:col-start-1 lg:row-start-2 lg:px-8"
      >
        <div class="max-w-lg">
          <p class="text-base font-semibold leading-8 text-blue-600">
            {{ error?.statusCode }}
          </p>
          <h1
            class="mt-4 text-3xl font-bold font-display tracking-tight text-zinc-100 sm:text-5xl"
          >
            Oh no!
          </h1>
          <p
            v-if="message"
            class="mt-3 font-bold text-base leading-7 text-red-500"
          >
            {{ message }}
          </p>
          <p class="mt-6 text-base leading-7 text-zinc-400">
            An error occurred while responding to your request. If you believe
            this to be a bug, please report it. Try signing in and see if it
            resolves the issue.
          </p>
          <div class="mt-10">
            <!-- full app reload to fix errors -->
            <a
              href="/store"
              class="text-sm font-semibold leading-7 text-blue-600"
              ><span aria-hidden="true">&larr;</span> Back to store</a
            >
          </div>
        </div>
      </main>
      <footer class="self-end lg:col-span-2 lg:col-start-1 lg:row-start-3">
        <div class="border-t border-zinc-700 bg-zinc-900 py-10">
          <nav
            class="mx-auto flex w-full max-w-7xl items-center gap-x-4 px-6 text-sm leading-7 text-zinc-400 lg:px-8"
          >
            <NuxtLink href="/docs">Documentation</NuxtLink>
            <svg
              viewBox="0 0 2 2"
              aria-hidden="true"
              class="h-0.5 w-0.5 fill-zinc-600"
            >
              <circle cx="1" cy="1" r="1" />
            </svg>
            <a href="https://discord.gg/NHx46XKJWA" target="_blank"
              >Support Discord</a
            >
          </nav>
        </div>
      </footer>
      <div
        class="hidden lg:relative lg:col-start-2 lg:row-start-1 lg:row-end-4 lg:block"
      >
        <img
          src="@/assets/wallpaper.jpg"
          alt=""
          class="absolute inset-0 h-full w-full object-cover"
        />
      </div>
    </div>
  </NuxtLayout>
</template>

<script setup lang="ts">
import type { NuxtError } from "#app";

const props = defineProps({
  error: Object as () => NuxtError,
});

const statusCode = props.error?.statusCode;
const message =
  props.error?.statusMessage ||
  props.error?.message ||
  "An unknown error occurred.";

// Detect if we were in Big Picture Mode when the error occurred
const isBigPicture = ref(false);

onMounted(() => {
  // Check if we came from a BPM route or if BPM is active
  const referrer = document.referrer || "";
  const currentUrl = window.location.href;
  isBigPicture.value =
    currentUrl.includes("/bigpicture") ||
    referrer.includes("/bigpicture") ||
    // Also check if the app was in fullscreen (BPM is always fullscreen)
    !!document.fullscreenElement;
});

function goBack() {
  clearError({ redirect: "/bigpicture" });
}

function goLibrary() {
  clearError({ redirect: "/bigpicture" });
}

console.error("[ERROR PAGE]", props.error);
</script>
