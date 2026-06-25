<template>
  <button
    v-if="isInstalled"
    class="transition-transform duration-300 hover:scale-105 active:scale-95 inline-flex gap-x-2 items-center rounded-md bg-zinc-800/50 px-6 font-semibold text-white shadow-xl backdrop-blur-sm hover:bg-zinc-800/80 uppercase font-display disabled:opacity-50 disabled:cursor-progress"
    :disabled="testing"
    :title="
      testing
        ? `Running test (${elapsedLabel})...`
        : 'Launch the game with a 45 second observation window, classify the result, and report it to the server. Promotes alive results based on whether the menu rendered.'
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

/**
 * Ask the user whether the menu actually rendered, returning their answer
 * via the existing app modal infrastructure (NOT `window.confirm`, which
 * has well-documented unreliability inside Tauri's WebView2 on Windows —
 * it can auto-dismiss or return without showing).
 *
 * Resolves to `true` if the user clicked "Yes, plays correctly", `false`
 * otherwise. Skipping the confirmation is treated as "not rendered" so
 * we don't accidentally promote stuck-on-black-screen games.
 */
function askDidItRender(): Promise<boolean> {
  return new Promise((resolve) => {
    createModal(
      ModalType.Confirmation,
      {
        title: "Did the game render?",
        description:
          "The game launched and stayed alive for the full observation window. " +
          "Did you actually see the menu / game UI render correctly on screen? " +
          'Confirm to mark this as "Plays correctly", or cancel to leave it as "Launches but no render".',
        buttonText: "Yes, plays correctly",
      },
      (e, c) => {
        c();
        resolve(e === "confirm");
      },
    );
  });
}

async function runTest() {
  if (testing.value) return;
  testing.value = true;
  startTicker();

  try {
    // Default options: 45s observation, auto-kill after, no extra notes.
    const outcome = await invoke<CompatTestOutcome>("start_compat_test", {
      gameId,
      versionIndex: 0,
      options: {
        timeoutSecs: 45,
        leaveRunning: false,
      },
    });

    // Final status starts as the orchestrator's classification but may
    // get promoted to AliveRenders by the user's answer below. We delay
    // the toast until after the confirm so it reflects the final state.
    let finalStatus = outcome.status;

    if (outcome.status === "AliveNoRender") {
      const rendered = await askDidItRender();
      try {
        await invoke("confirm_compat_render", {
          gameId,
          rendered,
        });
        if (rendered) finalStatus = "AliveRenders";
      } catch (err) {
        console.warn("confirm_compat_render failed:", err);
      }
    }

    emit("result", { ...outcome, status: finalStatus });

    if (!outcome.posted) {
      console.warn(
        `[compat] test result for ${gameId} did not reach the server`,
        outcome,
      );
    }

    // Refresh the cached summary so the per-game CompatPanel and any
    // other components watching `useCompatSummary()` reflect the new
    // result without requiring a hard page reload. Failure to refresh
    // is non-fatal — the data is still on the server, the UI will
    // just lag until the next manual reload.
    try {
      await refreshCompatSummary();
    } catch (e) {
      console.warn("[compat] post-test summary refresh failed:", e);
    }
  } catch (err) {
    console.error("compat test failed:", err);
    createModal(
      ModalType.Notification,
      {
        title: "Compatibility test failed",
        description: `The test couldn't run: ${
          err instanceof Error ? err.message : String(err)
        }`,
        buttonText: "Close",
      },
      (e, c) => c(),
    );
  } finally {
    stopTicker();
    testing.value = false;
  }
}

onUnmounted(stopTicker);
</script>
