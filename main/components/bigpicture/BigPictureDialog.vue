<template>
  <Teleport to="body">
    <Transition name="bp-dialog">
      <div
        v-if="visible"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/70"
        :class="{ 'backdrop-blur-sm': !reducedMotion }"
        @click.self="handleCancel"
      >
        <div
          class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-8 max-w-lg w-full mx-4"
        >
          <h2
            v-if="title"
            class="text-xl font-semibold font-display text-zinc-100 mb-2"
          >
            {{ title }}
          </h2>

          <p v-if="message" class="text-zinc-400 mb-6">
            {{ message }}
          </p>

          <slot />

          <div class="flex items-center justify-end gap-3 mt-6">
            <button
              v-if="showCancel"
              class="flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium bg-zinc-800 text-zinc-300 hover:bg-zinc-700 hover:text-zinc-100 transition-colors"
              @click="handleCancel"
            >
              <BigPictureButtonPrompt button="B" :label="cancelLabel" size="sm" />
            </button>

            <button
              class="flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="
                destructive
                  ? 'bg-red-600 text-white hover:bg-red-500'
                  : 'bg-blue-600 text-white hover:bg-blue-500'
              "
              @click="handleConfirm"
            >
              <BigPictureButtonPrompt button="A" :label="confirmLabel" size="sm" />
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useReducedMotion } from "~/composables/bp-reduced-motion";

const { reducedMotion } = useReducedMotion();
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
  // Direct mapping — A=confirm, B=cancel. No D-pad focus.
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!props.visible) return;
      handleConfirm();
    }),
  );
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
