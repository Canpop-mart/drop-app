<template>
  <Teleport to="body">
    <Transition name="bp-dialog">
      <div
        v-if="visible"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        @click.self="handleCancel"
      >
        <div
          class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-8 max-w-lg w-full mx-4"
        >
          <!-- Title -->
          <h2
            v-if="title"
            class="text-xl font-semibold font-display text-zinc-100 mb-2"
          >
            {{ title }}
          </h2>

          <!-- Message -->
          <p v-if="message" class="text-zinc-400 mb-6">
            {{ message }}
          </p>

          <!-- Custom content slot -->
          <slot />

          <!-- Buttons -->
          <div class="flex items-center justify-end gap-3 mt-6">
            <button
              v-if="showCancel"
              ref="cancelBtn"
              class="flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="[
                focusedButton === 'cancel'
                  ? 'bg-zinc-700 text-zinc-100 ring-2 ring-blue-500'
                  : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200',
              ]"
              @click="handleCancel"
            >
              <span
                class="inline-block px-1 py-0.5 bg-red-700/60 text-red-300 rounded text-xs mr-1"
                >B</span
              >
              {{ cancelLabel }}
            </button>

            <button
              ref="confirmBtn"
              class="flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="[
                focusedButton === 'confirm'
                  ? 'bg-blue-600 text-white ring-2 ring-blue-400 shadow-lg shadow-blue-500/30'
                  : 'bg-blue-600/80 text-blue-100 hover:bg-blue-600',
                destructive
                  ? focusedButton === 'confirm'
                    ? 'bg-red-600 ring-red-400 shadow-red-500/30'
                    : 'bg-red-600/80 text-red-100 hover:bg-red-600'
                  : '',
              ]"
              @click="handleConfirm"
            >
              <span
                class="inline-block px-1 py-0.5 bg-green-700/60 text-green-300 rounded text-xs mr-1"
                >A</span
              >
              {{ confirmLabel }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useFocusNavigation } from "~/composables/focus-navigation";

const focusNav = useFocusNavigation();
let lockId = 0;

const props = withDefaults(
  defineProps<{
    visible: boolean;
    title?: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    showCancel?: boolean;
    destructive?: boolean;
  }>(),
  {
    confirmLabel: "Confirm",
    cancelLabel: "Cancel",
    showCancel: true,
    destructive: false,
  },
);

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const focusedButton = ref<"confirm" | "cancel">("confirm");
const gamepad = useGamepad();
const unsubs: (() => void)[] = [];

function handleConfirm() {
  emit("confirm");
}

function handleCancel() {
  emit("cancel");
}

watch(
  () => props.visible,
  (v) => {
    if (v) {
      focusedButton.value = "confirm";
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

  // M6 fix: D-pad left/right AND up/down to switch focus between buttons
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadLeft, () => {
      if (!props.visible) return;
      if (props.showCancel) focusedButton.value = "cancel";
    }),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadRight, () => {
      if (!props.visible) return;
      focusedButton.value = "confirm";
    }),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!props.visible) return;
      if (props.showCancel) focusedButton.value = "cancel";
    }),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!props.visible) return;
      focusedButton.value = "confirm";
    }),
  );

  // A = select focused button
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!props.visible) return;
      if (focusedButton.value === "confirm") handleConfirm();
      else handleCancel();
    }),
  );

  // B = cancel
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!props.visible) return;
      handleCancel();
    }),
  );
}

function unwireGamepad() {
  for (const unsub of unsubs) unsub();
  unsubs.length = 0;
}

onUnmounted(() => {
  unwireGamepad();
  if (props.visible) {
    focusNav.releaseInputLock(lockId);
  }
});
</script>

<style scoped>
.bp-dialog-enter-active,
.bp-dialog-leave-active {
  transition: opacity 0.2s ease;
}

.bp-dialog-enter-active > div,
.bp-dialog-leave-active > div {
  transition:
    transform 0.2s ease,
    opacity 0.2s ease;
}

.bp-dialog-enter-from,
.bp-dialog-leave-to {
  opacity: 0;
}

.bp-dialog-enter-from > div {
  transform: scale(0.95);
  opacity: 0;
}

.bp-dialog-leave-to > div {
  transform: scale(0.95);
  opacity: 0;
}
</style>
