<template>
  <div class="min-h-full w-full flex items-center justify-center">
    <div class="flex flex-col items-center">
      <XCircleIcon class="h-12 w-12 text-red-600" aria-hidden="true" />
      <div class="mt-3 text-center sm:mt-5">
        <h1 class="text-3xl font-semibold font-display leading-6 text-zinc-100">
          Authentication failed
        </h1>
        <div class="mt-4">
          <p class="text-sm text-zinc-400 max-w-sm">
            Drop encountered an error while connecting to your instance. Error:
            <span class="text-zinc-200">{{ message }}</span>
          </p>
        </div>
        <div class="mt-10 flex items-center justify-center gap-x-6">
          <NuxtLink href="/auth" class="text-sm font-semibold text-zinc-100"
            ><span aria-hidden="true">&larr;</span> Back to authentication
          </NuxtLink>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { XCircleIcon } from "@heroicons/vue/16/solid";

const route = useRoute();

// Sanitize the error query param: coerce to string, cap length,
// strip control chars. Mustache interpolation already escapes HTML,
// but defense-in-depth is cheap.
function sanitizeError(raw: unknown): string {
  if (raw == null) return "An unknown error occurred";
  const asString = Array.isArray(raw) ? raw.join(", ") : String(raw);
  // eslint-disable-next-line no-control-regex
  const stripped = asString.replace(/[\x00-\x1F\x7F]/g, "");
  return stripped.slice(0, 500) || "An unknown error occurred";
}

const message = sanitizeError(route.query.error);

definePageMeta({
  layout: "mini",
});
</script>
