<template>
  <Teleport to="body">
    <!-- Steam OSK mode: render a focused native input so the Steam
         on-screen keyboard has somewhere to type into. Our custom key
         UI isn't shown. -->
    <Transition name="kb-slide">
      <div
        v-if="visible && steamOskMode"
        class="fixed inset-x-0 bottom-0 z-[60] flex flex-col items-center pb-6 bg-gradient-to-t from-zinc-950 via-zinc-950/95 to-transparent pt-8"
      >
        <div class="w-full max-w-4xl px-6 py-3 bg-zinc-900 rounded-xl border border-blue-500/50 shadow-lg">
          <input
            ref="steamInputRef"
            :value="modelValue"
            :placeholder="placeholder"
            class="w-full bg-transparent text-zinc-100 text-lg font-medium outline-none placeholder-zinc-600"
            @input="(e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value)"
            @keydown.enter.prevent="emit('submit')"
            @keydown.escape.prevent="emit('close')"
          />
        </div>
        <div class="flex items-center gap-6 mt-2 text-xs text-zinc-500">
          <span>Use the Steam keyboard to type</span>
          <div class="flex-1" />
          <BigPictureButtonPrompt button="B" label="Close" size="sm" />
        </div>
      </div>
    </Transition>

    <!-- Custom keyboard mode -->
    <Transition name="kb-slide">
      <div
        v-if="visible && !steamOskMode"
        class="fixed inset-x-0 bottom-0 z-[60] flex flex-col items-center pb-4 bg-gradient-to-t from-zinc-950 via-zinc-950/95 to-transparent pt-8"
      >
        <!-- Input preview -->
        <div
          class="w-full max-w-4xl mb-4 px-6 py-3 bg-zinc-900 rounded-xl border border-zinc-700/50 text-zinc-100 text-lg font-medium min-h-[3rem] flex items-center"
        >
          <span v-if="modelValue">{{ modelValue }}</span>
          <span v-else class="text-zinc-600">{{ placeholder }}</span>
          <span class="animate-pulse ml-0.5 text-blue-400">|</span>
        </div>

        <!-- Keyboard rows -->
        <div class="flex flex-col gap-1.5 max-w-4xl w-full">
          <div
            v-for="(row, rowIdx) in currentLayout"
            :key="rowIdx"
            class="flex justify-center gap-1.5"
          >
            <button
              v-for="(key, keyIdx) in row"
              :key="keyIdx"
              :ref="(el: any) => registerKey(el as HTMLElement, rowIdx, keyIdx)"
              class="flex items-center justify-center rounded-lg text-sm font-medium transition-all duration-100 select-none"
              :class="[
                keyClass(key),
                focusedRow === rowIdx && focusedCol === keyIdx
                  ? 'bg-blue-600 text-white scale-105 shadow-lg shadow-blue-500/30'
                  : 'bg-zinc-800/80 text-zinc-300 hover:bg-zinc-700',
              ]"
              @click="pressKey(key)"
            >
              {{ keyLabel(key) }}
            </button>
          </div>
        </div>

        <!-- Paste + hints row -->
        <div class="flex items-center gap-6 mt-3 text-xs text-zinc-500">
          <button
            type="button"
            class="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-zinc-800/80 text-zinc-300 hover:bg-zinc-700 transition-colors"
            :class="{ 'ring-2 ring-green-400': pasteFlash }"
            @click="paste"
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="9" y="2" width="6" height="4" rx="1" />
              <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
            </svg>
            <span>Paste</span>
          </button>
          <span
            v-if="pasteError"
            class="text-red-400 text-[11px]"
          >{{ pasteError }}</span>
          <div class="flex-1" />
          <BigPictureButtonPrompt button="A" label="Type" size="sm" />
          <BigPictureButtonPrompt button="B" label="Close" size="sm" />
          <BigPictureButtonPrompt button="X" label="Backspace" size="sm" />
          <BigPictureButtonPrompt button="Y" label="Space" size="sm" />
          <BigPictureButtonPrompt button="LT" label="Paste" size="sm" />
          <BigPictureButtonPrompt button="LB" label="Shift" size="sm" />
          <BigPictureButtonPrompt button="RB" label="Submit" size="sm" />
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useDeckMode } from "~/composables/deck-mode";
import { invoke } from "@tauri-apps/api/core";

const props = defineProps<{
  visible: boolean;
  modelValue: string;
  placeholder?: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  close: [];
  submit: [];
}>();

// ── Keyboard layouts ──────────────────────────────────────────────────────

const LOWER = [
  ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
  ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
  ["a", "s", "d", "f", "g", "h", "j", "k", "l"],
  ["z", "x", "c", "v", "b", "n", "m"],
];

const UPPER = [
  ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")"],
  ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
  ["A", "S", "D", "F", "G", "H", "J", "K", "L"],
  ["Z", "X", "C", "V", "B", "N", "M"],
];

const shifted = ref(false);
const currentLayout = computed(() => (shifted.value ? UPPER : LOWER));

const focusedRow = ref(1); // Start on letter row
const focusedCol = ref(0);

// ── Key refs for scrolling ────────────────────────────────────────────────

const keyRefs = new Map<string, HTMLElement>();

function registerKey(el: HTMLElement | null, row: number, col: number) {
  if (el) {
    keyRefs.set(`${row}-${col}`, el);
  }
}

// ── Key helpers ───────────────────────────────────────────────────────────

function keyLabel(key: string): string {
  return key;
}

function keyClass(key: string): string {
  // All keys get the same base width — sized for Steam Deck readability
  return "w-16 h-14 text-base";
}

function pressKey(key: string) {
  emit("update:modelValue", props.modelValue + key);
}

function backspace() {
  if (props.modelValue.length > 0) {
    emit("update:modelValue", props.modelValue.slice(0, -1));
  }
}

function space() {
  emit("update:modelValue", props.modelValue + " ");
}

const pasteFlash = ref(false);
const pasteError = ref("");

async function paste() {
  pasteError.value = "";
  try {
    const text =
      typeof navigator !== "undefined" && navigator.clipboard
        ? await navigator.clipboard.readText()
        : "";
    if (!text) {
      pasteError.value = "Clipboard empty";
      return;
    }
    emit("update:modelValue", props.modelValue + text);
    pasteFlash.value = true;
    setTimeout(() => (pasteFlash.value = false), 600);
  } catch (e) {
    // Clipboard access can be blocked by the browser (insecure context,
    // permissions, or gamescope sandboxing). Give the user a hint instead
    // of silently failing.
    pasteError.value = "Clipboard unavailable";
    console.warn("[BPM:KB] clipboard paste failed:", e);
  }
}

// ── Gamepad wiring ────────────────────────────────────────────────────────

const gamepad = useGamepad();
const focusNav = useFocusNavigation();
const { isGamescope: _isGS } = useDeckMode();
const unsubs: (() => void)[] = [];

// On Gamescope (Steam Deck), physical X reports as West and physical Y as
// North — swapped from standard.  We swap which logical button triggers
// backspace vs space so the physical buttons match the on-screen labels.
const _bkspBtn = _isGS.value ? GamepadButton.North : GamepadButton.West;
const _spaceBtn = _isGS.value ? GamepadButton.West : GamepadButton.North;

// Note: Previously tried invoking steam://open/keyboard on SteamOS but
// the Tauri webview can't navigate to steam:// protocol URLs. Our custom
// on-screen keyboard works reliably on all platforms including Steam Deck.

function clampFocus() {
  const layout = currentLayout.value;
  if (focusedRow.value >= layout.length) focusedRow.value = layout.length - 1;
  if (focusedRow.value < 0) focusedRow.value = 0;
  const row = layout[focusedRow.value];
  if (focusedCol.value >= row.length) focusedCol.value = row.length - 1;
  if (focusedCol.value < 0) focusedCol.value = 0;
}

let kbLockId = 0;
const steamOskMode = ref(false);
const steamInputRef = ref<HTMLInputElement | null>(null);

function shouldUseSteamOsk(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem("bpm:keyboardMode") === "steam";
}

async function openSteamOSK(): Promise<boolean> {
  try {
    await invoke("open_steam_keyboard");
    return true;
  } catch (e) {
    console.warn("[BPM:KB] Steam OSK unavailable, falling back to custom:", e);
    return false;
  }
}

watch(
  () => props.visible,
  async (v) => {
    if (v) {
      if (shouldUseSteamOsk()) {
        // Render a focused native input so Steam's OSK has a target.
        // If the OSK invoke fails, fall back to the custom keyboard.
        steamOskMode.value = true;
        await nextTick();
        steamInputRef.value?.focus();
        const ok = await openSteamOSK();
        if (!ok) {
          steamOskMode.value = false;
          focusedRow.value = 1;
          focusedCol.value = 0;
          kbLockId = focusNav.acquireInputLock();
          wireGamepad();
        } else {
          // Still handle B-to-close on the gamepad while Steam OSK is up.
          kbLockId = focusNav.acquireInputLock();
          wireGamepadSteamMode();
        }
        return;
      }
      steamOskMode.value = false;
      focusedRow.value = 1;
      focusedCol.value = 0;
      kbLockId = focusNav.acquireInputLock();
      wireGamepad();
    } else {
      unwireGamepad();
      focusNav.releaseInputLock(kbLockId);
      steamOskMode.value = false;
    }
  },
);

function wireGamepad() {
  unwireGamepad();

  const bypass = { bypassInputLock: true };

  // D-pad navigation
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!props.visible) return;
      focusedRow.value--;
      clampFocus();
    }, bypass),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!props.visible) return;
      focusedRow.value++;
      clampFocus();
    }, bypass),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadLeft, () => {
      if (!props.visible) return;
      focusedCol.value--;
      clampFocus();
    }, bypass),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadRight, () => {
      if (!props.visible) return;
      focusedCol.value++;
      clampFocus();
    }, bypass),
  );

  // A = type focused key
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!props.visible) return;
      const key = currentLayout.value[focusedRow.value]?.[focusedCol.value];
      if (key) pressKey(key);
    }, bypass),
  );

  // B = close
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!props.visible) return;
      emit("close");
    }, bypass),
  );

  // X = backspace (swapped on Gamescope so physical button matches label)
  unsubs.push(
    gamepad.onButton(_bkspBtn, () => {
      if (!props.visible) return;
      backspace();
    }, bypass),
  );

  // Y = space (swapped on Gamescope so physical button matches label)
  unsubs.push(
    gamepad.onButton(_spaceBtn, () => {
      if (!props.visible) return;
      space();
    }, bypass),
  );

  // LB = toggle shift
  unsubs.push(
    gamepad.onButton(GamepadButton.LeftBumper, () => {
      if (!props.visible) return;
      shifted.value = !shifted.value;
    }, bypass),
  );

  // RB = submit
  unsubs.push(
    gamepad.onButton(GamepadButton.RightBumper, () => {
      if (!props.visible) return;
      emit("submit");
    }, bypass),
  );

  // LT = paste from clipboard
  unsubs.push(
    gamepad.onButton(GamepadButton.LeftTrigger, () => {
      if (!props.visible) return;
      paste();
    }, bypass),
  );
}

function wireGamepadSteamMode() {
  unwireGamepad();
  // Only B = close in steam mode — let the OSK handle everything else.
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!props.visible) return;
      emit("close");
    }, { bypassInputLock: true }),
  );
}

function unwireGamepad() {
  for (const unsub of unsubs) unsub();
  unsubs.length = 0;
}

onUnmounted(() => {
  unwireGamepad();
  // L3 fix: only release lock if still held (visible). The watch handler
  // already releases when visible→false, so this prevents a double-release
  // that could accidentally unlock a newer lock owner.
  if (props.visible) {
    focusNav.releaseInputLock(kbLockId);
  }
});
</script>

<style scoped>
.kb-slide-enter-active,
.kb-slide-leave-active {
  transition:
    transform 0.3s ease,
    opacity 0.3s ease;
}

.kb-slide-enter-from,
.kb-slide-leave-to {
  transform: translateY(100%);
  opacity: 0;
}
</style>