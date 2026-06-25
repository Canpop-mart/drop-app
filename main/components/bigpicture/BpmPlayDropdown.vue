<template>
  <div class="relative inline-flex">
    <!-- Main action button -->
    <button
      :ref="(el: any) => registerAction?.(el, { onSelect: executeAction, onContext: toggleMenu })"
      class="inline-flex items-center pl-8 py-4 text-lg gap-3 font-semibold rounded-l-xl transition-all shadow-lg"
      :class="buttonClass"
      @click="executeAction"
    >
      <PlayIcon v-if="selectedMode === 'play'" class="size-6" />
      <SignalIcon v-else class="size-6" />
      {{ actionLabel }}
    </button>

    <!-- Dropdown toggle -->
    <button
      :ref="(el: any) => registerAction?.(el, { onSelect: toggleMenu })"
      class="inline-flex items-center px-3 py-4 font-semibold rounded-r-xl transition-all shadow-lg border-l"
      :class="chevronClass"
      @click.stop="toggleMenu"
    >
      <ChevronDownIcon class="size-5" :class="{ 'rotate-180': menuOpen }" />
    </button>

    <!-- Dropdown menu -->
    <Transition name="dropdown">
      <div
        v-if="menuOpen"
        class="absolute left-0 top-full mt-2 z-50 min-w-full rounded-xl bg-zinc-900 border border-zinc-700/50 shadow-2xl overflow-hidden"
      >
        <button
          class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
          :class="[
            focusedIndex === 0 ? 'bg-blue-600 text-white' : 'text-zinc-300 hover:bg-zinc-800',
            selectedMode === 'play' && focusedIndex !== 0 ? 'text-blue-400' : '',
          ]"
          @click="selectOption('play')"
          @mouseenter="focusedIndex = 0"
        >
          <PlayIcon class="size-5" />
          <span class="font-medium">Play</span>
        </button>
        <button
          class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
          :class="[
            focusedIndex === 1 ? 'bg-blue-600 text-white' : 'text-zinc-300 hover:bg-zinc-800',
            selectedMode === 'stream' && focusedIndex !== 1 ? 'text-purple-400' : '',
          ]"
          @click="selectOption('stream')"
          @mouseenter="focusedIndex = 1"
        >
          <SignalIcon class="size-5" />
          <span class="font-medium">Stream</span>
          <span v-if="streaming" class="text-sm opacity-60 ml-auto">Active</span>
        </button>
      </div>
    </Transition>

    <!-- Click-away overlay -->
    <div v-if="menuOpen" class="fixed inset-0 z-40" @click="closeMenu" />
  </div>
</template>

<script setup lang="ts">
import { devLog } from "~/composables/dev-mode";
import { PlayIcon } from "@heroicons/vue/20/solid";
import { ChevronDownIcon, SignalIcon } from "@heroicons/vue/20/solid";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useFocusNavigation } from "~/composables/focus-navigation";

const props = defineProps<{
  registerAction?: (el: any, opts: { onSelect: () => void; onContext?: () => void }) => void;
  streaming?: boolean;
}>();

const emit = defineEmits<{
  play: [];
  stream: [];
}>();

type Mode = "play" | "stream";

devLog("state","[BPM:PLAY-DROPDOWN] Component setup");

const selectedMode = ref<Mode>("play");
const menuOpen = ref(false);
const focusedIndex = ref(0);

const focusNav = useFocusNavigation();
let lockId = 0;
const gamepad = useGamepad();
const unsubs: (() => void)[] = [];

const actionLabel = computed(() =>
  selectedMode.value === "play" ? "Play" : "Stream",
);

const buttonClass = computed(() =>
  selectedMode.value === "play"
    ? "bg-blue-600 hover:bg-blue-400 text-white shadow-blue-600/20 hover:shadow-blue-500/30 hover:scale-105"
    : "bg-purple-600 hover:bg-purple-500 text-white shadow-purple-600/20 hover:shadow-purple-500/30 hover:scale-105",
);
const chevronClass = computed(() =>
  selectedMode.value === "play"
    ? "bg-blue-700 hover:bg-blue-500 text-white border-blue-500/30"
    : "bg-purple-700 hover:bg-purple-500 text-white border-purple-500/30",
);

const MODES: Mode[] = ["play", "stream"];

function executeAction() {
  if (menuOpen.value) {
    selectOption(MODES[focusedIndex.value]);
    return;
  }
  if (selectedMode.value === "play") {
    emit("play");
  } else {
    emit("stream");
  }
}

function toggleMenu() {
  if (menuOpen.value) {
    closeMenu();
  } else {
    openMenu();
  }
}

function openMenu() {
  menuOpen.value = true;
  focusedIndex.value = MODES.indexOf(selectedMode.value);
  if (focusedIndex.value < 0) focusedIndex.value = 0;
  lockId = focusNav.acquireInputLock();
  wireGamepad();
}

function closeMenu() {
  menuOpen.value = false;
  unwireGamepad();
  focusNav.releaseInputLock(lockId);
}

function selectOption(mode: Mode) {
  selectedMode.value = mode;
  closeMenu();
  // Execute the selected action immediately
  if (mode === "play") {
    emit("play");
  } else {
    emit("stream");
  }
}

function wireGamepad() {
  unwireGamepad();

  const bypass = { bypassInputLock: true };

  unsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!menuOpen.value) return;
      focusedIndex.value = Math.max(0, focusedIndex.value - 1);
    }, bypass),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!menuOpen.value) return;
      focusedIndex.value = Math.min(MODES.length - 1, focusedIndex.value + 1);
    }, bypass),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!menuOpen.value) return;
      selectOption(MODES[focusedIndex.value]);
    }, bypass),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!menuOpen.value) return;
      closeMenu();
    }, bypass),
  );
}

function unwireGamepad() {
  for (const unsub of unsubs) unsub();
  unsubs.length = 0;
}

onUnmounted(() => {
  unwireGamepad();
  if (menuOpen.value) {
    focusNav.releaseInputLock(lockId);
  }
});
</script>

<style scoped>
.dropdown-enter-active,
.dropdown-leave-active {
  transition: all 0.15s ease;
}
.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}
</style>
