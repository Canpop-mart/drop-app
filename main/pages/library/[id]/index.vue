<template>
  <!-- One delisted or unreachable game must not blank the whole library. The
       actual page (banner, tabs, install/launch controls) lives in
       <GameDetailPage>, which does the top-level `useGame(id)` await. Catching
       that throw in a local boundary here renders a "couldn't load" state for
       just this game, instead of letting it bubble up to the library-wide
       NuxtErrorBoundary in library.vue (which blanks the whole library).

       Keyed by `id` so navigating between games (if the page instance is reused
       rather than re-created) resets the boundary and re-runs the load. -->
  <NuxtErrorBoundary :key="id">
    <GameDetailPage />
    <template #error="{ error }">
      <main
        class="grid min-h-full w-full place-items-center px-6 py-24 sm:py-32 lg:px-8"
      >
        <div class="max-w-md text-center">
          <p class="text-base font-semibold text-blue-500">Error</p>
          <h1
            class="mt-4 text-3xl font-bold font-display tracking-tight text-zinc-100 sm:text-4xl"
          >
            Couldn't load this game
          </h1>
          <p class="mt-6 text-base leading-7 text-zinc-400">
            Drop couldn't load this game. It may have been removed from the
            store, or the server may be unreachable.
          </p>
          <p class="mt-2 text-sm font-monospace text-zinc-600">{{ error }}</p>
          <div class="mt-8 flex items-center justify-center gap-3">
            <button
              class="rounded-md bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-100 transition-colors hover:bg-zinc-700"
              @click="router.back()"
            >
              Go back
            </button>
            <NuxtLink
              to="/library"
              class="rounded-md px-4 py-2 text-sm font-semibold text-blue-400 transition-colors hover:text-blue-300"
            >
              Back to library
            </NuxtLink>
          </div>
        </div>
      </main>
    </template>
  </NuxtErrorBoundary>
</template>

<script setup lang="ts">
/**
 * Thin loader/wrapper for the library game-detail page.
 *
 * The page itself lives in <GameDetailPage> (components/game-detail/Page.vue),
 * which does the top-level `await useGame(id)`. Keeping that await behind a
 * local NuxtErrorBoundary means a single game that fails to load — e.g. a
 * delisted game no longer in the server library, whose metadata was never
 * cached — renders a local "couldn't load" state here rather than tripping the
 * library-wide boundary in library.vue and blanking every game.
 */
const route = useRoute();
const router = useRouter();
const id = computed(() => route.params.id.toString());
</script>
