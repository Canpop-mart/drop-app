<template>
  <button
    v-if="isInstalled"
    class="transition-transform duration-300 hover:scale-105 active:scale-95 inline-flex gap-x-2 items-center rounded-md bg-zinc-800/50 px-6 font-semibold text-white shadow-xl backdrop-blur-sm hover:bg-zinc-800/80 uppercase font-display disabled:opacity-50 disabled:cursor-progress"
    :disabled="testing"
    :title="
      testing
        ? `Running test (${elapsedLabel})...`
        : 'Launch the game with a 45 second observation window, classify the result, and report it to the server. Promotes \`alive\` results based on whether the menu rendered.'
    "
    @click="runTest"
  >
    <BeakerIcon class="size-5" :class="{ 'animate-pulse': testing }" />
    {{ testing ? `Testing… ${elapsedLabel}` : "Test compatibility" }}
  </button>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { BeakerIcon } from "@heroicons/vue/24/solid";

const { gameId, isInstalled } = defineProps<{
  gameId: string;
  isInstalled: boolean;
}>();

const emit = defineEmits<{
  (e: "result", payload: CompatTestOutcome): void;
}>();

type CompatTestOutcome = {
  /**
   * Mirrors the server's `GameCompatibilityStatus` enum. The Rust side
   * serializes one of: AliveRenders | AliveNoRender | EarlyExit | Crash
   * | NoLaunch | InstallFailed.
   */
  status: string;
  signature: string | null;
  elapsedSecs: number;
  posted: boolean;
};

const testing = ref(false);
const startedAt = ref<number | null>(null);
const tickHandle = ref<ReturnType<typeof setInterval> | null>(null);
const elapsedSecs = ref(0);

const elapsedLabel = computed(() => {
  const s = elapsedSecs.value;
  if (s < 60) return `${s}s`;
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, "0")}`;
});

function startTicker() {
  startedAt.value = Date.now();
  elapsedSecs.value = 0;
  tickHandle.value = setInterval(() => {
    if (!startedAt.value) return;
    elapsedSecs.value = Math.floor((Date.now() - startedAt.value) / 1000);
  }, 1000);
}

function stopTicker() {
  if (tickHandle.value) {
    clearInterval(tickHandle.value);
    tickHandle.value = null;
  }
  startedAt.value = null;
}

async function runTest() {
  if (testing.value) return;
  testing.value = true;
  startTicker();

  try {
    // Default options: 45s observation, auto-kill after, no extra notes.
    // The user gets to promote the result to AliveRenders via a follow-up
    // dialog if the process stayed alive — see below.
    const outcome = await invoke<CompatTestOutcome>("start_compat_test", {
      gameId,
      versionIndex: 0,
      options: {
        timeoutSecs: 45,
        leaveRunning: false,
      },
    });

    emit("result", outcome);

    // Render-confirm dialog: only fires when the process was alive at the
    // end of the observation window. The user has eyes on the screen, so
    // their "did the menu render?" answer is the truth — we promote the
    // server-side status accordingly.
    if (outcome.status === "AliveNoRender") {
      const rendered = window.confirm(
        "Test complete. The game launched and stayed alive for 45s.\n\n" +
          "Did you actually see the main menu / game UI render correctly on screen?\n\n" +
          'OK = Yes, mark as "Plays correctly".\nCancel = No, leave as "Launches but no render".',
      );
      try {
        await invoke("confirm_compat_render", {
          gameId,
          rendered,
        });
      } catch (err) {
        console.warn("confirm_compat_render failed:", err);
      }
    }

    if (!outcome.posted) {
      console.warn(
        `[compat] test result for ${gameId} did not reach the server`,
        outcome,
      );
    }
  } catch (err) {
    console.error("compat test failed:", err);
    window.alert(
      `Compat test failed to run: ${
        err instanceof Error ? err.message : String(err)
      }`,
    );
  } finally {
    stopTicker();
    testing.value = false;
  }
}

onUnmounted(stopTicker);
</script>
