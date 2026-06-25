<template>
  <CollapsibleSection v-if="support" title="Controller support">
    <div class="flex flex-col items-center gap-3 py-1 text-center">
      <!-- Gamepad fills fully for Full support, half for Partial. -->
      <span class="relative inline-block size-10">
        <GamepadIcon class="absolute inset-0 size-10 text-zinc-700" />
        <GamepadIcon
          class="absolute inset-0 size-10"
          :class="support === 'Full' ? 'text-green-400' : 'text-amber-400'"
          :style="support === 'Full' ? '' : 'clip-path: inset(0 50% 0 0)'"
        />
      </span>
      <span
        class="rounded-full px-3 py-1 text-xs font-bold uppercase tracking-wide"
        :class="
          support === 'Full'
            ? 'bg-green-500/15 text-green-400 ring-1 ring-green-500/30'
            : 'bg-amber-500/15 text-amber-400 ring-1 ring-amber-500/30'
        "
      >
        {{ support === "Full" ? "Full support" : "Partial support" }}
      </span>
      <p class="max-w-sm text-sm text-zinc-400">{{ description }}</p>
    </div>
  </CollapsibleSection>
</template>

<script setup lang="ts">
import CollapsibleSection from "~/components/CollapsibleSection.vue";
import GamepadIcon from "~/components/Icons/GamepadIcon.vue";
import type { Game } from "~/types";

const props = defineProps<{ game: Game; emulated?: boolean }>();

// Emulated games always play with a controller through the emulator, so they
// report full support regardless of the (Steam-derived) metadata field.
const support = computed(() => {
  if (props.emulated) return "Full";
  const c = props.game.mControllerSupport;
  return c === "Full" || c === "Partial" ? c : null;
});

const description = computed(() => {
  if (props.emulated) return "Plays with a controller through the emulator.";
  return support.value === "Full"
    ? "Plays great with a controller, no setup needed."
    : "Has partial controller support; some menus may still want a keyboard or mouse.";
});
</script>
