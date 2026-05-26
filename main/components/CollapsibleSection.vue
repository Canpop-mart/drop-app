<template>
  <section
    class="rounded-xl bg-zinc-800/50 backdrop-blur-sm overflow-hidden"
  >
    <!-- Header — always visible, click toggles. Whole row is the
         affordance (not just the chevron) so the click target is
         comfortably large. -->
    <button
      type="button"
      class="w-full flex items-center justify-between gap-3 px-6 py-4 text-left hover:bg-zinc-800/30 transition-colors"
      :aria-expanded="open"
      @click="open = !open"
    >
      <div class="flex items-center gap-3 min-w-0">
        <!-- Optional left-side icon slot — useful for typographic
             accents (trophy, gallery, etc.) without forcing every
             header to use one. -->
        <span v-if="$slots.icon" class="shrink-0">
          <slot name="icon" />
        </span>
        <h2 class="text-base font-display font-semibold text-zinc-100 truncate">
          {{ title }}
        </h2>
        <span
          v-if="badge"
          class="shrink-0 text-xs text-zinc-400 tabular-nums"
        >
          {{ badge }}
        </span>
      </div>
      <ChevronDownIcon
        class="size-4 text-zinc-400 transition-transform shrink-0"
        :class="{ 'rotate-180': open }"
      />
    </button>

    <!-- Body — wrapped in a transition so expand/collapse has motion.
         We toggle with v-show rather than v-if so the child component
         keeps its state (e.g., a carousel doesn't reset to slide 1
         every time the user collapses + re-expands the gallery). -->
    <Transition name="collapse">
      <div v-show="open" class="border-t border-zinc-700/40 px-6 py-5">
        <slot />
      </div>
    </Transition>
  </section>
</template>

<script setup lang="ts">
/**
 * A disclosure card — header always visible, body expand/collapse via
 * chevron. Drop's library-detail page used to render its sections as
 * static cards inside a single tab body; turning each into a
 * collapsible gives the user control over how much vertical real
 * estate the page consumes, especially on the merged Community tab
 * (Achievements + Leaderboard + Firsts + Activity all stacked).
 *
 * Default state is controlled by `defaultOpen` (true unless told
 * otherwise). Internal `open` ref is initialised once on mount and
 * isn't synced back to the prop — the parent doesn't need to track
 * each section's expanded state.
 */
import { ChevronDownIcon } from "@heroicons/vue/24/outline";

const props = withDefaults(
  defineProps<{
    title: string;
    /** Optional small badge text after the title (e.g. "5 / 42"). */
    badge?: string;
    /** Whether the section is expanded on first render. Default open. */
    defaultOpen?: boolean;
  }>(),
  { defaultOpen: true },
);

const open = ref(props.defaultOpen);
</script>

<style scoped>
/* Slide-fade for the body. `overflow:hidden` on the outer section
   keeps the corners clean while the body transitions in/out. */
.collapse-enter-active,
.collapse-leave-active {
  transition:
    opacity 200ms ease,
    transform 200ms ease;
}
.collapse-enter-from,
.collapse-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
