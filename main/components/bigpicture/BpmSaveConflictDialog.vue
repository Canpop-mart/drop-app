<template>
  <Teleport to="body">
    <Transition name="bp-dialog">
      <div
        v-if="visible"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
      >
        <div
          class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-8 max-w-lg w-full mx-4"
        >
          <h2 class="text-xl font-semibold font-display text-zinc-100 mb-2">
            Cloud Save Conflict
          </h2>
          <p class="text-zinc-400 text-sm mb-4">
            Some saves were modified both locally and in the cloud.
          </p>

          <div ref="scrollContainer" class="space-y-3 max-h-72 overflow-y-auto pr-1">
            <div
              v-for="(conflict, i) in conflicts"
              :key="conflict.filename"
              :ref="(el: any) => { if (el) conflictRows[i] = el }"
              class="rounded-xl border p-4 transition-colors"
              :class="focusArea === 'conflicts' && focusedRow === i
                ? 'border-blue-500/60 bg-zinc-800/70'
                : 'border-zinc-700/50 bg-zinc-800/40'"
            >
              <div class="flex items-center justify-between mb-3">
                <span class="text-sm font-medium text-zinc-200 truncate">
                  {{ conflict.filename }}
                </span>
                <span
                  class="text-xs px-2 py-0.5 rounded-full"
                  :class="
                    conflict.saveType === 'save'
                      ? 'bg-blue-500/20 text-blue-300'
                      : conflict.saveType === 'state'
                        ? 'bg-purple-500/20 text-purple-300'
                        : 'bg-green-500/20 text-green-300'
                  "
                >
                  {{ conflict.saveType }}
                </span>
              </div>

              <div class="grid grid-cols-2 gap-3">
                <!-- Local -->
                <button
                  class="rounded-xl border p-3 text-left transition-all focus:outline-none"
                  :class="
                    choices[i] === 'keep_local'
                      ? 'border-blue-500 bg-blue-500/10 ring-2 ring-blue-500/50'
                      : 'border-zinc-700 bg-zinc-800 hover:border-zinc-500'
                  "
                  @click="choices[i] = 'keep_local'"
                >
                  <div class="text-sm font-medium text-zinc-200 mb-1">This PC</div>
                  <div class="text-xs text-zinc-400">
                    {{ formatSize(conflict.localSize) }} &middot;
                    {{ formatDate(conflict.localModifiedAt * 1000) }}
                  </div>
                </button>

                <!-- Cloud -->
                <button
                  class="rounded-xl border p-3 text-left transition-all focus:outline-none"
                  :class="
                    choices[i] === 'keep_cloud'
                      ? 'border-blue-500 bg-blue-500/10 ring-2 ring-blue-500/50'
                      : 'border-zinc-700 bg-zinc-800 hover:border-zinc-500'
                  "
                  @click="choices[i] = 'keep_cloud'"
                >
                  <div class="text-sm font-medium text-zinc-200 mb-1">Cloud</div>
                  <div class="text-xs text-zinc-400">
                    {{ formatSize(conflict.cloudSize) }} &middot;
                    {{ formatDate(conflict.cloudModifiedAt) }}
                  </div>
                  <div
                    v-if="conflict.cloudUploadedFrom"
                    class="text-xs text-zinc-500 mt-0.5"
                  >
                    from {{ conflict.cloudUploadedFrom }}
                  </div>
                </button>
              </div>
            </div>
          </div>

          <!-- Buttons -->
          <div class="flex items-center justify-end gap-3 mt-6">
            <button
              class="flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="focusArea === 'buttons' && focusedButton === 'cancel'
                ? 'bg-zinc-700 text-zinc-100 ring-2 ring-blue-500'
                : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200'"
              @click="keepAllLocal"
            >
              <BigPictureButtonPrompt button="B" label="Keep All Local" size="sm" />
            </button>
            <button
              class="flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium transition-colors"
              :class="focusArea === 'buttons' && focusedButton === 'confirm'
                ? 'bg-blue-600 text-white ring-2 ring-blue-400 shadow-lg shadow-blue-500/30'
                : 'bg-blue-600/80 text-blue-100 hover:bg-blue-600'"
              @click="submit"
            >
              <BigPictureButtonPrompt button="A" label="Continue" size="sm" />
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
import { invoke } from "@tauri-apps/api/core";
import type { SaveConflict } from "~/types/save-sync";

const props = defineProps<{
  visible: boolean;
  gameId: string;
  conflicts: SaveConflict[];
}>();

const emit = defineEmits<{
  resolved: [];
}>();

const choices = ref<string[]>([]);
// Navigation state: "conflicts" area (rows) or "buttons" area (confirm/cancel)
const focusArea = ref<"conflicts" | "buttons">("conflicts");
const focusedRow = ref(0);
const focusedButton = ref<"confirm" | "cancel">("confirm");

const scrollContainer = ref<HTMLElement | null>(null);
const conflictRows = ref<Record<number, HTMLElement>>({});

const focusNav = useFocusNavigation();
const gamepad = useGamepad();
let lockId = 0;
const unsubs: (() => void)[] = [];

// Auto-scroll focused conflict row into view
watch(focusedRow, () => {
  nextTick(() => {
    const row = conflictRows.value[focusedRow.value];
    if (row) {
      row.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  });
});

watch(
  () => props.conflicts,
  (val) => {
    choices.value = val.map(() => "keep_local");
    focusedRow.value = 0;
    focusArea.value = val.length > 0 ? "conflicts" : "buttons";
  },
  { immediate: true },
);

watch(
  () => props.visible,
  (v) => {
    if (v) {
      focusArea.value = props.conflicts.length > 0 ? "conflicts" : "buttons";
      focusedRow.value = 0;
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

  // DPad Up — move up through rows, then wrap to buttons
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!props.visible) return;
      if (focusArea.value === "buttons") {
        // Jump back to last conflict row
        if (props.conflicts.length > 0) {
          focusArea.value = "conflicts";
          focusedRow.value = props.conflicts.length - 1;
        }
      } else if (focusedRow.value > 0) {
        focusedRow.value--;
      }
    }),
  );

  // DPad Down — move down through rows, then to buttons
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!props.visible) return;
      if (focusArea.value === "conflicts") {
        if (focusedRow.value < props.conflicts.length - 1) {
          focusedRow.value++;
        } else {
          // Past last row → jump to buttons
          focusArea.value = "buttons";
          focusedButton.value = "confirm";
        }
      } else {
        // Already in buttons — no-op (or wrap to top)
      }
    }),
  );

  // DPad Left/Right — toggle local/cloud for focused row, or switch buttons
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadLeft, () => {
      if (!props.visible) return;
      if (focusArea.value === "conflicts") {
        choices.value[focusedRow.value] = "keep_local";
      } else {
        focusedButton.value = "cancel";
      }
    }),
  );
  unsubs.push(
    gamepad.onButton(GamepadButton.DPadRight, () => {
      if (!props.visible) return;
      if (focusArea.value === "conflicts") {
        choices.value[focusedRow.value] = "keep_cloud";
      } else {
        focusedButton.value = "confirm";
      }
    }),
  );

  // A = confirm action
  unsubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!props.visible) return;
      if (focusArea.value === "buttons") {
        if (focusedButton.value === "confirm") submit();
        else keepAllLocal();
      } else {
        // In conflict rows, A jumps down to confirm button
        focusArea.value = "buttons";
        focusedButton.value = "confirm";
      }
    }),
  );

  // B = keep all local / cancel
  unsubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!props.visible) return;
      keepAllLocal();
    }),
  );
}

function unwireGamepad() {
  for (const u of unsubs) u();
  unsubs.length = 0;
}

onUnmounted(() => {
  unwireGamepad();
  if (props.visible) {
    focusNav.releaseInputLock(lockId);
  }
});

function keepAllLocal() {
  choices.value = props.conflicts.map(() => "keep_local");
  submit();
}

async function submit() {
  const resolutions = props.conflicts.map((c, i) => ({
    filename: c.filename,
    choice: choices.value[i],
  }));

  try {
    await invoke("resolve_save_conflicts", {
      payload: {
        gameId: props.gameId,
        resolutions,
      },
    });
  } catch (e) {
    console.warn("[SAVE-SYNC] Failed to send conflict resolutions:", e);
  }

  emit("resolved");
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(input: string | number): string {
  try {
    const d = new Date(input);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return String(input);
  }
}
</script>

<style scoped>
.bp-dialog-enter-active,
.bp-dialog-leave-active {
  transition: opacity 0.2s ease;
}
.bp-dialog-enter-active > div,
.bp-dialog-leave-active > div {
  transition: transform 0.2s ease, opacity 0.2s ease;
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
