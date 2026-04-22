<template>
  <Transition name="bpm-ra-cheat-fade">
    <div
      v-if="open"
      class="fixed inset-0 z-[200] flex items-center justify-center bg-black/80 backdrop-blur-sm"
      @click.self="$emit('close')"
    >
      <div
        class="relative w-full max-w-2xl rounded-2xl border border-zinc-700 bg-zinc-900 shadow-2xl overflow-hidden"
      >
        <div class="flex items-center justify-between px-6 py-4 border-b border-zinc-800">
          <div>
            <h2 class="text-lg font-display font-semibold text-zinc-100">
              RetroArch controller shortcuts
            </h2>
            <p class="text-xs text-zinc-500 mt-0.5">
              Hold {{ hotkeyLabel }} + the button below
            </p>
          </div>
          <button
            class="size-8 flex items-center justify-center rounded-lg text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800"
            @click="$emit('close')"
            aria-label="Close"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5">
              <path d="M6.28 5.22a.75.75 0 0 0-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 1 0 1.06 1.06L10 11.06l3.72 3.72a.75.75 0 1 0 1.06-1.06L11.06 10l3.72-3.72a.75.75 0 0 0-1.06-1.06L10 8.94 6.28 5.22Z" />
            </svg>
          </button>
        </div>

        <div class="px-6 py-5 grid grid-cols-2 gap-x-6 gap-y-4">
          <div
            v-for="combo in combos"
            :key="combo.label"
            class="flex items-center gap-3 rounded-xl bg-zinc-950/50 border border-zinc-800 px-3 py-3"
          >
            <div
              class="size-10 shrink-0 flex items-center justify-center rounded-lg text-zinc-100 text-xs font-bold"
              :class="combo.colorClass"
            >
              {{ combo.button }}
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-sm font-semibold text-zinc-100 truncate">
                {{ combo.label }}
              </div>
              <div class="text-xs text-zinc-500 truncate">
                {{ combo.description }}
              </div>
            </div>
          </div>
        </div>

        <div class="px-6 py-3 border-t border-zinc-800 text-xs text-zinc-500 flex items-center justify-between">
          <span>Keyboard: {{ keyboardLabel }}</span>
          <span class="text-zinc-600">Press B to close</span>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onUnmounted, watch } from "vue";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useFocusNavigation } from "~/composables/focus-navigation";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const focusNav = useFocusNavigation();
const gamepad = useGamepad();
let lockId = 0;
const unsubs: (() => void)[] = [];

watch(
  () => props.open,
  (v) => {
    if (v) {
      lockId = focusNav.acquireInputLock();
      wireGamepad();
    } else {
      unwireGamepad();
      focusNav.releaseInputLock(lockId);
    }
  },
);

function wireGamepad() {
  unwireGamepad();
  const bypass = { bypassInputLock: true };
  // B/East closes the cheatsheet
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!props.open) return;
      emit("close");
    }, bypass),
  );
  // A/South also closes — cheatsheet is read-only, either is fine
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!props.open) return;
      emit("close");
    }, bypass),
  );
}

function unwireGamepad() {
  for (const u of unsubs) u();
  unsubs.length = 0;
}

onUnmounted(() => {
  unwireGamepad();
  if (props.open) {
    focusNav.releaseInputLock(lockId);
  }
});

// On Linux (Steam Deck), Drop maps R3 + buttons (SDL2 button indices).
// On Windows, same combos via XInput. Labels below match what the user
// actually presses, not the underlying SDL/XInput numbers.
const hotkeyLabel = "R3 (right stick click)";

const keyboardLabel = "F2 save · F4 load · Space fast-forward · Esc quit";

const combos = computed(() => [
  {
    label: "Save state",
    description: "Snapshot current progress",
    button: "RB",
    colorClass: "bg-green-600",
  },
  {
    label: "Load state",
    description: "Restore last snapshot",
    button: "LB",
    colorClass: "bg-blue-600",
  },
  {
    label: "Fast forward",
    description: "Hold to speed up gameplay",
    button: "RT",
    colorClass: "bg-amber-600",
  },
  {
    label: "Quit",
    description: "Exit back to Drop",
    button: "Start",
    colorClass: "bg-rose-600",
  },
]);
</script>

<style scoped>
.bpm-ra-cheat-fade-enter-active,
.bpm-ra-cheat-fade-leave-active {
  transition: opacity 0.18s ease-out;
}
.bpm-ra-cheat-fade-enter-from,
.bpm-ra-cheat-fade-leave-to {
  opacity: 0;
}
</style>
