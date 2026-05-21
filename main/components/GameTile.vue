<template>
  <button
    class="group flex flex-col text-left transition-transform duration-200 hover:-translate-y-1"
    @click="$emit('select')"
  >
    <div
      class="relative aspect-[3/4] overflow-hidden bg-zinc-800 ring-1 ring-zinc-700/50 group-hover:ring-blue-500/50 transition-colors"
      :class="rounded === 'lg' ? 'rounded-lg' : 'rounded-xl'"
    >
      <img
        v-if="coverUrl"
        :src="coverUrl"
        :alt="name"
        class="w-full h-full object-cover"
        loading="lazy"
      />
      <div
        v-else
        class="w-full h-full flex items-center justify-center text-zinc-600 text-xs px-2 text-center"
      >
        {{ name }}
      </div>

      <!-- "ROM" marker — top-left, for emulated games. -->
      <div
        v-if="rom"
        class="absolute top-2 left-2 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase bg-purple-500/30 text-purple-200 backdrop-blur-sm"
      >
        ROM
      </div>

      <!-- Badge stack — top-right. "Update" trumps the plain installed dot. -->
      <div class="absolute top-2 right-2 flex flex-col gap-1 items-end">
        <span
          v-if="updateAvailable"
          class="px-1.5 py-0.5 rounded text-[10px] font-bold uppercase bg-blue-500/30 text-blue-200 backdrop-blur-sm"
        >
          Update
        </span>
        <span
          v-else-if="installed"
          class="size-2 rounded-full bg-green-500 shadow-lg shadow-green-500/50"
          title="Installed"
        />
      </div>

      <!-- Hover gradient so the cover doesn't look static under the cursor. -->
      <div
        class="absolute inset-0 bg-gradient-to-t from-zinc-950/80 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity"
      />
    </div>
    <p
      class="mt-2 text-xs font-medium text-zinc-300 truncate group-hover:text-zinc-100 transition-colors"
    >
      {{ name }}
    </p>
  </button>
</template>

<script setup lang="ts">
/**
 * Single store/library game tile — the one canonical "box-art card".
 *
 * Before this component the same ~45 lines of cover-art markup were
 * copy-pasted across `LibraryGrid`, `StoreShelf`, the store Browse grid,
 * `pages/library/collections.vue`, and the profile showcase. They drifted
 * (ROM badge here, update badge there) and every styling tweak meant five
 * edits. This consolidates them.
 *
 * Deliberately presentational: it takes an already-resolved `coverUrl`
 * rather than an object ID, because callers resolve through two different
 * protocols — `useObject()` for native `object://` IDs, and
 * `serverUrl("api/v1/object/...")` for store metadata. Resolving here
 * would force one strategy on every caller.
 */
defineProps<{
  /** Fully-resolved cover image URL, or empty/undefined for the fallback. */
  coverUrl?: string | null;
  /** Display name — also the fallback text when there's no cover. */
  name: string;
  /** Show the green "installed" dot. */
  installed?: boolean;
  /** Show the blue "Update" pill (takes precedence over the dot). */
  updateAvailable?: boolean;
  /** Show the purple "ROM" pill for emulated games. */
  rom?: boolean;
  /** Corner radius — `xl` (default) for the main grids, `lg` for denser ones. */
  rounded?: "lg" | "xl";
}>();

defineEmits<{
  (e: "select"): void;
}>();
</script>
